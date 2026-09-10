//! Facades Tauri de `cheques`.
//!
//! La logique vit dans `gescom_noyau::cheques` : le serveur v2
//! execute exactement le meme code que le comptoir.

use tauri::State;
use crate::commandes::ventes::EtatApp;

#[tauri::command]
pub fn enregistrer_cheque(
    etat: State<EtatApp>,
    paiement_id: Option<String>,
    vente_id: Option<String>,
    numero: String,
    banque: String,
    tireur: Option<String>,
    montant: i64,
    date_emission: Option<String>,
    date_echeance: Option<String>,
) -> Result<String, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::cheques::enregistrer_cheque(&conn, paiement_id, vente_id, numero, banque, tireur, montant, date_emission, date_echeance)
}
#[tauri::command]
pub fn lire_cheques(
    etat: State<EtatApp>,
    statut: Option<String>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::cheques::lire_cheques(&conn, statut)
}
#[tauri::command]
pub fn changer_statut_cheque(
    etat: State<EtatApp>,
    cheque_id: String,
    statut: String,
    motif: Option<String>,
) -> Result<serde_json::Value, String> {
    let mut conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::cheques::changer_statut_cheque(&mut conn, cheque_id, statut, motif)
}
