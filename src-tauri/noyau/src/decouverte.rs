//! La decouverte du serveur sur le reseau local (27/10/2026).
//!
//! Une caisse neuve ne sait pas ou est le serveur. Plutot que de faire
//! recopier une adresse IP lue sur un autre ecran, elle pose une question
//! en diffusion UDP sur le port 40000 (`PORT`), et chaque serveur Gescom
//! qui l'entend repond : le nom de son poste, ses adresses IPv4, son port
//! TCP, ses versions. L'ecran propose, le commercant clique, et l'adresse
//! est ensuite TESTEE comme si elle avait ete tapee (`tester_serveur`).
//!
//! Ici le FORMAT seulement, pur, compile des deux cotes : la question, la
//! reponse, leur lecture, et ce que la caisse en retient. Le serveur
//! ecoute (`serveur/src/decouverte.rs`), la fenetre interroge
//! (`src-tauri/src/reseau.rs::rechercher_serveurs`).
//!
//! **Aucune donnee de la boutique** dans la reponse : le nom de la
//! machine, ses adresses, son port et ses versions — ce que le voisinage
//! reseau de Windows montre deja a qui est branche sur la meme box.
//!
//! Le format est versionne (`v`). Une reponse d'une version inconnue se
//! lit comme un silence, jamais comme une reponse a moitie comprise ; le
//! serveur, lui, repond dans SA version a toute question Gescom, pour
//! qu'une caisse plus recente puisse au moins dire qu'il est plus ancien.

use std::net::{IpAddr, Ipv4Addr};

use serde::{Deserialize, Serialize};

use crate::protocole::{port_dans_la_plage, PORT_DEFAUT};

/// La version du format de la question et de la reponse.
pub const VERSION: u32 = 1;

/// Le port UDP de la decouverte : toujours le premier de la plage, meme
/// quand le serveur ecoute en TCP sur un autre (D39) — sinon la caisse
/// devrait deja connaitre le port pour le trouver.
pub const PORT: u16 = PORT_DEFAUT;

/// Ce qui marque une question et une reponse Gescom.
pub const MARQUE: &str = "gescom";

/// Ce que le serveur dit etre.
pub const PRODUIT: &str = "Gescom Serveur";

/// Une question tient en une vingtaine d'octets ; au-dela, ce n'est pas
/// la notre — on ne lit meme pas.
pub const TAILLE_QUESTION_MAX: usize = 256;

/// Une reponse : quelques centaines d'octets. La borne protege la caisse
/// d'un paquet monstrueux envoye par n'importe qui sur le reseau.
pub const TAILLE_REPONSE_MAX: usize = 4096;

/// Une machine a rarement plus de quelques cartes ; au-dela on coupe.
pub const ADRESSES_MAX: usize = 16;

#[derive(Serialize, Deserialize)]
struct Question {
    q: String,
    v: u32,
}

/// La question, telle que la caisse l'envoie : `{"q":"gescom","v":1}`.
pub fn question() -> Vec<u8> {
    serde_json::to_vec(&Question { q: MARQUE.to_string(), v: VERSION }).unwrap_or_default()
}

/// La version demandee si ces octets sont une question Gescom ; `None`
/// sinon — et le serveur se tait : un paquet etranger ne recoit rien.
pub fn lire_question(octets: &[u8]) -> Option<u32> {
    if octets.is_empty() || octets.len() > TAILLE_QUESTION_MAX {
        return None;
    }
    let q: Question = serde_json::from_slice(octets).ok()?;
    (q.q == MARQUE && q.v >= 1).then_some(q.v)
}

/// La reponse d'un serveur.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Annonce {
    /// `MARQUE` : l'echo d'un autre logiciel n'est pas un serveur Gescom.
    pub r: String,
    /// `VERSION`.
    pub v: u32,
    /// « Gescom Serveur ».
    pub produit: String,
    /// Le nom du poste (celui de Windows).
    pub poste: String,
    /// Ses adresses IPv4 sur le reseau local.
    pub adresses: Vec<String>,
    /// Le port TCP ou il sert les caisses.
    pub port: u16,
    pub version_protocole: u32,
    pub version_serveur: String,
}

