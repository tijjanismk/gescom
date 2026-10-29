//! D45 — le seuil d'alerte des articles, sur les deux moteurs.
//!
//! Un article sans seuil prend celui du dossier (5 d'office, comme
//! avant) ; un seuil propre se donne a la creation (Parametres, Achats),
//! ensuite sur la fiche, ou par l'import. Le tableau de bord, le rapport
//! et la liste du stock comptent avec le meme seuil ; 0 coupe l'alerte.
//!
//!   cargo test -p gescom-noyau --test seuil_base
//!   GESCOM_PG=postgresql://…/gescom_test cargo test -p gescom-noyau --test seuil_base -- --test-threads=1

mod commun;

use commun::*;
use gescom_noyau::base::Base;
use gescom_noyau::{assistant, catalogue_csv, comptoir, parametres, tableau_bord};

/// Un article neuf, et `quantite` en stock dans le magasin par defaut.
fn article_avec_stock(base: &mut Base, nom: &str, seuil: Option<f64>, quantite: f64) -> String {
    let id = parametres::creer_article_complet_sur_base(base, nom.into(), None, "pièce".into(), 100, seuil).unwrap();
    let depot = depot_defaut(base);
    let dossier = base.dossier().to_string();
    base.executer(
        "INSERT INTO mouvement_stock
           (id, article_id, depot_id, type_mouvement, quantite_delta, motif, auteur_id,
            date_mouvement, cree_le, cree_par, origine, dossier_id)
         VALUES (?1, ?2, ?3, 'entree', ?4, 'essai', 'essai', ?5, ?5, 'essai', 'essai', ?6)",
        &gescom_noyau::parametres![
            uuid::Uuid::new_v4().to_string(), id.clone(), depot, quantite,
            gescom_noyau::utils::maintenant_iso(), dossier
        ],
    )
    .unwrap();
    id
}

fn seuil_de(base: &mut Base, id: &str) -> Option<f64> {
    base.lire_une("SELECT seuil_alerte FROM article WHERE id = ?1", &gescom_noyau::parametres![id], |r| r.get::<Option<f64>>(0))
        .unwrap()
        .unwrap()
}

fn alertes(base: &mut Base) -> i64 {
    tableau_bord::lire_resume_dashboard_sur(base, None).unwrap()["stock_alertes"].as_i64().unwrap()
}

#[test]
fn sans_seuil_l_article_prend_celui_du_dossier() {
    let mut base = base_avec_demo();
    let avant = alertes(&mut base);
    article_avec_stock(&mut base, "Savon", None, 3.0);
    assert_eq!(alertes(&mut base), avant + 1, "3 < 5 : alerte, comme avant D45");
    // Le dossier releve son seuil a 2 : le savon n'est plus en alerte.
    assistant::definir_reglages_dossier_sur(&mut base, 0, 0, Some(2.0)).unwrap();
    assert_eq!(alertes(&mut base), avant);
    assert_eq!(assistant::lire_reglages_dossier_sur(&mut base).unwrap()["seuil_defaut"].as_f64(), Some(2.0));
}

#[test]
fn le_seuil_propre_de_l_article_passe_devant_et_zero_coupe_l_alerte() {
    let mut base = base_avec_demo();
    let avant = alertes(&mut base);
    let ciment = article_avec_stock(&mut base, "Ciment", Some(20.0), 15.0);
    let clous = article_avec_stock(&mut base, "Clous", Some(0.0), 1.0);
    assert_eq!(seuil_de(&mut base, &ciment), Some(20.0));
    assert_eq!(alertes(&mut base), avant + 1, "le ciment (15 < 20), pas les clous (seuil 0)");

    // La fiche : le seuil change, ou revient a celui du dossier.
    let r = parametres::definir_seuil_article_sur(&mut base, ciment.clone(), Some(10.0)).unwrap();
    assert_eq!(r["seuil_effectif"].as_f64(), Some(10.0));
    assert_eq!(alertes(&mut base), avant);
    let r = parametres::definir_seuil_article_sur(&mut base, clous.clone(), None).unwrap();
    assert_eq!((r["seuil_alerte"].as_f64(), r["seuil_effectif"].as_f64()), (None, Some(5.0)));
    assert_eq!(alertes(&mut base), avant + 1, "les clous reprennent le seuil du dossier : 1 < 5");

    // La liste du stock porte le seuil et le statut de chaque ligne.
    let page = gescom_noyau::pagination::lire_stocks_pagines_sur_base(&mut base, 0, 200, Some("Clous".into()), false, None).unwrap();
    let ligne = &page["donnees"][0];
    assert_eq!((ligne["seuil"].as_f64(), ligne["statut"].as_str()), (Some(5.0), Some("alerte")));
    // Le rapport aussi.
    let rapport = gescom_noyau::rapports::lire_rapport_stock_sur_base(&mut base).unwrap();
    let ciment_r = rapport.iter().find(|l| l["nom"] == "Ciment").unwrap();
    assert_eq!(ciment_r["statut"], "ok", "15 face au seuil 10");
}

