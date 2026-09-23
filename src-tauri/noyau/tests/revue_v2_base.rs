//! La revue du 23/09/2026 — ce que la v2 laissait passer.
//!
//! - D26 : l'auteur d'un geste est l'utilisateur de la session, pas le
//!   premier compte actif de son rôle ;
//! - D27 : le serveur juge la saisie — pas de règlement « par avoir »
//!   sans avoir, pas de paiement négatif, pas de quantité négative ;
//! - le règlement d'une créance sur SQLite est tout ou rien ;
//! - sur PostgreSQL, une erreur ignorée dans une transaction ne fait
//!   plus disparaître l'opération en silence.
//!
//! Chaque scénario vérifie la base APRÈS le geste (ou son refus) :
//! stock, caisse, créance. SQLite par défaut, PostgreSQL avec
//! `GESCOM_PG` — sauf les deux `sur_sqlite_*`, qui portent sur la
//! version `Connection` et n'existent que sur SQLite.

mod commun;

use commun::*;
use gescom_noyau::argent::{self, ParamsLigneInput};
use gescom_noyau::base::Base;
use gescom_noyau::{achats, auteur, auth, caisse, creances, parametres, pieces};

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

fn admin(base: &mut Base) -> String {
    base.lire_une(
        "SELECT u.id FROM utilisateur u JOIN role r ON r.id = u.role_id
         WHERE r.nom = 'patron' ORDER BY u.cree_le LIMIT 1",
        &[],
        |r| r.get::<String>(0),
    )
    .unwrap()
    .expect("un patron amorcé")
}

/// Deux caissiers du MÊME rôle — le cas que la v2 confondait.
fn deux_caissiers(base: &mut Base) -> (String, String) {
    let par = admin(base);
    let awa = auth::creer_utilisateur_sur_base(
        base, "Awa".into(), "awa".into(), None, "motdepasse-a".into(), "caissier".into(), par.clone(),
    )
    .expect("Awa");
    let bakary = auth::creer_utilisateur_sur_base(
        base, "Bakary".into(), "bakary".into(), None, "motdepasse-b".into(), "caissier".into(), par,
    )
    .expect("Bakary");
    (awa, bakary)
}

fn vente_a_credit(base: &mut Base, quantite: f64) -> (String, i64) {
    let depot = depot_defaut(base);
    let sucre = article_unite(base, "Sucre");
    let client = client_reel(base);
    let v = argent::creer_vente_sur_base(
        base, client, depot.clone(), "credit".into(),
        vec![ligne_vente(&sucre, &depot, quantite)], Some("patron".into()), None, None, None,
    )
    .expect("vente à crédit");
    (v["vente_id"].as_str().unwrap().to_string(), v["total"].as_i64().unwrap())
}

fn statut_vente(base: &mut Base, vente_id: &str) -> String {
    base.lire_une("SELECT statut FROM vente WHERE id = ?1", &parametres![vente_id], |r| r.get::<String>(0))
        .unwrap()
        .expect("vente")
}

fn paye(base: &mut Base, vente_id: &str) -> i64 {
    compter(
        base,
        "SELECT CAST(COALESCE(SUM(montant), 0) AS BIGINT) FROM paiement WHERE vente_id = ?1",
        &parametres![vente_id],
    )
}

fn caisse_nette(base: &mut Base) -> i64 {
    compter(
        base,
        "SELECT CAST(COALESCE(SUM(CASE WHEN sens = 'entree' THEN montant ELSE -montant END), 0) AS BIGINT)
         FROM mouvement_caisse",
        &[],
    )
}

// =====================================================================
//  D26 — qui a fait le geste
// =====================================================================

