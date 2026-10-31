//! D42 — l'argent de la paie pris dans le tiroir, l'avance datee d'un
//! jour passe et etalee. Sur SQLite, PostgreSQL et le compte limite :
//! la caisse, l'avance et la fiche verifiees apres chaque geste, et le
//! dernier scenario sous le detecteur de requetes non cloisonnees.
//!
//! Les dates relatives (« il y a trois jours ») suivent l'horloge : une
//! avance ne se date ni dans le futur ni a plus de 31 jours.

mod commun;

use chrono::{Duration, Local};
use commun::*;
use gescom_noyau::avances::{self, Saisie};
use gescom_noyau::base::Base;
use gescom_noyau::personnel::{self, Fiche};
use gescom_noyau::{caisse, journaux_comptables, paie, parametres};
use serde_json::{json, Value};

fn creer(base: &mut Base, v: Value) -> String {
    let f: Fiche = serde_json::from_value(v).unwrap();
    personnel::creer_sur(base, f).unwrap()["id"].as_str().unwrap().to_string()
}

/// Il y a `n` jours, `AAAA-MM-JJ`.
fn il_y_a(n: i64) -> String {
    (Local::now().date_naive() - Duration::days(n)).format("%Y-%m-%d").to_string()
}

fn caisse_du_jour(base: &mut Base) -> Value {
    caisse::lire_resume_caisse_sur(base).unwrap()
}

fn mouvements(base: &mut Base, motif: &str) -> i64 {
    let dossier = base.dossier().to_string();
    compter(
        base,
        "SELECT CAST(COUNT(*) AS BIGINT) FROM mouvement_caisse WHERE motif = ?1 AND dossier_id = ?2",
        &parametres![motif, dossier],
    )
}

fn nombre_d_avances(base: &mut Base) -> i64 {
    let dossier = base.dossier().to_string();
    compter(base, "SELECT CAST(COUNT(*) AS BIGINT) FROM avance WHERE dossier_id = ?1", &parametres![dossier])
}

fn tiroir(montant_par_paie: Option<i64>) -> Saisie {
    Saisie { du_tiroir: true, mensualite: montant_par_paie, ..Saisie::default() }
}

fn id(v: &Value) -> String {
    v["id"].as_str().unwrap().to_string()
}

/// Une fiche de septembre validee pour Awa (60 000 F au mois).
fn fiche_validee(base: &mut Base, employe: &str) -> String {
    let f = paie::preparer_sur(base, employe.into(), "2026-09-01".into(), "2026-09-30".into(), false).unwrap();
    paie::valider_sur(base, id(&f)).unwrap();
    id(&f)
}

