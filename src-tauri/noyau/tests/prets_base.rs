//! EQ-1 (D47) — le pret au personnel sur plusieurs mois. Sur SQLite,
//! PostgreSQL et le compte limite : le pret, la fiche de paie, la caisse
//! et le journal PA verifies apres chaque geste, et le dernier scenario
//! sous le detecteur de requetes non cloisonnees.
//!
//! Un pret se date au plus 31 jours en arriere et se retient a partir du
//! mois suivant : les mois des fiches suivent l'horloge (`mois(1)` = le
//! mois prochain).

mod commun;

use chrono::{Local, Months, NaiveDate};
use commun::*;
use gescom_noyau::avances;
use gescom_noyau::base::Base;
use gescom_noyau::coeur::pret::mois_plus;
use gescom_noyau::personnel::{self, Fiche};
use gescom_noyau::prets::{self, Saisie};
use gescom_noyau::{caisse, historique, journaux_comptables, paie, parametres};
use serde_json::{json, Value};

fn creer(base: &mut Base, v: Value) -> String {
    let f: Fiche = serde_json::from_value(v).unwrap();
    personnel::creer_sur(base, f).unwrap()["id"].as_str().unwrap().to_string()
}

fn id(v: &Value) -> String {
    v["id"].as_str().unwrap().to_string()
}

/// Le mois courant + `n`, `AAAA-MM`.
fn mois(n: i64) -> String {
    mois_plus(&Local::now().format("%Y-%m").to_string(), n).unwrap()
}

/// Le premier et le dernier jour d'un mois.
fn bornes(m: &str) -> (String, String) {
    let d = NaiveDate::parse_from_str(&format!("{m}-01"), "%Y-%m-%d").unwrap();
    let f = d.checked_add_months(Months::new(1)).unwrap().pred_opt().unwrap();
    (d.format("%Y-%m-%d").to_string(), f.format("%Y-%m-%d").to_string())
}

fn preparer(base: &mut Base, employe: &str, du: &str, au: &str) -> Value {
    paie::preparer_sur(base, employe.into(), du.into(), au.into(), false).unwrap()
}

/// La fiche du mois `m`, preparee et validee.
fn fiche_du_mois(base: &mut Base, employe: &str, m: &str) -> Value {
    let (du, au) = bornes(m);
    let f = preparer(base, employe, &du, &au);
    paie::valider_sur(base, id(&f)).unwrap()
}

