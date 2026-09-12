//! `achats::*_sur_base` — les achats et retours fournisseur, sur `Base`.
//!
//! L'achat fait entrer le stock ET sortir l'argent ; ces scénarios
//! vérifient les deux, plus la dette que `paiement_fournisseur` porte
//! (D36) et le refus caisse fermée (D46). Testé sur SQLite par défaut,
//! sur PostgreSQL avec `GESCOM_PG` (voir `commun`).

mod commun;

use commun::*;
use gescom_noyau::achats::{self, LigneAchat, LigneRetourFournisseur};
use gescom_noyau::base::Base;
use gescom_noyau::parametres;
use gescom_noyau::pieces;

fn ligne_achat(a: &(String, String, f64, i64), quantite: f64, prix: i64) -> LigneAchat {
    LigneAchat {
        article_id: a.0.clone(),
        unite_vente_id: a.1.clone(),
        quantite,
        facteur: a.2,
        prix_achat: prix,
    }
}

fn ligne_retour(a: &(String, String, f64, i64), quantite: f64, prix: i64) -> LigneRetourFournisseur {
    LigneRetourFournisseur {
        article_id: a.0.clone(),
        unite_vente_id: a.1.clone(),
        quantite,
        facteur: a.2,
        prix_achat: prix,
    }
}

fn achat_comptant(base: &mut Base, f: &str, quantite: f64, prix: i64) -> serde_json::Value {
    let sucre = article_unite(base, "Sucre");
    achats::enregistrer_achat_sur_base(
        base,
        Some(f.to_string()),
        None,
        vec![ligne_achat(&sucre, quantite, prix)],
        Some("comptant".into()),
        None,
        None,
        None,
        Some("patron".into()),
        None,
    )
    .expect("achat comptant")
}

fn caisse_solde_sorties(base: &mut Base) -> i64 {
    compter(
        base,
        "SELECT CAST(COALESCE(SUM(montant), 0) AS BIGINT) FROM mouvement_caisse WHERE sens = 'sortie'",
        &[],
    )
}

#[test]
fn un_achat_comptant_fait_entrer_le_stock_et_sortir_l_argent() {
    let mut base = base_avec_demo();
    let sucre = article_unite(&mut base, "Sucre");
    let depot = depot_defaut(&mut base);
    let f = fournisseur(&mut base, "Grossiste");
    let avant = stock(&mut base, &sucre.0, &depot);
    ouvrir_caisse(&mut base);

    let r = achat_comptant(&mut base, &f, 10.0, 500);
    assert_eq!(r["total"], 5_000);
    assert_eq!(r["regle"], 5_000);
    assert_eq!(r["statut"], "paye");
    assert!(r["numero"].as_str().unwrap().starts_with("FAF-"));

    assert_eq!(stock(&mut base, &sucre.0, &depot), avant + 10.0);
    assert_eq!(caisse_solde_sorties(&mut base), 5_000, "l'argent quitte le tiroir");
    let piece_id = r["piece_id"].as_str().unwrap().to_string();
    assert_eq!(statut_piece(&mut base, &piece_id), "paye");
    let regle = compter(
        &mut base,
        "SELECT CAST(COALESCE(SUM(montant), 0) AS BIGINT) FROM paiement_fournisseur WHERE piece_id = ?1",
        &parametres![piece_id.clone()],
    );
    assert_eq!(regle, 5_000, "c'est ce paiement que lit la dette (D36)");

    // Le mouvement porte l'id de la PIECE (D51).
    let n = compter(
        &mut base,
        "SELECT COUNT(*) FROM mouvement_stock WHERE operation_id = ?1 AND type_mouvement = 'achat'",
        &parametres![piece_id],
    );
    assert_eq!(n, 1);
    // Et le dernier prix d'achat suit.
    let dernier = compter(&mut base, "SELECT dernier_prix_achat FROM article WHERE id = ?1", &parametres![sucre.0.clone()]);
    assert_eq!(dernier, 500);
}

