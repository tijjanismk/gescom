//! Facades Tauri de `catalogue`.
//!
//! La logique vit dans `gescom_noyau::catalogue` : le serveur v2
//! execute exactement le meme code que le comptoir.

use tauri::State;
use crate::commandes::ventes::EtatApp;

#[allow(unused_imports)]
pub use gescom_noyau::catalogue_csv::echapper;
#[tauri::command]
pub fn exporter_articles_csv(
    etat: State<EtatApp>,
) -> Result<String, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::catalogue_csv::exporter_articles_csv(&conn, )
}
#[allow(unused_imports)]
pub use gescom_noyau::catalogue_csv::decouper;
#[allow(unused_imports)]
pub use gescom_noyau::catalogue_csv::normaliser_entete;
#[allow(unused_imports)]
pub use gescom_noyau::catalogue_csv::parser_montant;
#[allow(unused_imports)]
pub use gescom_noyau::catalogue_csv::parser_decimal;
#[tauri::command]
pub fn importer_articles_csv(
    etat: State<EtatApp>,
    contenu: String,
    mettre_a_jour: Option<bool>,
) -> Result<serde_json::Value, String> {
    let mut conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::catalogue_csv::importer_articles_csv(&mut conn, contenu, mettre_a_jour)
}
#[tauri::command]
pub fn lire_etat_stock(
    etat: State<EtatApp>,
    depot_id: Option<String>,
    avec_zero: Option<bool>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::catalogue_csv::lire_etat_stock(&conn, depot_id, avec_zero)
}
