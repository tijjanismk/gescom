//! Commandes Tauri pour les avoirs à la vente et le scanner de code-barres.

use tauri::State;
use crate::commandes::ventes::EtatApp;
use crate::utils::maintenant_iso;

// =====================================================================
//  AVOIRS CLIENT
// =====================================================================

/// Lire les avoirs ouverts d'un client spécifique.
#[tauri::command]
pub fn lire_avoirs_client(
    etat: State<EtatApp>,
    client_id: String,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let mut stmt = conn.prepare(
        "SELECT id, montant, cree_le
         FROM avoir
         WHERE client_id = ?1 AND statut = 'ouvert'
         ORDER BY cree_le ASC"
    ).map_err(|e| e.to_string())?;

    let x = stmt.query_map(rusqlite::params![client_id], |row| {
        Ok(serde_json::json!({
            "id":      row.get::<_, String>(0)?,
            "montant": row.get::<_, i64>(1)?,
            "cree_le": row.get::<_, String>(2)?,
        }))
    })
    .map_err(|e| e.to_string())?
    .filter_map(|r| r.ok())
    .collect();

    Ok(x)
}

/// Total des avoirs ouverts d'un client.
#[tauri::command]
pub fn total_avoirs_client(
    etat: State<EtatApp>,
    client_id: String,
) -> Result<i64, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let total: i64 = conn.query_row(
        "SELECT CAST(COALESCE(SUM(montant), 0) AS INTEGER)
         FROM avoir WHERE client_id = ?1 AND statut = 'ouvert'",
        rusqlite::params![client_id],
        |r| r.get(0),
    ).unwrap_or(0);

    Ok(total)
}

