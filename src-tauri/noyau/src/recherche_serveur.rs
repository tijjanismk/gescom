//! Trouver le serveur depuis un poste : la caisse et Gescom Equipe (D40, D55).
//!
//! `decouverte` porte le FORMAT de la question et de la reponse ; ici le
//! geste du poste, partage par les deux fenetres pour qu'il n'y ait
//! qu'une seule recherche (D55) :
//!
//! - `rechercher` : la question part en diffusion UDP sur le port 40000
//!   (et a ce poste meme), on ecoute deux secondes, une ligne par serveur ;
//! - `tester_serveur` : `/sante` d'une adresse, tapee ou trouvee, avant
//!   de l'enregistrer — une adresse acceptee sans essai donne un poste qui
//!   parait branche et qui echoue au premier geste ;
//! - l'adresse de Gescom Equipe (`equipe.json`), qui peut desormais
//!   recevoir la sienne depuis son ecran de connexion.
//!
//! Rien ici n'ouvre de base ni ne connait Tauri : Gescom Equipe s'en sert
//! sans base (D28).

use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream, ToSocketAddrs, UdpSocket};
use std::path::Path;
use std::time::{Duration, Instant};

use crate::decouverte::{self, ServeurTrouve};
use crate::protocole::{adresse_joignable, PORT_DEFAUT};

// =====================================================================
//  La recherche (D40)
// =====================================================================

/// Les serveurs Gescom du reseau local. Rien n'est enregistre : l'ecran
/// remplit l'adresse, qui est ensuite TESTEE et enregistree comme une
/// adresse tapee. Une liste vide n'est pas une erreur : l'ecran dit les
/// causes probables et laisse saisir a la main.
pub fn rechercher() -> Result<Vec<ServeurTrouve>, String> {
    let socket = UdpSocket::bind("0.0.0.0:0").map_err(|e| format!("Recherche impossible sur ce poste : {e}"))?;
    socket
        .set_broadcast(true)
        .map_err(|e| format!("Recherche impossible sur ce poste : {e}"))?;
    let question = decouverte::question();
    // 255.255.255.255, la diffusion du reseau de ce poste (un /24, celui
    // des box : Windows n'envoie pas toujours la premiere sur toutes les
    // cartes), et ce poste lui-meme (un serveur ici, ecoutant ce seul poste).
    let mut cibles: Vec<SocketAddr> = vec![
        SocketAddr::from((Ipv4Addr::BROADCAST, decouverte::PORT)),
        SocketAddr::from((Ipv4Addr::LOCALHOST, decouverte::PORT)),
    ];
    if let Some(ip) = adresse_locale_ipv4() {
        let o = ip.octets();
        cibles.push(SocketAddr::from((Ipv4Addr::new(o[0], o[1], o[2], 255), decouverte::PORT)));
    }
    let envoyer = |cibles: &[SocketAddr]| cibles.iter().filter(|c| socket.send_to(&question, c).is_ok()).count();
    if envoyer(&cibles) == 0 {
        return Err("La question n'a pas pu partir sur le réseau : ce poste a-t-il une carte réseau branchée ?".to_string());
    }

    // Deux secondes d'ecoute ; la question repart une fois a mi-chemin :
    // un paquet UDP peut se perdre, surtout en Wi-Fi.
    let debut = Instant::now();
    let duree = Duration::from_secs(2);
    let mut relancee = false;
    let mut recues: Vec<(decouverte::Annonce, IpAddr)> = Vec::new();
    let mut tampon = vec![0u8; decouverte::TAILLE_REPONSE_MAX];
    while debut.elapsed() < duree {
        if !relancee && debut.elapsed() >= duree / 2 {
            envoyer(&cibles);
            relancee = true;
        }
        let reste = duree.saturating_sub(debut.elapsed()).clamp(Duration::from_millis(10), Duration::from_millis(250));
        socket.set_read_timeout(Some(reste)).ok();
        // Une erreur ici : le delai, ou, sous Windows, la question envoyee
        // a ce poste sans serveur qui revient en « connexion
        // reinitialisee ». On continue d'ecouter.
        if let Ok((n, de)) = socket.recv_from(&mut tampon) {
            if let Some(a) = decouverte::lire_reponse(&tampon[..n]) {
                if recues.len() < 64 {
                    recues.push((a, de.ip()));
                }
            }
        }
    }
    Ok(decouverte::regrouper(&recues, crate::VERSION_PROTOCOLE, resoudre_vite))
}

