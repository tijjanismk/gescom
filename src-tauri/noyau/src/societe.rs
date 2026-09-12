//! Commandes Tauri pour les paramètres de la société et l'impression des factures.

use crate::utils::maintenant_iso;

// =====================================================================
//  PARAMÈTRES SOCIÉTÉ
// =====================================================================

pub fn lire_parametres_societe(
    conn: &rusqlite::Connection,
) -> Result<serde_json::Value, String> {

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

pub fn sauvegarder_parametres_societe(
    conn: &rusqlite::Connection,
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

// =====================================================================
//  SUR L'UN OU L'AUTRE MOTEUR
// =====================================================================
//
// `parametres_societe` n'est pas cloisonnee : une installation, une
// societe qui imprime.

use crate::base::Base;
use crate::parametres;

pub fn lire_parametres_societe_sur_base(base: &mut Base) -> Result<serde_json::Value, String> {
    base.lire_une(
        "SELECT nom, adresse, telephone, telephone2, email,
                nif, rccm, site_web, pied_facture, devise
         FROM parametres_societe WHERE id = 1",
        &[],
        |r| {
            Ok(serde_json::json!({
                "nom":          r.get::<String>(0)?,
                "adresse":      r.get::<Option<String>>(1)?,
                "telephone":    r.get::<Option<String>>(2)?,
                "telephone2":   r.get::<Option<String>>(3)?,
                "email":        r.get::<Option<String>>(4)?,
                "nif":          r.get::<Option<String>>(5)?,
                "rccm":         r.get::<Option<String>>(6)?,
                "site_web":     r.get::<Option<String>>(7)?,
                "pied_facture": r.get::<Option<String>>(8)?,
                "devise":       r.get::<String>(9)?,
            }))
        },
    )
    .map_err(|e| e.0)?
    .ok_or_else(|| "Paramètres de la société introuvables".to_string())
}

#[allow(clippy::too_many_arguments)]
pub fn sauvegarder_parametres_societe_sur_base(
    base: &mut Base,
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
    // Les NULL sont types : sur PostgreSQL, `ON CONFLICT ... SET x = ?2`
    // ne sait pas deduire le type d'un parametre qui n'apparait qu'en
    // valeur inseree.
    base.executer(
        "INSERT INTO parametres_societe
         (id, nom, adresse, telephone, telephone2, email,
          nif, rccm, site_web, pied_facture, modifie_le)
         VALUES (1, ?1, CAST(?2 AS TEXT), CAST(?3 AS TEXT), CAST(?4 AS TEXT),
                 CAST(?5 AS TEXT), CAST(?6 AS TEXT), CAST(?7 AS TEXT),
                 CAST(?8 AS TEXT), CAST(?9 AS TEXT), ?10)
         ON CONFLICT (id) DO UPDATE SET
           nom = ?1, adresse = ?2, telephone = ?3, telephone2 = ?4,
           email = ?5, nif = ?6, rccm = ?7, site_web = ?8,
           pied_facture = ?9, modifie_le = ?10",
        &parametres![
            nom, adresse, telephone, telephone2, email, nif, rccm, site_web, pied_facture,
            maintenant_iso()
        ],
    )
    .map_err(|e| e.0)?;
    Ok(())
}
