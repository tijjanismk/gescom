//! D56 — l'avance etalee se retient une fois par mois civil, comme le
//! pret ; la commission d'une vente annulee apres qu'une fiche validee
//! l'a payee se reprend sur la fiche suivante, sans rien reecrire. Sur
//! SQLite, PostgreSQL et le compte limite : apres chaque geste, ce que
//! la personne doit encore (avance), les lignes de la fiche validee, le
//! journal PA ; le dernier scenario sous le detecteur de requetes non
//! cloisonnees.
//!
//! Les dates sont celles des exemples (aout a novembre 2026) : une vente,
//! un reglement, une annulation se datent du jour ou on les fait ; on les
//! antidate dans la base pour les poser dans le mois voulu.

mod commun;

use commun::*;
use gescom_noyau::argent::{self, ParamsLigneInput};
use gescom_noyau::avances::{self, Saisie};
use gescom_noyau::base::Base;
use gescom_noyau::personnel::{self, Fiche};
use gescom_noyau::{auteur, auth, creances, journaux_comptables, paie, parametres, pieces, presences};
use serde_json::{json, Value};

fn creer(base: &mut Base, v: Value) -> String {
    let f: Fiche = serde_json::from_value(v).unwrap();
    personnel::creer_sur(base, f).unwrap()["id"].as_str().unwrap().to_string()
}

fn id(v: &Value) -> String {
    v["id"].as_str().unwrap().to_string()
}

fn compte(base: &mut Base, nom: &str) -> String {
    let admin = base
        .lire_une("SELECT utilisateur_id FROM utilisateur_auth WHERE pseudo = 'admin'", &[], |r| r.get::<String>(0))
        .unwrap()
        .unwrap();
    auth::creer_utilisateur_sur_base(base, nom.into(), nom.to_lowercase(), None, "secret-123".into(), "caissier".into(), admin).unwrap()
}

fn vendeuse(base: &mut Base, nom: &str, salaire: Option<i64>) -> (String, String) {
    let u = compte(base, nom);
    let e = creer(base, json!({ "nom": nom, "fonction": "vendeuse", "commission_pct": 3.0, "salaire_mensuel": salaire, "utilisateur_id": u }));
    (u, e)
}

fn dater(base: &mut Base, sql: &str, id: &str, jour: &str) {
    let dossier = base.dossier().to_string();
    base.executer(sql, &parametres![format!("{jour}T10:00:00"), id, dossier]).unwrap();
}

/// Une vente a credit de `quantite` kg de sucre signee par `compte` le
/// `jour`, payee en entier le meme jour, avec sa facture ; rend
/// (montant, vente, facture).
fn vendre_paye(base: &mut Base, compte: &str, quantite: f64, jour: &str) -> (i64, String, String) {
    let depot = depot_defaut(base);
    let sucre = article_unite(base, "Sucre");
    let client = client_reel(base);
    let vente = {
        let _g = auteur::poser(compte);
        let v = argent::creer_vente_sur_base(
            base,
            client.clone(),
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
        v["vente_id"].as_str().unwrap().to_string()
    };
    let montant = (sucre.3 as f64 * quantite).round() as i64;
    creances::regler_creance_sur_base(base, vente.clone(), montant, "especes".into(), None).expect("reglement");
    let f = argent::creer_facture_depuis_vente_sur_base(base, vente.clone(), client, "credit".into(), None).expect("facture");
    let facture = f["piece_id"].as_str().or(f["id"].as_str()).unwrap().to_string();
    dater(base, "UPDATE vente SET date_vente = ?1, modifie_le = ?1 WHERE id = ?2 AND dossier_id = ?3", &vente, jour);
    dater(base, "UPDATE paiement SET date_paiement = ?1 WHERE vente_id = ?2 AND dossier_id = ?3", &vente, jour);
    (montant, vente, facture)
}

/// Annule la facture par avoir (le client garde son argent en avoir), le
/// `jour`.
fn annuler_le(base: &mut Base, vente: &str, facture: &str, jour: &str) {
    pieces::annuler_facture_par_avoir_sur_base(base, facture.into(), Some("avoir".into()), None, Some("Client revenu".into())).expect("annulation");
    let dossier = base.dossier().to_string();
    base.executer(
        "UPDATE piece_commerciale SET date_piece = ?1 WHERE piece_origine_id = ?2 AND origine = 'annulation' AND dossier_id = ?3",
        &parametres![format!("{jour}T10:00:00"), facture, dossier],
    )
    .unwrap();
    dater(base, "UPDATE vente SET modifie_le = ?1 WHERE id = ?2 AND dossier_id = ?3", vente, jour);
}

fn numero(base: &mut Base, facture: &str) -> String {
    let dossier = base.dossier().to_string();
    base.lire_une("SELECT numero FROM piece_commerciale WHERE id = ?1 AND dossier_id = ?2", &parametres![facture, dossier], |r| r.get::<String>(0))
        .unwrap()
        .unwrap()
}

fn preparer(base: &mut Base, employe: &str, du: &str, au: &str) -> Value {
    paie::preparer_sur(base, employe.into(), du.into(), au.into(), false).unwrap()
}

fn valider(base: &mut Base, f: &Value) -> Value {
    paie::valider_sur(base, id(f)).unwrap()
}

fn preparer_valider(base: &mut Base, employe: &str, du: &str, au: &str) -> Value {
    let f = preparer(base, employe, du, au);
    valider(base, &f)
}

fn lignes<'a>(f: &'a Value, genre: &str) -> Vec<&'a Value> {
    f["lignes"].as_array().unwrap().iter().filter(|l| l["genre"] == genre).collect()
}

