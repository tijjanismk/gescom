//! `fournisseurs` et le règlement fournisseur d'`argent` — sur `Base`.
//! Testé sur SQLite par défaut, sur PostgreSQL avec `GESCOM_PG`.

mod commun;

use commun::*;
use gescom_noyau::base::Base;
use gescom_noyau::parametres;
use gescom_noyau::{achats, argent, fournisseurs};

/// Un achat à crédit de `quantite` kg de sucre à 500 F : une FAF de
/// `quantite × 500` due. Rend l'id de la pièce.
fn acheter_a_credit(base: &mut Base, f: &str, quantite: f64) -> String {
    let sucre = article_unite(base, "Sucre");
    let r = achats::enregistrer_achat_sur_base(
        base, Some(f.to_string()), None,
        vec![achats::LigneAchat { article_id: sucre.0.clone(), unite_vente_id: sucre.1.clone(), quantite, facteur: 1.0, prix_achat: 500 }],
        Some("credit".into()), None, None, None, None, None,
    )
    .expect("achat");
    r["piece_id"].as_str().unwrap().to_string()
}

#[test]
fn un_fournisseur_se_cree_se_modifie_et_se_lit() {
    let mut base = base_avec_demo();
    let r = fournisseurs::creer_fournisseur_sur_base(&mut base, "Grossiste".into(), Some("70 00 00 00".into()), None, None, None, Some(true)).unwrap();
    let id = r["id"].as_str().unwrap().to_string();
    let liste = fournisseurs::lire_fournisseurs_sur_base(&mut base).unwrap();
    assert_eq!(liste.len(), 1);
    assert_eq!(liste[0]["est_voisin"], true);

    fournisseurs::modifier_fournisseur_sur_base(&mut base, id.clone(), " Grossiste Nord ".into(), Some("  ".into()), Some("Sikasso".into()), None, None, None).unwrap();
    let d = fournisseurs::lire_fournisseur_detail_sur_base(&mut base, id.clone()).unwrap();
    assert_eq!(d["nom"], "Grossiste Nord");
    assert!(d["telephone"].is_null(), "un champ vidé redevient NULL");
    assert_eq!(d["adresse"], "Sikasso");
    assert_eq!(d["est_voisin"], false);
    assert!(fournisseurs::modifier_fournisseur_sur_base(&mut base, "x".into(), "y".into(), None, None, None, None, None).is_err());
    assert!(fournisseurs::modifier_fournisseur_sur_base(&mut base, id, " ".into(), None, None, None, None, None).is_err());

    base.choisir_dossier("dossier-b").unwrap();
    assert!(fournisseurs::lire_fournisseurs_sur_base(&mut base).unwrap().is_empty());
}

