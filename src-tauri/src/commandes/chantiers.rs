//! Facades Tauri de `chantiers`.
//!
//! La logique vit dans `gescom_noyau::chantiers` : le serveur v2
//! execute exactement le meme code que le comptoir.

use tauri::State;
use crate::commandes::ventes::EtatApp;

#[tauri::command]
pub fn lire_taux_tva(
    etat: State<EtatApp>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::chantiers::lire_taux_tva(&conn, )
}
#[tauri::command]
pub fn sauvegarder_tva_article(
    etat: State<EtatApp>,
    article_id: String,
    taux_tva: f64,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::chantiers::sauvegarder_tva_article(&conn, article_id, taux_tva)
}
#[tauri::command]
pub fn lire_resume_tva(
    etat: State<EtatApp>,
    date_debut: String,
    date_fin: String,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::chantiers::lire_resume_tva(&conn, date_debut, date_fin)
}
#[tauri::command]
pub fn lire_dettes_fournisseurs(
    etat: State<EtatApp>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::chantiers::lire_dettes_fournisseurs(&conn, )
}
#[tauri::command]
pub fn regler_dette_fournisseur(
    etat: State<EtatApp>,
    fournisseur_id: String,
    montant: i64,
    mode: String,
    note: Option<String>,
    piece_id: Option<String>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::chantiers::regler_dette_fournisseur(&conn, fournisseur_id, montant, mode, note, piece_id)
}
#[allow(unused_imports)]
pub use gescom_noyau::chantiers::reimputer_paiements_globaux;
#[tauri::command]
pub fn marquer_irrecouvrable(
    etat: State<EtatApp>,
    vente_id: String,
    motif: String,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::chantiers::marquer_irrecouvrable(&conn, vente_id, motif)
}
#[tauri::command]
pub fn lire_irrecouvrable(
    etat: State<EtatApp>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::chantiers::lire_irrecouvrable(&conn, )
}
#[tauri::command]
pub fn lire_config_avoirs(
    etat: State<EtatApp>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::chantiers::lire_config_avoirs(&conn, )
}
#[tauri::command]
pub fn sauvegarder_config_avoirs(
    etat: State<EtatApp>,
    active: bool,
    duree_jours: i64,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::chantiers::sauvegarder_config_avoirs(&conn, active, duree_jours)
}
#[tauri::command]
pub fn expirer_avoirs(
    etat: State<EtatApp>,
) -> Result<i64, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::chantiers::expirer_avoirs(&conn, )
}
#[tauri::command]
pub fn lire_factures_fournisseur_ouvertes(
    etat: State<EtatApp>,
    fournisseur_id: String,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::chantiers::lire_factures_fournisseur_ouvertes(&conn, fournisseur_id)
}
#[tauri::command]
pub fn reactiver_avoir(
    etat: State<EtatApp>,
    avoir_id: String,
    utilisateur_role: Option<String>,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::chantiers::reactiver_avoir(&conn, avoir_id, utilisateur_role)
}
#[tauri::command]
pub fn lire_avoirs_expires(
    etat: State<EtatApp>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::chantiers::lire_avoirs_expires(&conn, )
}