fn presents(base: &mut Base, employe: &str, du: u32, au: u32, mois: &str) {
    for j in du..=au {
        presences::marquer_sur(base, employe.into(), format!("{mois}-{j:02}"), Some("present".into())).unwrap();
    }
}

/// Chaque ecriture du journal PA equilibree ; rend le solde (credit -
/// debit) d'un compte.
fn journal_pa(base: &mut Base) -> impl Fn(&str) -> i64 {
    let aujourd_hui = chrono::Local::now().date_naive();
    let (du, au) = ((aujourd_hui - chrono::Duration::days(1)).to_string(), (aujourd_hui + chrono::Duration::days(1)).to_string());
    let pa = journaux_comptables::ecritures_sur(base, &du, &au, Some("PA"), true).unwrap();
    for e in &pa {
        let d: i64 = e.lignes.iter().map(|l| l.debit).sum();
        let c: i64 = e.lignes.iter().map(|l| l.credit).sum();
        assert_eq!(d, c, "{} {}", e.piece, e.libelle);
    }
    move |compte: &str| pa.iter().flat_map(|e| e.lignes.iter()).filter(|l| l.compte == compte).map(|l| l.credit - l.debit).sum()
}

#[test]
fn un_journalier_a_la_quinzaine_ne_rembourse_l_avance_qu_une_fois_par_mois() {
    let mut base = base_avec_demo();
    let moussa = creer(&mut base, json!({ "nom": "Moussa", "fonction": "manœuvre", "tarif_journalier": 2500 }));
    let s = Saisie { mensualite: Some(6_000), ..Saisie::default() };
    let a = avances::donner_avance_sur(&mut base, moussa.clone(), 20_000, s, true).unwrap();
    let dossier = base.dossier().to_string();
    base.executer("UPDATE avance SET date_avance = '2026-08-20' WHERE id = ?1 AND dossier_id = ?2", &parametres![id(&a), dossier]).unwrap();
    presents(&mut base, &moussa, 1, 30, "2026-09");
    presents(&mut base, &moussa, 1, 31, "2026-10");

    // Du 1er au 15 : la mensualite du mois.
    let q1 = preparer(&mut base, &moussa, "2026-09-01", "2026-09-15");
    assert_eq!(lignes(&q1, "avance")[0]["montant"], -6_000);
    assert_eq!(lignes(&q1, "avance")[0]["libelle"], "Avance du 20/08/2026 (20 000 F ; 6 000 F par mois ; 14 000 F reportés)");
    let q1 = valider(&mut base, &q1);
    assert_eq!(q1["net"], 15 * 2_500 - 6_000);
    assert_eq!(avances::en_cours_sur(&mut base, &moussa).unwrap(), 14_000);

    // Du 16 au 30 : le mois a deja rembourse, rien de plus.
    let q2 = preparer(&mut base, &moussa, "2026-09-16", "2026-09-30");
    assert!(lignes(&q2, "avance").is_empty(), "une seule retenue par mois : {q2}");
    assert_eq!(q2["reporte"], 14_000);
    let q2 = valider(&mut base, &q2);
    assert_eq!(q2["net"], 15 * 2_500);
    assert_eq!(avances::en_cours_sur(&mut base, &moussa).unwrap(), 14_000, "rien retenu");

    // Une rectificative de la premiere quinzaine retient de nouveau la
    // mensualite : elle ne se compte pas elle-meme dans le mois.
    let r = paie::rectifier_sur(&mut base, id(&q1)).unwrap();
    assert_eq!(lignes(&r, "avance")[0]["montant"], -6_000);
    valider(&mut base, &r);
    assert_eq!(avances::en_cours_sur(&mut base, &moussa).unwrap(), 14_000, "rendu puis retenu");

    // Octobre : de nouveau une fois.
    let o1 = preparer_valider(&mut base, &moussa, "2026-10-01", "2026-10-15");
    assert_eq!(lignes(&o1, "avance")[0]["montant"], -6_000);
    let o2 = preparer(&mut base, &moussa, "2026-10-16", "2026-10-31");
    assert!(lignes(&o2, "avance").is_empty());
    assert_eq!(avances::en_cours_sur(&mut base, &moussa).unwrap(), 8_000);
}

