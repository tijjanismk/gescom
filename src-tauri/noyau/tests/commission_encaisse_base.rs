//! D42 — la commission se calcule sur l'encaisse : l'argent recu sur les
//! ventes du vendeur dans la periode, moins ce qui a ete rendu. Sur
//! SQLite, PostgreSQL et le compte limite ; le dernier scenario sous le
//! detecteur de requetes non cloisonnees.
//!
//! Les dates sont celles des exemples (septembre et octobre 2026) : une
//! vente, un reglement ou un retour se datent du jour ou on les fait ; on
//! les antidate dans la base pour les poser dans le mois voulu.

mod commun;

use commun::*;
use gescom_noyau::argent::{self, ParamsLigneInput};
use gescom_noyau::base::Base;
use gescom_noyau::personnel::{self, Fiche};
use gescom_noyau::{auteur, auth, creances, paie, parametres, retours};
use serde_json::{json, Value};

const SEPT: (&str, &str) = ("2026-09-01", "2026-09-30");
const OCT: (&str, &str) = ("2026-10-01", "2026-10-31");

fn creer(base: &mut Base, v: Value) -> String {
    let f: Fiche = serde_json::from_value(v).unwrap();
    personnel::creer_sur(base, f).unwrap()["id"].as_str().unwrap().to_string()
}

fn compte(base: &mut Base, nom: &str) -> String {
    let admin = base
        .lire_une("SELECT utilisateur_id FROM utilisateur_auth WHERE pseudo = 'admin'", &[], |r| r.get::<String>(0))
        .unwrap()
        .unwrap();
    auth::creer_utilisateur_sur_base(base, nom.into(), nom.to_lowercase(), None, "secret-123".into(), "caissier".into(), admin).unwrap()
}

fn dater(base: &mut Base, sql: &str, id: &str, jour: &str) {
    let dossier = base.dossier().to_string();
    base.executer(sql, &parametres![format!("{jour}T10:00:00"), id, dossier]).unwrap();
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
    dater(base, "UPDATE vente SET date_vente = ?1 WHERE id = ?2 AND dossier_id = ?3", &vente, jour);
    let dossier = base.dossier().to_string();
    let ligne = base
        .lire_une("SELECT id FROM ligne_vente WHERE vente_id = ?1 AND dossier_id = ?2", &parametres![vente.clone(), dossier], |r| r.get::<String>(0))
        .unwrap()
        .unwrap();
    ((sucre.3 as f64 * quantite).round() as i64, vente, ligne)
}

/// Le client paie `montant` en especes, le `jour`.
fn payer_le(base: &mut Base, vente: &str, montant: i64, jour: &str) {
    creances::regler_creance_sur_base(base, vente.into(), montant, "especes".into(), None).expect("reglement");
    let dossier = base.dossier().to_string();
    let aujourd_hui = chrono::Local::now().format("%Y-%m-%d").to_string();
    base.executer(
        "UPDATE paiement SET date_paiement = ?1
         WHERE vente_id = ?2 AND dossier_id = ?3 AND mode = 'especes' AND SUBSTR(date_paiement, 1, 10) = ?4",
        &parametres![format!("{jour}T10:00:00"), vente, dossier, aujourd_hui],
    )
    .unwrap();
}

/// Un retour de `quantite` kg resolu en remboursement, le `jour`.
fn rendre_le(base: &mut Base, vente: &str, ligne: &str, quantite: f64, jour: &str) {
    let r = retours::enregistrer_retour_sur_base(
        base, vente.into(), ligne.into(), quantite, "remboursement".into(), Some("especes".into()), None, None, None, None, None,
    )
    .expect("retour");
    let id = r["retour_id"].as_str().unwrap().to_string();
    dater(base, "UPDATE retour SET date_retour = ?1 WHERE id = ?2 AND dossier_id = ?3", &id, jour);
    // La part qui eteint la dette se date avec lui.
    let dossier = base.dossier().to_string();
    let aujourd_hui = chrono::Local::now().format("%Y-%m-%d").to_string();
    base.executer(
        "UPDATE paiement SET date_paiement = ?1
         WHERE vente_id = ?2 AND dossier_id = ?3 AND mode = 'retour' AND SUBSTR(date_paiement, 1, 10) = ?4",
        &parametres![format!("{jour}T10:00:00"), vente, dossier, aujourd_hui],
    )
    .unwrap();
}

fn commission(base: &mut Base, employe: &str, (du, au): (&str, &str)) -> Value {
    let f = paie::preparer_sur(base, employe.into(), du.into(), au.into(), false).unwrap();
    f["lignes"].as_array().unwrap().iter().find(|l| l["genre"] == "commission").unwrap().clone()
}

fn vendeuse(base: &mut Base, nom: &str) -> (String, String) {
    let u = compte(base, nom);
    let e = creer(base, json!({ "nom": nom, "fonction": "vendeuse", "commission_pct": 3.0, "utilisateur_id": u }));
    (u, e)
}