#[test]
fn caisse_fermee_l_achat_comptant_est_refuse_sans_rien_ecrire() {
    let mut base = base_avec_demo();
    let sucre = article_unite(&mut base, "Sucre");
    let depot = depot_defaut(&mut base);
    let f = fournisseur(&mut base, "Grossiste");
    let avant = stock(&mut base, &sucre.0, &depot);

    let refus = achats::enregistrer_achat_sur_base(
        &mut base, Some(f), None, vec![ligne_achat(&sucre, 1.0, 500)],
        Some("comptant".into()), None, None, None, None, None,
    )
    .unwrap_err();
    assert!(refus.contains("CAISSE_FERMEE"), "{refus}");
    assert_eq!(stock(&mut base, &sucre.0, &depot), avant, "rien n'est entré");
    assert_eq!(compter(&mut base, "SELECT COUNT(*) FROM piece_commerciale WHERE type_piece = 'facture_fournisseur'", &[]), 0);

    // A crédit sans acompte, la caisse n'a pas à être ouverte.
    let autre = fournisseur(&mut base, "Autre");
    let r = achats::enregistrer_achat_sur_base(
        &mut base, Some(autre), None, vec![ligne_achat(&sucre, 1.0, 500)],
        Some("credit".into()), None, None, None, None, None,
    )
    .expect("achat à crédit caisse fermée");
    assert_eq!(r["statut"], "emis");
    assert_eq!(r["regle"], 0);
}

#[test]
fn un_acompte_a_credit_ne_regle_que_l_acompte() {
    let mut base = base_avec_demo();
    let sucre = article_unite(&mut base, "Sucre");
    let f = fournisseur(&mut base, "Grossiste");
    ouvrir_caisse(&mut base);

    let r = achats::enregistrer_achat_sur_base(
        &mut base, Some(f), None, vec![ligne_achat(&sucre, 10.0, 500)],
        Some("credit".into()), Some("especes".into()), Some(2_000), None, None, None,
    )
    .unwrap();
    assert_eq!(r["statut"], "emis");
    assert_eq!(r["regle"], 2_000);
    assert_eq!(caisse_solde_sorties(&mut base), 2_000, "seul l'acompte sort du tiroir");
}

#[test]
fn un_achat_sans_fournisseur_entre_le_stock_sans_piece() {
    let mut base = base_avec_demo();
    let sucre = article_unite(&mut base, "Sucre");
    let depot = depot_defaut(&mut base);
    let avant = stock(&mut base, &sucre.0, &depot);

    let r = achats::enregistrer_achat_sur_base(
        &mut base, None, None, vec![ligne_achat(&sucre, 3.0, 500)],
        Some("credit".into()), None, None, None, None, None,
    )
    .unwrap();
    assert!(r["piece_id"].is_null());
    assert_eq!(stock(&mut base, &sucre.0, &depot), avant + 3.0);

    let refus = achats::enregistrer_achat_sur_base(
        &mut base, None, None, vec![], None, None, None, None, None, None,
    )
    .unwrap_err();
    assert!(refus.contains("Aucune ligne"));
}

