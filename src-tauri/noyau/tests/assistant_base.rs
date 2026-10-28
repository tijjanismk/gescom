//! D43 — l'assistant de creation du dossier, sur les deux moteurs.
//!
//! Une base neuve : le patron seul, la marque « a faire », tout le
//! reste refuse par le serveur. L'assistant fini : le patron a son nom,
//! son identifiant et son mot de passe (plus celui d'usine), la societe
//! et le dossier leur nom, l'exercice ses dates, le magasin son nom, la
//! TVA et le fond de caisse leur valeur, la premiere facture son numero,
//! les vendeurs leur compte. Une saisie refusee n'ecrit rien.
//!
//!   cargo test -p gescom-noyau --test assistant_base
//!   GESCOM_PG=postgresql://…/gescom_test cargo test -p gescom-noyau --test assistant_base -- --test-threads=1

mod commun;

use commun::*;
use gescom_noyau::assistant::{self, Saisie, Vendeur};
use gescom_noyau::base::Base;
use gescom_noyau::{auth, parametres};

fn patron(base: &mut Base) -> String {
    base.lire_une(
        "SELECT ua.utilisateur_id FROM utilisateur_auth ua
         JOIN utilisateur u ON u.id = ua.utilisateur_id
         JOIN role r ON r.id = u.role_id WHERE r.nom = 'patron'",
        &[],
        |r| r.get::<String>(0),
    )
    .unwrap()
    .unwrap()
}

fn aujourd_hui() -> String {
    gescom_noyau::utils::maintenant_iso()[..10].to_string()
}

/// Une saisie complete et juste, l'annee civile en cours.
fn saisie() -> Saisie {
    let annee = &aujourd_hui()[..4];
    Saisie {
        patron_nom: "Moussa Traoré".into(),
        patron_pseudo: "Moussa".into(),
        mot_de_passe: "Bamako-2026".into(),
        societe: "  Quincaillerie Traoré  ".into(),
        adresse: Some("Marché de Médine, Bamako".into()),
        telephone: Some("76 00 00 00".into()),
        telephone2: Some(" ".into()),
        nif: Some("084123456M".into()),
        rccm: Some("MA.BKO.2026.B.1234".into()),
        date_debut: format!("{annee}-01-01"),
        date_fin: format!("{annee}-12-31"),
        tva_pct: 18,
        numero_depart: Some(1251),
        magasin: "Boutique Médine".into(),
        fond_caisse: 25_000,
        vendeurs: vec![Vendeur { nom: "Awa Diallo".into(), pseudo: "awa".into(), mot_de_passe: "awa-2026".into() }],
        ..Default::default()
    }
}

fn une(base: &mut Base, sql: &str) -> Option<String> {
    let dossier = base.dossier().to_string();
    base.lire_une(sql, &parametres![dossier], |r| r.get::<String>(0)).unwrap()
}

#[test]
fn une_base_neuve_attend_son_assistant_et_n_a_que_le_patron() {
    let mut base = base_neuve();
    assert!(assistant::a_faire_sur(&mut base));
    assert_eq!(compter(&mut base, "SELECT COUNT(*) FROM utilisateur_auth", &[]), 1, "plus de compte employe d'usine");
    assert_eq!(gescom_noyau::amorcage::COMPTES_USINE.len(), 1);
    // Le serveur ne sert que l'assistant.
    assert!(assistant::refus_si_a_faire(&mut base, "creer_vente").unwrap().starts_with("ASSISTANT_A_FAIRE"));
    assert!(assistant::refus_si_a_faire(&mut base, "changer_mot_de_passe").is_some());
    for permise in ["lire_assistant", "terminer_assistant", "sauvegarder_logo", "lire_parametres_societe"] {
        assert!(assistant::refus_si_a_faire(&mut base, permise).is_none(), "{permise}");
    }
    // Ce que l'ecran propose.
    let id = patron(&mut base);
    let l = assistant::lire_sur(&mut base, &id).unwrap();
    assert_eq!(l["a_faire"], true);
    assert_eq!(l["patron_pseudo"], "admin");
    assert_eq!(l["magasin"], "Magasin principal");
    assert_eq!(l["tva_pct"], 0);
    assert!(l["date_debut"].as_str().unwrap().ends_with("-01-01"));
}

