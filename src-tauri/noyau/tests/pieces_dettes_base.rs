//! Quatre papiers qui disaient faux (ALERTES, 27/10/2026), sur les deux
//! moteurs :
//!
//! - une facture reglee en partie par avoir imprimait cette part comme
//!   due (`lire_donnees_piece` excluait les paiements `mode = 'avoir'`) ;
//! - une facture fournisseur encore en brouillon comptait comme une
//!   dette, et un reglement global la passait en « emis » ;
//! - un en-tete CSV incomplet retombait sur les positions v1.3 ;
//! - le recu d'une vente sans piece lisait la fiche cliente d'aujourd'hui.
//!
//! Stock, caisse et creance verifies apres chaque geste.
//!
//!   cargo test -p gescom-noyau --test pieces_dettes_base
//!   GESCOM_PG=postgresql://…/gescom_test cargo test -p gescom-noyau --test pieces_dettes_base -- --test-threads=1

mod commun;

use commun::*;
use gescom_noyau::argent::{self, ParamsLigneInput};
use gescom_noyau::base::Base;
use gescom_noyau::parametres;
use gescom_noyau::{achats, avoirs, catalogue_csv, chantiers, comptoir, creances, fournisseurs, fournisseurs_csv, pagination, passage, pieces};

fn ligne_vente(art: &(String, String, f64, i64), depot: &str, quantite: f64) -> ParamsLigneInput {
    ParamsLigneInput {
        article_id: art.0.clone(),
        unite_vente_id: art.1.clone(),
        depot_source_id: depot.to_string(),
        source_approvisionnement: "stock".into(),
        quantite,
        facteur: art.2,
        prix_reference: art.3,
        prix_pratique: art.3,
        taux_tva: None,
        a_decouvert: None,
    }
}

fn entrees_caisse(base: &mut Base) -> i64 {
    let dossier = base.dossier().to_string();
    compter(
        base,
        "SELECT CAST(COALESCE(SUM(montant), 0) AS BIGINT) FROM mouvement_caisse
         WHERE sens = 'entree' AND dossier_id = ?1",
        &parametres![dossier],
    )
}

fn sorties_caisse(base: &mut Base) -> i64 {
    let dossier = base.dossier().to_string();
    compter(
        base,
        "SELECT CAST(COALESCE(SUM(montant), 0) AS BIGINT) FROM mouvement_caisse
         WHERE sens = 'sortie' AND dossier_id = ?1",
        &parametres![dossier],
    )
}

fn texte(base: &mut Base, sql: &str, params: &[gescom_noyau::base::Valeur]) -> Option<String> {
    base.lire_une(sql, params, |r| r.get::<Option<String>>(0)).unwrap().flatten()
}

// =====================================================================
//  1. La part reglee par avoir n'est plus imprimee comme due
// =====================================================================

/// Une facture de 10 kg de sucre validee a credit avec 2 kg d'acompte en
/// especes, puis 3 kg reglés par un avoir. Rend (piece_id, vente_id, total).
fn facture_reglee_en_partie_par_avoir(base: &mut Base, client: &str) -> (String, String, i64) {
    let sucre = article_unite(base, "Sucre");
    let total = 10 * sucre.3;
    let especes = 2 * sucre.3;
    let par_avoir = 3 * sucre.3;
    avoirs::accorder_avoir_client_sur_base(base, client.to_string(), par_avoir, "geste commercial".into(), None)
        .expect("avoir accorde");
    let fac = pieces::creer_piece_sur_base(
        base, client.to_string(), "facture".into(), vec![ligne(&sucre, 10.0)], None, None, None, None, None, None,
    )
    .unwrap();
    let piece_id = fac["id"].as_str().unwrap().to_string();
    argent::valider_facture_sur_base(base, piece_id.clone(), "credit".into(), Some("especes".into()), Some(especes), None)
        .expect("facture validee avec acompte");
    let dossier = base.dossier().to_string();
    let vente_id = texte(base, "SELECT id FROM vente WHERE piece_id = ?1 AND dossier_id = ?2", &parametres![piece_id.clone(), dossier])
        .expect("la vente");
    let applique = avoirs::appliquer_avoir_vente_sur_base(base, vente_id.clone(), client.to_string(), par_avoir).unwrap();
    assert_eq!(applique, par_avoir);
    (piece_id, vente_id, total)
}

