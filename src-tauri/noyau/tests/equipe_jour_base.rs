//! EQ-3 — l'accueil du jour d'Equipe (PLAN-EQUIPE § 5, D49), sur les
//! deux moteurs et sous le compte limite. Ce qui attend la personne
//! connectee : ses rappels dus, les brouillons, les fiches a solder, les
//! avances en cours — chaque bloc selon ses droits, les montants pour qui
//! voit la paie seulement.

mod commun;

use commun::*;
use gescom_noyau::base::Base;
use gescom_noyau::coeur::lecture::{self, AccueilEquipe};
use gescom_noyau::personnel::{self, Fiche};
use gescom_noyau::{auth, avances, crm, dossiers, equipe_jour, paie, parametres, portes, prets};
use serde_json::{json, Value};

fn jour(decalage: i64) -> String {
    (chrono::Local::now().date_naive() + chrono::Duration::days(decalage)).format("%Y-%m-%d").to_string()
}

/// Le mois civil courant `(1er, dernier jour)`.
fn ce_mois() -> (String, String) {
    use chrono::{Datelike, Months};
    let premier = chrono::Local::now().date_naive().with_day(1).unwrap();
    let fin = premier.checked_add_months(Months::new(1)).unwrap().pred_opt().unwrap();
    (premier.format("%Y-%m-%d").to_string(), fin.format("%Y-%m-%d").to_string())
}

fn admin(base: &mut Base) -> String {
    base.lire_une("SELECT utilisateur_id FROM utilisateur_auth WHERE pseudo = 'admin'", &[], |r| r.get::<String>(0))
        .unwrap()
        .unwrap()
}

/// Les droits de l'accueil d'un compte, par le meme chemin que le
/// serveur (`portes::permissions_de_sur` puis `lecture::accueil_equipe`).
fn droits_de(base: &mut Base, utilisateur_id: &str) -> AccueilEquipe {
    let role = base
        .lire_une(
            "SELECT r.nom FROM utilisateur u JOIN role r ON r.id = u.role_id WHERE u.id = ?1",
            &parametres![utilisateur_id],
            |r| r.get::<String>(0),
        )
        .unwrap()
        .unwrap();
    let perms = portes::permissions_de_sur(base, utilisateur_id, &role);
    lecture::accueil_equipe(|p| perms.contains(p))
}

fn accueil(base: &mut Base, droits: AccueilEquipe, moi: &str) -> Value {
    equipe_jour::accueil_sur(base, droits, moi, &gescom_noyau::utils::maintenant_iso()).unwrap()
}

fn creer(base: &mut Base, v: Value) -> String {
    let f: Fiche = serde_json::from_value(v).unwrap();
    personnel::creer_sur(base, f).unwrap()["id"].as_str().unwrap().to_string()
}

/// Les montants qu'une reponse porte, a n'importe quelle profondeur.
fn montants(v: &Value) -> Vec<String> {
    let mut trouves = Vec::new();
    fn fouiller(v: &Value, chemin: &str, t: &mut Vec<String>) {
        if let Some(o) = v.as_object() {
            for (k, x) in o {
                let c = format!("{chemin}/{k}");
                if ["net", "reste", "montant"].contains(&k.as_str()) && !x.is_null() {
                    t.push(c.clone());
                }
                fouiller(x, &c, t);
            }
        }
    }
    fouiller(v, "", &mut trouves);
    trouves
}

/// La journee d'une boutique : trois rappels pour le patron (hier,
/// aujourd'hui, demain), un pour le caissier ; Awa validee et payee a
/// moitie, Moussa en brouillon, une avance de 10 000 F a Moussa.
struct Journee {
    patron: String,
    caissier: String,
    rappel_du_jour: String,
}

