//! Commandes Tauri pour le dashboard.

use tauri::State;
use crate::commandes::ventes::EtatApp;

#[tauri::command]
pub fn lire_resume_dashboard(
    etat: State<EtatApp>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    // Ventes du jour
    let ventes_jour: i64 = conn.query_row(
        "SELECT CAST(COALESCE(SUM(lv.prix_pratique * lv.quantite), 0) AS INTEGER)
         FROM vente v
         JOIN ligne_vente lv ON lv.vente_id = v.id
         WHERE DATE(v.date_vente) = DATE('now', 'localtime')",
        [], |r| r.get(0),
    ).unwrap_or(0);

    let nb_ventes_jour: i64 = conn.query_row(
        "SELECT COUNT(*) FROM vente
         WHERE DATE(date_vente) = DATE('now', 'localtime')",
        [], |r| r.get(0),
    ).unwrap_or(0);

    // Caisse
    let caisse_solde: i64 = conn.query_row(
        "SELECT CAST(COALESCE(
           (SELECT fond_ouverture FROM session_caisse WHERE statut = 'ouverte' LIMIT 1), 0
         ) + COALESCE(
           (SELECT SUM(montant) FROM mouvement_caisse mc
            JOIN session_caisse sc ON sc.id = mc.session_id
            WHERE sc.statut = 'ouverte' AND mc.sens = 'entree'), 0
         ) - COALESCE(
           (SELECT SUM(montant) FROM mouvement_caisse mc
            JOIN session_caisse sc ON sc.id = mc.session_id
            WHERE sc.statut = 'ouverte' AND mc.sens = 'sortie'), 0
         ) AS INTEGER)",
        [], |r| r.get(0),
    ).unwrap_or(0);

    let caisse_ouverte: bool = conn.query_row(
        "SELECT COUNT(*) FROM session_caisse WHERE statut = 'ouverte'",
        [], |r| r.get::<_, i64>(0),
    ).unwrap_or(0) > 0;

    // Créances clients
    let total_creances: i64 = conn.query_row(
        "SELECT CAST(COALESCE(SUM(
           (SELECT COALESCE(SUM(prix_pratique * quantite), 0)
            FROM ligne_vente WHERE vente_id = v.id) -
           (SELECT COALESCE(SUM(montant), 0)
            FROM paiement WHERE vente_id = v.id)
         ), 0) AS INTEGER)
         FROM vente v WHERE v.statut != 'payee'",
        [], |r| r.get(0),
    ).unwrap_or(0);

    let nb_clients_creance: i64 = conn.query_row(
        "SELECT COUNT(DISTINCT client_id) FROM vente WHERE statut != 'payee'",
        [], |r| r.get(0),
    ).unwrap_or(0);

    // Stock à régulariser
    let articles_a_regulariser: i64 = conn.query_row(
        "SELECT COUNT(*) FROM stock_depot WHERE quantite < 0",
        [], |r| r.get(0),
    ).unwrap_or(0);

    Ok(serde_json::json!({
        "ventes_jour":            ventes_jour,
        "nb_ventes_jour":         nb_ventes_jour,
        "caisse_solde":           caisse_solde,
        "caisse_ouverte":         caisse_ouverte,
        "total_creances":         total_creances,
        "nb_clients_creance":     nb_clients_creance,
        "articles_a_regulariser": articles_a_regulariser,
    }))
}

#[tauri::command]
pub fn lire_ventes_du_jour(
    etat: State<EtatApp>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let mut stmt = conn.prepare(
        "SELECT c.nom, v.statut, v.date_vente,
                CAST(COALESCE(SUM(lv.prix_pratique * lv.quantite), 0) AS INTEGER) as total
         FROM vente v
         JOIN client c ON c.id = v.client_id
         JOIN ligne_vente lv ON lv.vente_id = v.id
         WHERE DATE(v.date_vente) = DATE('now', 'localtime')
         GROUP BY v.id
         ORDER BY v.date_vente DESC
         LIMIT 20"
    ).map_err(|e| e.to_string())?;

    let x = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "client_nom":  row.get::<_, String>(0)?,
            "statut":      row.get::<_, String>(1)?,
            "date_vente":  row.get::<_, String>(2)?,
            "total":       row.get::<_, i64>(3)?,
        }))
    })
    .map_err(|e| e.to_string())?
    .filter_map(|r| r.ok())
    .collect();
    Ok(x)
}
