//! Facades Tauri de `rapports`.
//!
//! La logique vit dans `gescom_noyau::rapports` : le serveur v2
//! execute exactement le meme code que le comptoir.

use tauri::State;
use crate::commandes::ventes::EtatApp;

#[tauri::command]
pub fn lire_rapport_ca_mensuel(
    etat: State<EtatApp>,
    nb_mois: Option<i64>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::rapports::lire_rapport_ca_mensuel(&conn, nb_mois)
}
#[tauri::command]
pub fn lire_rapport_top_clients(
    etat: State<EtatApp>,
    date_debut: String,
    date_fin: String,
    limite: Option<i64>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::rapports::lire_rapport_top_clients(&conn, date_debut, date_fin, limite)
}
#[tauri::command]
pub fn lire_rapport_top_articles(
    etat: State<EtatApp>,
    date_debut: String,
    date_fin: String,
    limite: Option<i64>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::rapports::lire_rapport_top_articles(&conn, date_debut, date_fin, limite)
}
#[tauri::command]
pub fn lire_rapport_creances(
    etat: State<EtatApp>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::rapports::lire_rapport_creances(&conn, )
}
#[tauri::command]
pub fn lire_rapport_stock(
    etat: State<EtatApp>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::rapports::lire_rapport_stock(&conn, )
}
#[tauri::command]
pub fn lire_rapport_tva(
    etat: State<EtatApp>,
    date_debut: String,
    date_fin: String,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::rapports::lire_rapport_tva(&conn, date_debut, date_fin)
}
