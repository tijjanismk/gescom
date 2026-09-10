//! Facades Tauri de `journal`.
//!
//! La logique vit dans `gescom_noyau::journal` : le serveur v2
//! execute exactement le meme code que le comptoir.

use tauri::State;
use crate::commandes::ventes::EtatApp;

#[allow(unused_imports)]
pub use gescom_noyau::journal::jour;
#[tauri::command]
pub fn lire_journal_du_jour(
    etat: State<EtatApp>,
    date: Option<String>,
    depot_id: Option<String>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::journal::lire_journal_du_jour(&conn, date, depot_id)
}