#[test]
fn une_facture_reglee_en_partie_par_avoir_n_imprime_que_le_vrai_reste() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let client = client_reel(&mut base);
    let sucre = article_unite(&mut base, "Sucre");
    let depot = depot_defaut(&mut base);
    let stock_avant = stock(&mut base, &sucre.0, &depot);
    let caisse_avant = entrees_caisse(&mut base);

    let (piece_id, vente_id, total) = facture_reglee_en_partie_par_avoir(&mut base, &client);
    let (especes, par_avoir) = (2 * sucre.3, 3 * sucre.3);

    let d = pieces::lire_donnees_piece_sur_base(&mut base, piece_id).unwrap();
    let t = &d["totaux"];
    assert_eq!(t["total_ttc"], total);
    assert_eq!(t["total_paye"], especes + par_avoir, "l'avoir regle la facture : il est paye");
    assert_eq!(t["regle_par_avoir"], par_avoir, "{t}");
    assert_eq!(t["encaisse"], especes, "l'acompte imprime = l'argent verse seulement");
    assert_eq!(t["reste_du"], total - especes - par_avoir, "la part de l'avoir n'est plus due");

    // Apres le geste : la marchandise est sortie, seul l'acompte est
    // entre dans le tiroir, la creance est le vrai reste.
    assert_eq!(stock(&mut base, &sucre.0, &depot), stock_avant - 10.0);
    assert_eq!(entrees_caisse(&mut base) - caisse_avant, especes);
    let statut = texte(&mut base, "SELECT statut FROM vente WHERE id = ?1", &parametres![vente_id]).unwrap();
    assert_eq!(statut, "partiellement_payee");
    assert_eq!(avoirs::total_avoirs_client_sur_base(&mut base, client).unwrap(), 0, "l'avoir est consomme");
}

// =====================================================================
//  2. Une facture fournisseur en brouillon n'est pas une dette
// =====================================================================

#[test]
fn une_facture_fournisseur_en_brouillon_n_est_pas_une_dette() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let sucre = article_unite(&mut base, "Sucre");
    let depot = depot_defaut(&mut base);
    let f = fournisseur(&mut base, "Grossiste du marche");
    let stock_avant = stock(&mut base, &sucre.0, &depot);

    // Une vraie dette : un achat a credit de 10 kg a 500.
    achats::enregistrer_achat_sur_base(
        &mut base,
        Some(f.clone()),
        None,
        vec![achats::LigneAchat { article_id: sucre.0.clone(), unite_vente_id: sucre.1.clone(), quantite: 10.0, facteur: sucre.2, prix_achat: 500 }],
        Some("credit".into()),
        None,
        None,
        None,
        Some("patron".into()),
        None,
    )
    .expect("achat a credit");
    let dette = 5_000;
    // Et une facture preparee, pas encore validee.
    let brouillon = pieces::creer_piece_fournisseur_sur_base(
        &mut base, f.clone(), "facture_fournisseur".into(), vec![ligne(&sucre, 6.0)], None, None, None, None, None,
    )
    .unwrap();
    let brouillon_id = brouillon["id"].as_str().unwrap().to_string();
    assert_eq!(brouillon["statut"], "brouillon");

    // Le releve imprime, l'etat global, la fiche, les listes : la meme dette.
    let etat = fournisseurs::lire_etat_dette_fournisseur_sur_base(&mut base, f.clone()).unwrap();
    assert_eq!(etat["total_du"], dette, "{etat}");
    assert_eq!(etat["lignes"].as_array().unwrap().len(), 1, "le brouillon n'est pas une ligne due");
    let global = fournisseurs::lire_etat_dettes_global_sur_base(&mut base).unwrap();
    assert_eq!(global["total_general"], dette, "{global}");
    let fiche = fournisseurs::lire_fiche_fournisseur_sur_base(&mut base, f.clone()).unwrap();
    assert_eq!(fiche["stats"]["dette"], dette, "{fiche}");
    let avec = fournisseurs::lire_fournisseurs_avec_dettes_sur_base(&mut base).unwrap();
    assert_eq!(avec.iter().find(|l| l["id"] == f.as_str()).unwrap()["dette"], dette);
    let dettes = chantiers::lire_dettes_fournisseurs_sur_base(&mut base).unwrap();
    assert_eq!(dettes.iter().find(|l| l["id"] == f.as_str()).unwrap()["dette"], dette);
    let page = pagination::lire_fournisseurs_pagines_sur_base(&mut base, 0, 50, None).unwrap();
    let ligne_f = page["donnees"].as_array().unwrap().iter().find(|l| l["id"] == f.as_str()).cloned().expect("le fournisseur");
    assert_eq!(ligne_f["dette"], dette);

    // Un reglement global de toute la dette : il solde l'achat, rien ne
    // s'impute au brouillon, qui reste un brouillon.
    argent::regler_dette_fournisseur_sur_base(&mut base, f.clone(), dette, "especes".into(), None, None).expect("reglement");
    assert_eq!(statut_piece(&mut base, &brouillon_id), "brouillon", "un reglement ne valide pas une facture");
    let impute = compter(
        &mut base,
        "SELECT CAST(COALESCE(SUM(montant), 0) AS BIGINT) FROM paiement_fournisseur WHERE piece_id = ?1",
        &parametres![brouillon_id.clone()],
    );
    assert_eq!(impute, 0);
    assert_eq!(fournisseurs::lire_etat_dette_fournisseur_sur_base(&mut base, f.clone()).unwrap()["total_du"], 0);
    assert_eq!(sorties_caisse(&mut base), dette);

    // Validee a credit, elle devient la dette, et le stock entre.
    achats::valider_facture_fournisseur_sur_base(&mut base, brouillon_id, "credit".into(), None, None, Some("patron".into()))
        .expect("valider");
    assert_eq!(fournisseurs::lire_etat_dette_fournisseur_sur_base(&mut base, f.clone()).unwrap()["total_du"], 6 * sucre.3);
    assert_eq!(stock(&mut base, &sucre.0, &depot), stock_avant + 16.0);
}

