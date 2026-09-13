//! Images de la societe — logo, en-tete et pied, depot du CONTENU et
//! lecture en base64.
//!
//! - Le LOGO se place a cote du bloc de coordonnees, en petit.
//! - L'EN-TETE est un bandeau pleine largeur qui REMPLACE le logo et
//!   les coordonnees : c'est le papier a en-tete que le commercant
//!   fait deja imprimer, retrouve a l'ecran.
//! - Le PIED est son pendant en bas de page : mentions legales,
//!   coordonnees bancaires, slogan. Il remplace la ligne de texte
//!   `pied_facture`.
//!
//! Ni l'un ni l'autre sur imprimante thermique : 58 ou 80 mm de large,
//! en noir et blanc, un bandeau ne donne qu'une tache grise.
//!
//! Depuis D8, la commande recoit le CONTENU du fichier en base64, pas
//! un chemin local : un chemin de caisse ne designe rien chez le
//! serveur. La logique (validation, depot, enregistrement) vit dans
//! `noyau::images`, pour que le serveur fasse exactement la meme chose.

use tauri::{State, Manager};
use crate::commandes::ventes::EtatApp;

/// Le dossier de donnees de l'application — la ou les images sont
/// deposees, et le repli de lecture.
fn dossier_donnees(app: &tauri::AppHandle) -> Option<std::path::PathBuf> {
    app.path().app_data_dir().ok()
}

#[tauri::command]
pub fn sauvegarder_logo(
    app: tauri::AppHandle,
    etat: State<EtatApp>,
    nom: String,
    contenu: String,
) -> Result<(), String> {
    let dossier = dossier_donnees(&app)
        .ok_or_else(|| "Aucun dossier de donnees pour ranger l'image.".to_string())?;
    let octets = gescom_noyau::images::decoder_base64(&contenu)?;
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::images::ecrire(&conn, "logo", &nom, &octets, &dossier)
}

#[tauri::command]
pub fn sauvegarder_entete(
    app: tauri::AppHandle,
    etat: State<EtatApp>,
    nom: String,
    contenu: String,
) -> Result<(), String> {
    let dossier = dossier_donnees(&app)
        .ok_or_else(|| "Aucun dossier de donnees pour ranger l'image.".to_string())?;
    let octets = gescom_noyau::images::decoder_base64(&contenu)?;
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::images::ecrire(&conn, "entete", &nom, &octets, &dossier)
}

#[tauri::command]
pub fn sauvegarder_pied(
    app: tauri::AppHandle,
    etat: State<EtatApp>,
    nom: String,
    contenu: String,
) -> Result<(), String> {
    let dossier = dossier_donnees(&app)
        .ok_or_else(|| "Aucun dossier de donnees pour ranger l'image.".to_string())?;
    let octets = gescom_noyau::images::decoder_base64(&contenu)?;
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::images::ecrire(&conn, "pied", &nom, &octets, &dossier)
}

#[tauri::command]
pub fn lire_logo_base64(
    app: tauri::AppHandle,
    etat: State<EtatApp>,
) -> Result<Option<String>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::images::lire_base64(
        &conn, "logo", dossier_donnees(&app).as_deref(),
    )
}

#[tauri::command]
pub fn lire_entete_base64(
    app: tauri::AppHandle,
    etat: State<EtatApp>,
) -> Result<Option<String>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::images::lire_base64(
        &conn, "entete", dossier_donnees(&app).as_deref(),
    )
}

#[tauri::command]
pub fn lire_pied_base64(
    app: tauri::AppHandle,
    etat: State<EtatApp>,
) -> Result<Option<String>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::images::lire_base64(
        &conn, "pied", dossier_donnees(&app).as_deref(),
    )
}

#[tauri::command]
pub fn supprimer_pied(
    app: tauri::AppHandle,
    etat: State<EtatApp>,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::images::supprimer(&conn, "pied", dossier_donnees(&app).as_deref())
}

/// Supprime le logo actuel.
#[tauri::command]
pub fn supprimer_logo(
    app: tauri::AppHandle,
    etat: State<EtatApp>,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::images::supprimer(&conn, "logo", dossier_donnees(&app).as_deref())
}

/// Supprime l'en-tete. Le logo et les coordonnees reprennent leur
/// place a l'impression.
#[tauri::command]
pub fn supprimer_entete(
    app: tauri::AppHandle,
    etat: State<EtatApp>,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::images::supprimer(&conn, "entete", dossier_donnees(&app).as_deref())
}
