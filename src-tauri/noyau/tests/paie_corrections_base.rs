//! La revue de la paie du 28/10/2026 : onze defauts, chacun rejoue sur
//! son exemple, avec le montant juste (fiche, net, reste, avance creee,
//! CA mensuel, Historique masque). Sur SQLite, PostgreSQL et le compte
//! limite ; le dernier scenario rejoue tout sous le detecteur de
//! requetes non cloisonnees.
//!
//! Les dates sont celles des exemples (septembre et octobre 2026) : tous
//! ces jours sont passes, ils se marquent. Une vente, un retour ou une
//! avance se datent du jour ou on les fait ; on les antidate dans la
//! base pour les poser dans le mois de l'exemple.

mod commun;

use commun::*;
use gescom_noyau::argent::{self, ParamsLigneInput};
use gescom_noyau::base::Base;
use gescom_noyau::coeur::lecture;
use gescom_noyau::historique::{self, Filtre};
use gescom_noyau::personnel::{self, Fiche};
use gescom_noyau::{auteur, auth, avances, paie, parametres, presences, retours};
use serde_json::{json, Value};

const SEPT: (&str, &str) = ("2026-09-01", "2026-09-30");
const OCT: (&str, &str) = ("2026-10-01", "2026-10-31");

fn creer(base: &mut Base, v: Value) -> String {
    let f: Fiche = serde_json::from_value(v).unwrap();
    personnel::creer_sur(base, f).unwrap()["id"].as_str().unwrap().to_string()
}

fn fiche(v: Value) -> Fiche {
    serde_json::from_value(v).unwrap()
}

fn admin(base: &mut Base) -> String {
    base.lire_une("SELECT utilisateur_id FROM utilisateur_auth WHERE pseudo = 'admin'", &[], |r| r.get::<String>(0))
        .unwrap()
        .unwrap()
}

fn compte(base: &mut Base, nom: &str) -> String {
    let admin = admin(base);
    auth::creer_utilisateur_sur_base(base, nom.into(), nom.to_lowercase(), None, "secret-123".into(), "caissier".into(), admin).unwrap()
}

/// `2026-09-05`.
fn sept(j: u32) -> String {
    format!("2026-09-{j:02}")
}

fn marquer(base: &mut Base, employe: &str, jours: impl IntoIterator<Item = String>, etat: Option<&str>) {
    for j in jours {
        presences::marquer_sur(base, employe.into(), j, etat.map(str::to_string)).unwrap();
    }
}

/// Antidate une ligne (vente, retour, avance) au jour voulu.
fn dater(base: &mut Base, table: &str, colonne: &str, id: &str, jour: &str) {
    let dossier = base.dossier().to_string();
    base.executer(
        &format!("UPDATE {table} SET {colonne} = ?1 WHERE id = ?2 AND dossier_id = ?3"),
        &parametres![format!("{jour}T10:00:00"), id, dossier],
    )
    .unwrap();
}

/// Une vente a credit de `quantite` kg de sucre signee par `compte`, le
/// `jour` ; rend (montant, vente, ligne).
fn vendre_le(base: &mut Base, compte: &str, quantite: f64, jour: &str) -> (i64, String, String) {
    let _g = auteur::poser(compte);
    let depot = depot_defaut(base);
    let sucre = article_unite(base, "Sucre");
    let client = client_reel(base);
    let v = argent::creer_vente_sur_base(
        base,
        client,
        depot.clone(),
        "credit".into(),
        vec![ParamsLigneInput {
            article_id: sucre.0.clone(),
            unite_vente_id: sucre.1.clone(),
            depot_source_id: depot,
            source_approvisionnement: "stock".into(),
            quantite,
            facteur: sucre.2,
            prix_reference: sucre.3,
            prix_pratique: sucre.3,
            taux_tva: None,
            a_decouvert: None,
        }],
        None,
        None,
        None,
        None,
    )
    .expect("vente");
    let vente = v["vente_id"].as_str().unwrap().to_string();
    dater(base, "vente", "date_vente", &vente, jour);
    let dossier = base.dossier().to_string();
    let ligne = base
        .lire_une("SELECT id FROM ligne_vente WHERE vente_id = ?1 AND dossier_id = ?2", &parametres![vente.clone(), dossier], |r| r.get::<String>(0))
        .unwrap()
        .unwrap();
    ((sucre.3 as f64 * quantite).round() as i64, vente, ligne)
}

/// Un retour de `quantite` kg, date du `jour` ; rend son id.
fn rendre_le(base: &mut Base, vente: &str, ligne: &str, quantite: f64, jour: &str) -> String {
    let r = retours::enregistrer_retour_sur_base(
        base, vente.into(), ligne.into(), quantite, "remboursement".into(), Some("especes".into()), None, None, None, None, None,
    )
    .expect("retour");
    let id = r["retour_id"].as_str().unwrap().to_string();
    dater(base, "retour", "date_retour", &id, jour);
    id
}

fn preparer(base: &mut Base, employe: &str, (du, au): (&str, &str), prorata: bool) -> Value {
    paie::preparer_sur(base, employe.into(), du.into(), au.into(), prorata).unwrap()
}

fn id(v: &Value) -> String {
    v["id"].as_str().unwrap().to_string()
}

fn lignes<'a>(f: &'a Value, genre: &str) -> Vec<&'a Value> {
    f["lignes"].as_array().unwrap().iter().filter(|l| l["genre"] == genre).collect()
}