impl Annonce {
    pub fn nouvelle(
        poste: &str,
        adresses: &[Ipv4Addr],
        port: u16,
        version_protocole: u32,
        version_serveur: &str,
    ) -> Annonce {
        Annonce {
            r: MARQUE.to_string(),
            v: VERSION,
            produit: PRODUIT.to_string(),
            poste: poste.trim().to_string(),
            adresses: adresses.iter().take(ADRESSES_MAX).map(|a| a.to_string()).collect(),
            port,
            version_protocole,
            version_serveur: version_serveur.to_string(),
        }
    }
}

/// La reponse, en octets.
pub fn reponse(annonce: &Annonce) -> Vec<u8> {
    serde_json::to_vec(annonce).unwrap_or_default()
}

/// Ces octets sont-ils la reponse d'un serveur Gescom, dans une version
/// que ce poste sait lire ? Les adresses illisibles sont ecartees.
pub fn lire_reponse(octets: &[u8]) -> Option<Annonce> {
    if octets.is_empty() || octets.len() > TAILLE_REPONSE_MAX {
        return None;
    }
    let mut a: Annonce = serde_json::from_slice(octets).ok()?;
    if a.r != MARQUE || a.v != VERSION || a.port == 0 {
        return None;
    }
    a.poste = a.poste.trim().to_string();
    a.adresses = a
        .adresses
        .iter()
        .filter_map(|s| s.trim().parse::<Ipv4Addr>().ok())
        .take(ADRESSES_MAX)
        .map(|ip| ip.to_string())
        .collect();
    Some(a)
}

/// Le serveur ne repond qu'au reseau local : boucle, adresses privees
/// (10/8, 172.16/12, 192.168/16), lien local (169.254/16), et leurs
/// pendants IPv6. Une question venue d'ailleurs (adresse usurpee pour
/// faire du serveur un amplificateur) ne recoit rien.
pub fn source_du_reseau_local(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_loopback() || v4.is_private() || v4.is_link_local(),
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return source_du_reseau_local(IpAddr::V4(v4));
            }
            let premier = v6.segments()[0];
            v6.is_loopback()
                // fe80::/10 : lien local.
                || (premier & 0xffc0) == 0xfe80
                // fc00::/7 : adresses locales uniques.
                || (premier & 0xfe00) == 0xfc00
        }
    }
}

/// « HOTE » si le port est celui par defaut (la caisse le suppose), sinon
/// « HOTE:PORT ».
pub fn avec_port(hote: &str, port: u16) -> String {
    if port == PORT_DEFAUT {
        hote.to_string()
    } else {
        format!("{hote}:{port}")
    }
}

/// Un nom de poste qu'on peut taper comme adresse : lettres, chiffres,
/// tirets (un nom Windows), 63 caracteres au plus. Un nom avec espace ou
/// accent ne se resout pas sur le reseau : on propose alors l'IP.
pub fn nom_utilisable(nom: &str) -> bool {
    let n = nom.trim();
    !n.is_empty()
        && n.len() <= 63
        && !n.starts_with('-')
        && n.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.' || c == '_')
        && n.parse::<IpAddr>().is_err()
}

/// Un serveur trouve, tel que l'ecran de la caisse le propose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServeurTrouve {
    pub poste: String,
    pub port: u16,
    /// Ses adresses annoncees, plus celles d'ou la reponse est venue.
    pub adresses: Vec<String>,
    /// Les adresses d'ou une reponse est arrivee : elles, au moins, ont
    /// traverse le reseau jusqu'a ce poste.
    pub vu_de: Vec<String>,
    pub version_protocole: u32,
    pub version_serveur: String,
    /// « NOM » ou « NOM:40005 » — vide si le nom n'est pas utilisable.
    pub adresse_nom: String,
    /// « 192.168.1.20 » ou « 192.168.1.20:40005 », une adresse qui a repondu.
    pub adresse_ip: String,
    /// Ce qu'un clic remplit : le nom s'il mene a CE serveur depuis ce
    /// poste (il ne change pas quand la box redistribue les adresses),
    /// sinon l'IP.
    pub adresse_conseillee: String,
    pub nom_resolu: bool,
    /// Le port est dans la plage que la fenetre joint (D39).
    pub joignable: bool,
    /// Meme protocole que ce poste : sinon la connexion sera refusee.
    pub meme_protocole: bool,
}

