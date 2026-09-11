//! Le fond d'ouverture ne se compte qu'une fois.
//!
//! Il vit a DEUX endroits : la colonne `session_caisse.fond_ouverture`,
//! et un mouvement `motif = 'ouverture'` qui existe pour la trace.
//! C'est voulu — le journal de caisse doit montrer d'ou vient l'argent
//! du tiroir au matin.
//!
//! Mais trois calculs sur quatre additionnaient les deux. Un fond de
//! 10 000 et 3 000 de ventes donnaient 23 000 au lieu de 13 000 : le
//! commercant comptait son tiroir, trouvait 13 000, et l'application
//! lui annoncait un manque de 10 000. Tous les soirs, sur toutes les
//! caisses.
//!
//! Ces scenarios verifient les quatre chemins qui lisent ce solde.

use rusqlite::Connection;

use gescom_noyau::{caisse, persistance, tableau_bord};

const FOND: i64 = 10_000;

fn base() -> Connection {
    let conn = Connection::open_in_memory().expect("base en mémoire");
    persistance::initialiser_tables(&conn).expect("schéma");
    conn.execute_batch(
        "INSERT INTO role (id, nom, cree_le, modifie_le)
           VALUES ('r1', 'patron', '2026-01-01', '2026-01-01');
         INSERT INTO utilisateur (id, nom, role_id, actif, cree_le, modifie_le, origine)
           VALUES ('u1', 'Patron', 'r1', 1, '2026-01-01', '2026-01-01', 'test');",
    )
    .unwrap();
    conn
}

/// Ouvre la caisse puis y fait entrer `ventes` francs en especes.
fn caisse_avec(conn: &Connection, ventes: i64) -> String {
    let session = caisse::ouvrir_session_caisse(conn, FOND, "patron".to_string())
        .expect("caisse ouverte");

    if ventes > 0 {
        conn.execute(
            "INSERT INTO mouvement_caisse
               (id, session_id, sens, moyen, montant, motif,
                date_mouvement, cree_le, cree_par, origine)
             VALUES ('m-v', ?1, 'entree', 'especes', ?2, 'vente',
                     '2026-01-01', '2026-01-01', 'u1', 'test')",
            rusqlite::params![session, ventes],
        )
        .unwrap();
    }
    session
}

#[test]
fn le_fond_existe_bien_en_double_et_c_est_voulu() {
    // La trace : le journal de caisse doit montrer d'ou vient l'argent
    // du tiroir au matin. C'est cette trace qui rend le piege possible.
    let conn = base();
    let session = caisse_avec(&conn, 0);

    let colonne: i64 = conn
        .query_row(
            "SELECT fond_ouverture FROM session_caisse WHERE id = ?1",
            rusqlite::params![session],
            |r| r.get(0),
        )
        .unwrap();
    let mouvement: i64 = conn
        .query_row(
            "SELECT COALESCE(SUM(montant), 0) FROM mouvement_caisse
             WHERE session_id = ?1 AND motif = 'ouverture'",
            rusqlite::params![session],
            |r| r.get(0),
        )
        .unwrap();

    assert_eq!(colonne, FOND);
    assert_eq!(mouvement, FOND, "le mouvement de trace existe");
}

#[test]
fn l_ecran_caisse_ne_compte_le_fond_qu_une_fois() {
    let conn = base();
    caisse_avec(&conn, 3_000);

    let v = caisse::lire_resume_caisse(&conn).expect("caisse lue");
    assert_eq!(v["fond_ouverture"], FOND);
    assert_eq!(
        v["entrees_especes"], 3_000,
        "les entrées comptent les ventes, pas le fond"
    );
    assert_eq!(
        v["solde_theorique"],
        FOND + 3_000,
        "10 000 de fond + 3 000 de ventes = 13 000, pas 23 000"
    );
}

#[test]
fn le_tableau_de_bord_donne_le_meme_solde_que_l_ecran_caisse() {
    // Deux ecrans qui annoncent deux soldes differents pour le meme
    // tiroir, c'est pire qu'un seul faux : le commercant ne sait plus
    // lequel croire.
    let conn = base();
    caisse_avec(&conn, 3_000);

    let caisse_ecran = caisse::lire_resume_caisse(&conn).unwrap();
    let bord = tableau_bord::lire_resume_dashboard(&conn, None).unwrap();

    assert_eq!(bord["caisse_solde"], FOND + 3_000);
    assert_eq!(bord["caisse_solde"], caisse_ecran["solde_theorique"]);
}

#[test]
fn la_cloture_ne_reclame_pas_un_manque_qui_n_existe_pas() {
    // Le scenario qui coute le plus cher : le caissier compte son
    // tiroir, trouve exactement ce qu'il doit y avoir, et l'application
    // lui annonce qu'il manque le montant du fond.
    let conn = base();
    let session = caisse_avec(&conn, 3_000);

    caisse::fermer_session_caisse(&conn, session.clone(), FOND + 3_000)
        .expect("caisse fermée");

    let (solde, ecart): (i64, i64) = conn
        .query_row(
            "SELECT solde_theorique, ecart FROM session_caisse WHERE id = ?1",
            rusqlite::params![session],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(solde, FOND + 3_000);
    assert_eq!(ecart, 0, "le tiroir est juste, l'écart doit être nul");
}

#[test]
fn un_vrai_manque_reste_signale() {
    // La contrepartie : en corrigeant le double comptage, on ne doit
    // pas avoir rendu la cloture aveugle.
    let conn = base();
    let session = caisse_avec(&conn, 3_000);

    caisse::fermer_session_caisse(&conn, session.clone(), FOND + 3_000 - 500)
        .expect("caisse fermée");

    let ecart: i64 = conn
        .query_row(
            "SELECT ecart FROM session_caisse WHERE id = ?1",
            rusqlite::params![session],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(ecart, -500, "500 F manquent, il faut le dire");
}

#[test]
fn une_caisse_sans_fond_se_comporte_pareil() {
    // Fond a zero : aucun mouvement d'ouverture n'est ecrit. Le calcul
    // ne doit pas dependre de sa presence.
    let conn = base();
    let session = caisse::ouvrir_session_caisse(&conn, 0, "patron".to_string()).unwrap();
    conn.execute(
        "INSERT INTO mouvement_caisse
           (id, session_id, sens, moyen, montant, motif,
            date_mouvement, cree_le, cree_par, origine)
         VALUES ('m-v', ?1, 'entree', 'especes', 2500, 'vente',
                 '2026-01-01', '2026-01-01', 'u1', 'test')",
        rusqlite::params![session],
    )
    .unwrap();

    let ecran = caisse::lire_resume_caisse(&conn).unwrap();
    assert_eq!(ecran["solde_theorique"], 2_500);
}