/// L'adresse IPv4 de ce poste sur le reseau local (la carte que Windows
/// choisirait pour sortir) ; aucun paquet ne part.
fn adresse_locale_ipv4() -> Option<Ipv4Addr> {
    let s = UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("192.0.2.1:9").ok()?;
    match s.local_addr().ok()?.ip() {
        IpAddr::V4(ip) if !ip.is_unspecified() && !ip.is_loopback() => Some(ip),
        _ => None,
    }
}

/// Un nom de poste → ses adresses vues d'ici, une seconde et demie au
/// plus : un nom inconnu peut faire attendre la resolution Windows
/// (NetBIOS) bien plus longtemps. Pas de reponse a temps : on conseillera
/// l'adresse IP.
fn resoudre_vite(nom: &str) -> Vec<IpAddr> {
    let nom = nom.to_string();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let adresses: Vec<IpAddr> = (nom.as_str(), 0u16)
            .to_socket_addrs()
            .map(|it| it.map(|a| a.ip()).collect())
            .unwrap_or_default();
        let _ = tx.send(adresses);
    });
    rx.recv_timeout(Duration::from_millis(1500)).unwrap_or_default()
}

// =====================================================================
//  L'essai d'une adresse
// =====================================================================

/// « hote:port » → la premiere adresse resolue. Sans port explicite on
/// prend celui par defaut : le commercant tape « 192.168.1.10 », pas
/// « 192.168.1.10:40000 ».
pub fn premiere_adresse(hote: &str) -> Option<SocketAddr> {
    let avec_port = if hote.contains(':') { hote.to_string() } else { format!("{hote}:{PORT_DEFAUT}") };
    avec_port.to_socket_addrs().ok()?.next()
}

/// Interroge `/sante` du serveur.
///
/// Fait en Rust et non depuis la fenetre : le diagnostic doit distinguer
/// « serveur injoignable » de « le navigateur embarque a refuse la
/// reponse », deux pannes que `fetch` renvoie a l'identique. Une adresse
/// hors de la plage 40000-40100 est refusee avant d'essayer (D39, D55) :
/// meme si quelque chose y repond, la fenetre ne pourrait pas la joindre.
pub fn tester_serveur(adresse: &str) -> Result<serde_json::Value, String> {
    let adresse = adresse.trim().trim_start_matches("http://");
    let hote = adresse.split('/').next().filter(|s| !s.is_empty()).ok_or("Adresse vide.")?;
    adresse_joignable(hote)?;

    let cible = premiere_adresse(hote).ok_or_else(|| {
        format!("Adresse illisible ou nom inconnu sur ce réseau : « {hote} ». Vérifier l'orthographe, ou taper l'adresse IP.")
    })?;

    let mut flux = TcpStream::connect_timeout(&cible, Duration::from_secs(3))
        .map_err(|e| format!("Serveur injoignable sur {hote} : {e}"))?;
    flux.set_read_timeout(Some(Duration::from_secs(5))).ok();

    let requete = format!("GET /sante HTTP/1.1\r\nHost: {hote}\r\nConnection: close\r\n\r\n");
    flux.write_all(requete.as_bytes()).map_err(|e| e.to_string())?;

    let mut reponse = String::new();
    flux.read_to_string(&mut reponse)
        .map_err(|_| "Ce port répond, mais ce n'est pas un serveur Gescom.".to_string())?;

    let corps = reponse
        .split_once("\r\n\r\n")
        .map(|(_, c)| c)
        .ok_or("Ce port répond, mais ce n'est pas un serveur Gescom.")?;
    serde_json::from_str(corps).map_err(|_| "Ce port répond, mais ce n'est pas un serveur Gescom.".to_string())
}

// =====================================================================
//  L'adresse de Gescom Equipe (D55)
// =====================================================================

/// D'ou vient l'adresse qu'Equipe utilise — l'ecran de reglage le dit.
pub const SOURCE_EQUIPE: &str = "equipe";
pub const SOURCE_CAISSE: &str = "caisse";
pub const SOURCE_DEFAUT: &str = "defaut";

/// L'adresse d'Equipe et sa source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdresseEquipe {
    pub serveur: String,
    /// `SOURCE_EQUIPE`, `SOURCE_CAISSE` ou `SOURCE_DEFAUT`.
    pub source: &'static str,
}

