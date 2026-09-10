//! Le stock est la conséquence de ses mouvements.
//!
//! `stock_depot.quantite` était un compteur qu'on incrémentait à onze
//! endroits. Un compteur ne sait pas dire POURQUOI il vaut ce qu'il
//! vaut : quand un stock est faux, il n'y a rien à auditer. Et rien
//! n'empêchait un chemin d'écrire le compteur sans son mouvement —
//! l'import CSV le faisait — après quoi le stock et le journal
//! racontaient deux histoires différentes.
//!
//! Ces scénarios vérifient les trois promesses du changement : le
//! compteur suit, il ne peut plus diverger, et la reprise d'une base
//! existante ne change rien à ce que le commerçant voit.

use rusqlite::Connection;

use gescom_noyau::persistance;

fn base() -> Connection {
    let conn = Connection::open_in_memory().expect("base en mémoire");
    persistance::initialiser_tables(&conn).expect("schéma");
    conn.execute(
        "INSERT INTO depot (id, nom, est_defaut, actif, cree_le, modifie_le, origine)
         VALUES ('d1', 'Principal', 1, 1, '2026-01-01', '2026-01-01', 'test')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO article (id, nom, unite_base, actif, cree_le, modifie_le, origine)
         VALUES ('a1', 'Ciment', 'sac', 1, '2026-01-01', '2026-01-01', 'test')",
        [],
    )
    .unwrap();
    conn
}

fn mouvement(conn: &Connection, delta: f64, type_mouvement: &str) {
    conn.execute(
        "INSERT INTO mouvement_stock
         (id, article_id, depot_id, type_mouvement, quantite_delta,
          auteur_id, date_mouvement, cree_le, cree_par, origine)
         VALUES (?1, 'a1', 'd1', ?2, ?3, 't', '2026-01-01', '2026-01-01', 't', 'test')",
        rusqlite::params![uuid::Uuid::new_v4().to_string(), type_mouvement, delta],
    )
    .unwrap();
}

fn compteur(conn: &Connection) -> f64 {
    conn.query_row(
        "SELECT COALESCE(quantite, 0) FROM stock_depot
         WHERE article_id = 'a1' AND depot_id = 'd1'",
        [],
        |r| r.get(0),
    )
    .unwrap_or(0.0)
}

fn somme(conn: &Connection) -> f64 {
    conn.query_row(
        "SELECT COALESCE(SUM(quantite_delta), 0) FROM mouvement_stock
         WHERE article_id = 'a1' AND depot_id = 'd1'",
        [],
        |r| r.get(0),
    )
    .unwrap_or(0.0)
}

#[test]
fn le_compteur_suit_le_premier_mouvement() {
    // La ligne de stock n'existe pas encore : le declencheur doit la
    // creer, pas echouer en silence.
    let conn = base();
    assert_eq!(compteur(&conn), 0.0);
    mouvement(&conn, 50.0, "entree");
    assert_eq!(compteur(&conn), 50.0);
}

#[test]
fn le_compteur_suit_les_mouvements_suivants() {
    let conn = base();
    mouvement(&conn, 50.0, "entree");
    mouvement(&conn, -12.0, "vente");
    mouvement(&conn, 3.0, "retour");
    assert_eq!(compteur(&conn), 41.0);
    assert_eq!(compteur(&conn), somme(&conn), "le cache vaut la somme");
}

#[test]
fn le_compteur_descend_sous_zero_comme_avant() {
    // Une vente a decouvert reste possible : le client attend au
    // comptoir, c'est le stock informatique qui a du retard. Le
    // declencheur ne doit pas transformer cette regle metier en refus
    // technique.
    let conn = base();
    mouvement(&conn, 3.0, "entree");
    mouvement(&conn, -10.0, "vente");
    assert_eq!(compteur(&conn), -7.0);
}

