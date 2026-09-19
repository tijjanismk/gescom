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
    // Le fichier a TOUJOURS un nom unique et l'extension .html — quel
    // que soit le nom demande. Deux impressions de la meme piece
    // donnaient le meme fichier : la webview le tenait encore, ou le
    // sortait de son cache, et « parfois » c'etait l'ancien document ou
    // une page blanche qui s'imprimait. Et un nom sans extension (le
    // numero de piece tel quel) laissait WebView2 deviner le type.
    let tmp_dir = std::env::temp_dir().join("gescom_impression");
    std::fs::create_dir_all(&tmp_dir)
        .map_err(|e| format!("Impossible de préparer le dossier d'impression : {}", e))?;
    nettoyer_les_anciens(&tmp_dir);
    let base: String = nom_fichier
        .unwrap_or_else(|| "document".to_string())
        .trim_end_matches(".html")
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .take(60)
        .collect();
    let nom = format!(
        "{}_{}.html",
        if base.is_empty() { "document".to_string() } else { base },
        chrono::Local::now().format("%Y%m%d_%H%M%S%3f")
    );
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

    // Un second essai avec un autre label : la fenetre precedente peut
    // ne pas avoir fini de se fermer, et Tauri refuse deux fenetres du
    // meme nom. Avant, l'impression echouait sur ce refus.
    let mut derniere_erreur = String::new();
    for essai in 0..2u8 {
        let label = if essai == 0 { label.clone() } else { format!("{label}_{essai}") };
        match WebviewWindowBuilder::new(&app, &label, WebviewUrl::External(url.clone()))
            .title("Impression — Gescom")
            .inner_size(900.0, 1000.0)
            .center()
            .resizable(true)
            .build()
        {
            Ok(_) => return Ok(chemin.to_string_lossy().to_string()),
            Err(e) => derniere_erreur = e.to_string(),
        }
    }
    Err(format!("Impossible d'ouvrir la fenêtre d'impression : {derniere_erreur}"))
}

/// Les documents de plus d'un jour : le dossier temporaire ne doit pas
/// grossir a chaque facture. Silencieux — un fichier qu'on ne peut pas
/// effacer n'empeche pas d'imprimer.
fn nettoyer_les_anciens(dossier: &std::path::Path) {
    let Ok(entrees) = std::fs::read_dir(dossier) else { return };
    let limite = std::time::SystemTime::now() - std::time::Duration::from_secs(24 * 3600);
    for e in entrees.flatten() {
        if let Ok(meta) = e.metadata() {
            if meta.modified().map(|m| m < limite).unwrap_or(false) {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
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
