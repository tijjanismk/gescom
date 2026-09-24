//! G-2 — la fiche de paie (PLAN-EQUIPE, D31), sur les deux moteurs et
//! sous le compte limite. Les trois personnes de F-2 : Awa au mois sans
//! contrat, Moussa a la journee, Fanta au fixe + commission avec son
//! compte. La commission egale les ventes signees ; l'avance se retient
//! (et se reporte quand elle depasse) ; validee, plus rien ne change ;
//! une erreur se corrige par une rectificative.

mod commun;

use commun::*;
use gescom_noyau::argent::{self, ParamsLigneInput};
use gescom_noyau::base::Base;
use gescom_noyau::coeur::lecture;
use gescom_noyau::personnel::{self, Fiche};
use gescom_noyau::{auteur, auth, avances, dossiers, paie, parametres, presences};
use serde_json::{json, Value};

fn jour(decalage: i64) -> String {
    (chrono::Local::now().date_naive() + chrono::Duration::days(decalage)).format("%Y-%m-%d").to_string()
}

fn annee() -> String {
    chrono::Local::now().format("%Y").to_string()
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

struct Equipe {
    awa: String,
    moussa: String,
    fanta: String,
    compte_fanta: String,
}

fn equipe(base: &mut Base) -> Equipe {
    let admin = admin(base);
    let compte_fanta =
        auth::creer_utilisateur_sur_base(base, "Fanta".into(), "fanta".into(), None, "secret-123".into(), "caissier".into(), admin).unwrap();
    Equipe {
        awa: creer(base, json!({ "nom": "Awa", "fonction": "vendeuse", "salaire_mensuel": 60000 })),
        moussa: creer(base, json!({ "nom": "Moussa", "fonction": "manœuvre", "tarif_journalier": 2500 })),
        fanta: creer(base, json!({ "nom": "Fanta", "fonction": "vendeuse", "salaire_mensuel": 25000, "commission_pct": 3.0, "utilisateur_id": compte_fanta.clone() })),
        compte_fanta,
    }
}

/// Une vente a credit (pas de caisse) ; rend son montant.
fn vendre(base: &mut Base, quantite: f64) -> i64 {
    let depot = depot_defaut(base);
    let sucre = article_unite(base, "Sucre");
    let client = client_reel(base);
    argent::creer_vente_sur_base(
        base, client, depot.clone(), "credit".into(),
        vec![ParamsLigneInput {
            article_id: sucre.0.clone(), unite_vente_id: sucre.1.clone(), depot_source_id: depot,
            source_approvisionnement: "stock".into(), quantite, facteur: sucre.2,
            prix_reference: sucre.3, prix_pratique: sucre.3, taux_tva: None, a_decouvert: None,
        }],
        None, None, None, None,
    )
    .expect("vente");
    (sucre.3 as f64 * quantite).round() as i64
}

fn marquer(base: &mut Base, employe: &str, du: i64, n: i64, etat: &str) {
    for d in du..du + n {
        presences::marquer_sur(base, employe.into(), jour(d), Some(etat.into())).unwrap();
    }
}

fn ligne<'a>(f: &'a Value, genre: &str) -> Vec<&'a Value> {
    f["lignes"].as_array().unwrap().iter().filter(|l| l["genre"] == genre).collect()
}

fn retenu(base: &mut Base, avance_id: &str) -> i64 {
    let dossier = base.dossier().to_string();
    compter(base, "SELECT retenu FROM avance WHERE id = ?1 AND dossier_id = ?2", &parametres![avance_id, dossier])
}

