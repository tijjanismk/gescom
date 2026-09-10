//! Facades Tauri de `relances`.
//!
//! La logique vit dans `gescom_noyau::relances` : le serveur v2
//! execute exactement le meme code que le comptoir.

use tauri::State;
use crate::commandes::ventes::EtatApp;

#[tauri::command]
pub fn lire_creances_relances(
    etat: State<EtatApp>,
    en_retard_seulement: Option<bool>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::relances::lire_creances_relances(&conn, en_retard_seulement)
}
#[tauri::command]
pub fn enregistrer_relance(
    etat: State<EtatApp>,
    vente_id: String,
    canal: String,
    note: Option<String>,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::relances::enregistrer_relance(&conn, vente_id, canal, note)
}
#[tauri::command]
pub fn lire_historique_relances(
    etat: State<EtatApp>,
    vente_id: String,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::relances::lire_historique_relances(&conn, vente_id)
}
#[tauri::command]
pub fn lire_stats_relances(
    etat: State<EtatApp>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::relances::lire_stats_relances(&conn, )
}
