//! `serveur.json` : ou il est, comment il se reecrit (27/10/2026).
//!
//! Le fichier du service Windows (D12), `%ProgramData%\Gescom\serveur.json`.
//! Le service le lit en entier ; un serveur lance a la main sans `--port`
//! ni `--hote` y lit son port et son ecoute — c'est ce qui permet au
//! panneau Reseau de la console de les changer, et au serveur relance de
//! les reprendre.
//!
//! En build de debogage, `GESCOM_SERVEUR_JSON` remplace le chemin : un
//! test ne doit jamais ecrire dans le vrai `ProgramData` du poste. Sans
//! elle, un serveur de debogage REFUSE d'ecrire (il lit seulement) — un
//! essai oublie ne reconfigure pas le serveur de la boutique.

use std::path::{Path, PathBuf};

/// La variable qui remplace le chemin, en debogage seulement.
pub const VARIABLE: &str = "GESCOM_SERVEUR_JSON";

/// Le chemin remplace par `GESCOM_SERVEUR_JSON` (debogage seulement).
fn remplacement() -> Option<PathBuf> {
    #[cfg(debug_assertions)]
    if let Some(c) = std::env::var_os(VARIABLE).filter(|c| !c.is_empty()) {
        return Some(PathBuf::from(c));
    }
    None
}

/// Le fichier de configuration du serveur de ce poste, ou `None` hors de
/// Windows (pas de `ProgramData`, pas de service).
pub fn chemin() -> Option<PathBuf> {
    if let Some(c) = remplacement() {
        return Some(c);
    }
    #[cfg(windows)]
    {
        Some(crate::service::chemin_configuration())
    }
    #[cfg(not(windows))]
    {
        None
    }
}

/// Peut-on ECRIRE ce fichier depuis ce binaire ? Livre : oui. Debogage :
/// seulement quand le chemin est remplace (voir l'en-tete).
pub fn ecriture_permise() -> Result<PathBuf, String> {
    if cfg!(debug_assertions) && remplacement().is_none() {
        return Err(format!(
            "Build de débogage : le serveur n'écrit pas le vrai serveur.json de ce poste. \
             Poser {VARIABLE} sur un fichier d'essai."
        ));
    }
    chemin().ok_or_else(|| "Ce système n'a pas de fichier de configuration du serveur.".to_string())
}

/// Le contenu actuel, `None` s'il n'existe pas. Une erreur de lecture
/// autre que « absent » est rendue : on ne reecrit pas un fichier qu'on
/// n'a pas pu lire.
pub fn lire(chemin: &Path) -> Result<Option<String>, String> {
    match std::fs::read_to_string(chemin) {
        Ok(t) => Ok(Some(t)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("Lecture de {} impossible : {e}", chemin.display())),
    }
}

/// Ecrit sans jamais laisser un fichier a moitie ecrit : d'abord un
/// fichier temporaire a cote, puis on le renomme par-dessus l'ancien (un
/// renommage dans le meme dossier est atomique sous Windows comme
/// ailleurs). Une coupure de courant au mauvais moment laisse l'ancien
/// fichier, ou le nouveau — jamais une moitie.
pub fn ecrire(chemin: &Path, contenu: &str) -> Result<(), String> {
    if let Some(dossier) = chemin.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dossier)
            .map_err(|e| format!("Dossier {} impossible à créer : {e}", dossier.display()))?;
    }
    let temporaire = chemin.with_extension(format!("json.{}.tmp", std::process::id()));
    let ecrit = (|| -> std::io::Result<()> {
        use std::io::Write;
        let mut f = std::fs::File::create(&temporaire)?;
        f.write_all(contenu.as_bytes())?;
        f.sync_all()
    })();
    if let Err(e) = ecrit {
        let _ = std::fs::remove_file(&temporaire);
        return Err(format!("Écriture de {} impossible : {e}", temporaire.display()));
    }
    std::fs::rename(&temporaire, chemin).map_err(|e| {
        let _ = std::fs::remove_file(&temporaire);
        format!(
            "Remplacement de {} impossible : {e} (droits du dossier ? le service écrit sous LocalSystem, \
             un serveur lancé à la main sous le compte de la session).",
            chemin.display()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l_ecriture_remplace_d_un_coup_et_ne_laisse_rien_derriere() {
        let dossier = std::env::temp_dir().join(format!("gescom_conf_{}", std::process::id()));
        let fichier = dossier.join("serveur.json");
        assert_eq!(lire(&fichier).unwrap(), None);
        ecrire(&fichier, "{\"port\": 40000}\n").unwrap();
        ecrire(&fichier, "{\"port\": 40001}\n").unwrap();
        assert_eq!(lire(&fichier).unwrap().as_deref(), Some("{\"port\": 40001}\n"));
        let restes: Vec<_> = std::fs::read_dir(&dossier).unwrap().flatten().map(|e| e.file_name()).collect();
        assert_eq!(restes.len(), 1, "{restes:?}");
        let _ = std::fs::remove_dir_all(&dossier);
    }
}
