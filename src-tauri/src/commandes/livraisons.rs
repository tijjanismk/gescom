//! Facades Tauri de `livraisons`.
//!
//! La logique vit dans `gescom_noyau::livraisons` : le serveur v2
//! execute exactement le meme code que le comptoir.

use tauri::State;
use crate::commandes::ventes::EtatApp;

#[allow(unused_imports)]
pub use gescom_noyau::livraisons::etat;
#[tauri::command]
pub fn lire_livraison_piece(
    etat_app: State<EtatApp>,
    piece_id: String,
) -> Result<serde_json::Value, String> {
    let conn = etat_app.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::livraisons::lire_livraison_piece(&conn, piece_id)
}
#[allow(unused_imports)]
pub use gescom_noyau::livraisons::LigneLivraison;
#[tauri::command]
pub fn enregistrer_livraison(
    etat_app: State<EtatApp>,
    piece_id: String,
    lignes: Vec<LigneLivraison>,
) -> Result<serde_json::Value, String> {
    let mut conn = etat_app.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::livraisons::enregistrer_livraison(&mut conn, piece_id, lignes)
}
