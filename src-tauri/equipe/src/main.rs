//! Gescom Equipe — la coque (PLAN-EQUIPE, D28-D29).
//!
//! Une fenetre qui affiche `equipe.html` et ne parle qu'au serveur
//! Gescom. Elle n'ouvre aucune base. Sa seule commande dit a la page ou
//! joindre le serveur : celui que la caisse de ce poste utilise deja
//! (`poste.json` de Gescom), sinon le serveur de cette machine (D22 :
//! en monoposte, on lance le serveur sur la meme base).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::Manager;

/// Le port par defaut du serveur Gescom (`gescom_noyau::protocole`).
const PORT_DEFAUT: u16 = 7300;

/// Ce que la page attend (`src/lib/pont.ts`, `lire_config_reseau`).
#[derive(Serialize)]
struct ConfigReseau {
    mode: String,
    serveur: String,
    poste_nom: String,
    poste_empreinte: String,
}

/// Le reglage de la caisse Gescom de ce poste, s'il existe.
#[derive(Deserialize, Default)]
struct PosteGescom {
    #[serde(default)]
    mode: String,
    #[serde(default)]
    serveur: String,
    #[serde(default)]
    poste_nom: String,
    #[serde(default)]
    poste_empreinte: String,
}

/// L'identite propre d'Equipe sur ce poste, tiree une fois.
#[derive(Serialize, Deserialize)]
struct IdentiteEquipe {
    empreinte: String,
}

fn nom_machine() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "Poste".to_string())
}

/// `poste.json` de Gescom : son dossier de donnees est le voisin du
/// notre (`ml.gescom.app` a cote de `ml.gescom.equipe`).
fn poste_gescom(nos_donnees: &PathBuf) -> PosteGescom {
    nos_donnees
        .parent()
        .map(|p| p.join("ml.gescom.app").join("poste.json"))
        .and_then(|c| std::fs::read_to_string(c).ok())
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

/// L'empreinte d'Equipe : la meme machine que la caisse, mais un autre
/// poste pour le serveur — sinon les deux fenetres se renommeraient
/// l'une l'autre dans la liste des postes.
fn empreinte(nos_donnees: &PathBuf, gescom: &PosteGescom) -> String {
    if !gescom.poste_empreinte.is_empty() {
        return format!("{}-equipe", gescom.poste_empreinte);
    }
    let fichier = nos_donnees.join("equipe.json");
    if let Some(i) = std::fs::read_to_string(&fichier)
        .ok()
        .and_then(|t| serde_json::from_str::<IdentiteEquipe>(&t).ok())
    {
        return i.empreinte;
    }
    let i = IdentiteEquipe { empreinte: uuid::Uuid::new_v4().to_string() };
    let _ = std::fs::create_dir_all(nos_donnees);
    let _ = std::fs::write(&fichier, serde_json::to_string_pretty(&i).unwrap_or_default());
    i.empreinte
}

#[tauri::command]
fn lire_config_reseau(app: tauri::AppHandle) -> Result<ConfigReseau, String> {
    let nos_donnees = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Répertoire de données introuvable : {e}"))?;
    let gescom = poste_gescom(&nos_donnees);
    // Toujours « poste » : Equipe n'a pas de base locale (D28).
    let serveur = if gescom.mode == "poste" && !gescom.serveur.trim().is_empty() {
        gescom.serveur.trim().to_string()
    } else {
        format!("127.0.0.1:{PORT_DEFAUT}")
    };
    let nom = if gescom.poste_nom.trim().is_empty() { nom_machine() } else { gescom.poste_nom.clone() };
    Ok(ConfigReseau {
        mode: "poste".to_string(),
        serveur,
        poste_nom: format!("{nom} (Équipe)"),
        poste_empreinte: empreinte(&nos_donnees, &gescom),
    })
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![lire_config_reseau])
        .run(tauri::generate_context!())
        .expect("Gescom Équipe n'a pas pu démarrer");
}
