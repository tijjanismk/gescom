//! `dossiers::*_sur` — la v3 commence : lister et créer des dossiers,
//! une session qui porte son dossier, le dossier mémorisé d'une
//! personne. Testé sur SQLite par défaut, sur PostgreSQL avec
//! `GESCOM_PG`. Depuis D-2/D-3 (D22), la création passe sur les DEUX
//! moteurs, avec ses dates de travail (D21).

mod commun;

use commun::*;
use gescom_noyau::base::Base;
use gescom_noyau::parametres;
use gescom_noyau::{comptoir, dossiers, postes, sessions};

fn admin(base: &mut Base) -> String {
    base.lire_une(
        "SELECT utilisateur_id FROM utilisateur_auth WHERE pseudo = 'admin'",
        &[],
        |r| r.get::<String>(0),
    )
    .unwrap()
    .expect("admin")
}

#[test]
fn au_depart_un_seul_dossier_avec_l_exercice_de_l_annee() {
    let mut base = base_avec_demo();
    let liste = dossiers::lire_dossiers_sur(&mut base).unwrap();
    assert_eq!(liste.len(), 1);
    assert_eq!(liste[0]["id"], dossiers::DOSSIER_DEFAUT);
    assert_eq!(liste[0]["code"], "PRINCIPAL");
    assert_eq!(liste[0]["clos"], false);
    assert_eq!(liste[0]["exercices_ouverts"], 1);

    let d = dossiers::dossier_ouvert_sur(&mut base, dossiers::DOSSIER_DEFAUT).unwrap();
    assert_eq!(d.code, "PRINCIPAL");
    assert!(dossiers::dossier_ouvert_sur(&mut base, "inconnu").unwrap_err().contains("introuvable"));
}

#[test]
fn creer_un_dossier_lui_donne_son_exercice_son_magasin_et_son_client_de_passage() {
    let mut base = base_avec_demo();
    let r = dossiers::creer_dossier_sur(&mut base, " quinc ".into(), " Quincaillerie du Fleuve ".into(), None, None)
        .expect("dossier créé, sur SQLite comme sur PostgreSQL (D22)");
    assert_eq!(r["code"], "QUINC", "le code se normalise en majuscules");
    assert_eq!(r["societe"], "Quincaillerie du Fleuve");
    let id = r["id"].as_str().unwrap().to_string();

    let liste = dossiers::lire_dossiers_sur(&mut base).unwrap();
    assert_eq!(liste.len(), 2);
    let neuf = liste.iter().find(|d| d["id"] == id).unwrap();
    assert_eq!(neuf["exercices_ouverts"], 1, "l'exercice de l'année en cours");

    // Ses affaires à lui, rangées chez lui : le détecteur exige le
    // filtre, on le donne.
    let magasins = compter(
        &mut base,
        "SELECT COUNT(*) FROM depot WHERE dossier_id = ?1 AND est_defaut = 1 AND actif = 1",
        &parametres![id.clone()],
    );
    assert_eq!(magasins, 1, "un magasin par défaut, sinon il ne peut pas vendre");
    let passage = compter(
        &mut base,
        "SELECT COUNT(*) FROM client WHERE dossier_id = ?1 AND est_generique = 1",
        &parametres![id.clone()],
    );
    assert_eq!(passage, 1, "un client de passage, sinon pas de vente comptant");
    // Et rien chez le voisin.
    let chez_l_autre = compter(
        &mut base,
        "SELECT COUNT(*) FROM depot WHERE dossier_id = ?1",
        &parametres![dossiers::DOSSIER_DEFAUT],
    );
    assert_eq!(chez_l_autre, 1, "le dossier d'origine garde son seul magasin");

    // Le même code, deux fois : refus.
    let e = dossiers::creer_dossier_sur(&mut base, "quinc".into(), "Autre".into(), None, None).unwrap_err();
    assert!(e.contains("pris"), "{e}");
    // Un code impossible, un nom vide : refus, rien d'écrit.
    assert!(dossiers::creer_dossier_sur(&mut base, "a b".into(), "X".into(), None, None).is_err());
    assert!(dossiers::creer_dossier_sur(&mut base, "OK".into(), "  ".into(), None, None).is_err());
    assert!(dossiers::creer_dossier_sur(&mut base, "OK".into(), "X".into(), Some("2026-05-01".into()), Some("2026-04-01".into()))
        .is_err(), "une fin avant le début");
    assert_eq!(compter(&mut base, "SELECT COUNT(*) FROM dossier", &[]), 2);

    // Une fois dessus, ses exercices sont les siens.
    base.choisir_dossier(&id).unwrap();
    let ex = dossiers::lire_exercices_sur(&mut base).unwrap();
    assert_eq!(ex.len(), 1);
    assert_eq!(ex[0]["dossier_id"], id);

    // Ses clients portent son code : `client.code` est unique sur toute la
    // base, et le dossier d'origine a deja CLIENT00001.
    let c = comptoir::creer_client_rapide_sur(&mut base, "Awa".into(), None).unwrap();
    assert_eq!(c["code"], "QUINC-CLIENT00001", "{c}");
    let c2 = comptoir::creer_client_rapide_sur(&mut base, "Moussa".into(), None).unwrap();
    assert_eq!(c2["code"], "QUINC-CLIENT00002");
}

