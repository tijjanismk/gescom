//! Sauvegardes du serveur.
//!
//! En monoposte, la sauvegarde etait declenchee par l'application au
//! demarrage : si le commercant n'ouvrait pas Gescom, rien n'etait
//! sauvegarde. Le serveur, lui, tourne. C'est le bon endroit pour une
//! sauvegarde qui ne depend de personne.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crate::etat::Serveur;

/// Intervalle entre deux sauvegardes automatiques.
///
/// Vingt-quatre heures, la ou le monoposte attendait une semaine : le
/// serveur porte maintenant le travail de plusieurs caisses, et ce
/// qu'une journee perdue represente a ressaisir de memoire n'est plus
/// du meme ordre.
const HEURES_ENTRE_SAUVEGARDES: u64 = 24;

/// Nombre de sauvegardes conservees.
///
/// Une base corrompue peut l'etre depuis plusieurs jours sans que
/// personne ne s'en apercoive. Ne garder que la derniere reviendrait a
/// sauvegarder la corruption par-dessus la seule copie saine.
const COPIES_CONSERVEES: usize = 14;

pub fn dossier(srv: &Arc<Serveur>) -> PathBuf {
    let configure: Option<String> = srv.conn.as_ref().and_then(|m| m.lock().ok()).and_then(|c| {
        c.query_row(
            "SELECT valeur FROM config_app WHERE cle = 'dossier_sauvegarde'",
            [],
            |r| r.get::<_, String>(0),
        )
        .ok()
        .filter(|v| !v.trim().is_empty())
    });

    match configure {
        Some(d) => PathBuf::from(d),
        None => {
            let base = PathBuf::from(&srv.chemin_base);
            base.parent()
                .unwrap_or_else(|| std::path::Path::new("."))
                .join("sauvegardes")
        }
    }
}

/// Une sauvegarde, tout de suite.
pub fn maintenant(srv: &Arc<Serveur>) -> Result<String, String> {
    // `VACUUM INTO` est du SQLite — D4 (AI_CONTEXT/DECISIONS.md) prevoit
    // `pg_dump`, pas encore ecrit. Refuser clairement vaut mieux qu'un
    // fichier `.db` qui ne contiendrait qu'un schema vide.
    let Some(conn_mutex) = &srv.conn else {
        return Err(
            "La sauvegarde automatique n'est pas encore disponible sur PostgreSQL \
             (VACUUM INTO n'existe pas sur ce moteur — voir D4)."
                .to_string(),
        );
    };

    let dest_dir = dossier(srv);
    std::fs::create_dir_all(&dest_dir)
        .map_err(|e| format!("Impossible de créer le dossier de sauvegarde : {e}"))?;

    let horodatage = chrono::Local::now().format("%Y-%m-%d_%H-%M").to_string();
    let dest = dest_dir.join(format!("gescom_backup_{horodatage}.db"));
    let dest_texte = dest.to_string_lossy().to_string();

    {
        let conn = conn_mutex
            .lock()
            .map_err(|_| "Base indisponible.".to_string())?;
        // `VACUUM INTO` et non une copie de fichier : le contenu du WAL
        // est inclus. Une copie a la main donnerait une base amputee
        // des ecritures recentes — exactement les ventes du jour.
        //
        // Chemin en parametre lie, jamais interpole : une apostrophe
        // dans un nom d'utilisateur Windows casserait la requete.
        conn.execute("VACUUM INTO ?1", rusqlite::params![dest_texte])
            .map_err(|e| format!("Sauvegarde impossible : {e}"))?;
    }

    if let Ok(mut d) = srv.derniere_sauvegarde.lock() {
        *d = Some(gescom_noyau::utils::maintenant_iso());
    }
    elaguer(&dest_dir);
    Ok(dest_texte)
}

/// Supprime les sauvegardes au-dela des `COPIES_CONSERVEES` plus
/// recentes. Sans cela le dossier grossit jusqu'a remplir le disque du
/// poste serveur, ce qui arrete la boutique entiere.
fn elaguer(dossier: &PathBuf) {
    let Ok(entrees) = std::fs::read_dir(dossier) else {
        return;
    };
    let mut fichiers: Vec<_> = entrees
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with("gescom_backup_")
        })
        .collect();
    if fichiers.len() <= COPIES_CONSERVEES {
        return;
    }
    // Le nom porte l'horodatage en ISO : l'ordre alphabetique est
    // l'ordre chronologique, sans avoir a interroger le systeme de
    // fichiers dont les dates de modification mentent apres une copie.
    fichiers.sort_by_key(|e| e.file_name());
    let a_supprimer = fichiers.len() - COPIES_CONSERVEES;
    for e in fichiers.into_iter().take(a_supprimer) {
        std::fs::remove_file(e.path()).ok();
    }
}

/// Lance le fil de sauvegarde automatique.
pub fn planifier(srv: Arc<Serveur>) {
    std::thread::spawn(move || loop {
        // On dort d'abord : une sauvegarde au demarrage doublerait
        // celle de la veille sans rien apporter, et retarderait
        // l'ouverture du service un lundi matin.
        std::thread::sleep(Duration::from_secs(HEURES_ENTRE_SAUVEGARDES * 3600));
        match maintenant(&srv) {
            Ok(f) => eprintln!("[sauvegarde] {f}"),
            // Un echec de sauvegarde ne doit pas arreter le service :
            // les caisses continuent de vendre. Mais il doit se VOIR,
            // et `/sante` montrera une date qui n'avance plus.
            Err(e) => eprintln!("[sauvegarde] ECHEC : {e}"),
        }
    });
}