#[test]
fn a_la_reception_le_seuil_se_donne_et_un_seuil_faux_n_ecrit_rien() {
    let mut base = base_avec_demo();
    let a = comptoir::creer_article_rapide_sur(&mut base, "Peinture".into(), "pot".into(), 0, Some(4_000), Some(6.0)).unwrap();
    assert_eq!(seuil_de(&mut base, a["id"].as_str().unwrap()), Some(6.0));
    let avant = compter(&mut base, "SELECT COUNT(*) FROM article", &[]);
    assert!(comptoir::creer_article_rapide_sur(&mut base, "Vernis".into(), "pot".into(), 0, None, Some(-1.0)).is_err());
    assert!(parametres::creer_article_complet_sur_base(&mut base, "Vernis".into(), None, "pot".into(), 10, Some(f64::INFINITY)).is_err());
    assert!(assistant::definir_reglages_dossier_sur(&mut base, 0, 0, Some(-3.0)).is_err());
    assert_eq!(compter(&mut base, "SELECT COUNT(*) FROM article", &[]), avant, "aucun refus n'a créé d'article");
    // Un article qui ne suit pas son stock n'a pas de seuil.
    let sachet = parametres::creer_article_complet_sur_base(&mut base, "Sachets".into(), None, "paquet".into(), 25, None).unwrap();
    base.executer("UPDATE article SET gere_en_stock = 0 WHERE id = ?1", &gescom_noyau::parametres![sachet.clone()]).unwrap();
    assert!(parametres::definir_seuil_article_sur(&mut base, sachet, Some(3.0)).unwrap_err().contains("ne suit pas"));
}

#[test]
fn l_import_lit_la_colonne_seuil_et_l_export_la_rend() {
    let mut base = base_avec_demo();
    let csv = "Nom;Categorie;Unite;Prix;Seuil\nSavon;;pièce;250;12\nThé;;paquet;400;\nRiz local;;kg;650;30\nLait;;boîte;900;beaucoup"
        .to_string();
    let r = catalogue_csv::importer_articles_csv_sur_base(&mut base, csv, Some(true), true, false).unwrap();
    assert_eq!((r["crees"].as_i64(), r["mis_a_jour"].as_i64(), r["nb_erreurs"].as_i64()), (Some(2), Some(1), Some(1)), "{r}");
    assert!(r["erreurs"].to_string().contains("seuil"));
    let id = |base: &mut Base, nom: &str| -> String {
        base.lire_une("SELECT id FROM article WHERE nom = ?1", &gescom_noyau::parametres![nom], |r| r.get::<String>(0)).unwrap().unwrap()
    };
    let (savon, the, riz) = (id(&mut base, "Savon"), id(&mut base, "Thé"), id(&mut base, "Riz local"));
    assert_eq!(seuil_de(&mut base, &savon), Some(12.0));
    assert_eq!(seuil_de(&mut base, &the), None, "vide : celui du dossier");
    assert_eq!(seuil_de(&mut base, &riz), Some(30.0), "un existant reçoit le seuil");
    // Une colonne vide ne retire pas un seuil deja donne.
    catalogue_csv::importer_articles_csv_sur_base(&mut base, "Nom;Categorie;Unite;Prix;Seuil\nRiz local;;kg;650;".into(), Some(true), true, false).unwrap();
    assert_eq!(seuil_de(&mut base, &riz), Some(30.0));

    let export = catalogue_csv::exporter_articles_csv_sur_base(&mut base).unwrap();
    assert!(export.lines().next().unwrap().ends_with(";Seuil"));
    assert!(export.lines().any(|l| l.starts_with("Savon;") && l.ends_with(";12")), "{export}");
    assert!(export.lines().any(|l| l.starts_with("Thé;") && l.ends_with(";")), "vide à l'export");
}

#[test]
fn tout_passe_le_detecteur() {
    let mut base = base_avec_demo();
    base.auditer(true);
    let id = article_avec_stock(&mut base, "Savon", Some(8.0), 3.0);
    parametres::definir_seuil_article_sur(&mut base, id, None).unwrap();
    assistant::definir_reglages_dossier_sur(&mut base, 0, 0, Some(4.0)).unwrap();
    alertes(&mut base);
    gescom_noyau::pagination::lire_stocks_pagines_sur_base(&mut base, 0, 50, None, false, None).unwrap();
    gescom_noyau::rapports::lire_rapport_stock_sur_base(&mut base).unwrap();
}