#[test]
fn une_avance_prise_dans_le_tiroir_sort_de_la_caisse_et_y_revient_si_on_l_annule() {
    let mut base = base_avec_demo();
    let awa = creer(&mut base, json!({ "nom": "Awa", "fonction": "vendeuse", "salaire_mensuel": 60000 }));

    // Caisse fermee : refus, et rien n'est ecrit.
    let e = avances::donner_avance_sur(&mut base, awa.clone(), 10_000, tiroir(None), true).unwrap_err();
    assert!(e.starts_with("CAISSE_FERMEE"), "{e}");
    assert_eq!(nombre_d_avances(&mut base), 0, "pas d'avance sans sa sortie de caisse");

    let session = caisse::ouvrir_session_caisse_sur(&mut base, 50_000, "patron".into()).unwrap();
    let a = avances::donner_avance_sur(&mut base, awa.clone(), 10_000, tiroir(None), true).unwrap();
    assert_eq!(a["du_tiroir"], true);
    let c = caisse_du_jour(&mut base);
    assert_eq!((c["sorties_especes"].as_i64(), c["solde_theorique"].as_i64()), (Some(10_000), Some(40_000)), "{c}");
    assert_eq!(mouvements(&mut base, avances::MOTIF_CAISSE), 1);
    assert_eq!(avances::en_cours_sur(&mut base, &awa).unwrap(), 10_000, "elle reste a retenir sur la paie");
    let liste = avances::lister_sur(&mut base, Some(awa.clone()), true).unwrap();
    assert_eq!(liste[0]["du_tiroir"], true);

    // Le tiroir ne donne que des especes, et aujourd'hui.
    let om = Saisie { moyen: Some("orange_money".into()), ..tiroir(None) };
    assert!(avances::donner_avance_sur(&mut base, awa.clone(), 1_000, om, true).unwrap_err().contains("espèces"));
    let hier = Saisie { date: Some(il_y_a(1)), ..tiroir(None) };
    assert!(avances::donner_avance_sur(&mut base, awa.clone(), 1_000, hier, true).unwrap_err().contains("aujourd'hui"));
    assert_eq!(nombre_d_avances(&mut base), 1, "aucun refus n'a rien ecrit");

    // Annulee : l'argent revient dans le tiroir ouvert.
    avances::annuler_sur(&mut base, id(&a), Some("Erreur de personne".into())).unwrap();
    let c = caisse_du_jour(&mut base);
    assert_eq!((c["entrees_especes"].as_i64(), c["solde_theorique"].as_i64()), (Some(10_000), Some(50_000)), "{c}");
    assert_eq!(mouvements(&mut base, avances::MOTIF_CAISSE_ANNULEE), 1);
    assert_eq!(avances::en_cours_sur(&mut base, &awa).unwrap(), 0);

    // Le tiroir compte juste a la cloture.
    caisse::fermer_session_caisse_sur(&mut base, session, 50_000).unwrap();

    // Caisse fermee : une avance du tiroir ne s'annule plus sans elle.
    caisse::ouvrir_session_caisse_sur(&mut base, 10_000, "patron".into()).unwrap();
    let b = avances::donner_avance_sur(&mut base, awa.clone(), 3_000, tiroir(None), true).unwrap();
    let s = caisse_du_jour(&mut base)["session_id"].as_str().unwrap().to_string();
    caisse::fermer_session_caisse_sur(&mut base, s, 7_000).unwrap();
    assert!(avances::annuler_sur(&mut base, id(&b), None).unwrap_err().starts_with("CAISSE_FERMEE"));
    assert_eq!(avances::en_cours_sur(&mut base, &awa).unwrap(), 3_000, "l'avance est toujours due");
}

#[test]
fn une_avance_hors_caisse_ne_touche_pas_le_tiroir() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let awa = creer(&mut base, json!({ "nom": "Awa", "fonction": "vendeuse" }));
    let a = avances::donner_avance_sur(&mut base, awa, 5_000, Saisie::default(), true).unwrap();
    assert_eq!(a["du_tiroir"], false);
    assert_eq!(caisse_du_jour(&mut base)["sorties_especes"].as_i64(), Some(0));
    avances::annuler_sur(&mut base, id(&a), None).unwrap();
    assert_eq!(caisse_du_jour(&mut base)["entrees_especes"].as_i64(), Some(0), "rien ne revient : rien n'etait sorti");
}