#[test]
fn les_trois_personnes_de_f2() {
    let mut base = base_avec_demo();
    let e = equipe(&mut base);
    let (du, au) = (jour(-30), jour(0));
    marquer(&mut base, &e.moussa, -30, 22, "present");
    // Fanta signe deux ventes ; l'admin une, qui ne compte pas pour elle.
    let ventes_fanta = {
        let _g = auteur::poser(&e.compte_fanta);
        vendre(&mut base, 10.0) + vendre(&mut base, 5.0)
    };
    {
        let admin = admin(&mut base);
        let _g = auteur::poser(&admin);
        vendre(&mut base, 7.0);
    }
    ouvrir_caisse(&mut base);
    let avance = avances::donner_sur(&mut base, e.awa.clone(), 10_000, None, None).unwrap()["id"].as_str().unwrap().to_string();

    // Awa : le mois, moins l'avance.
    let awa = paie::preparer_sur(&mut base, e.awa.clone(), du.clone(), au.clone(), false).unwrap();
    assert_eq!(awa["statut"], "brouillon");
    assert_eq!((awa["brut"].as_i64(), awa["retenues"].as_i64(), awa["net"].as_i64()), (Some(60_000), Some(10_000), Some(50_000)));
    assert_eq!(ligne(&awa, "avance")[0]["source"], avance.as_str(), "la ligne dit quelle avance");
    assert_eq!(retenu(&mut base, &avance), 0, "un brouillon ne retient rien encore");

    // Moussa : 22 jours × 2 500.
    let moussa = paie::preparer_sur(&mut base, e.moussa.clone(), du.clone(), au.clone(), false).unwrap();
    assert_eq!(moussa["net"], 55_000);
    assert_eq!(ligne(&moussa, "jours")[0]["libelle"], "22 jours × 2 500 F");

    // Fanta : le fixe + 3 % de SES ventes signées.
    let fanta = paie::preparer_sur(&mut base, e.fanta.clone(), du.clone(), au.clone(), false).unwrap();
    let commission = ligne(&fanta, "commission")[0];
    assert_eq!(commission["prix"], ventes_fanta, "la commission se mesure sur les ventes signées par son compte");
    assert_eq!(commission["montant"], ((ventes_fanta as f64) * 0.03).round() as i64);
    assert_eq!(fanta["brut"], 25_000 + ((ventes_fanta as f64) * 0.03).round() as i64);

    // Awa : une prime, une casse ; puis au prorata (20 présents, 2 absents).
    paie::ajouter_ligne_sur(&mut base, awa["id"].as_str().unwrap().into(), "prime".into(), "Fin de mois".into(), None, None, Some(5_000)).unwrap();
    let awa = paie::ajouter_ligne_sur(&mut base, awa["id"].as_str().unwrap().into(), "retenue".into(), "Casse d'un carton".into(), None, None, Some(2_000)).unwrap();
    assert_eq!(awa["net"], 53_000, "60 000 + 5 000 - 2 000 - 10 000");
    marquer(&mut base, &e.awa, -30, 20, "present");
    marquer(&mut base, &e.awa, -10, 2, "absent");
    let awa_id = awa["id"].as_str().unwrap().to_string();
    let awa = paie::recalculer_sur(&mut base, awa_id.clone(), Some(true)).unwrap();
    assert_eq!(ligne(&awa, "base")[0]["montant"], 54_545, "60 000 × 20 / 22");
    assert_eq!(ligne(&awa, "prime").len(), 1, "les saisies restent");

    // Validée : un numéro, l'avance retenue, plus rien ne change.
    let v = paie::valider_sur(&mut base, awa_id.clone()).unwrap();
    assert_eq!(v["statut"], "validee");
    assert_eq!(v["numero"], format!("PAIE-{}-00001", annee()));
    assert_eq!(retenu(&mut base, &avance), 10_000);
    assert_eq!(avances::en_cours_sur(&mut base, &e.awa).unwrap(), 0);
    let net = v["net"].as_i64().unwrap();
    assert!(paie::ajouter_ligne_sur(&mut base, awa_id.clone(), "prime".into(), "x".into(), None, None, Some(1)).unwrap_err().contains("ne change plus"));
    assert!(paie::recalculer_sur(&mut base, awa_id.clone(), None).is_err());
    assert!(paie::supprimer_sur(&mut base, awa_id.clone()).is_err());
    assert!(paie::valider_sur(&mut base, awa_id.clone()).is_err());
    let f: Fiche = serde_json::from_value(json!({ "nom": "Awa", "fonction": "vendeuse", "salaire_mensuel": 90000 })).unwrap();
    personnel::modifier_sur(&mut base, e.awa.clone(), f).unwrap();
    assert_eq!(paie::lire_sur(&mut base, &awa_id).unwrap()["net"], net, "corriger son salaire ne réécrit pas ce qu'elle a touché");

    // Pas deux fiches pour la même période.
    let e2 = paie::preparer_sur(&mut base, e.awa.clone(), jour(-3), jour(0), false).unwrap_err();
    assert!(e2.contains("Awa a déjà une fiche") && e2.contains("PAIE-"), "{e2}");

    // La liste : trois fiches, personne à préparer.
    let l = paie::lister_sur(&mut base, du, au).unwrap();
    assert_eq!(l["fiches"].as_array().unwrap().len(), 3);
    assert!(l["a_preparer"].as_array().unwrap().is_empty());
    let n = compter(&mut base, "SELECT CAST(COUNT(*) AS BIGINT) FROM journal WHERE type_evenement = 'paie_validee' AND dossier_id = ?1", &parametres![dossiers::DOSSIER_DEFAUT]);
    assert_eq!(n, 1);
}

