//! F-2 — les fiches du personnel (PLAN-EQUIPE, D30), sur les deux
//! moteurs et sous le compte limite : une personne, pas un contrat.

mod commun;

use commun::*;
use gescom_noyau::base::Base;
use gescom_noyau::coeur::lecture;
use gescom_noyau::personnel::{self, Fiche};
use gescom_noyau::{auth, dossiers, parametres};
use serde_json::{json, Value};

fn fiche(v: Value) -> Fiche {
    serde_json::from_value(v).expect("fiche lisible")
}

fn admin(base: &mut Base) -> String {
    base.lire_une("SELECT utilisateur_id FROM utilisateur_auth WHERE pseudo = 'admin'", &[], |r| r.get::<String>(0))
        .unwrap()
        .unwrap()
}

/// Trois personnes comme dans une boutique : Awa au mois sans contrat,
/// Moussa a la journee, Fanta vendeuse a la commission avec son compte.
fn equipe(base: &mut Base) -> (String, String, String, String) {
    let admin = admin(base);
    let compte = auth::creer_utilisateur_sur_base(
        base, "Fanta".into(), "fanta".into(), None, "secret-123".into(), "caissier".into(), admin,
    )
    .unwrap();
    let awa = personnel::creer_sur(base, fiche(json!({ "nom": " Awa ", "fonction": "vendeuse", "salaire_mensuel": 60000 }))).unwrap();
    let moussa = personnel::creer_sur(base, fiche(json!({
        "nom": "Moussa", "fonction": "manœuvre", "tarif_journalier": 2500, "telephone": "76 00 00 00"
    }))).unwrap();
    let fanta = personnel::creer_sur(base, fiche(json!({
        "nom": "Fanta", "fonction": "vendeuse", "commission_pct": 2.0, "salaire_mensuel": 25000,
        "utilisateur_id": compte, "contrat_ecrit": true, "contrat_date": "2026-01-15", "declare": true, "numero_inps": "123456"
    }))).unwrap();
    (
        awa["id"].as_str().unwrap().into(),
        moussa["id"].as_str().unwrap().into(),
        fanta["id"].as_str().unwrap().into(),
        compte,
    )
}

#[test]
fn trois_personnes_trois_facons_d_etre_payees() {
    let mut base = base_avec_demo();
    let (awa, _, fanta, compte) = equipe(&mut base);
    let liste = personnel::lister_sur(&mut base, false).unwrap();
    assert_eq!(liste.len(), 3);
    let a = personnel::lire_sur(&mut base, &awa).unwrap();
    assert_eq!(a["nom"], "Awa");
    assert_eq!(a["remuneration_dite"], "60 000 F par mois");
    assert_eq!(a["contrat_ecrit"], false, "sans contrat : permis");
    assert_eq!(a["modes"], json!(["au mois"]));
    let f = personnel::lire_sur(&mut base, &fanta).unwrap();
    assert_eq!(f["remuneration_dite"], "25 000 F par mois + 2 % des ventes");
    assert_eq!(f["utilisateur_id"], compte.as_str());
    assert_eq!(f["utilisateur_nom"], "Fanta");
    assert_eq!(f["contrat_date"], "2026-01-15");
    let m = liste.iter().find(|x| x["nom"] == "Moussa").unwrap();
    assert_eq!(m["remuneration_dite"], "2 500 F par jour");

    // Rien de fixe, c'est permis aussi.
    let r = personnel::creer_sur(&mut base, fiche(json!({ "nom": "Bakary", "fonction": "livreur" }))).unwrap();
    assert_eq!(r["remuneration_dite"], "rien de fixe");

    let n = compter(
        &mut base,
        "SELECT CAST(COUNT(*) AS BIGINT) FROM journal WHERE type_evenement = 'employe_cree' AND dossier_id = ?1",
        &parametres![dossiers::DOSSIER_DEFAUT],
    );
    assert_eq!(n, 4);
}

#[test]
fn ce_qui_ne_tient_pas_est_refuse_en_le_disant() {
    let mut base = base_avec_demo();
    let (awa, _, _, compte) = equipe(&mut base);
    let refus = |b: &mut Base, v: Value| personnel::creer_sur(b, fiche(v)).unwrap_err();
    assert!(refus(&mut base, json!({ "nom": " ", "fonction": "x" })).contains("nom"));
    assert!(refus(&mut base, json!({ "nom": "Ali", "fonction": "" })).contains("Que fait Ali"));
    assert!(refus(&mut base, json!({ "nom": "Ali", "fonction": "x", "commission_pct": 150.0 })).contains("0 et 100"));
    assert!(refus(&mut base, json!({ "nom": "Ali", "fonction": "x", "utilisateur_id": "inconnu" })).contains("n'existe pas"));
    assert!(refus(&mut base, json!({ "nom": "Ali", "fonction": "x", "utilisateur_id": compte })).contains("déjà celui de Fanta"));
    assert!(refus(&mut base, json!({ "nom": "Ali", "fonction": "x", "date_entree": "hier" })).contains("illisible"));
    // Modifier Awa sans rien changer d'illégal passe ; lui donner le compte de Fanta, non.
    personnel::modifier_sur(&mut base, awa.clone(), fiche(json!({ "nom": "Awa", "fonction": "caissière", "salaire_mensuel": 65000 }))).unwrap();
    assert!(personnel::modifier_sur(&mut base, awa.clone(), fiche(json!({ "nom": "Awa", "fonction": "x", "utilisateur_id": compte })))
        .unwrap_err()
        .contains("Fanta"));
    assert_eq!(personnel::lire_sur(&mut base, &awa).unwrap()["fonction"], "caissière");
    // Au journal : la modification, sans montant.
    let j: String = base
        .lire_une(
            "SELECT nouveau_valeur FROM journal WHERE type_evenement = 'employe_modifie' AND dossier_id = ?1",
            &parametres![dossiers::DOSSIER_DEFAUT],
            |r| r.get::<String>(0),
        )
        .unwrap()
        .unwrap();
    assert!(j.contains("\"remuneration_changee\":true") && !j.contains("65"), "{j}");
}