/// L'adresse qu'Equipe utilise (D55, choix du proprietaire) :
///
/// 1. la sienne, reglee depuis son ecran de connexion — elle l'emporte :
///    c'est un geste fait dans Equipe ;
/// 2. sinon celle de la caisse de ce poste (`poste.json`) quand elle a un
///    role — en `monoposte`, le serveur local au port qu'il a pris ;
/// 3. sinon le serveur de cette machine, au port par defaut.
///
/// Une adresse hors de la plage (un fichier retouche a la main, un ancien
/// 7300) est ignoree : on passe a la suivante plutot que d'echouer a
/// chaque appel sur un « Failed to fetch ».
pub fn adresse_equipe(propre: Option<&str>, caisse_mode: &str, caisse_serveur: &str) -> AdresseEquipe {
    let utilisable = |a: &str| !a.trim().is_empty() && adresse_joignable(a.trim()).is_ok();
    if let Some(a) = propre.filter(|a| utilisable(a)) {
        return AdresseEquipe { serveur: a.trim().to_string(), source: SOURCE_EQUIPE };
    }
    if (caisse_mode == "poste" || caisse_mode == "monoposte") && utilisable(caisse_serveur) {
        return AdresseEquipe { serveur: caisse_serveur.trim().to_string(), source: SOURCE_CAISSE };
    }
    AdresseEquipe { serveur: format!("127.0.0.1:{PORT_DEFAUT}"), source: SOURCE_DEFAUT }
}

/// Le contenu d'`equipe.json` : l'empreinte d'Equipe sur ce poste, et
/// son adresse si on l'a reglee depuis Equipe.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReglageEquipe {
    pub empreinte: Option<String>,
    pub serveur: Option<String>,
}

fn lire_objet(chemin: &Path) -> serde_json::Map<String, serde_json::Value> {
    std::fs::read_to_string(chemin)
        .ok()
        // Le Bloc-notes peut y laisser un BOM : on le retire avant de lire.
        .and_then(|t| serde_json::from_str::<serde_json::Value>(t.trim_start_matches('\u{feff}')).ok())
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default()
}

/// Lit `equipe.json` ; absent ou illisible : rien.
pub fn lire_reglage_equipe(chemin: &Path) -> ReglageEquipe {
    let o = lire_objet(chemin);
    let texte = |cle: &str| {
        o.get(cle)
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    ReglageEquipe { empreinte: texte("empreinte"), serveur: texte("serveur") }
}

/// Ecrit une cle d'`equipe.json` en gardant les autres : l'empreinte
/// et l'adresse vivent dans le meme fichier, et reecrire l'une ne doit
/// jamais perdre l'autre (une empreinte perdue fait une machine de plus
/// dans la liste des postes). Fichier temporaire puis renommage : une
/// coupure de courant ne laisse pas un fichier a moitie ecrit.
fn ecrire_cle(chemin: &Path, cle: &str, valeur: Option<&str>) -> Result<(), String> {
    let mut o = lire_objet(chemin);
    match valeur {
        Some(v) => o.insert(cle.to_string(), serde_json::Value::String(v.to_string())),
        None => o.remove(cle),
    };
    if let Some(d) = chemin.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    let texte = serde_json::to_string_pretty(&serde_json::Value::Object(o)).map_err(|e| e.to_string())?;
    let temporaire = chemin.with_extension("json.tmp");
    std::fs::write(&temporaire, texte).map_err(|e| format!("Réglage d'Équipe non enregistré : {e}"))?;
    std::fs::rename(&temporaire, chemin).map_err(|e| format!("Réglage d'Équipe non enregistré : {e}"))
}

/// Retient l'empreinte d'Equipe, en gardant son adresse.
pub fn ecrire_empreinte_equipe(chemin: &Path, empreinte: &str) -> Result<(), String> {
    ecrire_cle(chemin, "empreinte", Some(empreinte))
}

/// Enregistre l'adresse d'Equipe (D55). `None` l'efface : Equipe
/// reprend celle de la caisse du poste. Refuse une adresse vide ou hors
/// de la plage, sans toucher au fichier. L'ecran l'a ESSAYEE avant
/// (`tester_serveur`) ; on ne la reessaie pas ici : enregistrer ne doit
/// pas dependre d'un aller-retour reseau de plus.
pub fn enregistrer_adresse_equipe(chemin: &Path, adresse: Option<&str>) -> Result<(), String> {
    let adresse = match adresse.map(str::trim) {
        None => None,
        Some("") => return Err("Taper l'adresse du serveur, ou la rechercher.".to_string()),
        Some(a) => {
            let a = a.trim_start_matches("http://").trim_end_matches('/');
            adresse_joignable(a)?;
            Some(a)
        }
    };
    ecrire_cle(chemin, "serveur", adresse)
}
