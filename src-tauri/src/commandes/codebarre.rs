//! Facades Tauri de `codebarre`.
//!
//! La logique vit dans `gescom_noyau::codebarre` : le serveur v2
//! execute exactement le meme code que le comptoir.

use tauri::State;
use crate::commandes::ventes::EtatApp;

#[allow(unused_imports)]
pub use gescom_noyau::codebarre::reserver_sequence;
#[tauri::command]
pub fn generer_code_barre(
    etat: State<EtatApp>,
    article_id: String,
) -> Result<String, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::codebarre::generer_code_barre(&conn, article_id)
}
#[tauri::command]
pub fn generer_codes_barres_manquants(
    etat: State<EtatApp>,
) -> Result<serde_json::Value, String> {
    let mut conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::codebarre::generer_codes_barres_manquants(&mut conn, )
}
#[tauri::command]
pub fn definir_code_barre(
    etat: State<EtatApp>,
    article_id: String,
    code: String,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::codebarre::definir_code_barre(&conn, article_id, code)
}
#[tauri::command]
pub fn lire_articles_codes_barres(
    etat: State<EtatApp>,
    sans_code_seulement: Option<bool>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::codebarre::lire_articles_codes_barres(&conn, sans_code_seulement)
}