#[test]
fn un_credit_se_commissionne_au_fil_de_ses_reglements() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let (u, aminata) = vendeuse(&mut base, "Aminata");
    let (m, vente, _) = vendre_le(&mut base, &u, 10.0, "2026-09-10");
    let moitie = m / 2;
    payer_le(&mut base, &vente, moitie, "2026-09-20");
    payer_le(&mut base, &vente, m - moitie, "2026-10-05");
    // Un credit jamais paye ne commissionne pas.
    vendre_le(&mut base, &u, 6.0, "2026-09-12");

    let s = commission(&mut base, &aminata, SEPT);
    assert_eq!(s["prix"], moitie, "septembre : la moitie recue");
    assert_eq!(s["montant"], (moitie * 3 + 50) / 100);
    assert!(s["libelle"].as_str().unwrap().contains("encaissés"), "{s}");
    let o = commission(&mut base, &aminata, OCT);
    assert_eq!(o["prix"], m - moitie, "octobre : le reste, le mois ou il arrive");
}

#[test]
fn une_dette_eteinte_par_un_retour_ne_compte_ni_d_un_cote_ni_de_l_autre() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let (u, aminata) = vendeuse(&mut base, "Aminata");
    let prix = article_unite(&mut base, "Sucre").3;
    // Rien de paye, 3 kg rendus : la dette baisse, rien n'est rendu en argent.
    let (_, impayee, ligne) = vendre_le(&mut base, &u, 10.0, "2026-09-10");
    rendre_le(&mut base, &impayee, &ligne, 3.0, "2026-09-15");
    // Payee en entier, 2 kg rendus en especes : 2 kg se deduisent.
    let (m, payee, ligne2) = vendre_le(&mut base, &u, 5.0, "2026-09-11");
    payer_le(&mut base, &payee, m, "2026-09-11");
    rendre_le(&mut base, &payee, &ligne2, 2.0, "2026-09-18");

    let s = commission(&mut base, &aminata, SEPT);
    assert_eq!(s["prix"], m - 2 * prix, "{s}");
    assert!(s["libelle"].as_str().unwrap().contains("rendus"), "{s}");
}

#[test]
fn un_reglement_apres_le_depart_ne_rapporte_rien() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let (u, fanta) = vendeuse(&mut base, "Fanta");
    let (m, vente, _) = vendre_le(&mut base, &u, 10.0, "2026-09-05");
    personnel::faire_partir_sur(&mut base, fanta.clone(), Some("2026-09-15".into()), None).unwrap();
    payer_le(&mut base, &vente, m, "2026-09-20");
    let s = commission(&mut base, &fanta, SEPT);
    assert_eq!(s["prix"], 0, "le reglement du 20 vient apres son depart du 15");
}

#[test]
fn une_vente_deja_commissionnee_sur_le_vendu_ne_l_est_pas_a_son_reglement() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let (u, aminata) = vendeuse(&mut base, "Aminata");
    let prix = article_unite(&mut base, "Sucre").3;
    let (m_sept, vieille, ligne) = vendre_le(&mut base, &u, 10.0, "2026-09-10");

    // La fiche de septembre, validee avant D42 : sur le vendu.
    let f = paie::preparer_sur(&mut base, aminata.clone(), SEPT.0.into(), SEPT.1.into(), false).unwrap();
    let f = paie::valider_sur(&mut base, f["id"].as_str().unwrap().into()).unwrap();
    let dossier = base.dossier().to_string();
    let conditions: String = base
        .lire_une("SELECT conditions FROM fiche_paie WHERE id = ?1 AND dossier_id = ?2", &parametres![f["id"].as_str().unwrap(), dossier.clone()], |r| {
            r.get::<String>(0)
        })
        .unwrap()
        .unwrap();
    let avant_d42 = conditions.replace("\"commission_sur_encaisse\":true", "\"commission_sur_encaisse\":false");
    assert_ne!(avant_d42, conditions, "la fiche dit sa regle");
    base.executer("UPDATE fiche_paie SET conditions = ?1 WHERE id = ?2 AND dossier_id = ?3", &parametres![avant_d42, f["id"].as_str().unwrap(), dossier])
        .unwrap();

    // En octobre : la vieille vente est payee et 1 kg en revient ; une
    // nouvelle est vendue et payee.
    payer_le(&mut base, &vieille, m_sept, "2026-10-03");
    rendre_le(&mut base, &vieille, &ligne, 1.0, "2026-10-08");
    let (m_oct, neuve, _) = vendre_le(&mut base, &u, 4.0, "2026-10-06");
    payer_le(&mut base, &neuve, m_oct, "2026-10-06");

    let o = commission(&mut base, &aminata, OCT);
    assert_eq!(o["prix"], m_oct - prix, "la neuve, moins le retour de la vieille pour sa valeur entiere : {o}");
}

#[test]
fn tout_ce_qui_est_porte_ici_passe_le_detecteur() {
    let mut base = base_avec_demo();
    base.auditer(true);
    ouvrir_caisse(&mut base);
    let (u, aminata) = vendeuse(&mut base, "Aminata");
    let (m, vente, ligne) = vendre_le(&mut base, &u, 10.0, "2026-09-10");
    payer_le(&mut base, &vente, m, "2026-09-12");
    rendre_le(&mut base, &vente, &ligne, 1.0, "2026-09-14");
    let f = paie::preparer_sur(&mut base, aminata.clone(), SEPT.0.into(), SEPT.1.into(), false).unwrap();
    paie::valider_sur(&mut base, f["id"].as_str().unwrap().into()).unwrap();
    paie::preparer_sur(&mut base, aminata, OCT.0.into(), OCT.1.into(), false).unwrap();
}