/// Les reponses recues — (annonce, adresse d'ou elle vient) — une fois
/// par serveur (meme nom de poste, meme port), triees par nom.
///
/// `resoudre` : un nom de machine → ses adresses vues d'ici (vide si on
/// ne sait pas). Separe pour que la regle se teste sans reseau. Le nom
/// n'est conseille que s'il mene a l'une des adresses de CE serveur : un
/// homonyme ailleurs sur le reseau ne doit pas detourner la caisse.
pub fn regrouper(
    recues: &[(Annonce, IpAddr)],
    protocole_local: u32,
    resoudre: impl Fn(&str) -> Vec<IpAddr>,
) -> Vec<ServeurTrouve> {
    let mut groupes: Vec<(Annonce, Vec<IpAddr>)> = Vec::new();
    for (a, de) in recues {
        let cle = |x: &Annonce| (x.poste.to_ascii_uppercase(), x.port);
        match groupes.iter_mut().find(|(g, _)| cle(g) == cle(a)) {
            Some((g, sources)) => {
                for ad in &a.adresses {
                    if !g.adresses.contains(ad) {
                        g.adresses.push(ad.clone());
                    }
                }
                if !sources.contains(de) {
                    sources.push(*de);
                }
            }
            None => groupes.push((a.clone(), vec![*de])),
        }
    }

    let mut trouves: Vec<ServeurTrouve> = groupes
        .into_iter()
        .map(|(a, sources)| {
            // Une IP qui a repondu, du reseau plutot que la boucle : c'est
            // celle que les AUTRES caisses verront aussi.
            let ip = sources
                .iter()
                .find(|s| !s.is_loopback())
                .or_else(|| sources.first())
                .map(|s| match s {
                    IpAddr::V6(v6) => v6.to_ipv4_mapped().map(IpAddr::V4).unwrap_or(*s),
                    _ => *s,
                })
                .map(|s| s.to_string())
                .unwrap_or_default();
            let mut adresses = a.adresses.clone();
            for s in &sources {
                let s = s.to_string();
                if !adresses.contains(&s) {
                    adresses.push(s);
                }
            }
            let vu_de: Vec<String> = sources.iter().map(|s| s.to_string()).collect();
            let adresse_nom = if nom_utilisable(&a.poste) { avec_port(&a.poste, a.port) } else { String::new() };
            let nom_resolu = !adresse_nom.is_empty() && {
                let ici = sources.iter().any(|s| s.is_loopback());
                resoudre(&a.poste).iter().any(|r| {
                    adresses.contains(&r.to_string()) || (r.is_loopback() && ici)
                })
            };
            let adresse_ip = if ip.is_empty() { String::new() } else { avec_port(&ip, a.port) };
            let adresse_conseillee = if nom_resolu { adresse_nom.clone() } else { adresse_ip.clone() };
            ServeurTrouve {
                poste: a.poste.clone(),
                port: a.port,
                adresses,
                vu_de,
                version_protocole: a.version_protocole,
                version_serveur: a.version_serveur.clone(),
                adresse_nom,
                adresse_ip,
                adresse_conseillee,
                nom_resolu,
                joignable: port_dans_la_plage(a.port),
                meme_protocole: a.version_protocole == protocole_local,
            }
        })
        .collect();
    trouves.sort_by(|x, y| x.poste.to_ascii_uppercase().cmp(&y.poste.to_ascii_uppercase()).then(x.port.cmp(&y.port)));
    trouves
}

