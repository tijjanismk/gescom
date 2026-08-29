//! Commandes Tauri pour les paramètres de la société et l'impression des factures.

use tauri::State;
use crate::commandes::ventes::EtatApp;
use crate::utils::maintenant_iso;

// =====================================================================
//  PARAMÈTRES SOCIÉTÉ
// =====================================================================

#[tauri::command]
pub fn lire_parametres_societe(
    etat: State<EtatApp>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let result = conn.query_row(
        "SELECT nom, adresse, telephone, telephone2, email,
                nif, rccm, site_web, pied_facture, devise
         FROM parametres_societe WHERE id = 1",
        [],
        |row| {
            Ok(serde_json::json!({
                "nom":          row.get::<_, String>(0)?,
                "adresse":      row.get::<_, Option<String>>(1)?,
                "telephone":    row.get::<_, Option<String>>(2)?,
                "telephone2":   row.get::<_, Option<String>>(3)?,
                "email":        row.get::<_, Option<String>>(4)?,
                "nif":          row.get::<_, Option<String>>(5)?,
                "rccm":         row.get::<_, Option<String>>(6)?,
                "site_web":     row.get::<_, Option<String>>(7)?,
                "pied_facture": row.get::<_, Option<String>>(8)?,
                "devise":       row.get::<_, String>(9)?,
            }))
        },
    ).map_err(|e| e.to_string())?;

    Ok(result)
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
    let maintenant = maintenant_iso();

    conn.execute(
        "INSERT INTO parametres_societe
         (id, nom, adresse, telephone, telephone2, email,
          nif, rccm, site_web, pied_facture, modifie_le)
         VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
         ON CONFLICT(id) DO UPDATE SET
           nom = ?1, adresse = ?2, telephone = ?3, telephone2 = ?4,
           email = ?5, nif = ?6, rccm = ?7, site_web = ?8,
           pied_facture = ?9, modifie_le = ?10",
        rusqlite::params![
            nom, adresse, telephone, telephone2, email,
            nif, rccm, site_web, pied_facture, maintenant
        ],
    ).map_err(|e| e.to_string())?;

    Ok(())
}

// =====================================================================
//  DONNÉES DE FACTURE
// =====================================================================

// lire_donnees_facture a ete retiree : depuis la fusion des impressions,
// les deux ecrans lisent lire_donnees_piece. Plus aucun appelant.
