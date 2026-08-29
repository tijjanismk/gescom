//! Impression des documents commerciaux.
//!
//! Le HTML est ouvert dans une FENÊTRE TAURI, pas dans le navigateur
//! système.
//!
//! Pourquoi : le navigateur imprime ses propres en-têtes — date, titre
//! de l'onglet, et surtout le chemin `file:///C:/Users/.../Temp/...`.
//! Un client recevait donc une facture portant le chemin d'un fichier
//! temporaire. Ce n'est pas corrigeable en CSS : ces en-têtes sont
//! ajoutés par le navigateur, hors du document.
//!
//! Une webview Tauri n'ajoute rien. On garde toute la mise en page HTML
//! et on obtient un document propre.

#![allow(unused_imports)]

use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

/// Ouvre le HTML dans une fenêtre dédiée et déclenche l'impression.
///
/// Le script d'impression est déjà dans le document généré par
/// `genererPDF.ts` (`window.onload → print()`). La fenêtre reste
/// ouverte après la boîte de dialogue : l'utilisateur peut relancer
/// l'impression ou fermer.
#[tauri::command]
pub async fn imprimer_facture(
    app: tauri::AppHandle,
    html: String,
    nom_fichier: Option<String>,
) -> Result<String, String> {
    // Le fichier temporaire reste nécessaire : une webview ne charge
    // pas de façon fiable une longue chaîne HTML en data: URL sous
    // Windows (limite de longueur).
    let tmp_dir = std::env::temp_dir();
    let nom = nom_fichier.unwrap_or_else(|| {
        format!(
            "gescom_{}.html",
            chrono::Local::now().format("%Y%m%d_%H%M%S")
        )
    });
    let chemin = tmp_dir.join(&nom);

    std::fs::write(&chemin, html.as_bytes())
        .map_err(|e| format!("Impossible d'écrire le fichier : {}", e))?;

    // Fermer une éventuelle fenêtre d'impression restée ouverte, pour
    // ne pas encombrer l'écran du caissier.
    for (nom_fenetre, fenetre) in app.webview_windows() {
        if nom_fenetre.starts_with("impression_") {
            let _ = fenetre.close();
        }
    }

    // Label unique : deux impressions rapprochées ne doivent pas se
    // disputer la même fenêtre.
    let label = format!(
        "impression_{}",
        chrono::Local::now().format("%Y%m%d%H%M%S%3f")
    );

    let url = tauri::Url::from_file_path(&chemin)
        .map_err(|_| "Chemin de fichier invalide".to_string())?;

    WebviewWindowBuilder::new(&app, &label, WebviewUrl::External(url))
        .title("Impression — Gescom")
        .inner_size(900.0, 1000.0)
        .center()
        .resizable(true)
        .build()
        .map_err(|e| format!("Impossible d'ouvrir la fenêtre : {}", e))?;

    Ok(chemin.to_string_lossy().to_string())
}

/// Ouvre un fichier avec le programme par défaut du système.
///
/// Conservée pour l'export et le partage — plus utilisée par
/// l'impression, qui passe désormais par une fenêtre Tauri.
#[tauri::command]
pub fn ouvrir_avec_systeme(chemin: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", &chemin])
            .spawn()
            .map_err(|e| format!("Impossible d'ouvrir : {}", e))?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&chemin)
            .spawn()
            .map_err(|e| format!("Impossible d'ouvrir : {}", e))?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&chemin)
            .spawn()
            .map_err(|e| format!("Impossible d'ouvrir : {}", e))?;
    }
    Ok(())
}
