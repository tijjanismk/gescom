//! `pagination`, `livraisons` (saisie ligne à ligne), `modeles`,
//! `catalogue_csv` — sur `Base`. Testé sur SQLite par défaut, sur
//! PostgreSQL avec `GESCOM_PG`.

mod commun;

use commun::*;
use gescom_noyau::argent::{self, ParamsLigneInput};
use gescom_noyau::base::Base;
use gescom_noyau::{catalogue_csv, livraisons, modeles, pagination, pieces};

fn vendre(base: &mut Base, quantite: f64, paye: bool) -> i64 {
    let depot = depot_defaut(base);
    let sucre = article_unite(base, "Sucre");
    let client = if paye { client_generique(base) } else { client_reel(base) };
    let montant = (sucre.3 as f64 * quantite).round() as i64;
    argent::creer_vente_sur_base(
        base, client, depot.clone(), if paye { "comptant" } else { "credit" }.into(),
        vec![ParamsLigneInput {
            article_id: sucre.0.clone(), unite_vente_id: sucre.1.clone(), depot_source_id: depot,
            source_approvisionnement: "stock".into(), quantite, facteur: sucre.2,
            prix_reference: sucre.3, prix_pratique: sucre.3, taux_tva: None, a_decouvert: None,
        }],
        None, if paye { Some(montant) } else { None }, None, None,
    )
    .expect("vente");
    montant
}

#[test]
fn les_ventes_se_paginent_et_se_filtrent() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    for _ in 0..3 {
        vendre(&mut base, 1.0, true);
    }
    let credit = vendre(&mut base, 2.0, false);

    let p = pagination::lire_ventes_paginees_sur_base(&mut base, 0, 3, None, None, None, None, None).unwrap();
    assert_eq!(p["total"], 4);
    assert_eq!(p["pages"], 2);
    assert_eq!(p["donnees"].as_array().unwrap().len(), 3);
    let p2 = pagination::lire_ventes_paginees_sur_base(&mut base, 1, 3, None, None, None, None, None).unwrap();
    assert_eq!(p2["donnees"].as_array().unwrap().len(), 1);

    let c = pagination::lire_ventes_paginees_sur_base(&mut base, 0, 10, None, Some("creance_ouverte".into()), None, None, None).unwrap();
    assert_eq!(c["total"], 1);
    assert_eq!(c["donnees"][0]["total"], credit);
    let hier = pagination::lire_ventes_paginees_sur_base(&mut base, 0, 10, None, None, Some("personnalise".into()), None, Some("2000-01-01".into())).unwrap();
    assert_eq!(hier["total"], 0);
    let auj = pagination::lire_ventes_paginees_sur_base(&mut base, 0, 10, None, None, Some("aujourd_hui".into()), None, None).unwrap();
    assert_eq!(auj["total"], 4);
    // Recherche insensible à la casse sur le nom du client.
    let nom = client_reel(&mut base);
    let nom: String = base.lire_une("SELECT nom FROM client WHERE id = ?1", &gescom_noyau::parametres![nom], |r| r.get::<String>(0)).unwrap().unwrap();
    let r = pagination::lire_ventes_paginees_sur_base(&mut base, 0, 10, Some(nom.to_uppercase()), None, None, None, None).unwrap();
    assert_eq!(r["total"], 1);

    let rec = pagination::lire_ventes_recentes_paginee_sur_base(&mut base, 0, 2, None, None, None, None).unwrap();
    assert_eq!(rec["total"], 4);
    assert_eq!(rec["donnees"].as_array().unwrap().len(), 2);
    assert_eq!(rec["donnees"][0]["lignes"][0]["article_nom"], "Sucre");
}