#[test]
fn deux_caissiers_du_meme_role_signent_chacun_leur_vente() {
    let mut base = base_avec_demo();
    let (awa, bakary) = deux_caissiers(&mut base);
    let depot = depot_defaut(&mut base);
    let sucre = article_unite(&mut base, "Sucre");
    let client = client_generique(&mut base);

    // Le serveur pose l'utilisateur de la session, et ne passe que le
    // RÔLE au noyau — exactement comme `api::rpc`.
    let vendre = |base: &mut Base, qui: &str| -> String {
        let _auteur = auteur::poser(qui);
        caisse::ouvrir_session_caisse_sur(base, 0, "caissier".into()).ok();
        let v = argent::creer_vente_sur_base(
            base, client.clone(), depot.clone(), "comptant".into(),
            vec![ligne_vente(&sucre, &depot, 1.0)], Some("caissier".into()),
            Some(sucre.3), None, None,
        )
        .expect("vente");
        v["vente_id"].as_str().unwrap().to_string()
    };
    let de_bakary = vendre(&mut base, &bakary);
    let d_awa = vendre(&mut base, &awa);

    let signe = |base: &mut Base, sql: &str, vente: &str| -> String {
        base.lire_une(sql, &parametres![vente], |r| r.get::<String>(0)).unwrap().expect(sql)
    };
    for (vente, qui) in [(&de_bakary, &bakary), (&d_awa, &awa)] {
        assert_eq!(&signe(&mut base, "SELECT auteur_id FROM vente WHERE id = ?1", vente), qui);
        assert_eq!(&signe(&mut base, "SELECT auteur_id FROM mouvement_stock WHERE operation_id = ?1", vente), qui);
        assert_eq!(&signe(&mut base, "SELECT auteur_id FROM paiement WHERE vente_id = ?1", vente), qui);
        assert_eq!(&signe(&mut base, "SELECT cree_par FROM mouvement_caisse WHERE operation_id = ?1", vente), qui);
    }

    // La caisse a été ouverte par Bakary (premier geste), pas par Awa.
    let ouvreur = base
        .lire_une("SELECT ouvert_par FROM session_caisse WHERE statut = 'ouverte'", &[], |r| r.get::<String>(0))
        .unwrap()
        .unwrap();
    assert_eq!(ouvreur, bakary, "la caisse porte son vrai ouvreur");
}

#[test]
fn la_garde_tombee_on_ne_signe_plus_au_nom_de_personne() {
    let mut base = base_avec_demo();
    let (_awa, bakary) = deux_caissiers(&mut base);
    {
        let _auteur = auteur::poser(&bakary);
        assert_eq!(argent::id_utilisateur_par_role_sur(&mut base, "caissier"), bakary);
    }
    // Hors requête (fenêtre monoposte, tâche du serveur) : l'ancien
    // repli, qui ne désigne pas Bakary.
    assert_eq!(auteur::courant(), None);
    let repli = argent::id_utilisateur_courant_sur(&mut base);
    assert_ne!(repli, bakary);
}

// =====================================================================
//  D27 — la saisie se juge au serveur
// =====================================================================

#[test]
fn regler_par_avoir_sans_avoir_est_refuse_et_la_creance_reste() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let (vente, total) = vente_a_credit(&mut base, 2.0);

    let refus = creances::regler_creance_sur_base(&mut base, vente.clone(), total, "avoir".into(), None)
        .unwrap_err();
    assert!(refus.contains("avoir"), "{refus}");
    let refus = creances::regler_creance_sur_base(&mut base, vente.clone(), total, "cadeau".into(), None)
        .unwrap_err();
    assert!(refus.contains("inconnu"), "{refus}");

    assert_eq!(statut_vente(&mut base, &vente), "creance_ouverte", "la dette ne s'efface pas");
    assert_eq!(paye(&mut base, &vente), 0);

    // Le chemin ordinaire, lui, passe toujours — et l'argent est en caisse.
    let r = creances::regler_creance_sur_base(&mut base, vente.clone(), total, "orange_money".into(), None)
        .expect("règlement ordinaire");
    assert_eq!(r["soldee"], true);
    assert_eq!(caisse_nette(&mut base), total);
}

#[test]
fn un_paiement_negatif_ne_vide_pas_la_caisse() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let (vente, total) = vente_a_credit(&mut base, 2.0);

    let refus = argent::enregistrer_paiement_sur_base(&mut base, vente.clone(), -50_000, "especes".into(), None)
        .unwrap_err();
    assert!(refus.contains("positif"), "{refus}");
    assert_eq!(caisse_nette(&mut base), 0, "aucune entrée négative");
    assert_eq!(paye(&mut base, &vente), 0);

    // L'ancienne porte ne dépasse plus le reste dû.
    argent::enregistrer_paiement_sur_base(&mut base, vente.clone(), total * 10, "especes".into(), None)
        .expect("paiement");
    assert_eq!(paye(&mut base, &vente), total, "plafonné au reste");
    assert_eq!(caisse_nette(&mut base), total);
    assert_eq!(statut_vente(&mut base, &vente), "payee");
}