#[test]
fn un_retour_fournisseur_sort_le_stock_et_refuse_le_decouvert() {
    let mut base = base_avec_demo();
    let sucre = article_unite(&mut base, "Sucre");
    let depot = depot_defaut(&mut base);
    let f = fournisseur(&mut base, "Grossiste");
    ouvrir_caisse(&mut base);
    let achat = achat_comptant(&mut base, &f, 10.0, 500);
    let faf = achat["piece_id"].as_str().unwrap().to_string();
    let avant = stock(&mut base, &sucre.0, &depot);

    let r = achats::enregistrer_retour_fournisseur_sur_base(
        &mut base, f.clone(), None, vec![ligne_retour(&sucre, 4.0, 500)],
        Some(faf.clone()), Some("avoir".into()), None, Some("abîmé".into()), None,
    )
    .unwrap();
    assert!(r["numero"].as_str().unwrap().starts_with("AVF-"));
    assert_eq!(r["total"], 2_000);
    assert_eq!(r["statut"], "emis", "un avoir réduit la dette, la caisse ne bouge pas");
    assert_eq!(stock(&mut base, &sucre.0, &depot), avant - 4.0);
    assert_eq!(caisse_solde_sorties(&mut base), 5_000, "inchangé depuis l'achat");

    // Le reliquat retournable de la facture a diminué d'autant.
    let factures = achats::lire_factures_fournisseur_retournables_sur_base(&mut base, f.clone()).unwrap();
    assert_eq!(factures.len(), 1);
    let l = &factures[0]["lignes"][0];
    assert_eq!(l["deja_retourne"], 4.0);
    assert_eq!(l["quantite_restante"], 6.0);
    assert_eq!(factures[0]["total"], 5_000);

    // Le stock disponible borne ce qu'on peut rendre.
    let en_stock = stock(&mut base, &sucre.0, &depot);
    let refus = achats::enregistrer_retour_fournisseur_sur_base(
        &mut base, f.clone(), None, vec![ligne_retour(&sucre, en_stock + 1.0, 500)],
        Some(faf), None, None, None, None,
    )
    .unwrap_err();
    assert!(refus.contains("Stock insuffisant"), "{refus}");

    // Un remboursement fait entrer l'argent et solde l'AVF.
    let r = achats::enregistrer_retour_fournisseur_sur_base(
        &mut base, f, None, vec![ligne_retour(&sucre, 1.0, 500)],
        None, Some("remboursement".into()), Some("especes".into()), None, None,
    )
    .unwrap();
    assert_eq!(r["statut"], "paye");
    let entrees = compter(
        &mut base,
        "SELECT CAST(COALESCE(SUM(montant), 0) AS BIGINT) FROM mouvement_caisse
         WHERE sens = 'entree' AND motif = 'retour_fournisseur'",
        &[],
    );
    assert_eq!(entrees, 500);
}

#[test]
fn valider_une_facture_fournisseur_brouillon_fait_entrer_le_stock_une_fois() {
    let mut base = base_avec_demo();
    let sucre = article_unite(&mut base, "Sucre");
    let depot = depot_defaut(&mut base);
    let f = fournisseur(&mut base, "Grossiste");
    ouvrir_caisse(&mut base);
    let avant = stock(&mut base, &sucre.0, &depot);

    // FAF directe, sans bon : la validation entre le stock elle-même.
    let faf = pieces::creer_piece_fournisseur_sur_base(
        &mut base, f.clone(), "facture_fournisseur".into(), vec![ligne(&sucre, 6.0)], None, None, None, None,
    )
    .unwrap();
    assert_eq!(faf["statut"], "brouillon");
    let faf_id = faf["id"].as_str().unwrap().to_string();

    let r = achats::valider_facture_fournisseur_sur_base(
        &mut base, faf_id.clone(), "comptant".into(), None, None, Some("patron".into()),
    )
    .unwrap();
    assert_eq!(r["statut"], "paye");
    assert_eq!(r["stock_entre"], true);
    assert_eq!(r["total"], 6 * sucre.3);
    assert_eq!(stock(&mut base, &sucre.0, &depot), avant + 6.0);
    assert_eq!(caisse_solde_sorties(&mut base), 6 * sucre.3);

    let refus = achats::valider_facture_fournisseur_sur_base(&mut base, faf_id, "comptant".into(), None, None, None)
        .unwrap_err();
    assert!(refus.contains("déjà"), "{refus}");

    // La chaîne BCF → BRF → FAF : la réception entre le stock, la
    // facture ne le fait pas entrer une seconde fois.
    let avant = stock(&mut base, &sucre.0, &depot);
    let bcf = pieces::creer_piece_fournisseur_sur_base(
        &mut base, f, "bon_commande_fournisseur".into(), vec![ligne(&sucre, 8.0)], None, None, None, None,
    )
    .unwrap();
    let brf = pieces::convertir_piece_sur_base(&mut base, bcf["id"].as_str().unwrap().to_string(), "bon_reception".into()).unwrap();
    let faf2 = pieces::convertir_piece_sur_base(&mut base, brf["id"].as_str().unwrap().to_string(), "facture_fournisseur".into()).unwrap();
    assert_eq!(faf2["statut"], "brouillon");
    let r = achats::valider_facture_fournisseur_sur_base(
        &mut base, faf2["id"].as_str().unwrap().to_string(), "credit".into(), None, None, None,
    )
    .unwrap();
    assert_eq!(r["stock_entre"], false, "le bon de réception s'en est chargé");
    assert_eq!(r["statut"], "emis");
    let _ = avant;
}

