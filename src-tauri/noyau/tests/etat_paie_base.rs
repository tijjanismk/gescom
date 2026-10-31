//! EQ-2 — l'etat de paie du mois (PLAN-EQUIPE § 5, D48), sur les deux
//! moteurs et sous le compte limite. Trois personnes validees — Awa au
//! mois (avance, casse, puis une rectificative), Moussa a la journee,
//! Kadia declaree (cotisation et charge) —, deux versements partiels,
//! un brouillon et une personne sans fiche. Les totaux egalent la somme
//! des fiches et le cout des salaires de Rapports ; un caissier est
//! refuse ; chaque dossier son etat.

mod commun;

use commun::*;
use gescom_noyau::base::Base;
use gescom_noyau::coeur::lecture;
use gescom_noyau::personnel::{self, Fiche};
use gescom_noyau::{auth, avances, dossiers, etat_paie, paie, parametres, portes, presences};
use serde_json::{json, Value};

/// Le mois civil `decalage` mois apres le mois courant : `(AAAA-MM, 1er, dernier)`.
fn mois(decalage: i32) -> (String, String, String) {
    use chrono::{Datelike, Months};
    let premier = chrono::Local::now().date_naive().with_day(1).unwrap();
    let d = if decalage >= 0 {
        premier.checked_add_months(Months::new(decalage as u32)).unwrap()
    } else {
        premier.checked_sub_months(Months::new((-decalage) as u32)).unwrap()
    };
    let fin = d.checked_add_months(Months::new(1)).unwrap().pred_opt().unwrap();
    (d.format("%Y-%m").to_string(), d.format("%Y-%m-%d").to_string(), fin.format("%Y-%m-%d").to_string())
}

fn creer(base: &mut Base, v: Value) -> String {
    let f: Fiche = serde_json::from_value(v).unwrap();
    personnel::creer_sur(base, f).unwrap()["id"].as_str().unwrap().to_string()
}

fn admin(base: &mut Base) -> String {
    base.lire_une("SELECT utilisateur_id FROM utilisateur_auth WHERE pseudo = 'admin'", &[], |r| r.get::<String>(0))
        .unwrap()
        .unwrap()
}

fn cotisation(libelle: &str, qui: &str, taux: f64) -> paie::Cotisation {
    serde_json::from_value(json!({ "libelle": libelle, "qui": qui, "taux": taux, "plafond": null, "compte": null })).unwrap()
}

fn id(v: &Value) -> String {
    v["id"].as_str().unwrap().to_string()
}

fn preparer_et_valider(base: &mut Base, employe: &str, du: &str, au: &str) -> Value {
    let f = paie::preparer_sur(base, employe.into(), du.into(), au.into(), false).unwrap();
    paie::valider_sur(base, id(&f)).unwrap()
}

struct Mois {
    mois: String,
    awa: String,
    moussa: String,
    kadia: String,
    issa: String,
    bina: String,
    /// Les fiches qui valent, a la fin du scenario.
    fiches: Vec<String>,
}

