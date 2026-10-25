//! Le client et son role : il garde la base, ou il est la caisse d'un
//! autre poste.
//!
//! ## Deux executables, un seul code metier
//!
//! - `Gescom.exe` — la fenetre. C'est ce fichier qui lui dit ou parler.
//! - `gescom-serveur.exe` — le service qui detient la base.
//!
//! La fenetre ne parle JAMAIS qu'au serveur (le 11/09/2026 : deux
//! chemins qui se contredisaient ont fait croire a un commercant que
//! ses donnees avaient disparu). Ce qui change d'un role a l'autre,
//! c'est OU est ce serveur (D22/D24, revises le 25/10/2026) :
//!
//! - **`monoposte`** — ce poste garde la base : la boutique a une
//!   caisse, ou le poste principal d'une boutique a plusieurs. Le
//!   serveur est sur cette machine (`127.0.0.1`) ; s'il ne repond pas,
//!   la fenetre lance celui qui est livre a cote d'elle
//!   (`demarrer_serveur_local`). Rien a saisir.
//! - **`poste`** — la caisse d'un autre poste : l'adresse se saisit, se
//!   teste, puis s'enregistre. Elle ne lance jamais de serveur.
//! - **`a_choisir`** — premier demarrage : la fenetre demande, une fois.
//!   On ne devine jamais le role d'un poste ; une installation existante
//!   garde le sien.
//!
//! C'est le front qui choisit le transport (`src/lib/pont.ts`) ; ce
//! module porte le reglage, l'identite du poste, et le demarrage du
//! serveur local.
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

use gescom_noyau::protocole::PORT_DEFAUT;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigReseau {
    /// `monoposte`, `poste` ou `a_choisir` (voir l'en-tete).
    pub mode: String,
    /// Adresse du serveur, sans schema : « 192.168.1.10:7300 ».
    pub serveur: String,
    pub poste_nom: String,
    pub poste_empreinte: String,
}

/// L'adresse du serveur quand ce poste garde la base.
pub fn adresse_locale() -> String {
    format!("127.0.0.1:{PORT_DEFAUT}")
}