#[test]
fn les_clients_les_stocks_et_les_fournisseurs_se_paginent() {
    let mut base = base_avec_demo();
    let credit = vendre(&mut base, 2.0, false);
    fournisseur(&mut base, "Grossiste Sikasso");

    let tous = pagination::lire_clients_pagines_sur_base(&mut base, 0, 50, None, false, None, None).unwrap();
    assert!(tous["total"].as_i64().unwrap() >= 1, "les vrais clients, pas le passage");
    assert!(tous["donnees"].as_array().unwrap().iter().all(|c| c["code"] != "client0000"));
    let debiteurs = pagination::lire_clients_pagines_sur_base(&mut base, 0, 50, None, true, None, Some("creance".into())).unwrap();
    assert_eq!(debiteurs["total"], 1);
    assert_eq!(debiteurs["donnees"][0]["total_creances"], credit);
    assert_eq!(debiteurs["donnees"][0]["nb_ventes"], 1);
    let sans = pagination::lire_clients_pagines_sur_base(&mut base, 0, 50, None, false, Some("sans".into()), None).unwrap();
    assert!(sans["donnees"].as_array().unwrap().iter().all(|c| c["nb_ventes"] == 0));
    assert_eq!(pagination::lire_clients_pagines_sur_base(&mut base, 0, 50, Some("zzzz".into()), false, None, None).unwrap()["total"], 0);

    let s = pagination::lire_stocks_pagines_sur_base(&mut base, 0, 2, Some("SUC".into()), false, None).unwrap();
    assert_eq!(s["total"], 1);
    let ligne = &s["donnees"][0];
    assert_eq!(ligne["article_nom"], "Sucre");
    assert_eq!(ligne["quantite"], 198.0);
    assert_eq!(ligne["unites"].as_array().unwrap().len(), 2, "les unités arrivent en tableau");
    assert_eq!(ligne["unites"][0]["facteur"], 1.0);
    assert_eq!(pagination::lire_stocks_pagines_sur_base(&mut base, 0, 50, None, true, None).unwrap()["total"], 0, "rien à régulariser");

    let f = pagination::lire_fournisseurs_pagines_sur_base(&mut base, 0, 10, Some("sikasso".into())).unwrap();
    assert_eq!(f["total"], 1);
    assert_eq!(f["donnees"][0]["dette"], 0);
    assert_eq!(pagination::lire_fournisseurs_pagines_sur_base(&mut base, 0, 10, Some("nulle part".into())).unwrap()["total"], 0);
}

#[test]
fn une_livraison_se_saisit_ligne_a_ligne_et_bouge_le_stock_de_l_ecart() {
    let mut base = base_avec_demo();
    let sucre = article_unite(&mut base, "Sucre");
    let depot = depot_defaut(&mut base);
    let client = client_reel(&mut base);
    let avant = stock(&mut base, &sucre.0, &depot);
    // Une commande convertie en BL par le raccourci sort tout ; ici on
    // veut un bon saisi ligne à ligne : on le crée directement.
    let bl = pieces::creer_piece_sur_base(&mut base, client, "bon_livraison".into(), vec![ligne(&sucre, 10.0)], None, None, None, None, None).unwrap();
    let bl_id = bl["id"].as_str().unwrap().to_string();

    let l = livraisons::lire_livraison_piece_sur_base(&mut base, bl_id.clone()).unwrap();
    assert_eq!(l["etat"], "non_livre");
    let ligne_id = l["lignes"][0]["id"].as_str().unwrap().to_string();
    assert_eq!(l["lignes"][0]["reste"], 10.0);

    let r = livraisons::enregistrer_livraison_sur_base(&mut base, bl_id.clone(), vec![livraisons::LigneLivraison { ligne_id: ligne_id.clone(), quantite_livree: 6.0 }]).unwrap();
    assert_eq!(r["etat"], "partiel");
    assert_eq!(stock(&mut base, &sucre.0, &depot), avant - 6.0, "six sortent");
    let r = livraisons::enregistrer_livraison_sur_base(&mut base, bl_id.clone(), vec![livraisons::LigneLivraison { ligne_id: ligne_id.clone(), quantite_livree: 7.0 }]).unwrap();
    assert_eq!(r["etat"], "partiel");
    assert_eq!(stock(&mut base, &sucre.0, &depot), avant - 7.0, "corriger 6 en 7 sort UNE unité");
    let r = livraisons::enregistrer_livraison_sur_base(&mut base, bl_id.clone(), vec![livraisons::LigneLivraison { ligne_id: ligne_id.clone(), quantite_livree: 50.0 }]).unwrap();
    assert_eq!(r["etat"], "livre");
    assert_eq!(r["livree"], 10.0, "plafonné au commandé");
    assert_eq!(stock(&mut base, &sucre.0, &depot), avant - 10.0);
    assert!(livraisons::enregistrer_livraison_sur_base(&mut base, bl_id, vec![livraisons::LigneLivraison { ligne_id: "x".into(), quantite_livree: 1.0 }]).is_err());
}