#[test]
fn le_stock_de_deux_depots_ne_se_melange_pas() {
    let conn = base();
    conn.execute(
        "INSERT INTO depot (id, nom, est_defaut, actif, cree_le, modifie_le, origine)
         VALUES ('d2', 'Annexe', 0, 1, '2026-01-01', '2026-01-01', 'test')",
        [],
    )
    .unwrap();
    mouvement(&conn, 50.0, "entree");
    conn.execute(
        "INSERT INTO mouvement_stock
         (id, article_id, depot_id, type_mouvement, quantite_delta,
          auteur_id, date_mouvement, cree_le, cree_par, origine)
         VALUES ('m-annexe', 'a1', 'd2', 'entree', 8.0, 't',
                 '2026-01-01', '2026-01-01', 't', 'test')",
        [],
    )
    .unwrap();

    assert_eq!(compteur(&conn), 50.0, "le principal ne bouge pas");
    let annexe: f64 = conn
        .query_row(
            "SELECT quantite FROM stock_depot WHERE depot_id = 'd2'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(annexe, 8.0);
}

#[test]
fn une_base_coherente_ne_signale_aucune_anomalie() {
    let conn = base();
    mouvement(&conn, 50.0, "entree");
    mouvement(&conn, -12.0, "vente");
    let anomalies = persistance::anomalies_metier(&conn);
    assert!(
        !anomalies.iter().any(|(l, _)| l.contains("correspondent pas")),
        "{anomalies:?}"
    );
}

#[test]
fn une_ecriture_directe_du_compteur_est_signalee() {
    // C'est la contrepartie du changement : avant, un stock faux
    // n'avait rien derriere lui a comparer. Maintenant si.
    let conn = base();
    mouvement(&conn, 50.0, "entree");
    conn.execute(
        "UPDATE stock_depot SET quantite = 999 WHERE article_id = 'a1'",
        [],
    )
    .unwrap();

    let anomalies = persistance::anomalies_metier(&conn);
    let ecart = anomalies
        .iter()
        .find(|(l, _)| l.contains("correspondent pas"));
    assert!(ecart.is_some(), "l'écart doit être signalé : {anomalies:?}");
    assert_eq!(ecart.unwrap().1, 1);
}

// =====================================================================
//  La reprise d'une base existante
// =====================================================================

/// Une base v1 a derive : le compteur ne vaut pas la somme.
///
/// La reprise doit faire rejoindre la SOMME au COMPTEUR, jamais
/// l'inverse. Ce que le commerçant voit aujourd'hui doit rester ce
/// qu'il verra demain — sinon il conclut à une perte de marchandise le
/// matin de la mise à jour.
#[test]
fn la_reprise_conserve_ce_que_le_commercant_voit() {
    let conn = Connection::open_in_memory().unwrap();
    // Schema seul, sans les migrations v2 : on simule une base v1.
    conn.execute_batch(include_str!("../src/persistance/schema.sql"))
        .unwrap();
    conn.execute(
        "INSERT INTO depot (id, nom, est_defaut, actif, cree_le, modifie_le, origine)
         VALUES ('d1', 'Principal', 1, 1, '2026-01-01', '2026-01-01', 'test')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO article (id, nom, unite_base, actif, cree_le, modifie_le, origine)
         VALUES ('a1', 'Ciment', 'sac', 1, '2026-01-01', '2026-01-01', 'test')",
        [],
    )
    .unwrap();

    // Le compteur dit 40, l'historique n'en explique que 30 : c'est
    // exactement ce que produisait un import CSV.
    conn.execute(
        "INSERT INTO stock_depot (id, article_id, depot_id, quantite)
         VALUES ('s1', 'a1', 'd1', 40)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO mouvement_stock
         (id, article_id, depot_id, type_mouvement, quantite_delta,
          auteur_id, date_mouvement, cree_le, cree_par, origine)
         VALUES ('m1', 'a1', 'd1', 'entree', 30, 't',
                 '2026-01-01', '2026-01-01', 't', 'test')",
        [],
    )
    .unwrap();

    persistance::v2::migrer(&conn).unwrap();

    assert_eq!(compteur(&conn), 40.0, "le stock affiche ne bouge pas");
    assert_eq!(somme(&conn), 40.0, "l'historique explique enfin les 40");

    // Et l'ecart devient une ligne visible, pas un mystere.
    let repris: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM mouvement_stock
             WHERE origine = 'migration' AND quantite_delta = 10",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(repris, 1);
}

#[test]
fn la_reprise_ne_se_joue_qu_une_fois() {
    let conn = base();
    mouvement(&conn, 50.0, "entree");
    let avant: i64 = conn
        .query_row("SELECT COUNT(*) FROM mouvement_stock", [], |r| r.get(0))
        .unwrap();

    // Rejouer les migrations, comme a chaque demarrage.
    persistance::v2::migrer(&conn).unwrap();
    persistance::v2::migrer(&conn).unwrap();

    let apres: i64 = conn
        .query_row("SELECT COUNT(*) FROM mouvement_stock", [], |r| r.get(0))
        .unwrap();
    assert_eq!(avant, apres, "aucun ajustement en double");
    assert_eq!(compteur(&conn), 50.0);
}
