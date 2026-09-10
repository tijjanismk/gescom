//! Facades Tauri de `transferts`.
//!
//! La logique vit dans `gescom_noyau::transferts` : le serveur v2
//! execute exactement le meme code que le comptoir.

use tauri::State;
use crate::commandes::ventes::EtatApp;

#[allow(unused_imports)]
pub use gescom_noyau::transferts::LigneTransfert;
#[allow(unused_imports)]
pub use gescom_noyau::transferts::reserver_bon;
#[tauri::command]
pub fn enregistrer_transfert(
    etat: State<EtatApp>,
    depot_source: String,
    depot_dest: String,
    lignes: Vec<LigneTransfert>,
    motif: Option<String>,
    utilisateur_role: Option<String>,
) -> Result<serde_json::Value, String> {
    let mut conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::transferts::enregistrer_transfert(&mut conn, depot_source, depot_dest, lignes, motif, utilisateur_role)
}
#[allow(unused_imports)]
pub use gescom_noyau::transferts::enregistrer_transfert_sur;
#[tauri::command]
pub fn lire_transferts(
    etat: State<EtatApp>,
    limite: Option<i64>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::transferts::lire_transferts(&conn, limite)
}
#[tauri::command]
pub fn lire_bon_transfert(
    etat: State<EtatApp>,
    bon: String,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::transferts::lire_bon_transfert(&conn, bon)
}
