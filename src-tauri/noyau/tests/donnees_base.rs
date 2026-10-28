//! Lot D — les donnees, sur les deux moteurs.
//!
//! - D.9  une facture fournisseur emise garde le nom ecrit a son emission,
//!        a l'impression, dans les listes et sur le recu ;
//! - D.10 « Rattacher a un client » une fiche de passage : toute son
//!        histoire passe au client choisi dans une transaction, ses
//!        factures gardent leur PAS-…, la fiche vide est desactivee, le
//!        journal dit qui, quoi, combien ;
//! - D.11 l'import CSV des articles et la colonne « Suivre le stock » ;
//! - D.12 les listes de ventes par pages de 50.
//!
//! Apres chaque geste : stock, caisse et creance ; un refus n'ecrit rien.
//!
//!   cargo test -p gescom-noyau --test donnees_base
//!   GESCOM_PG=postgresql://…/gescom_test cargo test -p gescom-noyau --test donnees_base -- --test-threads=1

mod commun;

use commun::*;
use gescom_noyau::argent::{self, ParamsLigneInput};
use gescom_noyau::base::Base;
use gescom_noyau::parametres;
use gescom_noyau::utils::maintenant_iso;
use gescom_noyau::{achats, catalogue_csv, comptoir, creances, fournisseurs, historique, pagination, passage, pieces, retours};

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

/// Une vente de sucre a ce client ; `paye` encaisse en especes.
fn vendre(base: &mut Base, client: &str, mode: &str, quantite: f64, paye: Option<i64>) -> serde_json::Value {
    let depot = depot_defaut(base);
    let sucre = article_unite(base, "Sucre");
    argent::creer_vente_sur_base(
        base,
        client.to_string(),
        depot.clone(),
        mode.into(),
        vec![ligne_vente(&sucre, &depot, quantite)],
        None,
        paye,
        paye.map(|_| "especes".to_string()),
        None,
    )
    .expect("vente")
}

/// (entrees, sorties) de la caisse du dossier.
fn caisse(base: &mut Base) -> (i64, i64) {
    let dossier = base.dossier().to_string();
    let mut somme = |sens: &str| {
        compter(
            base,
            "SELECT CAST(COALESCE(SUM(montant), 0) AS BIGINT) FROM mouvement_caisse
             WHERE sens = ?1 AND dossier_id = ?2",
            &parametres![sens, dossier.clone()],
        )
    };
    (somme("entree"), somme("sortie"))
}

/// (creance, nombre de ventes) d'un client, tels que la liste des
/// clients les montre.
fn creance_et_ventes(base: &mut Base, client_id: &str) -> (i64, i64) {
    let dossier = base.dossier().to_string();
    let nom: String = base
        .lire_une(
            "SELECT nom FROM client WHERE id = ?1 AND dossier_id = ?2",
            &parametres![client_id, dossier],
            |r| r.get::<String>(0),
        )
        .unwrap()
        .unwrap();
    let page = pagination::lire_clients_pagines_sur_base(base, 0, 50, Some(nom), false, None, None).unwrap();
    let l = page["donnees"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == client_id)
        .expect("le client est dans la liste")
        .clone();
    (l["total_creances"].as_i64().unwrap(), l["nb_ventes"].as_i64().unwrap())
}

/// Combien de lignes de `table` pointent ce client par `colonne`.
fn lignes_du_client(base: &mut Base, table: &str, colonne: &str, client_id: &str) -> i64 {
    let dossier = base.dossier().to_string();
    compter(
        base,
        &format!("SELECT COUNT(*) FROM {table} WHERE {colonne} = ?1 AND dossier_id = ?2"),
        &parametres![client_id, dossier],
    )
}

fn piece_imprimee(base: &mut Base, piece_id: &str) -> serde_json::Value {
    pieces::lire_donnees_piece_sur_base(base, piece_id.to_string()).unwrap()["piece"].clone()
}

fn nb_journal(base: &mut Base, evenement: &str) -> i64 {
    let dossier = base.dossier().to_string();
    compter(
        base,
        "SELECT COUNT(*) FROM journal WHERE type_evenement = ?1 AND dossier_id = ?2",
        &parametres![evenement, dossier],
    )
}

// =====================================================================
//  D.9 — le nom fige du fournisseur
// =====================================================================

