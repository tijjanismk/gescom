//! Commandes Tauri pour les retours et avoirs.
//!
//! Trois modes (§6 module-facturation-stock.md) :
//!   - remboursement  : sortie de caisse, stock remonte
//!   - echange        : stock retourné remonte, stock remplacement descend,
//!                      reliquat → remboursement ou avoir si positif,
//!                      complément client si négatif
//!   - avoir_conserve : avoir créé, stock remonte, pas de caisse

use tauri::State;
use crate::commandes::ventes::EtatApp;
use crate::utils::maintenant_iso;

// =====================================================================
//  LIRE LES VENTES RÉCENTES
// =====================================================================

#[tauri::command]
pub fn lire_ventes_recentes(
    etat: State<EtatApp>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let mut stmt = conn.prepare(
        "SELECT v.id, v.date_vente, v.statut, v.client_id,
                c.nom, c.code,
                f.numero
         FROM vente v
         JOIN client c ON c.id = v.client_id
         LEFT JOIN facture f ON f.vente_id = v.id AND f.statut = 'validee'
         ORDER BY v.date_vente DESC
         LIMIT 50"
    ).map_err(|e| e.to_string())?;

    let ventes_base: Vec<(String, String, String, String, String, String, Option<String>)> = {
        let x = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, Option<String>>(6)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
        x
    };

    let mut ventes = Vec::new();

    for (id, date_vente, statut, client_id, client_nom, client_code, numero_facture) in ventes_base {
        let mut stmt_l = conn.prepare(
            "SELECT lv.id, lv.article_id, a.nom, lv.unite_vente_id,
                    u.libelle, lv.depot_source_id,
                    lv.quantite, lv.prix_pratique,
                    CAST(lv.prix_pratique * lv.quantite AS INTEGER) as montant
             FROM ligne_vente lv
             JOIN article a ON a.id = lv.article_id
             JOIN unite_vente u ON u.id = lv.unite_vente_id
             WHERE lv.vente_id = ?1"
        ).map_err(|e| e.to_string())?;

        let lignes: Vec<serde_json::Value> = {
            let x = stmt_l.query_map(rusqlite::params![id], |row| {
                Ok(serde_json::json!({
                    "id":              row.get::<_, String>(0)?,
                    "article_id":      row.get::<_, String>(1)?,
                    "article_nom":     row.get::<_, String>(2)?,
                    "unite_vente_id":  row.get::<_, String>(3)?,
                    "unite_libelle":   row.get::<_, String>(4)?,
                    "depot_source_id": row.get::<_, String>(5)?,
                    "quantite":        row.get::<_, f64>(6)?,
                    "prix_pratique":   row.get::<_, i64>(7)?,
                    "montant":         row.get::<_, i64>(8)?,
                }))
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();
            x
        };

        let total: i64 = lignes.iter().filter_map(|l| l["montant"].as_i64()).sum();

        ventes.push(serde_json::json!({
            "id":             id,
            "date_vente":     date_vente,
            "statut":         statut,
            "client_id":      client_id,
            "client_nom":     client_nom,
            "client_code":    client_code,
            "numero_facture": numero_facture,
            "total":          total,
            "lignes":         lignes,
        }));
    }

    Ok(ventes)
}

// =====================================================================
//  ENREGISTRER UN RETOUR
// =====================================================================

#[tauri::command]
pub fn enregistrer_retour(
    etat: State<EtatApp>,
    vente_id: String,
    ligne_vente_id: String,
    quantite: f64,
    mode_resolution: String,
    // Remboursement / complément
    mode_encaissement: Option<String>,
    // Échange
    article_remplacement_id: Option<String>,
    unite_remplacement_id: Option<String>,
    quantite_remplacement: Option<f64>,
    // Reliquat positif d'échange
    mode_reliquat_positif: Option<String>,    // "remboursement" / "avoir"
    mode_encaissement_reliquat: Option<String>,
) -> Result<String, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let utilisateur_id = crate::commandes::ventes::id_utilisateur_courant_pub(&conn);
    let maintenant = maintenant_iso();
    let retour_id = uuid::Uuid::new_v4().to_string();

    // Récupérer les infos de la ligne de vente.
    let (article_id, _unite_vente_id, depot_source_id, facteur, prix_pratique, client_id):
        (String, String, String, f64, i64, String) = conn.query_row(
        "SELECT lv.article_id, lv.unite_vente_id, lv.depot_source_id,
                u.facteur, lv.prix_pratique, v.client_id
         FROM ligne_vente lv
         JOIN unite_vente u ON u.id = lv.unite_vente_id
         JOIN vente v ON v.id = lv.vente_id
         WHERE lv.id = ?1",
        rusqlite::params![ligne_vente_id],
        |row| Ok((
            row.get(0)?, row.get(1)?, row.get(2)?,
            row.get(3)?, row.get(4)?, row.get(5)?,
        )),
    ).map_err(|e| e.to_string())?;

    let montant_credit: i64 = (prix_pratique as f64 * quantite).round() as i64;
    let quantite_base = quantite * facteur;

    // 1. Remonter le stock de l'article retourné — TOUJOURS.
    conn.execute(
        "UPDATE stock_depot SET quantite = quantite + ?1
         WHERE article_id = ?2 AND depot_id = ?3",
        rusqlite::params![quantite_base, article_id, depot_source_id],
    ).map_err(|e| e.to_string())?;

    // Mouvement de stock — entrée.
    conn.execute(
        "INSERT INTO mouvement_stock
         (id, article_id, depot_id, type_mouvement, quantite_delta,
          operation_id, auteur_id, date_mouvement, cree_le, cree_par, origine)
         VALUES (?1, ?2, ?3, 'retour', ?4, ?5, ?6, ?7, ?8, ?9, 'app')",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(),
            article_id, depot_source_id, quantite_base,
            retour_id, utilisateur_id,
            maintenant, maintenant, utilisateur_id
        ],
    ).map_err(|e| e.to_string())?;

    // 2. Insérer le retour.
    conn.execute(
        "INSERT INTO retour
         (id, vente_id, article_id, unite_vente_id, quantite,
          depot_reintegration_id, mode_resolution, montant_credit,
          reliquat, reliquat_resolution, auteur_id, date_retour,
          cree_le, cree_par, origine)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,0,?9,?10,?11,?12,?13,'app')",
        rusqlite::params![
            retour_id, vente_id, article_id, _unite_vente_id, quantite,
            depot_source_id, mode_resolution, montant_credit,
            mode_encaissement.as_deref().unwrap_or("aucun"),
            utilisateur_id, maintenant, maintenant, utilisateur_id
        ],
    ).map_err(|e| e.to_string())?;

    // 3. Selon le mode de résolution.
    match mode_resolution.as_str() {

        "remboursement" => {
            let mode = mode_encaissement.as_deref().unwrap_or("especes");
            enregistrer_sortie_caisse(&conn, montant_credit, mode, &retour_id,
                "remboursement", &utilisateur_id, &maintenant)?;
        }

        "avoir_conserve" => {
            creer_avoir_client(&conn, &client_id, &retour_id,
                montant_credit, &maintenant)?;
        }

        "echange" => {
            // Décrémenter le stock de l'article de remplacement.
            if let (Some(art_remp_id), Some(unite_remp_id), Some(qte_remp)) = (
                &article_remplacement_id,
                &unite_remplacement_id,
                quantite_remplacement,
            ) {
                // Récupérer le facteur de l'unité de remplacement.
                let (facteur_remp, prix_remp, depot_defaut): (f64, i64, String) =
                    conn.query_row(
                    "SELECT u.facteur, u.prix_reference, d.id
                     FROM unite_vente u
                     JOIN depot d ON d.est_defaut = 1
                     WHERE u.id = ?1",
                    rusqlite::params![unite_remp_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                ).map_err(|e| e.to_string())?;

                let qte_base_remp = qte_remp * facteur_remp;
                let montant_remplacement: i64 =
                    (prix_remp as f64 * qte_remp).round() as i64;
                let reliquat = montant_credit - montant_remplacement;

                // Décrémenter le stock de l'article de remplacement.
                conn.execute(
                    "UPDATE stock_depot SET quantite = quantite - ?1
                     WHERE article_id = ?2 AND depot_id = ?3",
                    rusqlite::params![qte_base_remp, art_remp_id, depot_defaut],
                ).map_err(|e| e.to_string())?;

                // Mouvement de stock — sortie remplacement.
                conn.execute(
                    "INSERT INTO mouvement_stock
                     (id, article_id, depot_id, type_mouvement, quantite_delta,
                      operation_id, auteur_id, date_mouvement, cree_le, cree_par, origine)
                     VALUES (?1, ?2, ?3, 'echange', ?4, ?5, ?6, ?7, ?8, ?9, 'app')",
                    rusqlite::params![
                        uuid::Uuid::new_v4().to_string(),
                        art_remp_id, depot_defaut, -qte_base_remp,
                        retour_id, utilisateur_id,
                        maintenant, maintenant, utilisateur_id
                    ],
                ).map_err(|e| e.to_string())?;

                // Gérer le reliquat.
                if reliquat > 0 {
                    // Client reçoit de l'argent ou un avoir.
                    match mode_reliquat_positif.as_deref() {
                        Some("remboursement") => {
                            let mode = mode_encaissement_reliquat
                                .as_deref().unwrap_or("especes");
                            enregistrer_sortie_caisse(&conn, reliquat, mode,
                                &retour_id, "remboursement_reliquat",
                                &utilisateur_id, &maintenant)?;
                        }
                        _ => {
                            // Avoir par défaut.
                            creer_avoir_client(&conn, &client_id, &retour_id,
                                reliquat, &maintenant)?;
                        }
                    }
                } else if reliquat < 0 {
                    // Client paie la différence — entrée de caisse.
                    let mode = mode_encaissement.as_deref().unwrap_or("especes");
                    let session_id: Option<String> = conn.query_row(
                        "SELECT id FROM session_caisse WHERE statut = 'ouverte' LIMIT 1",
                        [], |row| row.get(0),
                    ).ok();

                    if let Some(sid) = session_id {
                        conn.execute(
                            "INSERT INTO mouvement_caisse
                             (id, session_id, sens, moyen, montant, motif,
                              operation_id, date_mouvement, cree_le, cree_par, origine)
                             VALUES (?1, ?2, 'entree', ?3, ?4, 'complement_echange',
                                     ?5, ?6, ?7, ?8, 'app')",
                            rusqlite::params![
                                uuid::Uuid::new_v4().to_string(),
                                sid, mode, (-reliquat), retour_id,
                                maintenant, maintenant, utilisateur_id
                            ],
                        ).map_err(|e| e.to_string())?;
                    }
                }
                // reliquat == 0 → soldé exactement, rien à faire.
            }
        }

        _ => {}
    }

    // 4. Journal.
    conn.execute(
        "INSERT INTO journal
         (id, type_evenement, entite_type, entite_id, auteur_id,
          nouveau_valeur, origine, date_evenement)
         VALUES (?1, 'retour_enregistre', 'retour', ?2, ?3, ?4, 'app', ?5)",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(),
            retour_id, utilisateur_id,
            format!(r#"{{"mode":"{}","credit":{}}}"#, mode_resolution, montant_credit),
            maintenant
        ],
    ).map_err(|e| e.to_string())?;

    Ok(retour_id)
}