#[test]
fn un_depart_ne_supprime_rien_et_un_retour_reprend_la_fiche() {
    let mut base = base_avec_demo();
    let (_, moussa, fanta, compte) = equipe(&mut base);
    let r = personnel::faire_partir_sur(&mut base, moussa.clone(), Some("2026-09-20".into()), Some("Retour au village".into())).unwrap();
    assert_eq!(r["statut"], "partie");
    assert_eq!(r["date_depart"], "2026-09-20");
    assert_eq!(personnel::lister_sur(&mut base, false).unwrap().len(), 2);
    let tous = personnel::lister_sur(&mut base, true).unwrap();
    assert_eq!(tous.len(), 3);
    assert_eq!(tous.last().unwrap()["nom"], "Moussa", "les partis après les actifs");
    assert!(personnel::faire_partir_sur(&mut base, moussa.clone(), None, None).unwrap_err().contains("déjà partie"));
    personnel::faire_revenir_sur(&mut base, moussa.clone()).unwrap();
    assert_eq!(personnel::lister_sur(&mut base, false).unwrap().len(), 3);

    // Fanta part ; son compte passe à une nouvelle ; Fanta ne peut revenir avec lui.
    personnel::faire_partir_sur(&mut base, fanta.clone(), None, None).unwrap();
    personnel::creer_sur(&mut base, fiche(json!({ "nom": "Kadia", "fonction": "vendeuse", "utilisateur_id": compte }))).unwrap();
    assert!(personnel::faire_revenir_sur(&mut base, fanta).unwrap_err().contains("Kadia"));
}

#[test]
fn le_personnel_reste_dans_son_dossier() {
    let mut base = base_avec_demo();
    let (awa, _, _, _) = equipe(&mut base);
    let b = dossiers::creer_dossier_sur(&mut base, "B".into(), "Boutique B".into(), None, None).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    base.choisir_dossier(&b).unwrap();
    assert!(personnel::lister_sur(&mut base, true).unwrap().is_empty());
    assert_eq!(personnel::lire_sur(&mut base, &awa).unwrap_err(), "Fiche introuvable.");
    assert!(personnel::faire_partir_sur(&mut base, awa.clone(), None, None).is_err());
    // Un magasin de l'autre dossier ne se donne pas.
    base.choisir_dossier(dossiers::DOSSIER_DEFAUT).unwrap();
    let depot_origine = depot_defaut(&mut base);
    base.choisir_dossier(&b).unwrap();
    assert!(personnel::creer_sur(&mut base, fiche(json!({ "nom": "Ali", "fonction": "x", "depot_id": depot_origine })))
        .unwrap_err()
        .contains("pas dans ce dossier"));
}

#[test]
fn ce_que_gagne_une_personne_ne_se_lit_qu_avec_la_paie() {
    let mut base = base_avec_demo();
    equipe(&mut base);
    // Personne ne gère ni ne paie : refusé, et le refus nomme une permission.
    assert_eq!(lecture::refus("lire_personnel", |_| false), Some("personnel:gerer"));
    assert_eq!(lecture::refus("lire_personnel", |p| p == "paie:preparer"), None);
    // Qui gère sans payer voit la fiche, pas les montants.
    let mut v = serde_json::to_value(personnel::lister_sur(&mut base, false).unwrap()).unwrap();
    lecture::filtrer("lire_personnel", |p| p == "personnel:gerer", "u", &mut v);
    for p in v.as_array().unwrap() {
        assert!(p["salaire_mensuel"].is_null() && p["commission_pct"].is_null() && p["remuneration_dite"].is_null(), "{p}");
        assert!(p["modes"].is_array(), "comment elle est payée reste lisible");
    }
    let mut w = serde_json::to_value(personnel::lister_sur(&mut base, false).unwrap()).unwrap();
    lecture::filtrer("lire_personnel", |p| p == "paie:valider", "u", &mut w);
    assert!(w.as_array().unwrap().iter().any(|p| p["salaire_mensuel"] == 60000));
}

#[test]
fn tout_ce_qui_est_porte_ici_passe_le_detecteur() {
    let mut base = base_avec_demo();
    base.auditer(true);
    let (awa, _, _, _) = equipe(&mut base);
    personnel::lister_sur(&mut base, true).unwrap();
    personnel::modifier_sur(&mut base, awa.clone(), fiche(json!({ "nom": "Awa", "fonction": "x" }))).unwrap();
    personnel::faire_partir_sur(&mut base, awa.clone(), None, None).unwrap();
    personnel::faire_revenir_sur(&mut base, awa).unwrap();
}