#[test]
fn une_base_de_demonstration_n_a_pas_d_assistant() {
    let mut base = base_avec_demo();
    assert!(!assistant::a_faire_sur(&mut base));
    assert!(assistant::refus_si_a_faire(&mut base, "creer_vente").is_none());
    assert_eq!(
        compter(&mut base, "SELECT COUNT(*) FROM utilisateur_auth WHERE pseudo = 'employe'", &[]),
        1,
        "le compte d'essai des scenarios et du banc"
    );
}

#[test]
fn l_assistant_fini_tout_est_en_place() {
    let mut base = base_neuve();
    let id = patron(&mut base);
    let r = assistant::terminer_sur(&mut base, &id, saisie()).unwrap();
    assert_eq!(r["societe"], "Quincaillerie Traoré");
    assert!(!r.to_string().contains("Bamako-2026"), "aucun mot de passe au résumé");

    // Le patron : nouveau nom, nouvel identifiant, nouveau mot de passe.
    let c = auth::connexion_sur(&mut base, "Moussa".into(), "Bamako-2026".into()).unwrap();
    assert_eq!(c["doit_changer_mdp"], false);
    assert_eq!(c["nom"], "Moussa Traoré");
    assert!(auth::connexion_sur(&mut base, "admin".into(), "admin123".into()).is_err(), "le mot de passe d'usine ne sert plus");

    // La societe, le dossier, l'exercice, le magasin.
    let soc = gescom_noyau::societe::lire_parametres_societe_sur_base(&mut base).unwrap();
    assert_eq!(soc["nom"], "Quincaillerie Traoré");
    assert_eq!(soc["nif"], "084123456M");
    assert_eq!(soc["telephone2"], serde_json::Value::Null, "un champ blanc reste vide");
    assert_eq!(une(&mut base, "SELECT societe FROM dossier WHERE id = ?1").as_deref(), Some("Quincaillerie Traoré"));
    let annee = &aujourd_hui()[..4];
    assert_eq!(
        une(&mut base, "SELECT date_debut FROM exercice WHERE dossier_id = ?1"),
        Some(format!("{annee}-01-01"))
    );
    assert_eq!(depot_nom(&mut base), "Boutique Médine");

    // Les reglages qui servent ensuite.
    let reg = assistant::lire_reglages_dossier_sur(&mut base).unwrap();
    assert_eq!((reg["tva_pct"].as_i64(), reg["fond_caisse"].as_i64()), (Some(18), Some(25_000)));
    let a = gescom_noyau::comptoir::creer_article_rapide_sur(&mut base, "Ciment CIMAF".into(), "sac".into(), 5_000, None).unwrap();
    let taux: f64 = base
        .lire_une("SELECT taux_tva_defaut FROM article WHERE id = ?1", &parametres![a["id"].as_str().unwrap()], |r| r.get::<f64>(0))
        .unwrap()
        .unwrap();
    assert!((taux - 0.18).abs() < 1e-9, "un article neuf prend la TVA du dossier ({taux})");
    let numero = gescom_noyau::argent::reserver_numero_sur(&mut base, "facture").unwrap();
    assert!(numero.ends_with("-01251"), "{numero}");

    // La vendeuse : un compte employe, mot de passe a changer.
    let v = auth::connexion_sur(&mut base, "awa".into(), "awa-2026".into()).unwrap();
    assert_eq!((v["role"].as_str(), v["doit_changer_mdp"].as_bool()), (Some("employe"), Some(true)));

    // Trace, et fait une fois pour toutes.
    let dossier = base.dossier().to_string();
    assert_eq!(
        compter(&mut base, "SELECT COUNT(*) FROM journal WHERE type_evenement = 'assistant_dossier_fait' AND dossier_id = ?1", &parametres![dossier]),
        1
    );
    assert!(!assistant::a_faire_sur(&mut base));
    assert!(assistant::refus_si_a_faire(&mut base, "creer_vente").is_none());
    let encore = assistant::terminer_sur(&mut base, &id, saisie()).unwrap_err();
    assert!(encore.contains("déjà créé"), "{encore}");
}

