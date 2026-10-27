//! Le reseau du serveur, regle depuis sa console (27/10/2026).
//!
//! « Une interface graphique dediee a mon serveur, comme sur Ciel », pour
//! regler l'IP et le port sans ouvrir `serveur.json` au Bloc-notes. Ici
//! les regles, pures et testees : qui peut changer, quoi, et comment le
//! fichier se reecrit. Le serveur les applique (`serveur/src/api.rs`,
//! route `/reseau`) et ecrit le geste au journal (`reseau_serveur.rs`).
//!
//! Trois garde-fous :
//!
//! - **seulement depuis le poste du serveur** (requete venue de la boucle
//!   locale) : une caisse ne reconfigure pas le serveur — elle se
//!   couperait elle-meme, et toutes les autres avec ;
//! - **pas quand la ligne de commande decide** (`--port`, `--hote`) : sur
//!   le poste qui garde la base, c'est la fenetre de Gescom qui lance le
//!   serveur et choisit son port dans la plage (D39) ; ecrire un fichier
//!   que personne ne relit serait faire semblant ;
//! - **un port de la plage** 40000-40100 (D39) : hors d'elle, aucune
//!   caisse ne joindrait plus le serveur.

use std::net::IpAddr;

use serde_json::{json, Value};

use crate::decouverte::{avec_port, nom_utilisable};
use crate::protocole::{port_dans_la_plage, PORT_DEFAUT, PORT_MAX};

/// Ecouter sur toutes les cartes : les caisses joignent le serveur.
pub const ECOUTE_TOUTES: &str = "0.0.0.0";
/// Ecouter sur ce poste seulement : aucune caisse ne le joint (un poste
/// qui travaille seul, ou le temps d'une reparation).
pub const ECOUTE_CE_POSTE: &str = "127.0.0.1";

/// Ce que le serveur ecoute.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reglage {
    pub port: u16,
    pub ecoute: String,
}

impl Reglage {
    /// « 0.0.0.0:40000 », pour le journal.
    pub fn en_texte(&self) -> String {
        format!("{}:{}", self.ecoute, self.port)
    }
}

/// L'ecoute dite au patron.
pub fn libelle_ecoute(ecoute: &str) -> String {
    match ecoute.trim() {
        ECOUTE_TOUTES => "toutes les cartes réseau : les caisses le joignent".to_string(),
        ECOUTE_CE_POSTE => "ce poste seulement : aucune caisse ne le joint".to_string(),
        autre => format!("l'adresse {autre} seulement (réglée à la main dans serveur.json)"),
    }
}

/// La requete vient-elle du poste du serveur lui-meme ? `ip` : l'adresse
/// de l'appelant telle que le serveur la voit. 127.0.0.0/8, ::1, et
/// l'IPv4 de boucle ecrite en IPv6 (« ::ffff:127.0.0.1 »).
pub fn requete_locale(ip: &str) -> bool {
    match ip.trim().parse::<IpAddr>() {
        Ok(IpAddr::V4(v4)) => v4.is_loopback(),
        Ok(IpAddr::V6(v6)) => v6.is_loopback() || v6.to_ipv4_mapped().is_some_and(|v4| v4.is_loopback()),
        Err(_) => false,
    }
}

/// Pourquoi un changement est refuse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refus {
    /// La requete vient d'une caisse, pas du poste du serveur.
    PasDuPosteServeur,
    /// `--port` ou `--hote` sur la ligne de commande : le fichier n'y
    /// changerait rien.
    GereParLaLigneDeCommande,
    /// Le port demande, tel qu'ecrit (il peut ne pas tenir sur 16 bits).
    PortHorsPlage(i64),
    EcouteInconnue(String),
    RienNeChange,
}

impl Refus {
    pub fn message(&self) -> String {
        match self {
            Refus::PasDuPosteServeur => "Le port et l'écoute du serveur se changent seulement sur le poste du \
                 serveur lui-même (fenêtre Gescom Serveur, ou http://127.0.0.1:<port>/console sur ce poste) : \
                 une caisse ne reconfigure pas le serveur."
                .to_string(),
            Refus::GereParLaLigneDeCommande => "Sur ce poste, le port est géré par Gescom : le serveur a été lancé \
                 avec --port ou --hote (par la fenêtre de Gescom, sur le poste qui garde la base, D39). \
                 Le changer ici ne servirait à rien."
                .to_string(),
            Refus::PortHorsPlage(p) => format!(
                "Le port {p} n'est pas dans la plage {PORT_DEFAUT}-{PORT_MAX} : les caisses Gescom ne joignent \
                 le serveur que sur ces ports. Choisir un port de la plage."
            ),
            Refus::EcouteInconnue(e) => format!(
                "Écoute « {e} » inconnue : « {ECOUTE_TOUTES} » (toutes les cartes réseau) ou « {ECOUTE_CE_POSTE} » \
                 (ce poste seulement)."
            ),
            Refus::RienNeChange => "Rien ne change : c'est déjà le port et l'écoute du serveur.".to_string(),
        }
    }
}

