//! Relances créances — historique, envoi WhatsApp, marquage.

use tauri::State;
use crate::commandes::ventes::EtatApp;
use crate::utils::maintenant_iso;

// =====================================================================
//  Lire créances avec détail pour relance
// =====================================================================

#[tauri::command]
pub fn lire_creances_relances(
    etat: State<EtatApp>,
    en_retard_seulement: Option<bool>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let filtre_retard = if en_retard_seulement == Some(true) {
        "AND (julianday('now') - julianday(v.date_vente)) > 30"
    } else { "" };

    let sql = format!(
        "SELECT
            v.id, v.date_vente, v.statut,
            c.id as client_id, c.nom as client_nom,
            c.code as client_code, c.telephone,
            f.numero as facture_num,
            CAST(COALESCE(
              (SELECT SUM(prix_pratique * quantite) FROM ligne_vente WHERE vente_id = v.id)
            , 0) AS INTEGER) as total,
            CAST(COALESCE(
              (SELECT SUM(montant) FROM paiement WHERE vente_id = v.id)
            , 0) AS INTEGER) as total_paye,
            CAST(julianday('now') - julianday(v.date_vente) AS INTEGER) as jours_retard,
            COALESCE(
              (SELECT COUNT(*) FROM relance_creance rc WHERE rc.vente_id = v.id)
            , 0) as nb_relances,
            COALESCE(
              (SELECT MAX(rc.date_relance) FROM relance_creance rc WHERE rc.vente_id = v.id)
            , NULL) as derniere_relance
         FROM vente v
         JOIN client c ON c.id = v.client_id
         LEFT JOIN facture f ON f.vente_id = v.id
         WHERE v.statut IN ('creance_ouverte', 'partiellement_payee')
           AND c.est_generique = 0
           {}
         ORDER BY jours_retard DESC, total DESC",
        filtre_retard
    );

    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;

    let x = stmt.query_map([], |row| {
        let total: i64 = row.get(8)?;
        let paye: i64 = row.get(9)?;
        Ok(serde_json::json!({
            "vente_id":       row.get::<_,String>(0)?,
            "date_vente":     row.get::<_,String>(1)?,
            "statut":         row.get::<_,String>(2)?,
            "client_id":      row.get::<_,String>(3)?,
            "client_nom":     row.get::<_,String>(4)?,
            "client_code":    row.get::<_,String>(5)?,
            "telephone":      row.get::<_,Option<String>>(6)?,
            "facture_num":    row.get::<_,Option<String>>(7)?,
            "total":          total,
            "total_paye":     paye,
            "reste":          total - paye,
            "jours_retard":   row.get::<_,i64>(10)?,
            "nb_relances":    row.get::<_,i64>(11)?,
            "derniere_relance": row.get::<_,Option<String>>(12)?,
        }))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
    Ok(x)
}

// =====================================================================
//  Enregistrer une relance
// =====================================================================

#[tauri::command]
pub fn enregistrer_relance(
    etat: State<EtatApp>,
    vente_id: String,
    canal: String,      // whatsapp | sms | appel | email | visite
    note: Option<String>,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    // Créer la table si elle n'existe pas
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS relance_creance (
            id           TEXT PRIMARY KEY,
            vente_id     TEXT NOT NULL REFERENCES vente(id),
            canal        TEXT NOT NULL DEFAULT 'whatsapp',
            note         TEXT,
            auteur_id    TEXT,
            date_relance TEXT NOT NULL,
            cree_le      TEXT NOT NULL,
            origine      TEXT NOT NULL DEFAULT 'app'
        )"
    ).map_err(|e| e.to_string())?;

    let auteur = crate::commandes::ventes::id_utilisateur_courant_pub(&conn);
    let now = maintenant_iso();

    conn.execute(
        "INSERT INTO relance_creance
         (id, vente_id, canal, note, auteur_id, date_relance, cree_le, origine)
         VALUES (?1,?2,?3,?4,?5,?6,?7,'app')",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(),
            vente_id, canal, note, auteur, now, now
        ],
    ).map_err(|e| e.to_string())?;

    Ok(())
}

// =====================================================================
//  Lire historique relances d'une créance
// =====================================================================

#[tauri::command]
pub fn lire_historique_relances(
    etat: State<EtatApp>,
    vente_id: String,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS relance_creance (
            id TEXT PRIMARY KEY, vente_id TEXT, canal TEXT,
            note TEXT, auteur_id TEXT, date_relance TEXT,
            cree_le TEXT, origine TEXT DEFAULT 'app'
        )"
    ).ok();

    let mut stmt = conn.prepare(
        "SELECT rc.id, rc.canal, rc.note, rc.date_relance, u.nom as auteur_nom
         FROM relance_creance rc
         LEFT JOIN utilisateur u ON u.id = rc.auteur_id
         WHERE rc.vente_id = ?1
         ORDER BY rc.date_relance DESC"
    ).map_err(|e| e.to_string())?;

    let x = stmt.query_map(rusqlite::params![vente_id], |row| {
        Ok(serde_json::json!({
            "id":           row.get::<_,String>(0)?,
            "canal":        row.get::<_,String>(1)?,
            "note":         row.get::<_,Option<String>>(2)?,
            "date_relance": row.get::<_,String>(3)?,
            "auteur_nom":   row.get::<_,Option<String>>(4)?,
        }))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
    Ok(x)
}

// =====================================================================
//  Stats relances globales
// =====================================================================

#[tauri::command]
pub fn lire_stats_relances(
    etat: State<EtatApp>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS relance_creance (
            id TEXT PRIMARY KEY, vente_id TEXT, canal TEXT,
            note TEXT, auteur_id TEXT, date_relance TEXT,
            cree_le TEXT, origine TEXT DEFAULT 'app'
        )"
    ).ok();

    let total_creances: i64 = conn.query_row(
        "SELECT COUNT(*) FROM vente
         WHERE statut IN ('creance_ouverte','partiellement_payee')", [],
        |r| r.get(0)
    ).unwrap_or(0);

    let sans_relance: i64 = conn.query_row(
        "SELECT COUNT(*) FROM vente v
         WHERE v.statut IN ('creance_ouverte','partiellement_payee')
           AND NOT EXISTS (SELECT 1 FROM relance_creance rc WHERE rc.vente_id = v.id)", [],
        |r| r.get(0)
    ).unwrap_or(0);

    let relances_semaine: i64 = conn.query_row(
        "SELECT COUNT(*) FROM relance_creance
         WHERE date_relance >= date('now', '-7 days')", [],
        |r| r.get(0)
    ).unwrap_or(0);

    let montant_en_jeu: i64 = conn.query_row(
        "SELECT CAST(COALESCE(SUM(
            (SELECT COALESCE(SUM(prix_pratique * quantite), 0) FROM ligne_vente WHERE vente_id = v.id) -
            (SELECT COALESCE(SUM(montant), 0) FROM paiement WHERE vente_id = v.id)
         ), 0) AS INTEGER)
         FROM vente v
         WHERE v.statut IN ('creance_ouverte','partiellement_payee')", [],
        |r| r.get(0)
    ).unwrap_or(0);

    Ok(serde_json::json!({
        "total_creances":   total_creances,
        "sans_relance":     sans_relance,
        "relances_semaine": relances_semaine,
        "montant_en_jeu":   montant_en_jeu,
    }))
}
