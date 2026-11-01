//! Gescom Equipe — la coque (PLAN-EQUIPE, D28-D29).
//!
//! Une fenetre qui affiche `equipe.html` et ne parle qu'au serveur
//! Gescom. Elle n'ouvre aucune base. Ses commandes disent a la page ou
//! joindre le serveur, et laissent regler cette adresse depuis l'ecran de
//! connexion (D55) : la sienne si on l'a reglee ici (`equipe.json`), sinon
//! celle de la caisse de ce poste (`poste.json` de Gescom), sinon le
//! serveur de cette machine (D22 : en monoposte, on lance le serveur sur
//! la meme base). La regle, l'essai et la recherche du serveur sont ceux
//! de la caisse (`gescom_noyau::recherche_serveur`, D40) : il n'y en a
//! qu'un.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tauri::Manager;

use gescom_noyau::decouverte::ServeurTrouve;
use gescom_noyau::recherche_serveur::{self, AdresseEquipe};

/// Ce que la page attend (`src/lib/pont.ts`, `lire_config_reseau`).
#[derive(Serialize)]
struct ConfigReseau {
    mode: String,
    serveur: String,
    poste_nom: String,
    poste_empreinte: String,
    /// D55 : d'ou vient l'adresse — « equipe » (reglee ici), « caisse »
    /// (celle de la caisse de ce poste) ou « defaut » (cette machine).
    source: String,
    /// L'adresse de la caisse de ce poste, si elle en a une utilisable :
    /// l'ecran propose de la reprendre.
    serveur_caisse: Option<String>,
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

fn nom_machine() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "Poste".to_string())
}

fn nos_donnees(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map_err(|e| format!("Répertoire de données introuvable : {e}"))
}

/// `equipe.json` : l'empreinte d'Equipe et, depuis D55, son adresse.
fn fichier_equipe(nos_donnees: &Path) -> PathBuf {
    nos_donnees.join("equipe.json")
}

/// `poste.json` de Gescom : son dossier de donnees est le voisin du
/// notre (`ml.gescom.app` a cote de `ml.gescom.equipe`). Equipe le lit,
/// ne l'ecrit jamais : il porte aussi le role de la caisse.
fn poste_gescom(nos_donnees: &Path) -> PosteGescom {
    nos_donnees
        .parent()
        .map(|p| p.join("ml.gescom.app").join("poste.json"))
        .and_then(|c| std::fs::read_to_string(c).ok())
        // Le Bloc-notes peut y laisser un BOM : on le retire avant de lire.
        .and_then(|t| serde_json::from_str(t.trim_start_matches('\u{feff}')).ok())
        .unwrap_or_default()
}

/// L'empreinte d'Equipe : la meme machine que la caisse, mais un autre
/// poste pour le serveur — sinon les deux fenetres se renommeraient
/// l'une l'autre dans la liste des postes.
fn empreinte(nos_donnees: &Path, gescom: &PosteGescom) -> String {
    if !gescom.poste_empreinte.is_empty() {
        return format!("{}-equipe", gescom.poste_empreinte);
    }
    let fichier = fichier_equipe(nos_donnees);
    if let Some(e) = recherche_serveur::lire_reglage_equipe(&fichier).empreinte {
        return e;
    }
    let e = uuid::Uuid::new_v4().to_string();
    // Garde l'adresse deja reglee dans le meme fichier.
    let _ = recherche_serveur::ecrire_empreinte_equipe(&fichier, &e);
    e
}

/// L'adresse d'Equipe et sa source (`recherche_serveur::adresse_equipe`).
fn adresse(nos_donnees: &Path, gescom: &PosteGescom) -> AdresseEquipe {
    let propre = recherche_serveur::lire_reglage_equipe(&fichier_equipe(nos_donnees)).serveur;
    recherche_serveur::adresse_equipe(propre.as_deref(), &gescom.mode, &gescom.serveur)
}

fn config(app: &tauri::AppHandle) -> Result<ConfigReseau, String> {
    let nos_donnees = nos_donnees(app)?;
    let gescom = poste_gescom(&nos_donnees);
    let a = adresse(&nos_donnees, &gescom);
    // L'adresse de la caisse seule, telle qu'Equipe la prendrait sans la
    // sienne : ce que « Reprendre celle de la caisse » donnerait.
    let caisse = recherche_serveur::adresse_equipe(None, &gescom.mode, &gescom.serveur);
    let nom = if gescom.poste_nom.trim().is_empty() { nom_machine() } else { gescom.poste_nom.clone() };
    Ok(ConfigReseau {
        // Toujours « poste » : Equipe n'a pas de base locale (D28).
        mode: "poste".to_string(),
        serveur: a.serveur,
        poste_nom: format!("{nom} (Équipe)"),
        poste_empreinte: empreinte(&nos_donnees, &gescom),
        source: a.source.to_string(),
        serveur_caisse: (caisse.source == recherche_serveur::SOURCE_CAISSE).then_some(caisse.serveur),
    })
}

#[tauri::command]
fn lire_config_reseau(app: tauri::AppHandle) -> Result<ConfigReseau, String> {
    config(&app)
}

/// L'ecran de reglage (D55) enregistre l'adresse d'Equipe, apres un essai
/// reussi. Meme signature que dans la caisse ; seul `poste` a un sens ici
/// — Equipe ne garde jamais de base — et le nom du poste ne se regle pas :
/// il suit celui de la caisse. `serveur` absent : on reprend l'adresse de
/// la caisse de ce poste.
#[tauri::command]
fn definir_config_reseau(
    app: tauri::AppHandle,
    mode: String,
    serveur: Option<String>,
) -> Result<ConfigReseau, String> {
    if mode != "poste" {
        return Err("Gescom Équipe ne garde pas de base : elle se branche sur le serveur de la boutique.".to_string());
    }
    let nos_donnees = nos_donnees(&app)?;
    recherche_serveur::enregistrer_adresse_equipe(&fichier_equipe(&nos_donnees), serveur.as_deref())?;
    config(&app)
}

#[tauri::command]
fn tester_serveur(adresse: String) -> Result<serde_json::Value, String> {
    recherche_serveur::tester_serveur(&adresse)
}

/// La recherche de la caisse (D40), sans attendre sur le fil de la fenetre.
#[tauri::command]
async fn rechercher_serveurs() -> Result<Vec<ServeurTrouve>, String> {
    tauri::async_runtime::spawn_blocking(recherche_serveur::rechercher)
        .await
        .map_err(|e| e.to_string())?
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            lire_config_reseau,
            definir_config_reseau,
            tester_serveur,
            rechercher_serveurs
        ])
        .run(tauri::generate_context!())
        .expect("Gescom Équipe n'a pas pu démarrer");
}
