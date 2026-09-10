//! Facades Tauri de `caisse`.
//!
//! La logique vit dans `gescom_noyau::caisse` : le serveur v2
//! execute exactement le meme code que le comptoir.

use tauri::State;
use crate::commandes::ventes::EtatApp;

#[tauri::command]
pub fn lire_resume_caisse(
    etat: State<EtatApp>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::caisse::lire_resume_caisse(&conn, )
}
#[tauri::command]
pub fn lire_mouvements_caisse_du_jour(
    etat: State<EtatApp>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::caisse::lire_mouvements_caisse_du_jour(&conn, )
}
#[tauri::command]
pub fn ouvrir_session_caisse(
    etat: State<EtatApp>,
    fond_ouverture: i64,
    utilisateur_role: String,
) -> Result<String, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::caisse::ouvrir_session_caisse(&conn, fond_ouverture, utilisateur_role)
}
#[tauri::command]
pub fn fermer_session_caisse(
    etat: State<EtatApp>,
    session_id: String,
    especes_comptees: i64,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::caisse::fermer_session_caisse(&conn, session_id, especes_comptees)
}
#[tauri::command]
pub fn enregistrer_depense(
    etat: State<EtatApp>,
    montant: i64,
    libelle: String,
    categorie: Option<String>,
    moyen: Option<String>,
    utilisateur_role: Option<String>,
) -> Result<String, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::caisse::enregistrer_depense(&conn, montant, libelle, categorie, moyen, utilisateur_role)
}
#[tauri::command]
pub fn lire_depenses_du_jour(
    etat: State<EtatApp>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::caisse::lire_depenses_du_jour(&conn, )
}
#[tauri::command]
pub fn lire_sessions_caisse(
    etat: State<EtatApp>,
    limite: Option<i64>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::caisse::lire_sessions_caisse(&conn, limite)
}
#[tauri::command]
pub fn lire_mouvements_session(
    etat: State<EtatApp>,
    session_id: String,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::caisse::lire_mouvements_session(&conn, session_id)
}
#[tauri::command]
pub fn lire_rapport_ecarts(
    etat: State<EtatApp>,
    jours: Option<i64>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::caisse::lire_rapport_ecarts(&conn, jours)
}
#[tauri::command]
pub fn modifier_depense(
    etat: State<EtatApp>,
    mouvement_id: String,
    montant: Option<i64>,
    libelle: Option<String>,
    categorie: Option<String>,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::caisse::modifier_depense(&conn, mouvement_id, montant, libelle, categorie)
}