fn a_preparer(base: &mut Base, (du, au): (&str, &str)) -> Vec<String> {
    paie::lister_sur(base, du.into(), au.into()).unwrap()["a_preparer"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["nom"].as_str().unwrap().to_string())
        .collect()
}

fn compter_journal(base: &mut Base, type_evenement: &str) -> i64 {
    let dossier = base.dossier().to_string();
    compter(base, "SELECT CAST(COUNT(*) AS BIGINT) FROM journal WHERE type_evenement = ?1 AND dossier_id = ?2", &parametres![type_evenement, dossier])
}

// =====================================================================
//  Point 1 — les dates d'entree et de depart
// =====================================================================

#[test]
fn point_1_bakary_parti_le_15_ne_touche_que_ses_15_jours_et_issa_n_est_pas_en_septembre() {
    let mut base = base_avec_demo();
    let bakary = creer(&mut base, json!({ "nom": "Bakary", "fonction": "magasinier", "salaire_mensuel": 60000 }));
    let issa = creer(&mut base, json!({ "nom": "Issa", "fonction": "gardien", "salaire_mensuel": 40000, "date_entree": "2026-10-15" }));
    personnel::faire_partir_sur(&mut base, bakary.clone(), Some(sept(15)), None).unwrap();

    // « À préparer » : qui était employé au moins un jour de la période.
    let s = a_preparer(&mut base, SEPT);
    assert!(s.contains(&"Bakary".to_string()), "sa dernière fiche se prépare : {s:?}");
    assert!(!s.contains(&"Issa".to_string()), "Issa n'était pas là en septembre : {s:?}");
    let o = a_preparer(&mut base, OCT);
    assert!(o.contains(&"Issa".to_string()) && !o.contains(&"Bakary".to_string()), "{o:?}");

    // La période hors de l'emploi se refuse, en le disant.
    assert!(paie::preparer_sur(&mut base, issa.clone(), SEPT.0.into(), SEPT.1.into(), false).unwrap_err().contains("avant son entrée"));
    assert!(paie::preparer_sur(&mut base, bakary.clone(), OCT.0.into(), OCT.1.into(), false).unwrap_err().contains("après son départ"));

    // Bakary : 60 000 × 15 / 30, même sans prorata (avant : 60 000 F).
    let b = preparer(&mut base, &bakary, SEPT, false);
    assert_eq!(b["net"], 30_000);
    assert_eq!(lignes(&b, "base")[0]["libelle"], "Salaire du mois, 15 jours d'emploi sur 30");
    // Issa : 40 000 × 17 / 31 = 21 935,48 -> 21 935 F (avant : 40 000 F en septembre).
    let i = preparer(&mut base, &issa, OCT, false);
    assert_eq!(i["net"], 21_935);
}

#[test]
fn point_1_moussa_marque_jusqu_au_20_ne_part_pas_le_15_sans_qu_on_efface_et_touche_37_500() {
    let mut base = base_avec_demo();
    let moussa = creer(&mut base, json!({ "nom": "Moussa", "fonction": "manœuvre", "tarif_journalier": 2500 }));
    marquer(&mut base, &moussa, (1..=20).map(sept), Some("present"));
    avances::donner_sur(&mut base, moussa.clone(), 5_000, None, None).unwrap();

    // Un départ avant des jours marqués : refusé, et le message dit quoi faire.
    let e = personnel::faire_partir_sur(&mut base, moussa.clone(), Some(sept(15)), None).unwrap_err();
    assert!(e.contains("a des jours marqués après le 15/09/2026") && e.contains("les effacer d'abord"), "{e}");
    marquer(&mut base, &moussa, (16..=20).map(sept), None);
    let r = personnel::faire_partir_sur(&mut base, moussa.clone(), Some(sept(15)), None).unwrap();
    let avertissements = r["avertissements"].as_array().unwrap();
    assert!(avertissements.iter().any(|a| a.as_str().unwrap().contains("avance en cours")), "{r}");
    assert_eq!(r["avance_en_cours"], 5_000);

    // Des marques d'avant la règle, restées après le départ, ne se paient
    // pas : le calcul se borne aux jours d'emploi.
    let dossier = base.dossier().to_string();
    for j in 16..=20 {
        base.executer(
            "INSERT INTO presence (id, dossier_id, employe_id, jour, etat, saisi_par, saisi_le) VALUES (?1, ?2, ?3, ?4, 'present', NULL, ?4)",
            &parametres![uuid::Uuid::new_v4().to_string(), dossier.clone(), moussa.clone(), sept(j)],
        )
        .unwrap();
    }
    let f = preparer(&mut base, &moussa, SEPT, false);
    assert_eq!(f["net"], 37_500, "15 × 2 500 (avant : 20 × 2 500 = 50 000)");
    assert_eq!(lignes(&f, "jours")[0]["libelle"], "15 jours × 2 500 F");
}

