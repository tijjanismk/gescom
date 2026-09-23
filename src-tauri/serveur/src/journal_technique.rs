//! Le journal technique du serveur — un fichier, une ligne par fait.
//!
//! Avant : `eprintln!` un peu partout, lisible seulement quand le
//! serveur tourne dans une console, et `serveur.log` ne recevait que
//! la sortie brute du service, sans heure ni niveau. Une caisse qui
//! affichait « erreur technique » a 11 h ne laissait aucune trace.
//!
//! Maintenant chaque refus, chaque erreur, chaque commande lente,
//! chaque demarrage et chaque sauvegarde font une ligne :
//!
//! ```text
//! 2026-09-20T11:02:14.318 [ERREUR] 192.168.1.12 POST /rpc · creer_vente · 3f2a…@CAISSE-1 · 500 · Base indisponible.
//! ```
//!
//! Le fichier tourne a 5 Mo (trois copies gardees) : il ne remplira
//! jamais le disque du serveur. Jamais de mot de passe ni de corps de
//! requete dedans (D10) : le message d'erreur, pas les donnees.
//!
//! Le contexte (adresse, route, commande, utilisateur) est pose par le
//! fil qui sert la requete — un fil par connexion — et prefixe chaque
//! ligne ecrite depuis ce fil.

use std::cell::RefCell;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use gescom_noyau::utils::maintenant_iso;

/// Au-dela, le fichier courant devient `.1`, `.1` devient `.2`, etc.
const TAILLE_MAX: u64 = 5 * 1024 * 1024;
const COPIES: u32 = 3;

struct Journal {
    chemin: Option<PathBuf>,
    /// Recopier aussi sur stderr — pour le serveur lance dans une
    /// console. Faux en service : stderr y est deja le fichier.
    console: bool,
}

static JOURNAL: Mutex<Journal> = Mutex::new(Journal { chemin: None, console: true });

thread_local! {
    static CONTEXTE: RefCell<String> = const { RefCell::new(String::new()) };
}

/// Ouvre le journal. `None` : seulement la console.
pub fn ouvrir(chemin: Option<PathBuf>, console: bool) {
    if let Some(c) = chemin.as_ref().and_then(|c| c.parent()) {
        let _ = std::fs::create_dir_all(c);
    }
    if let Ok(mut j) = JOURNAL.lock() {
        j.chemin = chemin;
        j.console = console;
    }
}

/// Le fichier en cours, s'il y en a un.
pub fn chemin() -> Option<PathBuf> {
    JOURNAL.lock().ok().and_then(|j| j.chemin.clone())
}

/// Le contexte des lignes ecrites depuis ce fil : « ip METHODE /route »,
/// puis « · commande · utilisateur@poste » quand on le sait.
pub fn contexte(texte: impl Into<String>) {
    let texte = texte.into();
    CONTEXTE.with(|c| *c.borrow_mut() = texte);
}

/// Complete le contexte du fil (la commande, une fois connue).
pub fn contexte_ajouter(texte: &str) {
    CONTEXTE.with(|c| {
        let mut c = c.borrow_mut();
        if !c.is_empty() {
            c.push_str(" · ");
        }
        c.push_str(texte);
    });
}

pub fn info(message: impl AsRef<str>) {
    ecrire("INFO", message.as_ref());
}

/// Un refus attendu : permission, jeton, commande inconnue, regle metier.
pub fn refus(message: impl AsRef<str>) {
    ecrire("REFUS", message.as_ref());
}

pub fn avertissement(message: impl AsRef<str>) {
    ecrire("AVERT", message.as_ref());
}

/// Une erreur remontee par une caisse (`POST /journal-poste`, v3 B-2).
pub fn poste(message: impl AsRef<str>) {
    ecrire("POSTE", message.as_ref());
}

/// Ce qui n'aurait pas du arriver : base indisponible, reponse coupee.
pub fn erreur(message: impl AsRef<str>) {
    ecrire("ERREUR", message.as_ref());
}

fn ecrire(niveau: &str, message: &str) {
    let ctx = CONTEXTE.with(|c| c.borrow().clone());
    let ligne = if ctx.is_empty() {
        format!("{} [{:<6}] {}\n", maintenant_iso(), niveau, message)
    } else {
        format!("{} [{:<6}] {} · {}\n", maintenant_iso(), niveau, ctx, message)
    };
    let Ok(j) = JOURNAL.lock() else { return };
    if j.console {
        eprint!("{ligne}");
    }
    if let Some(chemin) = j.chemin.as_ref() {
        tourner_si_plein(chemin);
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(chemin) {
            let _ = f.write_all(ligne.as_bytes());
        }
    }
}

/// Les niveaux, dans l'ordre du filtre de la console (B-3).
pub const NIVEAUX: &[&str] = &["ERREUR", "REFUS", "AVERT", "POSTE", "INFO"];