/// Ce que la fiche retient au titre d'un pret (en positif), 0 si rien.
fn retenu_pret(f: &Value) -> i64 {
    f["lignes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|l| l["genre"] == "avance" && l["libelle"].as_str().unwrap().starts_with("Prêt"))
        .map(|l| -l["montant"].as_i64().unwrap())
        .sum()
}

fn libelle_pret(f: &Value) -> String {
    f["lignes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["libelle"].as_str().unwrap().starts_with("Prêt"))
        .map(|l| l["libelle"].as_str().unwrap().to_string())
        .unwrap_or_default()
}

/// Une quinzaine de journalier, avec 30 000 F de prime.
fn quinzaine(base: &mut Base, employe: &str, du: &str, au: &str) -> Value {
    let f = preparer(base, employe, du, au);
    paie::ajouter_ligne_sur(base, id(&f), "prime".into(), "Quinzaine".into(), None, None, Some(30_000)).unwrap()
}

fn preter(base: &mut Base, employe: &str, montant: i64, mensualite: i64) -> Value {
    let s = Saisie { mensualite: Some(mensualite), motif: Some("Tabaski".into()), ..Saisie::default() };
    prets::preter_sur(base, employe.into(), montant, s).unwrap()
}

fn pret(base: &mut Base, p: &Value) -> Value {
    prets::lire_sur(base, &id(p)).unwrap()
}

/// Le journal PA d'aujourd'hui (les fiches s'y datent a leur validation) :
/// chaque ecriture equilibree, et le solde du compte 421 (debit - credit).
fn pa_et_421(base: &mut Base) -> (usize, i64) {
    let jour = Local::now().format("%Y-%m-%d").to_string();
    let pa = journaux_comptables::ecritures_sur(base, &jour, &jour, Some("PA"), true).unwrap();
    for e in &pa {
        let d: i64 = e.lignes.iter().map(|l| l.debit).sum();
        let c: i64 = e.lignes.iter().map(|l| l.credit).sum();
        assert_eq!(d, c, "{} {}", e.piece, e.libelle);
    }
    let solde = pa.iter().flat_map(|e| &e.lignes).filter(|l| l.compte == "421").map(|l| l.debit - l.credit).sum();
    (pa.len(), solde)
}

fn mouvements_de_caisse(base: &mut Base) -> i64 {
    let dossier = base.dossier().to_string();
    compter(base, "SELECT CAST(COUNT(*) AS BIGINT) FROM mouvement_caisse WHERE dossier_id = ?1", &parametres![dossier])
}

#[test]
fn un_pret_sur_six_fiches_se_retient_puis_se_ferme() {
    let mut base = base_avec_demo();
    let awa = creer(&mut base, json!({ "nom": "Awa", "fonction": "vendeuse", "salaire_mensuel": 60000, "avance_max": 5000 }));
    let caisse_avant = mouvements_de_caisse(&mut base);

    // 60 000 F pour la Tabaski : le plafond d'avance (5 000 F) ne compte pas.
    let p = preter(&mut base, &awa, 60_000, 10_000);
    assert_eq!((p["etat"].as_str(), p["reste"].as_i64(), p["retenir_des"].as_str()), (Some("en_cours"), Some(60_000), Some(mois(1).as_str())));
    assert_eq!(p["fin_prevue"].as_str(), Some(mois(6).as_str()), "six échéances à partir du mois prochain");
    assert_eq!(prets::en_cours_sur(&mut base, &awa).unwrap(), 60_000);
    assert_eq!(avances::en_cours_sur(&mut base, &awa).unwrap(), 0, "un prêt n'est pas une avance");
    assert!(avances::lister_sur(&mut base, None, false).unwrap().is_empty(), "l'onglet Avances ne le montre pas");
    assert_eq!(mouvements_de_caisse(&mut base), caisse_avant, "hors caisse");

    // Le mois du pret : rien n'est retenu.
    let f0 = fiche_du_mois(&mut base, &awa, &mois(0));
    assert_eq!((retenu_pret(&f0), f0["net"].as_i64(), f0["reporte"].as_i64()), (0, Some(60_000), Some(0)));

    // Six mois : 10 000 F chacun, puis soldé.
    for n in 1..=6 {
        let f = fiche_du_mois(&mut base, &awa, &mois(n));
        assert_eq!((retenu_pret(&f), f["net"].as_i64(), f["reporte"].as_i64()), (10_000, Some(50_000), Some(0)), "mois {n}");
        let r = pret(&mut base, &p);
        assert_eq!(r["reste"].as_i64(), Some(60_000 - 10_000 * n), "mois {n}");
        if n < 6 {
            assert!(libelle_pret(&f).contains(&format!("reste {}", gescom_noyau::coeur::plafonds::francs(60_000 - 10_000 * n))), "{}", libelle_pret(&f));
        }
    }
    let r = pret(&mut base, &p);
    assert_eq!((r["etat"].as_str(), r["reste"].as_i64(), r["retenu_paie"].as_i64()), (Some("solde"), Some(0), Some(60_000)));
    assert!(r["fin_prevue"].is_null());
    assert_eq!(r["mouvements"].as_array().unwrap().len(), 6, "{r}");
    assert!(prets::lister_sur(&mut base, Some(awa.clone()), true).unwrap().is_empty(), "soldé : plus en cours");
    assert_eq!(prets::lister_sur(&mut base, Some(awa.clone()), false).unwrap().len(), 1, "mais lisible");

    // Le septieme mois : plus rien.
    let f7 = fiche_du_mois(&mut base, &awa, &mois(7));
    assert_eq!((retenu_pret(&f7), f7["net"].as_i64()), (0, Some(60_000)));

    // PA : le pret au debit de 421, six retenues au credit ; 421 soldé.
    let (n, solde) = pa_et_421(&mut base);
    assert!(n >= 9, "le prêt et huit fiches : {n}");
    assert_eq!(solde, 0);
    assert_eq!(mouvements_de_caisse(&mut base), caisse_avant);
}

#[test]
fn une_fiche_trop_maigre_retient_ce_qu_elle_peut_et_le_pret_s_allonge() {
    let mut base = base_avec_demo();
    let awa = creer(&mut base, json!({ "nom": "Awa", "fonction": "vendeuse", "salaire_mensuel": 60000 }));
    let p = preter(&mut base, &awa, 30_000, 10_000);
    // Une avance d'aujourd'hui, toute sur la prochaine paie : les avances d'abord.
    avances::donner_sur(&mut base, awa.clone(), 55_000, None, None).unwrap();

    let f1 = fiche_du_mois(&mut base, &awa, &mois(1));
    assert_eq!(retenu_pret(&f1), 5_000, "60 000 - 55 000 d'avance");
    assert_eq!((f1["net"].as_i64(), f1["reporte"].as_i64()), (Some(0), Some(5_000)), "le manque à l'échéance se reporte");
    assert!(libelle_pret(&f1).contains("5 000 F faute de net"), "{}", libelle_pret(&f1));
    assert_eq!(avances::en_cours_sur(&mut base, &awa).unwrap(), 0, "l'avance est retenue en entier");

    // Le mois suivant : l'echeance, pas l'echeance plus le manque.
    let f2 = fiche_du_mois(&mut base, &awa, &mois(2));
    assert_eq!(retenu_pret(&f2), 10_000);
    let r = pret(&mut base, &p);
    assert_eq!(r["reste"].as_i64(), Some(15_000));
    assert_eq!(r["dernier_mois_retenu"].as_str(), Some(mois(2).as_str()));
    assert_eq!(r["fin_prevue"].as_str(), Some(mois(4).as_str()), "une échéance de plus qu'au départ (mois 3)");

    // Les deux dernieres, la seconde plus petite.
    let f3 = fiche_du_mois(&mut base, &awa, &mois(3));
    assert_eq!(retenu_pret(&f3), 10_000);
    let f4 = fiche_du_mois(&mut base, &awa, &mois(4));
    assert_eq!(retenu_pret(&f4), 5_000);
    assert!(libelle_pret(&f4).ends_with("dernière échéance"), "{}", libelle_pret(&f4));
    assert_eq!(pret(&mut base, &p)["etat"], "solde");
    assert_eq!(pa_et_421(&mut base).1, 0, "le prêt et l'avance sont soldés");
}

#[test]
fn a_la_quinzaine_l_echeance_ne_se_paie_qu_une_fois_par_mois() {
    let mut base = base_avec_demo();
    let moussa = creer(&mut base, json!({ "nom": "Moussa", "fonction": "manœuvre", "tarif_journalier": 2500 }));
    let p = preter(&mut base, &moussa, 30_000, 10_000);
    let (du, au) = bornes(&mois(1));
    let milieu = format!("{}-15", mois(1));
    let seize = format!("{}-16", mois(1));

    // Les jours a venir ne se marquent pas encore : une prime fait le
    // gain de chaque quinzaine.
    let a = quinzaine(&mut base, &moussa, &du, &milieu);
    let a = paie::valider_sur(&mut base, id(&a)).unwrap();
    assert_eq!(retenu_pret(&a), 10_000);
    let b = quinzaine(&mut base, &moussa, &seize, &au);
    assert_eq!(retenu_pret(&b), 0, "l'échéance du mois est déjà payée");
    assert_eq!(b["reporte"].as_i64(), Some(0), "rien ne manque");
    paie::valider_sur(&mut base, id(&b)).unwrap();

    let (du2, _) = bornes(&mois(2));
    let c = quinzaine(&mut base, &moussa, &du2, &format!("{}-15", mois(2)));
    assert_eq!(retenu_pret(&c), 10_000, "le mois suivant, l'échéance revient");
    paie::valider_sur(&mut base, id(&c)).unwrap();
    assert_eq!(pret(&mut base, &p)["reste"].as_i64(), Some(10_000));
}

#[test]
fn la_rectificative_rend_ce_qu_elle_avait_retenu_et_rouvre_un_pret_solde() {
    let mut base = base_avec_demo();
    let awa = creer(&mut base, json!({ "nom": "Awa", "fonction": "vendeuse", "salaire_mensuel": 60000 }));
    let p = preter(&mut base, &awa, 20_000, 10_000);
    let f1 = fiche_du_mois(&mut base, &awa, &mois(1));
    assert_eq!(pret(&mut base, &p)["reste"].as_i64(), Some(10_000));

    // Une casse oubliee en mois 1 : le net tombe a 5 000 F avant le pret.
    let r = paie::rectifier_sur(&mut base, id(&f1)).unwrap();
    assert_eq!(retenu_pret(&r), 10_000, "le brouillon rend d'abord ce que l'ancienne avait retenu");
    let r = paie::ajouter_ligne_sur(&mut base, id(&r), "retenue".into(), "Casse".into(), None, None, Some(55_000)).unwrap();
    assert_eq!((retenu_pret(&r), r["reporte"].as_i64()), (5_000, Some(5_000)));
    paie::valider_sur(&mut base, id(&r)).unwrap();
    assert_eq!(pret(&mut base, &p)["reste"].as_i64(), Some(15_000), "10 000 rendus, 5 000 retenus");

    // Mois 2 et 3 : le pret se solde.
    fiche_du_mois(&mut base, &awa, &mois(2));
    let f3 = fiche_du_mois(&mut base, &awa, &mois(3));
    assert_eq!(retenu_pret(&f3), 5_000);
    assert_eq!(pret(&mut base, &p)["etat"], "solde");

    // Une rectificative du mois 3 qui ne laisse rien : le pret se rouvre.
    let r3 = paie::rectifier_sur(&mut base, id(&f3)).unwrap();
    let r3 = paie::ajouter_ligne_sur(&mut base, id(&r3), "retenue".into(), "Absence".into(), None, None, Some(60_000)).unwrap();
    assert_eq!(retenu_pret(&r3), 0);
    paie::valider_sur(&mut base, id(&r3)).unwrap();
    let r = pret(&mut base, &p);
    assert_eq!((r["etat"].as_str(), r["reste"].as_i64()), (Some("en_cours"), Some(5_000)), "{r}");
    assert_eq!(pa_et_421(&mut base).1, 5_000, "421 dit ce qui reste dû");
}

#[test]
fn au_depart_la_derniere_fiche_retient_tout_le_reste_puis_remboursement_et_remise() {
    let mut base = base_avec_demo();
    let awa = creer(&mut base, json!({ "nom": "Awa", "fonction": "vendeuse", "salaire_mensuel": 60000 }));
    let p = preter(&mut base, &awa, 60_000, 10_000);
    fiche_du_mois(&mut base, &awa, &mois(1));

    // Elle part le 10 du mois 2 : la fiche du depart paie dix jours et
    // retient tout ce qu'elle peut.
    let depart = format!("{}-10", mois(2));
    let v = personnel::faire_partir_sur(&mut base, awa.clone(), Some(depart), None).unwrap();
    let avis: Vec<&str> = v["avertissements"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
    assert!(avis.iter().any(|a| a.contains("prêt en cours") && !a.contains("50 000")), "{avis:?}");
    let f2 = fiche_du_mois(&mut base, &awa, &mois(2));
    let net_avant = f2["brut"].as_i64().unwrap();
    assert!(net_avant > 0 && net_avant < 50_000, "dix jours de salaire : {net_avant}");
    assert_eq!((retenu_pret(&f2), f2["net"].as_i64()), (net_avant, Some(0)), "tout le net va au prêt");
    assert!(libelle_pret(&f2).contains("restent dus"), "{}", libelle_pret(&f2));
    let reste = 50_000 - net_avant;
    assert_eq!(f2["reporte"].as_i64(), Some(reste));
    assert_eq!(pret(&mut base, &p)["reste"].as_i64(), Some(reste));
    assert!(prets::preter_sur(&mut base, awa.clone(), 10_000, Saisie { mensualite: Some(1_000), ..Saisie::default() })
        .unwrap_err()
        .contains("partie"));

    // Elle rembourse 10 000 F par Orange Money ; le patron remet le reste.
    let r = prets::rembourser_sur(&mut base, id(&p), 10_000, Some("orange_money".into()), None).unwrap();
    assert_eq!(r["reste"].as_i64(), Some(reste - 10_000));
    assert!(prets::rembourser_sur(&mut base, id(&p), reste, None, None).unwrap_err().contains("Il ne reste que"));
    let r = prets::remettre_sur(&mut base, id(&p), Some("Départ en bons termes".into())).unwrap();
    assert_eq!((r["etat"].as_str(), r["rembourse"].as_i64(), r["remis"].as_i64()), (Some("solde"), Some(10_000), Some(reste - 10_000)));
    assert_eq!(r["mouvements"].as_array().unwrap().len(), 4, "deux fiches, un remboursement, une remise : {r}");
    assert!(prets::remettre_sur(&mut base, id(&p), None).unwrap_err().contains("soldé"));

    // PA : remboursement tresorerie / 421, remise 658 / 421 ; 421 soldé.
    let jour = Local::now().format("%Y-%m-%d").to_string();
    let pa = journaux_comptables::ecritures_sur(&mut base, &jour, &jour, Some("PA"), true).unwrap();
    let remise = pa.iter().find(|e| e.libelle.starts_with("Prêt remis")).expect("la remise au journal");
    assert!(remise.lignes.iter().any(|l| l.compte == "658" && l.debit == reste - 10_000), "{:?}", remise.lignes);
    assert!(pa.iter().any(|e| e.libelle.starts_with("Prêt remboursé")));
    assert_eq!(pa_et_421(&mut base).1, 0);
}

#[test]
fn ce_qui_est_refuse() {
    let mut base = base_avec_demo();
    let awa = creer(&mut base, json!({ "nom": "Awa", "fonction": "vendeuse", "salaire_mensuel": 60000 }));
    let s = |m: Option<i64>| Saisie { mensualite: m, ..Saisie::default() };
    assert!(prets::preter_sur(&mut base, awa.clone(), 60_000, s(None)).unwrap_err().contains("chaque mois"));
    assert!(prets::preter_sur(&mut base, awa.clone(), 60_000, s(Some(60_000))).unwrap_err().contains("avance"));
    let trop_tard = Saisie { premier_mois: Some(mois(5)), ..s(Some(10_000)) };
    assert!(prets::preter_sur(&mut base, awa.clone(), 60_000, trop_tard).unwrap_err().contains("première retenue"));

    // Caisse fermee, pris dans le tiroir : rien ne s'ecrit.
    let tiroir = Saisie { du_tiroir: true, ..s(Some(10_000)) };
    assert!(prets::preter_sur(&mut base, awa.clone(), 60_000, tiroir.clone()).unwrap_err().starts_with("CAISSE_FERMEE"));
    assert!(prets::lister_sur(&mut base, None, false).unwrap().is_empty());

    // Caisse ouverte : la sortie s'ecrit ; annule, l'argent revient.
    caisse::ouvrir_session_caisse_sur(&mut base, 100_000, "patron".into()).unwrap();
    let p = prets::preter_sur(&mut base, awa.clone(), 60_000, tiroir).unwrap();
    assert_eq!(p["du_tiroir"], true);
    assert_eq!(caisse::lire_resume_caisse_sur(&mut base).unwrap()["solde_theorique"].as_i64(), Some(40_000));
    assert!(prets::preter_sur(&mut base, awa.clone(), 10_000, s(Some(1_000))).unwrap_err().contains("un seul"));
    assert!(avances::annuler_sur(&mut base, id(&p), None).unwrap_err().contains("onglet Prêts"), "pas par le droit des avances");
    prets::annuler_sur(&mut base, id(&p), None).unwrap();
    assert_eq!(caisse::lire_resume_caisse_sur(&mut base).unwrap()["solde_theorique"].as_i64(), Some(100_000));
    assert_eq!(pret(&mut base, &p)["etat"], "annule");
    assert!(prets::rembourser_sur(&mut base, id(&p), 1_000, None, None).unwrap_err().contains("annulé"));

    // Une fois retenu, il ne s'annule plus.
    let q = preter(&mut base, &awa, 20_000, 10_000);
    fiche_du_mois(&mut base, &awa, &mois(1));
    assert!(prets::annuler_sur(&mut base, id(&q), None).unwrap_err().contains("ne s'annule plus"));
    assert!(prets::annuler_sur(&mut base, "inconnu".into(), None).is_err());
}

#[test]
fn tout_ce_qui_est_porte_ici_passe_le_detecteur() {
    let mut base = base_avec_demo();
    base.auditer(true);
    caisse::ouvrir_session_caisse_sur(&mut base, 100_000, "patron".into()).unwrap();
    let awa = creer(&mut base, json!({ "nom": "Awa", "fonction": "vendeuse", "salaire_mensuel": 60000 }));
    let a = prets::preter_sur(&mut base, awa.clone(), 6_000, Saisie { mensualite: Some(2_000), du_tiroir: true, ..Saisie::default() }).unwrap();
    prets::annuler_sur(&mut base, id(&a), None).unwrap();
    let p = preter(&mut base, &awa, 60_000, 10_000);
    prets::lister_sur(&mut base, None, true).unwrap();
    prets::lister_sur(&mut base, Some(awa.clone()), false).unwrap();
    let f = fiche_du_mois(&mut base, &awa, &mois(1));
    let r = paie::rectifier_sur(&mut base, id(&f)).unwrap();
    paie::recalculer_sur(&mut base, id(&r), None).unwrap();
    paie::valider_sur(&mut base, id(&r)).unwrap();
    prets::rembourser_sur(&mut base, id(&p), 5_000, None, None).unwrap();
    personnel::faire_partir_sur(&mut base, awa.clone(), Some(format!("{}-05", mois(2))), None).unwrap();
    prets::remettre_sur(&mut base, id(&p), None).unwrap();
    prets::lire_sur(&mut base, &id(&p)).unwrap();
    pa_et_421(&mut base);
    historique::lire_historique_sur(&mut base, historique::Filtre { recherche: Some("prêt".into()), sans_paie: true, ..Default::default() })
        .unwrap();
}