#[test]
fn point_1_un_compte_repris_ne_commissionne_pas_deux_fois() {
    let mut base = base_avec_demo();
    let u = compte(&mut base, "Vendeuse");
    let fanta = creer(&mut base, json!({ "nom": "Fanta", "fonction": "vendeuse", "commission_pct": 3.0, "utilisateur_id": u }));
    let (m1, ..) = vendre_le(&mut base, &u, 10.0, &sept(10));
    personnel::faire_partir_sur(&mut base, fanta.clone(), Some(sept(15)), None).unwrap();
    // Kadia reprend le compte, sans date d'entrée (facultative, D30).
    let kadia = creer(&mut base, json!({ "nom": "Kadia", "fonction": "vendeuse", "commission_pct": 3.0, "utilisateur_id": u }));
    let (m2, ..) = vendre_le(&mut base, &u, 5.0, &sept(20));

    let f = preparer(&mut base, &fanta, SEPT, false);
    let k = preparer(&mut base, &kadia, SEPT, false);
    let pct = |m: i64| (m * 3 + 50) / 100;
    assert_eq!(lignes(&f, "commission")[0]["prix"], m1, "Fanta : ses ventes jusqu'au 15");
    assert_eq!(lignes(&k, "commission")[0]["prix"], m2, "Kadia : depuis le lendemain du départ de Fanta");
    assert_eq!(f["net"].as_i64().unwrap() + k["net"].as_i64().unwrap(), pct(m1) + pct(m2), "chaque vente commissionnée une fois");
}

#[test]
fn point_1_le_retour_change_la_date_d_entree_et_les_mois_d_absence_ne_reviennent_pas() {
    let mut base = base_avec_demo();
    let sali = creer(&mut base, json!({ "nom": "Sali", "fonction": "vendeuse", "salaire_mensuel": 60000, "date_entree": "2026-01-05" }));
    personnel::faire_partir_sur(&mut base, sali.clone(), Some("2026-07-31".into()), None).unwrap();
    // Sa paie jusqu'au départ n'est pas faite : le retour attend.
    let e = personnel::faire_revenir_sur(&mut base, sali.clone(), Some("2026-09-16".into())).unwrap_err();
    assert!(e.contains("n'est sur aucune fiche validée"), "{e}");
    let juillet = preparer(&mut base, &sali, ("2026-07-01", "2026-07-31"), false);
    paie::valider_sur(&mut base, id(&juillet)).unwrap();
    let r = personnel::faire_revenir_sur(&mut base, sali.clone(), Some("2026-09-16".into())).unwrap();
    assert_eq!((r["statut"].as_str(), r["date_entree"].as_str()), (Some("actif"), Some("2026-09-16")));
    // L'ancienne entrée reste au journal.
    let dossier = base.dossier().to_string();
    let ancien: String = base
        .lire_une(
            "SELECT ancien_valeur FROM journal WHERE type_evenement = 'employe_revenu' AND dossier_id = ?1",
            &parametres![dossier],
            |r| r.get::<String>(0),
        )
        .unwrap()
        .unwrap();
    assert!(ancien.contains("2026-01-05") && ancien.contains("2026-07-31"), "{ancien}");
    // Août n'est pas « à préparer » ; septembre l'est, à partir du 16.
    assert!(!a_preparer(&mut base, ("2026-08-01", "2026-08-31")).contains(&"Sali".to_string()));
    assert!(a_preparer(&mut base, SEPT).contains(&"Sali".to_string()));
    assert_eq!(preparer(&mut base, &sali, SEPT, false)["net"], 30_000, "60 000 × 15 / 30");
}

// =====================================================================
//  Point 2 — le prorata : un jour non marque compte comme present
// =====================================================================

#[test]
fn point_2_le_prorata_se_mesure_sur_les_jours_d_emploi_en_entiers() {
    let mut base = base_avec_demo();
    // Awa, 75 000 F, entrée le 15/02/2026, 14 jours marqués présents.
    let awa = creer(&mut base, json!({ "nom": "Awa", "fonction": "vendeuse", "salaire_mensuel": 75000, "date_entree": "2026-02-15" }));
    marquer(&mut base, &awa, (15..=28).map(|j| format!("2026-02-{j:02}")), Some("present"));
    let f = preparer(&mut base, &awa, ("2026-02-01", "2026-02-28"), true);
    assert_eq!(f["net"], 37_500, "75 000 × 14 / 28 (avant : 75 000 F)");
    // Sans prorata : les bornes d'emploi seules, le même montant.
    let f = paie::recalculer_sur(&mut base, id(&f), Some(false)).unwrap();
    assert_eq!(f["net"], 37_500);

    // Fatou, 60 000 F : seulement deux absences marquées en septembre.
    let fatou = creer(&mut base, json!({ "nom": "Fatou", "fonction": "vendeuse", "salaire_mensuel": 60000 }));
    marquer(&mut base, &fatou, [sept(8), sept(9)], Some("absent"));
    let f = preparer(&mut base, &fatou, SEPT, true);
    assert_eq!(f["net"], 56_000, "60 000 × 28 / 30 (avant : 0 F)");
    assert_eq!(lignes(&f, "base")[0]["libelle"], "Salaire du mois au prorata, 28 jours sur 30 (2 absences)");
    // Sans prorata, les absences ne retirent rien.
    assert_eq!(paie::recalculer_sur(&mut base, id(&f), Some(false)).unwrap()["net"], 60_000);
}

// =====================================================================
//  Point 3 — qui gere sans voir la paie n'efface pas le salaire
// =====================================================================

