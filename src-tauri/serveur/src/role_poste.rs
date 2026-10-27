//! Le serveur d'une caisse s'efface (D24, revise le 25/10/2026).
//!
//! L'installeur de Gescom pose sur CHAQUE machine le raccourci qui lance
//! `gescom-serveur.exe` a l'ouverture de la session : utile sur le
//! poste qui garde la base (le serveur est pret avant que les caisses
//! s'allument), nuisible sur une caisse — un serveur de plus, sur une
//! base vide a lui, que personne n'utilise et qu'une caisse mal reglee
//! pourrait joindre.
//!
//! Lance SANS `--base` (le raccourci), le serveur lit donc le role du
//! poste dans `poste.json`, a cote de sa base par defaut, et s'arrete
//! si ce poste est la caisse d'une AUTRE machine. Dans le doute, il
//! tourne : un serveur inutile ne coute qu'un peu de memoire, un poste
//! principal eteint arrete toute la boutique. Le service Windows (D12),
//! les tests et le banc donnent `--base` : ils ne sont pas concernes.

use std::net::IpAddr;
use std::path::PathBuf;

/// Le dossier de donnees de Gescom sur ce poste : la base par defaut,
/// et `poste.json`. En build de debogage, `GESCOM_DONNEES` le remplace
/// (le banc d'ecran) — comme pour la fenetre ; un executable livre
/// l'ignore.
pub fn dossier_donnees() -> PathBuf {
    #[cfg(debug_assertions)]
    if let Some(d) = std::env::var_os("GESCOM_DONNEES").filter(|d| !d.is_empty()) {
        return PathBuf::from(d);
    }
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("ml.gescom.app")
}

/// Pourquoi ce serveur ne doit pas tourner ici, ou `None`.
///
/// `poste_json` : le contenu du fichier (absent : `None`).
/// `ici` : les adresses de cette machine, en plus des boucles locales.
/// `resoudre` : un nom de machine → ses adresses (vide si on ne sait
/// pas). Separe pour que la regle se teste sans reseau.
pub fn raison_de_s_effacer(
    poste_json: Option<&str>,
    ici: &[IpAddr],
    resoudre: impl Fn(&str) -> Vec<IpAddr>,
) -> Option<String> {
    // Le Bloc-notes peut y laisser un BOM : on le retire avant de lire.
    let v: serde_json::Value = serde_json::from_str(poste_json?.trim_start_matches('\u{feff}')).ok()?;
    if v.get("mode")?.as_str()? != "poste" {
        return None;
    }
    let serveur = v.get("serveur")?.as_str()?.trim();
    let serveur = serveur.trim_start_matches("http://").trim_end_matches('/');
    let hote = hote_de(serveur);
    if hote.is_empty() {
        return None;
    }
    let adresses: Vec<IpAddr> = match hote.parse::<IpAddr>() {
        Ok(ip) => vec![ip],
        Err(_) if hote.eq_ignore_ascii_case("localhost") => return None,
        Err(_) => resoudre(hote),
    };
    // On ne sait pas ou est ce serveur : on tourne.
    if adresses.is_empty() {
        return None;
    }
    let chez_nous = |ip: &IpAddr| ip.is_loopback() || ip.is_unspecified() || ici.contains(ip);
    if adresses.iter().any(chez_nous) {
        // « caisse » d'elle-meme : c'est le poste principal, branche sur
        // 127.0.0.1 comme on le faisait avant la revision de D24.
        return None;
    }
    Some(format!(
        "ce poste est la caisse de {serveur} (poste.json) : la base est là-bas, \
         ce serveur s'arrête. Pour que ce poste garde la base : Gescom → \
         Paramètres → Réseau → « Ce poste garde la base »."
    ))
}

