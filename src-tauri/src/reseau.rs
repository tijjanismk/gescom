//! Le client et son mode : seul, ou relie a un serveur.
//!
//! ## Deux executables, un seul code metier
//!
//! - `Gescom.exe` — la fenetre. C'est ce fichier qui lui dit ou parler.
//! - `gescom-serveur.exe` — le service qui detient la base.
//!
//! En **monoposte**, la fenetre ouvre la base locale et appelle ses
//! commandes par le pont Tauri, exactement comme en v1. Aucun reseau,
//! aucun service a installer : le commercant qui n'a qu'un ordinateur
//! ne paie rien pour un multiposte qu'il n'utilise pas.
//!
//! En **poste caisse**, la fenetre n'ouvre aucune base. Elle envoie ses
//! commandes au serveur par HTTP. C'est le front qui choisit le
//! transport (`src/lib/pont.ts`) ; ce module ne fait que porter le
//! reglage et l'identite du poste.
//!
//! ## Pourquoi l'identite du poste est un fichier, pas la base
//!
//! Un poste caisse ne doit pas dependre de sa base locale — il n'en a
//! pas. Son empreinte vit donc a cote, dans `poste.json`. Elle est
//! tiree une fois et ne bouge plus : c'est ce qui permet au serveur de
//! reconnaitre la machine d'un jour a l'autre, donc de la desactiver
//! si elle disparait du magasin.

use std::path::PathBuf;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::Manager;

use gescom_noyau::protocole::PORT_DEFAUT;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigReseau {
    /// `monoposte` ou `poste`.
    pub mode: String,
    /// Adresse du serveur, sans schema : « 192.168.1.10:7300 ».
    pub serveur: String,
    pub poste_nom: String,
    pub poste_empreinte: String,
}

impl Default for ConfigReseau {
    fn default() -> Self {
        ConfigReseau {
            // Le defaut est le comportement du v1. Une installation
            // existante qui se met a jour ne change pas de mode toute
            // seule un matin.
            mode: "monoposte".to_string(),
            serveur: format!("127.0.0.1:{PORT_DEFAUT}"),
            poste_nom: nom_machine(),
            poste_empreinte: uuid::Uuid::new_v4().to_string(),
        }
    }
}

fn nom_machine() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "Poste".to_string())
}

fn chemin_config(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dossier = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Répertoire de données introuvable : {e}"))?;
    std::fs::create_dir_all(&dossier).map_err(|e| e.to_string())?;
    Ok(dossier.join("poste.json"))
}

pub fn lire(app: &tauri::AppHandle) -> ConfigReseau {
    let Ok(chemin) = chemin_config(app) else {
        return ConfigReseau::default();
    };
    match std::fs::read_to_string(&chemin) {
        Ok(texte) => serde_json::from_str(&texte).unwrap_or_default(),
        Err(_) => {
            // Premier demarrage : on ecrit tout de suite, pour que
            // l'empreinte tiree maintenant soit celle de demain.
            let c = ConfigReseau::default();
            ecrire(app, &c).ok();
            c
        }
    }
}

fn ecrire(app: &tauri::AppHandle, c: &ConfigReseau) -> Result<(), String> {
    let chemin = chemin_config(app)?;
    let texte = serde_json::to_string_pretty(c).map_err(|e| e.to_string())?;
    std::fs::write(&chemin, texte).map_err(|e| e.to_string())
}

// =====================================================================
//  Commandes
// =====================================================================

#[tauri::command]
pub fn lire_config_reseau(app: tauri::AppHandle) -> Result<ConfigReseau, String> {
    Ok(lire(&app))
}

#[tauri::command]
pub fn definir_config_reseau(
    app: tauri::AppHandle,
    mode: String,
    serveur: String,
    poste_nom: String,
) -> Result<ConfigReseau, String> {
    if mode != "monoposte" && mode != "poste" {
        return Err(format!("Mode inconnu : « {mode} »."));
    }
    let mut c = lire(&app);
    // L'empreinte n'est JAMAIS regeneree ici : la reecrire ferait
    // apparaitre une machine de plus dans la liste des postes a chaque
    // changement de reglage, et le patron ne saurait plus laquelle
    // desactiver.
    c.mode = mode;
    c.serveur = serveur.trim().to_string();
    c.poste_nom = poste_nom.trim().to_string();
    if c.poste_nom.is_empty() {
        c.poste_nom = nom_machine();
    }
    ecrire(&app, &c)?;
    Ok(c)
}

/// Interroge `/sante` du serveur.
///
/// Fait en Rust et non depuis la fenetre : le diagnostic doit
/// distinguer « serveur injoignable » de « le navigateur embarque a
/// refuse la réponse », deux pannes que `fetch` renvoie a l'identique.
#[tauri::command]
pub fn tester_serveur(adresse: String) -> Result<serde_json::Value, String> {
    let adresse = adresse.trim().trim_start_matches("http://").to_string();
    let hote = adresse
        .split('/')
        .next()
        .filter(|s| !s.is_empty())
        .ok_or("Adresse vide.")?
        .to_string();

    let cible = hote
        .to_socket_addrs_premiere()
        .ok_or_else(|| format!("Adresse illisible : « {hote} »."))?;

    let mut flux = TcpStream::connect_timeout(&cible, Duration::from_secs(3))
        .map_err(|e| format!("Serveur injoignable sur {hote} : {e}"))?;
    flux.set_read_timeout(Some(Duration::from_secs(5))).ok();

    let requete = format!("GET /sante HTTP/1.1\r\nHost: {hote}\r\nConnection: close\r\n\r\n");
    flux.write_all(requete.as_bytes())
        .map_err(|e| e.to_string())?;

    let mut reponse = String::new();
    flux.read_to_string(&mut reponse).map_err(|e| e.to_string())?;

    let corps = reponse
        .split_once("\r\n\r\n")
        .map(|(_, c)| c)
        .ok_or("Réponse du serveur incomplète.")?;
    serde_json::from_str(corps)
        .map_err(|_| "Ce port répond, mais ce n'est pas un serveur Gescom.".to_string())
}

/// Petit utilitaire : « hote:port » -> adresse resolue.
trait ResoudrePremiere {
    fn to_socket_addrs_premiere(&self) -> Option<std::net::SocketAddr>;
}

impl ResoudrePremiere for String {
    fn to_socket_addrs_premiere(&self) -> Option<std::net::SocketAddr> {
        use std::net::ToSocketAddrs;
        // Sans port explicite on prend celui par defaut : le commercant
        // tape « 192.168.1.10 », pas « 192.168.1.10:7300 ».
        let avec_port = if self.contains(':') {
            self.clone()
        } else {
            format!("{self}:{PORT_DEFAUT}")
        };
        avec_port.to_socket_addrs().ok()?.next()
    }
}