#[test]
fn point_3_un_gestionnaire_sans_la_paie_garde_le_salaire_et_ne_le_lit_nulle_part() {
    let mut base = base_avec_demo();
    let u = compte(&mut base, "Awa");
    let awa = creer(&mut base, json!({ "nom": "Awa", "fonction": "vendeuse", "salaire_mensuel": 60000, "commission_pct": 2.0,
                                       "avance_max": 15000, "utilisateur_id": u }));
    // Son écran reçoit les montants masqués et les renvoie vides.
    let r = personnel::modifier_selon_droits_sur(&mut base, awa.clone(), fiche(json!({ "nom": "Awa", "fonction": "caissière", "utilisateur_id": u })), false)
        .unwrap();
    assert_eq!((r["fonction"].as_str(), r["salaire_mensuel"].as_i64(), r["avance_max"].as_i64()), (Some("caissière"), Some(60_000), Some(15_000)));
    assert_eq!(r["commission_pct"], 2.0);
    // La fiche suivante n'est pas à 0 F.
    assert_eq!(lignes(&preparer(&mut base, &awa, SEPT, false), "base")[0]["montant"], 60_000);

    // Les quatre réponses de la fiche sont masquées comme les lectures.
    let gere = |p: &str| p == "personnel:gerer";
    let ali = personnel::creer_sur(&mut base, fiche(json!({ "nom": "Ali", "fonction": "livreur", "tarif_journalier": 2000 }))).unwrap();
    let ali_id = id(&ali);
    let modif = personnel::modifier_selon_droits_sur(&mut base, ali_id.clone(), fiche(json!({ "nom": "Ali", "fonction": "livreur" })), false).unwrap();
    let parti = personnel::faire_partir_sur(&mut base, ali_id.clone(), None, None).unwrap();
    let revenu = personnel::faire_revenir_sur(&mut base, ali_id.clone(), None).unwrap();
    for (cmd, mut v) in [("creer_employe", ali), ("modifier_employe", modif), ("faire_partir_employe", parti), ("faire_revenir_employe", revenu)] {
        assert_eq!(v["tarif_journalier"], 2000, "{cmd} : en base, rien n'est perdu");
        lecture::filtrer(cmd, gere, "u", &mut v);
        assert!(v["tarif_journalier"].is_null() && v["remuneration_dite"].is_null(), "{cmd} : {v}");
    }

    // Le refus d'une avance ne chiffre le plafond qu'à qui le voit.
    let e = avances::donner_selon_droits_sur(&mut base, awa.clone(), 20_000, None, None, false).unwrap_err();
    assert!(e.contains("dépasserait son plafond") && !e.contains("15 000"), "{e}");
    let e = avances::donner_selon_droits_sur(&mut base, awa.clone(), 20_000, None, None, true).unwrap_err();
    assert!(e.contains("plafond de 15 000 F"), "{e}");

    // Le journal de paie se lit avec paie:valider (D31 : qui valide relit).
    let mut p = json!({});
    lecture::neutraliser("lire_journaux_comptables", |x| x == "paie:valider", &mut p);
    assert!(p.get("avecPaie").is_none());
}

// =====================================================================
//  Point 4 — la rectificative reprend les conditions de la fiche
// =====================================================================

#[test]
fn point_4_la_rectificative_reprend_le_salaire_du_mois_rectifie() {
    let mut base = base_avec_demo();
    let awa = creer(&mut base, json!({ "nom": "Awa", "fonction": "vendeuse", "salaire_mensuel": 60000 }));
    let f = preparer(&mut base, &awa, SEPT, false);
    paie::valider_sur(&mut base, id(&f)).unwrap();
    paie::verser_sur(&mut base, id(&f), 60_000, None).unwrap();
    // Awa passe à 75 000 F en octobre.
    personnel::modifier_sur(&mut base, awa.clone(), fiche(json!({ "nom": "Awa", "fonction": "vendeuse", "salaire_mensuel": 75000 }))).unwrap();
    // Septembre rectifié pour une prime de 5 000 F.
    let r = paie::rectifier_sur(&mut base, id(&f)).unwrap();
    assert_eq!(lignes(&r, "base")[0]["montant"], 60_000, "le salaire de septembre, pas celui d'aujourd'hui");
    paie::ajouter_ligne_sur(&mut base, id(&r), "prime".into(), "Oubliée".into(), None, None, Some(5_000)).unwrap();
    let r = paie::valider_sur(&mut base, id(&r)).unwrap();
    assert_eq!((r["net"].as_i64(), r["reste"].as_i64()), (Some(65_000), Some(5_000)), "reste 5 000 F (avant : 20 000 F)");
    // Octobre, lui, prend le nouveau salaire.
    assert_eq!(preparer(&mut base, &awa, OCT, false)["net"], 75_000);
}

#[test]
fn point_4_la_declaration_et_les_taux_de_cotisation_aussi() {
    let mut base = base_avec_demo();
    let kadia = creer(&mut base, json!({ "nom": "Kadia", "fonction": "comptable", "salaire_mensuel": 100000, "declare": true }));
    let c: paie::Cotisation = serde_json::from_value(json!({ "libelle": "INPS", "qui": "salarie", "taux": 3.6 })).unwrap();
    let c: paie::Cotisation = serde_json::from_value(paie::enregistrer_cotisation_sur(&mut base, c).unwrap()).unwrap();
    let f = preparer(&mut base, &kadia, SEPT, false);
    assert_eq!(f["net"], 96_400);
    paie::valider_sur(&mut base, id(&f)).unwrap();
    // Le taux change, Kadia n'est plus déclarée : septembre ne le sait pas.
    paie::enregistrer_cotisation_sur(&mut base, paie::Cotisation { taux: 5.0, ..c }).unwrap();
    personnel::modifier_sur(&mut base, kadia.clone(), fiche(json!({ "nom": "Kadia", "fonction": "comptable", "salaire_mensuel": 100000 }))).unwrap();
    let r = paie::rectifier_sur(&mut base, id(&f)).unwrap();
    assert_eq!(r["net"], 96_400, "3,6 % de 100 000, comme la fiche rectifiée");
    assert_eq!(lignes(&r, "cotisation")[0]["libelle"], "INPS — 3,6 % de 100 000 F");
    // Le brouillon recalculé garde ses conditions figées.
    assert_eq!(paie::recalculer_sur(&mut base, id(&r), None).unwrap()["net"], 96_400);
    assert!(paie::valider_sur(&mut base, id(&r)).is_ok());
}

