//! Facades Tauri de `sauvegarde`.
//!
//! La logique vit dans `gescom_noyau::sauvegarde` : le serveur v2
//! execute exactement le meme code que le comptoir.

use tauri::State;
use crate::commandes::ventes::EtatApp;

#[tauri::command]
pub fn sauvegarder_base(
    etat: State<EtatApp>,
    dossier_destination: String,
) -> Result<String, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::sauvegarde::sauvegarder_base(&conn, dossier_destination)
}
#[allow(unused_imports)]
pub use gescom_noyau::sauvegarde::effectuer_sauvegarde;
#[tauri::command]
pub fn lire_config_sauvegarde(
    etat: State<EtatApp>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::sauvegarde::lire_config_sauvegarde(&conn, )
}
#[tauri::command]
pub fn sauvegarde_auto_si_necessaire(
    etat: State<EtatApp>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::sauvegarde::sauvegarde_auto_si_necessaire(&conn, )
}
#[tauri::command]
pub fn sauvegarder_config_sauvegarde(
    etat: State<EtatApp>,
    dossier_sauvegarde: Option<String>,
    sauvegarde_auto: bool,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::sauvegarde::sauvegarder_config_sauvegarde(&conn, dossier_sauvegarde, sauvegarde_auto)
}
