//! Commandes dashboard — KPIs, top clients, top articles, ventes du jour.

use tauri::State;
use crate::commandes::ventes::EtatApp;

// =====================================================================
//  Résumé dashboard principal
// =====================================================================

#[tauri::command]
pub fn lire_resume_dashboard(
    etat: State<EtatApp>,
    depot_id: Option<String>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::tableau_bord::lire_resume_dashboard(&conn, depot_id)
}

// =====================================================================
//  Ventes par periode — jour (heures), semaine (jours),
//  mois (semaines), annee (mois)
// =====================================================================
//
// Une seule commande pour les quatre echelles : la difference tient au
// decoupage et au libelle, pas au calcul. Quatre commandes auraient fait
// quatre fois la meme somme, avec quatre occasions de diverger.


#[tauri::command]
pub fn lire_ventes_periode(
    etat: State<EtatApp>,
    periode: Option<String>,
    depot_id: Option<String>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::tableau_bord::lire_ventes_periode(&conn, periode, depot_id)
}

// =====================================================================
//  Top clients du mois (patron seulement)
// =====================================================================

#[tauri::command]
pub fn lire_top_clients(
    etat: State<EtatApp>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::tableau_bord::lire_top_clients(&conn, )
}

// =====================================================================
//  Top articles du mois (patron seulement)
// =====================================================================

#[tauri::command]
pub fn lire_top_articles(
    etat: State<EtatApp>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::tableau_bord::lire_top_articles(&conn, )
}