#[test]
fn une_vente_a_quantite_negative_ne_remonte_pas_le_stock() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let depot = depot_defaut(&mut base);
    let sucre = article_unite(&mut base, "Sucre");
    let client = client_generique(&mut base);
    let avant = stock(&mut base, &sucre.0, &depot);
    let ventes_avant = compter(&mut base, "SELECT COUNT(*) FROM vente", &[]);

    let refus = argent::creer_vente_sur_base(
        &mut base, client.clone(), depot.clone(), "comptant".into(),
        vec![ligne_vente(&sucre, &depot, -5.0)], None, Some(0), None, None,
    )
    .unwrap_err();
    assert!(refus.contains("Quantité"), "{refus}");

    let mut prix_negatif = ligne_vente(&sucre, &depot, 1.0);
    prix_negatif.prix_pratique = -800;
    assert!(argent::creer_vente_sur_base(
        &mut base, client.clone(), depot.clone(), "comptant".into(), vec![prix_negatif], None, None, None, None,
    )
    .is_err());
    assert!(argent::creer_vente_sur_base(
        &mut base, client.clone(), depot.clone(), "comptant".into(), vec![], None, None, None, None,
    )
    .is_err(), "une vente sans ligne");
    assert!(argent::creer_vente_sur_base(
        &mut base, client, depot.clone(), "comptant".into(),
        vec![ligne_vente(&sucre, &depot, 1.0)], None, Some(sucre.3), Some("avoir".into()), None,
    )
    .is_err(), "« avoir » n'est pas un moyen d'encaisser au comptoir");

    assert_eq!(stock(&mut base, &sucre.0, &depot), avant);
    assert_eq!(compter(&mut base, "SELECT COUNT(*) FROM vente", &[]), ventes_avant);
    assert_eq!(caisse_nette(&mut base), 0);
}

#[test]
fn une_reception_ou_une_piece_a_quantite_negative_est_refusee() {
    let mut base = base_avec_demo();
    let depot = depot_defaut(&mut base);
    let sucre = article_unite(&mut base, "Sucre");
    let fournisseur = fournisseur(&mut base, "Grossiste");
    let client = client_reel(&mut base);
    let avant = stock(&mut base, &sucre.0, &depot);

    let refus = achats::enregistrer_achat_sur_base(
        &mut base, Some(fournisseur), Some(depot.clone()),
        vec![achats::LigneAchat {
            article_id: sucre.0.clone(), unite_vente_id: sucre.1.clone(),
            quantite: -10.0, facteur: 1.0, prix_achat: 500,
        }],
        Some("credit".into()), None, None, None, None, None,
    )
    .unwrap_err();
    assert!(refus.contains("Quantité"), "{refus}");
    assert_eq!(stock(&mut base, &sucre.0, &depot), avant, "une réception ne sort pas de stock");

    assert!(pieces::creer_piece_sur_base(
        &mut base, client.clone(), "facture".into(), vec![ligne(&sucre, -2.0)],
        None, None, None, None, None, None,
    )
    .is_err());
    let mut remise = ligne(&sucre, 1.0);
    remise.remise_pct = 150.0;
    assert!(pieces::creer_piece_sur_base(
        &mut base, client.clone(), "devis".into(), vec![remise], None, None, None, None, None, None,
    )
    .is_err(), "une remise de 150 %");
    // Une pièce sans ligne reste permise (avoir accordé, K7).
    pieces::creer_piece_sur_base(&mut base, client, "devis".into(), vec![], None, None, None, None, None, None)
        .expect("devis vide");
}

#[test]
fn un_reglement_fournisseur_negatif_ne_fait_pas_apparaitre_d_argent() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let f = fournisseur(&mut base, "Grossiste");
    let refus = argent::regler_dette_fournisseur_sur_base(&mut base, f, -20_000, "especes".into(), None, None)
        .unwrap_err();
    assert!(refus.contains("positif"), "{refus}");
    assert_eq!(caisse_nette(&mut base), 0);
    assert_eq!(compter(&mut base, "SELECT COUNT(*) FROM paiement_fournisseur", &[]), 0);
}

#[test]
fn tout_ce_qui_est_touche_par_la_revue_passe_le_detecteur() {
    let mut base = base_avec_demo();
    let (_awa, bakary) = deux_caissiers(&mut base);
    base.auditer(true);
    let _auteur = auteur::poser(&bakary);
    ouvrir_caisse(&mut base);
    let (vente, total) = vente_a_credit(&mut base, 1.0);
    creances::regler_creance_sur_base(&mut base, vente.clone(), total / 2, "especes".into(), None)
        .expect("règlement");
    argent::enregistrer_paiement_sur_base(&mut base, vente, total, "cheque".into(), None)
        .expect("paiement");
}

// =====================================================================
//  La version `Connection` (le serveur sur fichier SQLite)
// =====================================================================