fn journee(base: &mut Base) -> Journee {
    let patron = admin(base);
    let caissier = auth::creer_utilisateur_sur_base(
        base, "Seydou".into(), "seydou".into(), None, "secret-123".into(), "caissier".into(), patron.clone(),
    )
    .unwrap();
    let client = client_reel(base);
    crm::creer_rappel_sur(base, client.clone(), patron.clone(), jour(-1), "Relancer la facture".into()).unwrap();
    let du_jour = crm::creer_rappel_sur(base, client.clone(), patron.clone(), jour(0), "Proposer le riz".into()).unwrap();
    crm::creer_rappel_sur(base, client.clone(), patron.clone(), jour(1), "Plus tard".into()).unwrap();
    crm::creer_rappel_sur(base, client, caissier.clone(), jour(0), "Appeler Awa".into()).unwrap();

    let awa = creer(base, json!({ "nom": "Awa", "fonction": "vendeuse", "salaire_mensuel": 60000 }));
    let moussa = creer(base, json!({ "nom": "Moussa", "fonction": "manœuvre", "tarif_journalier": 2500 }));
    let (du, au) = ce_mois();
    let f = paie::preparer_sur(base, awa, du.clone(), au.clone(), false).unwrap();
    let f = f["id"].as_str().unwrap().to_string();
    paie::valider_sur(base, f.clone()).unwrap();
    paie::verser_sur(base, f, 25_000, None).unwrap();
    avances::donner_sur(base, moussa.clone(), 10_000, None, None).unwrap();
    paie::preparer_sur(base, moussa, du, au, false).unwrap();

    Journee { patron, caissier, rappel_du_jour: du_jour["id"].as_str().unwrap().to_string() }
}

#[test]
fn le_patron_voit_tout() {
    let mut base = base_avec_demo();
    let j = journee(&mut base);
    let d = droits_de(&mut base, &j.patron);
    assert_eq!(d, AccueilEquipe { rappels: true, fiches: true, avances: true, prets: true, montants: true });
    let a = accueil(&mut base, d, &j.patron);

    // « Du » = aujourd'hui et en retard ; celui de demain attend.
    assert_eq!(a["rappels"], json!({ "nombre": 2, "en_retard": 1 }), "{a}");
    // Awa : 60 000 validee, 25 000 versee.
    assert_eq!(a["a_solder"]["nombre"], 1);
    assert_eq!(a["a_solder"]["reste"], 35_000);
    assert_eq!(a["a_solder"]["plus_ancien"], ce_mois().0);
    // Moussa en brouillon : son net tient compte de l'avance retenue.
    assert_eq!(a["brouillons"]["nombre"], 1);
    assert!(a["brouillons"]["net"].is_i64());
    // L'avance n'est retenue qu'a la validation : toujours en cours.
    assert_eq!(a["avances"], json!({ "nombre": 1, "personnes": 1, "reste": 10_000 }));
}

#[test]
fn un_caissier_ne_voit_aucun_montant() {
    let mut base = base_avec_demo();
    let j = journee(&mut base);
    let d = droits_de(&mut base, &j.caissier);
    assert_eq!(d, AccueilEquipe { rappels: true, fiches: false, avances: false, prets: false, montants: false });
    let a = accueil(&mut base, d, &j.caissier);
    // Ses rappels a lui, pas ceux du patron ; rien de la paie, pas meme un zero.
    assert_eq!(a["rappels"], json!({ "nombre": 1, "en_retard": 0 }));
    for bloc in ["brouillons", "a_solder", "avances", "prets"] {
        assert!(a.get(bloc).is_none(), "{bloc} ne doit pas exister pour un caissier : {a}");
    }
    assert!(montants(&a).is_empty(), "{:?}", montants(&a));
}

#[test]
fn qui_donne_les_avances_les_compte_sans_les_chiffrer() {
    let mut base = base_avec_demo();
    let j = journee(&mut base);
    let d = lecture::accueil_equipe(|p| p == "personnel:avancer");
    let a = accueil(&mut base, d, &j.caissier);
    assert_eq!(a["avances"], json!({ "nombre": 1, "personnes": 1, "reste": null }));
    assert!(a.get("rappels").is_none() && a.get("brouillons").is_none());
    assert!(montants(&a).is_empty());

    // Le comptable (`paie:preparer`) voit les fiches et leurs montants.
    let d = lecture::accueil_equipe(|p| p == "paie:preparer");
    assert_eq!(d, AccueilEquipe { rappels: false, fiches: true, avances: true, prets: true, montants: true });
    let a = accueil(&mut base, d, &j.caissier);
    assert_eq!(a["a_solder"]["reste"], 35_000);
}