#[test]
fn une_facture_fournisseur_emise_garde_le_nom_ecrit_a_son_emission() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let sucre = article_unite(&mut base, "Sucre");
    let depot = depot_defaut(&mut base);
    let dossier = base.dossier().to_string();
    let f = fournisseur(&mut base, "Ets Diarra");

    // Une facture payee comptant, une a credit, une encore en brouillon
    // (un bon de commande, lui, nait emis).
    let creer = |base: &mut Base, type_piece: &str, quantite: f64| -> String {
        pieces::creer_piece_fournisseur_sur_base(
            base, f.clone(), type_piece.into(), vec![ligne(&sucre, quantite)], None, None, None, None, None,
        )
        .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string()
    };
    let faf_comptant = creer(&mut base, "facture_fournisseur", 4.0);
    achats::valider_facture_fournisseur_sur_base(&mut base, faf_comptant.clone(), "comptant".into(), None, None, Some("patron".into()))
        .unwrap();
    let faf_credit = creer(&mut base, "facture_fournisseur", 3.0);
    achats::valider_facture_fournisseur_sur_base(&mut base, faf_credit.clone(), "credit".into(), None, None, Some("patron".into()))
        .unwrap();
    let brouillon = creer(&mut base, "facture_fournisseur", 1.0);
    assert_eq!(statut_piece(&mut base, &brouillon), "brouillon");
    assert_eq!(piece_imprimee(&mut base, &faf_comptant)["client_nom"], "Ets Diarra");

    let stock_avant = stock(&mut base, &sucre.0, &depot);
    let caisse_avant = caisse(&mut base);
    let dette = |base: &mut Base| {
        fournisseurs::lire_etat_dette_fournisseur_sur_base(base, f.clone()).unwrap()["total_du"].as_i64().unwrap()
    };
    let dette_avant = dette(&mut base);
    assert!(dette_avant >= 3 * sucre.3, "la facture à crédit est due ({dette_avant})");

    fournisseurs::modifier_fournisseur_sur_base(
        &mut base, f.clone(), "Diarra & Fils SARL".into(), Some("66 00 00 00".into()), None, None, None, None,
        false,
    )
    .unwrap();

    // Le papier : les factures emises gardent leur nom, le brouillon suit la fiche.
    assert_eq!(piece_imprimee(&mut base, &faf_comptant)["client_nom"], "Ets Diarra");
    assert_eq!(piece_imprimee(&mut base, &faf_credit)["client_nom"], "Ets Diarra");
    assert_eq!(piece_imprimee(&mut base, &brouillon)["client_nom"], "Diarra & Fils SARL");

    // Les listes : meme chose ; on retrouve par l'ancien nom comme par le nouveau.
    let liste = |base: &mut Base, recherche: Option<&str>| -> Vec<serde_json::Value> {
        pieces::lire_toutes_pieces_fournisseur_sur_base(
            base, None, None, recherche.map(str::to_string), Some(f.clone()), None, None,
        )
        .unwrap()
    };
    let toutes = liste(&mut base, None);
    let nom_de = |l: &[serde_json::Value], id: &str| l.iter().find(|p| p["id"] == id).unwrap()["tiers_nom"].clone();
    assert_eq!(nom_de(&toutes, &faf_comptant), "Ets Diarra");
    assert_eq!(nom_de(&toutes, &brouillon), "Diarra & Fils SARL");
    assert_eq!(liste(&mut base, Some("ets diarra")).len(), 2, "par l'ancien nom : les deux factures émises");
    assert_eq!(liste(&mut base, Some("Fils")).len(), 3, "par le nouveau nom : tout");

    // Le recu du reglement de la facture comptant.
    let reglement: String = base
        .lire_une(
            "SELECT id FROM paiement_fournisseur WHERE piece_id = ?1 AND dossier_id = ?2",
            &parametres![faf_comptant.clone(), dossier.clone()],
            |r| r.get::<String>(0),
        )
        .unwrap()
        .expect("le règlement comptant");
    let recu = creances::lire_donnees_recu_sur_base(&mut base, reglement, "fournisseur".into()).unwrap();
    assert_eq!(recu["tiers"]["nom"], "Ets Diarra");

    // Rien d'autre n'a bouge.
    assert_eq!(stock(&mut base, &sucre.0, &depot), stock_avant);
    assert_eq!(caisse(&mut base), caisse_avant);
    assert_eq!(dette(&mut base), dette_avant);

    // Le brouillon emis sous le nouveau nom, puis la fiche renommee
    // encore : il garde le nom de son emission, les autres le leur.
    achats::valider_facture_fournisseur_sur_base(&mut base, brouillon.clone(), "credit".into(), None, None, Some("patron".into()))
        .unwrap();
    fournisseurs::modifier_fournisseur_sur_base(&mut base, f.clone(), "DD SARL".into(), None, None, None, None, None, false).unwrap();
    assert_eq!(piece_imprimee(&mut base, &brouillon)["client_nom"], "Diarra & Fils SARL");
    assert_eq!(piece_imprimee(&mut base, &faf_credit)["client_nom"], "Ets Diarra");

    // Un fournisseur introuvable : refus, rien de fige.
    let figes = compter(
        &mut base,
        "SELECT COUNT(*) FROM piece_commerciale WHERE tiers_nom IS NOT NULL AND dossier_id = ?1",
        &parametres![dossier.clone()],
    );
    let refus = fournisseurs::modifier_fournisseur_sur_base(&mut base, "inconnu".into(), "X".into(), None, None, None, None, None, false)
        .unwrap_err();
    assert!(refus.contains("introuvable"), "{refus}");
    assert_eq!(
        compter(&mut base, "SELECT COUNT(*) FROM piece_commerciale WHERE tiers_nom IS NOT NULL AND dossier_id = ?1", &parametres![dossier]),
        figes
    );
}

