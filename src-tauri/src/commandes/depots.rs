//! Facades Tauri de `depots`.
//!
//! La logique vit dans `gescom_noyau::depots` : le serveur v2
//! execute exactement le meme code que le comptoir.

use tauri::State;
use crate::commandes::ventes::EtatApp;

#[allow(unused_imports)]
pub use gescom_noyau::comptoir::lire_stock_multi_depots_sur;

#[tauri::command]
pub fn lire_depots_detail(
    etat: State<EtatApp>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::depots::lire_depots_detail(&conn, )
}
#[tauri::command]
pub fn creer_depot(
    etat: State<EtatApp>,
    nom: String,
    est_defaut: Option<bool>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::depots::creer_depot(&conn, nom, est_defaut)
}
#[tauri::command]
pub fn renommer_depot(
    etat: State<EtatApp>,
    depot_id: String,
    nom: String,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::depots::renommer_depot(&conn, depot_id, nom)
}
#[tauri::command]
pub fn definir_depot_defaut(
    etat: State<EtatApp>,
    depot_id: String,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::depots::definir_depot_defaut(&conn, depot_id)
}
#[tauri::command]
pub fn desactiver_depot(
    etat: State<EtatApp>,
    depot_id: String,
    force: Option<bool>,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::depots::desactiver_depot(&conn, depot_id, force)
}
#[allow(unused_imports)]
pub use gescom_noyau::depots::desactiver_depot_sur;
#[tauri::command]
pub fn reactiver_depot(
    etat: State<EtatApp>,
    depot_id: String,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::depots::reactiver_depot(&conn, depot_id)
}
#[allow(unused_imports)]
pub use gescom_noyau::depots::reactiver_depot_sur;
#[tauri::command]
pub fn lire_stock_depot(
    etat: State<EtatApp>,
    depot_id: String,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::depots::lire_stock_depot(&conn, depot_id)
}
#[tauri::command]
pub fn lire_resume_par_depot(
    etat: State<EtatApp>,
    date_debut: Option<String>,
    date_fin: Option<String>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::depots::lire_resume_par_depot(&conn, date_debut, date_fin)
}
#[tauri::command]
pub fn lire_stock_article_depots(
    etat: State<EtatApp>,
    article_id: String,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::depots::lire_stock_article_depots(&conn, article_id)
}
#[tauri::command]
pub fn lire_stock_multi_depots(
    etat: State<EtatApp>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::depots::lire_stock_multi_depots(&conn, )
}
#[tauri::command]
pub fn lire_mouvements_stock(
    etat: State<EtatApp>,
    article_id: Option<String>,
    depot_id: Option<String>,
    type_mouvement: Option<String>,
    date_debut: Option<String>,
    date_fin: Option<String>,
    limite: Option<i64>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::depots::lire_mouvements_stock(&conn, article_id, depot_id, type_mouvement, date_debut, date_fin, limite)
}
#[tauri::command]
pub fn lire_ventes_a_decouvert(
    etat: State<EtatApp>,
    date_debut: Option<String>,
    date_fin: Option<String>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::depots::lire_ventes_a_decouvert(&conn, date_debut, date_fin)
}