/// Le mois dernier, entier : tous ses jours sont passes.
fn un_mois_de_paie(base: &mut Base) -> Mois {
    let (mois, du, au) = mois(-1);
    let le = |j: i64| format!("{}{:02}", &du[..8], j);
    paie::enregistrer_cotisation_sur(base, cotisation("INPS salarié", "salarie", 3.6)).unwrap();
    paie::enregistrer_cotisation_sur(base, cotisation("INPS employeur", "employeur", 16.4)).unwrap();
    let awa = creer(base, json!({ "nom": "Awa", "fonction": "vendeuse", "salaire_mensuel": 60000 }));
    let moussa = creer(base, json!({ "nom": "Moussa", "fonction": "manœuvre", "tarif_journalier": 2500 }));
    let kadia = creer(base, json!({ "nom": "Kadia", "fonction": "comptable", "salaire_mensuel": 100000, "declare": true }));
    let issa = creer(base, json!({ "nom": "Issa", "fonction": "gardien", "salaire_mensuel": 40000 }));
    let bina = creer(base, json!({ "nom": "Bina", "fonction": "livreur", "salaire_mensuel": 35000 }));
    for j in 1..=22 {
        presences::marquer_sur(base, moussa.clone(), le(j), Some("present".into())).unwrap();
    }
    let avance = avances::donner_sur(base, awa.clone(), 10_000, None, None).unwrap();
    let dossier = base.dossier().to_string();
    base.executer(
        "UPDATE avance SET date_avance = ?1 WHERE id = ?2 AND dossier_id = ?3",
        &parametres![format!("{}T10:00:00", le(15)), id(&avance), dossier],
    )
    .unwrap();

    // Awa : 60 000 - 10 000 d'avance - 2 000 de casse.
    let f = paie::preparer_sur(base, awa.clone(), du.clone(), au.clone(), false).unwrap();
    paie::ajouter_ligne_sur(base, id(&f), "retenue".into(), "Casse d'un carton".into(), None, None, Some(2_000)).unwrap();
    let f_awa = paie::valider_sur(base, id(&f)).unwrap();
    assert_eq!(f_awa["net"], 48_000);
    let f_moussa = preparer_et_valider(base, &moussa, &du, &au);
    assert_eq!(f_moussa["net"], 55_000);
    let f_kadia = preparer_et_valider(base, &kadia, &du, &au);
    assert_eq!(f_kadia["net"], 96_400, "100 000 - 3,6 %");

    // Deux versements partiels.
    paie::verser_sur(base, id(&f_awa), 20_000, Some("orange_money".into())).unwrap();
    paie::verser_sur(base, id(&f_moussa), 30_000, None).unwrap();
    // Issa : un brouillon. Bina : rien.
    paie::preparer_sur(base, issa.clone(), du.clone(), au.clone(), false).unwrap();

    // Awa : une prime oubliee, par une rectificative. Tant qu'elle n'est
    // pas validee, l'ancienne vaut, et la rectificative est un brouillon.
    let r = paie::rectifier_sur(base, id(&f_awa)).unwrap();
    paie::ajouter_ligne_sur(base, id(&r), "prime".into(), "Fin de mois".into(), None, None, Some(5_000)).unwrap();
    let e = etat_paie::lire_sur(base, mois.clone()).unwrap();
    let ligne_awa = e["lignes"].as_array().unwrap().iter().find(|l| l["nom"] == "Awa").unwrap();
    assert_eq!(ligne_awa["net"], 48_000, "pendant la rectification, l'ancienne fiche vaut");
    assert!(e["brouillons"].as_array().unwrap().iter().any(|b| b["nom"] == "Awa"), "la rectificative est à valider");
    let r = paie::valider_sur(base, id(&r)).unwrap();
    assert_eq!(r["net"], 53_000);

    Mois { mois, awa, moussa, kadia, issa, bina, fiches: vec![id(&r), id(&f_moussa), id(&f_kadia)] }
}

fn n(v: &Value, c: &str) -> i64 {
    v[c].as_i64().unwrap_or_else(|| panic!("{c} : {v}"))
}

const COLONNES: [&str; 9] = ["brut", "avances", "retenues", "cotisations", "charges", "net", "verse", "reste", "trop_verse"];

