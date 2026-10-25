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

// Une copie au plus par jour : le serveur copie des que la plus recente
// du dossier a vingt-quatre heures (la regle est dans le noyau).
//
// Vingt-quatre heures, la ou le monoposte attendait une semaine : le
// serveur porte maintenant le travail de plusieurs caisses, et ce
// qu'une journee perdue represente a ressaisir de memoire n'est plus
// du meme ordre.
use gescom_noyau::sauvegarde::HEURES_ENTRE_COPIES;

/// Nombre de sauvegardes conservees.
///
/// Une base corrompue peut l'etre depuis plusieurs jours sans que
/// personne ne s'en apercoive. Ne garder que la derniere reviendrait a
/// sauvegarder la corruption par-dessus la seule copie saine.
const COPIES_CONSERVEES: usize = 14;

/// Premier controle apres le demarrage : quelques minutes, le temps que
/// les caisses se connectent le matin sans attendre une copie.
/// `GESCOM_SAUVEGARDE_DELAI_S` le raccourcit (les tests de routes).
const PREMIER_CONTROLE_S: u64 = 5 * 60;

/// Ensuite, un coup d'oeil par heure : c'est la date de la derniere
/// copie qui decide, pas une minuterie de 24 h qu'une extinction remet
/// a zero.
const CONTROLE_S: u64 = 3600;

pub fn dossier(srv: &Arc<Serveur>) -> PathBuf {
    match srv.base.lock() {
        Ok(mut b) => gescom_noyau::sauvegarde::dossier_effectif_sur(&mut b),
        Err(_) => PathBuf::from("sauvegardes"),
    }
}

/// La date de la copie la plus recente du dossier, telle que `/sante`
/// la montre — lue au demarrage, pour qu'un redemarrage ne fasse pas
/// croire que rien n'a jamais ete copie.
pub fn derniere_copie(srv: &Arc<Serveur>) -> Option<String> {
    gescom_noyau::sauvegarde::copies_dans(&dossier(srv)).first().map(|c| c.date.clone())
}

/// Une sauvegarde, tout de suite.
///
/// SQLite : `VACUUM INTO`, ici, sur la connexion brute — le contenu du
/// WAL est inclus, une copie de fichier donnerait une base amputee des
/// ventes du jour — puis la copie est RELUE (integrite, table `vente`) ;
/// une copie abimee est effacee et l'echec dit. PostgreSQL : `pg_dump`,
/// par le noyau, avec l'URL que le serveur tient deja (D4, D10).
///
/// Le verrou de la base se prend UNE fois, pour le dossier et la copie :
/// le reprendre a l'interieur est l'interblocage du 13/09.
pub fn maintenant(srv: &Arc<Serveur>) -> Result<String, String> {
    let mut base = srv.base.lock().map_err(|_| "Base indisponible.".to_string())?;
    let dest_dir = gescom_noyau::sauvegarde::dossier_effectif_sur(&mut base);
    std::fs::create_dir_all(&dest_dir)
        .map_err(|e| format!("Impossible de créer le dossier de sauvegarde : {e}"))?;

    let dest_texte = match srv.est_postgres {
        false => {
            let dest = dest_dir.join(gescom_noyau::sauvegarde::nom_de_copie("db"));
            let dest_texte = dest.to_string_lossy().to_string();
            let conn = base.sqlite().ok_or_else(|| "Base SQLite attendue.".to_string())?;
            // Chemin en parametre lie, jamais interpole : une apostrophe
            // dans un nom d'utilisateur Windows casserait la requete.
            conn.execute("VACUUM INTO ?1", rusqlite::params![dest_texte.clone()])
                .map_err(|e| format!("Sauvegarde impossible : {e}"))?;
            if let Err(e) = gescom_noyau::sauvegarde::verifier_copie_sqlite(&dest) {
                let _ = std::fs::remove_file(&dest);
                return Err(format!("La copie écrite ne se relit pas, effacée : {e}"));
            }
            dest_texte
        }
        // Le verrou tient le temps de pg_dump : quelques secondes sur une
        // boutique, pendant lesquelles les caisses attendent.
        true => gescom_noyau::sauvegarde::sauvegarder_base_sur_base(
            &mut base,
            dest_dir.to_string_lossy().to_string(),
        )?,
    };
    drop(base);

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
    // Le nom porte l'horodatage : les copies sont rangees par date, sans
    // interroger le systeme de fichiers dont les dates de modification
    // mentent apres une copie.
    for c in gescom_noyau::sauvegarde::copies_dans(dossier).into_iter().skip(COPIES_CONSERVEES) {
        std::fs::remove_file(&c.chemin).ok();
    }
}

/// Copie si la derniere a vingt-quatre heures (ou s'il n'y en a pas).
fn copier_si_due(srv: &Arc<Serveur>) {
    let derniere = gescom_noyau::sauvegarde::copies_dans(&dossier(srv))
        .first()
        .and_then(|c| chrono::NaiveDateTime::parse_from_str(&c.date, "%Y-%m-%dT%H:%M:%S").ok());
    let maintenant_local = chrono::Local::now().naive_local();
    if !gescom_noyau::sauvegarde::sauvegarde_due(derniere, maintenant_local, HEURES_ENTRE_COPIES) {
        return;
    }
    match maintenant(srv) {
        Ok(f) => crate::journal_technique::info(format!("Sauvegarde automatique : {f}")),
        // Un echec de sauvegarde ne doit pas arreter le service : les
        // caisses continuent de vendre. Mais il doit se VOIR : le journal,
        // et `/sante` qui montre une date qui n'avance plus. On
        // reessaiera au prochain controle, dans une heure.
        Err(e) => crate::journal_technique::erreur(format!("Sauvegarde automatique ÉCHOUÉE : {e}")),
    }
}

/// Lance le fil de sauvegarde automatique.
///
/// Revue du 25/09 : ce fil dormait 24 h AVANT sa premiere copie. Le
/// poste d'une boutique s'eteint le soir et se rallume le matin : il ne
/// tenait jamais 24 h, et aucune copie automatique ne partait. Il
/// regarde maintenant peu apres le demarrage, puis chaque heure, si la
/// derniere copie du dossier a un jour — et copie si oui.
pub fn planifier(srv: Arc<Serveur>) {
    let premier = std::env::var("GESCOM_SAUVEGARDE_DELAI_S")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(PREMIER_CONTROLE_S);
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(premier));
        loop {
            copier_si_due(&srv);
            std::thread::sleep(Duration::from_secs(CONTROLE_S));
        }
    });
}
