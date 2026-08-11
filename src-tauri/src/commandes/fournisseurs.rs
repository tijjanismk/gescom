//! Commandes Tauri pour fournisseurs et opérations de stock.

use tauri::State;
use crate::commandes::ventes::EtatApp;
use crate::utils::maintenant_iso;

// =====================================================================
//  FOURNISSEURS
// =====================================================================

#[tauri::command]
pub fn lire_fournisseurs(
    etat: State<EtatApp>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let mut stmt = conn.prepare(
        "SELECT id, nom, telephone FROM fournisseur
         WHERE actif = 1 AND est_voisin = 0
         ORDER BY nom ASC"
    ).map_err(|e| e.to_string())?;

    let x = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "id":        row.get::<_, String>(0)?,
            "nom":       row.get::<_, String>(1)?,
            "telephone": row.get::<_, Option<String>>(2)?,
        }))
    })
    .map_err(|e| e.to_string())?
    .filter_map(|r| r.ok())
    .collect();
    Ok(x)
}

#[tauri::command]
pub fn lire_fournisseurs_avec_dettes(
    etat: State<EtatApp>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    // Pour l'instant les dettes fournisseurs ne sont pas encore implémentées
    // On retourne la liste des fournisseurs avec total_dettes = 0
    let mut stmt = conn.prepare(
        "SELECT id, nom, telephone, est_voisin
         FROM fournisseur
         WHERE actif = 1
         ORDER BY nom ASC"
    ).map_err(|e| e.to_string())?;

    let x = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "id":           row.get::<_, String>(0)?,
            "nom":          row.get::<_, String>(1)?,
            "telephone":    row.get::<_, Option<String>>(2)?,
            "est_voisin":   row.get::<_, i64>(3)? != 0,
            "total_dettes": 0_i64,
            "nb_achats":    0_i64,
        }))
    })
    .map_err(|e| e.to_string())?
    .filter_map(|r| r.ok())
    .collect();
    Ok(x)
}

#[tauri::command]
pub fn creer_fournisseur(
    etat: State<EtatApp>,
    nom: String,
    telephone: Option<String>,
    nif: Option<String>,
    adresse: Option<String>,
) -> Result<String, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let id = uuid::Uuid::new_v4().to_string();
    let maintenant = maintenant_iso();

    conn.execute(
        "INSERT INTO fournisseur
         (id, nom, telephone, nif, adresse, est_voisin, actif,
          cree_le, modifie_le, origine)
         VALUES (?1, ?2, ?3, ?4, ?5, 0, 1, ?6, ?7, 'app')",
        rusqlite::params![
            id, nom, telephone, nif, adresse,
            maintenant, maintenant
        ],
    ).map_err(|e| e.to_string())?;

    Ok(id)
}

// =====================================================================
//  OPÉRATIONS DE STOCK
// =====================================================================

/// Entrée de stock — achat reçu.
/// Crée un mouvement de type 'achat' et met à jour le stock.
#[tauri::command]
pub fn enregistrer_entree_stock(
    etat: State<EtatApp>,
    article_id: String,
    depot_id: String,
    quantite: f64,
    prix_achat: Option<i64>,
    fournisseur_id: Option<String>,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let utilisateur_id = crate::commandes::ventes::id_utilisateur_courant_pub(&conn);
    let maintenant = maintenant_iso();
    let mouvement_id = uuid::Uuid::new_v4().to_string();

    // Mettre à jour le stock.
    conn.execute(
        "UPDATE stock_depot SET quantite = quantite + ?1
         WHERE article_id = ?2 AND depot_id = ?3",
        rusqlite::params![quantite, article_id, depot_id],
    ).map_err(|e| e.to_string())?;

    // Mettre à jour le prix d'achat si fourni.
    if let Some(prix) = prix_achat {
        conn.execute(
            "UPDATE article SET dernier_prix_achat = ?1, modifie_le = ?2
             WHERE id = ?3",
            rusqlite::params![prix, maintenant, article_id],
        ).map_err(|e| e.to_string())?;
    }

    // Enregistrer le mouvement de stock.
    conn.execute(
        "INSERT INTO mouvement_stock
         (id, article_id, depot_id, type_mouvement, quantite_delta,
          auteur_id, date_mouvement, cree_le, cree_par, origine)
         VALUES (?1, ?2, ?3, 'achat', ?4, ?5, ?6, ?7, ?8, 'app')",
        rusqlite::params![
            mouvement_id, article_id, depot_id, quantite,
            utilisateur_id, maintenant, maintenant, utilisateur_id
        ],
    ).map_err(|e| e.to_string())?;

    // Journaliser.
    conn.execute(
        "INSERT INTO journal
         (id, type_evenement, entite_type, entite_id, auteur_id,
          nouveau_valeur, origine, date_evenement)
         VALUES (?1, 'entree_stock', 'stock_depot', ?2, ?3, ?4, 'app', ?5)",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(),
            article_id,
            utilisateur_id,
            format!(r#"{{"quantite":{}, "fournisseur":{}}}"#,
                quantite,
                fournisseur_id.as_deref().unwrap_or("null")
            ),
            maintenant
        ],
    ).map_err(|e| e.to_string())?;

    Ok(())
}

/// Ajustement d'inventaire style Ciel.
/// Saisit la quantité réelle comptée, calcule l'écart et crée un mouvement ajustement.
#[tauri::command]
pub fn enregistrer_ajustement_inventaire(
    etat: State<EtatApp>,
    article_id: String,
    depot_id: String,
    quantite_reelle: f64,
    motif: String,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let utilisateur_id = crate::commandes::ventes::id_utilisateur_courant_pub(&conn);
    let maintenant = maintenant_iso();

    // Lire le stock actuel.
    let stock_actuel: f64 = conn.query_row(
        "SELECT COALESCE(quantite, 0) FROM stock_depot
         WHERE article_id = ?1 AND depot_id = ?2",
        rusqlite::params![article_id, depot_id],
        |row| row.get(0),
    ).unwrap_or(0.0);

    let delta = quantite_reelle - stock_actuel;

    if delta == 0.0 {
        return Ok(()); // Aucun écart — rien à faire.
    }

    // Mettre à jour le stock.
    conn.execute(
        "UPDATE stock_depot SET quantite = ?1
         WHERE article_id = ?2 AND depot_id = ?3",
        rusqlite::params![quantite_reelle, article_id, depot_id],
    ).map_err(|e| e.to_string())?;

    // Enregistrer le mouvement.
    conn.execute(
        "INSERT INTO mouvement_stock
         (id, article_id, depot_id, type_mouvement, quantite_delta,
          motif, auteur_id, date_mouvement, cree_le, cree_par, origine)
         VALUES (?1, ?2, ?3, 'ajustement', ?4, ?5, ?6, ?7, ?8, ?9, 'app')",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(),
            article_id, depot_id, delta,
            motif, utilisateur_id,
            maintenant, maintenant, utilisateur_id
        ],
    ).map_err(|e| e.to_string())?;

    // Journaliser.
    conn.execute(
        "INSERT INTO journal
         (id, type_evenement, entite_type, entite_id, auteur_id,
          ancien_valeur, nouveau_valeur, origine, date_evenement)
         VALUES (?1, 'ajustement_stock', 'stock_depot', ?2, ?3, ?4, ?5, 'app', ?6)",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(),
            article_id,
            utilisateur_id,
            stock_actuel.to_string(),
            quantite_reelle.to_string(),
            maintenant
        ],
    ).map_err(|e| e.to_string())?;

    Ok(())
}