//! Données de démarrage pour la première utilisation.
//!
//! ## Pourquoi ce module vit dans le noyau
//!
//! Il vivait dans le crate applicatif, et n'était donc appelé que par
//! la fenêtre. `gescom-serveur.exe` ouvrant une base neuve n'avait NI
//! rôle NI compte : personne ne pouvait se connecter, et aucun écran
//! n'existait pour créer le premier compte — la seule issue était de
//! lancer la fenêtre une fois sur le même fichier.
//!
//! Les deux exécutables partagent maintenant le même amorçage, comme
//! ils partagent déjà le même code métier.
//!
//! DEUX niveaux :
//!   - ESSENTIEL — toujours créé : rôles, comptes, dépôt par défaut,
//!     client « Comptant », paramètres société. Sans eux l'application
//!     ne peut pas fonctionner.
//!   - DÉMO — clients et articles fictifs, pour tester. Créés
//!     UNIQUEMENT si la variable d'environnement GESCOM_DEMO=1.
//!
//! Chez un commerçant, les données de démo seraient à supprimer une par
//! une. On ne les crée donc jamais en production.
//!
//! Comptes : admin/admin123 (patron), employe/employe123 (employé).
//! Les deux exigent un changement de mot de passe à la première
//! connexion.

use rusqlite::{Connection, Result, params};
use uuid::Uuid;
use crate::utils::maintenant_iso;
use crate::auth::hasher_mot_de_passe_pub;

pub fn base_est_vide(conn: &Connection) -> bool {
     let count: i64 = conn
        .query_row(
            // Les COMPTES, pas les roles.
            //
            // C'etait « SELECT COUNT(*) FROM role », et ca a cesse
            // d'etre vrai le jour ou la migration s'est mise a poser des
            // roles livres : une base neuve paraissait deja amorcee, le
            // seed ne tournait pas, et PERSONNE ne pouvait se connecter.
            //
            // « Vide » veut dire une seule chose ici : il n'existe aucun
            // moyen d'entrer.
            "SELECT COUNT(*) FROM utilisateur_auth",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);

    count == 0
}