#[test]
fn point_4_une_fiche_d_avant_le_28_10_se_relit_sur_ses_lignes() {
    let mut base = base_avec_demo();
    let sali = creer(&mut base, json!({ "nom": "Sali", "fonction": "vendeuse", "salaire_mensuel": 50000 }));
    let f = preparer(&mut base, &sali, SEPT, false);
    paie::valider_sur(&mut base, id(&f)).unwrap();
    // Une fiche validée avant la règle n'a pas de conditions figées.
    let dossier = base.dossier().to_string();
    base.executer("UPDATE fiche_paie SET conditions = NULL WHERE id = ?1 AND dossier_id = ?2", &parametres![id(&f), dossier]).unwrap();
    personnel::modifier_sur(&mut base, sali.clone(), fiche(json!({ "nom": "Sali", "fonction": "vendeuse", "salaire_mensuel": 90000 }))).unwrap();
    let r = paie::rectifier_sur(&mut base, id(&f)).unwrap();
    assert_eq!(lignes(&r, "base")[0]["montant"], 50_000, "relu sur la ligne de l'ancienne fiche");
    let dossier = base.dossier().to_string();
    let j: String = base
        .lire_une(
            "SELECT nouveau_valeur FROM journal WHERE type_evenement = 'paie_rectificative' AND dossier_id = ?1",
            &parametres![dossier],
            |r| r.get::<String>(0),
        )
        .unwrap()
        .unwrap();
    assert!(j.contains("relues sur les lignes"), "le journal dit d'où viennent les conditions : {j}");
}

// =====================================================================
//  Point 5 — les jours d'une fiche validee sont figes
// =====================================================================

#[test]
fn point_5_un_jour_paye_ne_se_marque_plus_sauf_pendant_une_rectificative() {
    let mut base = base_avec_demo();
    let ali = creer(&mut base, json!({ "nom": "Ali", "fonction": "livreur", "tarif_journalier": 2500 }));
    marquer(&mut base, &ali, (1..=10).map(sept), Some("present"));
    let f = preparer(&mut base, &ali, SEPT, false);
    let f = paie::valider_sur(&mut base, id(&f)).unwrap();
    assert_eq!(f["net"], 25_000);
    let numero = f["numero"].as_str().unwrap().to_string();

    let e = presences::marquer_sur(&mut base, ali.clone(), sept(11), Some("present".into())).unwrap_err();
    assert!(e.contains(&format!("sur la fiche {numero} déjà validée : faire une rectificative")), "{e}");
    assert!(presences::marquer_sur(&mut base, ali.clone(), sept(5), None).is_err(), "ni effacer");
    let t = presences::tous_presents_sur(&mut base, sept(12)).unwrap();
    assert_eq!(t["marques"], 0);
    assert_eq!(t["figes"][0]["nom"], "Ali");
    let g = presences::lire_mois_sur(&mut base, "2026-09").unwrap();
    assert_eq!(g["personnes"][0]["fiches_validees"][0]["numero"], numero.as_str(), "la grille le sait");

    // Une rectificative rouvre les jours ; elle les relit.
    let fid = numero_id(&mut base, &numero);
    let r = paie::rectifier_sur(&mut base, fid).unwrap();
    presences::marquer_sur(&mut base, ali.clone(), sept(11), Some("present".into())).unwrap();
    assert!(paie::valider_sur(&mut base, id(&r)).unwrap_err().contains("recalculer"));
    assert_eq!(paie::recalculer_sur(&mut base, id(&r), None).unwrap()["net"], 27_500);
    paie::valider_sur(&mut base, id(&r)).unwrap();
    // Validée, elle fige à son tour.
    assert!(presences::marquer_sur(&mut base, ali.clone(), sept(12), Some("present".into())).unwrap_err().contains("déjà validée"));
}

fn numero_id(base: &mut Base, numero: &str) -> String {
    let dossier = base.dossier().to_string();
    base.lire_une("SELECT id FROM fiche_paie WHERE numero = ?1 AND dossier_id = ?2", &parametres![numero, dossier], |r| r.get::<String>(0))
        .unwrap()
        .unwrap()
}

// =====================================================================
//  Point 6 — le trop-verse devient une avance retenue ensuite
// =====================================================================

