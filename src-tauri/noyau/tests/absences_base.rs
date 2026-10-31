//! EQ-6 — les absences notees, sans calcul legal (PLAN-EQUIPE § 5,
//! D52), sur les deux moteurs et sous le compte limite. Le motif d'un
//! jour A ou ½ (maladie, permission, conge, absence) et le compte de
//! l'annee par motif ; la paie ne lit que l'etat, comme avant.

mod commun;

use commun::*;
use chrono::{Datelike, NaiveDate};
use gescom_noyau::base::Base;
use gescom_noyau::personnel::{self, Fiche};
use gescom_noyau::{dossiers, paie, parametres, presences};
use serde_json::{json, Value};

fn creer(base: &mut Base, v: Value) -> String {
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

fn marquer(base: &mut Base, e: &str, jour: &str, etat: &str, motif: Option<&str>) -> Result<Value, String> {
    presences::marquer_avec_motif_sur(base, e.into(), jour.into(), Some(etat.into()), motif.map(String::from))
}

/// Le mois de D52 : 18 presents, deux jours de maladie, une demi-journee
/// de permission, un conge, une absence sans motif dit.
fn le_mois(base: &mut Base, e: &str, jours: &[String]) {
    for j in &jours[..18] {
        marquer(base, e, j, "present", None).unwrap();
    }
    marquer(base, e, &jours[18], "absent", Some("maladie")).unwrap();
    marquer(base, e, &jours[19], "absent", Some("maladie")).unwrap();
    marquer(base, e, &jours[20], "demi", Some("permission")).unwrap();
    marquer(base, e, &jours[21], "absent", Some("conge")).unwrap();
    marquer(base, e, &jours[22], "absent", None).unwrap();
}

fn compte<'a>(a: &'a Value, e: &str) -> &'a Value {
    a["personnes"].as_array().unwrap().iter().find(|p| p["employe_id"] == e).unwrap_or(&Value::Null)
}

fn ligne<'a>(f: &'a Value, genre: &str) -> &'a Value {
    f["lignes"].as_array().unwrap().iter().find(|l| l["genre"] == genre).unwrap()
}

#[test]
fn un_mois_avec_trois_motifs_et_la_paie_comme_avant() {
    let mut base = base_avec_demo();
    let moussa = creer(&mut base, json!({ "nom": "Moussa", "fonction": "manœuvre", "tarif_journalier": 2500 }));
    let awa = creer(&mut base, json!({ "nom": "Awa", "fonction": "vendeuse", "salaire_mensuel": 60000 }));
    // Ali : les memes jours, sans aucun motif — la paie de reference.
    let ali = creer(&mut base, json!({ "nom": "Ali", "fonction": "manœuvre", "tarif_journalier": 2500 }));
    let (mois, jours) = mois_dernier();
    le_mois(&mut base, &moussa, &jours);
    le_mois(&mut base, &awa, &jours);
    for j in &jours[..18] {
        presences::marquer_sur(&mut base, ali.clone(), j.clone(), Some("present".into())).unwrap();
    }
    for (j, etat) in [(18, "absent"), (19, "absent"), (20, "demi"), (21, "absent"), (22, "absent")] {
        presences::marquer_sur(&mut base, ali.clone(), jours[j].clone(), Some(etat.into())).unwrap();
    }

    // La grille : les etats comme avant, les motifs a cote.
    let g = presences::lire_mois_sur(&mut base, &mois).unwrap();
    let m = g["personnes"].as_array().unwrap().iter().find(|p| p["employe_id"] == moussa.as_str()).unwrap();
    assert_eq!(m["jours_travailles"], 18.5);
    assert_eq!((m["etats"][&jours[18]].as_str(), m["motifs"][&jours[18]].as_str()), (Some("absent"), Some("maladie")));
    assert_eq!((m["etats"][&jours[20]].as_str(), m["motifs"][&jours[20]].as_str()), (Some("demi"), Some("permission")));
    assert_eq!(m["motifs"][&jours[21]], "conge");
    assert_eq!(m["motifs"][&jours[22]], "absence", "un A sans motif choisi est une absence");
    assert!(m["motifs"].get(&jours[0]).is_none(), "un jour présent n'a pas de motif");

    // Le compte de l'annee, par motif, en jours.
    let a = presences::absences_annee_sur(&mut base, &mois[..4]).unwrap();
    let c = compte(&a, &moussa);
    assert_eq!(c["motifs"], json!({ "maladie": 2.0, "permission": 0.5, "conge": 1.0, "absence": 1.0 }));
    assert_eq!(c["total"], 4.5);
    assert_eq!(compte(&a, &ali)["motifs"]["absence"], 4.5, "sans motif dit, tout est « absence »");

    // La paie a la journee ne compte que les jours travailles, comme
    // avant : le motif n'y change rien.
    let du = jours[0].clone();
    let au = jours.last().unwrap().clone();
    let fm = paie::preparer_sur(&mut base, moussa.clone(), du.clone(), au.clone(), false).unwrap();
    let fa = paie::preparer_sur(&mut base, ali.clone(), du.clone(), au.clone(), false).unwrap();
    assert_eq!(ligne(&fm, "jours")["libelle"], "18,5 jours × 2 500 F");
    assert_eq!(fm["net"], 46_250);
    assert_eq!(fm["net"], fa["net"], "avec ou sans motif, la même paie");

    // Le mensuel au prorata : chaque A retire, le conge aussi (D52, 3).
    let n = jours.len() as i64;
    let w = paie::preparer_sur(&mut base, awa.clone(), du, au, true).unwrap();
    let payes = 2 * (n - 4) - 1;
    assert_eq!(ligne(&w, "base")["montant"], (60_000 * payes + n) / (2 * n));
}

