//! Données de test pour le premier démarrage.
//! Appelé une seule fois si la base est vide.
//! Peuple la base avec des données réalistes pour Bamako.

use rusqlite::{Connection, Result, params};
use uuid::Uuid;
use crate::utils::maintenant_iso;

/// Vérifie si la base est vide (aucun article).
pub fn base_est_vide(conn: &Connection) -> bool {
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM article", [], |row| row.get(0))
        .unwrap_or(0);
    count == 0
}

/// Peuple la base avec des données de test.
pub fn seeder(conn: &Connection) -> Result<()> {
    let maintenant = maintenant_iso();
    let origine = "seed";

    // ---- Rôles ----
    let role_patron_id = Uuid::new_v4().to_string();
    let role_employe_id = Uuid::new_v4().to_string();

    conn.execute(
        "INSERT OR IGNORE INTO role (id, nom, permissions, cree_le, modifie_le, origine)
         VALUES (?1, 'patron', '[]', ?2, ?3, ?4)",
        params![role_patron_id, maintenant, maintenant, origine],
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO role (id, nom, permissions, cree_le, modifie_le, origine)
         VALUES (?1, 'employe', '[]', ?2, ?3, ?4)",
        params![role_employe_id, maintenant, maintenant, origine],
    )?;

    // ---- Utilisateurs ----
    let user_patron_id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT OR IGNORE INTO utilisateur
         (id, nom, role_id, actif, cree_le, modifie_le, origine)
         VALUES (?1, 'Patron', ?2, 1, ?3, ?4, ?5)",
        params![user_patron_id, role_patron_id, maintenant, maintenant, origine],
    )?;

    // ---- Dépôt par défaut ----
    let depot_id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT OR IGNORE INTO depot
         (id, nom, est_defaut, actif, cree_le, modifie_le, origine)
         VALUES (?1, 'Dépôt principal', 1, 1, ?2, ?3, ?4)",
        params![depot_id, maintenant, maintenant, origine],
    )?;

    // ---- Client générique ----
    let client_generique_id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT OR IGNORE INTO client
         (id, code, nom, est_generique, actif,
          cree_le, modifie_le, cree_par, modifie_par, origine)
         VALUES (?1, 'CLIENT00000', 'Comptant', 1, 1, ?2, ?3, 'system', 'system', ?4)",
        params![client_generique_id, maintenant, maintenant, origine],
    )?;

    // ---- Clients de test ----
    let clients = vec![
        ("CLIENT00001", "Amadou Diarra",    Some("76000001")),
        ("CLIENT00002", "Fatoumata Koné",   Some("65000002")),
        ("CLIENT00003", "Ibrahim Traoré",   Some("70000003")),
        ("CLIENT00004", "Mariam Coulibaly", None),
    ];

    for (code, nom, telephone) in clients {
        let id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT OR IGNORE INTO client
             (id, code, nom, telephone, est_generique, actif,
              cree_le, modifie_le, cree_par, modifie_par, origine)
             VALUES (?1, ?2, ?3, ?4, 0, 1, ?5, ?6, ?7, ?8, ?9)",
            params![
                id, code, nom, telephone,
                maintenant, maintenant,
                user_patron_id, user_patron_id, origine
            ],
        )?;
    }

    // ---- Catégories ----
    let cat_alim_id = Uuid::new_v4().to_string();
    let cat_hygiene_id = Uuid::new_v4().to_string();
    let cat_boisson_id = Uuid::new_v4().to_string();

    for (id, nom) in [
        (&cat_alim_id,    "Alimentation"),
        (&cat_hygiene_id, "Hygiène"),
        (&cat_boisson_id, "Boissons"),
    ] {
        conn.execute(
            "INSERT OR IGNORE INTO categorie
             (id, nom, schema_attributs, actif, cree_le, modifie_le, origine)
             VALUES (?1, ?2, '[]', 1, ?3, ?4, ?5)",
            params![id, nom, maintenant, maintenant, origine],
        )?;
    }

    // ---- Articles avec leurs unités de vente et stock ----
    let articles = vec![
        // (nom, categorie_id, unite_base, [(libelle, facteur, prix_ref)], stock_initial)
        (
            "Sucre", &cat_alim_id, "kg",
            vec![
                ("kg", 1.0_f64, 800_i64),
                ("sac 50kg", 50.0, 35000),
            ],
            200.0_f64,
        ),
        (
            "Riz local", &cat_alim_id, "kg",
            vec![
                ("kg", 1.0, 600),
                ("sac 25kg", 25.0, 13500),
            ],
            150.0,
        ),
        (
            "Huile végétale", &cat_alim_id, "litre",
            vec![
                ("litre", 1.0, 1200),
                ("bidon 5L", 5.0, 5500),
            ],
            80.0,
        ),
        (
            "Farine de blé", &cat_alim_id, "kg",
            vec![
                ("kg", 1.0, 500),
                ("sac 50kg", 50.0, 22000),
            ],
            100.0,
        ),
        (
            "Savon Palmolive", &cat_hygiene_id, "unite",
            vec![
                ("unité", 1.0, 500),
                ("carton 24", 24.0, 10800),
            ],
            120.0,
        ),
        (
            "Lait en poudre Nido", &cat_alim_id, "unite",
            vec![
                ("boîte 400g", 1.0, 3500),
                ("carton 6", 6.0, 19000),
            ],
            30.0,
        ),
        (
            "Coca-Cola", &cat_boisson_id, "unite",
            vec![
                ("bouteille 33cl", 1.0, 600),
                ("casier 24", 24.0, 12000),
            ],
            48.0,
        ),
        (
            "Eau minérale Diago", &cat_boisson_id, "litre",
            vec![
                ("bouteille 1.5L", 1.0, 500),
                ("pack 6", 6.0, 2500),
            ],
            60.0,
        ),
    ];

    for (nom, cat_id, unite_base, unites, stock_initial) in &articles {
        let article_id = Uuid::new_v4().to_string();

        conn.execute(
            "INSERT OR IGNORE INTO article
             (id, nom, categorie_id, unite_base, gere_en_stock,
              attributs, actif, cree_le, modifie_le,
              cree_par, modifie_par, origine)
             VALUES (?1, ?2, ?3, ?4, 1, '{}', 1, ?5, ?6, ?7, ?8, ?9)",
            params![
                article_id, nom, cat_id, unite_base,
                maintenant, maintenant,
                user_patron_id, user_patron_id, origine
            ],
        )?;

        // Unités de vente
        for (libelle, facteur, prix_reference) in unites {
            let unite_id = Uuid::new_v4().to_string();
            conn.execute(
                "INSERT OR IGNORE INTO unite_vente
                 (id, article_id, libelle, facteur, prix_reference, actif,
                  cree_le, modifie_le, cree_par, modifie_par, origine)
                 VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?7, ?8, ?9, ?10)",
                params![
                    unite_id, article_id, libelle, facteur, prix_reference,
                    maintenant, maintenant,
                    user_patron_id, user_patron_id, origine
                ],
            )?;
        }

        // Stock initial dans le dépôt par défaut
        let stock_id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT OR IGNORE INTO stock_depot
             (id, article_id, depot_id, quantite)
             VALUES (?1, ?2, ?3, ?4)",
            params![stock_id, article_id, depot_id, stock_initial],
        )?;
    }

    println!("Seed terminé — base peuplée avec {} articles.", articles.len());
    Ok(())
}