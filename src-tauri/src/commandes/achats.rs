//! Facades Tauri de `achats`.
//!
//! La logique vit dans `gescom_noyau::achats` : le serveur v2
//! execute exactement le meme code que le comptoir.

use tauri::State;
use crate::commandes::ventes::EtatApp;

#[allow(unused_imports)]
pub use gescom_noyau::achats::LigneAchat;
#[tauri::command]
pub fn enregistrer_achat(
    etat: State<EtatApp>,
    fournisseur_id: Option<String>,
    depot_id: Option<String>,
    lignes: Vec<LigneAchat>,
    mode_reglement: Option<String>,
    mode_paiement: Option<String>,
    acompte: Option<i64>,
    note: Option<String>,
    utilisateur_role: Option<String>,
    piece_origine_id: Option<String>,
) -> Result<serde_json::Value, String> {
    let mut conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::achats::enregistrer_achat(&mut conn, fournisseur_id, depot_id, lignes, mode_reglement, mode_paiement, acompte, note, utilisateur_role, piece_origine_id)
}
#[allow(unused_imports)]
pub use gescom_noyau::achats::LigneRetourFournisseur;
#[tauri::command]
pub fn enregistrer_retour_fournisseur(
    etat: State<EtatApp>,
    fournisseur_id: String,
    depot_id: Option<String>,
    lignes: Vec<LigneRetourFournisseur>,
    piece_origine_id: Option<String>,
    mode_resolution: Option<String>,
    mode_encaissement: Option<String>,
    motif: Option<String>,
    utilisateur_role: Option<String>,
) -> Result<serde_json::Value, String> {
    let mut conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::achats::enregistrer_retour_fournisseur(&mut conn, fournisseur_id, depot_id, lignes, piece_origine_id, mode_resolution, mode_encaissement, motif, utilisateur_role)
}
#[allow(unused_imports)]
pub use gescom_noyau::achats::enregistrer_retour_fournisseur_sur;
#[allow(unused_imports)]
pub use gescom_noyau::achats::verifier_stock_disponible;
#[tauri::command]
pub fn lire_factures_fournisseur_retournables(
    etat: State<EtatApp>,
    fournisseur_id: String,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::achats::lire_factures_fournisseur_retournables(&conn, fournisseur_id)
}

#[tauri::command]
pub fn valider_facture_fournisseur(
    etat: State<EtatApp>,
    piece_id: String,
    mode_reglement: String,
    mode_paiement: Option<String>,
    acompte: Option<i64>,
    utilisateur_role: Option<String>,
) -> Result<serde_json::Value, String> {
    let mut conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::achats::valider_facture_fournisseur(&mut conn, piece_id, mode_reglement, mode_paiement, acompte, utilisateur_role)
}

#[tauri::command]
pub fn annuler_facture_fournisseur_par_avoir(
    etat: State<EtatApp>,
    piece_id: String,
    mode_resolution: Option<String>,
    mode_encaissement: Option<String>,
    motif: Option<String>,
    utilisateur_role: Option<String>,
) -> Result<serde_json::Value, String> {
    let mut conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::achats::annuler_facture_fournisseur_par_avoir(&mut conn, piece_id, mode_resolution, mode_encaissement, motif, utilisateur_role)
}