// =====================================================================
//  3. Un en-tete CSV se lit par ses noms, jamais par position
// =====================================================================

fn nb_articles(base: &mut Base) -> i64 {
    compter(base, "SELECT COUNT(*) FROM article", &[])
}

fn article_par_nom(base: &mut Base, nom: &str) -> (String, Option<i64>) {
    base.lire_une(
        "SELECT id, dernier_prix_achat FROM article WHERE nom = ?1",
        &parametres![nom],
        |r| Ok((r.get::<String>(0)?, r.get::<Option<i64>>(1)?)),
    )
    .unwrap()
    .expect("l'article importe")
}

#[test]
fn un_entete_incomplet_se_lit_par_ses_noms() {
    let mut base = base_avec_demo();
    let depot = depot_defaut(&mut base);

    // `Prix achat` absent : le stock n'est plus lu comme prix d'achat.
    let r = catalogue_csv::importer_articles_csv_sur_base(
        &mut base,
        "Nom;Categorie;Unite;Prix;Stock\nRiz parfume;Cereales;Sac;15000;12\n".into(),
        Some(true),
        true,
        false,
    )
    .expect("import");
    assert_eq!(r["crees"], 1, "{r}");
    let (riz, prix_achat) = article_par_nom(&mut base, "Riz parfume");
    assert!(prix_achat.unwrap_or(0) != 12, "le stock lu comme prix d'achat : {prix_achat:?}");
    assert_eq!(stock(&mut base, &riz, &depot), 12.0);

    // Les colonnes dans le desordre, sans Categorie ni Unite.
    let r = catalogue_csv::importer_articles_csv_sur_base(
        &mut base,
        "Stock;Prix;Nom\n4;2500;Savon noir\n".into(),
        Some(true),
        true,
        false,
    )
    .expect("import");
    assert_eq!(r["crees"], 1, "{r}");
    let (savon, _) = article_par_nom(&mut base, "Savon noir");
    assert_eq!(stock(&mut base, &savon, &depot), 4.0);

    // Sans en-tete, l'ordre v1.3 tient toujours.
    catalogue_csv::importer_articles_csv_sur_base(
        &mut base,
        "Mil;Cereales;Sac;9000;7000;0;;5\n".into(),
        Some(true),
        true,
        false,
    )
    .expect("import v1.3");
    let (mil, prix_achat) = article_par_nom(&mut base, "Mil");
    assert_eq!(prix_achat, Some(7000));
    assert_eq!(stock(&mut base, &mil, &depot), 5.0);
}