#[test]
fn point_6_le_trop_verse_est_repris_sur_la_fiche_suivante() {
    use gescom_noyau::journaux_comptables;
    let mut base = base_avec_demo();
    let binta = creer(&mut base, json!({ "nom": "Binta", "fonction": "vendeuse", "salaire_mensuel": 60000 }));
    let f = preparer(&mut base, &binta, SEPT, false);
    paie::valider_sur(&mut base, id(&f)).unwrap();
    paie::verser_sur(&mut base, id(&f), 60_000, None).unwrap();
    // On rectifie septembre : une casse de 8 000 F oubliée.
    let r = paie::rectifier_sur(&mut base, id(&f)).unwrap();
    paie::ajouter_ligne_sur(&mut base, id(&r), "retenue".into(), "Casse".into(), None, None, Some(8_000)).unwrap();
    let r = paie::valider_sur(&mut base, id(&r)).unwrap();
    assert_eq!((r["net"].as_i64(), r["verse"].as_i64()), (Some(52_000), Some(60_000)));
    assert_eq!((r["trop_verse"].as_i64(), r["reste"].as_i64()), (Some(8_000), Some(0)), "repris : la fiche est payée (avant : reste −8 000 F)");
    // Une avance de 8 000 F, au nom de Binta, dite et au journal.
    assert_eq!(avances::en_cours_sur(&mut base, &binta).unwrap(), 8_000);
    let a = &avances::lister_sur(&mut base, Some(binta.clone()), true).unwrap()[0];
    assert_eq!((a["moyen"].as_str(), a["trop_verse_numero"].as_str()), (Some("trop_verse"), r["numero"].as_str()));
    assert_eq!(compter_journal(&mut base, "avance_trop_verse"), 1);

    // Octobre la retient (avant : octobre ignorait les 8 000 F).
    let o = preparer(&mut base, &binta, OCT, false);
    assert_eq!((lignes(&o, "avance")[0]["montant"].as_i64(), o["net"].as_i64()), (Some(-8_000), Some(52_000)));
    paie::valider_sur(&mut base, id(&o)).unwrap();
    assert_eq!(avances::en_cours_sur(&mut base, &binta).unwrap(), 0);

    // Le journal PA reste juste : 421 soldé, 422 = ce qui reste à verser.
    let jour = gescom_noyau::utils::maintenant_iso()[..10].to_string();
    let pa = journaux_comptables::ecritures_sur(&mut base, "2026-01-01", &jour, Some("PA"), true).unwrap();
    for e in &pa {
        assert_eq!(e.lignes.iter().map(|l| l.debit).sum::<i64>(), e.lignes.iter().map(|l| l.credit).sum::<i64>(), "{}", e.libelle);
    }
    let solde = |c: &str| -> i64 { pa.iter().flat_map(|e| e.lignes.iter()).filter(|l| l.compte == c).map(|l| l.credit - l.debit).sum() };
    assert_eq!(solde("421"), 0, "l'avance née du trop-versé, puis retenue");
    assert_eq!(solde("422"), 52_000, "rien de dû sur septembre, octobre à verser");
    assert!(pa.iter().any(|e| e.libelle.starts_with("Versé en trop, repris en avance")));

    // Une seconde rectificative au même net ne reprend pas deux fois.
    let r2 = paie::rectifier_sur(&mut base, id(&r)).unwrap();
    let r2 = paie::valider_sur(&mut base, id(&r2)).unwrap();
    assert_eq!((r2["reste"].as_i64(), compter_journal(&mut base, "avance_trop_verse")), (Some(0), 1));
}

// =====================================================================
//  Point 7 — la commission deduit les retours de la periode
// =====================================================================

#[test]
fn point_7_la_commission_deduit_les_retours_a_leur_date_sauf_l_echange() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let u = compte(&mut base, "Aminata");
    let aminata = creer(&mut base, json!({ "nom": "Aminata", "fonction": "vendeuse", "commission_pct": 3.0, "utilisateur_id": u }));
    let prix = article_unite(&mut base, "Sucre").3;
    let (m, vente, ligne) = vendre_le(&mut base, &u, 10.0, &sept(10));
    rendre_le(&mut base, &vente, &ligne, 2.0, &sept(20));
    // Un échange ne rend pas la vente (RETOUR_HORS_CA) : il ne se déduit pas.
    let echange = rendre_le(&mut base, &vente, &ligne, 1.0, &sept(25));
    let dossier = base.dossier().to_string();
    base.executer("UPDATE retour SET mode_resolution = 'echange' WHERE id = ?1 AND dossier_id = ?2", &parametres![echange, dossier]).unwrap();
    // Un retour d'octobre se déduit en octobre, pas en septembre.
    rendre_le(&mut base, &vente, &ligne, 1.0, "2026-10-05");

    let f = preparer(&mut base, &aminata, SEPT, false);
    let net_ventes = m - 2 * prix;
    assert_eq!(lignes(&f, "commission")[0]["prix"], net_ventes);
    assert_eq!(f["net"], (net_ventes * 3 + 50) / 100, "3 % des ventes, retours de septembre déduits (avant : 3 % de {m})");
    let o = preparer(&mut base, &aminata, OCT, false);
    assert_eq!(o["net"], 0, "octobre : un retour sans vente, pas de commission négative");
    assert!(lignes(&o, "commission")[0]["libelle"].as_str().unwrap().contains("pas de commission"));
}

// =====================================================================
//  Point 8 — au mois, un mois civil entier
// =====================================================================

