//! D36 — un article qui ne suit pas son stock, sur les deux moteurs.
//!
//! Il se vend sans jamais etre a decouvert (comptoir et facture), ses
//! mouvements s'ecrivent quand meme ; il sort de l'ecran Stock ; le
//! transfert et l'inventaire le refusent ; cesser dit ce qui sort du
//! stock, reprendre exige un comptage par magasin et repart de la
//! quantite reelle ; le geste est trace.
//!
//!   cargo test -p gescom-noyau --test suivi_stock_base
//!   GESCOM_PG=postgresql://…/gescom_test cargo test -p gescom-noyau --test suivi_stock_base -- --test-threads=1

mod commun;

use commun::*;
use gescom_noyau::argent::{self, ParamsLigneInput};
use gescom_noyau::base::Base;
use gescom_noyau::parametres;
use gescom_noyau::utils::maintenant_iso;
use gescom_noyau::{catalogue, fournisseurs, pagination, suivi_stock, transferts};

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

fn arreter_le_suivi(base: &mut Base, article: &str) -> serde_json::Value {
    suivi_stock::changer_suivi_stock_sur(base, article.to_string(), false, vec![]).expect("cesser de suivre")
}

fn decouvert_de(base: &mut Base, vente_id: &str) -> i64 {
    base.lire_une("SELECT vente_a_decouvert FROM ligne_vente WHERE vente_id = ?1", &parametres![vente_id], |r| r.get::<i64>(0))
        .unwrap()
        .unwrap()
}

fn mouvements_de_vente(base: &mut Base, article: &str) -> i64 {
    let dossier = base.dossier().to_string();
    compter(
        base,
        "SELECT COUNT(*) FROM mouvement_stock WHERE article_id = ?1 AND type_mouvement = 'vente' AND dossier_id = ?2",
        &parametres![article, dossier],
    )
}

#[test]
fn cesser_de_suivre_dit_ce_qui_sort_et_la_vente_n_est_plus_a_decouvert() {
    let mut base = base_avec_demo();
    let depot = depot_defaut(&mut base);
    let sucre = article_unite(&mut base, "Sucre"); // 200 en stock (démo)
    let avant = stock(&mut base, &sucre.0, &depot);

    let r = arreter_le_suivi(&mut base, &sucre.0);
    assert_eq!(r["suivi"], false);
    assert_eq!(r["sortis_du_stock"].as_f64().unwrap(), avant, "la réponse dit ce qui sort du stock");
    assert!(suivi_stock::changer_suivi_stock_sur(&mut base, sucre.0.clone(), false, vec![]).unwrap_err().contains("déjà"));

    // Au comptoir, au-dela du stock : jamais a decouvert, mouvement ecrit.
    let client = client_reel(&mut base);
    let v = argent::creer_vente_sur_base(
        &mut base, client, depot.clone(), "comptant".into(),
        vec![ligne_vente(&sucre, &depot, avant + 50.0)], None, None, None, None,
    )
    .unwrap();
    assert_eq!(decouvert_de(&mut base, v["vente_id"].as_str().unwrap()), 0, "non suivi : jamais à découvert");
    assert_eq!(mouvements_de_vente(&mut base, &sucre.0), 1, "le mouvement s'écrit quand même");

    // Un article suivi, lui, l'est toujours.
    let riz = article_unite(&mut base, "Riz local");
    let stock_riz = stock(&mut base, &riz.0, &depot);
    let client = client_reel(&mut base);
    let v = argent::creer_vente_sur_base(
        &mut base, client, depot.clone(), "comptant".into(),
        vec![ligne_vente(&riz, &depot, stock_riz + 1.0)], None, None, None, None,
    )
    .unwrap();
    assert_eq!(decouvert_de(&mut base, v["vente_id"].as_str().unwrap()), 1, "suivi : à découvert, comme avant");
}

#[test]
fn une_facture_d_un_article_non_suivi_n_est_pas_a_decouvert() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let depot = depot_defaut(&mut base);
    let sucre = article_unite(&mut base, "Sucre");
    arreter_le_suivi(&mut base, &sucre.0);
    let dossier = base.dossier().to_string();
    let now = maintenant_iso();
    let piece = uuid::Uuid::new_v4().to_string();
    let client = client_reel(&mut base);
    let quantite = 500.0;
    base.executer(
        "INSERT INTO piece_commerciale
           (id, type_piece, numero, statut, tiers_type, tiers_id, depot_id,
            auteur_id, date_piece, remise_globale, cree_le, modifie_le, origine, dossier_id)
         VALUES (?1,'facture',?2,'brouillon','client',?3,?4,'test',?5,0.0,?5,?5,'test',?6)",
        &parametres![piece.clone(), format!("FAC-SUIVI-{}", &piece[..8]), client, depot, now.clone(), dossier.clone()],
    )
    .unwrap();
    base.executer(
        "INSERT INTO ligne_piece
           (id, piece_id, article_id, unite_vente_id, quantite, prix_unitaire,
            remise_pct, remise_montant, taux_tva, montant_tva, montant_ht, cree_le, dossier_id)
         VALUES (?1,?2,?3,?4,?5,?6,0,0,0,0,?7,?8,?9)",
        &parametres![uuid::Uuid::new_v4().to_string(), piece.clone(), sucre.0.clone(), sucre.1.clone(), quantite, sucre.3,
            (sucre.3 as f64 * quantite) as i64, now, dossier],
    )
    .unwrap();
    let r = argent::valider_facture_sur_base(&mut base, piece, "comptant".into(), Some("especes".into()), None, None).unwrap();
    assert_eq!(decouvert_de(&mut base, r["vente_id"].as_str().unwrap()), 0);
}