/// « 192.168.1.10:40000 » → « 192.168.1.10 » ; « [::1]:40000 » → « ::1 ».
fn hote_de(serveur: &str) -> &str {
    if let Some(reste) = serveur.strip_prefix('[') {
        return reste.split(']').next().unwrap_or("");
    }
    match serveur.matches(':').count() {
        // Une adresse IPv6 nue a plusieurs « : » et pas de port.
        0 | 2.. => serveur,
        _ => serveur.split(':').next().unwrap_or(""),
    }
}

/// La regle, sur la vraie machine : le `poste.json` du dossier de
/// donnees, l'adresse de ce poste sur le reseau local, le DNS du systeme.
pub fn verifier() -> Option<String> {
    let texte = std::fs::read_to_string(dossier_donnees().join("poste.json")).ok();
    let ici: Vec<IpAddr> = crate::reseau_local::adresse_locale()
        .and_then(|a| a.parse().ok())
        .into_iter()
        .collect();
    raison_de_s_effacer(texte.as_deref(), &ici, |hote| {
        use std::net::ToSocketAddrs;
        (hote, 0u16)
            .to_socket_addrs()
            .map(|it| it.map(|a| a.ip()).collect())
            .unwrap_or_default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ici() -> Vec<IpAddr> {
        vec!["192.168.1.20".parse().unwrap()]
    }
    fn personne(_: &str) -> Vec<IpAddr> {
        vec![]
    }
    fn json(mode: &str, serveur: &str) -> String {
        format!(r#"{{"mode":"{mode}","serveur":"{serveur}","poste_nom":"C","poste_empreinte":"e"}}"#)
    }

    #[test]
    fn la_caisse_d_un_autre_poste_s_efface() {
        let r = raison_de_s_effacer(Some(&json("poste", "192.168.1.10:40000")), &ici(), personne);
        assert!(r.unwrap().contains("192.168.1.10:40000"));
    }

    #[test]
    fn le_poste_qui_garde_la_base_tourne() {
        assert!(raison_de_s_effacer(Some(&json("monoposte", "127.0.0.1:40000")), &ici(), personne).is_none());
    }

    #[test]
    fn un_poste_neuf_ou_sans_fichier_tourne() {
        assert!(raison_de_s_effacer(None, &ici(), personne).is_none());
        assert!(raison_de_s_effacer(Some(&json("a_choisir", "")), &ici(), personne).is_none());
        assert!(raison_de_s_effacer(Some("pas du json"), &ici(), personne).is_none());
    }

    #[test]
    fn une_caisse_branchee_sur_elle_meme_est_le_poste_principal() {
        // Le reglage du proprietaire avant la revision : « poste »
        // sur 127.0.0.1. Son serveur doit continuer de tourner.
        for s in ["127.0.0.1:40000", "localhost:40000", "LOCALHOST", "[::1]:40000", "::1", "192.168.1.20:40000", "0.0.0.0:40000"] {
            assert!(raison_de_s_effacer(Some(&json("poste", s)), &ici(), personne).is_none(), "{s}");
        }
    }

    #[test]
    fn un_nom_de_machine_se_resout_et_dans_le_doute_on_tourne() {
        let principal = |h: &str| if h == "caisse-principale" { vec!["192.168.1.10".parse().unwrap()] } else { vec![] };
        let moi = |h: &str| if h == "moi" { vec!["192.168.1.20".parse().unwrap()] } else { vec![] };
        assert!(raison_de_s_effacer(Some(&json("poste", "caisse-principale:40000")), &ici(), principal).is_some());
        assert!(raison_de_s_effacer(Some(&json("poste", "moi:40000")), &ici(), moi).is_none());
        // Nom introuvable (reseau coupe au demarrage) : on tourne.
        assert!(raison_de_s_effacer(Some(&json("poste", "inconnu:40000")), &ici(), personne).is_none());
    }

    #[test]
    fn une_caisse_sans_adresse_tourne() {
        assert!(raison_de_s_effacer(Some(&json("poste", "")), &ici(), personne).is_none());
    }
}
