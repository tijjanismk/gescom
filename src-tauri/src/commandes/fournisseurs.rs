//! Facades Tauri de `fournisseurs`.
//!
//! La logique vit dans `gescom_noyau::fournisseurs` : le serveur v2
//! execute exactement le meme code que le comptoir.

use tauri::State;
use crate::commandes::ventes::EtatApp;

#[tauri::command]
pub fn lire_fournisseurs(
    etat: State<EtatApp>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::fournisseurs::lire_fournisseurs(&conn, )
}
#[tauri::command]
pub fn lire_fournisseurs_avec_dettes(
    etat: State<EtatApp>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::fournisseurs::lire_fournisseurs_avec_dettes(&conn, )
}
#[tauri::command]
pub fn creer_fournisseur(
    etat: State<EtatApp>,
    nom: String,
    telephone: Option<String>,
    adresse: Option<String>,
    nif: Option<String>,
    email: Option<String>,
    est_voisin: Option<bool>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::fournisseurs::creer_fournisseur(&conn, nom, telephone, adresse, nif, email, est_voisin)
}
#[tauri::command]
pub fn modifier_fournisseur(
    etat: State<EtatApp>,
    fournisseur_id: String,
    nom: String,
    telephone: Option<String>,
    adresse: Option<String>,
    nif: Option<String>,
    email: Option<String>,
    est_voisin: Option<bool>,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::fournisseurs::modifier_fournisseur(&conn, fournisseur_id, nom, telephone, adresse, nif, email, est_voisin)
}
#[tauri::command]
pub fn lire_etat_dette_fournisseur(
    etat: State<EtatApp>,
    fournisseur_id: String,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::fournisseurs::lire_etat_dette_fournisseur(&conn, fournisseur_id)
}
#[tauri::command]
pub fn lire_etat_dettes_global(
    etat: State<EtatApp>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::fournisseurs::lire_etat_dettes_global(&conn, )
}
#[tauri::command]
pub fn enregistrer_entree_stock(
    etat: State<EtatApp>,
    article_id: String,
    depot_id: Option<String>,
    quantite: f64,
    prix_achat: Option<i64>,
    fournisseur_id: Option<String>,
    utilisateur_role: Option<String>,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::fournisseurs::enregistrer_entree_stock(&conn, article_id, depot_id, quantite, prix_achat, fournisseur_id, utilisateur_role)
}
#[tauri::command]
pub fn enregistrer_retour_sans_facture(
    etat: State<EtatApp>,
    article_id: String,
    depot_id: Option<String>,
    quantite: f64,
    fournisseur_id: Option<String>,
    motif: Option<String>,
    utilisateur_role: Option<String>,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::fournisseurs::enregistrer_retour_sans_facture(&conn, article_id, depot_id, quantite, fournisseur_id, motif, utilisateur_role)
}
#[allow(unused_imports)]
pub use gescom_noyau::fournisseurs::enregistrer_retour_sans_facture_sur;
#[tauri::command]
pub fn enregistrer_ajustement_inventaire(
    etat: State<EtatApp>,
    article_id: String,
    depot_id: String,
    quantite_reelle: f64,
    motif: Option<String>,
    utilisateur_role: Option<String>,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::fournisseurs::enregistrer_ajustement_inventaire(&conn, article_id, depot_id, quantite_reelle, motif, utilisateur_role)
}
#[tauri::command]
pub fn lire_fournisseur_detail(
    etat: State<EtatApp>,
    fournisseur_id: String,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::fournisseurs::lire_fournisseur_detail(&conn, fournisseur_id)
}
#[tauri::command]
pub fn lire_fiche_fournisseur(
    etat: State<EtatApp>,
    fournisseur_id: String,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::fournisseurs::lire_fiche_fournisseur(&conn, fournisseur_id)
}
#[tauri::command]
pub fn annuler_paiement_fournisseur(
    etat: State<EtatApp>,
    paiement_id: String,
    motif: String,
    remboursement: bool,
    utilisateur_role: Option<String>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::fournisseurs::annuler_paiement_fournisseur(&conn, paiement_id, motif, remboursement, utilisateur_role)
}
