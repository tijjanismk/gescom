//! Facades Tauri de `retours`.
//!
//! La logique vit dans `gescom_noyau::retours` : le serveur v2
//! execute exactement le meme code que le comptoir.

use tauri::State;
use crate::commandes::ventes::EtatApp;

#[tauri::command]
pub fn lire_ventes_recentes(
    etat: State<EtatApp>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::retours::lire_ventes_recentes(&conn, )
}
#[tauri::command]
pub fn enregistrer_retour(
    etat: State<EtatApp>,
    vente_id: String,
    ligne_vente_id: String,
    quantite: f64,
    mode_resolution: String,
    mode_encaissement: Option<String>,
    article_remplacement_id: Option<String>,
    unite_remplacement_id: Option<String>,
    quantite_remplacement: Option<f64>,
    mode_reliquat_positif: Option<String>,
    mode_encaissement_reliquat: Option<String>,
) -> Result<serde_json::Value, String> {
    let mut conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::retours::enregistrer_retour(&mut conn, vente_id, ligne_vente_id, quantite, mode_resolution, mode_encaissement, article_remplacement_id, unite_remplacement_id, quantite_remplacement, mode_reliquat_positif, mode_encaissement_reliquat)
}
#[allow(unused_imports)]
pub use gescom_noyau::retours::enregistrer_retour_sur;
#[allow(unused_imports)]
pub use gescom_noyau::retours::enregistrer_sortie_caisse;
#[allow(unused_imports)]
pub use gescom_noyau::retours::creer_avoir_client;
#[tauri::command]
pub fn lire_avoirs_ouverts_tous(
    etat: State<EtatApp>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::retours::lire_avoirs_ouverts_tous(&conn, )
}