#[test]
fn une_avance_se_date_d_un_jour_passe_borne() {
    let mut base = base_avec_demo();
    let entree = il_y_a(10);
    let awa = creer(&mut base, json!({ "nom": "Awa", "fonction": "vendeuse", "date_entree": entree }));
    let date = |d: String| Saisie { date: Some(d), ..Saisie::default() };

    let a = avances::donner_avance_sur(&mut base, awa.clone(), 5_000, date(il_y_a(3)), true).unwrap();
    assert_eq!(a["date_avance"], il_y_a(3));
    assert!(a["avertissement"].is_null(), "aucune paie validee sur ce jour");
    let demain = (Local::now().date_naive() + Duration::days(1)).format("%Y-%m-%d").to_string();
    assert!(avances::donner_avance_sur(&mut base, awa.clone(), 1_000, date(demain), true).unwrap_err().contains("futur"));
    assert!(avances::donner_avance_sur(&mut base, awa.clone(), 1_000, date(il_y_a(40)), true).unwrap_err().contains("31 jours"));
    let e = avances::donner_avance_sur(&mut base, awa.clone(), 1_000, date(il_y_a(12)), true).unwrap_err();
    assert!(e.contains("avant"), "{e}");
    assert_eq!(nombre_d_avances(&mut base), 1);

    // L'Historique garde le jour de la saisie ET le jour de l'avance.
    let dossier = base.dossier().to_string();
    let (cree, valeur) = base
        .lire_une(
            "SELECT a.cree_le, j.nouveau_valeur FROM avance a JOIN journal j ON j.entite_id = a.id AND j.dossier_id = a.dossier_id
             WHERE a.id = ?1 AND a.dossier_id = ?2",
            &parametres![id(&a), dossier],
            |r| Ok((r.get::<String>(0)?, r.get::<String>(1)?)),
        )
        .unwrap()
        .unwrap();
    assert!(cree.starts_with(&il_y_a(0)), "saisie aujourd'hui : {cree}");
    assert!(valeur.contains(&il_y_a(3)), "{valeur}");
}

#[test]
fn une_avance_datee_d_une_paie_deja_validee_le_dit() {
    let mut base = base_avec_demo();
    let moussa = creer(&mut base, json!({ "nom": "Moussa", "fonction": "manœuvre", "tarif_journalier": 2500 }));
    let f = paie::preparer_sur(&mut base, moussa.clone(), il_y_a(6), il_y_a(1), false).unwrap();
    let f = paie::valider_sur(&mut base, id(&f)).unwrap();
    let saisie = Saisie { date: Some(il_y_a(2)), ..Saisie::default() };
    let a = avances::donner_avance_sur(&mut base, moussa, 2_000, saisie, true).unwrap();
    let avis = a["avertissement"].as_str().unwrap_or_default();
    assert!(avis.contains(f["numero"].as_str().unwrap()) && avis.contains("suivante"), "{a}");
}

#[test]
fn une_avance_etalee_se_retient_par_mensualites() {
    let mut base = base_avec_demo();
    let awa = creer(&mut base, json!({ "nom": "Awa", "fonction": "vendeuse", "salaire_mensuel": 60000 }));
    let s = Saisie { mensualite: Some(10_000), ..Saisie::default() };
    let a = avances::donner_avance_sur(&mut base, awa.clone(), 30_000, s, true).unwrap();
    assert_eq!(a["mensualite"], 10_000);
    // Posee en aout, pour la retenir sur septembre puis octobre.
    let dossier = base.dossier().to_string();
    base.executer("UPDATE avance SET date_avance = '2026-08-20' WHERE id = ?1 AND dossier_id = ?2", &parametres![id(&a), dossier])
        .unwrap();

    let sept = fiche_validee(&mut base, &awa);
    let f = paie::lire_sur(&mut base, &sept).unwrap();
    let avance: Vec<&Value> = f["lignes"].as_array().unwrap().iter().filter(|l| l["genre"] == "avance").collect();
    assert_eq!(avance[0]["montant"], -10_000);
    assert_eq!(avance[0]["libelle"], "Avance du 20/08/2026 (30 000 F ; 10 000 F par paie ; 20 000 F reportés)");
    assert_eq!(f["net"], 50_000);
    assert_eq!(avances::en_cours_sur(&mut base, &awa).unwrap(), 20_000);

    let oct = paie::preparer_sur(&mut base, awa.clone(), "2026-10-01".into(), "2026-10-31".into(), false).unwrap();
    assert_eq!(oct["net"], 50_000, "10 000 F encore, pas 20 000 F");
    let zero = Saisie { mensualite: Some(0), ..Saisie::default() };
    assert!(avances::donner_avance_sur(&mut base, awa, 1_000, zero, true).unwrap_err().contains("plus grande que zéro"));
}