/// « UNIQUE constraint failed: client.code » a chaque nouveau client,
/// des qu'un code plus grand que le nombre de clients existait : le
/// code suivait COUNT + 1. Il suit le plus grand deja pris (D28).
#[test]
fn le_code_client_suit_le_plus_grand_deja_pris_pas_le_nombre_de_clients() {
    let mut base = base_avec_demo();
    let now = gescom_noyau::utils::maintenant_iso();
    // La demo a CLIENT00001..00004. Un import a pose 00010 ; 00005 a ete
    // supprime : quatre clients reels + un — COUNT + 1 donnerait 00005.
    base.executer(
        "INSERT INTO client (id, code, nom, est_generique, actif, cree_le, modifie_le, origine, dossier_id)
         VALUES ('c-dix', 'CLIENT00010', 'Importe', 0, 1, ?1, ?1, 'import', ?2)",
        &parametres![now, dossiers::DOSSIER_DEFAUT],
    )
    .unwrap();
    let c = comptoir::creer_client_rapide_sur(&mut base, "Neuf".into(), None).expect("le code ne se heurte a rien");
    assert_eq!(c["code"], "CLIENT00011", "{c}");
    let c2 = comptoir::creer_client_rapide_sur(&mut base, "Encore".into(), None).unwrap();
    assert_eq!(c2["code"], "CLIENT00012");
}

#[test]
fn une_session_porte_son_dossier_et_ne_le_choisit_qu_une_fois() {
    let mut base = base_avec_demo();
    let utilisateur = admin(&mut base);
    let poste = postes::inscrire_ou_retrouver_sur(&mut base, "Caisse 1", "emp-1", "caisse", None).unwrap();

    // Ouverte sans dossier : l'état le dit, et le choix se fait après.
    let (sid, ..) = sessions::ouvrir_dans_dossier_sur(&mut base, &poste.id, &utilisateur, None).unwrap();
    match sessions::etat_sur(&mut base, &sid) {
        sessions::Etat::Valide { dossier_id, .. } => assert!(dossier_id.is_none()),
        autre => panic!("session valide attendue : {autre:?}"),
    }
    sessions::choisir_dossier_session_sur(&mut base, &sid, dossiers::DOSSIER_DEFAUT).unwrap();
    match sessions::etat_sur(&mut base, &sid) {
        sessions::Etat::Valide { dossier_id, .. } => assert_eq!(dossier_id.as_deref(), Some(dossiers::DOSSIER_DEFAUT)),
        autre => panic!("session valide attendue : {autre:?}"),
    }
    // Changer de dossier, c'est se déconnecter : le second choix refuse.
    let e = sessions::choisir_dossier_session_sur(&mut base, &sid, dossiers::DOSSIER_DEFAUT).unwrap_err();
    assert!(e.contains("deconnecter"), "{e}");

    // Ouverte directement sur un dossier : rien à choisir.
    let (s2, ..) =
        sessions::ouvrir_dans_dossier_sur(&mut base, &poste.id, &utilisateur, Some(dossiers::DOSSIER_DEFAUT)).unwrap();
    match sessions::etat_sur(&mut base, &s2) {
        sessions::Etat::Valide { dossier_id, .. } => assert!(dossier_id.is_some()),
        autre => panic!("{autre:?}"),
    }
    // L'ancienne forme reste : sans dossier, comme avant.
    let (s3, ..) = sessions::ouvrir_sur(&mut base, &poste.id, &utilisateur).unwrap();
    match sessions::etat_sur(&mut base, &s3) {
        sessions::Etat::Valide { dossier_id, .. } => assert!(dossier_id.is_none()),
        autre => panic!("{autre:?}"),
    }
}