// =====================================================================
//  Fonctions utilitaires internes
// =====================================================================

fn enregistrer_sortie_caisse(
    conn: &rusqlite::Connection,
    montant: i64,
    mode: &str,
    operation_id: &str,
    motif: &str,
    utilisateur_id: &str,
    maintenant: &str,
) -> Result<(), String> {
    let session_id: Option<String> = conn.query_row(
        "SELECT id FROM session_caisse WHERE statut = 'ouverte' LIMIT 1",
        [], |row| row.get(0),
    ).ok();

    if let Some(sid) = session_id {
        conn.execute(
            "INSERT INTO mouvement_caisse
             (id, session_id, sens, moyen, montant, motif,
              operation_id, date_mouvement, cree_le, cree_par, origine)
             VALUES (?1, ?2, 'sortie', ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'app')",
            rusqlite::params![
                uuid::Uuid::new_v4().to_string(),
                sid, mode, montant, motif, operation_id,
                maintenant, maintenant, utilisateur_id
            ],
        ).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn creer_avoir_client(
    conn: &rusqlite::Connection,
    client_id: &str,
    retour_id: &str,
    montant: i64,
    maintenant: &str,
) -> Result<(), String> {
    conn.execute(
        "INSERT INTO avoir
         (id, client_id, retour_id, montant, statut, cree_le, origine)
         VALUES (?1, ?2, ?3, ?4, 'ouvert', ?5, 'app')",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(),
            client_id, retour_id, montant, maintenant
        ],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

// =====================================================================
//  AVOIRS OUVERTS
// =====================================================================

#[tauri::command]
pub fn lire_avoirs_ouverts_tous(
    etat: State<EtatApp>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let mut stmt = conn.prepare(
        "SELECT av.id, c.nom, c.code, av.montant, av.cree_le
         FROM avoir av
         JOIN client c ON c.id = av.client_id
         WHERE av.statut = 'ouvert'
         ORDER BY av.cree_le DESC"
    ).map_err(|e| e.to_string())?;

    let x = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "id":          row.get::<_, String>(0)?,
            "client_nom":  row.get::<_, String>(1)?,
            "client_code": row.get::<_, String>(2)?,
            "montant":     row.get::<_, i64>(3)?,
            "cree_le":     row.get::<_, String>(4)?,
        }))
    })
    .map_err(|e| e.to_string())?
    .filter_map(|r| r.ok())
    .collect();
    Ok(x)
}
