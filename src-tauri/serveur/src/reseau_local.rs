//! Ce que le serveur doit dire de lui-même au démarrage.
//!
//! ## Pourquoi ce module existe
//!
//! Deux questions tombent au moment d'installer un poste caisse, et
//! aucune n'a de réponse évidente pour un commerçant :
//!
//! - **quelle adresse taper ?** « 0.0.0.0:40000 » n'est pas une adresse
//!   qu'on saisit ailleurs, c'est ce sur quoi le serveur écoute ;
//! - **pourquoi la caisse ne joint-elle rien ?** Le pare-feu de Windows
//!   bloque en silence : pas de message, pas de journal, juste un
//!   « Serveur injoignable » du côté de la caisse.
//!
//! Le serveur répond donc aux deux tout seul, à chaque démarrage.

/// L'adresse de ce poste sur le réseau local.
///
/// On ouvre une socket UDP vers une adresse extérieure sans **rien
/// envoyer** : cela suffit à faire choisir à Windows l'interface qu'il
/// utiliserait, donc l'adresse que les autres postes voient. Énumérer
/// les cartes réseau donnerait aussi les adaptateurs virtuels de
/// VMware, Hyper-V et Bluetooth — et le commerçant recopierait la
/// mauvaise.
///
/// `None` s'il n'y a pas de route : une machine sans réseau ne sert
/// pas de serveur, et le dire vaut mieux que d'inventer une adresse.
pub fn adresse_locale() -> Option<String> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    // Adresse de documentation (RFC 5737) : elle n'est jamais routée,
    // et aucun paquet ne part. On ne veut que la décision de routage.
    socket.connect("192.0.2.1:9").ok()?;
    let addr = socket.local_addr().ok()?;
    let ip = addr.ip().to_string();
    if ip == "0.0.0.0" {
        return None;
    }
    Some(ip)
}

/// Le nom du poste, celui que Windows annonce sur le réseau local. C'est
/// lui qu'on conseille de taper sur les caisses : il ne change pas quand
/// la box redistribue les adresses.
pub fn nom_poste() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .map(|n| n.trim().to_string())
        .unwrap_or_default()
}

/// Une adresse IPv4 de ce poste, sur une carte en service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Carte {
    pub ip: std::net::Ipv4Addr,
    /// Le nom de la carte dans Windows (« Wi-Fi », « Ethernet »).
    pub carte: String,
    /// L'adresse est donnée par la box (DHCP) : elle peut changer.
    pub dhcp: bool,
    /// Celle que Windows choisit pour sortir (`adresse_locale`) : celle
    /// que les caisses voient d'habitude.
    pub principale: bool,
}

/// Les adresses IPv4 de ce poste, la principale d'abord. On les liste
/// toutes (les cartes virtuelles de VMware ou Hyper-V comprises), mais on
/// marque celle que Windows utilise : c'est elle qu'on conseille.
pub fn cartes() -> Vec<Carte> {
    let principale: Option<std::net::Ipv4Addr> = adresse_locale().and_then(|a| a.parse().ok());
    let mut cartes = cartes_du_systeme();
    if let Some(p) = principale {
        if !cartes.iter().any(|c| c.ip == p) {
            cartes.push(Carte { ip: p, carte: String::new(), dhcp: false, principale: true });
        }
    }
    for c in cartes.iter_mut() {
        c.principale = Some(c.ip) == principale;
    }
    cartes.sort_by_key(|c| !c.principale);
    cartes
}

