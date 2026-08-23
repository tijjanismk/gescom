//! Commandes pour le règlement des créances existantes.

use tauri::State;
use crate::commandes::ventes::EtatApp;
use crate::utils::maintenant_iso;

/// Lire toutes les ventes avec créance ouverte ou partielle.
#[tauri::command]
pub fn lire_creances_ouvertes(
    etat: State<EtatApp>,
    recherche: Option<String>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let filtre = recherche.as_deref().unwrap_or("").to_lowercase();

    let mut stmt = conn.prepare(
        "SELECT v.id, v.date_vente, v.statut,
                c.id as client_id, c.nom, c.code, c.telephone,
                p.numero as numero_facture,
                CAST(COALESCE(SUM(lv.prix_pratique * lv.quantite), 0) AS INTEGER) as total,
                CAST(COALESCE(
                    (SELECT SUM(montant) FROM paiement WHERE vente_id = v.id), 0
                ) AS INTEGER) as total_paye
         FROM vente v
         JOIN client c ON c.id = v.client_id
         LEFT JOIN ligne_vente lv ON lv.vente_id = v.id
         LEFT JOIN piece_commerciale p ON p.id = v.piece_id
         WHERE v.statut IN ('creance_ouverte', 'partiellement_payee')
         GROUP BY v.id
         ORDER BY v.date_vente DESC"
    ).map_err(|e| e.to_string())?;

    let x = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,   // vente id
            row.get::<_, String>(1)?,   // date_vente
            row.get::<_, String>(2)?,   // statut
            row.get::<_, String>(3)?,   // client_id
            row.get::<_, String>(4)?,   // client nom
            row.get::<_, String>(5)?,   // client code
            row.get::<_, Option<String>>(6)?,  // telephone
            row.get::<_, Option<String>>(7)?,  // numero_facture
            row.get::<_, i64>(8)?,      // total
            row.get::<_, i64>(9)?,      // total_paye
        ))
    })
    .map_err(|e| e.to_string())?
    .filter_map(|r| r.ok())
    .filter(|(_, _, _, _, nom, _, _, _, _, _)| {
        filtre.is_empty() || nom.to_lowercase().contains(&filtre)
    })
    .map(|(id, date, statut, client_id, client_nom, client_code,
           telephone, numero_facture, total, total_paye)| {
        let reste = total - total_paye;
        serde_json::json!({
            "id":             id.clone(),
            "vente_id":       id,
            "date_vente":     date,
            "statut":         statut,
            "client_id":      client_id,
            "client_nom":     client_nom,
            "client_code":    client_code,
            "telephone":      telephone,
            "numero_facture": numero_facture,
            "total":          total,
            "total_paye":     total_paye,
            "reste":          reste,
        })
    })
    .collect();

    Ok(x)
}

/// Enregistrer un paiement sur une créance existante.
/// Met à jour le statut de la vente automatiquement.
#[tauri::command]
pub fn regler_creance(
    etat: State<EtatApp>,
    vente_id: String,
    montant: i64,
    mode: String,
    utilisateur_role: Option<String>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let role = utilisateur_role.as_deref().unwrap_or("patron");
    let auteur_id = crate::commandes::ventes::id_utilisateur_par_role(&conn, role);
    let maintenant = maintenant_iso();

    if montant <= 0 {
        return Err("Le montant doit être positif".to_string());
    }

    // Vérifier que la vente existe et a un reste dû
    let (total, total_paye_avant): (i64, i64) = conn.query_row(
        "SELECT
            CAST(COALESCE(SUM(lv.prix_pratique * lv.quantite), 0) AS INTEGER),
            CAST(COALESCE((SELECT SUM(montant) FROM paiement WHERE vente_id = v.id), 0) AS INTEGER)
         FROM vente v
         LEFT JOIN ligne_vente lv ON lv.vente_id = v.id
         WHERE v.id = ?1
         GROUP BY v.id",
        rusqlite::params![vente_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    ).map_err(|_| "Vente introuvable".to_string())?;

    let reste_avant = total - total_paye_avant;
    if reste_avant <= 0 {
        return Err("Cette vente est déjà entièrement payée".to_string());
    }

    // Limiter le paiement au reste dû (pas de surpayement)
    let montant_effectif = montant.min(reste_avant);

    // Insérer le paiement
    conn.execute(
        "INSERT INTO paiement
         (id, vente_id, montant, mode, date_paiement, auteur_id, cree_le, cree_par, origine)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'app')",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(),
            vente_id, montant_effectif, mode,
            maintenant, auteur_id, maintenant, auteur_id
        ],
    ).map_err(|e| e.to_string())?;

    // Calculer le nouveau total payé
    let total_paye_apres = total_paye_avant + montant_effectif;
    let nouveau_statut = if total_paye_apres >= total {
        "payee"
    } else {
        "partiellement_payee"
    };

    // Mettre à jour le statut de la vente
    conn.execute(
        "UPDATE vente SET statut = ?1, modifie_le = ?2 WHERE id = ?3",
        rusqlite::params![nouveau_statut, maintenant, vente_id],
    ).map_err(|e| e.to_string())?;

    // Alimenter la caisse si session ouverte
    if mode != "avoir" {
        let session_id: Option<String> = conn.query_row(
            "SELECT id FROM session_caisse WHERE statut = 'ouverte' LIMIT 1",
            [], |r| r.get(0),
        ).ok();

        if let Some(sid) = session_id {
            conn.execute(
                "INSERT INTO mouvement_caisse
                 (id, session_id, sens, moyen, montant, motif,
                  operation_id, date_mouvement, cree_le, cree_par, origine)
                 VALUES (?1, ?2, 'entree', ?3, ?4, 'vente', ?5, ?6, ?7, ?8, 'app')",
                rusqlite::params![
                    uuid::Uuid::new_v4().to_string(),
                    sid, mode, montant_effectif, vente_id,
                    maintenant, maintenant, auteur_id
                ],
            ).ok();
        }
    }

    // Journal
    conn.execute(
        "INSERT INTO journal
         (id, type_evenement, entite_type, entite_id, auteur_id,
          nouveau_valeur, origine, date_evenement)
         VALUES (?1, 'creance_reglee', 'vente', ?2, ?3, ?4, 'app', ?5)",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(),
            vente_id, auteur_id,
            format!(r#"{{"montant":{},"mode":"{}","statut":"{}"}}"#,
                montant_effectif, mode, nouveau_statut),
            maintenant
        ],
    ).ok();

    Ok(serde_json::json!({
        "montant_encaisse":  montant_effectif,
        "reste_apres":       total - total_paye_apres,
        "statut":            nouveau_statut,
        "soldee":            nouveau_statut == "payee",
    }))
}