//! Gestion du logo société — upload et lecture en base64.

use tauri::{State, Manager};
use crate::commandes::ventes::EtatApp;
use std::io::Read;

/// Copie le logo choisi dans le répertoire de l'app et retourne le chemin.
#[tauri::command]
pub fn sauvegarder_logo(
    app: tauri::AppHandle,
    chemin_source: String,
) -> Result<String, String> {
    let data_dir = app.path().app_data_dir()
        .map_err(|e| e.to_string())?;

    // Détecter l'extension
    let ext = std::path::Path::new(&chemin_source)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png")
        .to_lowercase();

    if !["png", "jpg", "jpeg", "webp", "svg"].contains(&ext.as_str()) {
        return Err("Format non supporté — utiliser PNG, JPG ou SVG".to_string());
    }

    let dest = data_dir.join(format!("logo.{}", ext));

    std::fs::copy(&chemin_source, &dest)
        .map_err(|e| format!("Impossible de copier le logo : {}", e))?;

    Ok(dest.to_string_lossy().to_string())
}

/// Lit le logo et le retourne en base64 pour l'intégrer dans le HTML de facture.
#[tauri::command]
pub fn lire_logo_base64(
    app: tauri::AppHandle,
    etat: State<EtatApp>,
) -> Result<Option<String>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    // Lire le chemin du logo depuis les paramètres
    let chemin_logo: Option<String> = conn.query_row(
        "SELECT logo_chemin FROM parametres_societe WHERE id = 1",
        [], |r| r.get(0),
    ).ok().flatten();

    let chemin = match chemin_logo {
        Some(c) if !c.is_empty() => c,
        _ => {
            // Chercher un logo dans le répertoire de l'app
            let data_dir = app.path().app_data_dir()
                .map_err(|e| e.to_string())?;
            let candidats = ["logo.png", "logo.jpg", "logo.jpeg", "logo.svg"];
            let trouve = candidats.iter()
                .map(|f| data_dir.join(f))
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
        .map_err(|e| format!("Impossible de lire le logo : {}", e))?;
    let mut buffer = Vec::new();
    fichier.read_to_end(&mut buffer)
        .map_err(|e| format!("Erreur lecture : {}", e))?;

    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&buffer);
    Ok(Some(format!("data:{};base64,{}", mime, b64)))
}

/// Supprime le logo actuel.
#[tauri::command]
pub fn supprimer_logo(
    etat: State<EtatApp>,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE parametres_societe SET logo_chemin = NULL WHERE id = 1",
        [],
    ).map_err(|e| e.to_string())?;
    Ok(())
}