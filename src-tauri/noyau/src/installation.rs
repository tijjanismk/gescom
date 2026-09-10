//! Gescom doit être **installé**, pas copié.
//!
//! ## Le problème
//!
//! `Gescom.exe` se suffit à lui-même : SQLite est compilé dedans, le
//! front est embarqué. Posé sur une clé USB, il démarrait sur
//! n'importe quel poste. Il suffisait d'un employé qui copie un fichier
//! pour que le logiciel se retrouve dans une deuxième boutique.
//!
//! ## Ce qu'on vérifie, et pourquoi c'est celui-là
//!
//! L'installateur NSIS écrit deux traces dans le registre de
//! l'utilisateur :
//!
//! - `HKCU\Software\Gescom\Gescom` (valeur par défaut) = le dossier
//!   d'installation ;
//! - `HKCU\…\Uninstall\Gescom` → `InstallLocation`, entre guillemets.
//!
//! Au démarrage, on compare le dossier de l'exécutable qui tourne à
//! celui-là. Copié ailleurs — clé USB, autre poste, autre dossier — la
//! comparaison échoue et le logiciel refuse.
//!
//! ## Ce que ce n'est pas
//!
//! Ce n'est **pas** une licence : rien à activer, rien à demander à
//! personne, aucun fichier à recevoir. Installer suffit. Ce n'est pas
//! non plus une protection contre quelqu'un qui démonte l'exécutable ou
//! qui relance l'installateur sur dix machines — ce n'est pas ce qu'on
//! cherche à empêcher. Ce qu'on empêche, c'est le glisser-déposer.

/// Ce que le démarrage doit faire de la situation.
#[derive(Debug, Clone, PartialEq)]
pub enum Etat {
    /// Lancé depuis son dossier d'installation.
    Installee,
    /// Aucune trace d'installation sur ce poste.
    NonInstallee,
    /// Installé, mais l'exécutable qui tourne vient d'ailleurs.
    Deplacee { attendu: String, reel: String },
}

impl Etat {
    pub fn autorise(&self) -> bool {
        matches!(self, Etat::Installee)
    }

    /// Ce qu'on montre à l'écran. Le message doit dire quoi FAIRE : un
    /// « erreur de démarrage » sans suite produit un appel, et de
    /// l'inquiétude sur des données qui vont parfaitement bien.
    pub fn message(&self) -> String {
        match self {
            Etat::Installee => String::new(),
            Etat::NonInstallee =>
                "Cette copie de Gescom n'a pas été installée sur cet ordinateur.\n\n\
                 Gescom ne fonctionne pas depuis une clé USB ni depuis un fichier \
                 copié. Lancer le programme d'installation, puis ouvrir Gescom \
                 depuis le menu Démarrer.".to_string(),
            Etat::Deplacee { attendu, .. } => format!(
                "Ce fichier Gescom.exe a été déplacé hors de son dossier \
                 d'installation.\n\n\
                 Ouvrir Gescom depuis le menu Démarrer, ou depuis :\n{attendu}\n\n\
                 Vos données ne sont pas touchées : elles ne sont pas dans ce \
                 fichier."
            ),
        }
    }
}

/// Vérifie que l'exécutable courant tourne depuis son installation.
pub fn verifier() -> Etat {
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        // Sans chemin d'exécutable, on ne peut rien affirmer. Bloquer
        // ici punirait un cas qu'on ne comprend pas ; on laisse passer
        // plutôt que d'inventer une panne.
        Err(_) => return Etat::Installee,
    };
    let Some(dossier_exe) = exe.parent() else {
        return Etat::Installee;
    };

    match dossier_installation() {
        None => Etat::NonInstallee,
        Some(attendu) => {
            if memes_dossiers(dossier_exe, std::path::Path::new(&attendu)) {
                Etat::Installee
            } else {
                Etat::Deplacee {
                    attendu,
                    reel: dossier_exe.to_string_lossy().to_string(),
                }
            }
        }
    }
}

