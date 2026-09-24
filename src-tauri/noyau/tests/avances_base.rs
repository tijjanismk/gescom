//! G-1 — les avances sur salaire (PLAN-EQUIPE, D32), sur les deux
//! moteurs et sous le compte limite : une sortie de caisse rattachee a
//! la personne, sous son plafond, et qui s'annule en rendant l'argent.

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

#[test]
fn une_avance_sort_de_la_caisse_au_nom_de_la_personne() {
    let mut base = base_avec_demo();
    let awa = creer(&mut base, json!({ "nom": "Awa", "fonction": "vendeuse", "salaire_mensuel": 60000, "avance_max": 15000 }));

    // Caisse fermée : rien ne sort.
    assert!(avances::donner_sur(&mut base, awa.clone(), 10_000, None, None).unwrap_err().contains("ouvrir la caisse"));
    ouvrir_caisse(&mut base);
    let r = avances::donner_sur(&mut base, awa.clone(), 10_000, None, Some("Loyer".into())).unwrap();
    assert_eq!(r["en_cours"], 10_000);
    assert_eq!(sorties_especes(&mut base), 10_000, "la caisse le sait");
    let m = compter(
        &mut base,
        "SELECT CAST(COUNT(*) AS BIGINT) FROM mouvement_caisse WHERE motif = 'avance' AND operation_id = ?1 AND dossier_id = ?2",
        &parametres![r["id"].as_str().unwrap(), dossiers::DOSSIER_DEFAUT],
    );
    assert_eq!(m, 1, "le mouvement de caisse porte l'avance");

    // Le plafond compte ce qui est déjà en cours.
    let e = avances::donner_sur(&mut base, awa.clone(), 6_000, None, None).unwrap_err();
    assert!(e.contains("dépasserait son plafond de 15 000 F"), "{e}");
    avances::donner_sur(&mut base, awa.clone(), 5_000, Some("orange_money".into()), None).unwrap();
    assert_eq!(avances::en_cours_sur(&mut base, &awa).unwrap(), 15_000);
    assert_eq!(sorties_especes(&mut base), 10_000, "Orange Money ne sort pas du tiroir");

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
    assert_eq!(sorties_especes(&mut base), 0, "aucun refus n'a rien sorti");
}

#[test]
fn une_avance_donnee_par_erreur_s_annule_et_l_argent_revient() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let awa = creer(&mut base, json!({ "nom": "Awa", "fonction": "vendeuse" }));
    let a = avances::donner_sur(&mut base, awa.clone(), 8_000, None, None).unwrap();
    let id = a["id"].as_str().unwrap().to_string();
    avances::annuler_sur(&mut base, id.clone(), Some("Mauvaise personne".into())).unwrap();
    assert_eq!(avances::en_cours_sur(&mut base, &awa).unwrap(), 0);
    let entrees = compter(
        &mut base,
        "SELECT CAST(COALESCE(SUM(montant), 0) AS BIGINT) FROM mouvement_caisse
         WHERE motif = 'avance_annulee' AND sens = 'entree' AND dossier_id = ?1",
        &parametres![dossiers::DOSSIER_DEFAUT],
    );
    assert_eq!(entrees, 8_000, "l'argent revient dans le tiroir");
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