#[test]
fn on_ne_valide_que_ce_qui_est_a_jour() {
    let mut base = base_avec_demo();
    let e = equipe(&mut base);
    marquer(&mut base, &e.moussa, -10, 5, "present");
    let f = paie::preparer_sur(&mut base, e.moussa.clone(), jour(-10), jour(0), false).unwrap();
    let id = f["id"].as_str().unwrap().to_string();
    assert_eq!(f["net"], 12_500);
    // Un jour de plus marqué après le calcul : la validation le voit.
    marquer(&mut base, &e.moussa, -5, 1, "present");
    assert!(paie::valider_sur(&mut base, id.clone()).unwrap_err().contains("recalculer"));
    assert_eq!(paie::recalculer_sur(&mut base, id.clone(), None).unwrap()["net"], 15_000);
    // Une ligne calculée ne se retire pas ; une retenue trop grosse bloque.
    let jours_id = ligne(&paie::lire_sur(&mut base, &id).unwrap(), "jours")[0]["id"].as_str().unwrap().to_string();
    assert!(paie::retirer_ligne_sur(&mut base, id.clone(), jours_id).unwrap_err().contains("calculée"));
    let f = paie::ajouter_ligne_sur(&mut base, id.clone(), "retenue".into(), "Casse".into(), None, None, Some(20_000)).unwrap();
    assert!(paie::valider_sur(&mut base, id.clone()).unwrap_err().contains("dépassent"));
    let casse = ligne(&f, "retenue")[0]["id"].as_str().unwrap().to_string();
    let f = paie::retirer_ligne_sur(&mut base, id.clone(), casse).unwrap();
    assert_eq!(f["net"], 15_000);
    // À la tâche, saisi : 3 livraisons × 1 000.
    let f = paie::ajouter_ligne_sur(&mut base, id.clone(), "tache".into(), "Livraisons".into(), Some(3.0), Some(1_000), None).unwrap();
    assert_eq!(f["net"], 18_000);
    assert!(paie::valider_sur(&mut base, id.clone()).is_ok());
    // Un brouillon se jette, une période invalide se refuse.
    let b = paie::preparer_sur(&mut base, e.awa.clone(), jour(-10), jour(0), false).unwrap();
    paie::supprimer_sur(&mut base, b["id"].as_str().unwrap().into()).unwrap();
    assert!(paie::lire_sur(&mut base, b["id"].as_str().unwrap()).is_err());
    assert!(paie::preparer_sur(&mut base, e.awa.clone(), jour(0), jour(-1), false).is_err());
}

