//! Commandes Tauri pour les ventes.
//! Ces fonctions sont appelées depuis le frontend React via invoke().

use tauri::State;
use serde::{Deserialize, Serialize};

use crate::porte::{ContexteUtilisateur, op_creer_vente, op_enregistrer_paiement};
use crate::persistance::ventes::ParamsLigne;

/// État partagé — la connexion SQLite accessible depuis toutes les commandes.
pub struct EtatApp {
    pub conn: std::sync::Mutex<rusqlite::Connection>,
}


/// Paramètres d'une ligne de vente — version sérialisable pour le frontend.
#[derive(Deserialize)]
pub struct ParamsLigneJson {
    pub article_id: String,
    pub unite_vente_id: String,
    pub depot_source_id: String,
    pub source_approvisionnement: String,
    pub quantite: f64,
    pub facteur: f64,
    pub prix_reference: i64,
    pub prix_pratique: i64,
}

/// Résultat retourné au frontend après création d'une vente.
#[derive(Serialize)]
pub struct ResultatVente {
    pub vente_id: String,
}

/// Récupère l'id du premier utilisateur actif en base.
/// Évite de dépendre d'un id hardcodé depuis le frontend.
fn id_utilisateur_courant(conn: &rusqlite::Connection) -> String {
    conn.query_row(
        "SELECT id FROM utilisateur WHERE actif = 1 LIMIT 1",
        [],
        |row| row.get(0),
    )
    .unwrap_or_else(|_| "system".to_string())
}
pub fn id_utilisateur_courant_pub(conn: &rusqlite::Connection) -> String {
    id_utilisateur_courant(conn)
}

/// Commande : créer une vente.
#[tauri::command]
pub fn creer_vente(
    etat: State<EtatApp>,
    client_id: String,
    depot_id: String,
    mode_reglement: String,
    lignes: Vec<ParamsLigneJson>,
    utilisateur_role: String,
) -> Result<ResultatVente, String> {
    let mut conn = etat.conn.lock().map_err(|e| e.to_string())?;

    // Récupérer le vrai id depuis la base.
    let vrai_id = id_utilisateur_courant(&conn);

    let ctx = ContexteUtilisateur {
        id:      vrai_id,
        role:    utilisateur_role,
        origine: "machine-1".to_string(),
    };

    // Collecter dans des types possédés pour éviter les problèmes de lifetime.
    let lignes_owned: Vec<(String, String, String, String, f64, f64, i64, i64)> = lignes
        .iter()
        .map(|l| (
            l.article_id.clone(),
            l.unite_vente_id.clone(),
            l.depot_source_id.clone(),
            l.source_approvisionnement.clone(),
            l.quantite,
            l.facteur,
            l.prix_reference,
            l.prix_pratique,
        ))
        .collect();

    let lignes_rust: Vec<ParamsLigne> = lignes_owned
        .iter()
        .map(|(art, uv, dep, src, qte, fac, prix_ref, prix_prat)| ParamsLigne {
            article_id:               art,
            unite_vente_id:           uv,
            depot_source_id:          dep,
            source_approvisionnement: src,
            quantite:                 *qte,
            facteur:                  *fac,
            prix_reference:           *prix_ref,
            prix_pratique:            *prix_prat,
        })
        .collect();

    let vente_id = op_creer_vente(
        &mut conn,
        &ctx,
        &client_id,
        &depot_id,
        &mode_reglement,
        &lignes_rust,
    )
    .map_err(|e| e.to_string())?;

    Ok(ResultatVente { vente_id })
}

/// Commande : enregistrer un paiement.
#[tauri::command]
pub fn enregistrer_paiement(
    etat: State<EtatApp>,
    vente_id: String,
    montant: i64,
    mode: String,
    avoir_id: Option<String>,
    utilisateur_role: String,
) -> Result<String, String> {
    let mut conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let vrai_id = id_utilisateur_courant(&conn);

    let ctx = ContexteUtilisateur {
        id:      vrai_id,
        role:    utilisateur_role,
        origine: "machine-1".to_string(),
    };

    let id = op_enregistrer_paiement(
        &mut conn,
        &ctx,
        &vente_id,
        montant,
        &mode,
        avoir_id.as_deref(),
    )
    .map_err(|e| e.to_string())?;

    Ok(id)
}

