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
use std::io::Read;

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
fn lire_image_base64(
    app: &tauri::AppHandle,
    conn: &rusqlite::Connection,
    colonne: &str,
    base: &str,
) -> Result<Option<String>, String> {
    // `format!` sur un nom de COLONNE, pas sur une valeur : les deux
    // seules chaines possibles sont ecrites ici, jamais recues.
    let chemin_bd: Option<String> = conn.query_row(
        &format!("SELECT {} FROM parametres_societe WHERE id = 1", colonne),
        [], |r| r.get(0),
    ).ok().flatten();

    let chemin = match chemin_bd {
        Some(c) if !c.is_empty() => c,
        _ => {
            // Repli : image posee dans le repertoire de l'app sans que
            // le chemin ait ete enregistre.
            let data_dir = app.path().app_data_dir()
                .map_err(|e| e.to_string())?;
            let trouve = ["png", "jpg", "jpeg", "svg", "webp"].iter()
                .map(|e| data_dir.join(format!("{}.{}", base, e)))
                .find(|p| p.exists());
            match trouve {
                Some(p) => p.to_string_lossy().to_string(),
                None => return Ok(None),
            }
        }
    };

    let path = std::path::Path::new(&chemin);
    if !path.exists() {
        return Ok(None);
    }

    let ext = path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png")
        .to_lowercase();

    let mime = match ext.as_str() {
        "svg" => "image/svg+xml",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        _ => "image/png",
    };

    let mut fichier = std::fs::File::open(&chemin)
        .map_err(|e| format!("Impossible de lire l'image : {}", e))?;
    let mut buffer = Vec::new();
    fichier.read_to_end(&mut buffer)
        .map_err(|e| format!("Erreur lecture : {}", e))?;

    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&buffer);
    Ok(Some(format!("data:{};base64,{}", mime, b64)))
}

#[tauri::command]
pub fn lire_logo_base64(
    app: tauri::AppHandle,
    etat: State<EtatApp>,
) -> Result<Option<String>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    lire_image_base64(&app, &conn, "logo_chemin", "logo")
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
    lire_image_base64(&app, &conn, "pied_chemin", "pied")
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
    lire_image_base64(&app, &conn, "entete_chemin", "entete")
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