fn depot_nom(base: &mut Base) -> String {
    let dossier = base.dossier().to_string();
    base.lire_une("SELECT nom FROM depot WHERE est_defaut = 1 AND dossier_id = ?1", &parametres![dossier], |r| r.get::<String>(0))
        .unwrap()
        .unwrap()
}

#[test]
fn une_saisie_refusee_n_ecrit_rien() {
    let mut base = base_neuve();
    let id = patron(&mut base);
    let refus = |m: fn(&mut Saisie)| {
        let mut s = saisie();
        m(&mut s);
        s
    };
    let cas: Vec<(Saisie, &str)> = vec![
        (refus(|s| s.mot_de_passe = "admin123".into()), "usine"),
        (refus(|s| s.mot_de_passe = "12345".into()), "6 caractères"),
        (refus(|s| s.societe = "   ".into()), "Raison sociale"),
        (refus(|s| s.tva_pct = 20), "TVA"),
        (refus(|s| s.numero_depart = Some(0)), "numéro"),
        (refus(|s| s.fond_caisse = -5), "négatif"),
        (refus(|s| s.magasin = "".into()), "magasin"),
        (refus(|s| s.vendeurs[0].pseudo = "moussa".into()), "deux fois"),
        (refus(|s| s.vendeurs[0].mot_de_passe = "awa".into()), "Awa Diallo"),
        (refus(|s| s.date_fin = "2099-12-31".into()), "5 ans"),
        (refus(|s| s.date_debut = "n'importe".into()), "illisible"),
    ];
    for (s, attendu) in cas {
        let e = assistant::terminer_sur(&mut base, &id, s).unwrap_err();
        assert!(e.contains(attendu), "« {attendu} » attendu : {e}");
    }
    assert!(assistant::a_faire_sur(&mut base), "toujours à faire");
    assert_eq!(depot_nom(&mut base), "Magasin principal");
    assert!(auth::connexion_sur(&mut base, "admin".into(), "admin123".into()).is_ok(), "rien n'a bougé");
    assert_eq!(compter(&mut base, "SELECT COUNT(*) FROM utilisateur_auth", &[]), 1);
}

#[test]
fn les_dates_doivent_couvrir_le_jour_de_l_assistant() {
    let mut base = base_neuve();
    let id = patron(&mut base);
    let mut s = saisie();
    s.date_debut = "2026-07-01".into();
    s.date_fin = "2027-06-30".into();
    // Un 15 mars 2026 : l'exercice juillet-juin n'a pas commence.
    let e = assistant::terminer_au_sur(&mut base, &id, s.clone(), "2026-03-15T09:00:00").unwrap_err();
    assert!(e.contains("commencent après"), "{e}");
    // Un 1er septembre 2026 : il passe, et l'exercice garde ses dates.
    assistant::terminer_au_sur(&mut base, &id, s, "2026-09-01T09:00:00").unwrap();
    assert_eq!(une(&mut base, "SELECT date_fin FROM exercice WHERE dossier_id = ?1").as_deref(), Some("2027-06-30"));
}

#[test]
fn le_premier_numero_ne_se_regle_plus_apres_une_facture() {
    let mut base = base_neuve();
    let id = patron(&mut base);
    // Une facture deja sortie (impossible par le serveur, qui ne sert que
    // l'assistant ; une base d'avant ou une main dans la base).
    gescom_noyau::argent::reserver_numero_sur(&mut base, "facture").unwrap();
    let e = assistant::terminer_sur(&mut base, &id, saisie()).unwrap_err();
    assert!(e.contains("premier numéro"), "{e}");
    // Sans numero de depart, l'assistant passe.
    let mut s = saisie();
    s.numero_depart = None;
    assistant::terminer_sur(&mut base, &id, s).unwrap();
}