#[test]
fn le_dossier_memorise_d_une_personne_se_pose_et_s_oublie() {
    let mut base = base_avec_demo();
    let utilisateur = admin(&mut base);
    assert_eq!(dossiers::dossier_memorise_sur(&mut base, &utilisateur).unwrap(), None);
    dossiers::memoriser_dossier_sur(&mut base, &utilisateur, Some(dossiers::DOSSIER_DEFAUT)).unwrap();
    assert_eq!(
        dossiers::dossier_memorise_sur(&mut base, &utilisateur).unwrap().as_deref(),
        Some(dossiers::DOSSIER_DEFAUT)
    );
    // Deux fois : on remplace, on n'empile pas.
    dossiers::memoriser_dossier_sur(&mut base, &utilisateur, Some(dossiers::DOSSIER_DEFAUT)).unwrap();
    dossiers::memoriser_dossier_sur(&mut base, &utilisateur, None).unwrap();
    assert_eq!(dossiers::dossier_memorise_sur(&mut base, &utilisateur).unwrap(), None);
    // Une autre personne n'est pas concernée.
    assert_eq!(dossiers::dossier_memorise_sur(&mut base, "quelqu-un-d-autre").unwrap(), None);
}

#[test]
fn tout_ce_qui_est_porte_dans_dossiers_passe_le_detecteur() {
    let mut base = base_avec_demo();
    base.auditer(true);
    let utilisateur = admin(&mut base);
    dossiers::lire_dossiers_sur(&mut base).expect("dossiers");
    dossiers::dossier_ouvert_sur(&mut base, dossiers::DOSSIER_DEFAUT).expect("dossier");
    dossiers::lire_exercices_sur(&mut base).expect("exercices");
    dossiers::memoriser_dossier_sur(&mut base, &utilisateur, Some(dossiers::DOSSIER_DEFAUT)).expect("mémoriser");
    dossiers::dossier_memorise_sur(&mut base, &utilisateur).expect("lire");
    dossiers::memoriser_dossier_sur(&mut base, &utilisateur, None).expect("oublier");
    let audit = dossiers::creer_dossier_sur(
        &mut base,
        "AUDIT".into(),
        "Audit".into(),
        Some("2020-01-01".into()),
        Some("2020-12-31".into()),
    )
    .expect("créer");
    base.choisir_dossier(audit["id"].as_str().unwrap()).unwrap();
    let premier = dossiers::lire_exercices_sur(&mut base).unwrap()[0]["id"].as_str().unwrap().to_string();
    dossiers::prolonger_exercice_sur(&mut base, premier.clone(), "2021-01-31".into()).expect("prolonger");
    dossiers::ouvrir_exercice_sur(&mut base, "2021-02-01".into(), "2022-01-31".into()).expect("ouvrir");
    dossiers::clore_exercice_sur(&mut base, premier).expect("clore");
    dossiers::verifier_date_sur(&mut base, "2021-06-01").expect("vérifier");
    base.choisir_dossier(dossiers::DOSSIER_DEFAUT).unwrap();
    let poste = postes::inscrire_ou_retrouver_sur(&mut base, "Caisse 1", "emp-1", "caisse", None).unwrap();
    let (sid, ..) = sessions::ouvrir_dans_dossier_sur(&mut base, &poste.id, &utilisateur, None).unwrap();
    sessions::choisir_dossier_session_sur(&mut base, &sid, dossiers::DOSSIER_DEFAUT).expect("choisir");
    sessions::etat_sur(&mut base, &sid);
}