/// Juge une demande de changement. Rend le reglage a ecrire.
///
/// L'ordre des refus est celui qu'on veut lire : d'abord QUI demande et
/// D'OU, ensuite ce qu'il demande. L'ecoute demandee peut rester celle
/// d'aujourd'hui meme si elle a ete reglee a la main (une adresse precise
/// dans `serveur.json`) : changer le port ne doit pas forcer a la changer.
pub fn juger(
    actuel: &Reglage,
    demande: &Reglage,
    depuis_ce_poste: bool,
    gere_par_la_ligne_de_commande: bool,
) -> Result<Reglage, Refus> {
    if !depuis_ce_poste {
        return Err(Refus::PasDuPosteServeur);
    }
    if gere_par_la_ligne_de_commande {
        return Err(Refus::GereParLaLigneDeCommande);
    }
    if !port_dans_la_plage(demande.port) {
        return Err(Refus::PortHorsPlage(i64::from(demande.port)));
    }
    let ecoute = demande.ecoute.trim().to_string();
    if ecoute != ECOUTE_TOUTES && ecoute != ECOUTE_CE_POSTE && ecoute != actuel.ecoute.trim() {
        return Err(Refus::EcouteInconnue(ecoute));
    }
    let nouveau = Reglage { port: demande.port, ecoute };
    if nouveau.port == actuel.port && nouveau.ecoute == actuel.ecoute.trim() {
        return Err(Refus::RienNeChange);
    }
    Ok(nouveau)
}

/// Pourquoi le panneau ne propose pas de changer, ou `None`.
pub fn pourquoi_pas(depuis_ce_poste: bool, gere_par_la_ligne_de_commande: bool, a_la_permission: bool) -> Option<String> {
    if !a_la_permission {
        return Some(
            "Changer le port ou l'écoute demande la permission « Modifier les paramètres » (le patron).".to_string(),
        );
    }
    if !depuis_ce_poste {
        return Some(Refus::PasDuPosteServeur.message());
    }
    if gere_par_la_ligne_de_commande {
        return Some(Refus::GereParLaLigneDeCommande.message());
    }
    None
}

/// Le nouveau contenu de `serveur.json` : le port et l'ecoute changent,
/// TOUT LE RESTE est garde (la base — avec son mot de passe PostgreSQL —,
/// les sauvegardes, le journal, des cles qu'on ne connait pas).
///
/// Un fichier illisible n'est PAS ecrase : il peut porter l'adresse de la
/// base, et le remplacer par deux cles ferait demarrer le service sur une
/// base neuve et vide. On refuse et on dit de le corriger.
pub fn fusionner_configuration(existant: Option<&str>, reglage: &Reglage) -> Result<String, String> {
    let texte = existant.map(|t| t.trim_start_matches('\u{feff}').trim()).filter(|t| !t.is_empty());
    let mut objet = match texte {
        None => serde_json::Map::new(),
        Some(t) => match serde_json::from_str::<Value>(t) {
            Ok(Value::Object(m)) => m,
            _ => {
                return Err("serveur.json est illisible : Gescom ne l'écrase pas (il peut porter l'adresse de la \
                            base). Le corriger au Bloc-notes, puis recommencer."
                    .to_string())
            }
        },
    };
    objet.insert("port".to_string(), json!(reglage.port));
    objet.insert("hote".to_string(), json!(reglage.ecoute));
    serde_json::to_string_pretty(&Value::Object(objet))
        .map(|t| format!("{t}\n"))
        .map_err(|e| e.to_string())
}