#[test]
fn une_colonne_inconnue_ou_manquante_refuse_le_fichier_sans_rien_ecrire() {
    let mut base = base_avec_demo();
    let avant = nb_articles(&mut base);

    let refus = catalogue_csv::importer_articles_csv_sur_base(
        &mut base,
        "Nom;Prix;Quantite\nThe vert;1500;30\n".into(),
        Some(true),
        true,
        false,
    )
    .unwrap_err();
    assert!(refus.contains("« Quantite » inconnue"), "{refus}");
    assert!(refus.contains("Stock"), "le message dit les noms reconnus : {refus}");

    let refus = catalogue_csv::importer_articles_csv_sur_base(
        &mut base,
        "Nom;Categorie;Prix de vente\nThe vert;Boissons;1500\n".into(),
        Some(true),
        true,
        false,
    )
    .unwrap_err();
    assert!(refus.contains("« Prix » manquante"), "{refus}");

    let refus = catalogue_csv::importer_articles_csv_sur_base(
        &mut base,
        "Nom;Prix;Prix\nThe vert;1500;1600\n".into(),
        Some(true),
        true,
        false,
    )
    .unwrap_err();
    assert!(refus.contains("en double"), "{refus}");
    assert_eq!(nb_articles(&mut base), avant, "rien n'a ete importe");

    // Les fournisseurs : `Nom;NIF` ne lit plus le NIF comme telephone.
    let r = fournisseurs_csv::importer_fournisseurs_csv_sur_base(
        &mut base,
        "Nom;NIF\nSogeco;NIF-4455\n".into(),
        false,
        false,
    )
    .expect("import fournisseurs");
    assert_eq!(r["crees"], 1, "{r}");
    let (tel, nif): (Option<String>, Option<String>) = base
        .lire_une("SELECT telephone, nif FROM fournisseur WHERE nom = 'Sogeco'", &[], |l| {
            Ok((l.get::<Option<String>>(0)?, l.get::<Option<String>>(1)?))
        })
        .unwrap()
        .unwrap();
    assert_eq!((tel, nif.as_deref()), (None, Some("NIF-4455")));
    let refus = fournisseurs_csv::importer_fournisseurs_csv_sur_base(&mut base, "Nom;Ville\nX;Kayes\n".into(), false, false)
        .unwrap_err();
    assert!(refus.contains("« Ville » inconnue"), "{refus}");
}

// =====================================================================
//  4. Le recu d'une vente sans piece garde le nom du jour de la vente
// =====================================================================

/// Le paiement en especes de la vente.
fn paiement_de(base: &mut Base, vente_id: &str) -> String {
    let dossier = base.dossier().to_string();
    texte(base, "SELECT id FROM paiement WHERE vente_id = ?1 AND mode = 'especes' AND dossier_id = ?2", &parametres![vente_id, dossier])
        .expect("le paiement")
}

