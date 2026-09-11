//! Une base neuve doit etre utilisable tout de suite.
//!
//! L'amorcage vivait dans le crate applicatif : seule la fenetre le
//! declenchait. `gescom-serveur.exe` ouvrant une base neuve n'avait NI
//! role NI compte — il demarrait, ecoutait, et refusait toutes les
//! connexions avec « Identifiant ou mot de passe incorrect », sans
//! qu'aucun ecran ne permette de creer le premier compte.
//!
//! Ces scenarios verifient les trois promesses de l'amorcage : il donne
//! de quoi se connecter, il ne se rejoue jamais, et il ne fabrique pas
//! de donnees fictives chez un commercant.

use rusqlite::Connection;

use gescom_noyau::{auth, persistance, seed};

fn base_neuve() -> Connection {
    let conn = Connection::open_in_memory().expect("base en mémoire");
    persistance::initialiser_tables(&conn).expect("schéma");
    conn
}

fn compte(conn: &Connection, table: &str) -> i64 {
    conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
        .unwrap_or(-1)
}

#[test]
fn une_base_neuve_est_vide() {
    // « Vide » veut dire : aucun moyen d'entrer. Pas « aucune ligne » —
    // les migrations posent des roles livres, et confondre les deux
    // faisait croire qu'une base neuve etait deja amorcee.
    let conn = base_neuve();
    assert!(seed::base_est_vide(&conn));
    assert_eq!(compte(&conn, "utilisateur_auth"), 0);
    assert!(compte(&conn, "role") > 0, "les rôles livrés sont là");
}

#[test]
fn l_amorcage_donne_de_quoi_se_connecter() {
    // C'est LA promesse : apres l'amorcage, quelqu'un peut entrer.
    let conn = base_neuve();
    assert!(seed::amorcer_si_vide(&conn).expect("amorçage"));

    let patron = auth::connexion(&conn, "admin".to_string(), "admin123".to_string())
        .expect("le patron doit pouvoir se connecter");
    assert_eq!(patron["role"], "patron");
    assert_eq!(
        patron["doit_changer_mdp"], true,
        "un mot de passe d'usine doit se changer à la première connexion"
    );

    let employe =
        auth::connexion(&conn, "employe".to_string(), "employe123".to_string())
            .expect("l'employé aussi");
    assert_eq!(employe["role"], "employe");
}

#[test]
fn un_mot_de_passe_faux_est_refuse() {
    // Sans cette verification, un amorcage qui ecrirait un hash casse
    // laisserait passer n'importe quoi.
    let conn = base_neuve();
    seed::amorcer_si_vide(&conn).unwrap();
    assert!(auth::connexion(&conn, "admin".to_string(), "admin".to_string()).is_err());
}

#[test]
fn l_amorcage_pose_le_minimum_pour_vendre() {
    // Sans depot par defaut, aucune vente ne sort de stock ; sans
    // client generique, la vente au comptoir n'a personne a qui
    // s'attacher.
    let conn = base_neuve();
    seed::amorcer_si_vide(&conn).unwrap();

    let depot: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM depot WHERE est_defaut = 1 AND actif = 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(depot, 1, "un dépôt par défaut, et un seul");

    let generique: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM client WHERE est_generique = 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(generique, 1);

    assert_eq!(compte(&conn, "parametres_societe"), 1);
}

#[test]
fn l_amorcage_ne_se_rejoue_pas() {
    // Il tourne a CHAQUE demarrage. S'il rejouait, chaque lancement
    // ajouterait un depot par defaut et un client comptant de plus.
    let conn = base_neuve();
    assert!(seed::amorcer_si_vide(&conn).unwrap());

    let avant = (
        compte(&conn, "role"),
        compte(&conn, "utilisateur"),
        compte(&conn, "depot"),
        compte(&conn, "client"),
    );

    assert!(!seed::amorcer_si_vide(&conn).unwrap(), "il ne rejoue pas");
    assert!(!seed::amorcer_si_vide(&conn).unwrap());

    let apres = (
        compte(&conn, "role"),
        compte(&conn, "utilisateur"),
        compte(&conn, "depot"),
        compte(&conn, "client"),
    );
    assert_eq!(avant, apres);
}

#[test]
fn sans_gescom_demo_aucun_article_fictif() {
    // Chez un commercant, des articles de demo seraient a supprimer un
    // par un. On ne les cree donc jamais sans le demander.
    //
    // La variable d'environnement est partagee par tous les tests du
    // binaire : on ne la modifie pas ici, on verifie seulement que le
    // defaut est bien « rien ».
    if std::env::var("GESCOM_DEMO").as_deref() == Ok("1") {
        return;
    }
    let conn = base_neuve();
    seed::amorcer_si_vide(&conn).unwrap();

    assert_eq!(compte(&conn, "article"), 0);
    let clients_reels: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM client WHERE est_generique = 0",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(clients_reels, 0, "aucun client fictif");
}

#[test]
fn une_base_deja_peuplee_n_est_pas_amorcee() {
    // Le garde-fou qui compte le plus : reamorcer une base existante
    // recreerait des comptes d'usine avec des mots de passe connus.
    let conn = base_neuve();
    conn.execute(
        "INSERT INTO role (id, nom, cree_le, modifie_le)
         VALUES ('r1', 'le-role-du-commercant', '2026-01-01', '2026-01-01')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO utilisateur (id, nom, role_id, actif, cree_le, modifie_le, origine)
         VALUES ('u1', 'Le patron', 'r1', 1, '2026-01-01', '2026-01-01', 'app')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO utilisateur_auth
           (utilisateur_id, pseudo, mot_de_passe, doit_changer_mdp)
         VALUES ('u1', 'lepatron', 'un-hash', 0)",
        [],
    )
    .unwrap();

    assert!(!seed::base_est_vide(&conn));
    assert!(!seed::amorcer_si_vide(&conn).unwrap());
    assert_eq!(
        compte(&conn, "utilisateur_auth"),
        1,
        "aucun compte d'usine ajouté"
    );
}
