//! `dossiers::avis_fin_exercice_sur` — prevenir avant la fin des dates
//! de travail (trouve le 27/10/2026). Le 1er janvier 2027, si personne
//! n'a prolonge, toute ecriture datee est refusee (D21) : le tableau de
//! bord le dit un mois avant, rougit la derniere semaine, et le jour
//! venu rend le refus mot pour mot. Teste sur SQLite par defaut, sur
//! PostgreSQL avec `GESCOM_PG`.
//!
//! Les dossiers ont des dates DONNEES : le scenario ne depend pas de
//! l'annee ou il tourne.

mod commun;

use commun::*;
use gescom_noyau::base::Base;
use gescom_noyau::{dossiers, parametres};

fn dossier(base: &mut Base, code: &str, du: &str, au: &str) -> String {
    let r = dossiers::creer_dossier_sur(base, code.into(), format!("Boutique {code}"), Some(du.into()), Some(au.into()))
        .expect("dossier créé");
    let id = r["id"].as_str().unwrap().to_string();
    base.choisir_dossier(&id).unwrap();
    id
}

fn avis(base: &mut Base, aujourd_hui: &str) -> serde_json::Value {
    dossiers::avis_fin_exercice_sur(base, aujourd_hui).expect("avis")
}

fn premier_exercice(base: &mut Base) -> String {
    dossiers::lire_exercices_sur(base).unwrap()[0]["id"].as_str().unwrap().to_string()
}

/// Ce qu'un dossier a ecrit : l'avis n'en ajoute rien.
fn traces(base: &mut Base, dossier: &str) -> (i64, i64) {
    (
        compter(
            base,
            "SELECT CAST(COUNT(*) AS BIGINT) FROM journal WHERE dossier_id = ?1",
            &parametres![dossier],
        ),
        compter(
            base,
            "SELECT CAST(COUNT(*) AS BIGINT) FROM exercice WHERE dossier_id = ?1 AND modifie_le = cree_le",
            &parametres![dossier],
        ),
    )
}

#[test]
fn un_mois_avant_la_fin_le_tableau_de_bord_previent_puis_rougit() {
    let mut base = base_avec_demo();
    let bout = dossier(&mut base, "BOUT", "2026-01-01", "2026-12-31");
    let avant = traces(&mut base, &bout);

    let a = avis(&mut base, "2026-10-27T09:30:00.000");
    assert_eq!(a["aujourd_hui"], "2026-10-27");
    assert_eq!(a["fin"], "2026-12-31");
    assert_eq!(a["jours_restants"], 65);
    assert_eq!(a["avis"], "rien", "deux mois avant : rien à dire");
    assert_eq!(a["preavis_jours"], 30);
    assert!(a["refus"].is_null());

    let a = avis(&mut base, "2026-12-01");
    assert_eq!((a["avis"].as_str(), a["jours_restants"].as_i64()), (Some("proche"), Some(30)));
    let a = avis(&mut base, "2026-12-24");
    assert_eq!((a["avis"].as_str(), a["jours_restants"].as_i64()), (Some("urgent"), Some(7)));
    let a = avis(&mut base, "2026-12-31T18:00:00");
    assert_eq!(
        (a["avis"].as_str(), a["jours_restants"].as_i64()),
        (Some("urgent"), Some(0)),
        "le dernier jour, on vend encore"
    );

    // Le 2 janvier au matin : l'avis rend le refus du serveur, mot pour mot.
    let a = avis(&mut base, "2027-01-02T08:00:00");
    assert_eq!(a["avis"], "bloque");
    assert!(a["fin"].is_null() && a["jours_restants"].is_null());
    let refus = dossiers::verifier_date_sur(&mut base, "2027-01-02").unwrap_err();
    assert_eq!(a["refus"], refus);
    assert_eq!(
        refus,
        "Le 2 janvier 2027 est hors des dates de travail (jusqu'au 31 décembre 2026). \
         Prolonger l'exercice ou en ouvrir un nouveau."
    );

    assert_eq!(traces(&mut base, &bout), avant, "lire l'avis n'écrit rien");
}

