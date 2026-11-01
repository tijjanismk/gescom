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
use std::net::TcpStream;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use gescom_noyau::decouverte::ServeurTrouve;
use gescom_noyau::recherche_serveur;
use gescom_noyau::protocole::{adresse_joignable, port_dans_la_plage, port_ecrit, ports_de_la_plage, PORT_DEFAUT, PORT_MAX};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigReseau {
    /// `monoposte`, `poste` ou `a_choisir` (voir l'en-tete).
    pub mode: String,
    /// Adresse du serveur, sans schema : « 192.168.1.10:40000 ».
    pub serveur: String,
    pub poste_nom: String,
    pub poste_empreinte: String,
}

/// L'adresse du serveur quand ce poste garde la base, au port par
/// defaut. Le serveur local a pu en prendre un autre de la plage (D39,
/// `demarrer_serveur_local`) : celui-la est retenu dans `poste.json`.
pub fn adresse_locale() -> String {
    format!("127.0.0.1:{PORT_DEFAUT}")
}

/// L'adresse locale ecrite dans `poste.json` si elle est sur cette
/// machine et dans la plage ; sinon celle du port par defaut. Un ancien
/// fichier peut porter 7300, abandonne le 27/10/2026 (D39).
fn adresse_locale_retenue(ecrite: &str) -> String {
    match port_ecrit(ecrite) {
        Some(Ok(p)) if port_dans_la_plage(p) && ecrite.trim().starts_with("127.0.0.1:") => format!("127.0.0.1:{p}"),
        _ => adresse_locale(),
    }
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
            // Le Bloc-notes enregistre en UTF-8 « avec BOM » : sans le
            // retirer, le fichier paraissait illisible, et le poste perdait
            // son role et son empreinte.
            let texte = texte.trim_start_matches('\u{feff}');
            let mut c: ConfigReseau = serde_json::from_str(texte).unwrap_or_default();
            // Ce poste garde la base : son serveur est ici, au port
            // retenu de la plage. Un ancien fichier peut porter une autre
            // adresse, inutilisee jusqu'ici.
            if c.mode == "monoposte" {
                c.serveur = adresse_locale_retenue(&c.serveur);
            } else if c.mode == "poste" && adresse_joignable(&c.serveur).is_err() {
                // Une caisse reglee sur un port que la fenetre ne joint
                // plus (7300, abandonne le 27/10/2026, D39) : on redemande
                // une fois, plutot que d'echouer a chaque appel.
                c.mode = "a_choisir".to_string();
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
    let mut c = lire(&app);
    // `a_choisir` ne s'ecrit pas : c'est l'etat d'un poste neuf, pas un
    // choix qu'on fait.
    let serveur = match mode.as_str() {
        // Ce poste garde la base : son serveur est ici, quoi qu'on passe,
        // au port deja retenu s'il y en a un.
        "monoposte" => adresse_locale_retenue(&c.serveur),
        "poste" if serveur.trim().is_empty() => {
            return Err("Une caisse a besoin de l'adresse du poste principal.".to_string())
        }
        "poste" => {
            adresse_joignable(serveur.trim())?;
            serveur.trim().to_string()
        }
        _ => return Err(format!("Mode inconnu : « {mode} ».")),
    };
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

/// Interroge `/sante` du serveur : la regle partagee avec Gescom Equipe
/// (`noyau::recherche_serveur`, D55). Refuse une adresse hors de la plage.
#[tauri::command]
pub fn tester_serveur(adresse: String) -> Result<serde_json::Value, String> {
    recherche_serveur::tester_serveur(&adresse)
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
    let mut c = lire(&app);
    if c.mode != "monoposte" {
        return Err("Ce poste est la caisse d'un autre poste : il ne lance pas de serveur.".to_string());
    }
    let arguments = arguments_serveur_local()?;
    let retenu = port_ecrit(&c.serveur).and_then(Result::ok).unwrap_or(PORT_DEFAUT);
    let (port, sante) = tauri::async_runtime::spawn_blocking(move || demarrer_et_attendre(arguments, retenu))
        .await
        .map_err(|e| e.to_string())??;
    // Le port pris reste ecrit : la page (`pont.ts`) et Gescom Equipe le
    // relisent dans `poste.json`.
    let adresse = format!("127.0.0.1:{port}");
    if c.serveur != adresse {
        c.serveur = adresse;
        ecrire(&app, &c)?;
    }
    Ok(sante)
}

/// Ce qu'on passe au serveur local, hors le port (choisi a chaque
/// lancement, voir `lancer_sur`). Livre : rien d'autre, comme le
/// raccourci. En debogage : le dossier du banc (`GESCOM_DONNEES`) —
/// sans lui, on refuse : un essai ouvrirait la vraie base du proprietaire.
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

/// Le serveur de ce poste, sur un port de la plage (D39) : celui qui
/// repond deja, sinon un nouveau sur le premier port libre — le port
/// retenu d'abord, puis 40000, 40001... Rend le port et `/sante`.
///
/// Un serveur Gescom qui repond deja sur cette machine est repris tel
/// quel, meme a un autre port que le retenu (le raccourci de demarrage
/// l'a lance sur 40000, par exemple) : en lancer un second sur la meme
/// base ferait deux serveurs qui s'ignorent.
fn demarrer_et_attendre(arguments: Vec<String>, retenu: u16) -> Result<(u16, serde_json::Value), String> {
    let _tour = UN_DEMARRAGE_A_LA_FOIS.lock().unwrap_or_else(|e| e.into_inner());
    let candidats: Vec<u16> = std::iter::once(retenu)
        .filter(|p| port_dans_la_plage(*p))
        .chain(ports_de_la_plage().filter(|p| *p != retenu))
        .collect();
    // Deja la ? Le port retenu, 40000 — celui du raccourci de demarrage —
    // et celui du service Windows de ce poste (serveur.json) : son port a
    // pu etre change depuis sa console, et le manquer ferait lancer ici un
    // SECOND serveur, sur une autre base. Pas toute la plage : sous
    // Windows, une connexion refusee sur 127.0.0.1 peut prendre deux
    // secondes ; cent ports, trois minutes.
    let mut deja_la = vec![retenu];
    for p in [Some(PORT_DEFAUT), port_du_service()].into_iter().flatten() {
        if !deja_la.contains(&p) {
            deja_la.push(p);
        }
    }
    for port in deja_la {
        if let Ok(sante) = tester_serveur(format!("127.0.0.1:{port}")) {
            return Ok((port, sante));
        }
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
    for port in candidats {
        match tester_serveur(format!("127.0.0.1:{port}")) {
            // Un serveur Gescom ici : le reprendre (voir plus haut).
            Ok(sante) => return Ok((port, sante)),
            // Quelque chose ecoute, mais pas Gescom : port pris.
            Err(e) if !e.starts_with("Serveur injoignable") => continue,
            Err(_) => {}
        }
        match lancer_sur(&exe, &arguments, port)? {
            Some(sante) => return Ok((port, sante)),
            // Un autre programme a pris ce port entre-temps : le suivant.
            None => continue,
        }
    }
    Err(format!(
        "Aucun port libre de {PORT_DEFAUT} à {PORT_MAX} sur cette machine : d'autres programmes les \
         tiennent tous. Gescom Serveur ne peut pas s'installer."
    ))
}

/// Lance le serveur sur ce port et attend qu'il reponde. `Ok(None)` : il
/// s'est arrete parce qu'un autre programme tient le port — essayer le
/// suivant. `Err` : il s'est arrete pour une autre raison, ou ne repond
/// pas ; le dire plutot qu'essayer cent ports pour rien.
fn lancer_sur(exe: &std::path::Path, arguments: &[String], port: u16) -> Result<Option<serde_json::Value>, String> {
    let adresse = format!("127.0.0.1:{port}");
    let mut commande = std::process::Command::new(exe);
    commande
        .args(arguments)
        .args(["--port", &port.to_string()])
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
            // repond, c'est gagne ; si quelqu'un d'autre le tient, on
            // passe au suivant.
            if let Ok(sante) = tester_serveur(adresse.clone()) {
                return Ok(Some(sante));
            }
            if let Some(cible) = recherche_serveur::premiere_adresse(&adresse) {
                if TcpStream::connect_timeout(&cible, Duration::from_secs(1)).is_ok() {
                    return Ok(None);
                }
            }
            return Err(format!(
                "Gescom Serveur s'est arrêté au démarrage ({statut}). Son journal, à côté de la base, dit pourquoi."
            ));
        }
        if let Ok(sante) = tester_serveur(adresse.clone()) {
            return Ok(Some(sante));
        }
    }
    Err("Gescom Serveur est lancé mais ne répond toujours pas après vingt secondes.".to_string())
}

/// Le port du service Gescom Serveur de ce poste, s'il a un `serveur.json`
/// (`%ProgramData%\Gescom`, D12) avec un port de la plage. En debogage,
/// `GESCOM_SERVEUR_JSON` le remplace, comme cote serveur.
fn port_du_service() -> Option<u16> {
    #[cfg(debug_assertions)]
    let remplace = std::env::var_os("GESCOM_SERVEUR_JSON").filter(|c| !c.is_empty()).map(PathBuf::from);
    #[cfg(not(debug_assertions))]
    let remplace: Option<PathBuf> = None;
    let chemin = remplace.unwrap_or_else(|| {
        std::env::var_os("ProgramData")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"))
            .join("Gescom")
            .join("serveur.json")
    });
    let texte = std::fs::read_to_string(chemin).ok()?;
    let v: serde_json::Value = serde_json::from_str(texte.trim_start_matches('\u{feff}')).ok()?;
    v.get("port")
        .and_then(|p| p.as_u64())
        .and_then(|p| u16::try_from(p).ok())
        .filter(|p| port_dans_la_plage(*p))
}

// =====================================================================
//  « Rechercher le serveur » (27/10/2026)
// =====================================================================

/// Les serveurs Gescom du reseau local (`noyau::recherche_serveur`, la
/// meme recherche que Gescom Equipe, D55). Rien n'est enregistre : l'ecran
/// remplit l'adresse, qui est ensuite TESTEE et enregistree comme une
/// adresse tapee.
#[tauri::command]
pub async fn rechercher_serveurs() -> Result<Vec<ServeurTrouve>, String> {
    tauri::async_runtime::spawn_blocking(recherche_serveur::rechercher).await.map_err(|e| e.to_string())?
}
