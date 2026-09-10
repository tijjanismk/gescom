//! Les lectures du comptoir, jouées sur une base en mémoire.
//!
//! Ce sont les premières commandes métier que le serveur v2 sert aux
//! postes caisse. Ce qui doit être vrai ici l'est donc pour les deux
//! chemins d'un coup — comptoir et caisse.

use rusqlite::Connection;

use gescom_noyau::{catalogue, persistance};

fn base() -> Connection {
    let conn = Connection::open_in_memory().expect("base en mémoire");
    persistance::initialiser_tables(&conn).expect("schéma");
    conn
}

fn depot(conn: &Connection, id: &str, nom: &str, defaut: bool, actif: bool) {
    conn.execute(
        "INSERT INTO depot (id, nom, est_defaut, actif, cree_le, modifie_le, origine)
         VALUES (?1, ?2, ?3, ?4, '2026-01-01', '2026-01-01', 'test')",
        rusqlite::params![id, nom, defaut as i64, actif as i64],
    )
    .unwrap();
}

fn article(conn: &Connection, id: &str, nom: &str, prix_achat: i64) {
    conn.execute(
        "INSERT INTO article
         (id, nom, unite_base, actif, dernier_prix_achat,
          cree_le, modifie_le, origine)
         VALUES (?1, ?2, 'kg', 1, ?3, '2026-01-01', '2026-01-01', 'test')",
        rusqlite::params![id, nom, prix_achat],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO unite_vente
         (id, article_id, libelle, facteur, prix_reference, actif,
          cree_le, modifie_le, origine)
         VALUES (?1, ?2, 'Kilo', 1.0, 500, 1, '2026-01-01', '2026-01-01', 'test')",
        rusqlite::params![format!("u-{id}"), id],
    )
    .unwrap();
}

fn poser_stock(conn: &Connection, article_id: &str, depot_id: &str, q: f64) {
    conn.execute(
        "INSERT INTO stock_depot (id, article_id, depot_id, quantite)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(article_id, depot_id) DO UPDATE SET quantite = ?4",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(),
            article_id,
            depot_id,
            q
        ],
    )
    .unwrap();
}

// =====================================================================
//  Dépôts
// =====================================================================

#[test]
fn le_depot_par_defaut_vient_en_tete() {
    let conn = base();
    depot(&conn, "d-annexe", "Annexe", false, true);
    depot(&conn, "d-principal", "Principal", true, true);

    let v = catalogue::lire_depots(&conn).unwrap();
    // L'ordre n'est pas cosmétique : le premier de la liste est celui
    // que l'écran préselectionne au comptoir.
    assert_eq!(v[0]["id"], serde_json::json!("d-principal"));
    assert_eq!(v[0]["est_defaut"], serde_json::json!(true));
}

#[test]
fn un_depot_ferme_ne_figure_plus_au_catalogue() {
    let conn = base();
    depot(&conn, "d1", "Principal", true, true);
    depot(&conn, "d2", "Ancien magasin", false, false);

    let v = catalogue::lire_depots(&conn).unwrap();
    assert_eq!(v.len(), 1, "un dépôt fermé ne doit plus être proposé");
}

// =====================================================================
//  Articles
// =====================================================================

#[test]
fn le_stock_lu_est_celui_du_depot_demande() {
    let conn = base();
    depot(&conn, "d1", "Principal", true, true);
    depot(&conn, "d2", "Annexe", false, true);
    article(&conn, "a1", "Ciment", 4000);
    poser_stock(&conn, "a1", "d1", 10.0);
    poser_stock(&conn, "a1", "d2", 300.0);

    let lire = |d: &str| -> f64 {
        catalogue::lire_articles_avec_unites(&conn, None, Some(d.to_string()))
            .unwrap()[0]["stock"]
            .as_f64()
            .unwrap()
    };
    assert_eq!(lire("d1"), 10.0);
    assert_eq!(lire("d2"), 300.0);
}

#[test]
fn un_depot_inconnu_retombe_sur_le_defaut() {
    let conn = base();
    depot(&conn, "d1", "Principal", true, true);
    article(&conn, "a1", "Ciment", 4000);
    poser_stock(&conn, "a1", "d1", 7.0);

    // L'écran doit s'ouvrir même après désactivation du dépôt mémorisé
    // dans la barre latérale. Échouer ici afficherait une page blanche
    // au lieu du catalogue.
    let v = catalogue::lire_articles_avec_unites(
        &conn,
        None,
        Some("depot-efface".to_string()),
    )
    .unwrap();
    assert_eq!(v[0]["stock"], serde_json::json!(7.0));
}

#[test]
fn le_prix_dachat_ne_sort_que_pour_le_patron() {
    let conn = base();
    depot(&conn, "d1", "Principal", true, true);
    article(&conn, "a1", "Ciment", 4000);

    let employe =
        catalogue::lire_articles_avec_unites(&conn, Some("employe".into()), None).unwrap();
    // §7 — un employé n'a pas à connaître la marge du patron parce
    // qu'il sait ouvrir les outils du navigateur. Le filtre est côté
    // serveur, pas côté écran.
    assert!(employe[0].get("dernier_prix_achat").is_none());

    let patron =
        catalogue::lire_articles_avec_unites(&conn, Some("patron".into()), None).unwrap();
    assert_eq!(patron[0]["dernier_prix_achat"], serde_json::json!(4000));
}

#[test]
fn les_unites_dun_article_sont_regroupees() {
    let conn = base();
    depot(&conn, "d1", "Principal", true, true);
    article(&conn, "a1", "Ciment", 4000);
    conn.execute(
        "INSERT INTO unite_vente
         (id, article_id, libelle, facteur, prix_reference, actif,
          cree_le, modifie_le, origine)
         VALUES ('u-sac', 'a1', 'Sac', 50.0, 24000, 1,
                 '2026-01-01', '2026-01-01', 'test')",
        [],
    )
    .unwrap();

    let v = catalogue::lire_articles_avec_unites(&conn, None, None).unwrap();
    assert_eq!(v.len(), 1, "un seul article, deux unités");
    let unites = v[0]["unites"].as_array().unwrap();
    assert_eq!(unites.len(), 2);
    // Trié par facteur croissant : le kilo avant le sac, comme au
    // comptoir où l'on vend d'abord au détail.
    assert_eq!(unites[0]["libelle"], serde_json::json!("Kilo"));
}

// =====================================================================
//  Clients
// =====================================================================

#[test]
fn le_client_de_passage_se_retrouve() {
    let conn = base();
    conn.execute(
        "INSERT INTO client
         (id, code, nom, est_generique, actif, cree_le, modifie_le,
          cree_par, modifie_par, origine)
         VALUES ('c0', 'CLI00000', 'Client de passage', 1, 1,
                 '2026-01-01', '2026-01-01', 't', 't', 'test')",
        [],
    )
    .unwrap();
    let g = catalogue::lire_client_generique(&conn).unwrap();
    assert_eq!(g["code"], serde_json::json!("CLI00000"));
}

#[test]
fn un_client_desactive_ne_sort_plus() {
    let conn = base();
    for (id, actif) in [("c1", 1), ("c2", 0)] {
        conn.execute(
            "INSERT INTO client
             (id, code, nom, est_generique, actif, cree_le, modifie_le,
              cree_par, modifie_par, origine)
             VALUES (?1, ?1, 'Amadou', 0, ?2,
                     '2026-01-01', '2026-01-01', 't', 't', 'test')",
            rusqlite::params![id, actif],
        )
        .unwrap();
    }
    assert_eq!(catalogue::lire_clients(&conn).unwrap().len(), 1);
}