pub fn seeder(conn: &Connection) -> Result<()> {
    let now = maintenant_iso();
    let origine = "seed";

    // ---- Rôles ----
    let role_patron_id = Uuid::new_v4().to_string();
    let role_employe_id = Uuid::new_v4().to_string();

    // `acces_total` : le patron peut tout, et une commande ajoutee
    // demain lui reste accessible sans que personne ait a cocher une
    // case. Sur une base neuve, la reprise de la migration ne peut pas
    // l'avoir fait — elle tourne avant que ce role existe.
    conn.execute(
        "INSERT OR IGNORE INTO role
           (id, nom, permissions, acces_total, description,
            cree_le, modifie_le, origine)
         VALUES (?1, 'patron', '[]', 1,
                 'Accès complet à la boutique.', ?2, ?3, ?4)",
        params![role_patron_id, now, now, origine],
    )?;
    // La meme liste que la reprise donne aux bases existantes : les deux
    // chemins doivent produire le meme employe.
    let employe = serde_json::json!([
        "ventes:creer",
        "paiements:creer",
        "clients:creer",
        "clients:modifier",
        "articles:creer",
        "caisse:mouvementer",
        "pieces:creer",
        "retours:creer"
    ])
    .to_string();
    conn.execute(
        "INSERT OR IGNORE INTO role
           (id, nom, permissions, acces_total, description,
            cree_le, modifie_le, origine)
         VALUES (?1, 'employe', ?2, 0,
                 'Vend, encaisse, crée clients et articles.', ?3, ?4, ?5)",
        params![role_employe_id, employe, now, now, origine],
    )?;

    // ---- Utilisateurs ----
    let user_patron_id = Uuid::new_v4().to_string();
    let user_employe_id = Uuid::new_v4().to_string();

    conn.execute(
        "INSERT OR IGNORE INTO utilisateur
         (id, nom, role_id, actif, cree_le, modifie_le, origine)
         VALUES (?1, 'Patron', ?2, 1, ?3, ?4, ?5)",
        params![user_patron_id, role_patron_id, now, now, origine],
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO utilisateur
         (id, nom, role_id, actif, cree_le, modifie_le, origine)
         VALUES (?1, 'Employé', ?2, 1, ?3, ?4, ?5)",
        params![user_employe_id, role_employe_id, now, now, origine],
    )?;

    // ---- Auth ----
    let hash_patron = hasher_mot_de_passe_pub("admin123").unwrap_or_default();
    let hash_employe = hasher_mot_de_passe_pub("employe123").unwrap_or_default();

    conn.execute(
        "INSERT OR IGNORE INTO utilisateur_auth
         (utilisateur_id, pseudo, email, mot_de_passe, doit_changer_mdp)
         VALUES (?1, 'admin', 'admin@gescom.ml', ?2, 1)",
        params![user_patron_id, hash_patron],
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO utilisateur_auth
         (utilisateur_id, pseudo, email, mot_de_passe, doit_changer_mdp)
         VALUES (?1, 'employe', 'employe@gescom.ml', ?2, 1)",
        params![user_employe_id, hash_employe],
    )?;

    // ---- Dépôt par défaut ----
    let depot_id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT OR IGNORE INTO depot
         (id, nom, est_defaut, actif, cree_le, modifie_le, origine)
         VALUES (?1, 'Dépôt principal', 1, 1, ?2, ?3, ?4)",
        params![depot_id, now, now, origine],
    )?;

    // ---- Client générique (comptant) ----
    let client_generique_id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT OR IGNORE INTO client
         (id, code, nom, est_generique, actif, cree_le, modifie_le, cree_par, modifie_par, origine)
         VALUES (?1, 'CLIENT00000', 'Comptant', 1, 1, ?2, ?3, ?4, ?5, ?6)",
        params![
            client_generique_id,
            now,
            now,
            user_patron_id,
            user_patron_id,
            origine
        ],
    )?;

    // ---- Paramètres société ----
    conn.execute(
        "INSERT OR IGNORE INTO parametres_societe (id, nom, pied_facture)
         VALUES (1, 'Ma Boutique', 'Merci de votre confiance')",
        [],
    )?;

    // =================================================================
    //  Au-delà de ce point : DONNÉES DE DÉMONSTRATION.
    //  Activées par GESCOM_DEMO=1 uniquement.
    // =================================================================
    let mode_demo = std::env::var("GESCOM_DEMO")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    if !mode_demo {
        println!("[seed] Base initialisée avec succès (mode production)");
        return Ok(());
    }

    println!("[seed] GESCOM_DEMO=1 — Insertion des données de test");

    // ---- Clients de test ----
    let clients = vec![
        ("CLIENT00001", "Amadou Diarra", Some("76000001")),
        ("CLIENT00002", "Fatoumata Koné", Some("65000002")),
        ("CLIENT00003", "Ibrahim Traoré", Some("70000003")),
        ("CLIENT00004", "Mariam Coulibaly", None),
    ];
    for (code, nom, tel) in &clients {
        conn.execute(
            "INSERT OR IGNORE INTO client
             (id, code, nom, telephone, est_generique, actif,
              cree_le, modifie_le, cree_par, modifie_par, origine)
             VALUES (?1, ?2, ?3, ?4, 0, 1, ?5, ?6, ?7, ?8, ?9)",
            params![
                Uuid::new_v4().to_string(),
                code,
                nom,
                tel,
                now,
                now,
                user_patron_id,
                user_patron_id,
                origine
            ],
        )?;
    }

    // ---- Catégories ----
    let cat_alim = Uuid::new_v4().to_string();
    let cat_hygiene = Uuid::new_v4().to_string();
    let cat_boisson = Uuid::new_v4().to_string();
    for (id, nom) in [
        (&cat_alim, "Alimentation"),
        (&cat_hygiene, "Hygiène"),
        (&cat_boisson, "Boissons"),
    ] {
        conn.execute(
            "INSERT OR IGNORE INTO categorie
             (id, nom, schema_attributs, actif, cree_le, modifie_le, origine)
             VALUES (?1, ?2, '[]', 1, ?3, ?4, ?5)",
            params![id, nom, now, now, origine],
        )?;
    }

    // ---- Articles ----
    let articles: Vec<(&str, &str, &str, Vec<(&str, f64, i64)>, f64)> = vec![
        (
            "Sucre",
            &cat_alim,
            "kg",
            vec![("kg", 1.0, 800), ("sac 50kg", 50.0, 35000)],
            200.0,
        ),
        (
            "Riz local",
            &cat_alim,
            "kg",
            vec![("kg", 1.0, 600), ("sac 25kg", 25.0, 13500)],
            150.0,
        ),
        (
            "Huile végétale",
            &cat_alim,
            "litre",
            vec![("litre", 1.0, 1200), ("bidon 5L", 5.0, 5500)],
            80.0,
        ),
        (
            "Farine de blé",
            &cat_alim,
            "kg",
            vec![("kg", 1.0, 500), ("sac 50kg", 50.0, 22000)],
            100.0,
        ),
        (
            "Savon Palmolive",
            &cat_hygiene,
            "unite",
            vec![("unité", 1.0, 500), ("carton 24", 24.0, 10800)],
            120.0,
        ),
        (
            "Lait Nido 400g",
            &cat_alim,
            "unite",
            vec![("boîte", 1.0, 3500), ("carton 6", 6.0, 19000)],
            30.0,
        ),
        (
            "Coca-Cola 33cl",
            &cat_boisson,
            "unite",
            vec![("bouteille", 1.0, 600), ("casier 24", 24.0, 12000)],
            48.0,
        ),
        (
            "Eau Diago 1.5L",
            &cat_boisson,
            "litre",
            vec![("bouteille", 1.0, 500), ("pack 6", 6.0, 2500)],
            60.0,
        ),
    ];

    for (nom, cat_id, unite_base, unites, stock) in &articles {
        let art_id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT OR IGNORE INTO article
             (id, nom, categorie_id, unite_base, gere_en_stock, attributs,
              actif, cree_le, modifie_le, cree_par, modifie_par, origine)
             VALUES (?1, ?2, ?3, ?4, 1, '{}', 1, ?5, ?6, ?7, ?8, ?9)",
            params![
                art_id,
                nom,
                cat_id,
                unite_base,
                now,
                now,
                user_patron_id,
                user_patron_id,
                origine
            ],
        )?;

        for (libelle, facteur, prix) in unites {
            conn.execute(
                "INSERT OR IGNORE INTO unite_vente
                 (id, article_id, libelle, facteur, prix_reference, actif,
                  cree_le, modifie_le, cree_par, modifie_par, origine)
                 VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?7, ?8, ?9, ?10)",
                params![
                    Uuid::new_v4().to_string(),
                    art_id,
                    libelle,
                    facteur,
                    prix,
                    now,
                    now,
                    user_patron_id,
                    user_patron_id,
                    origine
                ],
            )?;
        }

        conn.execute(
            "INSERT OR IGNORE INTO stock_depot (id, article_id, depot_id, quantite)
             VALUES (?1, ?2, ?3, ?4)",
            params![Uuid::new_v4().to_string(), art_id, depot_id, stock],
        )?;
    }

    println!(
        "[seed] Démo chargée : {} articles, {} clients créés",
        articles.len(),
        clients.len()
    );

    Ok(())
}
/// Amorce la base si elle est vide, et ne fait rien sinon.
///
/// Point d'entree unique des deux executables. Le test de vacuite et
/// l'amorcage etaient appeles separement, ce qui laissait a chaque
/// appelant la possibilite d'oublier l'un des deux — c'est ainsi que le
/// serveur s'est retrouve a ouvrir des bases sans aucun compte.
///
/// Rend `Ok(true)` si l'amorcage a eu lieu.
pub fn amorcer_si_vide(conn: &Connection) -> Result<bool> {
    if !base_est_vide(conn) {
        return Ok(false);
    }
    seeder(conn)?;
    Ok(true)
}