#[test]
fn un_salaire_verse_du_tiroir_sort_de_la_caisse() {
    let mut base = base_avec_demo();
    let awa = creer(&mut base, json!({ "nom": "Awa", "fonction": "vendeuse", "salaire_mensuel": 60000 }));
    let fiche = fiche_validee(&mut base, &awa);

    let e = paie::verser_avec_sur(&mut base, fiche.clone(), 20_000, None, true).unwrap_err();
    assert!(e.starts_with("CAISSE_FERMEE"), "{e}");
    assert_eq!(paie::lire_sur(&mut base, &fiche).unwrap()["reste"], 60_000, "rien de verse");

    caisse::ouvrir_session_caisse_sur(&mut base, 100_000, "patron".into()).unwrap();
    assert!(paie::verser_avec_sur(&mut base, fiche.clone(), 20_000, Some("moov_money".into()), true).unwrap_err().contains("espèces"));
    let f = paie::verser_avec_sur(&mut base, fiche.clone(), 20_000, None, true).unwrap();
    assert_eq!(f["reste"], 40_000);
    assert_eq!(caisse_du_jour(&mut base)["solde_theorique"].as_i64(), Some(80_000));
    assert_eq!(mouvements(&mut base, paie::MOTIF_CAISSE), 1);

    // Le reste par Orange Money, hors caisse.
    let f = paie::verser_avec_sur(&mut base, fiche.clone(), 40_000, Some("orange_money".into()), false).unwrap();
    assert_eq!(f["reste"], 0);
    assert_eq!(caisse_du_jour(&mut base)["solde_theorique"].as_i64(), Some(80_000), "le tiroir n'a rien vu");
    let tiroirs: Vec<bool> = f["versements"].as_array().unwrap().iter().map(|v| v["du_tiroir"].as_bool().unwrap()).collect();
    assert_eq!(tiroirs, vec![true, false]);
}

#[test]
fn la_comptabilite_ne_compte_pas_deux_fois_l_argent_du_tiroir() {
    let mut base = base_avec_demo();
    caisse::ouvrir_session_caisse_sur(&mut base, 100_000, "patron".into()).unwrap();
    let awa = creer(&mut base, json!({ "nom": "Awa", "fonction": "vendeuse", "salaire_mensuel": 60000 }));
    avances::donner_avance_sur(&mut base, awa, 10_000, tiroir(None), true).unwrap();
    let du = il_y_a(0);
    // PA porte l'avance une fois ; CA (les depenses de caisse) ne la
    // reprend pas : sinon 421 et 571 seraient touches deux fois.
    let pa = journaux_comptables::ecritures_sur(&mut base, &du, &du, Some("PA"), true).unwrap();
    assert_eq!(pa.iter().filter(|e| e.libelle.starts_with("Avance")).count(), 1);
    let ca = journaux_comptables::ecritures_sur(&mut base, &du, &du, Some("CA"), true).unwrap();
    assert!(ca.is_empty(), "{:?}", ca.iter().map(|e| &e.libelle).collect::<Vec<_>>());
}

#[test]
fn tout_ce_qui_est_porte_ici_passe_le_detecteur() {
    let mut base = base_avec_demo();
    base.auditer(true);
    caisse::ouvrir_session_caisse_sur(&mut base, 100_000, "patron".into()).unwrap();
    let awa = creer(&mut base, json!({ "nom": "Awa", "fonction": "vendeuse", "salaire_mensuel": 60000 }));
    let a = avances::donner_avance_sur(&mut base, awa.clone(), 4_000, tiroir(Some(2_000)), true).unwrap();
    avances::lister_sur(&mut base, None, true).unwrap();
    avances::annuler_sur(&mut base, id(&a), None).unwrap();
    let b = Saisie { date: Some(il_y_a(2)), mensualite: Some(1_000), ..Saisie::default() };
    avances::donner_avance_sur(&mut base, awa.clone(), 3_000, b, true).unwrap();
    let fiche = fiche_validee(&mut base, &awa);
    paie::verser_avec_sur(&mut base, fiche, 10_000, None, true).unwrap();
}