/// Les cartes en service et leurs adresses IPv4, par `GetAdaptersAddresses`
/// (iphlpapi) : une liste exacte, et pour chaque adresse son origine —
/// DHCP ou réglée à la main —, sans lire la sortie traduite d'`ipconfig`.
#[cfg(windows)]
fn cartes_du_systeme() -> Vec<Carte> {
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        GetAdaptersAddresses, GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_DNS_SERVER, GAA_FLAG_SKIP_MULTICAST,
        IF_TYPE_SOFTWARE_LOOPBACK, IP_ADAPTER_ADDRESSES_LH,
    };
    use windows_sys::Win32::NetworkManagement::Ndis::IfOperStatusUp;
    use windows_sys::Win32::Networking::WinSock::{IpPrefixOriginDhcp, AF_INET, SOCKADDR_IN};

    const ERROR_BUFFER_OVERFLOW: u32 = 111;
    let drapeaux = GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST | GAA_FLAG_SKIP_DNS_SERVER;
    // Des u64 : la liste rendue doit etre alignee sur 8 octets.
    let mut taille: u32 = 16 * 1024;
    let mut tampon: Vec<u64> = Vec::new();
    let mut lu = false;
    for _ in 0..3 {
        tampon = vec![0u64; (taille as usize).div_ceil(8)];
        // SAFETY : le tampon fait `taille` octets au moins ; Windows y
        // ecrit une liste chainee qui ne pointe que dans le tampon.
        let r = unsafe {
            GetAdaptersAddresses(
                AF_INET as u32,
                drapeaux,
                std::ptr::null(),
                tampon.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH,
                &mut taille,
            )
        };
        if r == 0 {
            lu = true;
            break;
        }
        if r != ERROR_BUFFER_OVERFLOW {
            return Vec::new();
        }
    }
    if !lu {
        return Vec::new();
    }

    let mut cartes = Vec::new();
    let mut p = tampon.as_ptr() as *const IP_ADAPTER_ADDRESSES_LH;
    while !p.is_null() {
        // SAFETY : chaque maillon vit dans `tampon`, qui vit jusqu'ici.
        let a = unsafe { &*p };
        if a.OperStatus == IfOperStatusUp && a.IfType != IF_TYPE_SOFTWARE_LOOPBACK {
            let nom = unsafe { texte_large(a.FriendlyName) };
            let mut u = a.FirstUnicastAddress;
            while !u.is_null() {
                let ua = unsafe { &*u };
                let sa = ua.Address.lpSockaddr;
                if !sa.is_null() && unsafe { (*sa).sa_family } == AF_INET {
                    let sin = unsafe { &*(sa as *const SOCKADDR_IN) };
                    // S_addr est en ordre reseau tel quel en memoire.
                    let ip = std::net::Ipv4Addr::from(unsafe { sin.sin_addr.S_un.S_addr }.to_ne_bytes());
                    if !ip.is_loopback() && !ip.is_unspecified() {
                        cartes.push(Carte {
                            ip,
                            carte: nom.clone(),
                            dhcp: ua.PrefixOrigin == IpPrefixOriginDhcp,
                            principale: false,
                        });
                    }
                }
                u = ua.Next;
            }
        }
        p = a.Next;
    }
    cartes
}

/// Une chaine large de Windows, terminee par un zero.
#[cfg(windows)]
unsafe fn texte_large(p: *const u16) -> String {
    if p.is_null() {
        return String::new();
    }
    let mut n = 0usize;
    while *p.add(n) != 0 && n < 512 {
        n += 1;
    }
    String::from_utf16_lossy(std::slice::from_raw_parts(p, n))
}

#[cfg(not(windows))]
fn cartes_du_systeme() -> Vec<Carte> {
    Vec::new()
}

/// Les noms des règles de pare-feu posées par `outils/parefeu.ps1` et
/// l'installeur du serveur : TCP sur la plage (D39), UDP 40000 pour la
/// recherche du serveur par les caisses.
pub const REGLE_TCP: &str = "Gescom serveur";
pub const REGLE_UDP: &str = "Gescom serveur (recherche)";

/// La sortie de `netsh … show rule` couvre-t-elle ce port ? Un port écrit
/// tel quel (« 40000 ») ou une plage qui le contient (« 40000-40100 ») —
/// sans dépendre de la langue de Windows, qui traduit les intitulés mais
/// pas les nombres.
pub fn regle_couvre(texte: &str, port: u16) -> bool {
    texte
        .split(|c: char| !(c.is_ascii_digit() || c == '-'))
        .filter(|t| !t.is_empty())
        .any(|t| match t.split_once('-') {
            Some((a, b)) => match (a.parse::<u32>(), b.parse::<u32>()) {
                (Ok(a), Ok(b)) => (a..=b).contains(&(port as u32)),
                _ => false,
            },
            None => t.parse::<u32>().ok() == Some(port as u32),
        })
}