#[test]
fn point_8_deux_quinzaines_ne_font_pas_deux_salaires() {
    let mut base = base_avec_demo();
    let awa = creer(&mut base, json!({ "nom": "Awa", "fonction": "vendeuse", "salaire_mensuel": 60000 }));
    for (du, au) in [("2026-09-01", "2026-09-15"), ("2026-09-16", "2026-09-30"), ("2026-09-10", "2026-09-10")] {
        let e = paie::preparer_sur(&mut base, awa.clone(), du.into(), au.into(), false).unwrap_err();
        assert!(e.contains("mois entier"), "{du} → {au} : {e}");
    }
    // Un journalier, lui, se paie à la quinzaine.
    let moussa = creer(&mut base, json!({ "nom": "Moussa", "fonction": "manœuvre", "tarif_journalier": 2500 }));
    assert!(paie::preparer_sur(&mut base, moussa, "2026-09-01".into(), "2026-09-15".into(), false).is_ok());
    let f = preparer(&mut base, &awa, SEPT, false);
    paie::valider_sur(&mut base, id(&f)).unwrap();
    let s = paie::salaires_par_mois_sur(&mut base, "2026-01").unwrap();
    assert_eq!(s.get("2026-09"), Some(&60_000), "un salaire au CA mensuel (avant : 120 000 F)");
}

// =====================================================================
//  Point 9 — l'Historique garde les montants, mais les masque
// =====================================================================

#[test]
fn point_9_l_historique_ne_montre_la_paie_qu_a_qui_la_voit() {
    let mut base = base_avec_demo();
    let awa = creer(&mut base, json!({ "nom": "Awa", "fonction": "vendeuse", "salaire_mensuel": 60000 }));
    avances::donner_sur(&mut base, awa.clone(), 10_000, None, None).unwrap();
    let f = preparer(&mut base, &awa, SEPT, false);
    paie::valider_sur(&mut base, id(&f)).unwrap();
    paie::verser_sur(&mut base, id(&f), 45_017, None).unwrap();

    let page = historique::lire_historique_sur(&mut base, Filtre::default()).unwrap();
    let ligne = |v: &Value, t: &str| v["lignes"].as_array().unwrap().iter().find(|l| l["type"] == t).cloned().unwrap();
    // Au journal, les montants restent (traçabilité).
    assert_eq!(ligne(&page, "paie_validee")["nouveau"]["net"], 60_000);
    assert_eq!(ligne(&page, "paie_versee")["nouveau"]["montant"], 45_017);
    // À la lecture, qui n'a que journal:lire ne les voit pas.
    let mut v = page.clone();
    lecture::filtrer("lire_historique", |p| p == "journal:lire", "u", &mut v);
    assert!(ligne(&v, "paie_validee")["nouveau"]["net"].is_null());
    assert!(ligne(&v, "paie_versee")["nouveau"]["montant"].is_null());
    assert!(ligne(&v, "avance_donnee")["nouveau"]["montant"].is_null());
    assert_eq!(ligne(&v, "paie_validee")["nouveau"]["nom"], "Awa", "qui et quoi restent lisibles");
    let mut w = page.clone();
    lecture::filtrer("lire_historique", |p| p == "journal:lire" || p == "paie:valider", "u", &mut w);
    assert_eq!(w, page);
    // Et sa recherche ne trouve pas la fiche par son montant.
    let sans = historique::lire_historique_sur(&mut base, Filtre { recherche: Some("45017".into()), sans_paie: true, ..Default::default() }).unwrap();
    assert_eq!(sans["total"], 0);
    let avec = historique::lire_historique_sur(&mut base, Filtre { recherche: Some("45017".into()), ..Default::default() }).unwrap();
    assert_eq!(avec["total"], 1);
}

// =====================================================================
//  Point 10 — le reglage des cotisations, en une transaction
// =====================================================================

#[test]
fn point_10_une_cotisation_et_son_historique_s_ecrivent_ensemble() {
    let mut base = base_avec_demo();
    let c: paie::Cotisation = serde_json::from_value(json!({ "libelle": "INPS", "qui": "salarie", "taux": 3.6 })).unwrap();
    let c: paie::Cotisation = serde_json::from_value(paie::enregistrer_cotisation_sur(&mut base, c).unwrap()).unwrap();
    paie::enregistrer_cotisation_sur(&mut base, paie::Cotisation { taux: 4.0, ..c.clone() }).unwrap();
    // Un refus n'écrit ni la cotisation ni l'historique.
    assert!(paie::enregistrer_cotisation_sur(&mut base, paie::Cotisation { id: "inconnue".into(), ..c.clone() }).is_err());
    paie::retirer_cotisation_sur(&mut base, c.id.clone()).unwrap();
    assert!(paie::retirer_cotisation_sur(&mut base, c.id).is_err());
    assert_eq!(compter_journal(&mut base, "cotisation_modifiee"), 2);
    assert_eq!(compter_journal(&mut base, "cotisation_retiree"), 1);
}

// =====================================================================
//  Point 11 — les montants enormes se refusent sans faire tomber la base
// =====================================================================