#[test]
fn le_recu_d_une_vente_sans_piece_garde_le_nom_du_jour_de_la_vente() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let depot = depot_defaut(&mut base);
    let sucre = article_unite(&mut base, "Sucre");
    let client = client_reel(&mut base);
    let nom_du_jour = texte(&mut base, "SELECT nom FROM client WHERE id = ?1", &parametres![client.clone()]).unwrap();
    let stock_avant = stock(&mut base, &sucre.0, &depot);
    let caisse_avant = entrees_caisse(&mut base);

    // Une vente a credit, 1 kg verse sur 3.
    let v = argent::creer_vente_sur_base(
        &mut base,
        client.clone(),
        depot.clone(),
        "credit".into(),
        vec![ligne_vente(&sucre, &depot, 3.0)],
        None,
        Some(sucre.3),
        Some("especes".into()),
        None,
    )
    .expect("vente");
    let vente_id = v["vente_id"].as_str().unwrap().to_string();
    let paiement = paiement_de(&mut base, &vente_id);

    // La fiche est renommee ensuite.
    comptoir::modifier_client_sur(&mut base, client.clone(), "Nouveau nom".into(), Some("76000000".into()), None, None, None).unwrap();

    let recu = creances::lire_donnees_recu_sur_base(&mut base, paiement.clone(), "client".into()).unwrap();
    assert_eq!(recu["tiers"]["nom"], nom_du_jour.as_str(), "{recu}");
    assert_eq!(recu["reste_du"], 2 * sucre.3);
    // Une vente faite apres le renommage prend le nouveau nom.
    let v2 = argent::creer_vente_sur_base(
        &mut base, client.clone(), depot.clone(), "credit".into(), vec![ligne_vente(&sucre, &depot, 1.0)],
        None, Some(sucre.3), Some("especes".into()), None,
    )
    .unwrap();
    let p2 = paiement_de(&mut base, v2["vente_id"].as_str().unwrap());
    assert_eq!(creances::lire_donnees_recu_sur_base(&mut base, p2, "client".into()).unwrap()["tiers"]["nom"], "Nouveau nom");

    // Le passant rattache a ce client : son recu garde son code, sans les
    // coordonnees du client.
    let comptant = client_generique(&mut base);
    let vp = argent::creer_vente_sur_base(
        &mut base, comptant, depot.clone(), "comptant".into(), vec![ligne_vente(&sucre, &depot, 1.0)],
        None, Some(sucre.3), Some("especes".into()), None,
    )
    .expect("vente au comptant");
    let vp_id = vp["vente_id"].as_str().unwrap().to_string();
    let passant = texte(&mut base, "SELECT client_id FROM vente WHERE id = ?1", &parametres![vp_id.clone()]).unwrap();
    let code = texte(&mut base, "SELECT code FROM client WHERE id = ?1", &parametres![passant.clone()]).unwrap();
    let pp = paiement_de(&mut base, &vp_id);
    passage::rattacher_sur(&mut base, passant, client.clone()).expect("rattachement");
    let recu = creances::lire_donnees_recu_sur_base(&mut base, pp, "client".into()).unwrap();
    assert_eq!(recu["tiers"]["nom"], code.as_str(), "{recu}");
    assert!(recu["tiers"]["telephone"].is_null(), "{recu}");

    // Apres les gestes : 5 kg sortis, 3 kg encaisses, 2 kg dus.
    assert_eq!(stock(&mut base, &sucre.0, &depot), stock_avant - 5.0);
    assert_eq!(entrees_caisse(&mut base) - caisse_avant, 3 * sucre.3);
    let dossier = base.dossier().to_string();
    let du = compter(
        &mut base,
        "SELECT CAST(COALESCE(SUM(
            (SELECT COALESCE(SUM(prix_pratique * quantite), 0) FROM ligne_vente WHERE vente_id = v.id)
          - (SELECT COALESCE(SUM(montant), 0) FROM paiement WHERE vente_id = v.id)), 0) AS BIGINT)
         FROM vente v WHERE v.client_id = ?1 AND v.dossier_id = ?2",
        &parametres![client, dossier],
    );
    assert_eq!(du, 2 * sucre.3);
}

// =====================================================================
//  Tout passe le detecteur
// =====================================================================

#[test]
fn tout_ce_qui_est_corrige_ici_passe_le_detecteur() {
    let mut base = base_avec_demo();
    base.auditer(true);
    ouvrir_caisse(&mut base);
    let client = client_reel(&mut base);
    let sucre = article_unite(&mut base, "Sucre");
    let depot = depot_defaut(&mut base);

    let (piece_id, _, _) = facture_reglee_en_partie_par_avoir(&mut base, &client);
    pieces::lire_donnees_piece_sur_base(&mut base, piece_id).unwrap();

    let f = fournisseur(&mut base, "Grossiste");
    pieces::creer_piece_fournisseur_sur_base(
        &mut base, f.clone(), "facture_fournisseur".into(), vec![ligne(&sucre, 2.0)], None, None, None, None, None,
    )
    .unwrap();
    fournisseurs::lire_etat_dette_fournisseur_sur_base(&mut base, f.clone()).unwrap();
    fournisseurs::lire_etat_dettes_global_sur_base(&mut base).unwrap();
    fournisseurs::lire_fiche_fournisseur_sur_base(&mut base, f.clone()).unwrap();
    fournisseurs::lire_fournisseurs_avec_dettes_sur_base(&mut base).unwrap();
    chantiers::lire_dettes_fournisseurs_sur_base(&mut base).unwrap();
    pagination::lire_fournisseurs_pagines_sur_base(&mut base, 0, 50, None).unwrap();
    argent::regler_dette_fournisseur_sur_base(&mut base, f, 100, "especes".into(), None, None).expect("avance");

    catalogue_csv::importer_articles_csv_sur_base(&mut base, "Nom;Prix;Stock\nThe vert;1500;3\n".into(), Some(true), true, false)
        .expect("import");
    fournisseurs_csv::importer_fournisseurs_csv_sur_base(&mut base, "Nom;NIF\nSogeco;N1\n".into(), false, false).expect("import");

    let v = argent::creer_vente_sur_base(
        &mut base, client.clone(), depot.clone(), "credit".into(), vec![ligne_vente(&sucre, &depot, 1.0)],
        None, Some(100), Some("especes".into()), None,
    )
    .unwrap();
    let p = paiement_de(&mut base, v["vente_id"].as_str().unwrap());
    comptoir::modifier_client_sur(&mut base, client.clone(), "Autre nom".into(), None, None, None, None).unwrap();
    creances::lire_donnees_recu_sur_base(&mut base, p, "client".into()).unwrap();
    let comptant = client_generique(&mut base);
    let vp = argent::creer_vente_sur_base(
        &mut base, comptant, depot.clone(), "comptant".into(), vec![ligne_vente(&sucre, &depot, 1.0)],
        None, Some(sucre.3), Some("especes".into()), None,
    )
    .unwrap();
    let dossier = base.dossier().to_string();
    let passant = texte(
        &mut base,
        "SELECT client_id FROM vente WHERE id = ?1 AND dossier_id = ?2",
        &parametres![vp["vente_id"].as_str().unwrap(), dossier],
    )
    .unwrap();
    passage::rattacher_sur(&mut base, passant, client).expect("rattachement");
}