/// Les `n` dernieres lignes du journal, d'un niveau ou de tous. On lit
/// aussi la copie precedente quand le fichier courant vient de tourner
/// et n'en a pas assez.
pub fn dernieres_lignes(n: usize, niveau: Option<&str>) -> Result<Vec<String>, String> {
    if let Some(v) = niveau {
        if !NIVEAUX.contains(&v) {
            return Err(format!("Niveau inconnu : « {v} »."));
        }
    }
    let Some(chemin) = chemin() else { return Ok(Vec::new()) };
    let courant = std::fs::read_to_string(&chemin).unwrap_or_default();
    let mut lignes = filtrer(&courant, niveau, n);
    if lignes.len() < n {
        let tige = chemin.file_stem().and_then(|s| s.to_str()).unwrap_or("serveur");
        let ext = chemin.extension().and_then(|s| s.to_str()).unwrap_or("log");
        let avant = std::fs::read_to_string(chemin.with_file_name(format!("{tige}.1.{ext}"))).unwrap_or_default();
        let mut anciennes = filtrer(&avant, niveau, n - lignes.len());
        anciennes.append(&mut lignes);
        lignes = anciennes;
    }
    Ok(lignes)
}

/// Les `n` dernieres lignes d'un texte de journal, du niveau demande.
/// Le niveau se lit a sa place — `[NIVEAU` juste apres l'horodatage —
/// et pas n'importe ou : un message qui CONTIENT « [ERREUR] » n'est
/// pas une erreur.
pub fn filtrer(texte: &str, niveau: Option<&str>, n: usize) -> Vec<String> {
    let garde = |l: &&str| match niveau {
        None => !l.trim().is_empty(),
        Some(v) => l
            .split_once(' ')
            .map(|(_, reste)| reste.starts_with(&format!("[{v}")))
            .unwrap_or(false),
    };
    let toutes: Vec<&str> = texte.lines().filter(garde).collect();
    toutes[toutes.len().saturating_sub(n)..].iter().map(|s| s.to_string()).collect()
}

/// `serveur.log` → `serveur.1.log` → `serveur.2.log` → `serveur.3.log` → oubli.
fn tourner_si_plein(chemin: &Path) {
    let Ok(meta) = std::fs::metadata(chemin) else { return };
    if meta.len() < TAILLE_MAX {
        return;
    }
    let copie = |n: u32| -> PathBuf {
        let tige = chemin.file_stem().and_then(|s| s.to_str()).unwrap_or("serveur");
        let ext = chemin.extension().and_then(|s| s.to_str()).unwrap_or("log");
        chemin.with_file_name(format!("{tige}.{n}.{ext}"))
    };
    let _ = std::fs::remove_file(copie(COPIES));
    for n in (1..COPIES).rev() {
        let _ = std::fs::rename(copie(n), copie(n + 1));
    }
    let _ = std::fs::rename(chemin, copie(1));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_filtre_lit_le_niveau_a_sa_place() {
        let texte = "\
2026-09-23T10:00:00 [INFO  ] Serveur démarré
2026-09-23T10:01:00 [REFUS ] 1.2.3.4 POST /rpc · 401 · jeton
2026-09-23T10:02:00 [POSTE ] 1.2.3.4 POST /journal-poste · CAISSE · ventes · faux [ERREUR] dans le message
2026-09-23T10:03:00 [ERREUR] 1.2.3.4 POST /rpc · 500 · Base indisponible.
2026-09-23T10:04:00 [ERREUR] 1.2.3.4 POST /rpc · 500 · encore
";
        let e = filtrer(texte, Some("ERREUR"), 200);
        assert_eq!(e.len(), 2, "{e:?}");
        assert!(e[0].contains("Base indisponible"));
        assert_eq!(filtrer(texte, Some("ERREUR"), 1), vec![e[1].clone()], "les dernieres, pas les premieres");
        assert_eq!(filtrer(texte, None, 200).len(), 5);
        assert_eq!(filtrer(texte, Some("POSTE"), 200).len(), 1);
        assert!(filtrer("", None, 10).is_empty());
    }

    #[test]
    fn le_journal_tourne_quand_il_est_plein() {
        let dossier = std::env::temp_dir().join(format!("gescom-journal-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dossier).unwrap();
        let chemin = dossier.join("serveur.log");
        std::fs::write(&chemin, vec![b'x'; TAILLE_MAX as usize]).unwrap();
        tourner_si_plein(&chemin);
        assert!(!chemin.exists());
        assert!(dossier.join("serveur.1.log").exists());
        // Une seconde rotation decale la premiere copie.
        std::fs::write(&chemin, vec![b'y'; TAILLE_MAX as usize]).unwrap();
        tourner_si_plein(&chemin);
        assert!(dossier.join("serveur.2.log").exists());
        assert!(dossier.join("serveur.1.log").exists());
        let _ = std::fs::remove_dir_all(&dossier);
    }
}
