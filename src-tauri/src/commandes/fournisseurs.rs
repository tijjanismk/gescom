//! Commandes Tauri pour fournisseurs et opérations de stock.

use tauri::State;
use crate::commandes::ventes::EtatApp;
use crate::utils::maintenant_iso;

#[tauri::command]
pub fn lire_fournisseurs(etat: State<EtatApp>) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(
        "SELECT id, nom, telephone FROM fournisseur
         WHERE actif = 1 AND est_voisin = 0 ORDER BY nom ASC"
    ).map_err(|e| e.to_string())?;
    let x = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_,String>(0)?,
            "nom": row.get::<_,String>(1)?,
            "telephone": row.get::<_,Option<String>>(2)?,
        }))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
    Ok(x)
}

#[tauri::command]
pub fn lire_fournisseurs_avec_dettes(
    etat: State<EtatApp>
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(
        "SELECT id, nom, telephone, est_voisin FROM fournisseur
         WHERE actif = 1 ORDER BY nom ASC"
    ).map_err(|e| e.to_string())?;
    let x = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_,String>(0)?,
            "nom": row.get::<_,String>(1)?,
            "telephone": row.get::<_,Option<String>>(2)?,
            "est_voisin": row.get::<_,i64>(3)? != 0,
            "total_dettes": 0_i64,
            "nb_achats": 0_i64,
        }))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
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
    let now = maintenant_iso();
    conn.execute(
        "INSERT INTO fournisseur
         (id, nom, telephone, nif, adresse, est_voisin, actif, cree_le, modifie_le, origine)
         VALUES (?1,?2,?3,?4,?5,0,1,?6,?7,'app')",
        rusqlite::params![id, nom, telephone, nif, adresse, now, now],
    ).map_err(|e| e.to_string())?;
    Ok(id)
}

#[tauri::command]
pub fn enregistrer_entree_stock(
    etat: State<EtatApp>,
    article_id: String,
    depot_id: Option<String>,
    quantite: f64,
    prix_achat: Option<i64>,
    fournisseur_id: Option<String>,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let auteur = crate::commandes::ventes::id_utilisateur_courant_pub(&conn);
    let now = maintenant_iso();

    let depot_reel = match depot_id {
        Some(id) => id,
        None => conn.query_row(
            "SELECT id FROM depot WHERE est_defaut = 1 AND actif = 1 LIMIT 1",
            [], |r| r.get(0)
        ).map_err(|e| e.to_string())?,
    };

    conn.execute(
        "INSERT INTO stock_depot (id, article_id, depot_id, quantite)
         VALUES (?1,?2,?3,?4)
         ON CONFLICT(article_id, depot_id) DO UPDATE SET quantite = quantite + ?4",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(), article_id, depot_reel, quantite
        ],
    ).map_err(|e| e.to_string())?;

    if let Some(prix) = prix_achat {
        conn.execute(
            "UPDATE article SET dernier_prix_achat = ?1, modifie_le = ?2 WHERE id = ?3",
            rusqlite::params![prix, now, article_id],
        ).ok();
    }

    conn.execute(
        "INSERT INTO mouvement_stock
         (id, article_id, depot_id, type_mouvement, quantite_delta,
          auteur_id, date_mouvement, cree_le, cree_par, origine)
         VALUES (?1,?2,?3,'achat',?4,?5,?6,?7,?8,'app')",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(), article_id, depot_reel,
            quantite, auteur, now, now, auteur
        ],
    ).map_err(|e| e.to_string())?;

    conn.execute(
        "INSERT INTO journal
         (id, type_evenement, entite_type, entite_id, auteur_id,
          nouveau_valeur, origine, date_evenement)
         VALUES (?1,'entree_stock','stock_depot',?2,?3,?4,'app',?5)",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(), article_id, auteur,
            format!(r#"{{"quantite":{},"fournisseur":{}}}"#,
                quantite,
                fournisseur_id.as_deref()
                    .map(|s| format!("\"{}\"", s))
                    .unwrap_or("null".to_string())
            ), now
        ],
    ).ok();
    Ok(())
}

#[tauri::command]
pub fn enregistrer_ajustement_inventaire(
    etat: State<EtatApp>,
    article_id: String,
    depot_id: String,
    quantite_reelle: f64,
    motif: String,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let auteur = crate::commandes::ventes::id_utilisateur_courant_pub(&conn);
    let now = maintenant_iso();

    let stock_actuel: f64 = conn.query_row(
        "SELECT COALESCE(quantite, 0) FROM stock_depot
         WHERE article_id = ?1 AND depot_id = ?2",
        rusqlite::params![article_id, depot_id], |r| r.get(0),
    ).unwrap_or(0.0);

    let delta = quantite_reelle - stock_actuel;
    if delta == 0.0 { return Ok(()); }

    conn.execute(
        "UPDATE stock_depot SET quantite = ?1
         WHERE article_id = ?2 AND depot_id = ?3",
        rusqlite::params![quantite_reelle, article_id, depot_id],
    ).map_err(|e| e.to_string())?;

    conn.execute(
        "INSERT INTO mouvement_stock
         (id, article_id, depot_id, type_mouvement, quantite_delta,
          motif, auteur_id, date_mouvement, cree_le, cree_par, origine)
         VALUES (?1,?2,?3,'ajustement',?4,?5,?6,?7,?8,?9,'app')",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(), article_id, depot_id,
            delta, motif, auteur, now, now, auteur
        ],
    ).map_err(|e| e.to_string())?;

    conn.execute(
        "INSERT INTO journal
         (id, type_evenement, entite_type, entite_id, auteur_id,
          ancien_valeur, nouveau_valeur, origine, date_evenement)
         VALUES (?1,'ajustement_stock','stock_depot',?2,?3,?4,?5,'app',?6)",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(), article_id, auteur,
            stock_actuel.to_string(), quantite_reelle.to_string(), now
        ],
    ).ok();
    Ok(())
}