// =====================================================================
//  Les memes corrections, version fenetre (`fn(conn)`, SQLite seul)
// =====================================================================

mod fenetre {
    use gescom_noyau::{catalogue_csv, comptoir, creances, fournisseurs, persistance, pieces};
    use rusqlite::{params, Connection};

    const T: &str = "2026-10-01T10:00:00";

    fn base() -> Connection {
        let conn = Connection::open_in_memory().expect("base en mémoire");
        persistance::initialiser_tables(&conn).expect("schéma");
        conn.execute(
            "INSERT INTO depot (id, nom, est_defaut, actif, cree_le, modifie_le, origine)
             VALUES ('d1', 'Boutique', 1, 1, ?1, ?1, 'test')",
            params![T],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO article (id, nom, unite_base, actif, cree_le, modifie_le, origine)
             VALUES ('a1', 'Sucre', 'kg', 1, ?1, ?1, 'test')",
            params![T],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO unite_vente (id, article_id, libelle, facteur, prix_reference, actif, cree_le, modifie_le, origine)
             VALUES ('u1', 'a1', 'Kilo', 1.0, 1000, 1, ?1, ?1, 'test')",
            params![T],
        )
        .unwrap();
        conn
    }

    fn piece(conn: &Connection, id: &str, type_piece: &str, statut: &str, tiers_type: &str, tiers: &str, montant: i64) {
        conn.execute(
            "INSERT INTO piece_commerciale (id, type_piece, numero, statut, tiers_type, tiers_id, date_piece, cree_le, modifie_le)
             VALUES (?1, ?2, ?1, ?3, ?4, ?5, ?6, ?6, ?6)",
            params![id, type_piece, statut, tiers_type, tiers, T],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO ligne_piece (id, piece_id, article_id, unite_vente_id, quantite, prix_unitaire, montant_ht, cree_le)
             VALUES (?1, ?2, 'a1', 'u1', 1, ?3, ?3, ?4)",
            params![format!("l-{id}"), id, montant, T],
        )
        .unwrap();
    }

    fn client(conn: &Connection, id: &str, code: &str, nom: &str) {
        conn.execute(
            "INSERT INTO client (id, code, nom, cree_le, modifie_le) VALUES (?1, ?2, ?3, ?4, ?4)",
            params![id, code, nom, T],
        )
        .unwrap();
    }

