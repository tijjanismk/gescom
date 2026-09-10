//! Facades Tauri de `auth`.
//!
//! La logique vit dans `gescom_noyau::auth` : le serveur v2
//! execute exactement le meme code que le comptoir.

use tauri::State;
use crate::commandes::ventes::EtatApp;

#[tauri::command]
pub fn connexion(
    etat: State<EtatApp>,
    identifiant: String,
    mot_de_passe: String,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::auth::connexion(&conn, identifiant, mot_de_passe)
}
#[tauri::command]
pub fn changer_mot_de_passe(
    etat: State<EtatApp>,
    utilisateur_id: String,
    ancien_mdp: String,
    nouveau_mdp: String,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::auth::changer_mot_de_passe(&conn, utilisateur_id, ancien_mdp, nouveau_mdp)
}
#[tauri::command]
pub fn creer_utilisateur(
    etat: State<EtatApp>,
    nom: String,
    pseudo: String,
    email: Option<String>,
    mot_de_passe: String,
    role_nom: String,
    auteur_id: String,
) -> Result<String, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::auth::creer_utilisateur(&conn, nom, pseudo, email, mot_de_passe, role_nom, auteur_id)
}
#[tauri::command]
pub fn lire_utilisateurs(
    etat: State<EtatApp>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::auth::lire_utilisateurs(&conn, )
}
#[allow(unused_imports)]
pub use gescom_noyau::auth::hasher_mot_de_passe_pub;
