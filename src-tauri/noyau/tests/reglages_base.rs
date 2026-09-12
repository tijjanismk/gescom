//! `societe`, `codebarre`, `roles`, `parametres` — les réglages, sur
//! `Base`. Testé sur SQLite par défaut, sur PostgreSQL avec `GESCOM_PG`.

mod commun;

use commun::*;
use gescom_noyau::parametres;
use gescom_noyau::{codebarre, parametres as reglages, roles, societe};

#[test]
fn les_parametres_societe_se_lisent_et_se_sauvent() {
    let mut base = base_avec_demo();
    let avant = societe::lire_parametres_societe_sur_base(&mut base).unwrap();
    assert!(avant["nom"].is_string());

    societe::sauvegarder_parametres_societe_sur_base(
        &mut base, "Ma Boutique".into(), Some("Bamako".into()), None, None, None, None, None, None, None,
    )
    .unwrap();
    let apres = societe::lire_parametres_societe_sur_base(&mut base).unwrap();
    assert_eq!(apres["nom"], "Ma Boutique");
    assert_eq!(apres["adresse"], "Bamako");
    assert!(apres["telephone"].is_null(), "un champ vidé redevient NULL");
    assert_eq!(apres["devise"], avant["devise"], "la devise n'est pas touchée");
}

