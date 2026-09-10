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
    chemin: String,
    ids: Option<Vec<String>>,
) -> Result<String, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let lot = modeles::exporter(&conn, ids)?;
    // Indenté : le fichier doit rester lisible et comparable. Un
    // export minifié rend impossible de voir, dans un éditeur de
    // texte, ce qui a changé entre deux boutiques.
    let texte = serde_json::to_string_pretty(&lot).map_err(|e| e.to_string())?;
    std::fs::write(&chemin, texte)
        .map_err(|e| format!("Écriture impossible : {e}"))?;
    Ok(chemin)
}

#[tauri::command]
pub fn importer_modeles(etat: State<EtatApp>, chemin: String) -> Result<Bilan, String> {
    let texte = std::fs::read_to_string(&chemin)
        .map_err(|e| format!("Lecture impossible : {e}"))?;
    let lot: Lot = serde_json::from_str(&texte).map_err(|_| {
        "Ce fichier n'est pas un export de modèles Gescom lisible.".to_string()
    })?;
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let auteur = id_utilisateur_courant_pub(&conn);
    modeles::importer(&conn, &lot, &auteur)
}
