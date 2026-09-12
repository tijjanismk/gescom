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
    // Le reglage se lit par `Base` : il vaut sur les deux moteurs.
    let configure: Option<String> = srv.base.lock().ok().and_then(|mut b| {
        b.lire_une(
            "SELECT valeur FROM config_app WHERE cle = 'dossier_sauvegarde'",
            &[],
            |r| r.get::<String>(0),
        )
        .ok()
        .flatten()
        .filter(|v| !v.trim().is_empty())
    });

    match configure {
        Some(d) => PathBuf::from(d),
        // A cote du fichier SQLite ; a cote de l'executable quand la
        // cible est une URL, qui n'a pas de « a cote ».
        None if srv.conn.is_some() => {
            let base = PathBuf::from(&srv.chemin_base);
            base.parent()
                .unwrap_or_else(|| std::path::Path::new("."))
                .join("sauvegardes")
        }
        None => std::env::current_exe()
            .ok()
            .and_then(|e| e.parent().map(|d| d.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."))
            .join("sauvegardes"),
    }
}

/// Une sauvegarde, tout de suite.
///
/// SQLite : `VACUUM INTO`, ici, sur la connexion brute — le contenu du
/// WAL est inclus, une copie de fichier donnerait une base amputee des
/// ventes du jour. PostgreSQL : `pg_dump`, par le noyau, avec l'URL
/// que le serveur tient deja (D4, D10).
pub fn maintenant(srv: &Arc<Serveur>) -> Result<String, String> {
    let dest_dir = dossier(srv);
    std::fs::create_dir_all(&dest_dir)
        .map_err(|e| format!("Impossible de créer le dossier de sauvegarde : {e}"))?;

    let dest_texte = match &srv.conn {
        Some(conn_mutex) => {
            let horodatage = chrono::Local::now().format("%Y-%m-%d_%H-%M").to_string();
            let dest = dest_dir.join(format!("gescom_backup_{horodatage}.db"));
            let dest_texte = dest.to_string_lossy().to_string();
            let conn = conn_mutex.lock().map_err(|_| "Base indisponible.".to_string())?;
            // Chemin en parametre lie, jamais interpole : une apostrophe
            // dans un nom d'utilisateur Windows casserait la requete.
            conn.execute("VACUUM INTO ?1", rusqlite::params![dest_texte.clone()])
                .map_err(|e| format!("Sauvegarde impossible : {e}"))?;
            dest_texte
        }
        None => {
            // Le verrou tient le temps de pg_dump : quelques secondes
            // sur une boutique, pendant lesquelles les caisses
            // attendent. Acceptable une fois par jour, la nuit.
            let mut base = srv.base.lock().map_err(|_| "Base indisponible.".to_string())?;
            gescom_noyau::sauvegarde::sauvegarder_base_sur_base(
                &mut base,
                dest_dir.to_string_lossy().to_string(),
            )?
        }
    };

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
