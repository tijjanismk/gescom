//! Facades Tauri de `avoirs`.
//!
//! La logique vit dans `gescom_noyau::avoirs` : le serveur v2
//! execute exactement le meme code que le comptoir.

use tauri::State;
use crate::commandes::ventes::EtatApp;

#[tauri::command]
pub fn lire_avoirs_client(
    etat: State<EtatApp>,
    client_id: String,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::avoirs::lire_avoirs_client(&conn, client_id)
}
#[tauri::command]
pub fn total_avoirs_client(
    etat: State<EtatApp>,
    client_id: String,
) -> Result<i64, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::avoirs::total_avoirs_client(&conn, client_id)
}
#[tauri::command]
pub fn appliquer_avoir_vente(
    etat: State<EtatApp>,
    vente_id: String,
    client_id: String,
    montant_demande: i64,
) -> Result<i64, String> {
    let mut conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::avoirs::appliquer_avoir_vente(&mut conn, vente_id, client_id, montant_demande)
}
#[tauri::command]
pub fn chercher_article_par_code_barre(
    etat: State<EtatApp>,
    code_barre: String,
) -> Result<Option<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::avoirs::chercher_article_par_code_barre(&conn, code_barre)
}
#[tauri::command]
pub fn lire_config_scanner(
    etat: State<EtatApp>,
) -> Result<bool, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::avoirs::lire_config_scanner(&conn, )
}
#[tauri::command]
pub fn sauvegarder_config_scanner(
    etat: State<EtatApp>,
    actif: bool,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::avoirs::sauvegarder_config_scanner(&conn, actif)
}
#[tauri::command]
pub fn sauvegarder_code_barre_article(
    etat: State<EtatApp>,
    article_id: String,
    code_barre: String,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::avoirs::sauvegarder_code_barre_article(&conn, article_id, code_barre)
}
#[tauri::command]
pub fn lire_articles_avec_codes_barres(
    etat: State<EtatApp>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::avoirs::lire_articles_avec_codes_barres(&conn, )
}
#[tauri::command]
pub fn rembourser_avoir(
    etat: State<EtatApp>,
    piece_id: String,
    montant: i64,
    mode: String,
    utilisateur_role: Option<String>,
) -> Result<serde_json::Value, String> {
    let mut conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::avoirs::rembourser_avoir(&mut conn, piece_id, montant, mode, utilisateur_role)
}