    fn vente(conn: &Connection, id: &str, client: &str, piece_id: Option<&str>, montant: i64) {
        conn.execute(
            "INSERT INTO vente (id, client_id, depot_id, date_vente, cree_le, modifie_le, piece_id)
             VALUES (?1, ?2, 'd1', ?3, ?3, ?3, ?4)",
            params![id, client, T, piece_id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO ligne_vente (id, vente_id, article_id, unite_vente_id, depot_source_id, quantite, prix_reference, prix_pratique, cree_le)
             VALUES (?1, ?2, 'a1', 'u1', 'd1', 1, ?3, ?3, ?4)",
            params![format!("lv-{id}"), id, montant, T],
        )
        .unwrap();
    }

    fn paiement(conn: &Connection, id: &str, vente: &str, mode: &str, montant: i64) {
        conn.execute(
            "INSERT INTO paiement (id, vente_id, montant, mode, date_paiement, cree_le) VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
            params![id, vente, montant, mode, T],
        )
        .unwrap();
    }

    #[test]
    fn la_part_reglee_par_avoir_n_est_pas_due() {
        let conn = base();
        client(&conn, "c1", "CLI-1", "Awa");
        piece(&conn, "FAC-1", "facture", "emis", "client", "c1", 10_000);
        vente(&conn, "v1", "c1", Some("FAC-1"), 10_000);
        paiement(&conn, "p1", "v1", "especes", 2_000);
        paiement(&conn, "p2", "v1", "avoir", 3_000);
        let t = &pieces::lire_donnees_piece(&conn, "FAC-1".into()).unwrap()["totaux"];
        assert_eq!(t["total_paye"], 5_000, "{t}");
        assert_eq!(t["regle_par_avoir"], 3_000);
        assert_eq!(t["encaisse"], 2_000);
        assert_eq!(t["reste_du"], 5_000);
    }

    #[test]
    fn le_brouillon_fournisseur_n_est_pas_une_dette() {
        let conn = base();
        conn.execute("INSERT INTO fournisseur (id, nom, cree_le, modifie_le) VALUES ('f1', 'Grossiste', ?1, ?1)", params![T])
            .unwrap();
        piece(&conn, "FAF-1", "facture_fournisseur", "emis", "fournisseur", "f1", 5_000);
        piece(&conn, "FAF-2", "facture_fournisseur", "brouillon", "fournisseur", "f1", 7_000);
        let etat = fournisseurs::lire_etat_dette_fournisseur(&conn, "f1".into()).unwrap();
        assert_eq!(etat["total_du"], 5_000, "{etat}");
        assert_eq!(fournisseurs::lire_etat_dettes_global(&conn).unwrap()["total_general"], 5_000);
    }

    #[test]
    fn un_entete_csv_se_lit_par_ses_noms() {
        let mut conn = base();
        let r = catalogue_csv::importer_articles_csv(&mut conn, "Nom;Categorie;Unite;Prix;Stock\nRiz;Cereales;Sac;15000;12\n".into(), Some(true))
            .unwrap();
        assert_eq!(r["crees"], 1, "{r}");
        let (pa, stock): (Option<i64>, f64) = conn
            .query_row(
                "SELECT a.dernier_prix_achat, (SELECT SUM(quantite) FROM stock_depot WHERE article_id = a.id)
                 FROM article a WHERE a.nom = 'Riz'",
                [],
                |l| Ok((l.get(0)?, l.get(1)?)),
            )
            .unwrap();
        assert!(pa.unwrap_or(0) != 12, "{pa:?}");
        assert_eq!(stock, 12.0);
        let refus = catalogue_csv::importer_articles_csv(&mut conn, "Nom;Prix;Quantite\nThe;1500;3\n".into(), Some(true))
            .unwrap_err();
        assert!(refus.contains("« Quantite » inconnue"), "{refus}");
    }

    #[test]
    fn le_recu_garde_le_nom_du_jour_de_la_vente() {
        let conn = base();
        client(&conn, "c1", "CLI-1", "Awa");
        vente(&conn, "v1", "c1", None, 3_000);
        paiement(&conn, "p1", "v1", "especes", 1_000);
        comptoir::modifier_client(&conn, "c1".into(), "Awa Diallo".into(), None, None, None, None).unwrap();
        let recu = creances::lire_donnees_recu(&conn, "p1".into(), "client".into()).unwrap();
        assert_eq!(recu["tiers"]["nom"], "Awa", "{recu}");
        assert_eq!(recu["reste_du"], 2_000);
        vente(&conn, "v2", "c1", None, 1_000);
        paiement(&conn, "p2", "v2", "especes", 1_000);
        let recu = creances::lire_donnees_recu(&conn, "p2".into(), "client".into()).unwrap();
        assert_eq!(recu["tiers"]["nom"], "Awa Diallo");
    }
}
