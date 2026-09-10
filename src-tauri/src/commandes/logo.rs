//! Images de la societe — logo et en-tete, upload et lecture en base64.
//!
//! Deux images, deux usages distincts :
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

use tauri::{State, Manager};
use crate::commandes::ventes::EtatApp;

/// Copie une image dans le repertoire de l'app. `base` vaut "logo" ou
/// "entete" — le reste du traitement est identique.
fn copier_image(
    app: &tauri::AppHandle,
    chemin_source: &str,
    base: &str,
) -> Result<String, String> {
    let data_dir = app.path().app_data_dir()
        .map_err(|e| e.to_string())?;

    // Détecter l'extension
    let ext = std::path::Path::new(chemin_source)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png")
        .to_lowercase();

    if !["png", "jpg", "jpeg", "webp", "svg"].contains(&ext.as_str()) {
        return Err("Format non supporté — utiliser PNG, JPG ou SVG".to_string());
    }

    let dest = data_dir.join(format!("{}.{}", base, ext));

    std::fs::copy(chemin_source, &dest)
        .map_err(|e| format!("Impossible de copier l'image : {}", e))?;

    Ok(dest.to_string_lossy().to_string())
}

#[tauri::command]
pub fn sauvegarder_logo(
    app: tauri::AppHandle,
    chemin_source: String,
) -> Result<String, String> {
    copier_image(&app, &chemin_source, "logo")
}

#[tauri::command]
pub fn sauvegarder_entete(
    app: tauri::AppHandle,
    etat: State<EtatApp>,
    chemin_source: String,
) -> Result<String, String> {
    let chemin = copier_image(&app, &chemin_source, "entete")?;
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE parametres_societe SET entete_chemin = ?1 WHERE id = 1",
        rusqlite::params![chemin],
    ).map_err(|e| e.to_string())?;
    Ok(chemin)
}

/// Lit une image de la societe en base64, prete a etre integree dans
/// le HTML (D4). `colonne` = "logo_chemin" ou "entete_chemin",
/// `base` = "logo" ou "entete".
/// Le dossier de donnees de l'application, pour le repli.
///
/// La lecture elle-meme vit dans `noyau::images` : le serveur doit
/// pouvoir la faire aussi, et deux copies de cette regle auraient
/// diverge — c'est deja arrive au calcul de dette.
fn dossier_donnees(app: &tauri::AppHandle) -> Option<std::path::PathBuf> {
    app.path().app_data_dir().ok()
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
pub fn sauvegarder_pied(
    app: tauri::AppHandle,
    etat: State<EtatApp>,
    chemin_source: String,
) -> Result<String, String> {
    let chemin = copier_image(&app, &chemin_source, "pied")?;
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE parametres_societe SET pied_chemin = ?1 WHERE id = 1",
        rusqlite::params![chemin],
    ).map_err(|e| e.to_string())?;
    Ok(chemin)
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
pub fn supprimer_pied(etat: State<EtatApp>) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE parametres_societe SET pied_chemin = NULL WHERE id = 1", [],
    ).map_err(|e| e.to_string())?;
    Ok(())
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

/// Supprime le logo actuel.
#[tauri::command]
pub fn supprimer_logo(etat: State<EtatApp>) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE parametres_societe SET logo_chemin = NULL WHERE id = 1", [],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

/// Supprime l'en-tete. Le logo et les coordonnees reprennent leur
/// place a l'impression.
#[tauri::command]
pub fn supprimer_entete(etat: State<EtatApp>) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE parametres_societe SET entete_chemin = NULL WHERE id = 1", [],
    ).map_err(|e| e.to_string())?;
    Ok(())
}