#[test]
fn point_11_une_valeur_enorme_est_refusee_et_la_base_reste_debout() {
    let mut base = base_avec_demo();
    let awa = creer(&mut base, json!({ "nom": "Awa", "fonction": "vendeuse", "salaire_mensuel": 60000 }));
    let f = preparer(&mut base, &awa, SEPT, false);
    let fid = id(&f);
    for (genre, quantite, prix, montant) in [
        ("prime", None, None, Some(i64::MAX)),
        ("retenue", None, None, Some(1_000_000_001)),
        ("tache", Some(1e300), Some(1_000), None),
        ("tache", Some(100_000.0), Some(1_000_000_000), None),
    ] {
        let e = paie::ajouter_ligne_sur(&mut base, fid.clone(), genre.into(), "x".into(), quantite, prix, montant).unwrap_err();
        assert!(e.contains("trop grand") || e.contains("trop grande"), "{genre} : {e}");
    }
    assert!(avances::donner_sur(&mut base, awa.clone(), i64::MAX, None, None).unwrap_err().contains("trop grand"));
    assert!(personnel::creer_sur(&mut base, fiche(json!({ "nom": "X", "fonction": "x", "salaire_mensuel": i64::MAX }))).unwrap_err().contains("trop grand"));
    // « 15.000 » tapé à l'écran vaut 15 000 (l'écran le lit, lib/montant.ts) ;
    // ici, la base répond toujours et la fiche n'a pas bougé.
    let f = paie::ajouter_ligne_sur(&mut base, fid.clone(), "prime".into(), "Fin de mois".into(), None, None, Some(15_000)).unwrap();
    assert_eq!(f["net"], 75_000);
    let v = paie::valider_sur(&mut base, fid.clone()).unwrap();
    assert!(paie::verser_sur(&mut base, id(&v), i64::MAX, None).unwrap_err().contains("trop grand"));
}

// =====================================================================
//  Tout passe le detecteur
// =====================================================================

#[test]
fn tout_ce_qui_est_porte_ici_passe_le_detecteur() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let u = compte(&mut base, "Vendeuse");
    base.auditer(true);
    let awa = creer(&mut base, json!({ "nom": "Awa", "fonction": "vendeuse", "salaire_mensuel": 60000, "commission_pct": 2.0,
                                       "utilisateur_id": u, "declare": true, "avance_max": 50000 }));
    let moussa = creer(&mut base, json!({ "nom": "Moussa", "fonction": "manœuvre", "tarif_journalier": 2500 }));
    let c: paie::Cotisation = serde_json::from_value(json!({ "libelle": "INPS", "qui": "salarie", "taux": 3.6 })).unwrap();
    paie::enregistrer_cotisation_sur(&mut base, c).unwrap();
    let (_, vente, ligne) = vendre_le(&mut base, &u, 5.0, &sept(10));
    rendre_le(&mut base, &vente, &ligne, 1.0, &sept(12));
    marquer(&mut base, &moussa, (1..=5).map(sept), Some("present"));
    marquer(&mut base, &awa, [sept(3)], Some("absent"));
    personnel::modifier_selon_droits_sur(&mut base, awa.clone(), fiche(json!({ "nom": "Awa", "fonction": "caissière", "utilisateur_id": u })), false)
        .unwrap();
    avances::donner_selon_droits_sur(&mut base, awa.clone(), 5_000, None, None, false).unwrap();
    for p in [&awa, &moussa] {
        let f = preparer(&mut base, p, SEPT, true);
        paie::valider_sur(&mut base, id(&f)).unwrap();
        paie::verser_sur(&mut base, id(&f), 1_000, None).unwrap();
        presences::marquer_sur(&mut base, p.clone(), sept(6), Some("present".into())).unwrap_err();
        let r = paie::rectifier_sur(&mut base, id(&f)).unwrap();
        presences::marquer_sur(&mut base, p.clone(), sept(6), Some("demi".into())).unwrap();
        paie::recalculer_sur(&mut base, id(&r), None).unwrap();
        paie::ajouter_ligne_sur(&mut base, id(&r), "retenue".into(), "Casse".into(), None, None, Some(500)).unwrap();
        let r = paie::valider_sur(&mut base, id(&r)).unwrap();
        paie::donnees_bulletin_sur(&mut base, id(&r)).unwrap();
    }
    // Un trop-versé repris en avance, sous le détecteur.
    let sali = creer(&mut base, json!({ "nom": "Sali", "fonction": "vendeuse", "salaire_mensuel": 10000 }));
    let f = preparer(&mut base, &sali, SEPT, false);
    paie::valider_sur(&mut base, id(&f)).unwrap();
    paie::verser_sur(&mut base, id(&f), 10_000, None).unwrap();
    let r = paie::rectifier_sur(&mut base, id(&f)).unwrap();
    paie::ajouter_ligne_sur(&mut base, id(&r), "retenue".into(), "Casse".into(), None, None, Some(2_000)).unwrap();
    paie::valider_sur(&mut base, id(&r)).unwrap();
    avances::lister_sur(&mut base, None, false).unwrap();
    presences::tous_presents_sur(&mut base, sept(20)).unwrap();
    presences::lire_mois_sur(&mut base, "2026-09").unwrap();
    paie::lister_sur(&mut base, SEPT.0.into(), SEPT.1.into()).unwrap();
    personnel::faire_partir_sur(&mut base, moussa.clone(), Some(sept(30)), None).unwrap();
    personnel::faire_revenir_sur(&mut base, moussa.clone(), Some("2026-10-20".into())).unwrap();
    historique::lire_historique_sur(&mut base, Filtre { recherche: Some("x".into()), sans_paie: true, ..Default::default() }).unwrap();
    gescom_noyau::journaux_comptables::ecritures_sur(&mut base, "2026-01-01", "2099-12-31", Some("PA"), true).unwrap();
    let c = paie::cotisations_sur(&mut base).unwrap().remove(0);
    paie::retirer_cotisation_sur(&mut base, c.id).unwrap();
}