#[test]
fn un_code_barre_interne_se_genere_et_un_code_fabricant_se_conserve() {
    let mut base = base_avec_demo();
    let sucre = article_unite(&mut base, "Sucre");
    let code = codebarre::generer_code_barre_sur_base(&mut base, sucre.0.clone()).unwrap();
    assert_eq!(code.len(), 13);
    assert!(code.starts_with("20"), "préfixe interne (D34) : {code}");
    let encore = codebarre::generer_code_barre_sur_base(&mut base, sucre.0.clone()).unwrap();
    assert_eq!(code, encore, "un article déjà codé garde son code");

    let riz = article_unite(&mut base, "Riz local");
    let refus = codebarre::definir_code_barre_sur_base(&mut base, riz.0.clone(), "1234".into()).unwrap_err();
    assert!(refus.contains("EAN-13"));
    let refus = codebarre::definir_code_barre_sur_base(&mut base, riz.0.clone(), code.clone()).unwrap_err();
    assert!(refus.contains("Sucre"), "le refus nomme l'article en conflit : {refus}");
    codebarre::definir_code_barre_sur_base(&mut base, riz.0.clone(), "3017620422003".into()).unwrap();

    let tous = codebarre::lire_articles_codes_barres_sur_base(&mut base, None).unwrap();
    let riz_j = tous.iter().find(|a| a["nom"] == "Riz local").unwrap();
    assert_eq!(riz_j["interne"], false);
    assert_eq!(riz_j["valide"], true);
    let sans = codebarre::lire_articles_codes_barres_sur_base(&mut base, Some(true)).unwrap();
    assert!(sans.iter().all(|a| a["code_barre"] == ""));

    let r = codebarre::generer_codes_barres_manquants_sur_base(&mut base).unwrap();
    assert_eq!(r["generes"].as_i64().unwrap(), sans.len() as i64);
    assert!(codebarre::lire_articles_codes_barres_sur_base(&mut base, Some(true)).unwrap().is_empty());
    // Les codes générés en lot sont tous distincts.
    let codes: std::collections::HashSet<String> = codebarre::lire_articles_codes_barres_sur_base(&mut base, None)
        .unwrap()
        .into_iter()
        .map(|a| a["code_barre"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(codes.len(), tous.len());
}

#[test]
fn les_roles_se_gerent_avec_leurs_garde_fous() {
    let mut base = base_avec_demo();
    let avant = roles::lire_roles_sur_base(&mut base).unwrap();
    let patron = avant.as_array().unwrap().iter().find(|r| r["nom"] == "patron").unwrap();
    assert_eq!(patron["acces_total"], true);
    assert!(patron["permissions"].as_array().unwrap().len() >= 20, "accès total = tout le catalogue");

    let refus = roles::creer_role_sur_base(&mut base, "livreur".into(), None, vec!["voler:tout".into()]).unwrap_err();
    assert!(refus.contains("inconnue"));
    let r = roles::creer_role_sur_base(&mut base, " Livreur ".into(), Some("les tournées".into()), vec!["ventes:creer".into()]).unwrap();
    assert_eq!(r["nom"], "livreur", "normalisé en minuscules");
    let refus = roles::creer_role_sur_base(&mut base, "livreur".into(), None, vec![]).unwrap_err();
    assert!(refus.contains("existe déjà"));
    let id = r["id"].as_str().unwrap().to_string();

    roles::modifier_role_sur_base(&mut base, id.clone(), None, vec!["ventes:creer".into(), "stock:transferer".into()]).unwrap();
    let apres = roles::lire_roles_sur_base(&mut base).unwrap();
    let livreur = apres.as_array().unwrap().iter().find(|r| r["nom"] == "livreur").unwrap();
    assert_eq!(livreur["permissions"].as_array().unwrap().len(), 2);
    assert_eq!(livreur["description"], "", "description vidée");

    let superadmin = apres.as_array().unwrap().iter().find(|r| r["nom"] == "superadmin").unwrap();
    let refus = roles::supprimer_role_sur_base(&mut base, superadmin["id"].as_str().unwrap().to_string()).unwrap_err();
    assert!(refus.contains("ne se supprime pas"));
    let refus = roles::supprimer_role_sur_base(&mut base, patron["id"].as_str().unwrap().to_string()).unwrap_err();
    assert!(refus.contains("utilisateur"), "un rôle porté ne se supprime pas : {refus}");
    let r = roles::supprimer_role_sur_base(&mut base, id).unwrap();
    assert_eq!(r["supprime"], "livreur");
}

#[test]
fn une_permission_personnelle_s_ajoute_et_se_retire() {
    let mut base = base_avec_demo();
    let employe: String = base
        .lire_une(
            "SELECT u.id FROM utilisateur u JOIN role r ON r.id = u.role_id WHERE r.nom = 'employe' LIMIT 1",
            &[],
            |r| r.get::<String>(0),
        )
        .unwrap()
        .unwrap();
    let avant = roles::lire_permissions_utilisateur_sur_base(&mut base, employe.clone()).unwrap();
    assert_eq!(avant["role"], "employe");
    let n = avant["effectives"].as_array().unwrap().len();

    let apres = roles::definir_permission_utilisateur_sur_base(
        &mut base, employe.clone(), "utilisateurs:gerer".into(), Some(true), Some("test".into()),
    )
    .unwrap();
    assert_eq!(apres["effectives"].as_array().unwrap().len(), n + 1);
    assert_eq!(apres["reglages"][0]["accorde"], true);

    let retour = roles::definir_permission_utilisateur_sur_base(&mut base, employe.clone(), "utilisateurs:gerer".into(), None, None).unwrap();
    assert_eq!(retour["effectives"].as_array().unwrap().len(), n);
    assert!(retour["reglages"].as_array().unwrap().is_empty());

    let refus = roles::definir_permission_utilisateur_sur_base(&mut base, employe, "x:y".into(), Some(true), None).unwrap_err();
    assert!(refus.contains("inconnue"));
}

#[test]
fn un_article_complet_nait_avec_son_unite_et_son_stock_a_zero() {
    let mut base = base_avec_demo();
    let depot = depot_defaut(&mut base);
    let cat = reglages::creer_categorie_sur_base(&mut base, "Quincaillerie".into()).unwrap();
    assert!(reglages::lire_categories_sur_base(&mut base).unwrap().iter().any(|c| c["id"] == cat));

    let id = reglages::creer_article_complet_sur_base(&mut base, "Clous".into(), Some(cat), "kg".into(), 1_500).unwrap();
    let complets = reglages::lire_articles_complets_sur_base(&mut base).unwrap();
    let clous = complets.iter().find(|a| a["id"] == id).expect("l'article est listé");
    assert_eq!(clous["unites"].as_array().unwrap().len(), 1);
    assert_eq!(clous["unites"][0]["facteur"], 1.0);
    assert_eq!(stock(&mut base, &id, &depot), 0.0);
    let n = compter(&mut base, "SELECT COUNT(*) FROM stock_depot WHERE article_id = ?1", &parametres![id.clone()]);
    assert_eq!(n, 1, "la ligne de stock existe");

    // Un pack : refus du doublon de libellé, alerte sur un prix bizarre.
    let r = reglages::ajouter_unite_vente_sur_base(&mut base, id.clone(), "Carton 10".into(), 10.0, 15_000, None).unwrap();
    assert!(r["alerte"].is_null());
    let r = reglages::ajouter_unite_vente_sur_base(&mut base, id.clone(), "Sac 50".into(), 50.0, 10_000, Some("3017620422003".into())).unwrap();
    assert!(r["alerte"].as_str().unwrap().contains("inhabituel"));
    let refus = reglages::ajouter_unite_vente_sur_base(&mut base, id.clone(), "carton 10".into(), 10.0, 15_000, None).unwrap_err();
    assert!(refus.contains("existe déjà"));
    let refus = reglages::ajouter_unite_vente_sur_base(&mut base, id.clone(), "Autre".into(), 2.0, 3_000, Some("3017620422003".into())).unwrap_err();
    assert!(refus.contains("déjà attribué"));

    let unite_id = r["id"].as_str().unwrap().to_string();
    reglages::modifier_unite_vente_sur_base(&mut base, unite_id.clone(), Some("Sac de 50".into()), None, Some(60_000), None).unwrap();
    let complets = reglages::lire_articles_complets_sur_base(&mut base).unwrap();
    let clous = complets.iter().find(|a| a["id"] == id).unwrap();
    let sac = clous["unites"].as_array().unwrap().iter().find(|u| u["id"] == unite_id).unwrap();
    assert_eq!(sac["libelle"], "Sac de 50");
    assert_eq!(sac["prix_reference"], 60_000);
    assert_eq!(sac["facteur"], 50.0, "non transmis : conservé");

    let base_unite = clous["unites"][0]["id"].as_str().unwrap().to_string();
    let refus = reglages::modifier_unite_vente_sur_base(&mut base, base_unite.clone(), None, Some(2.0), None, None).unwrap_err();
    assert!(refus.contains("unité de base"));
    let refus = reglages::desactiver_unite_vente_sur_base(&mut base, base_unite).unwrap_err();
    assert!(refus.contains("unité de base"));
    reglages::desactiver_unite_vente_sur_base(&mut base, unite_id).unwrap();
    let complets = reglages::lire_articles_complets_sur_base(&mut base).unwrap();
    let clous = complets.iter().find(|a| a["id"] == id).unwrap();
    assert_eq!(clous["unites"].as_array().unwrap().len(), 2);
}

#[test]
fn les_reglages_d_ecran_se_posent_et_se_relisent() {
    let mut base = base_avec_demo();
    assert!(!reglages::lire_config_bon_sortie_sur_base(&mut base).unwrap());
    reglages::sauvegarder_config_bon_sortie_sur_base(&mut base, true).unwrap();
    assert!(reglages::lire_config_bon_sortie_sur_base(&mut base).unwrap());
    reglages::sauvegarder_config_bon_sortie_sur_base(&mut base, false).unwrap();
    assert!(!reglages::lire_config_bon_sortie_sur_base(&mut base).unwrap());

    reglages::sauvegarder_config_suivi_livraison_sur_base(&mut base, true).unwrap();
    assert!(reglages::lire_config_suivi_livraison_sur_base(&mut base).unwrap());

    let sig = reglages::lire_config_signatures_sur_base(&mut base).unwrap();
    assert_eq!(sig["signature_facture_gauche"], "Pour acquit");
    let mut v = std::collections::HashMap::new();
    v.insert("signature_facture_gauche".to_string(), "  Le gérant ".to_string());
    v.insert("signature_defaut_droite".to_string(), "".to_string());
    v.insert("pirate".to_string(), "x".to_string());
    reglages::sauvegarder_config_signatures_sur_base(&mut base, v).unwrap();
    let sig = reglages::lire_config_signatures_sur_base(&mut base).unwrap();
    assert_eq!(sig["signature_facture_gauche"], "Le gérant");
    assert_eq!(sig["signature_defaut_droite"], "", "une chaîne vide volontaire reste vide");
    assert_eq!(compter(&mut base, "SELECT COUNT(*) FROM config_app WHERE cle = 'pirate'", &[]), 0);
}

#[test]
fn les_stocks_et_le_diagnostic_se_lisent() {
    let mut base = base_avec_demo();
    let stocks = reglages::lire_stocks_sur_base(&mut base).unwrap();
    assert!(!stocks.is_empty());
    assert!(stocks.iter().any(|s| s["article_nom"] == "Sucre" && s["quantite"] == 200.0));

    let d = reglages::diagnostiquer_base_sur_base(&mut base).unwrap();
    assert_eq!(d["sain"], true, "{d}");

    // Un stock écrit à la main sans mouvement se voit.
    base.executer("UPDATE stock_depot SET quantite = quantite + 1", &[]).unwrap();
    let d = reglages::diagnostiquer_base_sur_base(&mut base).unwrap();
    assert_eq!(d["sain"], false);
    assert!(d["anomalies"].as_array().unwrap().iter().any(|a| a["libelle"].as_str().unwrap().contains("mouvements")));

    base.choisir_dossier("dossier-b").unwrap();
    assert!(reglages::lire_stocks_sur_base(&mut base).unwrap().is_empty(), "le stock de l'autre société ne se voit pas");
}

#[test]
fn tout_ce_qui_est_porte_dans_les_reglages_passe_le_detecteur() {
    let mut base = base_avec_demo();
    base.auditer(true);
    let sucre = article_unite(&mut base, "Sucre");

    societe::lire_parametres_societe_sur_base(&mut base).expect("société");
    societe::sauvegarder_parametres_societe_sur_base(&mut base, "X".into(), None, None, None, None, None, None, None, None).expect("société");
    codebarre::generer_code_barre_sur_base(&mut base, sucre.0.clone()).expect("code");
    codebarre::generer_codes_barres_manquants_sur_base(&mut base).expect("codes");
    codebarre::definir_code_barre_sur_base(&mut base, sucre.0.clone(), "".into()).expect("définir");
    codebarre::lire_articles_codes_barres_sur_base(&mut base, Some(true)).expect("liste codes");
    roles::lire_roles_sur_base(&mut base).expect("rôles");
    let r = roles::creer_role_sur_base(&mut base, "t".into(), None, vec![]).expect("créer rôle");
    let id = r["id"].as_str().unwrap().to_string();
    roles::modifier_role_sur_base(&mut base, id.clone(), None, vec![]).expect("modifier rôle");
    roles::supprimer_role_sur_base(&mut base, id).expect("supprimer rôle");
    let u = gescom_noyau::argent::id_utilisateur_courant_sur(&mut base);
    roles::lire_permissions_utilisateur_sur_base(&mut base, u.clone()).expect("permissions");
    roles::definir_permission_utilisateur_sur_base(&mut base, u, "ventes:creer".into(), Some(false), None).expect("définir");
    reglages::lire_categories_sur_base(&mut base).expect("catégories");
    reglages::creer_categorie_sur_base(&mut base, "c".into()).expect("catégorie");
    reglages::lire_articles_complets_sur_base(&mut base).expect("articles");
    let a = reglages::creer_article_complet_sur_base(&mut base, "A".into(), None, "u".into(), 100).expect("article");
    let un = reglages::ajouter_unite_vente_sur_base(&mut base, a, "p".into(), 2.0, 200, None).expect("unité");
    let uid = un["id"].as_str().unwrap().to_string();
    reglages::modifier_unite_vente_sur_base(&mut base, uid.clone(), None, None, None, None).expect("modifier unité");
    reglages::desactiver_unite_vente_sur_base(&mut base, uid).expect("désactiver unité");
    reglages::lire_config_bon_sortie_sur_base(&mut base).expect("bon de sortie");
    reglages::sauvegarder_config_bon_sortie_sur_base(&mut base, true).expect("bon de sortie");
    reglages::lire_config_suivi_livraison_sur_base(&mut base).expect("suivi");
    reglages::sauvegarder_config_suivi_livraison_sur_base(&mut base, true).expect("suivi");
    reglages::lire_config_signatures_sur_base(&mut base).expect("signatures");
    reglages::sauvegarder_config_signatures_sur_base(&mut base, Default::default()).expect("signatures");
    reglages::lire_stocks_sur_base(&mut base).expect("stocks");
    reglages::diagnostiquer_base_sur_base(&mut base).expect("diagnostic");
}
