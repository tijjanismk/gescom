//! `dossiers::*_sur` — la v3 commence : lister et créer des dossiers,
//! une session qui porte son dossier, le dossier mémorisé d'une
//! personne. Testé sur SQLite par défaut, sur PostgreSQL avec
//! `GESCOM_PG` — et c'est là que la création d'un dossier passe : sur
//! SQLite, elle refuse, exprès.

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
    let r = dossiers::creer_dossier_sur(&mut base, " quinc ".into(), " Quincaillerie du Fleuve ".into());

    if !base.est_postgres() {
        // Sur une base fichier, un second dossier serait servi de travers
        // par les commandes de la fenêtre : refus net, rien d'écrit.
        let e = r.unwrap_err();
        assert!(e.contains("PostgreSQL"), "{e}");
        assert_eq!(compter(&mut base, "SELECT COUNT(*) FROM dossier", &[]), 1);
        return;
    }

    let r = r.expect("dossier créé");
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
    let e = dossiers::creer_dossier_sur(&mut base, "quinc".into(), "Autre".into()).unwrap_err();
    assert!(e.contains("pris"), "{e}");
    // Un code impossible, un nom vide : refus, rien d'écrit.
    assert!(dossiers::creer_dossier_sur(&mut base, "a b".into(), "X".into()).is_err());
    assert!(dossiers::creer_dossier_sur(&mut base, "OK".into(), "  ".into()).is_err());
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
    if base.est_postgres() {
        dossiers::creer_dossier_sur(&mut base, "AUDIT".into(), "Audit".into()).expect("créer");
    }
    let poste = postes::inscrire_ou_retrouver_sur(&mut base, "Caisse 1", "emp-1", "caisse", None).unwrap();
    let (sid, ..) = sessions::ouvrir_dans_dossier_sur(&mut base, &poste.id, &utilisateur, None).unwrap();
    sessions::choisir_dossier_session_sur(&mut base, &sid, dossiers::DOSSIER_DEFAUT).expect("choisir");
    sessions::etat_sur(&mut base, &sid);
}