#[test]
fn trois_personnes_deux_versements_partiels() {
    let mut base = base_avec_demo();
    let m = un_mois_de_paie(&mut base);
    let e = etat_paie::lire_sur(&mut base, m.mois.clone()).unwrap();
    let lignes = e["lignes"].as_array().unwrap();
    assert_eq!(lignes.iter().map(|l| l["nom"].as_str().unwrap()).collect::<Vec<_>>(), ["Awa", "Kadia", "Moussa"], "par nom");

    let ligne = |qui: &str| lignes.iter().find(|l| l["employe_id"] == qui).unwrap();
    let awa = ligne(&m.awa);
    assert_eq!(
        COLONNES.map(|c| n(awa, c)),
        [65_000, 10_000, 2_000, 0, 0, 53_000, 20_000, 33_000, 0],
        "la rectificative remplace l'ancienne, et le versement l'a suivie"
    );
    assert_eq!(awa["numeros"].as_array().unwrap().len(), 1, "la fiche remplacée ne compte pas");
    assert_eq!(COLONNES.map(|c| n(ligne(&m.moussa), c)), [55_000, 0, 0, 0, 0, 55_000, 30_000, 25_000, 0]);
    assert_eq!(COLONNES.map(|c| n(ligne(&m.kadia), c)), [100_000, 0, 0, 3_600, 16_400, 96_400, 0, 96_400, 0]);

    // Les totaux : la somme des lignes, et la somme des fiches.
    let total = &e["total"];
    for c in COLONNES {
        assert_eq!(n(total, c), lignes.iter().map(|l| n(l, c)).sum::<i64>(), "total {c}");
    }
    let fiches: Vec<Value> = m.fiches.iter().map(|f| paie::lire_sur(&mut base, f).unwrap()).collect();
    for c in ["brut", "net", "verse", "reste"] {
        assert_eq!(n(total, c), fiches.iter().map(|f| n(f, c)).sum::<i64>(), "{c} : la somme des fiches");
    }
    assert_eq!(
        n(total, "avances") + n(total, "retenues") + n(total, "cotisations"),
        fiches.iter().map(|f| n(f, "retenues")).sum::<i64>(),
        "les trois colonnes de retenues font les retenues des fiches"
    );
    assert_eq!(n(total, "net"), 204_400);
    assert_eq!(n(total, "reste"), 204_400 - 50_000);
    // Le cout du mois : le meme chiffre que Rapports -> CA mensuel.
    let salaires = paie::salaires_par_mois_sur(&mut base, &m.mois).unwrap();
    assert_eq!(n(&e, "cout"), salaires[&m.mois]);
    assert_eq!(n(&e, "cout"), 220_000 + 16_400);

    // Hors des totaux : le brouillon d'Issa, Bina sans fiche.
    let brouillons = e["brouillons"].as_array().unwrap();
    assert_eq!(brouillons.len(), 1);
    assert_eq!(brouillons[0]["employe_id"], m.issa.as_str());
    let sans = e["sans_fiche"].as_array().unwrap();
    assert_eq!(sans.len(), 1);
    assert_eq!(sans[0]["employe_id"], m.bina.as_str());
    assert!(e["etabli_par"].is_string() && e["etabli_le"].is_string());

    // Le mois d'avant : rien.
    let (avant, ..) = mois(-2);
    let vide = etat_paie::lire_sur(&mut base, avant).unwrap();
    assert!(vide["lignes"].as_array().unwrap().is_empty());
    assert_eq!(n(&vide["total"], "net"), 0);
    assert!(etat_paie::lire_sur(&mut base, "2026-13".into()).unwrap_err().contains("Mois illisible"));
}

/// Une semaine a cheval compte dans le mois ou elle finit, une fois.
#[test]
fn une_fiche_compte_au_mois_de_sa_fin() {
    let mut base = base_avec_demo();
    let (mois_passe, _, fin_passe) = mois(-1);
    let (mois_courant, debut_courant, _) = mois(0);
    let moussa = creer(&mut base, json!({ "nom": "Moussa", "fonction": "manœuvre", "tarif_journalier": 2500 }));
    let du = format!("{}25", &fin_passe[..8]);
    let au = format!("{}02", &debut_courant[..8]);
    let f = preparer_et_valider(&mut base, &moussa, &du, &au);
    let e = etat_paie::lire_sur(&mut base, mois_courant.clone()).unwrap();
    assert_eq!(e["lignes"].as_array().unwrap().len(), 1);
    assert_eq!(e["lignes"][0]["numeros"][0], f["numero"]);
    assert!(etat_paie::lire_sur(&mut base, mois_passe).unwrap()["lignes"].as_array().unwrap().is_empty());
}

