//! Commandes Tauri paginées pour ventes, clients, stock, fournisseurs.
//!
//! Toutes retournent { donnees: [...], total: N, pages: P }
//! pour permettre la pagination côté frontend.

use tauri::State;
use crate::commandes::ventes::EtatApp;

// =====================================================================
//  Utilitaire : construire la clause WHERE et les params dynamiquement
// =====================================================================

fn periode_vers_dates(periode: &str) -> (Option<String>, Option<String>) {
    let auj = chrono::Local::now().format("%Y-%m-%d").to_string();
    match periode {
        "aujourd_hui" => (Some(auj.clone()), Some(auj)),
        "semaine" => {
            let lundi = chrono::Local::now()
                - chrono::Duration::days(chrono::Local::now().weekday()
                    .num_days_from_monday() as i64);
            (Some(lundi.format("%Y-%m-%d").to_string()), Some(auj))
        }
        "mois" => {
            let debut = chrono::Local::now()
                .with_day(1)
                .unwrap_or(chrono::Local::now());
            (Some(debut.format("%Y-%m-%d").to_string()), Some(auj))
        }
        _ => (None, None),
    }
}

use chrono::Datelike;

// =====================================================================
//  VENTES PAGINÉES
// =====================================================================

#[tauri::command]
pub fn lire_ventes_paginees(
    etat: State<EtatApp>,
    page: i64,
    limite: i64,
    recherche: Option<String>,
    statut: Option<String>,
    periode: Option<String>,
    date_debut: Option<String>,
    date_fin: Option<String>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    // Résoudre les dates selon la période.
    let (date_debut_calc, date_fin_calc) = match periode.as_deref() {
        Some(p) if p != "personnalise" && !p.is_empty() => periode_vers_dates(p),
        _ => (date_debut, date_fin),
    };

    // Construire les conditions WHERE dynamiquement.
    let mut conditions = vec!["v.id IS NOT NULL".to_string()];

    if let Some(ref s) = statut {
        if !s.is_empty() {
            conditions.push(format!("v.statut = '{}'", s.replace('\'', "''")));
        }
    }
    if let Some(ref d) = date_debut_calc {
        if !d.is_empty() {
            conditions.push(format!("DATE(v.date_vente) >= '{}'", d));
        }
    }
    if let Some(ref d) = date_fin_calc {
        if !d.is_empty() {
            conditions.push(format!("DATE(v.date_vente) <= '{}'", d));
        }
    }
    if let Some(ref r) = recherche {
        if !r.is_empty() {
            conditions.push(format!(
                "c.nom LIKE '%{}%'",
                r.replace('\'', "''")
            ));
        }
    }

    let where_clause = conditions.join(" AND ");

    // Compter le total.
    let total: i64 = conn.query_row(
        &format!(
            "SELECT COUNT(DISTINCT v.id)
             FROM vente v
             JOIN client c ON c.id = v.client_id
             WHERE {}", where_clause
        ),
        [],
        |row| row.get(0),
    ).unwrap_or(0);

    // Lire la page.
    let offset = page * limite;
    let sql = format!(
        "SELECT v.id, v.date_vente, v.statut, v.mode_reglement,
                c.nom as client_nom, c.code as client_code,
                CAST(COALESCE(SUM(lv.prix_pratique * lv.quantite), 0) AS INTEGER) as total,
                f.numero as numero_facture
         FROM vente v
         JOIN client c ON c.id = v.client_id
         LEFT JOIN ligne_vente lv ON lv.vente_id = v.id
         LEFT JOIN facture f ON f.vente_id = v.id AND f.statut = 'validee'
         WHERE {}
         GROUP BY v.id
         ORDER BY v.date_vente DESC
         LIMIT {} OFFSET {}",
        where_clause, limite, offset
    );

    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;

    let donnees: Vec<serde_json::Value> = {
        let x = stmt.query_map([], |row| {
            Ok(serde_json::json!({
                "id":             row.get::<_, String>(0)?,
                "date_vente":     row.get::<_, String>(1)?,
                "statut":         row.get::<_, String>(2)?,
                "mode_reglement": row.get::<_, String>(3)?,
                "client_nom":     row.get::<_, String>(4)?,
                "client_code":    row.get::<_, String>(5)?,
                "total":          row.get::<_, i64>(6)?,
                "numero_facture": row.get::<_, Option<String>>(7)?,
            }))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
        x
    };

    let pages = (total as f64 / limite as f64).ceil() as i64;

    Ok(serde_json::json!({
        "donnees": donnees,
        "total":   total,
        "pages":   pages,
        "page":    page,
    }))
}

// =====================================================================
//  CLIENTS PAGINÉS
// =====================================================================

#[tauri::command]
pub fn lire_clients_pagines(
    etat: State<EtatApp>,
    page: i64,
    limite: i64,
    recherche: Option<String>,
    avec_creances_seulement: bool,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let mut conditions = vec![
        "c.actif = 1".to_string(),
        "c.est_generique = 0".to_string(),
    ];

    if let Some(ref r) = recherche {
        if !r.is_empty() {
            conditions.push(format!(
                "(c.nom LIKE '%{r}%' OR c.telephone LIKE '%{r}%')",
                r = r.replace('\'', "''")
            ));
        }
    }

    if avec_creances_seulement {
        conditions.push("total_creances > 0".to_string());
    }

    let where_base = conditions[..2].join(" AND ");
    let _where_full = conditions.join(" AND ");

    let sql_count = format!(
        "SELECT COUNT(*) FROM (
           SELECT c.id,
             COALESCE(SUM(CASE WHEN v.statut != 'payee'
               THEN CAST(lv_sum.total AS INTEGER) - CAST(p_sum.paye AS INTEGER)
               ELSE 0 END), 0) as total_creances
           FROM client c
           LEFT JOIN vente v ON v.client_id = c.id
           LEFT JOIN (SELECT vente_id, SUM(prix_pratique * quantite) as total
                      FROM ligne_vente GROUP BY vente_id) lv_sum ON lv_sum.vente_id = v.id
           LEFT JOIN (SELECT vente_id, SUM(montant) as paye
                      FROM paiement GROUP BY vente_id) p_sum ON p_sum.vente_id = v.id
           WHERE {}
           GROUP BY c.id
         ) WHERE {}", where_base,
        if avec_creances_seulement { "total_creances > 0" } else { "1=1" }
    );

    let total: i64 = conn.query_row(&sql_count, [], |row| row.get(0)).unwrap_or(0);

    let offset = page * limite;
    let sql = format!(
        "SELECT c.id, c.code, c.nom, c.telephone,
                COALESCE(SUM(CASE WHEN v.statut != 'payee'
                  THEN CAST(lv_sum.total AS INTEGER) - CAST(p_sum.paye AS INTEGER)
                  ELSE 0 END), 0) as total_creances,
                COUNT(DISTINCT v.id) as nb_ventes
         FROM client c
         LEFT JOIN vente v ON v.client_id = c.id
         LEFT JOIN (SELECT vente_id, SUM(prix_pratique * quantite) as total
                    FROM ligne_vente GROUP BY vente_id) lv_sum ON lv_sum.vente_id = v.id
         LEFT JOIN (SELECT vente_id, SUM(montant) as paye
                    FROM paiement GROUP BY vente_id) p_sum ON p_sum.vente_id = v.id
         WHERE {}
         GROUP BY c.id
         HAVING {}
         ORDER BY total_creances DESC, c.nom ASC
         LIMIT {} OFFSET {}",
        where_base,
        if avec_creances_seulement { "total_creances > 0" } else { "1=1" },
        limite, offset
    );

    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;

    let donnees: Vec<serde_json::Value> = {
        let x = stmt.query_map([], |row| {
            Ok(serde_json::json!({
                "id":             row.get::<_, String>(0)?,
                "code":           row.get::<_, String>(1)?,
                "nom":            row.get::<_, String>(2)?,
                "telephone":      row.get::<_, Option<String>>(3)?,
                "total_creances": row.get::<_, i64>(4)?,
                "nb_ventes":      row.get::<_, i64>(5)?,
            }))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
        x
    };

    let pages = (total as f64 / limite as f64).ceil() as i64;

    Ok(serde_json::json!({
        "donnees": donnees,
        "total":   total,
        "pages":   pages,
        "page":    page,
    }))
}

// =====================================================================
//  STOCK PAGINÉ
// =====================================================================

#[tauri::command]
pub fn lire_stocks_pagines(
    etat: State<EtatApp>,
    page: i64,
    limite: i64,
    recherche: Option<String>,
    a_regulariser_seulement: bool,
    categorie_id: Option<String>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let mut conditions = vec!["a.actif = 1".to_string()];

    if let Some(ref r) = recherche {
        if !r.is_empty() {
            conditions.push(format!(
                "a.nom LIKE '%{}%'",
                r.replace('\'', "''")
            ));
        }
    }
    if let Some(ref cid) = categorie_id {
        if !cid.is_empty() {
            conditions.push(format!("a.categorie_id = '{}'", cid.replace('\'', "''")));
        }
    }
    if a_regulariser_seulement {
        conditions.push("sd.quantite < 0".to_string());
    }

    let where_clause = conditions.join(" AND ");

    let total: i64 = conn.query_row(
        &format!(
            "SELECT COUNT(*) FROM stock_depot sd
             JOIN article a ON a.id = sd.article_id
             WHERE {}", where_clause
        ),
        [],
        |row| row.get(0),
    ).unwrap_or(0);

    let offset = page * limite;
    let sql = format!(
        "SELECT a.id, a.nom, a.unite_base, d.nom, sd.quantite, sd.depot_id
         FROM stock_depot sd
         JOIN article a ON a.id = sd.article_id
         JOIN depot d ON d.id = sd.depot_id
         WHERE {}
         ORDER BY sd.quantite ASC, a.nom ASC
         LIMIT {} OFFSET {}",
        where_clause, limite, offset
    );

    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;

    let donnees: Vec<serde_json::Value> = {
        let x = stmt.query_map([], |row| {
            Ok(serde_json::json!({
                "article_id":  row.get::<_, String>(0)?,
                "article_nom": row.get::<_, String>(1)?,
                "unite_base":  row.get::<_, String>(2)?,
                "depot_nom":   row.get::<_, String>(3)?,
                "quantite":    row.get::<_, f64>(4)?,
                "depot_id":    row.get::<_, String>(5)?,
            }))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
        x
    };

    let pages = (total as f64 / limite as f64).ceil() as i64;

    Ok(serde_json::json!({
        "donnees": donnees,
        "total":   total,
        "pages":   pages,
        "page":    page,
    }))
}

// =====================================================================
//  FOURNISSEURS PAGINÉS
// =====================================================================

#[tauri::command]
pub fn lire_fournisseurs_pagines(
    etat: State<EtatApp>,
    page: i64,
    limite: i64,
    recherche: Option<String>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let mut conditions = vec![
        "f.actif = 1".to_string(),
        "f.est_voisin = 0".to_string(),
    ];

    if let Some(ref r) = recherche {
        if !r.is_empty() {
            conditions.push(format!(
                "(f.nom LIKE '%{r}%' OR f.telephone LIKE '%{r}%')",
                r = r.replace('\'', "''")
            ));
        }
    }

    let where_clause = conditions.join(" AND ");

    let total: i64 = conn.query_row(
        &format!("SELECT COUNT(*) FROM fournisseur f WHERE {}", where_clause),
        [],
        |row| row.get(0),
    ).unwrap_or(0);

    let offset = page * limite;
    let sql = format!(
        "SELECT f.id, f.nom, f.telephone, f.nif, f.adresse, f.est_voisin,
                CAST(COALESCE(
                  (SELECT COALESCE(SUM(
                     CASE pc.type_piece
                       WHEN 'facture_fournisseur' THEN lp.montant_ht + lp.montant_tva
                       -- Un AVF deja rembourse en especes ('paye') ne
                       -- reduit pas la dette : la caisse l'a deja fait.
                       WHEN 'avoir_fournisseur'   THEN
                            CASE WHEN pc.statut = 'paye' THEN 0
                                 ELSE -(lp.montant_ht + lp.montant_tva) END
                       ELSE 0 END), 0)
                   FROM piece_commerciale pc
                   JOIN ligne_piece lp ON lp.piece_id = pc.id
                   WHERE pc.tiers_type = 'fournisseur'
                     AND pc.tiers_id = f.id
                     AND pc.type_piece IN ('facture_fournisseur','avoir_fournisseur')
                     AND pc.statut <> 'annule'), 0
                ) AS INTEGER) as total_achats,
                CAST(COALESCE(
                  (SELECT SUM(pf.montant) FROM paiement_fournisseur pf
                   WHERE pf.fournisseur_id = f.id), 0
                ) AS INTEGER) as total_paye,
                (SELECT COUNT(*) FROM mouvement_stock ms
                 WHERE ms.type_mouvement = 'achat'
                   AND ms.fournisseur_id = f.id) as nb_achats
         FROM fournisseur f
         WHERE {}
         ORDER BY f.nom ASC
         LIMIT {} OFFSET {}",
        where_clause, limite, offset
    );

    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;

    let donnees: Vec<serde_json::Value> = {
        let x = stmt.query_map([], |row| {
            let total_achats: i64 = row.get(6)?;
            let total_paye:   i64 = row.get(7)?;
            let dette = (total_achats - total_paye).max(0);
            Ok(serde_json::json!({
                "id":        row.get::<_, String>(0)?,
                "nom":       row.get::<_, String>(1)?,
                "telephone": row.get::<_, Option<String>>(2)?,
                "nif":       row.get::<_, Option<String>>(3)?,
                "adresse":   row.get::<_, Option<String>>(4)?,
                "est_voisin":   row.get::<_, i64>(5)? != 0,
                "total_achats": total_achats,
                "total_paye":   total_paye,
                "dette":        dette,
                "nb_achats":    row.get::<_, i64>(8)?,
                // Conserve pour compatibilite avec d'eventuels appelants.
                "total_dettes": dette,
            }))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
        x
    };

    let pages = (total as f64 / limite as f64).ceil() as i64;

    Ok(serde_json::json!({
        "donnees": donnees,
        "total":   total,
        "pages":   pages,
        "page":    page,
    }))
}

// =====================================================================
//  RETOURS PAGINÉS
// =====================================================================

#[tauri::command]
pub fn lire_ventes_recentes_paginee(
    etat: State<EtatApp>,
    page: i64,
    limite: i64,
    recherche: Option<String>,
    periode: Option<String>,
    date_debut: Option<String>,
    date_fin: Option<String>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let (date_debut_calc, date_fin_calc) = match periode.as_deref() {
        Some(p) if p != "personnalise" && !p.is_empty() => periode_vers_dates(p),
        _ => (date_debut, date_fin),
    };

    let mut conditions = vec!["v.id IS NOT NULL".to_string()];

    if let Some(ref d) = date_debut_calc {
        if !d.is_empty() {
            conditions.push(format!("DATE(v.date_vente) >= '{}'", d));
        }
    }
    if let Some(ref d) = date_fin_calc {
        if !d.is_empty() {
            conditions.push(format!("DATE(v.date_vente) <= '{}'", d));
        }
    }
    if let Some(ref r) = recherche {
        if !r.is_empty() {
            conditions.push(format!("c.nom LIKE '%{}%'", r.replace('\'', "''")));
        }
    }

    let where_clause = conditions.join(" AND ");

    let total: i64 = conn.query_row(
        &format!(
            "SELECT COUNT(DISTINCT v.id)
             FROM vente v JOIN client c ON c.id = v.client_id
             WHERE {}", where_clause
        ),
        [],
        |row| row.get(0),
    ).unwrap_or(0);

    let offset = page * limite;
    let sql = format!(
        "SELECT v.id, v.date_vente, v.statut, v.client_id,
                c.nom, c.code, f.numero
         FROM vente v
         JOIN client c ON c.id = v.client_id
         LEFT JOIN facture f ON f.vente_id = v.id AND f.statut = 'validee'
         WHERE {}
         ORDER BY v.date_vente DESC
         LIMIT {} OFFSET {}",
        where_clause, limite, offset
    );

    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;

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

    let mut donnees = Vec::new();
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

        let total_vente: i64 = lignes.iter()
            .filter_map(|l| l["montant"].as_i64()).sum();

        donnees.push(serde_json::json!({
            "id":             id,
            "date_vente":     date_vente,
            "statut":         statut,
            "client_id":      client_id,
            "client_nom":     client_nom,
            "client_code":    client_code,
            "numero_facture": numero_facture,
            "total":          total_vente,
            "lignes":         lignes,
        }));
    }

    let pages = (total as f64 / limite as f64).ceil() as i64;

    Ok(serde_json::json!({
        "donnees": donnees,
        "total":   total,
        "pages":   pages,
        "page":    page,
    }))
}