// =====================================================================
//  D.10 — rattacher une fiche de passage a un client
// =====================================================================

#[test]
fn rattacher_un_passant_deplace_toute_son_histoire_et_garde_ses_factures() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let dossier = base.dossier().to_string();
    let depot = depot_defaut(&mut base);
    let sucre = article_unite(&mut base, "Sucre");
    let comptant = client_generique(&mut base);
    let awa = client_reel(&mut base); // Amadou Diarra, telephone 76000001

    // Le client cible a deja une dette.
    vendre(&mut base, &awa, "credit", 2.0, None);
    // Le passant : une vente payee, sa facture, un retour rembourse.
    let r = vendre(&mut base, &comptant, "comptant", 5.0, Some(5 * sucre.3));
    let passant = r["client_id"].as_str().unwrap().to_string();
    let code = r["client_code"].as_str().unwrap().to_string();
    assert_eq!(code, "PAS-000001");
    let vente = r["vente_id"].as_str().unwrap().to_string();
    let f = argent::creer_facture_depuis_vente_sur_base(&mut base, vente.clone(), comptant.clone(), "comptant".into(), None).unwrap();
    let piece = f["piece_id"].as_str().unwrap().to_string();
    let numero = f["numero"].as_str().unwrap().to_string();
    let ligne_id: String = base
        .lire_une(
            "SELECT id FROM ligne_vente WHERE vente_id = ?1 AND dossier_id = ?2",
            &parametres![vente.clone(), dossier.clone()],
            |l| l.get::<String>(0),
        )
        .unwrap()
        .unwrap();
    retours::enregistrer_retour_sur_base(
        &mut base, vente.clone(), ligne_id, 1.0, "remboursement".into(), Some("especes".into()), None, None, None, None, None,
    )
    .unwrap();
    // Toutes les tables qui pointent un client : un avoir, un echange et
    // un rappel poses a la main (l'ecran ne les donne pas a un passant),
    // pour verifier que chacune suit.
    let now = maintenant_iso();
    base.executer(
        "INSERT INTO avoir (id, client_id, montant, statut, cree_le, origine, dossier_id)
         VALUES (?1, ?2, 500, 'ouvert', ?3, 'test', ?4)",
        &parametres![uuid::Uuid::new_v4().to_string(), passant.clone(), now.clone(), dossier.clone()],
    )
    .unwrap();
    base.executer(
        "INSERT INTO echange (id, dossier_id, client_id, genre, quoi, date_echange, cree_le)
         VALUES (?1, ?2, ?3, 'note', 'Revenu avec son ticket', ?4, ?4)",
        &parametres![uuid::Uuid::new_v4().to_string(), dossier.clone(), passant.clone(), now.clone()],
    )
    .unwrap();
    base.executer(
        "INSERT INTO rappel (id, dossier_id, client_id, pour_utilisateur_id, quand, quoi, cree_le)
         VALUES (?1, ?2, ?3, 'patron', ?4, 'Le rappeler', ?4)",
        &parametres![uuid::Uuid::new_v4().to_string(), dossier.clone(), passant.clone(), now],
    )
    .unwrap();
    let paiement: String = base
        .lire_une(
            "SELECT pa.id FROM paiement pa WHERE pa.vente_id = ?1 AND pa.dossier_id = ?2 AND pa.montant > 0",
            &parametres![vente.clone(), dossier.clone()],
            |l| l.get::<String>(0),
        )
        .unwrap()
        .expect("le paiement du passant");
    let paiements_passant = compter(
        &mut base,
        "SELECT COUNT(*) FROM paiement WHERE vente_id = ?1 AND dossier_id = ?2",
        &parametres![vente.clone(), dossier.clone()],
    );
    let paiements_awa = |base: &mut Base| {
        compter(
            base,
            "SELECT COUNT(*) FROM paiement pa JOIN vente v ON v.id = pa.vente_id
             WHERE v.client_id = ?1 AND v.dossier_id = ?2 AND pa.dossier_id = ?2",
            &parametres![awa.clone(), dossier.clone()],
        )
    };

    // L'etat avant le geste.
    let stock_avant = stock(&mut base, &sucre.0, &depot);
    let caisse_avant = caisse(&mut base);
    let (creance_avant, ventes_avant) = creance_et_ventes(&mut base, &awa);
    assert_eq!(creance_avant, 2 * sucre.3);
    let paiements_avant = paiements_awa(&mut base);
    let journal_avant = nb_journal(&mut base, "passage_rattache");
    // Awa a un telephone : la facture du passant ne le prendra pas.
    let tel_awa: Option<String> = base
        .lire_une("SELECT telephone FROM client WHERE id = ?1 AND dossier_id = ?2", &parametres![awa.clone(), dossier.clone()], |l| {
            l.get::<Option<String>>(0)
        })
        .unwrap()
        .unwrap();
    assert!(tel_awa.is_some());

    let r = passage::rattacher_sur(&mut base, passant.clone(), awa.clone()).expect("le rattachement");
    assert_eq!(r["passage_code"], code.as_str());
    let d = &r["deplaces"];
    assert_eq!((d["ventes"].as_i64(), d["pieces"].as_i64(), d["retours"].as_i64()), (Some(1), Some(1), Some(1)), "{r}");
    assert_eq!(d["paiements"].as_i64(), Some(paiements_passant));
    assert_eq!((d["avoirs"].as_i64(), d["echanges"].as_i64(), d["rappels"].as_i64()), (Some(1), Some(1), Some(1)));

    // Le client cible : la vente, ses paiements, sa piece, son avoir.
    let (creance_apres, ventes_apres) = creance_et_ventes(&mut base, &awa);
    assert_eq!(ventes_apres, ventes_avant + 1, "la vente du passant est à lui");
    assert_eq!(creance_apres, creance_avant, "payée en entier : sa dette ne bouge pas");
    assert_eq!(paiements_awa(&mut base), paiements_avant + paiements_passant);
    let client_vente: String = base
        .lire_une("SELECT client_id FROM vente WHERE id = ?1 AND dossier_id = ?2", &parametres![vente.clone(), dossier.clone()], |l| {
            l.get::<String>(0)
        })
        .unwrap()
        .unwrap();
    assert_eq!(client_vente, awa);
    assert!(
        pieces::lire_pieces_client_sur_base(&mut base, awa.clone(), None).unwrap().iter().any(|p| p["id"] == piece.as_str()),
        "la facture est dans sa fiche"
    );
    assert_eq!(lignes_du_client(&mut base, "avoir", "client_id", &awa), 1);
    // L'argent et la marchandise n'ont pas bouge.
    assert_eq!(stock(&mut base, &sucre.0, &depot), stock_avant);
    assert_eq!(caisse(&mut base), caisse_avant);

    // Plus rien sur la fiche de passage ; elle est desactivee, pas effacee.
    for (table, colonne) in [("vente", "client_id"), ("avoir", "client_id"), ("echange", "client_id"), ("rappel", "client_id")] {
        assert_eq!(lignes_du_client(&mut base, table, colonne, &passant), 0, "{table}");
    }
    assert_eq!(
        compter(
            &mut base,
            "SELECT COUNT(*) FROM piece_commerciale WHERE tiers_type = 'client' AND tiers_id = ?1 AND dossier_id = ?2",
            &parametres![passant.clone(), dossier.clone()]
        ),
        0
    );
    let (actif, generique, statut): (i64, i64, String) = base
        .lire_une(
            "SELECT actif, est_generique, statut FROM client WHERE id = ?1 AND dossier_id = ?2",
            &parametres![passant.clone(), dossier.clone()],
            |l| Ok((l.get::<i64>(0)?, l.get::<i64>(1)?, l.get::<String>(2)?)),
        )
        .unwrap()
        .expect("la fiche reste, désactivée");
    assert_eq!((actif, generique, statut.as_str()), (0, 1, "passage"));
    assert!(passage::lire_passages_sur(&mut base, None).unwrap().is_empty(), "plus dans les passages");
    assert!(passage::lire_passages_sur(&mut base, Some(code.clone())).unwrap().is_empty());
    // Le « Comptant » reste la porte de l'ecran.
    assert_eq!(gescom_noyau::catalogue::lire_client_generique_sur(&mut base).unwrap()["id"], comptant.as_str());

    // La facture emise garde le nom et le code de son emission, sans les
    // coordonnees du client ; le recu aussi.
    let p = piece_imprimee(&mut base, &piece);
    assert_eq!(p["client_nom"], code.as_str(), "{p}");
    assert!(p["client_code"].is_null());
    assert!(p["client_telephone"].is_null(), "pas le téléphone d'Awa sur le ticket du passant");
    let recu = creances::lire_donnees_recu_sur_base(&mut base, paiement, "client".into()).unwrap();
    assert_eq!((recu["tiers"]["nom"].as_str(), recu["tiers"]["code"].as_str()), (Some(code.as_str()), Some("")));
    assert!(recu["tiers"]["telephone"].is_null());
    // Le passant revient avec son ticket : Pieces le retrouve par le code.
    let trouvees = pieces::lire_toutes_pieces_client_sur_base(
        &mut base, None, None, Some(code.clone()), None, None, None, None, None, None, None,
    )
    .unwrap();
    assert!(trouvees.iter().any(|p| p["id"] == piece.as_str() && p["numero"] == numero.as_str()));

    // Le journal : qui, quoi, combien — dans l'Historique du client.
    assert_eq!(nb_journal(&mut base, "passage_rattache"), journal_avant + 1);
    let h = historique::lire_historique_sur(
        &mut base,
        historique::Filtre {
            tiers_id: Some(awa.clone()),
            type_evenement: Some("passage_rattache".into()),
            par_page: 10,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(h["total"], 1, "{h}");
    let l = &h["lignes"][0];
    assert_eq!(l["ancien"]["code"], code.as_str());
    assert_eq!(l["nouveau"]["deplaces"]["ventes"], 1);
    assert_eq!(l["libelle_type"], "Client de passage rattaché à un client");

    // ---- Les refus : rien ne s'ecrit ----
    let r2 = vendre(&mut base, &comptant, "comptant", 1.0, Some(sucre.3));
    let passant2 = r2["client_id"].as_str().unwrap().to_string();
    let stock_avant = stock(&mut base, &sucre.0, &depot);
    let caisse_avant = caisse(&mut base);
    let refus = |base: &mut Base, source: &str, cible: &str| passage::rattacher_sur(base, source.into(), cible.into()).unwrap_err();
    assert!(refus(&mut base, &passant, &awa).contains("déjà"), "une fiche déjà rattachée");
    assert!(refus(&mut base, &passant2, &comptant).contains("ordinaire"), "pas au « Comptant »");
    assert!(refus(&mut base, &passant2, &passant).contains("ordinaire"), "pas à une autre fiche de passage");
    assert!(refus(&mut base, &passant2, &passant2).contains("elle-même"));
    assert!(refus(&mut base, &awa, &passant2).contains("passage"), "un client ordinaire ne se rattache pas");
    assert!(refus(&mut base, &comptant, &awa).contains("passage"), "le « Comptant » non plus");
    assert!(refus(&mut base, &passant2, "inconnu").contains("introuvable"));
    // Une cible desactivee.
    let mariam: String = base
        .lire_une("SELECT id FROM client WHERE code = 'CLIENT00004' AND dossier_id = ?1", &parametres![dossier.clone()], |l| {
            l.get::<String>(0)
        })
        .unwrap()
        .unwrap();
    base.executer("UPDATE client SET actif = 0 WHERE id = ?1 AND dossier_id = ?2", &parametres![mariam.clone(), dossier.clone()])
        .unwrap();
    assert!(refus(&mut base, &passant2, &mariam).contains("désactivé"));
    assert_eq!(nb_journal(&mut base, "passage_rattache"), journal_avant + 1, "aucun refus n'écrit");
    assert_eq!(lignes_du_client(&mut base, "vente", "client_id", &passant2), 1);
    assert_eq!(passage::lire_passages_sur(&mut base, None).unwrap().len(), 1);
    assert_eq!(stock(&mut base, &sucre.0, &depot), stock_avant);
    assert_eq!(caisse(&mut base), caisse_avant);

    // D34 : un prospect qui recoit des ventes devient client.
    base.executer(
        "UPDATE client SET actif = 1, statut = 'prospect' WHERE id = ?1 AND dossier_id = ?2",
        &parametres![mariam.clone(), dossier.clone()],
    )
    .unwrap();
    passage::rattacher_sur(&mut base, passant2.clone(), mariam.clone()).unwrap();
    let statut: String = base
        .lire_une("SELECT statut FROM client WHERE id = ?1 AND dossier_id = ?2", &parametres![mariam.clone(), dossier.clone()], |l| {
            l.get::<String>(0)
        })
        .unwrap()
        .unwrap();
    assert_eq!(statut, "client");
    // Renommer le client ensuite ne touche pas le ticket du passant.
    comptoir::modifier_client_sur(&mut base, awa.clone(), "Amadou D.".into(), None, None, None, None).unwrap();
    assert_eq!(piece_imprimee(&mut base, &piece)["client_nom"], code.as_str());
}

// =====================================================================
//  D.11 — l'import CSV et « Suivre le stock »
// =====================================================================

const ENTETE: &str = "Nom;Categorie;Unite;Prix;Prix achat;TVA %;Code barre;Stock;Suivre le stock";

fn suivi(base: &mut Base, nom: &str) -> Option<i64> {
    base.lire_une("SELECT gere_en_stock FROM article WHERE nom = ?1", &parametres![nom], |r| r.get::<i64>(0))
        .unwrap()
}

fn prix_de(base: &mut Base, nom: &str) -> i64 {
    article_unite(base, nom).3
}

#[test]
fn l_import_csv_pose_le_suivi_du_stock_sans_deviner() {
    let mut base = base_avec_demo();
    let depot = depot_defaut(&mut base);
    let dossier = base.dossier().to_string();
    let sucre = article_unite(&mut base, "Sucre");
    let stock_sucre = stock(&mut base, &sucre.0, &depot);

    // Sans la colonne (un fichier d'avant) : un nouvel article suit son stock.
    let r = catalogue_csv::importer_articles_csv_sur_base(
        &mut base,
        "Nom;Categorie;Unite;Prix;Prix achat;TVA %;Code barre;Stock\nClous;Quincaillerie;kg;1500;1000;0;;40".into(),
        Some(true),
        true,
        false,
    )
    .unwrap();
    assert_eq!((r["crees"].as_i64(), r["nb_erreurs"].as_i64()), (Some(1), Some(0)), "{r}");
    assert_eq!(suivi(&mut base, "Clous"), Some(1));
    let clous = article_unite(&mut base, "Clous");
    assert_eq!(stock(&mut base, &clous.0, &depot), 40.0);

    // Avec la colonne.
    let journal_avant = nb_journal(&mut base, "suivi_stock_change");
    let csv = format!(
        "{ENTETE}\n\
         Sachets;Emballage;paquet;100;;;;5;non\n\
         Gobelets;Emballage;paquet;50;;;;;OUI\n\
         Pointes;Quincaillerie;kg;900;;;;;\n\
         Cordes;Nouvelle;m;300;;;;;peut-être\n\
         Sucre;;kg;950;;;;;non\n\
         Riz local;;kg;650;;;;;oui"
    );
    let r = catalogue_csv::importer_articles_csv_sur_base(&mut base, csv, Some(true), true, false).unwrap();
    assert_eq!((r["crees"].as_i64(), r["mis_a_jour"].as_i64(), r["nb_erreurs"].as_i64()), (Some(3), Some(1), Some(2)), "{r}");
    let erreurs = r["erreurs"].to_string();
    assert!(erreurs.contains("Cordes") && erreurs.contains("oui ou non") && erreurs.contains("peut-être"), "{erreurs}");
    assert!(erreurs.contains("Sucre") && erreurs.contains("Articles"), "{erreurs}");
    assert_eq!(suivi(&mut base, "Sachets"), Some(0), "« non » : créé hors du suivi");
    assert_eq!(suivi(&mut base, "Gobelets"), Some(1));
    assert_eq!(suivi(&mut base, "Pointes"), Some(1), "vide : suivi");
    // La ligne illisible n'a rien ecrit, pas meme sa categorie.
    assert_eq!(suivi(&mut base, "Cordes"), None);
    assert_eq!(compter(&mut base, "SELECT COUNT(*) FROM categorie WHERE nom = 'Nouvelle'", &[]), 0);
    // Un article existant ne change pas de suivi par l'import : la ligne
    // est refusee en entier (son prix non plus ne bouge pas).
    assert_eq!(suivi(&mut base, "Sucre"), Some(1));
    assert_eq!(prix_de(&mut base, "Sucre"), sucre.3);
    assert_eq!(prix_de(&mut base, "Riz local"), 650, "« oui » sur un article suivi : la mise à jour passe");
    // L'Historique dit l'article cree hors du suivi ; ses mouvements
    // s'ecrivent quand meme (D36).
    assert_eq!(nb_journal(&mut base, "suivi_stock_change"), journal_avant + 1);
    let sachets = article_unite(&mut base, "Sachets");
    assert_eq!(stock(&mut base, &sachets.0, &depot), 5.0);
    assert_eq!(stock(&mut base, &sucre.0, &depot), stock_sucre, "le sucre n'a pas bougé");

    // Sans le droit de regler le suivi : creer un article non suivi est refuse.
    let r = catalogue_csv::importer_articles_csv_sur_base(
        &mut base,
        format!("{ENTETE}\nBidons;;u;400;;;;;non\nSeaux;;u;700;;;;;oui"),
        Some(true),
        false,
        false,
    )
    .unwrap();
    assert_eq!((r["crees"].as_i64(), r["nb_erreurs"].as_i64()), (Some(1), Some(1)), "{r}");
    assert!(r["erreurs"].to_string().contains("droit"), "{r}");
    assert_eq!(suivi(&mut base, "Bidons"), None);
    assert_eq!(suivi(&mut base, "Seaux"), Some(1));

    // L'export ecrit la colonne, et se reimporte sans une erreur.
    let export = catalogue_csv::exporter_articles_csv_sur_base(&mut base).unwrap();
    let entete = export.lines().next().unwrap().trim_start_matches('\u{FEFF}').to_string();
    assert!(entete.ends_with(";Suivre le stock"), "{entete}");
    assert!(export.lines().any(|l| l.starts_with("Sachets;") && l.ends_with(";non")), "{export}");
    assert!(export.lines().any(|l| l.starts_with("Sucre;") && l.ends_with(";oui")), "{export}");
    let r = catalogue_csv::importer_articles_csv_sur_base(&mut base, export, Some(true), false, false).unwrap();
    assert_eq!(r["nb_erreurs"], 0, "{r}");
    assert_eq!(r["crees"], 0, "{r}");
    assert_eq!(suivi(&mut base, "Sachets"), Some(0));
    let _ = dossier;
}

// =====================================================================
//  D.12 — les ventes par pages de 50
// =====================================================================

#[test]
fn les_ventes_se_lisent_par_pages_de_50() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let dossier = base.dossier().to_string();
    let sucre = article_unite(&mut base, "Sucre");
    let client = client_reel(&mut base);
    let comptant = client_generique(&mut base);
    for _ in 0..54 {
        vendre(&mut base, &client, "comptant", 1.0, Some(sucre.3));
    }
    // La 55e : un passant, avec sa facture.
    let r = vendre(&mut base, &comptant, "comptant", 1.0, Some(sucre.3));
    let vente_passant = r["vente_id"].as_str().unwrap().to_string();
    let f = argent::creer_facture_depuis_vente_sur_base(&mut base, vente_passant.clone(), comptant, "comptant".into(), None).unwrap();
    let numero = f["numero"].as_str().unwrap().to_string();
    let en_base = compter(&mut base, "SELECT COUNT(*) FROM vente WHERE dossier_id = ?1", &parametres![dossier.clone()]);
    assert_eq!(en_base, 55);

    let page = |base: &mut Base, p: i64, recherche: Option<&str>| {
        pagination::lire_ventes_recentes_paginee_sur_base(base, p, 50, recherche.map(str::to_string), None, None, None).unwrap()
    };
    let p0 = page(&mut base, 0, None);
    let p1 = page(&mut base, 1, None);
    assert_eq!((p0["total"].as_i64(), p0["pages"].as_i64()), (Some(55), Some(2)));
    let d0 = p0["donnees"].as_array().unwrap().clone();
    let d1 = p1["donnees"].as_array().unwrap().clone();
    assert_eq!((d0.len(), d1.len()), (50, 5));
    // Deux pages sans doublon, qui couvrent tout.
    let mut ids: Vec<String> = d0.iter().chain(d1.iter()).map(|v| v["id"].as_str().unwrap().to_string()).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), 55);
    // La plus recente d'abord.
    let dates: Vec<String> = d0.iter().chain(d1.iter()).map(|v| v["date_vente"].as_str().unwrap().to_string()).collect();
    assert!(dates.windows(2).all(|w| w[0] >= w[1]), "{dates:?}");
    // Ce que la fenetre de retour lit : client generique, deja revenu.
    assert!(d0[0]["lignes"][0]["deja_retourne"].is_number());
    assert!(d0.iter().chain(d1.iter()).all(|v| v["client_generique"].is_boolean()));

    // Le ticket rapporte : retrouve par le numero de sa facture.
    let t = page(&mut base, 0, Some(&numero));
    assert_eq!(t["total"], 1, "{t}");
    assert_eq!(t["donnees"][0]["id"], vente_passant.as_str());
    assert_eq!(t["donnees"][0]["client_generique"], true, "un passant : pas d'avoir");
    // Par le code du client, insensible a la casse.
    let code: String = base
        .lire_une("SELECT code FROM client WHERE id = ?1 AND dossier_id = ?2", &parametres![client.clone(), dossier.clone()], |r| {
            r.get::<String>(0)
        })
        .unwrap()
        .unwrap();
    assert_eq!(page(&mut base, 0, Some(&code.to_lowercase()))["total"], 54);

    // Un retour se voit dans la page.
    let vente = d1[0]["id"].as_str().unwrap().to_string();
    let ligne = d1[0]["lignes"][0]["id"].as_str().unwrap().to_string();
    retours::enregistrer_retour_sur_base(
        &mut base, vente.clone(), ligne, 1.0, "remboursement".into(), Some("especes".into()), None, None, None, None, None,
    )
    .unwrap();
    let p1 = page(&mut base, 1, None);
    let v = p1["donnees"].as_array().unwrap().iter().find(|v| v["id"] == vente.as_str()).expect("même page").clone();
    assert_eq!(v["lignes"][0]["deja_retourne"], 1.0);

    // L'autre liste (sans les lignes), meme pagination.
    let l0 = pagination::lire_ventes_paginees_sur_base(&mut base, 0, 50, None, None, None, None, None).unwrap();
    let l1 = pagination::lire_ventes_paginees_sur_base(&mut base, 1, 50, None, None, None, None, None).unwrap();
    assert_eq!((l0["total"].as_i64(), l0["donnees"].as_array().unwrap().len(), l1["donnees"].as_array().unwrap().len()), (Some(55), 50, 5));
    let n = pagination::lire_ventes_paginees_sur_base(&mut base, 0, 50, Some(numero), None, None, None, None).unwrap();
    assert_eq!(n["total"], 1);
}

