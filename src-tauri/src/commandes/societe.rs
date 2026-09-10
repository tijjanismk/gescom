//! Facades Tauri de `societe`.
//!
//! La logique vit dans `gescom_noyau::societe` : le serveur v2
//! execute exactement le meme code que le comptoir.

use tauri::State;
use crate::commandes::ventes::EtatApp;

#[tauri::command]
pub fn lire_parametres_societe(
    etat: State<EtatApp>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::societe::lire_parametres_societe(&conn, )
}
#[tauri::command]
pub fn sauvegarder_parametres_societe(
    etat: State<EtatApp>,
    nom: String,
    adresse: Option<String>,
    telephone: Option<String>,
    telephone2: Option<String>,
    email: Option<String>,
    nif: Option<String>,
    rccm: Option<String>,
    site_web: Option<String>,
    pied_facture: Option<String>,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::societe::sauvegarder_parametres_societe(&conn, nom, adresse, telephone, telephone2, email, nif, rccm, site_web, pied_facture)
}
