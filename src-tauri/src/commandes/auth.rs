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

// =====================================================================
//  Roles et permissions
// =====================================================================

#[tauri::command]
pub fn lire_catalogue_permissions() -> serde_json::Value {
    gescom_noyau::roles::lire_catalogue_permissions()
}

#[tauri::command]
pub fn lire_roles(etat: State<EtatApp>) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::roles::lire_roles(&conn)
}

#[tauri::command]
pub fn creer_role(
    etat: State<EtatApp>,
    nom: String,
    description: Option<String>,
    permissions: Vec<String>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::roles::creer_role(&conn, nom, description, permissions)
}

#[tauri::command]
pub fn modifier_role(
    etat: State<EtatApp>,
    role_id: String,
    description: Option<String>,
    permissions: Vec<String>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::roles::modifier_role(&conn, role_id, description, permissions)
}

#[tauri::command]
pub fn supprimer_role(
    etat: State<EtatApp>,
    role_id: String,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::roles::supprimer_role(&conn, role_id)
}

#[tauri::command]
pub fn lire_permissions_utilisateur(
    etat: State<EtatApp>,
    utilisateur_id: String,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::roles::lire_permissions_utilisateur(&conn, utilisateur_id)
}

#[tauri::command]
pub fn definir_permission_utilisateur(
    etat: State<EtatApp>,
    utilisateur_id: String,
    permission: String,
    accorde: Option<bool>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::roles::definir_permission_utilisateur(
        &conn, utilisateur_id, permission, accorde, None,
    )
}
