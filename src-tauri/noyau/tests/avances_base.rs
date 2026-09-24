//! G-1 — les avances sur salaire (PLAN-EQUIPE, D32), sur les deux
//! moteurs et sous le compte limite : rattachees a la personne, sous son
//! plafond, **hors caisse** (decision du 24/09), et qui s'annulent tant
//! que rien n'en est retenu.

mod commun;

use commun::*;
use gescom_noyau::base::Base;
use gescom_noyau::personnel::{self, Fiche};
use gescom_noyau::{avances, caisse, dossiers, parametres};
use serde_json::json;

fn creer(base: &mut Base, v: serde_json::Value) -> String {
    let f: Fiche = serde_json::from_value(v).unwrap();
    personnel::creer_sur(base, f).unwrap()["id"].as_str().unwrap().to_string()
}

fn sorties_especes(base: &mut Base) -> i64 {
    caisse::lire_sessions_caisse_sur_base(base, None).unwrap()[0]["sorties_especes"].as_i64().unwrap_or(0)
}

fn mouvements_de_caisse(base: &mut Base) -> i64 {
    compter(
        base,
        "SELECT CAST(COUNT(*) AS BIGINT) FROM mouvement_caisse WHERE motif IN ('avance', 'avance_annulee') AND dossier_id = ?1",
        &parametres![dossiers::DOSSIER_DEFAUT],
    )
}

#[test]
fn une_avance_est_rattachee_a_la_personne_hors_caisse() {
    let mut base = base_avec_demo();
    let awa = creer(&mut base, json!({ "nom": "Awa", "fonction": "vendeuse", "salaire_mensuel": 60000, "avance_max": 15000 }));

    // La paie est independante de la caisse : pas besoin de l'ouvrir.
    let r = avances::donner_sur(&mut base, awa.clone(), 10_000, None, Some("Loyer".into())).unwrap();
    assert_eq!(r["en_cours"], 10_000);
    ouvrir_caisse(&mut base);
    assert_eq!(sorties_especes(&mut base), 0, "le tiroir du jour n'en sait rien");
    assert_eq!(mouvements_de_caisse(&mut base), 0);

    // Le plafond compte ce qui est déjà en cours.
    let e = avances::donner_sur(&mut base, awa.clone(), 6_000, None, None).unwrap_err();
    assert!(e.contains("dépasserait son plafond de 15 000 F"), "{e}");
    avances::donner_sur(&mut base, awa.clone(), 5_000, Some("orange_money".into()), None).unwrap();
    assert_eq!(avances::en_cours_sur(&mut base, &awa).unwrap(), 15_000);

    let liste = avances::lister_sur(&mut base, Some(awa.clone()), true).unwrap();
    assert_eq!(liste.len(), 2);
    assert_eq!(liste.iter().map(|a| a["reste"].as_i64().unwrap()).sum::<i64>(), 15_000);
    let n = compter(
        &mut base,
        "SELECT CAST(COUNT(*) AS BIGINT) FROM journal WHERE type_evenement = 'avance_donnee' AND dossier_id = ?1",
        &parametres![dossiers::DOSSIER_DEFAUT],
    );
    assert_eq!(n, 2);
}

#[test]
fn ce_qui_ne_se_donne_pas() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let moussa = creer(&mut base, json!({ "nom": "Moussa", "fonction": "manœuvre" }));
    assert!(avances::donner_sur(&mut base, moussa.clone(), 0, None, None).is_err());
    assert!(avances::donner_sur(&mut base, moussa.clone(), 1000, Some("bitcoin".into()), None).unwrap_err().contains("inconnu"));
    assert!(avances::donner_sur(&mut base, "inconnu".into(), 1000, None, None).unwrap_err().contains("introuvable"));
    personnel::faire_partir_sur(&mut base, moussa.clone(), None, None).unwrap();
    assert!(avances::donner_sur(&mut base, moussa, 1000, None, None).unwrap_err().contains("partie"));
    assert!(avances::lister_sur(&mut base, None, false).unwrap().is_empty(), "aucun refus n'a rien écrit");
}

#[test]
fn une_avance_donnee_par_erreur_s_annule() {
    let mut base = base_avec_demo();
    let awa = creer(&mut base, json!({ "nom": "Awa", "fonction": "vendeuse" }));
    let a = avances::donner_sur(&mut base, awa.clone(), 8_000, None, None).unwrap();
    let id = a["id"].as_str().unwrap().to_string();
    avances::annuler_sur(&mut base, id.clone(), Some("Mauvaise personne".into())).unwrap();
    assert_eq!(avances::en_cours_sur(&mut base, &awa).unwrap(), 0);
    let date = compter(
        &mut base,
        "SELECT CAST(COUNT(*) AS BIGINT) FROM avance WHERE id = ?1 AND statut = 'annulee' AND annule_le IS NOT NULL AND dossier_id = ?2",
        &parametres![id.clone(), dossiers::DOSSIER_DEFAUT],
    );
    assert_eq!(date, 1, "l'annulation est datée");
    assert_eq!(mouvements_de_caisse(&mut base), 0, "la caisse n'est pas touchée");
    assert!(avances::annuler_sur(&mut base, id, None).unwrap_err().contains("plus en cours"));

    // Une avance déjà retenue en partie par une fiche ne s'annule plus.
    let b = avances::donner_sur(&mut base, awa.clone(), 5_000, None, None).unwrap();
    let bid = b["id"].as_str().unwrap().to_string();
    base.executer("UPDATE avance SET retenu = 2000 WHERE id = ?1 AND dossier_id = ?2", &parametres![bid.clone(), dossiers::DOSSIER_DEFAUT])
        .unwrap();
    assert!(avances::annuler_sur(&mut base, bid, None).unwrap_err().contains("déjà retenu"));
    assert_eq!(avances::en_cours_sur(&mut base, &awa).unwrap(), 3_000, "reste à retenir");
}

#[test]
fn les_avances_restent_dans_leur_dossier() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let awa = creer(&mut base, json!({ "nom": "Awa", "fonction": "vendeuse" }));
    let a = avances::donner_sur(&mut base, awa.clone(), 4_000, None, None).unwrap();
    let b = dossiers::creer_dossier_sur(&mut base, "B".into(), "Boutique B".into(), None, None).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    base.choisir_dossier(&b).unwrap();
    assert!(avances::lister_sur(&mut base, None, false).unwrap().is_empty());
    assert_eq!(avances::en_cours_sur(&mut base, &awa).unwrap(), 0);
    assert_eq!(avances::annuler_sur(&mut base, a["id"].as_str().unwrap().into(), None).unwrap_err(), "Avance introuvable.");
}

#[test]
fn tout_ce_qui_est_porte_ici_passe_le_detecteur() {
    let mut base = base_avec_demo();
    base.auditer(true);
    ouvrir_caisse(&mut base);
    let awa = creer(&mut base, json!({ "nom": "Awa", "fonction": "vendeuse", "avance_max": 50000 }));
    let a = avances::donner_sur(&mut base, awa.clone(), 4_000, None, None).unwrap();
    avances::lister_sur(&mut base, None, true).unwrap();
    avances::en_cours_sur(&mut base, &awa).unwrap();
    avances::annuler_sur(&mut base, a["id"].as_str().unwrap().into(), None).unwrap();
}