#[test]
fn l_avance_qui_depasse_se_reporte_sur_la_fiche_suivante() {
    let mut base = base_avec_demo();
    let e = equipe(&mut base);
    ouvrir_caisse(&mut base);
    marquer(&mut base, &e.moussa, -10, 5, "present");
    let a = avances::donner_sur(&mut base, e.moussa.clone(), 20_000, None, None).unwrap()["id"].as_str().unwrap().to_string();
    let f = paie::preparer_sur(&mut base, e.moussa.clone(), jour(-10), jour(0), false).unwrap();
    assert_eq!((f["brut"].as_i64(), f["net"].as_i64(), f["reporte"].as_i64()), (Some(12_500), Some(0), Some(7_500)));
    assert!(ligne(&f, "avance")[0]["libelle"].as_str().unwrap().contains("7 500 F reportés"));
    paie::valider_sur(&mut base, f["id"].as_str().unwrap().into()).unwrap();
    assert_eq!(retenu(&mut base, &a), 12_500);
    assert_eq!(avances::en_cours_sur(&mut base, &e.moussa).unwrap(), 7_500);
    // La fiche suivante reprend ce qui reste.
    let g = paie::preparer_sur(&mut base, e.moussa.clone(), jour(1), jour(30), false).unwrap();
    let g = paie::ajouter_ligne_sur(&mut base, g["id"].as_str().unwrap().into(), "prime".into(), "".into(), None, None, Some(10_000)).unwrap();
    assert_eq!((g["net"].as_i64(), g["reporte"].as_i64()), (Some(2_500), Some(0)));
    paie::valider_sur(&mut base, g["id"].as_str().unwrap().into()).unwrap();
    assert_eq!(retenu(&mut base, &a), 20_000);
    assert!(avances::lister_sur(&mut base, Some(e.moussa.clone()), true).unwrap().is_empty(), "l'avance est soldée");
}

#[test]
fn la_rectificative_remplace_sans_reecrire() {
    let mut base = base_avec_demo();
    let e = equipe(&mut base);
    ouvrir_caisse(&mut base);
    let a = avances::donner_sur(&mut base, e.awa.clone(), 10_000, None, None).unwrap()["id"].as_str().unwrap().to_string();
    let f = paie::preparer_sur(&mut base, e.awa.clone(), jour(-30), jour(0), false).unwrap();
    let id = f["id"].as_str().unwrap().to_string();
    paie::ajouter_ligne_sur(&mut base, id.clone(), "prime".into(), "Fin de mois".into(), None, None, Some(5_000)).unwrap();
    assert!(paie::rectifier_sur(&mut base, id.clone()).unwrap_err().contains("brouillon"));
    let v = paie::valider_sur(&mut base, id.clone()).unwrap();
    assert_eq!(v["net"], 55_000);

    // On a oublié une casse : une rectificative, mêmes saisies, même avance.
    let r = paie::rectifier_sur(&mut base, id.clone()).unwrap();
    let rid = r["id"].as_str().unwrap().to_string();
    assert_eq!(r["rectifie_numero"], v["numero"]);
    assert_eq!(ligne(&r, "prime").len(), 1);
    assert_eq!(ligne(&r, "avance")[0]["montant"], -10_000, "ce que l'ancienne a retenu reste à retenir");
    assert!(paie::rectifier_sur(&mut base, id.clone()).unwrap_err().contains("déjà une fiche"), "une rectificative à la fois");
    let r = paie::ajouter_ligne_sur(&mut base, rid.clone(), "retenue".into(), "Casse".into(), None, None, Some(3_000)).unwrap();
    assert_eq!(r["net"], 52_000);
    // Tant qu'elle n'est pas validée, l'ancienne vaut.
    assert_eq!(paie::lire_sur(&mut base, &id).unwrap()["statut"], "validee");
    let r = paie::valider_sur(&mut base, rid.clone()).unwrap();
    assert_eq!(r["numero"], format!("PAIE-{}-00002", annee()));
    let ancienne = paie::lire_sur(&mut base, &id).unwrap();
    assert_eq!(ancienne["statut"], "remplacee");
    assert_eq!(ancienne["remplacee_par"], r["numero"]);
    assert_eq!(ancienne["net"], 55_000, "l'ancienne n'est pas réécrite");
    assert_eq!(retenu(&mut base, &a), 10_000, "l'avance n'est pas retenue deux fois");
    assert!(paie::rectifier_sur(&mut base, id).unwrap_err().contains("remplacée"));
    // On peut repréparer ? Non : la rectificative couvre la période.
    assert!(paie::preparer_sur(&mut base, e.awa.clone(), jour(-5), jour(0), false).is_err());
}

