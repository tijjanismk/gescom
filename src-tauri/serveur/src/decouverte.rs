//! Le serveur repond aux caisses qui le cherchent (27/10/2026).
//!
//! Une caisse pose sa question en diffusion UDP sur le port 40000
//! (`gescom_noyau::decouverte`, le format) ; ce fil y repond par le nom
//! du poste, ses adresses IPv4, le port TCP et les versions — rien de la
//! boutique. Il ecoute toujours 40000, meme quand le serveur sert les
//! caisses sur un autre port de la plage : sinon il faudrait deja
//! connaitre le port pour le trouver.
//!
//! Ce n'est qu'une commodite : si 40000 est deja pris en UDP (un autre
//! serveur Gescom sur la meme machine), le journal technique le dit et le
//! serveur continue — les caisses saisiront l'adresse a la main.
//!
//! Garde-fous : on ne repond qu'aux questions bien formees, venues du
//! reseau local (`source_du_reseau_local`), et pas plus de
//! `REPONSES_PAR_SECONDE` par seconde — une reponse plus grosse que la
//! question ne doit pas faire du serveur un amplificateur.

use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use gescom_noyau::decouverte as format;

use crate::journal_technique as journal;
use crate::reseau_local;

/// En debogage seulement : remplace le port UDP (les tests ne prennent
/// pas 40000 au serveur de la boutique) ; « 0 » coupe la decouverte.
pub const VARIABLE_PORT: &str = "GESCOM_DECOUVERTE_PORT";

/// Au-dela, on se tait jusqu'a la seconde suivante.
const REPONSES_PAR_SECONDE: u32 = 20;

/// Le port UDP a ecouter, ou `None` (decouverte coupee).
pub fn port_udp() -> Option<u16> {
    #[cfg(debug_assertions)]
    if let Ok(v) = std::env::var(VARIABLE_PORT) {
        if let Ok(p) = v.trim().parse::<u16>() {
            return (p != 0).then_some(p);
        }
    }
    Some(format::PORT)
}

/// Le fil qui ecoute, a arreter avant un redemarrage : le serveur
/// relance doit retrouver le port UDP libre.
pub struct Ecoute {
    arret: Arc<AtomicBool>,
    fil: Option<JoinHandle<()>>,
}

impl Ecoute {
    pub fn arreter(mut self) {
        self.arret.store(true, Ordering::SeqCst);
        if let Some(f) = self.fil.take() {
            let _ = f.join();
        }
    }
}

/// Se met a l'ecoute et repond dans un fil a part. Rend l'ecoute et son
/// etat pour le panneau Reseau : `Ok(port)`, ou pourquoi elle ne tourne
/// pas. `hote` : l'ecoute TCP du serveur — sur « ce poste seulement »
/// (127.0.0.1), la decouverte ne repond qu'a ce poste, elle n'annonce pas
/// aux caisses un serveur qu'elles ne joindraient pas. `relance` : le
/// serveur vient de se relancer, l'ancien processus libere peut-etre
/// encore le port — on reessaie quelques secondes.
pub fn lancer(port_tcp: u16, hote: &str, relance: bool) -> (Option<Ecoute>, Result<u16, String>) {
    let Some(port) = port_udp() else {
        return (None, Err("recherche automatique coupée (essai)".to_string()));
    };
    let local_seulement = hote.trim().parse::<std::net::IpAddr>().is_ok_and(|ip| ip.is_loopback());
    let liaison = if local_seulement { format!("127.0.0.1:{port}") } else { format!("0.0.0.0:{port}") };
    let debut = Instant::now();
    let socket = loop {
        match UdpSocket::bind(&liaison) {
            Ok(s) => break s,
            Err(e) if relance && debut.elapsed() < Duration::from_secs(5) => {
                let _ = e;
                std::thread::sleep(Duration::from_millis(250));
            }
            Err(e) => {
                let raison = format!(
                    "UDP {port} déjà pris ({e}) — un autre serveur Gescom sur ce poste ? « Rechercher le serveur » \
                     ne trouvera pas celui-ci ; les caisses saisiront son adresse."
                );
                journal::avertissement(format!("Recherche automatique du serveur indisponible : {raison}"));
                return (None, Err(raison));
            }
        }
    };
    // Une demi-seconde : assez pour regarder le drapeau d'arret souvent.
    socket.set_read_timeout(Some(Duration::from_millis(500))).ok();
    let arret = Arc::new(AtomicBool::new(false));
    let drapeau = Arc::clone(&arret);
    let hote_fixe: Option<Ipv4Addr> = hote.trim().parse::<Ipv4Addr>().ok().filter(|ip| !ip.is_unspecified());
    let fil = std::thread::Builder::new()
        .name("decouverte".to_string())
        .spawn(move || ecouter(socket, port_tcp, hote_fixe, drapeau))
        .ok();
    if fil.is_none() {
        return (None, Err("fil de la recherche automatique impossible à lancer".to_string()));
    }
    journal::info(format!(
        "Recherche automatique : réponse aux caisses sur UDP {port}{}",
        if local_seulement { " (ce poste seulement)" } else { "" }
    ));
    (Some(Ecoute { arret, fil }), Ok(port))
}

fn ecouter(socket: UdpSocket, port_tcp: u16, hote_fixe: Option<Ipv4Addr>, arret: Arc<AtomicBool>) {
    let mut tampon = [0u8; format::TAILLE_QUESTION_MAX];
    let mut seconde = Instant::now();
    let mut repondues = 0u32;
    while !arret.load(Ordering::SeqCst) {
        let (n, de): (usize, SocketAddr) = match socket.recv_from(&mut tampon) {
            Ok(r) => r,
            // Delai ecoule : on regarde le drapeau. Sous Windows, une
            // reponse envoyee a une caisse deja partie revient en
            // « connexion reinitialisee » sur la lecture suivante, et un
            // paquet trop gros en erreur : rien de grave, on continue.
            Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => continue,
            Err(_) => {
                std::thread::sleep(Duration::from_millis(50));
                continue;
            }
        };
        if format::lire_question(&tampon[..n]).is_none() || !format::source_du_reseau_local(de.ip()) {
            continue;
        }
        if seconde.elapsed() >= Duration::from_secs(1) {
            seconde = Instant::now();
            repondues = 0;
        }
        if repondues >= REPONSES_PAR_SECONDE {
            continue;
        }
        repondues += 1;
        // Les adresses se relisent a chaque question : la box a pu en
        // donner une autre depuis le demarrage. Une ecoute sur une seule
        // adresse (ce poste seulement, ou reglee a la main) n'annonce
        // qu'elle : c'est la seule qui mene au serveur.
        let adresses: Vec<Ipv4Addr> = match hote_fixe {
            Some(ip) => vec![ip],
            None => reseau_local::cartes().into_iter().map(|c| c.ip).collect(),
        };
        let annonce = format::Annonce::nouvelle(
            &reseau_local::nom_poste(),
            &adresses,
            port_tcp,
            gescom_noyau::VERSION_PROTOCOLE,
            env!("CARGO_PKG_VERSION"),
        );
        let _ = socket.send_to(&format::reponse(&annonce), de);
    }
}
