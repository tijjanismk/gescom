//! F-3 — les jours travailles (PLAN-EQUIPE), sur les deux moteurs et
//! sous le compte limite. Un journalier, 22 jours : la paie lira ce
//! compte.

mod commun;

use commun::*;
use chrono::{Datelike, NaiveDate};
use gescom_noyau::base::Base;
use gescom_noyau::personnel::{self, Fiche};
use gescom_noyau::{dossiers, presences};
use serde_json::json;

fn creer(base: &mut Base, v: serde_json::Value) -> String {
    let f: Fiche = serde_json::from_value(v).unwrap();
    personnel::creer_sur(base, f).unwrap()["id"].as_str().unwrap().to_string()
}

/// Le mois dernier : tous ses jours sont passes.
fn mois_dernier() -> (String, Vec<String>) {
    let aujourd_hui = NaiveDate::parse_from_str(&gescom_noyau::utils::maintenant_iso()[..10], "%Y-%m-%d").unwrap();
    let premier = aujourd_hui.with_day(1).unwrap().pred_opt().unwrap().with_day(1).unwrap();
    let mois = premier.format("%Y-%m").to_string();
    let jours = gescom_noyau::coeur::presences::jours_du_mois(&mois).unwrap();
    (mois, jours)
}

#[test]
fn un_journalier_vingt_deux_jours_et_des_demi_journees() {
    let mut base = base_avec_demo();
    let moussa = creer(&mut base, json!({ "nom": "Moussa", "fonction": "manœuvre", "tarif_journalier": 2500 }));
    let awa = creer(&mut base, json!({ "nom": "Awa", "fonction": "vendeuse", "salaire_mensuel": 60000 }));
    let (mois, jours) = mois_dernier();
    for j in &jours[..22] {
        presences::marquer_sur(&mut base, moussa.clone(), j.clone(), Some("present".into())).unwrap();
    }
    presences::marquer_sur(&mut base, moussa.clone(), jours[22].clone(), Some("demi".into())).unwrap();
    presences::marquer_sur(&mut base, moussa.clone(), jours[23].clone(), Some("absent".into())).unwrap();
    assert_eq!(presences::jours_travailles_sur(&mut base, &moussa, &jours[0], jours.last().unwrap()).unwrap(), 22.5);

    // Se corriger : le demi devient absent, un jour se vide.
    presences::marquer_sur(&mut base, moussa.clone(), jours[22].clone(), Some("absent".into())).unwrap();
    presences::marquer_sur(&mut base, moussa.clone(), jours[0].clone(), None).unwrap();
    assert_eq!(presences::jours_travailles_sur(&mut base, &moussa, &jours[0], jours.last().unwrap()).unwrap(), 21.0);

    let g = presences::lire_mois_sur(&mut base, &mois).unwrap();
    assert_eq!(g["jours"].as_array().unwrap().len(), jours.len());
    let m = g["personnes"].as_array().unwrap().iter().find(|p| p["employe_id"] == moussa.as_str()).unwrap();
    assert_eq!(m["jours_travailles"], 21.0);
    assert_eq!(m["etats"][&jours[1]], "present");
    assert!(m["etats"].get(&jours[0]).is_none(), "un jour vidé n'est pas su");
    let a = g["personnes"].as_array().unwrap().iter().find(|p| p["employe_id"] == awa.as_str()).unwrap();
    assert_eq!(a["jours_travailles"], 0.0);
}

#[test]
fn tout_le_monde_present_sans_ecraser_une_absence() {
    let mut base = base_avec_demo();
    let moussa = creer(&mut base, json!({ "nom": "Moussa", "fonction": "manœuvre" }));
    let awa = creer(&mut base, json!({ "nom": "Awa", "fonction": "vendeuse" }));
    let (_, jours) = mois_dernier();
    presences::marquer_sur(&mut base, awa.clone(), jours[3].clone(), Some("absent".into())).unwrap();
    let r = presences::tous_presents_sur(&mut base, jours[3].clone()).unwrap();
    assert_eq!(r["marques"], 1, "seul Moussa manquait");
    assert_eq!(presences::jours_travailles_sur(&mut base, &awa, &jours[3], &jours[3]).unwrap(), 0.0, "l'absence reste");
    assert_eq!(presences::jours_travailles_sur(&mut base, &moussa, &jours[3], &jours[3]).unwrap(), 1.0);
    assert_eq!(presences::tous_presents_sur(&mut base, jours[3].clone()).unwrap()["marques"], 0, "rejoué : rien de plus");
}

