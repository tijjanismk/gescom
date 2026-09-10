//! Façade Tauri de la licence.

use tauri::{Manager, State};

use gescom_noyau::empreinte::empreinte_poste;
use gescom_noyau::licence::{self, EtatLicence};

use crate::commandes::ventes::EtatApp;

/// Le fichier de licence, à côté de la base.
///
/// Un fichier et non une colonne en base : une base restaurée depuis la
/// sauvegarde d'un autre poste emporterait sinon sa licence avec elle.
fn chemin_licence(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let dossier = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Répertoire de données introuvable : {e}"))?;
    std::fs::create_dir_all(&dossier).map_err(|e| e.to_string())?;
    Ok(dossier.join("gescom.licence"))
}

fn aujourdhui() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

/// L'état courant : licence installée, sinon essai.
#[tauri::command]
pub fn lire_etat_licence(
    app: tauri::AppHandle,
    etat: State<EtatApp>,
) -> Result<EtatLicence, String> {
    let empreinte = empreinte_poste();
    let jour = aujourdhui();

    if let Ok(chemin) = chemin_licence(&app) {
        if let Ok(texte) = std::fs::read_to_string(&chemin) {
            if !texte.trim().is_empty() {
                let e = licence::verifier(&texte, &empreinte, &jour);
                // Une licence expirée ou émise pour une autre machine
                // ne doit PAS retomber sur l'essai : le message perdrait
                // sa cause, et le commerçant appellerait pour une panne
                // qu'on saurait déjà nommer.
                return Ok(e);
            }
        }
    }

    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let debut = licence::debut_essai(&conn, &jour);
    Ok(licence::etat_essai(&debut, &jour, &empreinte))
}

#[tauri::command]
pub fn lire_empreinte_poste() -> String {
    empreinte_poste()
}

/// Installe une licence collée dans l'écran.
///
/// N'écrit QUE si elle est valable ici : enregistrer un jeton refusé
/// remplacerait une licence qui marche par une qui ne marche pas, et
/// il n'y aurait plus de retour en arrière.
#[tauri::command]
pub fn activer_licence(
    app: tauri::AppHandle,
    texte: String,
) -> Result<EtatLicence, String> {
    let empreinte = empreinte_poste();
    let etat = licence::verifier(&texte, &empreinte, &aujourdhui());

    if let EtatLicence::Valide { .. } = &etat {
        let chemin = chemin_licence(&app)?;
        std::fs::write(&chemin, texte.trim())
            .map_err(|e| format!("Enregistrement de la licence impossible : {e}"))?;
    }
    Ok(etat)
}

#[tauri::command]
pub fn importer_licence_fichier(
    app: tauri::AppHandle,
    chemin: String,
) -> Result<EtatLicence, String> {
    let texte = std::fs::read_to_string(&chemin)
        .map_err(|e| format!("Lecture impossible : {e}"))?;
    activer_licence(app, texte)
}