#[test]
fn un_identifiant_deja_pris_se_refuse() {
    let mut base = base_neuve();
    let id = patron(&mut base);
    gescom_noyau::amorcage::mode_essai(&mut base).unwrap();
    // `mode_essai` a pose « fait » ; on remet la marque pour l'essai.
    base.executer(
        "UPDATE config_app SET valeur = 'a_faire' WHERE cle = 'assistant_dossier'",
        &[],
    )
    .unwrap();
    let mut s = saisie();
    s.vendeurs[0].pseudo = "EMPLOYE".into();
    let e = assistant::terminer_sur(&mut base, &id, s).unwrap_err();
    assert!(e.contains("déjà pris"), "{e}");
}

#[test]
fn les_reglages_du_dossier_se_changent_ensuite() {
    let mut base = base_avec_demo();
    let r = assistant::definir_reglages_dossier_sur(&mut base, 18, 10_000).unwrap();
    assert_eq!((r["tva_pct"].as_i64(), r["fond_caisse"].as_i64()), (Some(18), Some(10_000)));
    assert!(assistant::definir_reglages_dossier_sur(&mut base, 5, 0).is_err());
    assert!(assistant::definir_reglages_dossier_sur(&mut base, 0, -1).is_err());
    assert_eq!(assistant::fond_habituel_sur(&mut base), 10_000, "le refus n'a rien écrit");
    let dossier = base.dossier().to_string();
    assert_eq!(
        compter(&mut base, "SELECT COUNT(*) FROM journal WHERE type_evenement = 'reglages_dossier_modifies' AND dossier_id = ?1", &parametres![dossier]),
        1
    );
}

#[test]
fn tout_passe_le_detecteur() {
    let mut base = base_neuve();
    base.auditer(true);
    let id = patron(&mut base);
    assistant::lire_sur(&mut base, &id).unwrap();
    assistant::terminer_sur(&mut base, &id, saisie()).unwrap();
    assistant::definir_reglages_dossier_sur(&mut base, 0, 5_000).unwrap();
    assistant::lire_reglages_dossier_sur(&mut base).unwrap();
    assistant::definir_tva_boutique_sur(&mut base, 18, true).unwrap();
}

/// L'etat de la TVA se change dans l'application (Parametres -> TVA) :
/// les nouveaux articles seulement, ou tous, en une transaction.
#[test]
fn l_etat_de_la_tva_se_change_seul_ou_avec_tous_les_articles() {
    let mut base = base_avec_demo();
    let taux = |base: &mut Base| -> Vec<f64> {
        base.lire_plusieurs("SELECT taux_tva_defaut FROM article WHERE actif = 1", &[], |r| r.get::<f64>(0)).unwrap()
    };
    let avant = taux(&mut base);
    let r = assistant::definir_tva_boutique_sur(&mut base, 18, false).unwrap();
    assert_eq!((r["tva_pct"].as_i64(), r["articles_modifies"].as_i64()), (Some(18), Some(0)));
    assert_eq!(taux(&mut base), avant, "les articles existants gardent leur taux");
    assert!((assistant::taux_nouvel_article_sur(&mut base) - 0.18).abs() < 1e-9);

    let r = assistant::definir_tva_boutique_sur(&mut base, 18, true).unwrap();
    assert_eq!(r["articles_modifies"].as_i64(), Some(avant.len() as i64));
    assert!(taux(&mut base).iter().all(|t| (t - 0.18).abs() < 1e-9), "tous à 18 %");
    assert_eq!(r["nb_articles"].as_i64(), Some(avant.len() as i64));

    assert!(assistant::definir_tva_boutique_sur(&mut base, 5, true).is_err(), "0 ou 18 seulement");
    assert!(taux(&mut base).iter().all(|t| (t - 0.18).abs() < 1e-9), "le refus n'a rien écrit");
    let dossier = base.dossier().to_string();
    assert_eq!(
        compter(&mut base, "SELECT COUNT(*) FROM journal WHERE type_evenement = 'reglages_dossier_modifies' AND dossier_id = ?1", &parametres![dossier]),
        2
    );
}