// =====================================================================
//  Les versions `Connection` (la fenetre monoposte), corrigees ensemble
// =====================================================================

#[test]
fn les_versions_de_la_fenetre_suivent() {
    // Elles ne parlent qu'a SQLite : sur PostgreSQL, rien a prouver.
    if std::env::var("GESCOM_PG").is_ok() {
        return;
    }
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let sucre = article_unite(&mut base, "Sucre");
    let comptant = client_generique(&mut base);
    let f = fournisseur(&mut base, "Ets Diarra");
    let faf = pieces::creer_piece_fournisseur_sur_base(
        &mut base, f.clone(), "facture_fournisseur".into(), vec![ligne(&sucre, 2.0)], None, None, None, None, None,
    )
    .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    achats::valider_facture_fournisseur_sur_base(&mut base, faf.clone(), "comptant".into(), None, None, Some("patron".into())).unwrap();
    let r = vendre(&mut base, &comptant, "comptant", 1.0, Some(sucre.3));
    let vente = r["vente_id"].as_str().unwrap().to_string();
    let fac = argent::creer_facture_depuis_vente_sur_base(&mut base, vente.clone(), comptant, "comptant".into(), None).unwrap();
    let numero = fac["numero"].as_str().unwrap().to_string();

    let conn = base.sqlite().expect("SQLite");
    // D.9
    fournisseurs::modifier_fournisseur(conn, f.clone(), "Diarra & Fils".into(), None, None, None, None, None).unwrap();
    assert_eq!(pieces::lire_donnees_piece(conn, faf.clone()).unwrap()["piece"]["client_nom"], "Ets Diarra");
    let liste = pieces::lire_toutes_pieces_fournisseur(conn, None, None, Some("Ets".into()), Some(f.clone()), None, None).unwrap();
    assert_eq!(liste.len(), 1);
    assert_eq!(liste[0]["tiers_nom"], "Ets Diarra");
    // D.12
    let p = pagination::lire_ventes_recentes_paginee(conn, 0, 50, Some(numero.clone()), None, None, None).unwrap();
    assert_eq!(p["total"], 1, "{p}");
    assert_eq!(p["donnees"][0]["id"], vente.as_str());
    assert_eq!(p["donnees"][0]["client_generique"], true);
    assert!(p["donnees"][0]["lignes"][0]["deja_retourne"].is_number());
    let l = pagination::lire_ventes_paginees(conn, 0, 50, Some(numero.clone()), None, None, None, None).unwrap();
    assert_eq!(l["total"], 1);
    assert_eq!(l["donnees"][0]["numero_facture"], numero.as_str());
    // D.11
    let export = catalogue_csv::exporter_articles_csv(conn).unwrap();
    assert!(export.lines().next().unwrap().ends_with(";Suivre le stock"));
    let conn = base.sqlite_mut().expect("SQLite");
    let r = catalogue_csv::importer_articles_csv(
        conn,
        format!("{ENTETE}\nSachets;;u;100;;;;;non\nCordes;;m;300;;;;;bof\nSucre;;kg;950;;;;;non"),
        Some(true),
    )
    .unwrap();
    assert_eq!((r["crees"].as_i64(), r["nb_erreurs"].as_i64()), (Some(1), Some(2)), "{r}");
    assert_eq!(suivi(&mut base, "Sachets"), Some(0));
    assert_eq!(suivi(&mut base, "Sucre"), Some(1));
    assert_eq!(nb_journal(&mut base, "suivi_stock_change"), 1);
}

