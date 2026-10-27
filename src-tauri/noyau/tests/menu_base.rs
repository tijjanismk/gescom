//! D37 — le menu d'un dossier, sur les deux moteurs.
//!
//! Par defaut rien n'est cache ; « Boutique simple » cache ce qu'il
//! annonce ; Caisse et Parametres ne se cachent pas (refus sans
//! ecriture) ; chaque dossier a son menu ; le geste est trace.
//!
//!   cargo test -p gescom-noyau --test menu_base
//!   GESCOM_PG=postgresql://…/gescom_test cargo test -p gescom-noyau --test menu_base -- --test-threads=1

mod commun;

use commun::*;
use gescom_noyau::coeur::menu as regles;
use gescom_noyau::menu;
use gescom_noyau::parametres;

fn caches(v: &serde_json::Value) -> Vec<String> {
    v["caches"].as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect()
}

#[test]
fn par_defaut_rien_n_est_cache_et_l_ecran_a_de_quoi_regler() {
    let mut base = base_avec_demo();
    let m = menu::lire_menu_sur(&mut base).unwrap();
    assert!(caches(&m).is_empty());
    assert_eq!(m["masquables"].as_array().unwrap().len(), regles::MASQUABLES.len());
    assert_eq!(m["toujours"].as_array().unwrap().len(), 4);
    assert!(m["boutique_simple"].as_array().unwrap().iter().any(|x| x == "cheques"));
}

#[test]
fn boutique_simple_puis_complet() {
    let mut base = base_avec_demo();
    let simple = regles::caches_boutique_simple();
    let m = menu::definir_menu_sur(&mut base, simple.clone()).unwrap();
    assert_eq!(caches(&m), simple);
    assert_eq!(caches(&menu::lire_menu_sur(&mut base).unwrap()), simple, "relu");
    // Le geste est trace.
    let dossier = base.dossier().to_string();
    assert_eq!(
        compter(&mut base, "SELECT COUNT(*) FROM journal WHERE type_evenement = 'menu_modifie' AND dossier_id = ?1", &parametres![dossier]),
        1
    );
    let m = menu::definir_menu_sur(&mut base, vec![]).unwrap();
    assert!(caches(&m).is_empty(), "« Complet » : tout revient");
}

#[test]
fn caisse_et_parametres_ne_se_cachent_pas_et_rien_ne_s_ecrit() {
    let mut base = base_avec_demo();
    menu::definir_menu_sur(&mut base, vec!["cheques".into()]).unwrap();
    for h in ["caisse", "parametres", "ventes", "dashboard"] {
        let e = menu::definir_menu_sur(&mut base, vec!["relances".into(), h.into()]).unwrap_err();
        assert!(e.contains("toujours"), "{h} : {e}");
    }
    assert!(menu::definir_menu_sur(&mut base, vec!["inconnu".into()]).is_err());
    assert_eq!(caches(&menu::lire_menu_sur(&mut base).unwrap()), vec!["cheques".to_string()], "le réglage d'avant tient");
}

#[test]
fn chaque_dossier_a_son_menu() {
    let mut base = base_avec_demo();
    let b = gescom_noyau::dossiers::creer_dossier_sur(&mut base, "MENU".into(), "Boutique du menu".into(), None, None)
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    menu::definir_menu_sur(&mut base, vec!["historique".into()]).unwrap();
    base.choisir_dossier(&b).unwrap();
    assert!(caches(&menu::lire_menu_sur(&mut base).unwrap()).is_empty(), "l'autre dossier n'est pas touché");
    menu::definir_menu_sur(&mut base, vec!["transferts".into()]).unwrap();
    base.choisir_dossier(gescom_noyau::dossiers::DOSSIER_DEFAUT).unwrap();
    assert_eq!(caches(&menu::lire_menu_sur(&mut base).unwrap()), vec!["historique".to_string()]);
}

#[test]
fn tout_passe_le_detecteur() {
    let mut base = base_avec_demo();
    base.auditer(true);
    menu::lire_menu_sur(&mut base).unwrap();
    menu::definir_menu_sur(&mut base, regles::caches_boutique_simple()).unwrap();
}