#[test]
fn une_commission_versee_sur_une_vente_annulee_se_reprend_sur_la_fiche_suivante() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let (u, aminata) = vendeuse(&mut base, "Aminata", Some(40_000));
    let (m, vente, facture) = vendre_paye(&mut base, &u, 10.0, "2026-09-10");
    let commission = (m * 3 + 50) / 100;

    let sept = preparer_valider(&mut base, &aminata, "2026-09-01", "2026-09-30");
    assert_eq!(lignes(&sept, "commission")[0]["montant"], commission);
    paie::verser_sur(&mut base, id(&sept), sept["net"].as_i64().unwrap(), None).unwrap();

    annuler_le(&mut base, &vente, &facture, "2026-10-05");
    let fac = numero(&mut base, &facture);

    // La fiche de septembre, validee et payee, ne change pas.
    let relue = paie::lire_sur(&mut base, &id(&sept)).unwrap();
    assert_eq!(relue["lignes"], sept["lignes"]);
    assert_eq!((relue["net"].clone(), relue["reste"].clone()), (sept["net"].clone(), json!(0)));

    // Octobre : la commission versee en trop, retenue et dite avec la vente.
    let oct = preparer(&mut base, &aminata, "2026-10-01", "2026-10-31");
    let rep = lignes(&oct, "reprise_commission");
    assert_eq!(rep.len(), 1, "{oct}");
    assert_eq!((rep[0]["montant"].as_i64(), rep[0]["source"].as_str()), (Some(-commission), Some(vente.as_str())));
    let libelle = rep[0]["libelle"].as_str().unwrap();
    assert!(libelle.contains(&fac) && libelle.contains("du 10/09/2026") && libelle.contains("annulée le 05/10/2026"), "{libelle}");
    assert_eq!(lignes(&oct, "commission")[0]["montant"], 0, "rien encaissé en octobre");
    assert_eq!(oct["net"], 40_000 - commission);
    let oct = valider(&mut base, &oct);

    // Novembre : deja reprise, plus rien.
    let nov = preparer(&mut base, &aminata, "2026-11-01", "2026-11-30");
    assert!(lignes(&nov, "reprise_commission").is_empty(), "{nov}");

    // Une rectificative de septembre ouverte apres l'annulation garde la
    // commission (rien n'est reecrit) ; la reprise reste celle d'octobre.
    let r = paie::rectifier_sur(&mut base, id(&sept)).unwrap();
    assert_eq!(lignes(&r, "commission")[0]["montant"], commission);
    let r = valider(&mut base, &r);
    assert_eq!(avances::en_cours_sur(&mut base, &aminata).unwrap(), 0, "même net : pas de trop-versé");
    let nov = paie::recalculer_sur(&mut base, id(&nov), None).unwrap();
    assert!(lignes(&nov, "reprise_commission").is_empty(), "pas deux fois : {nov}");
    assert_eq!(paie::lire_sur(&mut base, &id(&oct)).unwrap()["lignes"], oct["lignes"]);

    // Le journal PA : equilibre ; les salaires (661) portent septembre et
    // octobre moins la reprise ; ce qui reste du (422) = ce qui reste a verser.
    let solde = journal_pa(&mut base);
    assert_eq!(-solde("661"), (40_000 + commission) + 40_000 - commission);
    let reste = paie::lire_sur(&mut base, &id(&r)).unwrap()["reste"].as_i64().unwrap()
        + paie::lire_sur(&mut base, &id(&oct)).unwrap()["reste"].as_i64().unwrap();
    assert_eq!(solde("422"), reste);
    assert_eq!(solde("758"), 0, "une reprise n'est pas une retenue saisie");
}