#[test]
fn annuler_une_facture_fournisseur_par_avoir_rend_tout() {
    let mut base = base_avec_demo();
    let sucre = article_unite(&mut base, "Sucre");
    let depot = depot_defaut(&mut base);
    let f = fournisseur(&mut base, "Grossiste");
    ouvrir_caisse(&mut base);
    let avant = stock(&mut base, &sucre.0, &depot);
    let achat = achat_comptant(&mut base, &f, 5.0, 400);
    let faf = achat["piece_id"].as_str().unwrap().to_string();

    let r = achats::annuler_facture_fournisseur_par_avoir_sur_base(
        &mut base, faf.clone(), None, None, None, None,
    )
    .unwrap();
    assert_eq!(r["total"], 2_000);
    assert_eq!(statut_piece(&mut base, &faf), "annule");
    assert_eq!(stock(&mut base, &sucre.0, &depot), avant, "la marchandise repart");

    let refus = achats::annuler_facture_fournisseur_par_avoir_sur_base(&mut base, faf, None, None, None, None).unwrap_err();
    assert!(refus.contains("déjà annulée"));

    // Un brouillon n'a rien à annuler.
    let brouillon = pieces::creer_piece_fournisseur_sur_base(
        &mut base, f, "facture_fournisseur".into(), vec![ligne(&sucre, 1.0)], None, None, None, None,
    )
    .unwrap();
    let refus = achats::annuler_facture_fournisseur_par_avoir_sur_base(
        &mut base, brouillon["id"].as_str().unwrap().to_string(), None, None, None, None,
    )
    .unwrap_err();
    assert!(refus.contains("brouillon"));
}

#[test]
fn deux_dossiers_ne_partagent_ni_achats_ni_dettes() {
    let mut base = base_avec_demo();
    let f = fournisseur(&mut base, "Grossiste");
    ouvrir_caisse(&mut base);
    achat_comptant(&mut base, &f, 1.0, 500);

    base.choisir_dossier("dossier-b").unwrap();
    let factures = achats::lire_factures_fournisseur_retournables_sur_base(&mut base, f).unwrap();
    assert!(factures.is_empty(), "la facture de l'autre société ne doit pas se voir");
}

#[test]
fn tout_ce_qui_est_porte_dans_achats_passe_le_detecteur() {
    let mut base = base_avec_demo();
    let sucre = article_unite(&mut base, "Sucre");
    let f = fournisseur(&mut base, "Grossiste");
    ouvrir_caisse(&mut base);
    base.auditer(true);

    let achat = achat_comptant(&mut base, &f, 10.0, 500);
    let faf = achat["piece_id"].as_str().unwrap().to_string();
    achats::lire_factures_fournisseur_retournables_sur_base(&mut base, f.clone()).expect("retournables");
    achats::enregistrer_retour_fournisseur_sur_base(
        &mut base, f.clone(), None, vec![ligne_retour(&sucre, 1.0, 500)],
        Some(faf), Some("remboursement".into()), None, None, None,
    )
    .expect("retour");
    let brouillon = pieces::creer_piece_fournisseur_sur_base(
        &mut base, f, "facture_fournisseur".into(), vec![ligne(&sucre, 2.0)], None, None, None, None,
    )
    .unwrap();
    let id = brouillon["id"].as_str().unwrap().to_string();
    achats::valider_facture_fournisseur_sur_base(&mut base, id.clone(), "comptant".into(), None, None, None)
        .expect("valider");
    achats::annuler_facture_fournisseur_par_avoir_sur_base(&mut base, id, None, None, None, None)
        .expect("annuler par avoir");
}