impl Default for ConfigReseau {
    fn default() -> Self {
        ConfigReseau {
            // Un poste.json neuf (ou illisible) ne dit pas le role du
            // poste : on le demande. Le deviner « monoposte » faisait
            // d'une caisse fraichement installee un poste principal,
            // avec son serveur et sa base vide. Une installation
            // existante garde le mode ecrit dans son fichier.
            mode: "a_choisir".to_string(),
            serveur: String::new(),
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
    let dossier = crate::dossier_donnees(app)?;
    std::fs::create_dir_all(&dossier).map_err(|e| e.to_string())?;
    Ok(dossier.join("poste.json"))
}

pub fn lire(app: &tauri::AppHandle) -> ConfigReseau {
    let Ok(chemin) = chemin_config(app) else {
        return ConfigReseau::default();
    };
    match std::fs::read_to_string(&chemin) {
        Ok(texte) => {
            let mut c: ConfigReseau = serde_json::from_str(&texte).unwrap_or_default();
            // Ce poste garde la base : son serveur est ici. Un ancien
            // fichier peut porter une autre adresse, inutilisee jusqu'ici.
            if c.mode == "monoposte" {
                c.serveur = adresse_locale();
            }
            c
        }
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
    serveur: Option<String>,
    poste_nom: Option<String>,
) -> Result<ConfigReseau, String> {
    let serveur = serveur.unwrap_or_default();
    // `a_choisir` ne s'ecrit pas : c'est l'etat d'un poste neuf, pas un
    // choix qu'on fait.
    let serveur = match mode.as_str() {
        // Ce poste garde la base : son serveur est ici, quoi qu'on passe.
        "monoposte" => adresse_locale(),
        "poste" if serveur.trim().is_empty() => {
            return Err("Une caisse a besoin de l'adresse du poste principal.".to_string())
        }
        "poste" => serveur.trim().to_string(),
        _ => return Err(format!("Mode inconnu : « {mode} ».")),
    };
    let mut c = lire(&app);
    // L'empreinte n'est JAMAIS regeneree ici : la reecrire ferait
    // apparaitre une machine de plus dans la liste des postes a chaque
    // changement de reglage, et le patron ne saurait plus laquelle
    // desactiver.
    c.mode = mode;
    c.serveur = serveur;
    c.poste_nom = poste_nom.unwrap_or_default().trim().to_string();
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

/// Le serveur de CETTE machine, demarre s'il ne repond pas.
///
/// Seulement pour le poste qui garde la base (`monoposte`) : une caisse
/// ne lance jamais de serveur — le sien aurait une base vide, et la
/// vraie est sur le poste principal. Lance comme le raccourci de
/// demarrage (memes defauts : meme base, meme journal), sans fenetre,
/// et survit a la fermeture de Gescom : la sauvegarde automatique et
/// Gescom Equipe en ont besoin. Rend `/sante`, ou dit pourquoi non.
///
/// Asynchrone : jusqu'a vingt secondes d'attente, la fenetre ne doit
/// pas geler pendant ce temps.
#[tauri::command]
pub async fn demarrer_serveur_local(app: tauri::AppHandle) -> Result<serde_json::Value, String> {
    let c = lire(&app);
    if c.mode != "monoposte" {
        return Err("Ce poste est la caisse d'un autre poste : il ne lance pas de serveur.".to_string());
    }
    let arguments = arguments_serveur_local()?;
    tauri::async_runtime::spawn_blocking(move || demarrer_et_attendre(arguments))
        .await
        .map_err(|e| e.to_string())?
}

/// Ce qu'on passe au serveur local. Livre : rien, comme le raccourci.
/// En debogage : le dossier du banc (`GESCOM_DONNEES`) — sans lui, on
/// refuse : un essai ouvrirait la vraie base du proprietaire.
fn arguments_serveur_local() -> Result<Vec<String>, String> {
    #[cfg(debug_assertions)]
    {
        let Some(d) = std::env::var_os("GESCOM_DONNEES").filter(|d| !d.is_empty()) else {
            return Err("Build de débogage : la fenêtre ne lance pas de serveur sur la vraie base. \
                        Lancer gescom-serveur à la main, ou poser GESCOM_DONNEES (banc d'écran)."
                .to_string());
        };
        let d = PathBuf::from(d);
        let chemin = |f: &str| d.join(f).to_string_lossy().to_string();
        Ok(vec![
            "--hote".into(), "127.0.0.1".into(),
            "--port".into(), PORT_DEFAUT.to_string(),
            "--base".into(), chemin("serveur.db"),
            "--journal".into(), chemin("serveur.log"),
            "--sauvegardes".into(), chemin("sauvegardes"),
        ])
    }
    #[cfg(not(debug_assertions))]
    Ok(Vec::new())
}

/// Une demande a la fois : deux demandes simultanees (l'ecran remonte,
/// un double clic) lancaient deux serveurs ; le second trouvait le port
/// pris, s'arretait, et son echec s'affichait alors que le premier
/// repondait. La seconde attend la premiere, puis trouve le serveur.
static UN_DEMARRAGE_A_LA_FOIS: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn demarrer_et_attendre(arguments: Vec<String>) -> Result<serde_json::Value, String> {
    let _tour = UN_DEMARRAGE_A_LA_FOIS.lock().unwrap_or_else(|e| e.into_inner());
    let adresse = adresse_locale();
    match tester_serveur(adresse.clone()) {
        Ok(sante) => return Ok(sante),
        // Le port repond, mais pas Gescom : un second serveur echouerait
        // sur le meme port. Le dire, plutot que d'attendre vingt secondes.
        Err(e) if e.contains("pas un serveur Gescom") => {
            return Err(format!(
                "Le port {PORT_DEFAUT} de cette machine est pris par un autre programme : \
                 Gescom Serveur ne peut pas s'y installer. {e}"
            ))
        }
        Err(_) => {}
    }

    let exe = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .parent()
        .map(|d| d.join(if cfg!(windows) { "gescom-serveur.exe" } else { "gescom-serveur" }))
        .ok_or("Dossier de Gescom introuvable.")?;
    if !exe.exists() {
        return Err(format!(
            "Gescom Serveur est introuvable à côté de Gescom ({}). Réinstaller Gescom.",
            exe.display()
        ));
    }
    let mut commande = std::process::Command::new(&exe);
    commande
        .args(&arguments)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP : pas de fenetre
        // noire a fermer par megarde, et il survit a la fermeture de Gescom.
        commande.creation_flags(0x0800_0000 | 0x0000_0200);
    }
    let mut enfant = commande
        .spawn()
        .map_err(|e| format!("Gescom Serveur n'a pas pu démarrer : {e}"))?;

    let debut = std::time::Instant::now();
    while debut.elapsed() < Duration::from_secs(20) {
        std::thread::sleep(Duration::from_millis(500));
        if let Ok(Some(statut)) = enfant.try_wait() {
            // Arrete : peut-etre parce qu'un autre (le raccourci de
            // demarrage, lance au meme moment) a pris le port. S'il
            // repond, c'est gagne.
            if let Ok(sante) = tester_serveur(adresse.clone()) {
                return Ok(sante);
            }
            return Err(format!(
                "Gescom Serveur s'est arrêté au démarrage ({statut}). Son journal, à côté de la base, dit pourquoi."
            ));
        }
        if let Ok(sante) = tester_serveur(adresse.clone()) {
            return Ok(sante);
        }
    }
    Err("Gescom Serveur est lancé mais ne répond toujours pas après vingt secondes.".to_string())
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