/// Compare deux dossiers en tenant compte de ce que Windows tolère.
///
/// `canonicalize` résout les liens, les `..` et la casse réelle du
/// disque : c'est ce qui évite de refuser une installation parfaitement
/// valide parce que le registre dit `C:\Users\…` et l'exécutable
/// `C:\USERS\…`. Si la résolution échoue (dossier supprimé, droits),
/// on retombe sur une comparaison textuelle insensible à la casse.
fn memes_dossiers(a: &std::path::Path, b: &std::path::Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(x), Ok(y)) => x == y,
        _ => {
            let norme = |p: &std::path::Path| {
                p.to_string_lossy()
                    .trim_end_matches(['\\', '/'])
                    .to_lowercase()
                    .replace('/', "\\")
            };
            norme(a) == norme(b)
        }
    }
}

#[cfg(windows)]
fn dossier_installation() -> Option<String> {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;

    // Les deux ruches : l'installateur écrit dans HKCU en mode
    // « utilisateur courant » (le réglage actuel) et dans HKLM si l'on
    // passe un jour en installation pour tous. Chercher dans une seule
    // ferait échouer le contrôle le jour du changement, chez tout le
    // monde à la fois.
    for ruche in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        let racine = RegKey::predef(ruche);

        // 1. La clef que l'installateur écrit pour se souvenir du
        //    dossier choisi. Valeur non guillemetée.
        if let Ok(v) = racine
            .open_subkey(r"Software\Gescom\Gescom")
            .and_then(|k| k.get_value::<String, _>(""))
        {
            if !v.trim().is_empty() {
                return Some(v.trim().to_string());
            }
        }

        // 2. Repli : l'entrée « Ajout/Suppression de programmes ».
        //    `InstallLocation` y est écrite ENTRE GUILLEMETS par NSIS ;
        //    les garder ferait échouer toute comparaison de chemin.
        if let Ok(v) = racine
            .open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Uninstall\Gescom")
            .and_then(|k| k.get_value::<String, _>("InstallLocation"))
        {
            let propre = v.trim().trim_matches('"').trim();
            if !propre.is_empty() {
                return Some(propre.to_string());
            }
        }
    }
    None
}

#[cfg(not(windows))]
fn dossier_installation() -> Option<String> {
    // Gescom ne cible que Windows. Ailleurs, on ne bloque rien : le
    // contrôle n'aurait aucune trace sur laquelle s'appuyer.
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_string_lossy().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_dossier_est_egal_a_lui_meme() {
        let ici = std::env::current_dir().unwrap();
        assert!(memes_dossiers(&ici, &ici));
    }

    #[test]
    fn la_casse_et_le_slash_final_ne_comptent_pas() {
        // Le registre et l'exécutable ne s'accordent pas toujours sur la
        // casse ni sur l'antislash final. Refuser pour ça bloquerait une
        // installation parfaitement valide.
        let a = std::path::PathBuf::from(r"C:\Program Files\Gescom");
        let b = std::path::PathBuf::from(r"c:\program files\gescom\");
        assert!(memes_dossiers(&a, &b));
    }

    #[test]
    fn deux_dossiers_differents_ne_se_confondent_pas() {
        let a = std::path::PathBuf::from(r"C:\Program Files\Gescom");
        let b = std::path::PathBuf::from(r"E:\Gescom");
        assert!(!memes_dossiers(&a, &b));
    }

    #[test]
    fn les_messages_disent_quoi_faire() {
        // Un blocage sans marche à suivre produit un appel, et de
        // l'inquiétude sur des données qui vont très bien.
        assert!(Etat::NonInstallee.message().contains("installation"));
        let d = Etat::Deplacee {
            attendu: r"C:\Gescom".to_string(),
            reel: r"E:\".to_string(),
        };
        assert!(d.message().contains(r"C:\Gescom"));
        assert!(d.message().contains("données"));
        assert!(Etat::Installee.message().is_empty());
    }

    #[test]
    fn seule_l_installation_autorise() {
        assert!(Etat::Installee.autorise());
        assert!(!Etat::NonInstallee.autorise());
        assert!(!Etat::Deplacee { attendu: String::new(), reel: String::new() }.autorise());
    }
}