/// Le patron fait le geste : l'avis se tait. Une prolongation repousse
/// la fin ; un exercice suivant deja ouvert aussi — sans quoi l'avis
/// crierait en decembre alors que janvier est deja pret.
#[test]
fn prolonger_ou_ouvrir_la_suite_fait_taire_l_avis() {
    let mut base = base_avec_demo();
    dossier(&mut base, "BOUT", "2026-01-01", "2026-12-31");
    let ex2026 = premier_exercice(&mut base);
    assert_eq!(avis(&mut base, "2026-12-28")["avis"], "urgent");

    dossiers::prolonger_exercice_sur(&mut base, ex2026.clone(), "2027-03-31".into()).expect("prolonger");
    let a = avis(&mut base, "2026-12-28");
    assert_eq!(a["fin"], "2027-03-31", "la prolongation compte");
    assert_eq!((a["avis"].as_str(), a["jours_restants"].as_i64()), (Some("rien"), Some(93)));
    assert_eq!(avis(&mut base, "2027-03-20")["avis"], "proche", "mais elle finit aussi");

    dossiers::ouvrir_exercice_sur(&mut base, "2027-04-01".into(), "2028-03-31".into()).expect("le suivant");
    let a = avis(&mut base, "2027-03-20");
    assert_eq!(a["fin"], "2028-03-31", "le suivant est déjà ouvert : on écrit sans interruption");
    assert_eq!(a["avis"], "rien");

    // On ne clot pas l'exercice ou l'on est encore (D38) : il arreterait
    // toutes les ventes. Une fois termine, si — et le suivant s'ecrit.
    assert!(dossiers::clore_exercice_au_sur(&mut base, ex2026.clone(), "2027-03-20")
        .unwrap_err()
        .contains("qu'un exercice terminé"));
    dossiers::clore_exercice_au_sur(&mut base, ex2026, "2027-04-02").expect("clore");
    assert!(dossiers::verifier_date_sur(&mut base, "2027-03-20").unwrap_err().contains("closes"));
    let a = avis(&mut base, "2027-04-02");
    assert_eq!((a["fin"].as_str(), a["avis"].as_str()), (Some("2028-03-31"), Some("rien")), "le suivant, lui, s'écrit");
}

/// Une boutique qui ne veut pas y repenser prolonge de trois ans, ou
/// ouvre un suivant de trois ans : c'est permis. Une faute d'annee
/// (au-dela de cinq ans) est refusee — une prolongation ne se
/// raccourcit pas — et le refus n'ecrit rien.
#[test]
fn prolonger_de_plus_de_deux_ans_passe_une_faute_d_annee_non() {
    let mut base = base_avec_demo();
    let bout = dossier(&mut base, "BOUT", "2026-01-01", "2026-12-31");
    let ex = premier_exercice(&mut base);

    dossiers::prolonger_exercice_sur(&mut base, ex.clone(), "2029-12-31".into()).expect("trois ans de plus");
    let a = avis(&mut base, "2026-12-28");
    assert_eq!((a["fin"].as_str(), a["avis"].as_str()), (Some("2029-12-31"), Some("rien")));
    assert!(dossiers::verifier_date_sur(&mut base, "2029-06-01").is_ok());
    dossiers::prolonger_exercice_sur(&mut base, ex.clone(), "2031-12-31".into()).expect("cinq ans après la fin prévue, pile");

    assert_eq!(
        dossiers::prolonger_exercice_sur(&mut base, ex.clone(), "2032-01-01".into()).unwrap_err(),
        "Une prolongation va au plus 5 ans après la fin prévue : jusqu'au 31 décembre 2031. Vérifier l'année."
    );
    assert!(dossiers::prolonger_exercice_sur(&mut base, ex, "2062-12-31".into()).is_err(), "2062 pour 2026");
    let lus = dossiers::lire_exercices_sur(&mut base).unwrap();
    assert_eq!((lus[0]["date_fin"].as_str(), lus[0]["fin_effective"].as_str()), (Some("2026-12-31"), Some("2031-12-31")));
    let prolongations = compter(
        &mut base,
        "SELECT CAST(COUNT(*) AS BIGINT) FROM journal WHERE type_evenement = 'exercice_prolonge' AND dossier_id = ?1",
        &parametres![bout],
    );
    assert_eq!(prolongations, 2, "les refus n'ont rien écrit");

    // Le suivant de trois ans, oui ; de quarante-six, non.
    dossier(&mut base, "NEUF", "2026-01-01", "2026-12-31");
    assert!(dossiers::ouvrir_exercice_sur(&mut base, "2027-01-01".into(), "2072-12-31".into())
        .unwrap_err()
        .contains("plus de 5 ans"));
    dossiers::ouvrir_exercice_sur(&mut base, "2027-01-01".into(), "2029-12-31".into()).expect("trois ans");
    assert_eq!(avis(&mut base, "2026-12-28")["fin"], "2029-12-31");
}