#[test]
fn un_pret_se_compte_a_part_des_avances() {
    // D47 apres fusion : le pret vit dans la table `avance` (nature
    // `pret`) ; l'accueil ne le compte pas parmi les avances, il a son
    // bloc, pour qui lit la paie seulement.
    let mut base = base_avec_demo();
    let j = journee(&mut base);
    let awa = base
        .lire_une("SELECT id FROM employe WHERE nom = 'Awa'", &[], |r| r.get::<String>(0))
        .unwrap()
        .unwrap();
    let s = prets::Saisie { mensualite: Some(10_000), ..prets::Saisie::default() };
    prets::preter_sur(&mut base, awa, 60_000, s).unwrap();

    let d = droits_de(&mut base, &j.patron);
    let a = accueil(&mut base, d, &j.patron);
    assert_eq!(a["avances"], json!({ "nombre": 1, "personnes": 1, "reste": 10_000 }), "le pret n'est pas une avance : {a}");
    assert_eq!(a["prets"], json!({ "nombre": 1, "personnes": 1, "reste": 60_000 }));

    // `personnel:avancer` seul : les avances, pas les prets.
    let a = accueil(&mut base, lecture::accueil_equipe(|p| p == "personnel:avancer"), &j.caissier);
    assert!(a.get("prets").is_none(), "{a}");
}

#[test]
fn un_rappel_fait_disparait() {
    let mut base = base_avec_demo();
    let j = journee(&mut base);
    let d = droits_de(&mut base, &j.patron);
    crm::marquer_rappel_fait_sur(&mut base, j.rappel_du_jour).unwrap();
    assert_eq!(accueil(&mut base, d, &j.patron)["rappels"], json!({ "nombre": 1, "en_retard": 1 }));
}

#[test]
fn une_fiche_soldee_ou_validee_sort_de_l_accueil() {
    let mut base = base_avec_demo();
    let j = journee(&mut base);
    let d = droits_de(&mut base, &j.patron);
    let (du, au) = ce_mois();
    let fiches = paie::lister_sur(&mut base, du, au).unwrap();
    for f in fiches["fiches"].as_array().unwrap() {
        let id = f["id"].as_str().unwrap().to_string();
        if f["statut"] == "brouillon" {
            paie::valider_sur(&mut base, id.clone()).unwrap();
        }
        let reste = paie::lire_sur(&mut base, &id).unwrap()["reste"].as_i64().unwrap();
        if reste > 0 {
            paie::verser_sur(&mut base, id, reste, None).unwrap();
        }
    }
    let a = accueil(&mut base, d, &j.patron);
    assert_eq!(a["brouillons"], json!({ "nombre": 0, "net": 0, "plus_ancien": null }));
    assert_eq!(a["a_solder"], json!({ "nombre": 0, "reste": 0, "plus_ancien": null }));
    // Les avances : ce que la liste de Paie → Avances dit en cours
    // (validee, la fiche de Moussa en a retenu ce que son net permet).
    let en_cours = avances::lister_sur(&mut base, None, true).unwrap();
    assert_eq!(a["avances"]["nombre"], en_cours.len());
    assert_eq!(a["avances"]["reste"], en_cours.iter().map(|x| x["reste"].as_i64().unwrap()).sum::<i64>());
}

#[test]
fn chaque_dossier_son_accueil() {
    let mut base = base_avec_demo();
    let j = journee(&mut base);
    let d = droits_de(&mut base, &j.patron);
    let b = dossiers::creer_dossier_sur(&mut base, "B".into(), "Boutique B".into(), None, None).unwrap()["id"].as_str().unwrap().to_string();
    base.choisir_dossier(&b).unwrap();
    let a = accueil(&mut base, d, &j.patron);
    assert_eq!(a["rappels"], json!({ "nombre": 0, "en_retard": 0 }));
    assert_eq!(a["brouillons"]["nombre"], 0);
    assert_eq!(a["a_solder"]["nombre"], 0);
    assert_eq!(a["avances"]["nombre"], 0);
}

#[test]
fn tout_ce_qui_est_porte_ici_passe_le_detecteur() {
    let mut base = base_avec_demo();
    base.auditer(true);
    let j = journee(&mut base);
    let d = droits_de(&mut base, &j.patron);
    accueil(&mut base, d, &j.patron);
}