#[test]
fn un_reglement_global_se_repartit_de_la_plus_ancienne_a_la_plus_recente() {
    let mut base = base_avec_demo();
    let f = fournisseur(&mut base, "Grossiste");
    let faf1 = acheter_a_credit(&mut base, &f, 4.0); // 2 000
    let faf2 = acheter_a_credit(&mut base, &f, 6.0); // 3 000

    let dettes = fournisseurs::lire_fournisseurs_avec_dettes_sur_base(&mut base).unwrap();
    assert_eq!(dettes[0]["dette"], 5_000);
    let etat = fournisseurs::lire_etat_dette_fournisseur_sur_base(&mut base, f.clone()).unwrap();
    assert_eq!(etat["lignes"].as_array().unwrap().len(), 2);
    assert_eq!(etat["net_du"], 5_000);
    let global = fournisseurs::lire_etat_dettes_global_sur_base(&mut base).unwrap();
    assert_eq!(global["total_general"], 5_000);
    assert_eq!(global["lignes"][0]["nb"], 2);

    // Caisse fermée : refus.
    assert!(argent::regler_dette_fournisseur_sur_base(&mut base, f.clone(), 2_500, "especes".into(), None, None).unwrap_err().contains("CAISSE_FERMEE"));
    ouvrir_caisse(&mut base);
    let r = argent::regler_dette_fournisseur_sur_base(&mut base, f.clone(), 2_500, "especes".into(), None, None).unwrap();
    assert_eq!(r["soldees"].as_array().unwrap().len(), 1, "la plus ancienne est soldée : {r}");
    assert_eq!(statut_piece(&mut base, &faf1), "paye");
    assert_eq!(statut_piece(&mut base, &faf2), "emis");
    // Une ligne de paiement PAR facture couverte, aucune sans piece_id.
    assert_eq!(compter(&mut base, "SELECT COUNT(*) FROM paiement_fournisseur WHERE piece_id IS NULL", &[]), 0);
    assert_eq!(compter(&mut base, "SELECT CAST(COALESCE(SUM(montant),0) AS BIGINT) FROM paiement_fournisseur WHERE piece_id = ?1", &parametres![faf2.clone()]), 500);
    let sorties = compter(&mut base, "SELECT CAST(COALESCE(SUM(montant),0) AS BIGINT) FROM mouvement_caisse WHERE sens = 'sortie' AND motif = 'reglement_fournisseur'", &[]);
    assert_eq!(sorties, 2_500);

    let fiche = fournisseurs::lire_fiche_fournisseur_sur_base(&mut base, f.clone()).unwrap();
    assert_eq!(fiche["stats"]["dette"], 2_500);
    assert_eq!(fiche["stats"]["total_paye"], 2_500);
    assert_eq!(fiche["stats"]["nb_achats"], 2);
    assert_eq!(fiche["paiements"].as_array().unwrap().len(), 2);
    assert_eq!(fiche["achats"].as_array().unwrap().len(), 2);
    assert!(fiche["achats"][0]["piece_numero"].as_str().unwrap().starts_with("FAF-"));

    // Imputation explicite sur la seconde, plafonnée par rien : le reste tombe à 0.
    let r = argent::regler_dette_fournisseur_sur_base(&mut base, f.clone(), 2_500, "especes".into(), Some("solde".into()), Some(faf2.clone())).unwrap();
    assert_eq!(r["soldees"].as_array().unwrap().len(), 2);
    assert_eq!(statut_piece(&mut base, &faf2), "paye");
    assert!(fournisseurs::lire_etat_dette_fournisseur_sur_base(&mut base, f).unwrap()["lignes"].as_array().unwrap().is_empty());
}

#[test]
fn annuler_un_paiement_fournisseur_rouvre_la_facture_et_fait_rentrer_l_argent() {
    let mut base = base_avec_demo();
    let f = fournisseur(&mut base, "Grossiste");
    let faf = acheter_a_credit(&mut base, &f, 2.0); // 1 000
    ouvrir_caisse(&mut base);
    argent::regler_dette_fournisseur_sur_base(&mut base, f.clone(), 1_000, "especes".into(), None, Some(faf.clone())).unwrap();
    assert_eq!(statut_piece(&mut base, &faf), "paye");
    let pid: String = base
        .lire_une("SELECT id FROM paiement_fournisseur WHERE piece_id = ?1", &parametres![faf.clone()], |r| r.get::<String>(0))
        .unwrap()
        .unwrap();

    assert!(fournisseurs::annuler_paiement_fournisseur_sur_base(&mut base, pid.clone(), " ".into(), true, None).is_err());
    let r = fournisseurs::annuler_paiement_fournisseur_sur_base(&mut base, pid.clone(), "saisi deux fois".into(), true, None).unwrap();
    assert_eq!(r["montant_annule"], 1_000);
    assert_eq!(r["entree_de_caisse"], true);
    assert_eq!(statut_piece(&mut base, &faf), "emis", "la facture redevient due");
    let entrees = compter(&mut base, "SELECT CAST(COALESCE(SUM(montant),0) AS BIGINT) FROM mouvement_caisse WHERE sens = 'entree' AND motif = 'remboursement'", &[]);
    assert_eq!(entrees, 1_000);
    assert!(fournisseurs::annuler_paiement_fournisseur_sur_base(&mut base, pid, "encore".into(), true, None).unwrap_err().contains("déjà"));
    let fiche = fournisseurs::lire_fiche_fournisseur_sur_base(&mut base, f).unwrap();
    assert_eq!(fiche["stats"]["dette"], 1_000);
    assert!(fiche["paiements"].as_array().unwrap().iter().any(|p| p["est_annulation"] == true && p["montant"] == -1_000));
    assert!(fiche["paiements"].as_array().unwrap().iter().any(|p| p["annule"] == true));
}