/// La règle `nom` existe-t-elle, et couvre-t-elle `port` ?
///
/// Vérifié parce que la règle est le point de panne le plus probable
/// d'une installation multiposte, et le plus muet : Windows refuse la
/// connexion sans rien écrire nulle part.
///
/// `None` quand on ne peut pas savoir — on n'affiche alors rien plutôt
/// que d'inquiéter pour rien.
#[cfg(windows)]
pub fn regle_presente(nom: &str, port: u16) -> Option<bool> {
    use std::process::Command;

    let sortie = Command::new("netsh")
        .args(["advfirewall", "firewall", "show", "rule", &format!("name={nom}")])
        .output()
        .ok()?;
    // `netsh` rend 1 et « Aucune règle » quand elle n'existe pas.
    if !sortie.status.success() {
        return Some(false);
    }
    // La règle peut exister sur un autre port (posée à la main) : on
    // vérifie qu'elle couvre bien celui-là.
    Some(regle_couvre(&String::from_utf8_lossy(&sortie.stdout), port))
}

#[cfg(not(windows))]
pub fn regle_presente(_nom: &str, _port: u16) -> Option<bool> {
    None
}

/// Le pare-feu laisse-t-il entrer les caisses sur ce port TCP ?
pub fn regle_parefeu_presente(port: u16) -> Option<bool> {
    regle_presente(REGLE_TCP, port)
}

/// Le pare-feu laisse-t-il passer la recherche du serveur (UDP) ?
pub fn regle_recherche_presente(port_udp: u16) -> Option<bool> {
    regle_presente(REGLE_UDP, port_udp)
}

/// Ce qu'on affiche au démarrage, sous l'adresse d'écoute.
pub fn conseils(port: u16) -> Vec<String> {
    let mut lignes = Vec::new();

    match adresse_locale() {
        Some(ip) => lignes.push(format!(
            "  pour les caisses : {ip}:{port}   ← l'adresse à saisir dans \
             Paramètres → Réseau"
        )),
        None => lignes.push(
            "  ATTENTION : aucune adresse réseau, ce poste n'est joignable que par \
             lui-même."
                .to_string(),
        ),
    }

    if regle_parefeu_presente(port) == Some(false) {
        lignes.push(String::new());
        lignes.push(
            "  ATTENTION, PARE-FEU : aucune règle « Gescom serveur » n'autorise le port"
                .to_string(),
        );
        lignes.push(format!(
            "    {port}. Les autres postes recevront « Serveur injoignable »"
        ));
        lignes.push("    sans autre explication.".to_string());
        lignes.push(String::new());
        lignes.push(
            "    Ouvrir PowerShell EN ADMINISTRATEUR sur ce poste, puis :"
                .to_string(),
        );
        lignes.push("      .\\outils\\parefeu.ps1 -Ouvrir".to_string());
    } else if regle_recherche_presente(gescom_noyau::decouverte::PORT) == Some(false) {
        // La regle TCP est la : les caisses joignent le serveur, mais
        // « Rechercher le serveur » ne le trouvera pas.
        lignes.push(format!(
            "  pare-feu : la recherche automatique (UDP {}) est fermée — les caisses devront \
             saisir l'adresse ; .\\outils\\parefeu.ps1 -Ouvrir l'ouvre.",
            gescom_noyau::decouverte::PORT
        ));
    }

    lignes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn une_regle_couvre_son_port_ou_sa_plage_quelle_que_soit_la_langue() {
        let fr = "Nom de la règle :                    Gescom serveur\r\nPortLocal :                          40000-40100\r\nProfils :                            Domaine,Privé\r\n";
        let en = "Rule Name:                            Gescom serveur\r\nLocalPort:                            40000\r\n";
        assert!(regle_couvre(fr, 40000) && regle_couvre(fr, 40057) && regle_couvre(fr, 40100));
        assert!(!regle_couvre(fr, 40101) && !regle_couvre(fr, 7300));
        assert!(regle_couvre(en, 40000) && !regle_couvre(en, 40001));
        assert!(!regle_couvre("", 40000));
    }
}
