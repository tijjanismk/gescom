//! L'empreinte du poste — ce à quoi une licence s'attache.
//!
//! ## Ce qu'on cherche, et ce qu'on ne cherche pas
//!
//! On veut un identifiant STABLE dans le temps sur une même machine, et
//! DIFFÉRENT d'une machine à l'autre. On ne cherche pas à identifier une
//! personne : l'empreinte est un condensé, elle ne se remonte pas vers
//! un numéro de série ni un nom d'utilisateur.
//!
//! ## Pourquoi le MachineGuid et rien d'autre
//!
//! Windows écrit un GUID à l'installation, dans
//! `HKLM\SOFTWARE\Microsoft\Cryptography\MachineGuid`. Il survit aux
//! changements de nom de machine, de disque, de carte réseau — tout ce
//! qui change dans la vraie vie d'une boutique quand on remplace une
//! pièce ou qu'on rebaptise le poste « CAISSE2 ».
//!
//! Y ajouter le nom de la machine ou le numéro de volume, comme le font
//! beaucoup de protections, casse la licence le jour où le commerçant
//! renomme son poste ou remplace un disque. Il rappelle alors le
//! vendeur un samedi de marché, furieux, pour une panne qu'on a
//! fabriquée soi-même.
//!
//! Ce qui casse la licence, en revanche : une réinstallation de
//! Windows. C'est assumé — le vendeur réémet. C'est aussi ce qui rend
//! la copie visible.

use sha2::{Digest, Sha256};

/// Sel du condensé.
///
/// Empêche de reconnaître un MachineGuid en comparant à une table
/// pré-calculée. L'empreinte reste un identifiant technique, pas une
/// donnée qu'on peut retourner contre le poste.
const SEL: &str = "gescom-poste-v1";

/// L'empreinte affichable : 12 caractères, en trois groupes.
///
/// `A1B2-C3D4-E5F6` se dicte au téléphone sans se tromper — et c'est
/// exactement comme ça qu'elle voyagera : le commerçant appelle, lit
/// son code, reçoit sa licence par WhatsApp.
///
/// Douze caractères hexadécimaux valent 48 bits. Deux postes tirant la
/// même empreinte est un événement qu'on ne verra jamais sur un parc de
/// quelques milliers de machines, et allonger le code le rendrait
/// pénible à dicter — ce qui coûterait plus, en appels, que la
/// collision qu'on évite.
pub fn empreinte_poste() -> String {
    let brut = identifiant_machine();
    let mut h = Sha256::new();
    h.update(SEL.as_bytes());
    h.update(brut.as_bytes());
    let condense = h.finalize();

    let hexa: String = condense
        .iter()
        .take(6)
        .map(|o| format!("{o:02X}"))
        .collect();
    format!("{}-{}-{}", &hexa[0..4], &hexa[4..8], &hexa[8..12])
}

#[cfg(windows)]
fn identifiant_machine() -> String {
    use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_64KEY};
    use winreg::RegKey;

    // KEY_WOW64_64KEY : sans lui, un binaire 32 bits est redirigé vers
    // la ruche WOW6432Node, où la clef n'existe pas — l'empreinte
    // tomberait alors sur le repli et changerait à chaque poste.
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let lu = hklm
        .open_subkey_with_flags(
            r"SOFTWARE\Microsoft\Cryptography",
            KEY_READ | KEY_WOW64_64KEY,
        )
        .and_then(|k| k.get_value::<String, _>("MachineGuid"));

    match lu {
        Ok(guid) if !guid.trim().is_empty() => guid,
        // Repli : une machine dont le GUID est illisible ne doit pas
        // devenir inactivable. Elle recevra une empreinte moins stable,
        // ce que le vendeur verra à la deuxième demande de licence.
        _ => repli(),
    }
}

#[cfg(not(windows))]
fn identifiant_machine() -> String {
    // Gescom ne cible que Windows. Ce chemin existe pour que les tests
    // tournent partout, et pour ne pas devoir réécrire le module le
    // jour d'un portage.
    std::env::var("GESCOM_EMPREINTE")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(repli)
}

/// Dernier recours : un identifiant tiré une fois et gardé sur le
/// disque, à côté des données de l'application.
fn repli() -> String {
    let dossier = std::env::var("APPDATA")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".to_string());
    let chemin = std::path::Path::new(&dossier)
        .join("ml.gescom.app")
        .join("poste.id");

    if let Ok(v) = std::fs::read_to_string(&chemin) {
        if !v.trim().is_empty() {
            return v.trim().to_string();
        }
    }
    let neuf = uuid::Uuid::new_v4().to_string();
    if let Some(parent) = chemin.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(&chemin, &neuf).ok();
    neuf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_format_se_dicte() {
        let e = empreinte_poste();
        assert_eq!(e.len(), 14, "AAAA-BBBB-CCCC");
        assert_eq!(e.chars().filter(|c| *c == '-').count(), 2);
        assert!(e.chars().all(|c| c.is_ascii_hexdigit() || c == '-'), "{e}");
    }

    #[test]
    fn deux_appels_donnent_la_meme_empreinte() {
        // Sans cette stabilité, la licence tomberait au redémarrage —
        // et le commerçant appellerait tous les matins.
        assert_eq!(empreinte_poste(), empreinte_poste());
    }
}