#[test]
fn ni_demain_ni_avant_l_entree_ni_apres_le_depart() {
    let mut base = base_avec_demo();
    let (_, jours) = mois_dernier();
    let ali = creer(&mut base, json!({ "nom": "Ali", "fonction": "livreur", "date_entree": jours[10] }));
    let demain = (NaiveDate::parse_from_str(&gescom_noyau::utils::maintenant_iso()[..10], "%Y-%m-%d").unwrap()
        + chrono::Duration::days(1))
    .format("%Y-%m-%d")
    .to_string();
    assert!(presences::marquer_sur(&mut base, ali.clone(), demain.clone(), Some("present".into())).unwrap_err().contains("pas encore arrivé"));
    assert!(presences::tous_presents_sur(&mut base, demain).is_err());
    assert!(presences::marquer_sur(&mut base, ali.clone(), jours[5].clone(), Some("present".into())).unwrap_err().starts_with("Ali : Entrée le"));
    personnel::faire_partir_sur(&mut base, ali.clone(), Some(jours[20].clone()), None).unwrap();
    assert!(presences::marquer_sur(&mut base, ali.clone(), jours[21].clone(), Some("present".into())).unwrap_err().contains("après"));
    presences::marquer_sur(&mut base, ali.clone(), jours[15].clone(), Some("present".into())).unwrap();
    assert!(presences::marquer_sur(&mut base, ali.clone(), jours[15].clone(), Some("malade".into())).unwrap_err().contains("inconnu"));
    // Parti dans le mois : il est encore dans la grille de ce mois.
    let (mois, _) = mois_dernier();
    let g = presences::lire_mois_sur(&mut base, &mois).unwrap();
    assert!(g["personnes"].as_array().unwrap().iter().any(|p| p["employe_id"] == ali.as_str()));
    assert!(presences::lire_mois_sur(&mut base, "sept").is_err());
}

#[test]
fn les_jours_restent_dans_leur_dossier() {
    let mut base = base_avec_demo();
    let moussa = creer(&mut base, json!({ "nom": "Moussa", "fonction": "manœuvre" }));
    let (mois, jours) = mois_dernier();
    presences::marquer_sur(&mut base, moussa.clone(), jours[2].clone(), Some("present".into())).unwrap();
    let b = dossiers::creer_dossier_sur(&mut base, "B".into(), "Boutique B".into(), None, None).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    base.choisir_dossier(&b).unwrap();
    assert!(presences::lire_mois_sur(&mut base, &mois).unwrap()["personnes"].as_array().unwrap().is_empty());
    assert_eq!(
        presences::marquer_sur(&mut base, moussa.clone(), jours[3].clone(), Some("present".into())).unwrap_err(),
        "Fiche introuvable."
    );
    assert_eq!(presences::tous_presents_sur(&mut base, jours[3].clone()).unwrap()["marques"], 0);
}

#[test]
fn tout_ce_qui_est_porte_ici_passe_le_detecteur() {
    let mut base = base_avec_demo();
    base.auditer(true);
    let moussa = creer(&mut base, json!({ "nom": "Moussa", "fonction": "manœuvre" }));
    let (mois, jours) = mois_dernier();
    presences::marquer_sur(&mut base, moussa.clone(), jours[2].clone(), Some("present".into())).unwrap();
    presences::tous_presents_sur(&mut base, jours[4].clone()).unwrap();
    presences::lire_mois_sur(&mut base, &mois).unwrap();
    presences::jours_travailles_sur(&mut base, &moussa, &jours[0], &jours[5]).unwrap();
}
