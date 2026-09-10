//! Les scenarios du multiposte, joues sur une base en memoire.
//!
//! Ce sont les regles qu'on ne peut pas verifier a l'oeil : une session
//! revoquee doit tomber tout de suite, un poste desactive doit couper
//! ses sessions, et la caisse nominative ne doit pas servir le tiroir
//! du voisin. Chacune de ces trois erreurs se solde par un compte faux
//! que personne ne rattache a sa cause.

use rusqlite::Connection;

use gescom_noyau::{caisses, persistance, postes, sessions};

fn base() -> Connection {
    let conn = Connection::open_in_memory().expect("base en mémoire");
    conn.execute_batch("PRAGMA foreign_keys=ON;").ok();
    persistance::initialiser_tables(&conn).expect("schéma");
    conn
}

fn creer_utilisateur(conn: &Connection, id: &str, nom: &str, role: &str) {
    let maintenant = gescom_noyau::utils::maintenant_iso();
    conn.execute(
        "INSERT OR IGNORE INTO role (id, nom, cree_le, modifie_le)
         VALUES (?1, ?1, ?2, ?2)",
        rusqlite::params![role, maintenant],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO utilisateur (id, nom, role_id, actif, cree_le, modifie_le)
         VALUES (?1, ?2, ?3, 1, ?4, ?4)",
        rusqlite::params![id, nom, role, maintenant],
    )
    .unwrap();
}

fn poste_test(conn: &Connection, nom: &str) -> String {
    postes::inscrire_ou_retrouver(conn, nom, &format!("emp-{nom}"), "caisse", None)
        .unwrap()
        .id
}

#[test]
fn un_poste_inscrit_deux_fois_reste_le_meme() {
    let conn = base();
    let a = poste_test(&conn, "Caisse 1");
    let b = poste_test(&conn, "Caisse 1");
    // Sinon chaque redemarrage du poste ajouterait une ligne, et la
    // liste des machines du magasin deviendrait illisible en une
    // semaine.
    assert_eq!(a, b);
    assert_eq!(postes::lister(&conn).unwrap().len(), 1);
}

#[test]
fn une_session_ouverte_est_valide() {
    let conn = base();
    creer_utilisateur(&conn, "u1", "Awa", "employe");
    let poste = poste_test(&conn, "Caisse 1");
    let (session_id, jeton, _) = sessions::ouvrir(&conn, &poste, "u1").unwrap();

    // 64 caracteres hexadecimaux : 256 bits d'alea. Un jeton plus
    // court se devine.
    assert_eq!(jeton.len(), 64);
    assert!(matches!(
        sessions::etat(&conn, &session_id),
        sessions::Etat::Valide { .. }
    ));
    assert_eq!(sessions::lister_actives(&conn).unwrap().len(), 1);
}

#[test]
fn une_session_revoquee_tombe_immediatement() {
    let conn = base();
    creer_utilisateur(&conn, "u1", "Awa", "employe");
    let poste = poste_test(&conn, "Caisse 1");
    let (session_id, _, _) = sessions::ouvrir(&conn, &poste, "u1").unwrap();

    sessions::revoquer(&conn, &session_id, "patron").unwrap();
    assert!(matches!(
        sessions::etat(&conn, &session_id),
        sessions::Etat::Revoquee
    ));
    assert!(sessions::lister_actives(&conn).unwrap().is_empty());
}

#[test]
fn desactiver_un_poste_coupe_ses_sessions() {
    let conn = base();
    creer_utilisateur(&conn, "u1", "Awa", "employe");
    let poste = poste_test(&conn, "Caisse 1");
    let (session_id, _, _) = sessions::ouvrir(&conn, &poste, "u1").unwrap();

    postes::desactiver(&conn, &poste, "patron").unwrap();
    // Le poste disparait du magasin : sa session ne doit pas lui
    // survivre huit heures.
    assert!(matches!(
        sessions::etat(&conn, &session_id),
        sessions::Etat::Revoquee
    ));
}

#[test]
fn desactiver_un_utilisateur_coupe_sa_session() {
    let conn = base();
    creer_utilisateur(&conn, "u1", "Awa", "employe");
    let poste = poste_test(&conn, "Caisse 1");
    let (session_id, _, _) = sessions::ouvrir(&conn, &poste, "u1").unwrap();

    conn.execute("UPDATE utilisateur SET actif = 0 WHERE id = 'u1'", [])
        .unwrap();
    assert!(matches!(
        sessions::etat(&conn, &session_id),
        sessions::Etat::Revoquee
    ));
}

