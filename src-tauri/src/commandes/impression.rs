//! Commande Rust pour l'impression de factures.
//! Écrit le HTML dans un fichier temporaire et l'ouvre
//! avec le navigateur par défaut du système.

#![allow(unused_imports)]

use std::io::Write;

#[tauri::command]
pub fn imprimer_facture(
    html: String,
    nom_fichier: Option<String>,
) -> Result<String, String> {
    let tmp_dir = std::env::temp_dir();
    let nom = nom_fichier.unwrap_or_else(|| {
        format!(
            "gescom_facture_{}.html",
            chrono::Local::now().format("%Y%m%d_%H%M%S")
        )
    });
    let chemin = tmp_dir.join(&nom);
    let chemin_str = chemin.to_string_lossy().to_string();

    // Écrire le HTML
    std::fs::write(&chemin, html.as_bytes())
        .map_err(|e| format!("Impossible d'écrire le fichier : {}", e))?;

    // Ouvrir avec le programme par défaut du système
    ouvrir_fichier(&chemin_str)?;

    Ok(chemin_str)
}

fn ouvrir_fichier(chemin: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", chemin])
            .spawn()
            .map_err(|e| format!("Impossible d'ouvrir : {}", e))?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(chemin)
            .spawn()
            .map_err(|e| format!("Impossible d'ouvrir : {}", e))?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(chemin)
            .spawn()
            .map_err(|e| format!("Impossible d'ouvrir : {}", e))?;
    }
    Ok(())
}