#[test]
fn les_modeles_se_gerent_avec_un_seul_actif_par_genre() {
    let mut base = base_avec_demo();
    let m = |id: &str, nom: &str, defaut: bool| modeles::Modele {
        id: id.into(), genre: "facture".into(), nom: nom.into(), format: "A4".into(),
        contenu: serde_json::json!({"blocs": []}), est_defaut: defaut, actif: false, modifie_le: String::new(),
    };
    modeles::enregistrer_sur_base(&mut base, &m("m1", "Usine", true), "test").unwrap();
    assert_eq!(modeles::lire_actif_sur_base(&mut base, "facture").unwrap().id, "m1", "le premier devient actif");
    modeles::enregistrer_sur_base(&mut base, &m("m2", "Perso", false), "test").unwrap();
    assert_eq!(modeles::lire_actif_sur_base(&mut base, "facture").unwrap().id, "m1", "le second ne prend pas la place");
    assert!(modeles::enregistrer_sur_base(&mut base, &m("m3", " ", false), "test").is_err());

    modeles::definir_actif_sur_base(&mut base, "m2").unwrap();
    let liste = modeles::lister_sur_base(&mut base, Some("facture")).unwrap();
    assert_eq!(liste.len(), 2);
    assert_eq!(liste.iter().filter(|x| x.actif).count(), 1);
    assert_eq!(modeles::lire_sur_base(&mut base, "m2").unwrap().nom, "Perso");
    assert!(modeles::lire_sur_base(&mut base, "zz").is_err());
    assert!(modeles::lister_sur_base(&mut base, Some("recu")).unwrap().is_empty());

    assert!(modeles::supprimer_sur_base(&mut base, "m1").is_err(), "un modèle d'usine ne se supprime pas");
    modeles::supprimer_sur_base(&mut base, "m2").unwrap();
    assert_eq!(modeles::lire_actif_sur_base(&mut base, "facture").unwrap().id, "m1", "l'usine reprend la main");

    let lot = modeles::exporter_sur_base(&mut base, None).unwrap();
    assert_eq!(lot.modeles.len(), 1);
    let mut lot2 = lot.clone();
    lot2.modeles.push(m("m9", "Importé", false));
    let bilan = modeles::importer_sur_base(&mut base, &lot2, "test").unwrap();
    assert_eq!(bilan.ajoutes, 1);
    assert_eq!(bilan.remplaces, 1);
    assert_eq!(modeles::lire_actif_sur_base(&mut base, "facture").unwrap().id, "m1", "l'import ne change pas l'actif");
}