/// D21 : « Tu ouvres le dossier Quincaillerie le 15 mars, tu dis : je
/// travaille dedans du 1er mars 2026 au 28 fevrier 2027. » Les dates
/// sont son premier exercice ; une ecriture hors de ces dates est
/// refusee avec la borne et ce qu'il faut faire.
#[test]
fn un_dossier_nait_avec_ses_dates_de_travail_et_refuse_ce_qui_en_sort() {
    let mut base = base_avec_demo();
    let r = dossiers::creer_dossier_sur(
        &mut base,
        "QUINC".into(),
        "Quincaillerie".into(),
        Some("2026-03-01".into()),
        Some("2027-02-28".into()),
    )
    .unwrap();
    assert_eq!(r["date_debut"], "2026-03-01");
    assert_eq!(r["date_fin"], "2027-02-28");
    let id = r["id"].as_str().unwrap().to_string();

    base.choisir_dossier(&id).unwrap();
    let ex = dossiers::lire_exercices_sur(&mut base).unwrap();
    assert_eq!(ex.len(), 1);
    assert_eq!(ex[0]["date_debut"], "2026-03-01");
    assert_eq!(ex[0]["date_fin"], "2027-02-28");

    assert!(dossiers::verifier_date_sur(&mut base, "2026-03-01").is_ok(), "le premier jour compte");
    assert!(dossiers::verifier_date_sur(&mut base, "2027-02-28T18:00:00").is_ok(), "le dernier aussi");
    assert_eq!(
        dossiers::verifier_date_sur(&mut base, "2027-04-05").unwrap_err(),
        "Le 5 avril 2027 est hors des dates de travail (jusqu'au 28 février 2027). \
         Prolonger l'exercice ou en ouvrir un nouveau."
    );
    assert!(
        dossiers::verifier_date_sur(&mut base, "2026-02-27").unwrap_err().contains("1er mars 2026"),
        "avant : le refus nomme le début"
    );

    // Le journal du dossier ou l'on a agi dit qui l'a cree.
    base.choisir_dossier(dossiers::DOSSIER_DEFAUT).unwrap();
    let n = compter(
        &mut base,
        "SELECT CAST(COUNT(*) AS BIGINT) FROM journal WHERE type_evenement = 'dossier_cree' AND dossier_id = ?1",
        &parametres![dossiers::DOSSIER_DEFAUT],
    );
    assert_eq!(n, 1);
}

fn exercices_de(base: &mut Base, dossier: &str) -> Vec<serde_json::Value> {
    base.choisir_dossier(dossier).unwrap();
    dossiers::lire_exercices_sur(base).unwrap()
}

fn au_journal(base: &mut Base, dossier: &str, type_evenement: &str) -> i64 {
    compter(
        base,
        "SELECT CAST(COUNT(*) AS BIGINT) FROM journal WHERE type_evenement = ?1 AND dossier_id = ?2",
        &parametres![type_evenement, dossier],
    )
}

/// D21 : « En janvier 2027 tu n'as pas fini les papiers : tu prolonges
/// jusqu'au 31 mars. » La prolongation va plus loin, jamais jusque dans
/// l'exercice suivant, et jamais pour l'exercice d'une autre societe.
#[test]
fn prolonger_repousse_la_borne_et_ne_mord_jamais_sur_la_suite() {
    let mut base = base_avec_demo();
    let r = dossiers::creer_dossier_sur(
        &mut base,
        "QUINC".into(),
        "Quincaillerie".into(),
        Some("2026-03-01".into()),
        Some("2027-02-28".into()),
    )
    .unwrap();
    let quinc = r["id"].as_str().unwrap().to_string();
    let ex = exercices_de(&mut base, &quinc);
    let premier = ex[0]["id"].as_str().unwrap().to_string();

    assert!(dossiers::verifier_date_sur(&mut base, "2027-03-15").is_err(), "avant la prolongation");
    dossiers::prolonger_exercice_sur(&mut base, premier.clone(), "2027-03-31".into()).expect("prolonger");
    assert!(dossiers::verifier_date_sur(&mut base, "2027-03-15").is_ok(), "après : le 15 mars s'écrit");
    let ex = dossiers::lire_exercices_sur(&mut base).unwrap();
    assert_eq!(ex[0]["date_fin"], "2027-02-28", "la fin donnée reste lisible");
    assert_eq!(ex[0]["fin_effective"], "2027-03-31");
    assert!(
        dossiers::verifier_date_sur(&mut base, "2027-04-05").unwrap_err().contains("prolongation comprise"),
        "au-delà, le refus dit que la prolongation compte"
    );

    // Une prolongation plus courte n'est pas une prolongation.
    assert_eq!(
        dossiers::prolonger_exercice_sur(&mut base, premier.clone(), "2027-03-20".into()).unwrap_err(),
        "L'exercice va déjà jusqu'au 31 mars 2027 : une prolongation va plus loin."
    );
    // L'exercice suivant commence le lendemain ; on ne prolonge plus dedans.
    dossiers::ouvrir_exercice_sur(&mut base, "2027-04-01".into(), "2028-03-31".into()).expect("le suivant");
    assert_eq!(
        dossiers::prolonger_exercice_sur(&mut base, premier.clone(), "2027-04-10".into()).unwrap_err(),
        "L'exercice suivant commence le 1er avril 2027 : la prolongation doit s'arrêter avant."
    );
    assert!(dossiers::prolonger_exercice_sur(&mut base, premier.clone(), "10/04/2027".into())
        .unwrap_err()
        .contains("illisible"));

    // Depuis le dossier d'origine, l'exercice de la quincaillerie n'existe pas.
    base.choisir_dossier(dossiers::DOSSIER_DEFAUT).unwrap();
    assert_eq!(
        dossiers::prolonger_exercice_sur(&mut base, premier.clone(), "2027-03-31".into()).unwrap_err(),
        "Exercice introuvable."
    );
    assert_eq!(dossiers::clore_exercice_sur(&mut base, premier).unwrap_err(), "Exercice introuvable.");

    assert_eq!(au_journal(&mut base, &quinc, "exercice_prolonge"), 1, "une seule prolongation a eu lieu");
    assert_eq!(au_journal(&mut base, &quinc, "exercice_ouvert"), 1);
}