#[test]
fn le_motif_se_corrige_apres_la_paie_l_etat_non() {
    let mut base = base_avec_demo();
    let moussa = creer(&mut base, json!({ "nom": "Moussa", "fonction": "manœuvre", "tarif_journalier": 2500 }));
    let (mois, jours) = mois_dernier();
    for j in &jours[..10] {
        marquer(&mut base, &moussa, j, "present", None).unwrap();
    }
    marquer(&mut base, &moussa, &jours[10], "absent", None).unwrap();
    let f = paie::preparer_sur(&mut base, moussa.clone(), jours[0].clone(), jours.last().unwrap().clone(), false).unwrap();
    let id = f["id"].as_str().unwrap().to_string();
    let v = paie::valider_sur(&mut base, id.clone()).unwrap();
    let net = v["net"].as_i64().unwrap();
    assert_eq!(net, 25_000);

    // Le certificat arrive apres la paie : le motif se corrige.
    let r = marquer(&mut base, &moussa, &jours[10], "absent", Some("maladie")).unwrap();
    assert_eq!(r["motif"], "maladie");
    let a = presences::absences_annee_sur(&mut base, &mois[..4]).unwrap();
    assert_eq!(compte(&a, &moussa)["motifs"]["maladie"], 1.0);
    assert_eq!(compte(&a, &moussa)["motifs"]["absence"], 0.0);

    // L'etat, lui, reste fige : ni present, ni efface, ni un jour vide marque.
    assert!(marquer(&mut base, &moussa, &jours[10], "present", None).unwrap_err().contains("déjà validée"));
    assert!(marquer(&mut base, &moussa, &jours[10], "demi", Some("maladie")).unwrap_err().contains("déjà validée"));
    assert!(presences::marquer_sur(&mut base, moussa.clone(), jours[10].clone(), None).unwrap_err().contains("déjà validée"));
    assert!(marquer(&mut base, &moussa, &jours[12], "absent", Some("maladie")).unwrap_err().contains("déjà validée"));
    assert_eq!(paie::lire_sur(&mut base, &id).unwrap()["net"], net, "la fiche validée n'a pas bougé");
    assert_eq!(presences::jours_travailles_sur(&mut base, &moussa, &jours[0], jours.last().unwrap()).unwrap(), 10.0);
}