fn base_sqlite() -> Base {
    // Ces scénarios portent sur les versions `conn` : sur PostgreSQL
    // ils ne prouvent rien, on les joue toujours en mémoire.
    let mut base = Base::ouvrir(":memory:").expect("base en mémoire");
    gescom_noyau::amorcage::amorcer(&mut base).expect("amorçage");
    gescom_noyau::amorcage::donnees_demo(&mut base).expect("démo");
    base
}

#[test]
fn sur_sqlite_le_reglement_est_tout_ou_rien() {
    let mut base = base_sqlite();
    ouvrir_caisse(&mut base);
    let (vente, total) = vente_a_credit(&mut base, 2.0);

    // Une panne au moment d'écrire la caisse — le dernier geste utile.
    base.executer_lot(
        "CREATE TRIGGER panne_caisse BEFORE INSERT ON mouvement_caisse
         BEGIN SELECT RAISE(ABORT, 'panne simulee'); END;",
    )
    .unwrap();
    let conn = base.sqlite().unwrap();
    let e = creances::regler_creance_datee(conn, vente.clone(), total, "especes".into(), None, None)
        .unwrap_err();
    assert!(e.contains("panne"), "{e}");
    assert_eq!(paye(&mut base, &vente), 0, "aucun paiement orphelin");
    assert_eq!(statut_vente(&mut base, &vente), "creance_ouverte");

    base.executer_lot("DROP TRIGGER panne_caisse;").unwrap();
    let conn = base.sqlite().unwrap();
    assert!(creances::regler_creance_datee(conn, vente.clone(), total, "avoir".into(), None, None).is_err());
    assert!(argent::enregistrer_paiement(conn, vente.clone(), -1_000, "especes".into(), None).is_err());
    creances::regler_creance_datee(conn, vente.clone(), total, "especes".into(), None, None)
        .expect("règlement");
    assert_eq!(paye(&mut base, &vente), total);
    assert_eq!(caisse_nette(&mut base), total);
}

#[test]
fn sur_sqlite_la_vente_juge_aussi_ses_lignes() {
    let mut base = base_sqlite();
    ouvrir_caisse(&mut base);
    let depot = depot_defaut(&mut base);
    let sucre = article_unite(&mut base, "Sucre");
    let client = client_generique(&mut base);
    let avant = stock(&mut base, &sucre.0, &depot);

    let conn = base.sqlite_mut().unwrap();
    let refus = argent::creer_vente_sur(
        conn, client, depot.clone(), "comptant".into(),
        vec![ligne_vente(&sucre, &depot, -5.0)], None, Some(0), None, None,
    )
    .unwrap_err();
    assert!(refus.contains("Quantité"), "{refus}");
    assert_eq!(stock(&mut base, &sucre.0, &depot), avant);
}

// =====================================================================
//  PostgreSQL : une erreur ignorée ne fait plus disparaître l'opération
// =====================================================================

/// Sur PostgreSQL, une instruction en échec avorte la transaction et
/// `COMMIT` répond `ROLLBACK` sans erreur : une écriture ignorée
/// (`let _ =`) faisait perdre toute l'opération pendant que la commande
/// rendait `Ok`. `valider` refuse désormais. Sur SQLite, l'échec ignoré
/// n'avorte rien et le reste s'écrit, comme avant.
#[test]
fn une_erreur_ignoree_dans_une_transaction_ne_se_valide_pas_en_silence() {
    let mut base = base_avec_demo();
    let dossier = base.dossier().to_string();
    let id = uuid::Uuid::new_v4().to_string();
    let now = gescom_noyau::utils::maintenant_iso();
    let pg = base.sqlite().is_none();

    let mut tx = base.transaction().unwrap();
    tx.executer(
        "INSERT INTO journal (id, type_evenement, entite_type, entite_id, origine, date_evenement, dossier_id)
         VALUES (?1, 'essai', 'essai', ?1, 'test', ?2, ?3)",
        &parametres![id.clone(), now, dossier],
    )
    .unwrap();
    // L'erreur ignorée, comme les `let _ = tx.executer(...)` du noyau.
    let _ = tx.executer("INSERT INTO table_qui_n_existe_pas (x) VALUES (1)", &[]);
    let resultat = tx.valider();

    let ecrit = compter(&mut base, "SELECT COUNT(*) FROM journal WHERE id = ?1", &parametres![id]);
    if pg {
        let e = resultat.unwrap_err();
        assert!(e.0.contains("Rien n'a été enregistré"), "{}", e.0);
        assert_eq!(ecrit, 0);
    } else {
        resultat.expect("SQLite valide le reste");
        assert_eq!(ecrit, 1);
    }
}