/// Le comptable clot 2026. Le 15 juin 2026 ne s'ecrit plus ; le 1er
/// janvier 2027 — le lendemain — non plus, tant que l'exercice suivant
/// n'est pas ouvert ; et il s'ouvre le lendemain, sans trou.
#[test]
fn apres_la_cloture_le_lendemain_attend_l_exercice_suivant() {
    let mut base = base_avec_demo();
    let r = dossiers::creer_dossier_sur(
        &mut base,
        "BOUT".into(),
        "Boutique".into(),
        Some("2026-01-01".into()),
        Some("2026-12-31".into()),
    )
    .unwrap();
    let bout = r["id"].as_str().unwrap().to_string();
    let ex2026 = exercices_de(&mut base, &bout)[0]["id"].as_str().unwrap().to_string();

    dossiers::clore_exercice_sur(&mut base, ex2026.clone()).expect("clore");
    assert_eq!(
        dossiers::verifier_date_sur(&mut base, "2026-06-15").unwrap_err(),
        "Les dates de travail du 1er janvier 2026 au 31 décembre 2026 sont closes : on n'y écrit plus."
    );
    assert!(
        dossiers::verifier_date_sur(&mut base, "2027-01-01").unwrap_err().contains("aucun exercice ouvert"),
        "le lendemain de la clôture : rien ne le couvre encore"
    );
    assert_eq!(dossiers::clore_exercice_sur(&mut base, ex2026.clone()).unwrap_err(), "Cet exercice est déjà clos.");
    assert!(dossiers::prolonger_exercice_sur(&mut base, ex2026.clone(), "2027-01-31".into())
        .unwrap_err()
        .contains("clos"));

    // Pas de trou, pas de chevauchement.
    assert_eq!(
        dossiers::ouvrir_exercice_sur(&mut base, "2027-01-05".into(), "2027-12-31".into()).unwrap_err(),
        "Un exercice commence le lendemain du précédent : le 1er janvier 2027. Pas de trou dans les dates de travail."
    );
    assert!(dossiers::ouvrir_exercice_sur(&mut base, "2026-12-01".into(), "2027-11-30".into())
        .unwrap_err()
        .starts_with("Chevauche l'exercice du 1er janvier 2026"));
    assert!(dossiers::ouvrir_exercice_sur(&mut base, "2027-01-01".into(), "2026-12-31".into())
        .unwrap_err()
        .contains("précède"));

    let r = dossiers::ouvrir_exercice_sur(&mut base, "2027-01-01".into(), "2027-12-31".into()).expect("le suivant");
    assert_eq!(r["date_debut"], "2027-01-01");
    assert!(dossiers::verifier_date_sur(&mut base, "2027-01-01").is_ok(), "le lendemain s'écrit enfin");
    assert!(dossiers::verifier_date_sur(&mut base, "2026-06-15").is_err(), "2026 reste clos");

    let ex = dossiers::lire_exercices_sur(&mut base).unwrap();
    assert_eq!(ex.len(), 2);
    assert_eq!(ex[0]["clos"], true);
    assert_eq!(ex[1]["clos"], false);
    assert_eq!(au_journal(&mut base, &bout, "exercice_clos"), 1);
    assert_eq!(au_journal(&mut base, &bout, "exercice_ouvert"), 1);
    // Le dossier d'origine n'a rien vu.
    assert_eq!(au_journal(&mut base, dossiers::DOSSIER_DEFAUT, "exercice_clos"), 0);
}
