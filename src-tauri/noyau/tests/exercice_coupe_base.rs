//! `dossiers::couper_exercice_sur` (D38) — couper un exercice en plein
//! milieu, comme dans Ciel : il s'arrete a la date choisie, le suivant
//! s'ouvre le lendemain dans la meme transaction. Nos regles tiennent :
//! pas de trou, rien de ce qui s'ecrivait hier n'est refuse demain, un
//! clos ne se touche pas, et aucune ecriture ne bouge. Teste sur SQLite
//! par defaut, sur PostgreSQL avec `GESCOM_PG`.

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

fn exercices(base: &mut Base) -> Vec<serde_json::Value> {
    dossiers::lire_exercices_sur(base).unwrap()
}

fn id(e: &serde_json::Value) -> String {
    e["id"].as_str().unwrap().to_string()
}

fn au_journal(base: &mut Base, dossier: &str, type_evenement: &str) -> i64 {
    compter(
        base,
        "SELECT CAST(COUNT(*) AS BIGINT) FROM journal WHERE type_evenement = ?1 AND dossier_id = ?2",
        &parametres![type_evenement, dossier],
    )
}

/// Le patron passe a un exercice juillet -> juin : il coupe 2026 au
/// 30 juin. Le 1er juillet commence le suivant, pour un an ; tout ce qui
/// s'ecrivait jusqu'au 31 decembre s'ecrit encore.
#[test]
fn couper_au_30_juin_ouvre_le_suivant_le_lendemain() {
    let mut base = base_avec_demo();
    let bout = dossier(&mut base, "BOUT", "2026-01-01", "2026-12-31");
    let premier = id(&exercices(&mut base)[0]);

    let r = dossiers::couper_exercice_sur(&mut base, premier.clone(), "2026-06-30".into(), None).expect("couper");
    assert_eq!(r["coupe_au"], "2026-06-30");
    assert_eq!((r["suivant"]["date_debut"].as_str(), r["suivant"]["date_fin"].as_str()), (Some("2026-07-01"), Some("2027-06-30")));

    let ex = exercices(&mut base);
    assert_eq!(ex.len(), 2);
    assert_eq!((ex[0]["date_debut"].as_str(), ex[0]["fin_effective"].as_str()), (Some("2026-01-01"), Some("2026-06-30")));
    assert_eq!((ex[1]["date_debut"].as_str(), ex[1]["date_fin"].as_str()), (Some("2026-07-01"), Some("2027-06-30")));
    assert_eq!(ex[1]["clos"], false);

    for jour in ["2026-01-01", "2026-06-30", "2026-07-01", "2026-12-31", "2027-06-30"] {
        assert!(dossiers::verifier_date_sur(&mut base, jour).is_ok(), "le {jour} s'écrit");
    }
    assert_eq!(
        dossiers::verifier_date_sur(&mut base, "2027-07-01").unwrap_err(),
        "Le 1er juillet 2027 est hors des dates de travail (jusqu'au 30 juin 2027). \
         Prolonger l'exercice ou en ouvrir un nouveau."
    );
    // L'avis de fin suit le nouveau cycle.
    let a = dossiers::avis_fin_exercice_sur(&mut base, "2026-12-28").unwrap();
    assert_eq!((a["fin"].as_str(), a["avis"].as_str()), (Some("2027-06-30"), Some("rien")));

    // Le comptable clot la premiere moitie : elle se ferme, la suite non.
    dossiers::clore_exercice_sur(&mut base, premier).expect("clore");
    assert!(dossiers::verifier_date_sur(&mut base, "2026-03-01").unwrap_err().contains("closes"));
    assert!(dossiers::verifier_date_sur(&mut base, "2026-08-01").is_ok());

    assert_eq!(au_journal(&mut base, &bout, "exercice_coupe"), 1);
    assert_eq!(au_journal(&mut base, &bout, "exercice_ouvert"), 1, "le suivant est au journal");
}

/// Une prolongation passait deja 2027 : le suivant va au moins jusque-la,
/// et la prolongation s'efface de la ligne (le journal la garde).
#[test]
fn couper_un_exercice_prolonge_garde_tout_ce_qui_s_ecrivait() {
    let mut base = base_avec_demo();
    dossier(&mut base, "BOUT", "2026-01-01", "2026-12-31");
    let premier = id(&exercices(&mut base)[0]);
    dossiers::prolonger_exercice_sur(&mut base, premier.clone(), "2028-03-31".into()).expect("prolonger");

    assert!(dossiers::couper_exercice_sur(&mut base, premier.clone(), "2026-06-30".into(), Some("2027-06-30".into()))
        .unwrap_err()
        .contains("au moins jusqu'au 31 mars 2028"));
    let r = dossiers::couper_exercice_sur(&mut base, premier, "2026-06-30".into(), None).expect("couper");
    assert_eq!(r["suivant"]["date_fin"], "2028-03-31");
    let ex = exercices(&mut base);
    assert!(ex[0]["prolonge_jusqu_au"].is_null(), "la fin est la coupe");
    assert_eq!(ex[0]["fin_effective"], "2026-06-30");
    assert!(dossiers::verifier_date_sur(&mut base, "2028-03-31").is_ok());
}

