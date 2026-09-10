//! Facades Tauri de `pieces`.
//!
//! La logique vit dans `gescom_noyau::pieces` : le serveur v2
//! execute exactement le meme code que le comptoir.

use tauri::State;
use crate::commandes::ventes::EtatApp;

#[allow(unused_imports)]
pub use gescom_noyau::argent::valider_facture_sur;

#[tauri::command]
pub fn lire_toutes_pieces_client(
    etat: State<EtatApp>,
    type_filtre: Option<String>,
    statut: Option<String>,
    recherche: Option<String>,
    date_debut: Option<String>,
    date_fin: Option<String>,
    montant_min: Option<i64>,
    montant_max: Option<i64>,
    impaye_seulement: Option<bool>,
    en_retard_seulement: Option<bool>,
    client_id: Option<String>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::pieces::lire_toutes_pieces_client(&conn, type_filtre, statut, recherche, date_debut, date_fin, montant_min, montant_max, impaye_seulement, en_retard_seulement, client_id)
}
#[tauri::command]
pub fn lire_pieces_client(
    etat: State<EtatApp>,
    client_id: String,
    type_filtre: Option<String>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::pieces::lire_pieces_client(&conn, client_id, type_filtre)
}
#[tauri::command]
pub fn lire_lignes_piece(
    etat: State<EtatApp>,
    piece_id: String,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::pieces::lire_lignes_piece(&conn, piece_id)
}
#[allow(unused_imports)]
pub use gescom_noyau::pieces::depot_de_piece;
#[allow(unused_imports)]
pub use gescom_noyau::pieces::LignePieceInput;
#[tauri::command]
pub fn creer_piece(
    etat: State<EtatApp>,
    client_id: String,
    type_piece: String,
    lignes: Vec<LignePieceInput>,
    remise_globale: Option<f64>,
    date_echeance: Option<String>,
    note: Option<String>,
    piece_origine_id: Option<String>,
    depot_id: Option<String>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::pieces::creer_piece(&conn, client_id, type_piece, lignes, remise_globale, date_echeance, note, piece_origine_id, depot_id)
}
#[allow(unused_imports)]
pub use gescom_noyau::pieces::creer_piece_sur;
#[allow(unused_imports)]
pub use gescom_noyau::pieces::inserer_lignes;
#[allow(unused_imports)]
pub use gescom_noyau::pieces::descendant_actif;
#[tauri::command]
pub fn convertir_piece(
    etat: State<EtatApp>,
    piece_id: String,
    nouveau_type: String,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::pieces::convertir_piece(&conn, piece_id, nouveau_type)
}
#[tauri::command]
pub fn convertir_commande_en_livraison_et_facture(
    etat: State<EtatApp>,
    piece_id: String,
) -> Result<serde_json::Value, String> {
    let mut conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::pieces::convertir_commande_en_livraison_et_facture(&mut conn, piece_id)
}
#[allow(unused_imports)]
pub use gescom_noyau::pieces::inserer_lignes_raw;
#[tauri::command]
pub fn valider_facture(
    etat: State<EtatApp>,
    piece_id: String,
    mode_reglement: String,
    mode_paiement: Option<String>,
    acompte: Option<i64>,
    utilisateur_role: Option<String>,
) -> Result<serde_json::Value, String> {
    let mut conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::pieces::valider_facture(&mut conn, piece_id, mode_reglement, mode_paiement, acompte, utilisateur_role)
}
#[tauri::command]
pub fn changer_statut_piece(
    etat: State<EtatApp>,
    piece_id: String,
    nouveau_statut: String,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::pieces::changer_statut_piece(&conn, piece_id, nouveau_statut)
}
#[tauri::command]
pub fn lire_donnees_piece(
    etat: State<EtatApp>,
    piece_id: String,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::pieces::lire_donnees_piece(&conn, piece_id)
}
#[tauri::command]
pub fn lire_fiche_client(
    etat: State<EtatApp>,
    client_id: String,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::pieces::lire_fiche_client(&conn, client_id)
}
#[tauri::command]
pub fn lire_toutes_pieces_fournisseur(
    etat: State<EtatApp>,
    type_filtre: Option<String>,
    statut: Option<String>,
    recherche: Option<String>,
    fournisseur_id: Option<String>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::pieces::lire_toutes_pieces_fournisseur(&conn, type_filtre, statut, recherche, fournisseur_id)
}
#[tauri::command]
pub fn creer_piece_fournisseur(
    etat: State<EtatApp>,
    fournisseur_id: String,
    type_piece: String,
    lignes: Vec<LignePieceInput>,
    remise_globale: Option<f64>,
    date_echeance: Option<String>,
    note: Option<String>,
    piece_origine_id: Option<String>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::pieces::creer_piece_fournisseur(&conn, fournisseur_id, type_piece, lignes, remise_globale, date_echeance, note, piece_origine_id)
}
#[tauri::command]
pub fn modifier_piece(
    etat: State<EtatApp>,
    piece_id: String,
    note: Option<String>,
    date_echeance: Option<String>,
    remise_globale: Option<f64>,
    lignes: Option<Vec<LignePieceInput>>,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::pieces::modifier_piece(&conn, piece_id, note, date_echeance, remise_globale, lignes)
}
#[tauri::command]
pub fn annuler_piece(
    etat: State<EtatApp>,
    piece_id: String,
    motif: Option<String>,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::pieces::annuler_piece(&conn, piece_id, motif)
}
#[tauri::command]
pub fn dupliquer_piece(
    etat: State<EtatApp>,
    piece_id: String,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::pieces::dupliquer_piece(&conn, piece_id)
}
#[tauri::command]
pub fn lire_piece_de_vente(
    etat: State<EtatApp>,
    vente_id: String,
) -> Result<Option<String>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::pieces::lire_piece_de_vente(&conn, vente_id)
}
#[tauri::command]
pub fn annuler_facture_par_avoir(
    etat: State<EtatApp>,
    piece_id: String,
    mode_remboursement: Option<String>,
    moyen: Option<String>,
    motif: Option<String>,
) -> Result<serde_json::Value, String> {
    let mut conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::pieces::annuler_facture_par_avoir(&mut conn, piece_id, mode_remboursement, moyen, motif)
}
#[allow(unused_imports)]
pub use gescom_noyau::pieces::annuler_facture_par_avoir_sur;
#[tauri::command]
pub fn lire_vente_de_piece(
    etat: State<EtatApp>,
    piece_id: String,
) -> Result<Option<String>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::pieces::lire_vente_de_piece(&conn, piece_id)
}

// Ouvre une fenetre Tauri : ne peut pas quitter le crate applicatif.
#[tauri::command]
/// Proxy vers l'impression. Depuis L36 elle ouvre une fenetre Tauri au
/// lieu du navigateur : elle est donc `async` et reclame l'AppHandle.
pub async fn imprimer_piece(
    app: tauri::AppHandle,
    html: String,
    nom_fichier: Option<String>,
) -> Result<String, String> {
    crate::commandes::impression::imprimer_facture(app, html, nom_fichier).await
}