#[cfg(test)]
mod tests {
    use super::*;

    fn annonce(poste: &str, ips: &[&str], port: u16) -> Annonce {
        let ips: Vec<Ipv4Addr> = ips.iter().map(|i| i.parse().unwrap()).collect();
        Annonce::nouvelle(poste, &ips, port, 1, "0.2.0")
    }

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn la_question_se_lit_et_rien_d_autre() {
        assert_eq!(question(), br#"{"q":"gescom","v":1}"#.to_vec());
        assert_eq!(lire_question(&question()), Some(1));
        // Une caisse plus recente : le serveur repond quand meme.
        assert_eq!(lire_question(br#"{"q":"gescom","v":2}"#), Some(2));
        for etranger in [
            &b""[..],
            b"bonjour",
            br#"{"q":"autre","v":1}"#,
            br#"{"q":"gescom","v":0}"#,
            br#"{"q":"gescom"}"#,
            br#"[1,2,3]"#,
        ] {
            assert_eq!(lire_question(etranger), None, "{}", String::from_utf8_lossy(etranger));
        }
        let long = format!(r#"{{"q":"gescom","v":1,"bourrage":"{}"}}"#, "x".repeat(400));
        assert_eq!(lire_question(long.as_bytes()), None, "une question trop longue n'est pas lue");
    }

    #[test]
    fn la_reponse_fait_l_aller_retour_sans_donnee_de_boutique() {
        let a = annonce("BOUTIQUE-PC", &["192.168.1.20"], 40000);
        let octets = reponse(&a);
        assert_eq!(lire_reponse(&octets), Some(a));
        // Exactement ces champs : rien de la base, rien des ventes.
        let v: serde_json::Value = serde_json::from_slice(&octets).unwrap();
        let mut cles: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
        cles.sort();
        assert_eq!(
            cles,
            ["adresses", "port", "poste", "produit", "r", "v", "version_protocole", "version_serveur"]
        );
        assert_eq!(v["produit"], "Gescom Serveur");
    }

    #[test]
    fn une_reponse_etrangere_ou_d_une_autre_version_est_un_silence() {
        let mut a = annonce("PC", &["192.168.1.20"], 40000);
        a.v = 2;
        assert_eq!(lire_reponse(&reponse(&a)), None);
        let mut b = annonce("PC", &["192.168.1.20"], 40000);
        b.r = "autre".into();
        assert_eq!(lire_reponse(&reponse(&b)), None);
        let mut c = annonce("PC", &["192.168.1.20"], 40000);
        c.port = 0;
        assert_eq!(lire_reponse(&reponse(&c)), None);
        assert_eq!(lire_reponse(b"pas du json"), None);
        assert_eq!(lire_reponse(&vec![b' '; TAILLE_REPONSE_MAX + 1]), None);
        // Une adresse illisible est ecartee, pas la reponse.
        let brut = br#"{"r":"gescom","v":1,"produit":"Gescom Serveur","poste":" PC ","adresses":["192.168.1.20","n'importe quoi","fe80::1"],"port":40003,"version_protocole":1,"version_serveur":"0.2.0"}"#;
        let lu = lire_reponse(brut).unwrap();
        assert_eq!(lu.adresses, vec!["192.168.1.20".to_string()]);
        assert_eq!(lu.poste, "PC");
    }

    #[test]
    fn le_serveur_ne_repond_qu_au_reseau_local() {
        for oui in ["127.0.0.1", "10.1.2.3", "172.16.0.9", "172.31.255.1", "192.168.1.30", "169.254.10.1", "::1", "fe80::1", "fd00::5", "::ffff:192.168.1.4"] {
            assert!(source_du_reseau_local(ip(oui)), "{oui}");
        }
        for non in ["8.8.8.8", "41.73.120.5", "172.32.0.1", "2001:db8::1", "::ffff:8.8.8.8"] {
            assert!(!source_du_reseau_local(ip(non)), "{non}");
        }
    }

    #[test]
    fn le_port_par_defaut_ne_s_ecrit_pas() {
        assert_eq!(avec_port("BOUTIQUE-PC", 40000), "BOUTIQUE-PC");
        assert_eq!(avec_port("192.168.1.20", 40005), "192.168.1.20:40005");
        assert!(nom_utilisable("BOUTIQUE-PC") && nom_utilisable("caisse_1"));
        for non in ["", " ", "Poste de Awa", "Boutique-Été", "192.168.1.20", "-PC"] {
            assert!(!nom_utilisable(non), "{non}");
        }
    }

    #[test]
    fn un_serveur_entendu_deux_fois_ne_s_affiche_qu_une() {
        // Le meme serveur repond par la boucle et par le reseau (la
        // question part a 255.255.255.255 ET a 127.0.0.1).
        let a = annonce("BOUTIQUE-PC", &["192.168.1.20"], 40000);
        let b = annonce("boutique-pc", &["192.168.1.20", "10.0.0.5"], 40000);
        let autre = annonce("BUREAU", &["192.168.1.30"], 40002);
        let recues = vec![
            (a, ip("127.0.0.1")),
            (b, ip("192.168.1.20")),
            (autre, ip("192.168.1.30")),
        ];
        let resoudre = |n: &str| if n.eq_ignore_ascii_case("BOUTIQUE-PC") { vec![ip("192.168.1.20")] } else { vec![] };
        let t = regrouper(&recues, 1, resoudre);
        assert_eq!(t.len(), 2);
        assert_eq!(t[0].poste, "BOUTIQUE-PC");
        assert_eq!(t[0].adresses, vec!["192.168.1.20", "10.0.0.5", "127.0.0.1"]);
        // L'IP proposee est celle du reseau, pas la boucle.
        assert_eq!(t[0].adresse_ip, "192.168.1.20");
        // Le nom mene a ce serveur : c'est lui qu'on conseille.
        assert!(t[0].nom_resolu);
        assert_eq!(t[0].adresse_conseillee, "BOUTIQUE-PC");
        // Le nom de BUREAU ne se resout pas d'ici : on conseille l'IP, avec son port.
        assert_eq!(t[1].poste, "BUREAU");
        assert!(!t[1].nom_resolu);
        assert_eq!(t[1].adresse_nom, "BUREAU:40002");
        assert_eq!(t[1].adresse_conseillee, "192.168.1.30:40002");
        assert!(t[1].joignable && t[1].meme_protocole);
    }

    #[test]
    fn un_homonyme_ailleurs_ne_detourne_pas_la_caisse() {
        let recues = vec![(annonce("PC", &["192.168.1.20"], 40000), ip("192.168.1.20"))];
        // « PC » se resout, mais vers une AUTRE machine.
        let t = regrouper(&recues, 1, |_| vec![ip("192.168.1.99")]);
        assert!(!t[0].nom_resolu);
        assert_eq!(t[0].adresse_conseillee, "192.168.1.20");
    }

    #[test]
    fn un_serveur_hors_plage_ou_d_un_autre_protocole_se_signale() {
        let mut a = annonce("PC", &["192.168.1.20"], 7300);
        a.version_protocole = 2;
        let t = regrouper(&[(a, ip("192.168.1.20"))], 1, |_| vec![]);
        assert!(!t[0].joignable);
        assert!(!t[0].meme_protocole);
    }

    #[test]
    fn un_serveur_de_ce_poste_se_propose_par_son_nom_ou_la_boucle() {
        let recues = vec![(annonce("MOI", &["192.168.1.20"], 40000), ip("127.0.0.1"))];
        let t = regrouper(&recues, 1, |_| vec![ip("127.0.0.1")]);
        assert!(t[0].nom_resolu);
        assert_eq!(t[0].adresse_ip, "127.0.0.1");
        assert_eq!(t[0].adresse_conseillee, "MOI");
    }
}
