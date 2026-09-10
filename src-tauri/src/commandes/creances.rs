//! Facades Tauri de `creances`.
//!
//! La logique vit dans `gescom_noyau::creances` : le serveur v2
//! execute exactement le meme code que le comptoir.

use tauri::State;
use crate::commandes::ventes::EtatApp;

#[tauri::command]
pub fn lire_etat_creances_client(
    etat: State<EtatApp>,
    client_id: String,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::creances::lire_etat_creances_client(&conn, client_id)
}
#[tauri::command]
pub fn lire_etat_creances_global(
    etat: State<EtatApp>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::creances::lire_etat_creances_global(&conn, )
}
#[tauri::command]
pub fn lire_reglements_client(
    etat: State<EtatApp>,
    client_id: String,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::creances::lire_reglements_client(&conn, client_id)
}
#[tauri::command]
pub fn annuler_reglement(
    etat: State<EtatApp>,
    paiement_id: String,
    motif: String,
    remboursement: bool,
    utilisateur_role: Option<String>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::creances::annuler_reglement(&conn, paiement_id, motif, remboursement, utilisateur_role)
}
#[tauri::command]
pub fn lire_donnees_recu(
    etat: State<EtatApp>,
    paiement_id: String,
    cote: String,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::creances::lire_donnees_recu(&conn, paiement_id, cote)
}
#[allow(unused_imports)]
pub use gescom_noyau::creances::societe;
#[tauri::command]
pub fn lire_creances_ouvertes(
    etat: State<EtatApp>,
    recherche: Option<String>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::creances::lire_creances_ouvertes(&conn, recherche)
}
#[tauri::command]
pub fn regler_creance(
    etat: State<EtatApp>,
    vente_id: String,
    montant: i64,
    mode: String,
    utilisateur_role: Option<String>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::creances::regler_creance(&conn, vente_id, montant, mode, utilisateur_role)
}
#[tauri::command]
pub fn solder_residus_creances(
    etat: State<EtatApp>,
    utilisateur_role: Option<String>,
    simulation: Option<bool>,
) -> Result<serde_json::Value, String> {
    let mut conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::creances::solder_residus_creances(&mut conn, utilisateur_role, simulation)
}
