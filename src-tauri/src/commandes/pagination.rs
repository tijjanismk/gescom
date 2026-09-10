//! Facades Tauri de `pagination`.
//!
//! La logique vit dans `gescom_noyau::pagination` : le serveur v2
//! execute exactement le meme code que le comptoir.

use tauri::State;
use crate::commandes::ventes::EtatApp;

#[allow(unused_imports)]
pub use gescom_noyau::pagination::periode_vers_dates;
#[allow(unused_imports)]
pub use gescom_noyau::pagination::Filtres;
#[tauri::command]
pub fn lire_ventes_paginees(
    etat: State<EtatApp>,
    page: i64,
    limite: i64,
    recherche: Option<String>,
    statut: Option<String>,
    periode: Option<String>,
    date_debut: Option<String>,
    date_fin: Option<String>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::pagination::lire_ventes_paginees(&conn, page, limite, recherche, statut, periode, date_debut, date_fin)
}
#[tauri::command]
pub fn lire_clients_pagines(
    etat: State<EtatApp>,
    page: i64,
    limite: i64,
    recherche: Option<String>,
    avec_creances_seulement: bool,
    ventes_filtre: Option<String>,
    tri: Option<String>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::pagination::lire_clients_pagines(&conn, page, limite, recherche, avec_creances_seulement, ventes_filtre, tri)
}
#[tauri::command]
pub fn lire_stocks_pagines(
    etat: State<EtatApp>,
    page: i64,
    limite: i64,
    recherche: Option<String>,
    a_regulariser_seulement: bool,
    categorie_id: Option<String>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::pagination::lire_stocks_pagines(&conn, page, limite, recherche, a_regulariser_seulement, categorie_id)
}
#[tauri::command]
pub fn lire_fournisseurs_pagines(
    etat: State<EtatApp>,
    page: i64,
    limite: i64,
    recherche: Option<String>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::pagination::lire_fournisseurs_pagines(&conn, page, limite, recherche)
}
#[tauri::command]
pub fn lire_ventes_recentes_paginee(
    etat: State<EtatApp>,
    page: i64,
    limite: i64,
    recherche: Option<String>,
    periode: Option<String>,
    date_debut: Option<String>,
    date_fin: Option<String>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::pagination::lire_ventes_recentes_paginee(&conn, page, limite, recherche, periode, date_debut, date_fin)
}