// =====================================================================
//  Le detecteur
// =====================================================================

#[test]
fn tout_passe_le_detecteur() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let sucre = article_unite(&mut base, "Sucre");
    let comptant = client_generique(&mut base);
    let awa = client_reel(&mut base);
    let f = fournisseur(&mut base, "Grossiste");
    let faf = pieces::creer_piece_fournisseur_sur_base(
        &mut base, f.clone(), "facture_fournisseur".into(), vec![ligne(&sucre, 2.0)], None, None, None, None, None,
    )
    .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    achats::valider_facture_fournisseur_sur_base(&mut base, faf.clone(), "comptant".into(), None, None, Some("patron".into())).unwrap();
    let r = vendre(&mut base, &comptant, "comptant", 1.0, Some(sucre.3));
    let passant = r["client_id"].as_str().unwrap().to_string();
    let vente = r["vente_id"].as_str().unwrap().to_string();
    let fac = argent::creer_facture_depuis_vente_sur_base(&mut base, vente, comptant, "comptant".into(), None).unwrap();
    let piece = fac["piece_id"].as_str().unwrap().to_string();
    let numero = fac["numero"].as_str().unwrap().to_string();

    base.auditer(true);
    // D.9
    fournisseurs::modifier_fournisseur_sur_base(&mut base, f.clone(), "Grossiste SA".into(), None, None, None, None, None, false).unwrap();
    pieces::lire_donnees_piece_sur_base(&mut base, faf).unwrap();
    pieces::lire_toutes_pieces_fournisseur_sur_base(&mut base, None, None, Some("gross".into()), None, None, None).unwrap();
    // D.10
    passage::lire_passages_sur(&mut base, Some("PAS".into())).unwrap();
    passage::rattacher_sur(&mut base, passant, awa).unwrap();
    pieces::lire_donnees_piece_sur_base(&mut base, piece).unwrap();
    pieces::lire_toutes_pieces_client_sur_base(&mut base, None, None, Some("PAS".into()), None, None, None, None, None, None, None)
        .unwrap();
    // D.11
    catalogue_csv::importer_articles_csv_sur_base(&mut base, format!("{ENTETE}\nSachets;;u;100;;;;;non\nSucre;;kg;800;;;;;oui"), None, true, false)
        .unwrap();
    catalogue_csv::exporter_articles_csv_sur_base(&mut base).unwrap();
    // D.12
    pagination::lire_ventes_recentes_paginee_sur_base(&mut base, 0, 50, Some(numero.clone()), None, None, None).unwrap();
    pagination::lire_ventes_paginees_sur_base(&mut base, 0, 50, Some(numero), None, None, None, None).unwrap();
}