#[test]
fn une_reprise_plus_grande_que_le_net_se_reporte_sur_la_fiche_d_apres() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let (u, fanta) = vendeuse(&mut base, "Fanta", None);
    let (grosse, vente, facture) = vendre_paye(&mut base, &u, 20.0, "2026-09-10");
    let (petite, ..) = vendre_paye(&mut base, &u, 2.0, "2026-10-12");
    let c = |m: i64| (m * 3 + 50) / 100;
    preparer_valider(&mut base, &fanta, "2026-09-01", "2026-09-30");
    annuler_le(&mut base, &vente, &facture, "2026-10-05");

    // Octobre : la commission du mois ne suffit pas ; le net reste a zero.
    let oct = preparer(&mut base, &fanta, "2026-10-01", "2026-10-31");
    let rep = lignes(&oct, "reprise_commission");
    assert_eq!(rep[0]["montant"].as_i64(), Some(-c(petite)));
    let manque = c(grosse) - c(petite);
    assert!(rep[0]["libelle"].as_str().unwrap().contains("reportés"), "{}", rep[0]["libelle"]);
    assert_eq!((oct["net"].as_i64(), oct["reporte"].as_i64()), (Some(0), Some(manque)));
    valider(&mut base, &oct);

    // Novembre : le reste, et le libelle dit ce qui a deja ete repris.
    vendre_paye(&mut base, &u, 30.0, "2026-11-03");
    let nov = preparer(&mut base, &fanta, "2026-11-01", "2026-11-30");
    let rep = lignes(&nov, "reprise_commission");
    assert_eq!(rep[0]["montant"].as_i64(), Some(-manque));
    assert!(rep[0]["libelle"].as_str().unwrap().contains("déjà repris"), "{}", rep[0]["libelle"]);
    valider(&mut base, &nov);
    let dec = preparer(&mut base, &fanta, "2026-12-01", "2026-12-31");
    assert!(lignes(&dec, "reprise_commission").is_empty());
    let _ = journal_pa(&mut base);
}

#[test]
fn tout_ce_qui_est_porte_ici_passe_le_detecteur() {
    let mut base = base_avec_demo();
    base.auditer(true);
    ouvrir_caisse(&mut base);
    let (u, aminata) = vendeuse(&mut base, "Aminata", Some(40_000));
    let s = Saisie { mensualite: Some(5_000), ..Saisie::default() };
    let a = avances::donner_avance_sur(&mut base, aminata.clone(), 15_000, s, true).unwrap();
    let dossier = base.dossier().to_string();
    base.executer("UPDATE avance SET date_avance = '2026-08-20' WHERE id = ?1 AND dossier_id = ?2", &parametres![id(&a), dossier]).unwrap();
    let (_, vente, facture) = vendre_paye(&mut base, &u, 10.0, "2026-09-10");
    preparer_valider(&mut base, &aminata, "2026-09-01", "2026-09-30");
    annuler_le(&mut base, &vente, &facture, "2026-10-05");
    let oct = preparer_valider(&mut base, &aminata, "2026-10-01", "2026-10-31");
    assert_eq!(lignes(&oct, "reprise_commission").len(), 1);
    assert_eq!(lignes(&oct, "avance")[0]["montant"], -5_000);
    let _ = journal_pa(&mut base);
}
