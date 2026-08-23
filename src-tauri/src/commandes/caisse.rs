//! Commandes Tauri pour la caisse.
//!
//! ⚠️ Le rapprochement ne porte que sur les ESPÈCES. Orange Money, Moov
//! Money et les chèques sont tracés en mouvement de caisse mais ne sont
//! pas dans le tiroir physique : les inclure dans le solde théorique
//! affiche un manque systématique à chaque clôture.

use tauri::State;
use crate::commandes::ventes::EtatApp;
use crate::utils::maintenant_iso;

// =====================================================================
//  RÉSUMÉ CAISSE
// =====================================================================

#[tauri::command]
pub fn lire_resume_caisse(
    etat: State<EtatApp>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    // Chercher une session ouverte.
    let session: Option<(String, i64, String)> = conn.query_row(
        "SELECT id, fond_ouverture, cree_le FROM session_caisse
         WHERE statut = 'ouverte' ORDER BY cree_le DESC LIMIT 1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).ok();

    match session {
        None => {
            // Vérifier s'il y a déjà eu des sessions.
            let nb: i64 = conn.query_row(
                "SELECT COUNT(*) FROM session_caisse", [], |r| r.get(0)
            ).unwrap_or(0);

            Ok(serde_json::json!({
                "session_id":       null,
                "statut":           if nb > 0 { "fermee" } else { "aucune" },
                "fond_ouverture":   0,
                "total_entrees":    0,
                "total_sorties":    0,
                "entrees_especes":  0,
                "sorties_especes":  0,
                "solde_theorique":  0,
                "nb_transactions":  0,
                "ouvert_le":        null,
            }))
        }
        Some((session_id, fond_ouverture, ouvert_le)) => {
            // Tous moyens confondus — pour les indicateurs d'activité.
            let total_entrees: i64 = conn.query_row(
                "SELECT CAST(COALESCE(SUM(montant), 0) AS INTEGER)
                 FROM mouvement_caisse
                 WHERE session_id = ?1 AND sens = 'entree'",
                rusqlite::params![session_id],
                |r| r.get(0),
            ).unwrap_or(0);

            let total_sorties: i64 = conn.query_row(
                "SELECT CAST(COALESCE(SUM(montant), 0) AS INTEGER)
                 FROM mouvement_caisse
                 WHERE session_id = ?1 AND sens = 'sortie'",
                rusqlite::params![session_id],
                |r| r.get(0),
            ).unwrap_or(0);

            // Espèces seules — pour le rapprochement du tiroir.
            let entrees_especes: i64 = conn.query_row(
                "SELECT CAST(COALESCE(SUM(montant), 0) AS INTEGER)
                 FROM mouvement_caisse
                 WHERE session_id = ?1 AND sens = 'entree' AND moyen = 'especes'",
                rusqlite::params![session_id],
                |r| r.get(0),
            ).unwrap_or(0);

            let sorties_especes: i64 = conn.query_row(
                "SELECT CAST(COALESCE(SUM(montant), 0) AS INTEGER)
                 FROM mouvement_caisse
                 WHERE session_id = ?1 AND sens = 'sortie' AND moyen = 'especes'",
                rusqlite::params![session_id],
                |r| r.get(0),
            ).unwrap_or(0);

            let nb_transactions: i64 = conn.query_row(
                "SELECT COUNT(*) FROM mouvement_caisse WHERE session_id = ?1",
                rusqlite::params![session_id],
                |r| r.get(0),
            ).unwrap_or(0);

            let solde_theorique = crate::coeur::caisse::solde_theorique(
                fond_ouverture, entrees_especes, sorties_especes
            );

            Ok(serde_json::json!({
                "session_id":      session_id,
                "statut":          "ouverte",
                "fond_ouverture":  fond_ouverture,
                "total_entrees":   total_entrees,
                "total_sorties":   total_sorties,
                "entrees_especes": entrees_especes,
                "sorties_especes": sorties_especes,
                "solde_theorique": solde_theorique,
                "nb_transactions": nb_transactions,
                "ouvert_le":       ouvert_le,
            }))
        }
    }
}

// =====================================================================
//  MOUVEMENTS DU JOUR
// =====================================================================