/// Appliquer un avoir à une vente.
/// Consomme les avoirs du plus ancien au plus récent.
/// Retourne le montant d'avoir effectivement appliqué.
#[tauri::command]
pub fn appliquer_avoir_vente(
    etat: State<EtatApp>,
    vente_id: String,
    client_id: String,
    montant_demande: i64,
) -> Result<i64, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let maintenant = maintenant_iso();
    let auteur_id = crate::commandes::ventes::id_utilisateur_courant_pub(&conn);

    // Lire les avoirs ouverts du client (du plus ancien au plus récent)
    let avoirs: Vec<(String, i64)> = {
        let mut stmt = conn.prepare(
            "SELECT id, montant FROM avoir
             WHERE client_id = ?1 AND statut = 'ouvert'
             ORDER BY cree_le ASC"
        ).map_err(|e| e.to_string())?;

        let x = stmt.query_map(rusqlite::params![client_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
        x
    };

    let mut reste_a_consommer = montant_demande;
    let mut total_applique: i64 = 0;

    for (avoir_id, montant_avoir) in avoirs {
        if reste_a_consommer <= 0 { break; }

        if montant_avoir <= reste_a_consommer {
            // Consommer l'avoir entièrement
            conn.execute(
                // 'consomme' — meme valeur que creer_vente. 'utilise'
                // etait un troisieme vocabulaire pour le meme etat.
                "UPDATE avoir SET statut = 'consomme', vente_utilisation_id = ?1
                 WHERE id = ?2",
                rusqlite::params![vente_id, avoir_id],
            ).map_err(|e| e.to_string())?;

            total_applique += montant_avoir;
            reste_a_consommer -= montant_avoir;
        } else {
            // Consommer partiellement — diviser l'avoir en deux
            let montant_utilise = reste_a_consommer;
            let montant_restant = montant_avoir - montant_utilise;

            // Marquer l'avoir original comme utilisé
            conn.execute(
                // 'consomme' — meme valeur que creer_vente. 'utilise'
                // etait un troisieme vocabulaire pour le meme etat.
                "UPDATE avoir SET statut = 'consomme', vente_utilisation_id = ?1
                 WHERE id = ?2",
                rusqlite::params![vente_id, avoir_id],
            ).map_err(|e| e.to_string())?;

            // Créer un nouvel avoir pour le solde restant
            conn.execute(
                "INSERT INTO avoir
                 (id, client_id, montant, statut, cree_le, origine)
                 VALUES (?1, ?2, ?3, 'ouvert', ?4, 'avoir_solde')",
                rusqlite::params![
                    uuid::Uuid::new_v4().to_string(),
                    client_id, montant_restant, maintenant
                ],
            ).map_err(|e| e.to_string())?;

            total_applique += montant_utilise;
            reste_a_consommer = 0;
        }
    }

    if total_applique > 0 {
        // Enregistrer le paiement par avoir
        conn.execute(
            "INSERT INTO paiement
             (id, vente_id, montant, mode, date_paiement, auteur_id, cree_le, cree_par, origine)
             VALUES (?1, ?2, ?3, 'avoir', ?4, ?5, ?6, ?7, 'app')",
            rusqlite::params![
                uuid::Uuid::new_v4().to_string(),
                vente_id, total_applique,
                maintenant, auteur_id, maintenant, auteur_id
            ],
        ).map_err(|e| e.to_string())?;

        // Mettre à jour le statut de la vente
        let total_vente: i64 = conn.query_row(
            "SELECT CAST(COALESCE(SUM(prix_pratique * quantite), 0) AS INTEGER)
             FROM ligne_vente WHERE vente_id = ?1",
            rusqlite::params![vente_id], |r| r.get(0),
        ).unwrap_or(0);

        let total_paye: i64 = conn.query_row(
            "SELECT CAST(COALESCE(SUM(montant), 0) AS INTEGER)
             FROM paiement WHERE vente_id = ?1",
            rusqlite::params![vente_id], |r| r.get(0),
        ).unwrap_or(0);

        let statut = if total_paye >= total_vente { "payee" }
            else if total_paye > 0 { "partiellement_payee" }
            else { "creance_ouverte" };

        conn.execute(
            "UPDATE vente SET statut = ?1, modifie_le = ?2 WHERE id = ?3",
            rusqlite::params![statut, maintenant, vente_id],
        ).map_err(|e| e.to_string())?;

        // Journal
        conn.execute(
            "INSERT INTO journal
             (id, type_evenement, entite_type, entite_id, auteur_id,
              nouveau_valeur, origine, date_evenement)
             VALUES (?1, 'avoir_applique', 'vente', ?2, ?3, ?4, 'app', ?5)",
            rusqlite::params![
                uuid::Uuid::new_v4().to_string(),
                vente_id, auteur_id,
                format!(r#"{{"montant_avoir":{}}}"#, total_applique),
                maintenant
            ],
        ).ok();
    }

    Ok(total_applique)
}

// =====================================================================
//  SCANNER CODE-BARRES
// =====================================================================

/// Chercher un article par son code-barres.
#[tauri::command]
pub fn chercher_article_par_code_barre(
    etat: State<EtatApp>,
    code_barre: String,
) -> Result<Option<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let depot_id: String = conn.query_row(
        "SELECT id FROM depot WHERE est_defaut = 1 AND actif = 1 LIMIT 1",
        [], |r| r.get(0),
    ).map_err(|e| e.to_string())?;

    let result = conn.query_row(
        "SELECT a.id, a.nom, a.unite_base,
                u.id, u.libelle, u.facteur, u.prix_reference,
                COALESCE(sd.quantite, 0) as stock
         FROM article a
         JOIN unite_vente u ON u.article_id = a.id AND u.actif = 1
         LEFT JOIN stock_depot sd ON sd.article_id = a.id AND sd.depot_id = ?2
         WHERE a.code_barre = ?1 AND a.actif = 1
         ORDER BY u.facteur ASC
         LIMIT 1",
        rusqlite::params![code_barre, depot_id],
        |row| {
            Ok(serde_json::json!({
                "id":         row.get::<_, String>(0)?,
                "nom":        row.get::<_, String>(1)?,
                "unite_base": row.get::<_, String>(2)?,
                "stock":      row.get::<_, f64>(7)?,
                "unites": [{
                    "id":             row.get::<_, String>(3)?,
                    "libelle":        row.get::<_, String>(4)?,
                    "facteur":        row.get::<_, f64>(5)?,
                    "prix_reference": row.get::<_, i64>(6)?,
                }]
            }))
        },
    );

    match result {
        Ok(article) => Ok(Some(article)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

/// Lire ou sauvegarder la config du scanner.
#[tauri::command]
pub fn lire_config_scanner(
    etat: State<EtatApp>,
) -> Result<bool, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS config_app (cle TEXT PRIMARY KEY, valeur TEXT NOT NULL)",
        [],
    ).ok();

    let actif: String = conn.query_row(
        "SELECT valeur FROM config_app WHERE cle = 'scanner_actif'",
        [], |r| r.get(0),
    ).unwrap_or_else(|_| "0".to_string());

    Ok(actif == "1")
}

#[tauri::command]
pub fn sauvegarder_config_scanner(
    etat: State<EtatApp>,
    actif: bool,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS config_app (cle TEXT PRIMARY KEY, valeur TEXT NOT NULL)",
        [],
    ).ok();

    conn.execute(
        "INSERT INTO config_app (cle, valeur) VALUES ('scanner_actif', ?1)
         ON CONFLICT(cle) DO UPDATE SET valeur = ?1",
        rusqlite::params![if actif { "1" } else { "0" }],
    ).map_err(|e| e.to_string())?;

    Ok(())
}

/// Enregistrer le code-barres d'un article depuis les paramètres.
#[tauri::command]
pub fn sauvegarder_code_barre_article(
    etat: State<EtatApp>,
    article_id: String,
    code_barre: String,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let maintenant = maintenant_iso();

    conn.execute(
        "UPDATE article SET code_barre = ?1, modifie_le = ?2 WHERE id = ?3",
        rusqlite::params![code_barre, maintenant, article_id],
    ).map_err(|e| e.to_string())?;

    Ok(())
}

/// Lire tous les articles avec leurs codes-barres pour la page Paramètres.
#[tauri::command]
pub fn lire_articles_avec_codes_barres(
    etat: State<EtatApp>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let mut stmt = conn.prepare(
        "SELECT id, nom, unite_base, code_barre
         FROM article WHERE actif = 1 ORDER BY nom ASC"
    ).map_err(|e| e.to_string())?;

    let x = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "id":         row.get::<_, String>(0)?,
            "nom":        row.get::<_, String>(1)?,
            "unite_base": row.get::<_, String>(2)?,
            "code_barre": row.get::<_, Option<String>>(3)?,
        }))
    })
    .map_err(|e| e.to_string())?
    .filter_map(|r| r.ok())
    .collect();
    Ok(x)
}