/// Ni un clos, ni un exercice qui a deja un suivant, ni une date hors de
/// l'exercice — et un refus n'ecrit rien.
#[test]
fn ce_qui_ne_se_coupe_pas_est_refuse_sans_rien_ecrire() {
    let mut base = base_avec_demo();
    let bout = dossier(&mut base, "BOUT", "2026-01-01", "2026-12-31");
    let premier = id(&exercices(&mut base)[0]);

    assert!(dossiers::couper_exercice_sur(&mut base, premier.clone(), "2026-12-31".into(), None)
        .unwrap_err()
        .contains("la veille de la fin"));
    assert!(dossiers::couper_exercice_sur(&mut base, premier.clone(), "2027-03-01".into(), None).is_err());
    assert!(dossiers::couper_exercice_sur(&mut base, premier.clone(), "2026-06-30".into(), Some("2072-06-30".into()))
        .unwrap_err()
        .contains("plus de 5 ans"));

    dossiers::ouvrir_exercice_sur(&mut base, "2027-01-01".into(), "2027-12-31".into()).expect("le suivant");
    assert!(dossiers::couper_exercice_sur(&mut base, premier.clone(), "2026-06-30".into(), None)
        .unwrap_err()
        .contains("on ne coupe que le dernier"));
    let dernier = id(&exercices(&mut base)[1]);
    // On clot dans l'ordre, une fois termine (D38).
    assert!(dossiers::clore_exercice_au_sur(&mut base, dernier.clone(), "2028-01-02")
        .unwrap_err()
        .contains("Clore d'abord"));
    dossiers::clore_exercice_au_sur(&mut base, premier.clone(), "2028-01-02").expect("clore 2026");
    dossiers::clore_exercice_au_sur(&mut base, dernier.clone(), "2028-01-02").expect("clore 2027");
    assert!(dossiers::couper_exercice_sur(&mut base, dernier, "2027-06-30".into(), None).unwrap_err().contains("clos"));

    // Depuis un autre dossier, l'exercice n'existe pas.
    base.choisir_dossier(dossiers::DOSSIER_DEFAUT).unwrap();
    assert_eq!(
        dossiers::couper_exercice_sur(&mut base, premier, "2026-06-30".into(), None).unwrap_err(),
        "Exercice introuvable."
    );

    base.choisir_dossier(&bout).unwrap();
    assert_eq!(exercices(&mut base).len(), 2, "aucun exercice de plus");
    assert_eq!(au_journal(&mut base, &bout, "exercice_coupe"), 0);
}

/// Couper ne touche que les exercices et le journal : ventes, stock,
/// caisse, reglements, factures restent tels quels, et aujourd'hui
/// s'ecrit encore.
#[test]
fn couper_ne_touche_ni_stock_ni_caisse_ni_creance() {
    let mut base = base_avec_demo();
    let defaut = dossiers::DOSSIER_DEFAUT;
    let maintenant = gescom_noyau::utils::maintenant_iso();
    let annee = &maintenant[..4];
    let tables = ["vente", "ligne_vente", "mouvement_stock", "stock_depot", "mouvement_caisse", "paiement", "facture"];
    let photo = |base: &mut Base| -> Vec<i64> {
        tables
            .iter()
            .map(|t| {
                compter(
                    base,
                    &format!("SELECT CAST(COUNT(*) AS BIGINT) FROM {t} WHERE dossier_id = ?1"),
                    &parametres![defaut],
                )
            })
            .collect()
    };
    let avant = photo(&mut base);
    let journal_avant = compter(
        &mut base,
        "SELECT CAST(COUNT(*) AS BIGINT) FROM journal WHERE dossier_id = ?1",
        &parametres![defaut],
    );

    let dernier = exercices(&mut base).last().map(id).unwrap();
    dossiers::couper_exercice_sur(&mut base, dernier, format!("{annee}-06-30"), None).expect("couper");

    assert_eq!(photo(&mut base), avant, "ni vente, ni stock, ni caisse, ni règlement, ni facture");
    let journal_apres = compter(
        &mut base,
        "SELECT CAST(COUNT(*) AS BIGINT) FROM journal WHERE dossier_id = ?1",
        &parametres![defaut],
    );
    assert_eq!(journal_apres, journal_avant + 2, "la coupe et l'ouverture du suivant");
    assert!(dossiers::verifier_date_sur(&mut base, &maintenant).is_ok(), "aujourd'hui s'écrit encore");
}

#[test]
fn tout_ce_que_touche_la_coupe_passe_le_detecteur() {
    let mut base = base_avec_demo();
    base.auditer(true);
    dossier(&mut base, "AUDIT", "2026-01-01", "2026-12-31");
    let premier = id(&exercices(&mut base)[0]);
    dossiers::prolonger_exercice_sur(&mut base, premier.clone(), "2027-01-31".into()).expect("prolonger");
    dossiers::couper_exercice_sur(&mut base, premier.clone(), "2026-06-30".into(), Some("2027-06-30".into()))
        .expect("couper");
    assert!(dossiers::couper_exercice_sur(&mut base, premier, "2026-03-31".into(), None).is_err());
    dossiers::lire_exercices_sur(&mut base).expect("lire");
    dossiers::verifier_date_sur(&mut base, "2026-09-01").expect("vérifier");
    dossiers::avis_fin_exercice_sur(&mut base, "2027-06-01").expect("avis");
}