/// Un dossier dont les dates commencent plus tard : aujourd'hui ne
/// s'ecrit pas encore, l'avis le dit avec la borne.
#[test]
fn un_dossier_sans_exercice_qui_couvre_aujourd_hui_est_bloque() {
    let mut base = base_avec_demo();
    dossier(&mut base, "NEUF", "2027-01-01", "2027-12-31");
    let a = avis(&mut base, "2026-12-15");
    assert_eq!(a["avis"], "bloque", "avant le premier jour");
    assert!(a["refus"].as_str().unwrap().contains("1er janvier 2027"), "{a}");
}

/// Chaque dossier a ses dates : l'avis de l'un ne dit rien de l'autre.
#[test]
fn chaque_dossier_a_son_avis() {
    let mut base = base_avec_demo();
    let quinc = dossier(&mut base, "QUINC", "2026-03-01", "2027-02-28");
    let bout = dossier(&mut base, "BOUT", "2026-01-01", "2026-12-31");

    base.choisir_dossier(&bout).unwrap();
    let a = avis(&mut base, "2026-12-15");
    assert_eq!((a["fin"].as_str(), a["avis"].as_str()), (Some("2026-12-31"), Some("proche")));
    base.choisir_dossier(&quinc).unwrap();
    let a = avis(&mut base, "2026-12-15");
    assert_eq!((a["fin"].as_str(), a["avis"].as_str()), (Some("2027-02-28"), Some("rien")));

    // Prolonger la quincaillerie ne change rien a la boutique.
    let ex = premier_exercice(&mut base);
    dossiers::prolonger_exercice_sur(&mut base, ex, "2027-06-30".into()).expect("prolonger");
    base.choisir_dossier(&bout).unwrap();
    assert_eq!(avis(&mut base, "2026-12-15")["fin"], "2026-12-31");
}

/// Sur la vraie horloge, comme le serveur l'appelle : le dossier
/// d'origine a l'exercice de l'annee en cours.
#[test]
fn sur_l_horloge_du_jour_le_dossier_d_origine_finit_le_31_decembre() {
    let mut base = base_avec_demo();
    let maintenant = gescom_noyau::utils::maintenant_iso();
    let a = avis(&mut base, &maintenant);
    assert_eq!(a["aujourd_hui"], &maintenant[..10]);
    assert_eq!(a["fin"], format!("{}-12-31", &maintenant[..4]));
    let attendu = gescom_noyau::coeur::fin_exercice::jours_restants(
        &format!("{}-12-31", &maintenant[..4]),
        gescom_noyau::coeur::dates::jour(&maintenant).unwrap(),
    );
    assert_eq!(a["jours_restants"].as_i64(), attendu);
}

#[test]
fn tout_ce_que_lit_l_avis_passe_le_detecteur() {
    let mut base = base_avec_demo();
    base.auditer(true);
    avis(&mut base, &gescom_noyau::utils::maintenant_iso());
    dossier(&mut base, "AUDIT", "2026-01-01", "2026-12-31");
    let ex = premier_exercice(&mut base);
    avis(&mut base, "2026-12-20");
    dossiers::prolonger_exercice_sur(&mut base, ex, "2027-01-31".into()).expect("prolonger");
    dossiers::ouvrir_exercice_sur(&mut base, "2027-02-01".into(), "2028-01-31".into()).expect("ouvrir");
    avis(&mut base, "2027-01-15");
    avis(&mut base, "2029-01-01");
}