#[test]
fn chaque_dossier_sa_paie_et_ses_numeros() {
    let mut base = base_avec_demo();
    let e = equipe(&mut base);
    let f = paie::preparer_sur(&mut base, e.awa.clone(), jour(-30), jour(0), false).unwrap();
    paie::valider_sur(&mut base, f["id"].as_str().unwrap().into()).unwrap();
    let b = dossiers::creer_dossier_sur(&mut base, "B".into(), "Boutique B".into(), None, None).unwrap()["id"].as_str().unwrap().to_string();
    base.choisir_dossier(&b).unwrap();
    assert!(paie::lire_sur(&mut base, f["id"].as_str().unwrap()).is_err());
    assert!(paie::preparer_sur(&mut base, e.awa.clone(), jour(-30), jour(0), false).unwrap_err().contains("introuvable"));
    let l = paie::lister_sur(&mut base, jour(-30), jour(0)).unwrap();
    assert!(l["fiches"].as_array().unwrap().is_empty());
    let issa = creer(&mut base, json!({ "nom": "Issa", "fonction": "gardien", "salaire_mensuel": 40000 }));
    let g = paie::preparer_sur(&mut base, issa, jour(-30), jour(0), false).unwrap();
    let g = paie::valider_sur(&mut base, g["id"].as_str().unwrap().into()).unwrap();
    assert_eq!(g["numero"], format!("PAIE-{}-00001", annee()), "chaque dossier numérote ses fiches");
}

#[test]
fn ne_lit_la_paie_que_qui_la_prepare_ou_la_valide() {
    for cmd in ["lire_fiches_paie", "lire_fiche_paie"] {
        assert!(lecture::refus(cmd, |_| false).is_some());
        assert!(lecture::refus(cmd, |p| p == "personnel:gerer").is_some(), "gérer le personnel ne suffit pas");
        assert_eq!(lecture::refus(cmd, |p| p == "paie:preparer"), None);
        assert_eq!(lecture::refus(cmd, |p| p == "paie:valider"), None);
    }
}

#[test]
fn tout_ce_qui_est_porte_ici_passe_le_detecteur() {
    let mut base = base_avec_demo();
    base.auditer(true);
    let e = equipe(&mut base);
    ouvrir_caisse(&mut base);
    marquer(&mut base, &e.moussa, -3, 2, "present");
    {
        let _g = auteur::poser(&e.compte_fanta);
        vendre(&mut base, 2.0);
    }
    avances::donner_sur(&mut base, e.awa.clone(), 1_000, None, None).unwrap();
    for p in [&e.awa, &e.moussa, &e.fanta] {
        let f = paie::preparer_sur(&mut base, p.clone(), jour(-3), jour(0), true).unwrap();
        let id = f["id"].as_str().unwrap().to_string();
        let f = paie::ajouter_ligne_sur(&mut base, id.clone(), "prime".into(), "x".into(), None, None, Some(100)).unwrap();
        let prime = ligne(&f, "prime")[0]["id"].as_str().unwrap().to_string();
        paie::retirer_ligne_sur(&mut base, id.clone(), prime).unwrap();
        paie::recalculer_sur(&mut base, id.clone(), Some(false)).unwrap();
        paie::valider_sur(&mut base, id.clone()).unwrap();
        let r = paie::rectifier_sur(&mut base, id).unwrap();
        paie::valider_sur(&mut base, r["id"].as_str().unwrap().into()).unwrap();
    }
    let b = paie::preparer_sur(&mut base, e.awa.clone(), jour(1), jour(2), false).unwrap();
    paie::supprimer_sur(&mut base, b["id"].as_str().unwrap().into()).unwrap();
    paie::lister_sur(&mut base, jour(-3), jour(0)).unwrap();
}