#[test]
fn entree_retour_sans_facture_et_ajustement_bougent_le_stock_sans_argent() {
    let mut base = base_avec_demo();
    let sucre = article_unite(&mut base, "Sucre");
    let depot = depot_defaut(&mut base);
    let avant = stock(&mut base, &sucre.0, &depot);

    fournisseurs::enregistrer_entree_stock_sur_base(&mut base, sucre.0.clone(), None, 10.0, Some(450), None, None).unwrap();
    assert_eq!(stock(&mut base, &sucre.0, &depot), avant + 10.0);
    assert_eq!(compter(&mut base, "SELECT dernier_prix_achat FROM article WHERE id = ?1", &parametres![sucre.0.clone()]), 450);
    assert_eq!(compter(&mut base, "SELECT COUNT(*) FROM piece_commerciale", &[]), 0, "ni pièce ni dette (D42)");

    let refus = fournisseurs::enregistrer_retour_sans_facture_sur_base(&mut base, sucre.0.clone(), None, avant + 11.0, None, None, None).unwrap_err();
    assert!(refus.contains("Stock insuffisant"));
    fournisseurs::enregistrer_retour_sans_facture_sur_base(&mut base, sucre.0.clone(), None, 10.0, None, Some("dépannage rendu".into()), None).unwrap();
    assert_eq!(stock(&mut base, &sucre.0, &depot), avant);
    assert_eq!(compter(&mut base, "SELECT COUNT(*) FROM journal WHERE type_evenement = 'retour_sans_facture'", &[]), 1);

    fournisseurs::enregistrer_ajustement_inventaire_sur_base(&mut base, sucre.0.clone(), depot.clone(), 150.0, Some("inventaire".into()), None).unwrap();
    assert_eq!(stock(&mut base, &sucre.0, &depot), 150.0);
    assert_eq!(compter(&mut base, "SELECT COUNT(*) FROM mouvement_caisse", &[]), 0, "aucun argent n'a bougé");
}

#[test]
fn tout_ce_qui_est_porte_ici_passe_le_detecteur() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let sucre = article_unite(&mut base, "Sucre");
    let depot = depot_defaut(&mut base);
    let f = fournisseur(&mut base, "F");
    let faf = acheter_a_credit(&mut base, &f, 1.0);
    base.auditer(true);

    fournisseurs::lire_fournisseurs_sur_base(&mut base).expect("liste");
    fournisseurs::lire_fournisseurs_avec_dettes_sur_base(&mut base).expect("dettes");
    let g = fournisseurs::creer_fournisseur_sur_base(&mut base, "G".into(), None, None, None, None, None).expect("créer");
    fournisseurs::modifier_fournisseur_sur_base(&mut base, g["id"].as_str().unwrap().to_string(), "G2".into(), None, None, None, None, None).expect("modifier");
    fournisseurs::lire_etat_dette_fournisseur_sur_base(&mut base, f.clone()).expect("état");
    fournisseurs::lire_etat_dettes_global_sur_base(&mut base).expect("global");
    fournisseurs::enregistrer_entree_stock_sur_base(&mut base, sucre.0.clone(), None, 1.0, None, Some(f.clone()), None).expect("entrée");
    fournisseurs::enregistrer_retour_sans_facture_sur_base(&mut base, sucre.0.clone(), None, 1.0, Some(f.clone()), None, None).expect("retour");
    fournisseurs::enregistrer_ajustement_inventaire_sur_base(&mut base, sucre.0.clone(), depot, 200.0, None, None).expect("ajustement");
    fournisseurs::lire_fournisseur_detail_sur_base(&mut base, f.clone()).expect("détail");
    argent::regler_dette_fournisseur_sur_base(&mut base, f.clone(), 200, "especes".into(), None, None).expect("régler");
    argent::regler_dette_fournisseur_sur_base(&mut base, f.clone(), 300, "especes".into(), None, Some(faf)).expect("régler imputé");
    let fiche = fournisseurs::lire_fiche_fournisseur_sur_base(&mut base, f).expect("fiche");
    let pid = fiche["paiements"][0]["id"].as_str().unwrap().to_string();
    fournisseurs::annuler_paiement_fournisseur_sur_base(&mut base, pid, "m".into(), false, None).expect("annuler");
}