#[test]
fn le_catalogue_s_exporte_et_se_reimporte() {
    let mut base = base_avec_demo();
    let sucre = article_unite(&mut base, "Sucre");
    let depot = depot_defaut(&mut base);
    let csv = catalogue_csv::exporter_articles_csv_sur_base(&mut base).unwrap();
    assert!(csv.starts_with('\u{FEFF}'));
    assert!(csv.lines().any(|l| l.starts_with("Sucre;")), "{csv}");
    let n_lignes = csv.lines().count();

    let r = catalogue_csv::importer_articles_csv_sur_base(
        &mut base,
        "Nom;Categorie;Unite;Prix;PrixAchat;TVA;CodeBarre;Stock\nSucre;Epicerie;kg;900;;;\nClous;Quincaillerie;kg;1500;1000;18;3017620422003;40\n;x;y;1\nSans prix;x;y;abc".into(),
        Some(true),
    )
    .unwrap();
    assert_eq!(r["crees"], 1);
    assert_eq!(r["mis_a_jour"], 1);
    assert_eq!(r["nb_erreurs"], 2, "{r}");
    let prix: i64 = base.lire_une("SELECT prix_reference FROM unite_vente WHERE id = ?1", &gescom_noyau::parametres![sucre.1.clone()], |r| r.get::<i64>(0)).unwrap().unwrap();
    assert_eq!(prix, 900, "le prix du sucre est mis à jour");
    let clous: String = base.lire_une("SELECT id FROM article WHERE nom = 'Clous'", &[], |r| r.get::<String>(0)).unwrap().unwrap();
    assert_eq!(stock(&mut base, &clous, &depot), 40.0, "le stock importé est une entrée");
    assert_eq!(compter(&mut base, "SELECT COUNT(*) FROM mouvement_stock WHERE article_id = ?1 AND type_mouvement = 'entree'", &gescom_noyau::parametres![clous.clone()]), 1);
    assert_eq!(compter(&mut base, "SELECT COUNT(*) FROM categorie WHERE nom = 'Quincaillerie'", &[]), 1);
    assert_eq!(catalogue_csv::exporter_articles_csv_sur_base(&mut base).unwrap().lines().count(), n_lignes + 1);

    let etat = catalogue_csv::lire_etat_stock_sur_base(&mut base, None, None).unwrap();
    assert!(etat["lignes"].as_array().unwrap().iter().any(|l| l["article"] == "Clous" && l["valeur"] == 40_000));
    assert!(etat["valeur_totale"].as_i64().unwrap() >= 40_000);
    let ailleurs = catalogue_csv::lire_etat_stock_sur_base(&mut base, Some("x".into()), Some(true)).unwrap();
    assert_eq!(ailleurs["nb_articles"], 0);
}

#[test]
fn tout_ce_qui_est_porte_ici_passe_le_detecteur() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    vendre(&mut base, 1.0, true);
    let sucre = article_unite(&mut base, "Sucre");
    let client = client_reel(&mut base);
    let bl = pieces::creer_piece_sur_base(&mut base, client, "bon_livraison".into(), vec![ligne(&sucre, 2.0)], None, None, None, None, None).unwrap();
    let bl_id = bl["id"].as_str().unwrap().to_string();
    base.auditer(true);

    pagination::lire_ventes_paginees_sur_base(&mut base, 0, 10, Some("a".into()), Some("payee".into()), Some("mois".into()), None, None).expect("ventes");
    pagination::lire_clients_pagines_sur_base(&mut base, 0, 10, Some("a".into()), true, Some("avec".into()), Some("nom".into())).expect("clients");
    pagination::lire_stocks_pagines_sur_base(&mut base, 0, 10, Some("s".into()), false, None).expect("stocks");
    pagination::lire_fournisseurs_pagines_sur_base(&mut base, 0, 10, Some("g".into())).expect("fournisseurs");
    pagination::lire_ventes_recentes_paginee_sur_base(&mut base, 0, 10, None, Some("semaine".into()), None, None).expect("récentes");
    let l = livraisons::lire_livraison_piece_sur_base(&mut base, bl_id.clone()).expect("livraison");
    let lid = l["lignes"][0]["id"].as_str().unwrap().to_string();
    livraisons::enregistrer_livraison_sur_base(&mut base, bl_id, vec![livraisons::LigneLivraison { ligne_id: lid, quantite_livree: 1.0 }]).expect("livrer");
    let m = modeles::Modele { id: "t".into(), genre: "g".into(), nom: "n".into(), format: "A4".into(), contenu: serde_json::json!({}), est_defaut: false, actif: false, modifie_le: String::new() };
    modeles::enregistrer_sur_base(&mut base, &m, "t").expect("modèle");
    modeles::lister_sur_base(&mut base, None).expect("lister");
    modeles::lire_sur_base(&mut base, "t").expect("lire");
    modeles::lire_actif_sur_base(&mut base, "g");
    modeles::definir_actif_sur_base(&mut base, "t").expect("actif");
    modeles::supprimer_sur_base(&mut base, "t").expect("supprimer");
    catalogue_csv::exporter_articles_csv_sur_base(&mut base).expect("export");
    catalogue_csv::importer_articles_csv_sur_base(&mut base, "Nom;Categorie;Unite;Prix\nX;C;u;10".into(), None).expect("import");
    catalogue_csv::lire_etat_stock_sur_base(&mut base, None, Some(true)).expect("état");
}