#[test]
fn une_session_expiree_ne_passe_plus() {
    let conn = base();
    creer_utilisateur(&conn, "u1", "Awa", "employe");
    let poste = poste_test(&conn, "Caisse 1");
    let (session_id, _, _) = sessions::ouvrir(&conn, &poste, "u1").unwrap();

    conn.execute(
        "UPDATE session_reseau SET expire_le = '2000-01-01T00:00:00.000' WHERE id = ?1",
        rusqlite::params![session_id],
    )
    .unwrap();
    assert!(matches!(
        sessions::etat(&conn, &session_id),
        sessions::Etat::Expiree
    ));
}

#[test]
fn le_redemarrage_ferme_toutes_les_sessions() {
    let conn = base();
    creer_utilisateur(&conn, "u1", "Awa", "employe");
    creer_utilisateur(&conn, "u2", "Moussa", "employe");
    let p1 = poste_test(&conn, "Caisse 1");
    let p2 = poste_test(&conn, "Caisse 2");
    sessions::ouvrir(&conn, &p1, "u1").unwrap();
    sessions::ouvrir(&conn, &p2, "u2").unwrap();

    // La correspondance jeton -> session vit en memoire : apres un
    // redemarrage elle est perdue, et laisser les sessions « actives »
    // en base ferait mentir la liste des postes connectes.
    sessions::revoquer_toutes(&conn, "redemarrage").unwrap();
    assert!(sessions::lister_actives(&conn).unwrap().is_empty());
}

// =====================================================================
//  La caisse
// =====================================================================

fn ouvrir_caisse(conn: &Connection, id: &str, utilisateur: Option<&str>) {
    let maintenant = gescom_noyau::utils::maintenant_iso();
    conn.execute(
        "INSERT INTO session_caisse
           (id, statut, fond_ouverture, ouvert_par, utilisateur_id, cree_le, modifie_le)
         VALUES (?1, 'ouverte', 0, ?2, ?2, ?3, ?3)",
        rusqlite::params![id, utilisateur, maintenant],
    )
    .unwrap();
}

#[test]
fn par_defaut_la_caisse_reste_unique() {
    let conn = base();
    // Le v1 tenait pour acquis qu'il n'y a qu'un tiroir (D46). Une
    // migration qui changerait ce defaut obligerait chaque commercant
    // a ouvrir deux caisses pour un seul comptoir.
    assert!(!caisses::par_utilisateur(&conn));

    assert!(caisses::exiger(&conn, None).is_err());
    ouvrir_caisse(&conn, "c1", None);
    assert_eq!(caisses::exiger(&conn, None).unwrap(), "c1");
    // Peu importe qui demande : c'est le tiroir du comptoir.
    assert_eq!(caisses::exiger(&conn, Some("u1")).unwrap(), "c1");
}

#[test]
fn en_mode_nominatif_chacun_son_tiroir() {
    let conn = base();
    creer_utilisateur(&conn, "u1", "Awa", "employe");
    creer_utilisateur(&conn, "u2", "Moussa", "employe");
    caisses::definir_par_utilisateur(&conn, true).unwrap();

    ouvrir_caisse(&conn, "c-awa", Some("u1"));
    assert_eq!(caisses::exiger(&conn, Some("u1")).unwrap(), "c-awa");

    // Moussa n'a pas ouvert : il ne doit PAS encaisser dans le tiroir
    // d'Awa. C'est tout l'interet du mode — l'ecart de cloture doit
    // rester imputable.
    assert!(caisses::exiger(&conn, Some("u2")).is_err());

    // Et une operation qui ne dit pas qui l'enregistre est refusee,
    // plutot que rattachee au premier tiroir venu.
    let err = caisses::exiger(&conn, None).unwrap_err();
    assert!(err.contains("CAISSE_SANS_UTILISATEUR"), "{err}");
}

#[test]
fn un_utilisateur_ne_peut_pas_ouvrir_deux_caisses() {
    let conn = base();
    creer_utilisateur(&conn, "u1", "Awa", "employe");
    caisses::definir_par_utilisateur(&conn, true).unwrap();
    ouvrir_caisse(&conn, "c-awa", Some("u1"));

    // Deux tiroirs ouverts au meme nom, c'est un fond de caisse compte
    // deux fois. L'index partiel doit le refuser en base, pas
    // seulement dans l'ecran.
    let deuxieme = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        ouvrir_caisse(&conn, "c-awa-2", Some("u1"));
    }));
    assert!(deuxieme.is_err());
}

#[test]
fn on_ne_change_pas_de_mode_caisse_ouverte() {
    let conn = base();
    ouvrir_caisse(&conn, "c1", None);
    // Basculer maintenant laisserait une session sans proprietaire
    // dans un monde ou tout en a un, et la cloture suivante ne saurait
    // a qui reclamer l'ecart.
    assert!(caisses::definir_par_utilisateur(&conn, true).is_err());
}
