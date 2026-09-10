//! Facades Tauri de `pieces_pos`.
//!
//! La logique vit dans `gescom_noyau::pieces_pos` : le serveur v2
//! execute exactement le meme code que le comptoir.

use tauri::State;
use crate::commandes::ventes::EtatApp;

#[allow(unused_imports)]
pub use gescom_noyau::argent::creer_facture_depuis_vente_sur;

#[tauri::command]
pub fn creer_facture_depuis_vente(
    etat: State<EtatApp>,
    vente_id: String,
    client_id: String,
    mode_reglement: String,
    utilisateur_role: Option<String>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::pieces_pos::creer_facture_depuis_vente(&conn, vente_id, client_id, mode_reglement, utilisateur_role)
}
#[tauri::command]
pub fn modifier_facture_pos(
    etat: State<EtatApp>,
    piece_id: String,
    note: Option<String>,
    date_echeance: Option<String>,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::pieces_pos::modifier_facture_pos(&conn, piece_id, note, date_echeance)
}
#[tauri::command]
pub fn valider_facture_credit(
    etat: State<EtatApp>,
    piece_id: String,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::pieces_pos::valider_facture_credit(&conn, piece_id)
}
