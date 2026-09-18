//! Façade Tauri des modèles de documents.
//!
//! La logique est dans `gescom_noyau::modeles` — le serveur v2 expose
//! exactement les mêmes fonctions, ce qui fait de ces commandes les
//! premières à exister des deux côtés sans être écrites deux fois.

use tauri::State;

use gescom_noyau::modeles::{self, Bilan, Lot, Modele};

use crate::commandes::ventes::{id_utilisateur_courant_pub, EtatApp};

#[tauri::command]
pub fn lire_modeles(
    etat: State<EtatApp>,
    genre: Option<String>,
) -> Result<Vec<Modele>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    modeles::lister(&conn, genre.as_deref()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn lire_modele(etat: State<EtatApp>, id: String) -> Result<Modele, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    modeles::lire(&conn, &id).map_err(|_| "Modèle introuvable.".to_string())
}

#[tauri::command]
pub fn lire_modele_actif(
    etat: State<EtatApp>,
    genre: String,
) -> Result<Option<Modele>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    Ok(modeles::lire_actif(&conn, &genre))
}

#[tauri::command]
pub fn enregistrer_modele(etat: State<EtatApp>, modele: Modele) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let auteur = id_utilisateur_courant_pub(&conn);
    modeles::enregistrer(&conn, &modele, &auteur)
}

#[tauri::command]
pub fn definir_modele_actif(etat: State<EtatApp>, id: String) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    modeles::definir_actif(&conn, &id)
}

#[tauri::command]
pub fn supprimer_modele(etat: State<EtatApp>, id: String) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    modeles::supprimer(&conn, &id)
}

// =====================================================================
//  Transport fichier
// =====================================================================

/// Écrit un lot de modèles dans un fichier.
///
/// Le chemin vient de l'écran, qui a ouvert la boîte de dialogue du
/// système. Rust ne choisit pas où écrire : il n'a aucun moyen de
/// savoir quelle clé USB le commerçant vient de brancher.
#[tauri::command]
pub fn exporter_modeles(
    etat: State<EtatApp>,
    ids: Option<Vec<String>>,
) -> Result<Lot, String> {
    // Le lot seulement : c'est l'ecran qui ecrit le fichier (plugin fs),
    // et sur une caisse c'est le serveur qui repond a cette commande.
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    modeles::exporter(&conn, ids)
}

#[tauri::command]
pub fn importer_modeles(
    app: tauri::AppHandle,
    etat: State<EtatApp>,
    lot: Lot,
) -> Result<Bilan, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let auteur = id_utilisateur_courant_pub(&conn);
    // Les images du lot se posent dans le dossier de donnees de l'app,
    // la ou vivent deja le logo et les autres images.
    use tauri::Manager;
    let dossier = app.path().app_data_dir().ok();
    modeles::importer_avec_images(&conn, &lot, &auteur, dossier.as_deref())
}