#[test]
fn le_csv_redit_l_etat() {
    let mut base = base_avec_demo();
    let m = un_mois_de_paie(&mut base);
    let c = etat_paie::csv_sur(&mut base, m.mois.clone()).unwrap();
    assert_eq!(c["nom_fichier"], format!("etat_paie_{}.csv", m.mois));
    let contenu = c["contenu"].as_str().unwrap();
    let lignes: Vec<&str> = contenu.trim_end().split("\r\n").collect();
    assert_eq!(lignes.len(), 5, "en-tête, trois personnes, TOTAL");
    assert!(lignes[1].starts_with("Awa;vendeuse;PAIE-"));
    assert_eq!(lignes[4], "TOTAL;;;220000;10000;2000;3600;204400;50000;154400;0;16400");
}

#[test]
fn ne_lit_l_etat_que_qui_prepare_ou_valide_la_paie() {
    let mut base = base_avec_demo();
    let admin = admin(&mut base);
    let caissier = auth::creer_utilisateur_sur_base(&mut base, "Sali".into(), "sali".into(), None, "secret-123".into(), "caissier".into(), admin.clone()).unwrap();
    let comptable = auth::creer_utilisateur_sur_base(&mut base, "Oumou".into(), "oumou".into(), None, "secret-123".into(), "comptable".into(), admin.clone()).unwrap();
    for cmd in ["lire_etat_paie", "exporter_etat_paie_csv"] {
        let perms = portes::permissions_de_sur(&mut base, &caissier, "caissier");
        assert!(lecture::refus(cmd, |p| perms.contains(p)).is_some(), "un caissier est refusé ({cmd})");
        assert!(lecture::refus(cmd, |p| p == "personnel:avancer" || p == "personnel:gerer").is_some(), "avancer ou gérer ne suffit pas");
        let perms = portes::permissions_de_sur(&mut base, &comptable, "comptable");
        assert_eq!(lecture::refus(cmd, |p| perms.contains(p)), None, "la comptable prépare la paie");
        let perms = portes::permissions_de_sur(&mut base, &admin, "patron");
        assert_eq!(lecture::refus(cmd, |p| perms.contains(p)), None, "le patron la valide");
        assert_eq!(lecture::refus(cmd, |p| p == "paie:valider"), None);
    }
}

#[test]
fn chaque_dossier_son_etat() {
    let mut base = base_avec_demo();
    let m = un_mois_de_paie(&mut base);
    let b = dossiers::creer_dossier_sur(&mut base, "B".into(), "Boutique B".into(), None, None).unwrap()["id"].as_str().unwrap().to_string();
    base.choisir_dossier(&b).unwrap();
    let e = etat_paie::lire_sur(&mut base, m.mois.clone()).unwrap();
    assert!(e["lignes"].as_array().unwrap().is_empty());
    assert!(e["brouillons"].as_array().unwrap().is_empty());
    assert!(e["sans_fiche"].as_array().unwrap().is_empty(), "le personnel de A n'est pas celui de B");
    base.choisir_dossier(dossiers::DOSSIER_DEFAUT).unwrap();
    assert_eq!(etat_paie::lire_sur(&mut base, m.mois).unwrap()["lignes"].as_array().unwrap().len(), 3);
}

#[test]
fn tout_ce_qui_est_porte_ici_passe_le_detecteur() {
    let mut base = base_avec_demo();
    base.auditer(true);
    let m = un_mois_de_paie(&mut base);
    etat_paie::lire_sur(&mut base, m.mois.clone()).unwrap();
    etat_paie::csv_sur(&mut base, m.mois).unwrap();
}
