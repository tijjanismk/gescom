//! Facades Tauri de `parametres`.
//!
//! La logique vit dans `gescom_noyau::parametres` : le serveur v2
//! execute exactement le meme code que le comptoir.

use tauri::State;
use crate::commandes::ventes::EtatApp;

#[tauri::command]
pub fn lire_categories(
    etat: State<EtatApp>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::parametres::lire_categories(&conn, )
}
#[tauri::command]
pub fn creer_categorie(
    etat: State<EtatApp>,
    nom: String,
) -> Result<String, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::parametres::creer_categorie(&conn, nom)
}
#[tauri::command]
pub fn lire_articles_complets(
    etat: State<EtatApp>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::parametres::lire_articles_complets(&conn, )
}
#[tauri::command]
pub fn creer_article_complet(
    etat: State<EtatApp>,
    nom: String,
    categorie_id: Option<String>,
    unite_base: String,
    prix_reference: i64,
) -> Result<String, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::parametres::creer_article_complet(&conn, nom, categorie_id, unite_base, prix_reference)
}
#[tauri::command]
pub fn ajouter_unite_vente(
    etat: State<EtatApp>,
    article_id: String,
    libelle: String,
    facteur: f64,
    prix_reference: i64,
    code_barre: Option<String>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::parametres::ajouter_unite_vente(&conn, article_id, libelle, facteur, prix_reference, code_barre)
}
#[tauri::command]
pub fn modifier_unite_vente(
    etat: State<EtatApp>,
    unite_id: String,
    libelle: Option<String>,
    facteur: Option<f64>,
    prix_reference: Option<i64>,
    code_barre: Option<String>,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::parametres::modifier_unite_vente(&conn, unite_id, libelle, facteur, prix_reference, code_barre)
}
#[tauri::command]
pub fn desactiver_unite_vente(
    etat: State<EtatApp>,
    unite_id: String,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::parametres::desactiver_unite_vente(&conn, unite_id)
}
#[tauri::command]
pub fn lire_config_bon_sortie(
    etat: State<EtatApp>,
) -> Result<bool, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::parametres::lire_config_bon_sortie(&conn, )
}
#[tauri::command]
pub fn sauvegarder_config_bon_sortie(
    etat: State<EtatApp>,
    actif: bool,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::parametres::sauvegarder_config_bon_sortie(&conn, actif)
}
#[tauri::command]
pub fn lire_config_suivi_livraison(
    etat: State<EtatApp>,
) -> Result<bool, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::parametres::lire_config_suivi_livraison(&conn, )
}
#[tauri::command]
pub fn sauvegarder_config_suivi_livraison(
    etat: State<EtatApp>,
    actif: bool,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::parametres::sauvegarder_config_suivi_livraison(&conn, actif)
}
#[tauri::command]
pub fn lire_config_signatures(
    etat: State<EtatApp>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::parametres::lire_config_signatures(&conn, )
}
#[tauri::command]
pub fn sauvegarder_config_signatures(
    etat: State<EtatApp>,
    valeurs: std::collections::HashMap<String, String>,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::parametres::sauvegarder_config_signatures(&conn, valeurs)
}
#[tauri::command]
pub fn lire_stocks(
    etat: State<EtatApp>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::parametres::lire_stocks(&conn, )
}
#[tauri::command]
pub fn diagnostiquer_base(
    etat: State<EtatApp>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::parametres::diagnostiquer_base(&conn, )
}
