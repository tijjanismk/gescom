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

/// Retourne toutes les données nécessaires pour générer une facture.
#[tauri::command]
pub fn lire_donnees_facture(
    etat: State<EtatApp>,
    vente_id: String,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    // Société
    let societe = conn.query_row(
        "SELECT nom, adresse, telephone, telephone2, email,
                nif, rccm, pied_facture, devise
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
                "pied_facture": row.get::<_, Option<String>>(7)?,
                "devise":       row.get::<_, String>(8)?,
            }))
        },
    ).map_err(|e| e.to_string())?;

    // Vente + client
    let vente = conn.query_row(
        "SELECT v.id, v.date_vente, v.statut, v.mode_reglement,
                c.nom, c.code, c.telephone, c.adresse, c.nif,
                f.numero, f.date_validation
         FROM vente v
         JOIN client c ON c.id = v.client_id
         LEFT JOIN facture f ON f.vente_id = v.id AND f.statut = 'validee'
         WHERE v.id = ?1",
        rusqlite::params![vente_id],
        |row| {
            Ok(serde_json::json!({
                "id":              row.get::<_, String>(0)?,
                "date_vente":      row.get::<_, String>(1)?,
                "statut":          row.get::<_, String>(2)?,
                "mode_reglement":  row.get::<_, String>(3)?,
                "client_nom":      row.get::<_, String>(4)?,
                "client_code":     row.get::<_, String>(5)?,
                "client_telephone":row.get::<_, Option<String>>(6)?,
                "client_adresse":  row.get::<_, Option<String>>(7)?,
                "client_nif":      row.get::<_, Option<String>>(8)?,
                "numero_facture":  row.get::<_, Option<String>>(9)?,
                "date_validation": row.get::<_, Option<String>>(10)?,
            }))
        },
    ).map_err(|e| e.to_string())?;

    // Lignes de vente
    let mut stmt = conn.prepare(
        "SELECT a.nom, u.libelle, lv.quantite,
                lv.prix_pratique, lv.prix_reference,
                CAST(lv.prix_pratique * lv.quantite AS INTEGER) as montant,
                COALESCE(lv.taux_tva, 0.0), COALESCE(lv.montant_tva, 0)
         FROM ligne_vente lv
         JOIN article a ON a.id = lv.article_id
         JOIN unite_vente u ON u.id = lv.unite_vente_id
         WHERE lv.vente_id = ?1
         ORDER BY a.nom"
    ).map_err(|e| e.to_string())?;

    let lignes: Vec<serde_json::Value> = {
        let x = stmt.query_map(rusqlite::params![vente_id], |row| {
            let quantite: f64  = row.get(2)?;
            let montant_ttc: i64 = row.get(5)?;   // prix_pratique est du TTC (D8)
            let montant_tva: i64 = row.get(7)?;
            let montant_ht = montant_ttc - montant_tva;
            // Prix unitaire HT, pour que P.U. x Qte = Montant HT sur la facture.
            let prix_unitaire_ht = if quantite > 0.0 {
                (montant_ht as f64 / quantite).round() as i64
            } else { montant_ht };

            Ok(serde_json::json!({
                "article_nom":   row.get::<_, String>(0)?,
                "unite_libelle": row.get::<_, String>(1)?,
                "quantite":      quantite,
                "prix_pratique": row.get::<_, i64>(3)?,   // TTC (compat.)
                "prix_reference":row.get::<_, i64>(4)?,
                "montant":       montant_ttc,             // TTC (compat.)
                "taux_tva":         row.get::<_, f64>(6)?,
                "montant_tva":      montant_tva,
                "montant_ht":       montant_ht,
                "prix_unitaire_ht": prix_unitaire_ht,
            }))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
        x
    };

    // Paiements
    let mut stmt_p = conn.prepare(
        "SELECT montant, mode, date_paiement FROM paiement
         WHERE vente_id = ?1 ORDER BY date_paiement"
    ).map_err(|e| e.to_string())?;

    let paiements: Vec<serde_json::Value> = {
        let x = stmt_p.query_map(rusqlite::params![vente_id], |row| {
            Ok(serde_json::json!({
                "montant":        row.get::<_, i64>(0)?,
                "mode":           row.get::<_, String>(1)?,
                "date_paiement":  row.get::<_, String>(2)?,
            }))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
        x
    };

    // total = TTC (somme des prix_pratique x quantite) — c'est le montant du.
    let total: i64 = lignes.iter()
        .filter_map(|l| l["montant"].as_i64())
        .sum();
    let total_tva: i64 = lignes.iter()
        .filter_map(|l| l["montant_tva"].as_i64())
        .sum();
    let total_ht = total - total_tva;
    let a_tva = total_tva > 0;

    let total_paye: i64 = paiements.iter()
        .filter_map(|p| p["montant"].as_i64())
        .sum();

    let reste = total - total_paye;

    Ok(serde_json::json!({
        "societe":    societe,
        "vente":      vente,
        "lignes":     lignes,
        "paiements":  paiements,
        "total":      total,
        "total_ht":   total_ht,
        "total_tva":  total_tva,
        "a_tva":      a_tva,
        "total_paye": total_paye,
        "reste":      reste,
    }))
}