#[tauri::command]
pub fn lire_mouvements_caisse_du_jour(
    etat: State<EtatApp>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let mut stmt = conn.prepare(
        "SELECT mc.id, mc.sens, mc.moyen, mc.montant, mc.motif, mc.date_mouvement
         FROM mouvement_caisse mc
         JOIN session_caisse sc ON sc.id = mc.session_id
         WHERE DATE(mc.date_mouvement) = DATE('now', 'localtime')
         ORDER BY mc.date_mouvement ASC"
    ).map_err(|e| e.to_string())?;

    let x = stmt.query_map([], |row| {
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
    Ok(x)
}

// =====================================================================
//  OUVERTURE DE SESSION
// =====================================================================

#[tauri::command]
pub fn ouvrir_session_caisse(
    etat: State<EtatApp>,
    fond_ouverture: i64,
    utilisateur_role: String,
) -> Result<String, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let utilisateur_id = crate::commandes::ventes::id_utilisateur_par_role(
        &conn, &utilisateur_role
    );
    let maintenant = maintenant_iso();

    // Vérifier qu'il n'y a pas déjà une session ouverte.
    let session_ouverte: i64 = conn.query_row(
        "SELECT COUNT(*) FROM session_caisse WHERE statut = 'ouverte'",
        [], |r| r.get(0),
    ).unwrap_or(0);

    if session_ouverte > 0 {
        return Err("Une session est déjà ouverte".to_string());
    }

    let session_id = uuid::Uuid::new_v4().to_string();

    conn.execute(
        "INSERT INTO session_caisse
         (id, statut, fond_ouverture, ouvert_par, cree_le, modifie_le, origine)
         VALUES (?1, 'ouverte', ?2, ?3, ?4, ?5, 'app')",
        rusqlite::params![
            session_id, fond_ouverture, utilisateur_id,
            maintenant, maintenant
        ],
    ).map_err(|e| e.to_string())?;

    // Mouvement d'ouverture si fond > 0.
    if fond_ouverture > 0 {
        conn.execute(
            "INSERT INTO mouvement_caisse
             (id, session_id, sens, moyen, montant, motif,
              date_mouvement, cree_le, cree_par, origine)
             VALUES (?1, ?2, 'entree', 'especes', ?3, 'ouverture', ?4, ?5, ?6, 'app')",
            rusqlite::params![
                uuid::Uuid::new_v4().to_string(),
                session_id, fond_ouverture,
                maintenant, maintenant, utilisateur_id
            ],
        ).map_err(|e| e.to_string())?;
    }

    Ok(session_id)
}

// =====================================================================
//  FERMETURE DE SESSION
// =====================================================================

#[tauri::command]
pub fn fermer_session_caisse(
    etat: State<EtatApp>,
    session_id: String,
    especes_comptees: i64,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let maintenant = maintenant_iso();

    let fond: i64 = conn.query_row(
        "SELECT fond_ouverture FROM session_caisse WHERE id = ?1",
        rusqlite::params![session_id], |r| r.get(0),
    ).map_err(|e| e.to_string())?;

    // ⚠️ ESPÈCES UNIQUEMENT. Sans ce filtre, l'écart intègre l'Orange
    // Money et la caisse paraît en déficit à chaque clôture.
    let entrees: i64 = conn.query_row(
        "SELECT CAST(COALESCE(SUM(montant), 0) AS INTEGER)
         FROM mouvement_caisse
         WHERE session_id = ?1 AND sens = 'entree' AND moyen = 'especes'",
        rusqlite::params![session_id], |r| r.get(0),
    ).unwrap_or(0);

    let sorties: i64 = conn.query_row(
        "SELECT CAST(COALESCE(SUM(montant), 0) AS INTEGER)
         FROM mouvement_caisse
         WHERE session_id = ?1 AND sens = 'sortie' AND moyen = 'especes'",
        rusqlite::params![session_id], |r| r.get(0),
    ).unwrap_or(0);

    let solde_theorique = crate::coeur::caisse::solde_theorique(fond, entrees, sorties);
    let ecart = crate::coeur::caisse::ecart_caisse(especes_comptees, solde_theorique);

    conn.execute(
        "UPDATE session_caisse SET
           statut = 'fermee',
           solde_theorique = ?1,
           especes_comptees = ?2,
           ecart = ?3,
           ferme_le = ?4,
           modifie_le = ?5
         WHERE id = ?6",
        rusqlite::params![
            solde_theorique, especes_comptees, ecart,
            maintenant, maintenant, session_id
        ],
    ).map_err(|e| e.to_string())?;

    Ok(())
}
