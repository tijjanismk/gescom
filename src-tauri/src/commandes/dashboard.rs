//! Commandes Tauri pour le dashboard, stock, clients et caisse.

use tauri::State;
use crate::commandes::ventes::EtatApp;
use crate::utils::maintenant_iso;

// =====================================================================
//  DASHBOARD
// =====================================================================

#[tauri::command]
pub fn lire_resume_dashboard(
    etat: State<EtatApp>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let aujourd_hui = chrono::Local::now().format("%Y-%m-%d").to_string();

    let (ventes_du_jour, nb_ventes_jour): (i64, i64) = conn.query_row(
        "SELECT COALESCE(SUM(CAST(lv.prix_pratique AS REAL) * lv.quantite), 0),
                COUNT(DISTINCT v.id)
         FROM vente v
         JOIN ligne_vente lv ON lv.vente_id = v.id
         WHERE DATE(v.date_vente) = ?1",
        rusqlite::params![aujourd_hui],
        |row| Ok((row.get(0)?, row.get(1)?)),
    ).unwrap_or((0, 0));

    let caisse_du_jour: i64 = conn.query_row(
        "SELECT COALESCE(SUM(m.montant), 0)
         FROM mouvement_caisse m
         JOIN session_caisse s ON s.id = m.session_id
         WHERE m.sens = 'entree' AND m.moyen = 'especes'
           AND DATE(m.date_mouvement) = ?1",
        rusqlite::params![aujourd_hui],
        |row| row.get(0),
    ).unwrap_or(0);

    let (creances_ouvertes, nb_clients_creance): (i64, i64) = conn.query_row(
        "SELECT COALESCE(SUM(total_v - paye_v), 0), COUNT(*)
         FROM (
           SELECT v.id,
             CAST(COALESCE(SUM(lv.prix_pratique * lv.quantite), 0) AS INTEGER) as total_v,
             COALESCE((SELECT SUM(p.montant) FROM paiement p WHERE p.vente_id = v.id), 0) as paye_v
           FROM vente v
           JOIN ligne_vente lv ON lv.vente_id = v.id
           WHERE v.statut != 'payee'
           GROUP BY v.id
         ) WHERE total_v > paye_v",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    ).unwrap_or((0, 0));

    let articles_a_regulariser: i64 = conn.query_row(
        "SELECT COUNT(*) FROM stock_depot WHERE quantite < 0",
        [],
        |row| row.get(0),
    ).unwrap_or(0);

    Ok(serde_json::json!({
        "ventes_du_jour":         ventes_du_jour,
        "nb_ventes_jour":         nb_ventes_jour,
        "caisse_du_jour":         caisse_du_jour,
        "creances_ouvertes":      creances_ouvertes,
        "nb_clients_creance":     nb_clients_creance,
        "articles_a_regulariser": articles_a_regulariser,
    }))
}

// =====================================================================
//  STOCK
// =====================================================================

#[tauri::command]
pub fn lire_stocks(
    etat: State<EtatApp>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let mut stmt = conn.prepare(
        "SELECT a.id, a.nom, a.unite_base, d.nom, sd.quantite
         FROM stock_depot sd
         JOIN article a ON a.id = sd.article_id
         JOIN depot d ON d.id = sd.depot_id
         WHERE a.actif = 1
         ORDER BY sd.quantite ASC, a.nom ASC"
    ).map_err(|e| e.to_string())?;

    let x = stmt
        .query_map([], |row| {
            Ok(serde_json::json!({
                "article_id":  row.get::<_, String>(0)?,
                "article_nom": row.get::<_, String>(1)?,
                "unite_base":  row.get::<_, String>(2)?,
                "depot_nom":   row.get::<_, String>(3)?,
                "quantite":    row.get::<_, f64>(4)?,
            }))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    Ok(x)
}

// =====================================================================
//  CLIENTS
// =====================================================================

#[tauri::command]
pub fn lire_clients_avec_creances(
    etat: State<EtatApp>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let mut stmt = conn.prepare(
        "SELECT c.id, c.code, c.nom, c.telephone,
                COALESCE(SUM(CASE WHEN v.statut != 'payee'
                    THEN CAST(lv_sum.total AS INTEGER) - CAST(p_sum.paye AS INTEGER)
                    ELSE 0 END), 0) as total_creances,
                COUNT(DISTINCT v.id) as nb_ventes
         FROM client c
         LEFT JOIN vente v ON v.client_id = c.id
         LEFT JOIN (
             SELECT vente_id, SUM(prix_pratique * quantite) as total
             FROM ligne_vente GROUP BY vente_id
         ) lv_sum ON lv_sum.vente_id = v.id
         LEFT JOIN (
             SELECT vente_id, SUM(montant) as paye
             FROM paiement GROUP BY vente_id
         ) p_sum ON p_sum.vente_id = v.id
         WHERE c.actif = 1 AND c.est_generique = 0
         GROUP BY c.id
         ORDER BY total_creances DESC, c.nom ASC"
    ).map_err(|e| e.to_string())?;

    let x = stmt
        .query_map([], |row| {
            Ok(serde_json::json!({
                "id":             row.get::<_, String>(0)?,
                "code":           row.get::<_, String>(1)?,
                "nom":            row.get::<_, String>(2)?,
                "telephone":      row.get::<_, Option<String>>(3)?,
                "total_creances": row.get::<_, i64>(4)?,
                "nb_ventes":      row.get::<_, i64>(5)?,
            }))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    Ok(x)
}

// =====================================================================
//  CAISSE
// =====================================================================

#[tauri::command]
pub fn lire_resume_caisse(
    etat: State<EtatApp>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let session = conn.query_row(
        "SELECT id, fond_initial, date_ouverture, statut, montant_compte, ecart
         FROM session_caisse WHERE statut = 'ouverte' LIMIT 1",
        [],
        |row| {
            Ok(serde_json::json!({
                "id":             row.get::<_, String>(0)?,
                "fond_initial":   row.get::<_, i64>(1)?,
                "date_ouverture": row.get::<_, String>(2)?,
                "statut":         row.get::<_, String>(3)?,
                "montant_compte": row.get::<_, Option<i64>>(4)?,
                "ecart":          row.get::<_, Option<i64>>(5)?,
            }))
        },
    ).ok();

    let session_id = session.as_ref()
        .and_then(|s| s["id"].as_str())
        .unwrap_or("")
        .to_string();

    let fond_initial: i64 = session.as_ref()
        .and_then(|s| s["fond_initial"].as_i64())
        .unwrap_or(0);

    // Mouvements de la session — correction du lifetime avec let x
    let mouvements: Vec<serde_json::Value> = if session_id.is_empty() {
        Vec::new()
    } else {
        let mut stmt = conn.prepare(
            "SELECT id, sens, moyen, montant, motif, date_mouvement
             FROM mouvement_caisse WHERE session_id = ?1
             ORDER BY date_mouvement DESC"
        ).map_err(|e| e.to_string())?;

        let x = stmt
            .query_map(rusqlite::params![session_id], |row| {
                Ok(serde_json::json!({
                    "id":             row.get::<_, String>(0)?,
                    "sens":           row.get::<_, String>(1)?,
                    "moyen":          row.get::<_, String>(2)?,
                    "montant":        row.get::<_, i64>(3)?,
                    "motif":          row.get::<_, String>(4)?,
                    "date_mouvement": row.get::<_, String>(5)?,
                }))
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();
        x
    };

    let total_entrees_especes: i64 = mouvements.iter()
        .filter(|m| m["sens"] == "entree" && m["moyen"] == "especes")
        .filter_map(|m| m["montant"].as_i64())
        .sum();

    let total_entrees_mobile: i64 = mouvements.iter()
        .filter(|m| m["sens"] == "entree" &&
            (m["moyen"] == "orange_money" || m["moyen"] == "moov_money"))
        .filter_map(|m| m["montant"].as_i64())
        .sum();

    let total_sorties: i64 = mouvements.iter()
        .filter(|m| m["sens"] == "sortie" && m["moyen"] == "especes")
        .filter_map(|m| m["montant"].as_i64())
        .sum();

    let solde_theorique_especes = fond_initial + total_entrees_especes - total_sorties;

    Ok(serde_json::json!({
        "session":                 session,
        "mouvements":              mouvements,
        "total_entrees_especes":   total_entrees_especes,
        "total_entrees_mobile":    total_entrees_mobile,
        "total_sorties":           total_sorties,
        "solde_theorique_especes": solde_theorique_especes,
    }))
}

#[tauri::command]
pub fn ouvrir_session_caisse(
    etat: State<EtatApp>,
    fond_initial: i64,
) -> Result<String, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let ouverte: i64 = conn.query_row(
        "SELECT COUNT(*) FROM session_caisse WHERE statut = 'ouverte'",
        [],
        |row| row.get(0),
    ).unwrap_or(0);

    if ouverte > 0 {
        return Err("Une session est déjà ouverte".to_string());
    }

    let id = uuid::Uuid::new_v4().to_string();
    let maintenant = maintenant_iso();
    let utilisateur_id = crate::commandes::ventes::id_utilisateur_courant_pub(&conn);

    conn.execute(
        "INSERT INTO session_caisse
         (id, fond_initial, date_ouverture, statut,
          cree_le, modifie_le, cree_par, modifie_par, origine)
         VALUES (?1, ?2, ?3, 'ouverte', ?4, ?5, ?6, ?7, 'app')",
        rusqlite::params![
            id, fond_initial, maintenant,
            maintenant, maintenant,
            utilisateur_id, utilisateur_id
        ],
    ).map_err(|e| e.to_string())?;

    Ok(id)
}

#[tauri::command]
pub fn fermer_session_caisse(
    etat: State<EtatApp>,
    montant_compte: i64,
) -> Result<i64, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let (session_id, fond_initial): (String, i64) = conn.query_row(
        "SELECT id, fond_initial FROM session_caisse WHERE statut = 'ouverte' LIMIT 1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    ).map_err(|_| "Aucune session ouverte".to_string())?;

    let entrees: i64 = conn.query_row(
        "SELECT COALESCE(SUM(montant), 0) FROM mouvement_caisse
         WHERE session_id = ?1 AND sens = 'entree' AND moyen = 'especes'",
        rusqlite::params![session_id],
        |row| row.get(0),
    ).unwrap_or(0);

    let sorties: i64 = conn.query_row(
        "SELECT COALESCE(SUM(montant), 0) FROM mouvement_caisse
         WHERE session_id = ?1 AND sens = 'sortie' AND moyen = 'especes'",
        rusqlite::params![session_id],
        |row| row.get(0),
    ).unwrap_or(0);

    let solde_theorique = fond_initial + entrees - sorties;
    let ecart = montant_compte - solde_theorique;
    let maintenant = maintenant_iso();
    let utilisateur_id = crate::commandes::ventes::id_utilisateur_courant_pub(&conn);

    conn.execute(
        "UPDATE session_caisse
         SET statut = 'fermee', date_fermeture = ?1,
             montant_compte = ?2, ecart = ?3,
             modifie_le = ?4, modifie_par = ?5
         WHERE id = ?6",
        rusqlite::params![
            maintenant, montant_compte, ecart,
            maintenant, utilisateur_id, session_id
        ],
    ).map_err(|e| e.to_string())?;

    Ok(ecart)
}