#[test]
fn un_motif_seulement_sur_un_jour_non_travaille() {
    let mut base = base_avec_demo();
    let awa = creer(&mut base, json!({ "nom": "Awa", "fonction": "vendeuse" }));
    let (mois, jours) = mois_dernier();
    assert!(marquer(&mut base, &awa, &jours[1], "present", Some("conge")).unwrap_err().contains("présent"));
    assert!(marquer(&mut base, &awa, &jours[1], "absent", Some("paludisme")).unwrap_err().contains("Motif inconnu"));
    assert!(presences::marquer_avec_motif_sur(&mut base, awa.clone(), jours[1].clone(), None, Some("conge".into()))
        .unwrap_err()
        .contains("effacé"));

    // « Tout le monde present » ne touche pas a un conge.
    marquer(&mut base, &awa, &jours[2], "absent", Some("conge")).unwrap();
    assert_eq!(presences::tous_presents_sur(&mut base, jours[2].clone()).unwrap()["marques"], 0);
    // Remarquer A sans motif le ramene a « absence » ; P l'efface.
    presences::marquer_sur(&mut base, awa.clone(), jours[2].clone(), Some("absent".into())).unwrap();
    let g = presences::lire_mois_sur(&mut base, &mois).unwrap();
    assert_eq!(compte(&g, &awa)["motifs"][&jours[2]], "absence");
    marquer(&mut base, &awa, &jours[2], "present", None).unwrap();
    let a = presences::absences_annee_sur(&mut base, &mois[..4]).unwrap();
    assert!(compte(&a, &awa).is_null(), "plus aucune absence : pas de ligne");

    // Un jour A d'avant D52 (motif vide en base) se compte « absence ».
    marquer(&mut base, &awa, &jours[3], "absent", Some("maladie")).unwrap();
    let dossier = base.dossier().to_string();
    base.executer("UPDATE presence SET motif = NULL WHERE dossier_id = ?1 AND employe_id = ?2", &parametres![dossier, awa.clone()])
        .unwrap();
    let a = presences::absences_annee_sur(&mut base, &mois[..4]).unwrap();
    assert_eq!(compte(&a, &awa)["motifs"]["absence"], 1.0);
    assert_eq!(compte(&presences::lire_mois_sur(&mut base, &mois).unwrap(), &awa)["motifs"][&jours[3]], "absence");

    // L'annee d'avant n'a rien ; une annee illisible se refuse.
    let avant = (mois[..4].parse::<i32>().unwrap() - 1).to_string();
    assert!(presences::absences_annee_sur(&mut base, &avant).unwrap()["personnes"].as_array().unwrap().is_empty());
    assert!(presences::absences_annee_sur(&mut base, "26").unwrap_err().contains("Année illisible"));
}

#[test]
fn les_absences_restent_dans_leur_dossier() {
    let mut base = base_avec_demo();
    let moussa = creer(&mut base, json!({ "nom": "Moussa", "fonction": "manœuvre" }));
    let (mois, jours) = mois_dernier();
    marquer(&mut base, &moussa, &jours[2], "absent", Some("maladie")).unwrap();
    let b = dossiers::creer_dossier_sur(&mut base, "B".into(), "Boutique B".into(), None, None).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    base.choisir_dossier(&b).unwrap();
    assert!(presences::absences_annee_sur(&mut base, &mois[..4]).unwrap()["personnes"].as_array().unwrap().is_empty());
    assert_eq!(marquer(&mut base, &moussa, &jours[3], "absent", Some("conge")).unwrap_err(), "Fiche introuvable.");
    base.choisir_dossier(dossiers::DOSSIER_DEFAUT).unwrap();
    let a = presences::absences_annee_sur(&mut base, &mois[..4]).unwrap();
    assert_eq!(compte(&a, &moussa)["total"], 1.0, "rien n'a fui vers B, rien n'est venu de B");
}

#[test]
fn tout_ce_qui_est_porte_ici_passe_le_detecteur() {
    let mut base = base_avec_demo();
    base.auditer(true);
    let moussa = creer(&mut base, json!({ "nom": "Moussa", "fonction": "manœuvre", "tarif_journalier": 2500 }));
    let (mois, jours) = mois_dernier();
    le_mois(&mut base, &moussa, &jours);
    let f = paie::preparer_sur(&mut base, moussa.clone(), jours[0].clone(), jours.last().unwrap().clone(), false).unwrap();
    paie::valider_sur(&mut base, f["id"].as_str().unwrap().into()).unwrap();
    marquer(&mut base, &moussa, &jours[22], "absent", Some("permission")).unwrap();
    presences::lire_mois_sur(&mut base, &mois).unwrap();
    presences::absences_annee_sur(&mut base, &mois[..4]).unwrap();
}