/// Ce qu'on tape sur une caisse, dans l'ordre ou on le conseille : le NOM
/// du poste d'abord (il ne change pas quand la box redistribue les
/// adresses), puis les IP. Sans port quand c'est celui par defaut.
pub fn adresses_a_saisir(poste: &str, ips: &[String], port: u16) -> Vec<String> {
    let mut a = Vec::new();
    if nom_utilisable(poste) {
        a.push(avec_port(poste.trim(), port));
    }
    for ip in ips {
        let x = avec_port(ip.trim(), port);
        if !ip.trim().is_empty() && !a.contains(&x) {
            a.push(x);
        }
    }
    a
}

/// L'avertissement quand l'adresse principale du poste est donnee par la
/// box (DHCP) : elle peut changer apres une coupure, et les caisses
/// reglees sur elle ne trouveraient plus le serveur.
pub fn avertissement_dhcp(ip_principale: Option<&str>, en_dhcp: bool, poste: &str) -> Option<String> {
    let ip = ip_principale?.trim();
    if !en_dhcp || ip.is_empty() {
        return None;
    }
    let conseil = if nom_utilisable(poste) {
        format!("Sur les caisses, utiliser plutôt le nom du poste, « {} »", poste.trim())
    } else {
        "Sur les caisses, utiliser plutôt « Rechercher le serveur »".to_string()
    };
    Some(format!(
        "L'adresse {ip} est donnée par la box (DHCP) : elle peut changer après une coupure ou un redémarrage, \
         et les caisses réglées sur elle ne trouveraient plus le serveur. {conseil} ; ou réserver cette \
         adresse pour ce poste dans la box."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(port: u16, ecoute: &str) -> Reglage {
        Reglage { port, ecoute: ecoute.to_string() }
    }

    #[test]
    fn seule_la_boucle_locale_est_le_poste_du_serveur() {
        for oui in ["127.0.0.1", "127.0.0.5", "::1", "::ffff:127.0.0.1", " 127.0.0.1 "] {
            assert!(requete_locale(oui), "{oui}");
        }
        for non in ["192.168.1.20", "10.0.0.1", "0.0.0.0", "fe80::1", "::ffff:192.168.1.2", "?", "", "localhost"] {
            assert!(!requete_locale(non), "{non}");
        }
    }

    #[test]
    fn un_changement_se_juge_dans_l_ordre() {
        let actuel = r(40000, ECOUTE_TOUTES);
        assert_eq!(juger(&actuel, &r(40005, ECOUTE_TOUTES), true, false), Ok(r(40005, ECOUTE_TOUTES)));
        assert_eq!(juger(&actuel, &r(40000, ECOUTE_CE_POSTE), true, false), Ok(r(40000, ECOUTE_CE_POSTE)));
        assert_eq!(juger(&actuel, &r(40100, " 127.0.0.1 "), true, false), Ok(r(40100, ECOUTE_CE_POSTE)));
        // D'une caisse : refuse, quoi qu'elle demande.
        assert_eq!(juger(&actuel, &r(7300, "x"), false, true), Err(Refus::PasDuPosteServeur));
        // Lance avec --port : le fichier n'y changerait rien.
        assert_eq!(juger(&actuel, &r(40005, ECOUTE_TOUTES), true, true), Err(Refus::GereParLaLigneDeCommande));
        // Hors de la plage (D39).
        for p in [7300, 39999, 40101, 0] {
            assert_eq!(juger(&actuel, &r(p, ECOUTE_TOUTES), true, false), Err(Refus::PortHorsPlage(i64::from(p))));
        }
        assert_eq!(
            juger(&actuel, &r(40001, "192.168.1.20"), true, false),
            Err(Refus::EcouteInconnue("192.168.1.20".into()))
        );
        assert_eq!(juger(&actuel, &r(40000, "0.0.0.0"), true, false), Err(Refus::RienNeChange));
    }

    #[test]
    fn une_ecoute_reglee_a_la_main_peut_rester() {
        let actuel = r(40000, "192.168.1.20");
        assert_eq!(juger(&actuel, &r(40002, "192.168.1.20"), true, false), Ok(r(40002, "192.168.1.20")));
        assert_eq!(juger(&actuel, &r(40000, ECOUTE_TOUTES), true, false), Ok(r(40000, ECOUTE_TOUTES)));
    }

    #[test]
    fn les_messages_disent_quoi_faire() {
        assert!(Refus::PortHorsPlage(7300).message().contains("40000-40100"));
        assert!(Refus::PortHorsPlage(70000).message().contains("70000"));
        assert!(Refus::GereParLaLigneDeCommande.message().contains("géré par Gescom"));
        assert!(Refus::PasDuPosteServeur.message().contains("poste du serveur"));
        assert!(pourquoi_pas(true, false, false).unwrap().contains("Modifier les paramètres"));
        assert_eq!(pourquoi_pas(false, false, true), Some(Refus::PasDuPosteServeur.message()));
        assert_eq!(pourquoi_pas(true, true, true), Some(Refus::GereParLaLigneDeCommande.message()));
        assert_eq!(pourquoi_pas(true, false, true), None);
    }

    #[test]
    fn le_fichier_garde_tout_le_reste() {
        let existant = "\u{feff}{\r\n  \"base\": \"postgresql://gescom:secret@127.0.0.1:5432/gescom\",\r\n  \"port\": 40000,\r\n  \"sauvegardes\": \"D:\\\\Sauvegardes\",\r\n  \"journal\": \"D:\\\\journal.log\",\r\n  \"cle_inconnue\": [1, 2]\r\n}\r\n";
        let nouveau = fusionner_configuration(Some(existant), &r(40007, ECOUTE_CE_POSTE)).unwrap();
        let v: Value = serde_json::from_str(&nouveau).unwrap();
        assert_eq!(v["port"], 40007);
        assert_eq!(v["hote"], "127.0.0.1");
        assert_eq!(v["base"], "postgresql://gescom:secret@127.0.0.1:5432/gescom");
        assert_eq!(v["sauvegardes"], "D:\\Sauvegardes");
        assert_eq!(v["journal"], "D:\\journal.log");
        assert_eq!(v["cle_inconnue"], json!([1, 2]));
        assert!(nouveau.ends_with('\n'));
    }

    #[test]
    fn un_fichier_absent_se_cree_un_fichier_illisible_ne_s_ecrase_pas() {
        let v: Value = serde_json::from_str(&fusionner_configuration(None, &r(40001, ECOUTE_TOUTES)).unwrap()).unwrap();
        assert_eq!(v, json!({ "port": 40001, "hote": "0.0.0.0" }));
        assert!(fusionner_configuration(Some("  "), &r(40001, ECOUTE_TOUTES)).is_ok());
        for illisible in ["{ \"base\": \"C:\\gescom.db\" ", "[1]", "40000", "pas du json"] {
            let e = fusionner_configuration(Some(illisible), &r(40001, ECOUTE_TOUTES)).unwrap_err();
            assert!(e.contains("ne l'écrase pas"), "{illisible} : {e}");
        }
    }

    #[test]
    fn le_nom_du_poste_d_abord_puis_les_ip() {
        let ips = vec!["192.168.1.20".to_string(), "10.0.0.4".to_string()];
        assert_eq!(adresses_a_saisir("BOUTIQUE-PC", &ips, 40000), ["BOUTIQUE-PC", "192.168.1.20", "10.0.0.4"]);
        assert_eq!(adresses_a_saisir("BOUTIQUE-PC", &ips[..1], 40005), ["BOUTIQUE-PC:40005", "192.168.1.20:40005"]);
        // Un nom qu'on ne peut pas taper : les IP seulement.
        assert_eq!(adresses_a_saisir("Poste de Awa", &ips[..1], 40000), ["192.168.1.20"]);
    }

    #[test]
    fn une_adresse_de_la_box_avertit_et_conseille_le_nom() {
        let a = avertissement_dhcp(Some("192.168.1.20"), true, "BOUTIQUE-PC").unwrap();
        assert!(a.contains("DHCP") && a.contains("« BOUTIQUE-PC »") && a.contains("192.168.1.20"), "{a}");
        assert!(avertissement_dhcp(Some("192.168.1.20"), false, "BOUTIQUE-PC").is_none());
        assert!(avertissement_dhcp(None, true, "BOUTIQUE-PC").is_none());
        assert!(avertissement_dhcp(Some("192.168.1.20"), true, "Poste de Awa").unwrap().contains("Rechercher le serveur"));
    }

    #[test]
    fn l_ecoute_se_dit_en_francais() {
        assert!(libelle_ecoute("0.0.0.0").contains("toutes les cartes"));
        assert!(libelle_ecoute("127.0.0.1").contains("ce poste seulement"));
        assert!(libelle_ecoute("192.168.1.20").contains("192.168.1.20"));
        assert_eq!(r(40000, "0.0.0.0").en_texte(), "0.0.0.0:40000");
    }
}