#[test]
fn hors_de_l_ecran_stock_et_des_gestes_de_stock() {
    let mut base = base_avec_demo();
    let depot = depot_defaut(&mut base);
    let sucre = article_unite(&mut base, "Sucre");
    let dans_le_stock = |base: &mut Base| -> bool {
        pagination::lire_stocks_pagines_sur_base(base, 0, 200, None, false, None).unwrap().to_string().contains(&sucre.0)
    };
    assert!(dans_le_stock(&mut base), "suivi : dans l'écran Stock");
    arreter_le_suivi(&mut base, &sucre.0);
    assert!(!dans_le_stock(&mut base), "non suivi : plus dans l'écran Stock");

    let caisse = catalogue::lire_articles_avec_unites_sur(&mut base, true, None).unwrap();
    let a = caisse.iter().find(|a| a["id"] == sucre.0.as_str()).unwrap();
    assert_eq!(a["gere_en_stock"], false, "la caisse le sait");
    let tous = gescom_noyau::parametres::lire_articles_complets_sur_base(&mut base).unwrap();
    assert_eq!(tous.iter().find(|a| a["id"] == sucre.0.as_str()).unwrap()["gere_en_stock"], false, "l'écran Articles aussi");

    let e = transferts::enregistrer_transfert_sur_base(
        &mut base, depot.clone(), "un-autre-magasin".into(),
        vec![transferts::LigneTransfert {
            article_id: sucre.0.clone(), unite_vente_id: sucre.1.clone(), quantite: 1.0, facteur: 1.0,
        }],
        None, None,
    )
    .unwrap_err();
    assert!(e.contains("ne suit pas son stock"), "{e}");
    let e = fournisseurs::enregistrer_ajustement_inventaire_sur_base(&mut base, sucre.0.clone(), depot, 3.0, None, None).unwrap_err();
    assert!(e.contains("ne suit pas son stock"), "{e}");
}

#[test]
fn reprendre_le_suivi_exige_un_comptage_par_magasin() {
    let mut base = base_avec_demo();
    let depot = depot_defaut(&mut base);
    let sucre = article_unite(&mut base, "Sucre");
    arreter_le_suivi(&mut base, &sucre.0);
    // Des ventes pendant qu'on ne comptait plus.
    let client = client_reel(&mut base);
    argent::creer_vente_sur_base(
        &mut base, client, depot.clone(), "comptant".into(),
        vec![ligne_vente(&sucre, &depot, 30.0)], None, None, None, None,
    )
    .unwrap();

    let e = suivi_stock::changer_suivi_stock_sur(&mut base, sucre.0.clone(), true, vec![]).unwrap_err();
    assert!(e.contains("compter chaque magasin"), "{e}");
    let e = suivi_stock::changer_suivi_stock_sur(&mut base, sucre.0.clone(), true, vec![(depot.clone(), -2.0)]).unwrap_err();
    assert!(e.contains("invalide"), "{e}");
    assert!(!suivi_stock::est_suivi_sur(&mut base, &sucre.0), "refusé : rien n'a changé");

    let r = suivi_stock::changer_suivi_stock_sur(&mut base, sucre.0.clone(), true, vec![(depot.clone(), 42.0)]).unwrap();
    assert_eq!(r["suivi"], true);
    assert!(suivi_stock::est_suivi_sur(&mut base, &sucre.0));
    assert_eq!(stock(&mut base, &sucre.0, &depot), 42.0, "le stock repart de la quantité comptée");
    let dossier = base.dossier().to_string();
    assert_eq!(
        compter(&mut base, "SELECT COUNT(*) FROM journal WHERE type_evenement = 'suivi_stock_change' AND entite_id = ?1 AND dossier_id = ?2",
            &parametres![sucre.0.clone(), dossier]),
        2,
        "cesser et reprendre : deux traces"
    );
}

#[test]
fn tout_passe_le_detecteur() {
    let mut base = base_avec_demo();
    base.auditer(true);
    let depot = depot_defaut(&mut base);
    let sucre = article_unite(&mut base, "Sucre");
    arreter_le_suivi(&mut base, &sucre.0);
    catalogue::lire_articles_avec_unites_sur(&mut base, true, None).unwrap();
    pagination::lire_stocks_pagines_sur_base(&mut base, 0, 50, None, false, None).unwrap();
    suivi_stock::changer_suivi_stock_sur(&mut base, sucre.0, true, vec![(depot, 5.0)]).unwrap();
}