/// Commande : lire tous les clients actifs (hors générique).
#[tauri::command]
pub fn lire_clients(
    etat: State<EtatApp>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let mut stmt = conn.prepare(
        "SELECT id, code, nom, telephone FROM client
         WHERE actif = 1 AND est_generique = 0
         ORDER BY code"
    ).map_err(|e| e.to_string())?;

    let clients: Vec<serde_json::Value> = stmt
        .query_map([], |row| {
            Ok(serde_json::json!({
                "id":        row.get::<_, String>(0)?,
                "code":      row.get::<_, String>(1)?,
                "nom":       row.get::<_, String>(2)?,
                "telephone": row.get::<_, Option<String>>(3)?,
            }))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    Ok(clients)
}

/// Commande : lire le client générique (Comptant).
#[tauri::command]
pub fn lire_client_generique(
    etat: State<EtatApp>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let client = conn.query_row(
        "SELECT id, code, nom FROM client WHERE est_generique = 1 LIMIT 1",
        [],
        |row| {
            Ok(serde_json::json!({
                "id":   row.get::<_, String>(0)?,
                "code": row.get::<_, String>(1)?,
                "nom":  row.get::<_, String>(2)?,
            }))
        },
    ).map_err(|e| e.to_string())?;

    Ok(client)
}

/// Commande : lire le dépôt par défaut.
#[tauri::command]
pub fn lire_depot_defaut(
    etat: State<EtatApp>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let depot = conn.query_row(
        "SELECT id, nom FROM depot WHERE est_defaut = 1 AND actif = 1 LIMIT 1",
        [],
        |row| {
            Ok(serde_json::json!({
                "id":  row.get::<_, String>(0)?,
                "nom": row.get::<_, String>(1)?,
            }))
        },
    ).map_err(|e| e.to_string())?;

    Ok(depot)
}

/// Commande : lire tous les articles actifs avec leurs unités de vente.
#[tauri::command]
pub fn lire_articles_avec_unites(
    etat: State<EtatApp>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let mut stmt = conn.prepare(
        "SELECT a.id, a.nom, a.unite_base,
                u.id, u.libelle, u.facteur, u.prix_reference
         FROM article a
         JOIN unite_vente u ON u.article_id = a.id AND u.actif = 1
         WHERE a.actif = 1
         ORDER BY a.nom, u.facteur ASC"
    ).map_err(|e| e.to_string())?;

    let mut articles: Vec<serde_json::Value> = Vec::new();
    let mut article_courant_id = String::new();

    stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, f64>(5)?,
            row.get::<_, i64>(6)?,
        ))
    })
    .map_err(|e| e.to_string())?
    .filter_map(|r| r.ok())
    .for_each(|(art_id, art_nom, unite_base, u_id, u_libelle, facteur, prix)| {
        let unite = serde_json::json!({
            "id":             u_id,
            "libelle":        u_libelle,
            "facteur":        facteur,
            "prix_reference": prix,
        });

        if art_id != article_courant_id {
            article_courant_id = art_id.clone();
            articles.push(serde_json::json!({
                "id":         art_id,
                "nom":        art_nom,
                "unite_base": unite_base,
                "unites":     [unite],
            }));
        } else if let Some(dernier) = articles.last_mut() {
            if let Some(unites) = dernier["unites"].as_array_mut() {
                unites.push(unite);
            }
        }
    });

    Ok(articles)
}

/// Commande : créer un client rapidement depuis l'écran de vente.
#[tauri::command]
pub fn creer_client_rapide(
    etat: State<EtatApp>,
    nom: String,
    telephone: Option<String>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let utilisateur_id = id_utilisateur_courant(&conn);

    let dernier: i64 = conn.query_row(
        "SELECT COUNT(*) FROM client WHERE est_generique = 0",
        [],
        |row| row.get(0),
    ).unwrap_or(0);

    let code = format!("CLIENT{:05}", dernier + 1);
    let id = uuid::Uuid::new_v4().to_string();
    let maintenant = crate::utils::maintenant_iso();

    conn.execute(
        "INSERT INTO client
         (id, code, nom, telephone, est_generique, actif,
          cree_le, modifie_le, cree_par, modifie_par, origine)
         VALUES (?1, ?2, ?3, ?4, 0, 1, ?5, ?6, ?7, ?8, 'app')",
        rusqlite::params![
            id, code, nom, telephone,
            maintenant, maintenant,
            utilisateur_id, utilisateur_id
        ],
    ).map_err(|e| e.to_string())?;

    Ok(serde_json::json!({
        "id":        id,
        "code":      code,
        "nom":       nom,
        "telephone": telephone,
    }))
}

/// Commande : créer un article rapidement depuis l'écran de vente.
#[tauri::command]
pub fn creer_article_rapide(
    etat: State<EtatApp>,
    nom: String,
    unite_base: String,
    prix_reference: i64,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let utilisateur_id = id_utilisateur_courant(&conn);

    let cat_id: String = conn.query_row(
        "SELECT id FROM categorie WHERE actif = 1 LIMIT 1",
        [],
        |row| row.get(0),
    ).map_err(|_| "Aucune catégorie disponible".to_string())?;

    let depot_id: String = conn.query_row(
        "SELECT id FROM depot WHERE est_defaut = 1 LIMIT 1",
        [],
        |row| row.get(0),
    ).map_err(|e| e.to_string())?;

    let article_id = uuid::Uuid::new_v4().to_string();
    let unite_id   = uuid::Uuid::new_v4().to_string();
    let stock_id   = uuid::Uuid::new_v4().to_string();
    let maintenant = crate::utils::maintenant_iso();

    conn.execute(
        "INSERT INTO article
         (id, nom, categorie_id, unite_base, gere_en_stock,
          attributs, actif, cree_le, modifie_le,
          cree_par, modifie_par, origine)
         VALUES (?1, ?2, ?3, ?4, 1, '{}', 1, ?5, ?6, ?7, ?8, 'app')",
        rusqlite::params![
            article_id, nom, cat_id, unite_base,
            maintenant, maintenant,
            utilisateur_id, utilisateur_id
        ],
    ).map_err(|e| e.to_string())?;

    conn.execute(
        "INSERT INTO unite_vente
         (id, article_id, libelle, facteur, prix_reference, actif,
          cree_le, modifie_le, cree_par, modifie_par, origine)
         VALUES (?1, ?2, ?3, 1.0, ?4, 1, ?5, ?6, ?7, ?8, 'app')",
        rusqlite::params![
            unite_id, article_id, unite_base, prix_reference,
            maintenant, maintenant,
            utilisateur_id, utilisateur_id
        ],
    ).map_err(|e| e.to_string())?;

    conn.execute(
        "INSERT INTO stock_depot (id, article_id, depot_id, quantite)
         VALUES (?1, ?2, ?3, 0.0)",
        rusqlite::params![stock_id, article_id, depot_id],
    ).map_err(|e| e.to_string())?;

    Ok(serde_json::json!({
        "id":         article_id,
        "nom":        nom,
        "unite_base": unite_base,
        "unites": [{
            "id":             unite_id,
            "libelle":        unite_base,
            "facteur":        1.0,
            "prix_reference": prix_reference,
        }]
    }))
}