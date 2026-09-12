//! Rapports — CA mensuel, top clients, top articles, export données.


// =====================================================================
//  CA mensuel sur N mois
// =====================================================================

pub fn lire_rapport_ca_mensuel(
    conn: &rusqlite::Connection,
    nb_mois: Option<i64>,
) -> Result<Vec<serde_json::Value>, String> {
    let mois = nb_mois.unwrap_or(12).min(24).max(1);

    let mut stmt = conn.prepare(
        "SELECT
            strftime('%Y-%m', date_vente) as mois,
            CAST(COALESCE(SUM(
              (SELECT COALESCE(SUM(prix_pratique * quantite), 0)
               FROM ligne_vente WHERE vente_id = v.id)
            ), 0) AS INTEGER) as ca,
            COUNT(*) as nb_ventes,
            CAST(COALESCE(SUM(
              (SELECT COALESCE(SUM(montant), 0) FROM paiement WHERE vente_id = v.id)
            ), 0) AS INTEGER) as encaisse
         FROM vente v
         WHERE v.statut != 'annulee'
           AND v.date_vente >= date('now', ?1)
         GROUP BY mois
         ORDER BY mois DESC"
    ).map_err(|e| e.to_string())?;

    let x = stmt.query_map(
        rusqlite::params![format!("-{} months", mois)],
        |row| Ok(serde_json::json!({
            "mois":      row.get::<_,String>(0)?,
            "ca":        row.get::<_,i64>(1)?,
            "nb_ventes": row.get::<_,i64>(2)?,
            "encaisse":  row.get::<_,i64>(3)?,
        }))
    ).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
    Ok(x)
}

// =====================================================================
//  Top clients sur une période
// =====================================================================

pub fn lire_rapport_top_clients(
    conn: &rusqlite::Connection,
    date_debut: String,
    date_fin: String,
    limite: Option<i64>,
) -> Result<Vec<serde_json::Value>, String> {
    let lim = limite.unwrap_or(20);

    let mut stmt = conn.prepare(
        "SELECT c.id, c.code, c.nom, c.telephone,
                CAST(COALESCE(SUM(
                  (SELECT COALESCE(SUM(prix_pratique * quantite), 0)
                   FROM ligne_vente WHERE vente_id = v.id)
                ), 0) AS INTEGER) as ca,
                COUNT(v.id) as nb_ventes,
                CAST(COALESCE(SUM(
                  (SELECT COALESCE(SUM(prix_pratique * quantite), 0)
                   FROM ligne_vente WHERE vente_id = v.id) -
                  (SELECT COALESCE(SUM(montant), 0)
                   FROM paiement WHERE vente_id = v.id)
                ), 0) AS INTEGER) as creances
         FROM vente v
         JOIN client c ON c.id = v.client_id
         WHERE v.date_vente BETWEEN ?1 AND ?2
           AND v.statut != 'annulee'
           AND c.est_generique = 0
         GROUP BY c.id
         ORDER BY ca DESC
         LIMIT ?3"
    ).map_err(|e| e.to_string())?;

    let x = stmt.query_map(
        rusqlite::params![date_debut, date_fin, lim],
        |row| Ok(serde_json::json!({
            "id":        row.get::<_,String>(0)?,
            "code":      row.get::<_,String>(1)?,
            "nom":       row.get::<_,String>(2)?,
            "telephone": row.get::<_,Option<String>>(3)?,
            "ca":        row.get::<_,i64>(4)?,
            "nb_ventes": row.get::<_,i64>(5)?,
            "creances":  row.get::<_,i64>(6)?,
        }))
    ).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
    Ok(x)
}

// =====================================================================
//  Top articles sur une période
// =====================================================================

pub fn lire_rapport_top_articles(
    conn: &rusqlite::Connection,
    date_debut: String,
    date_fin: String,
    limite: Option<i64>,
) -> Result<Vec<serde_json::Value>, String> {
    let lim = limite.unwrap_or(20);

    let mut stmt = conn.prepare(
        "SELECT a.id, a.nom, a.unite_base,
                CAST(COALESCE(SUM(lv.prix_pratique * lv.quantite), 0) AS INTEGER) as ca,
                COALESCE(SUM(lv.quantite), 0) as qte_vendue,
                COUNT(DISTINCT lv.vente_id) as nb_ventes,
                CAST(COALESCE(SUM(
                  (COALESCE(a.dernier_prix_achat, 0) * lv.quantite)
                ), 0) AS INTEGER) as cout_achat
         FROM ligne_vente lv
         JOIN article a ON a.id = lv.article_id
         JOIN vente v ON v.id = lv.vente_id
         WHERE v.date_vente BETWEEN ?1 AND ?2
           AND v.statut != 'annulee'
         GROUP BY a.id
         ORDER BY ca DESC
         LIMIT ?3"
    ).map_err(|e| e.to_string())?;

    let x = stmt.query_map(
        rusqlite::params![date_debut, date_fin, lim],
        |row| {
            let ca: i64 = row.get(3)?;
            let cout: i64 = row.get(6)?;
            Ok(serde_json::json!({
                "id":         row.get::<_,String>(0)?,
                "nom":        row.get::<_,String>(1)?,
                "unite":      row.get::<_,String>(2)?,
                "ca":         ca,
                "qte_vendue": row.get::<_,f64>(4)?,
                "nb_ventes":  row.get::<_,i64>(5)?,
                "marge":      ca - cout,
            }))
        }
    ).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
    Ok(x)
}

// =====================================================================
//  Rapport créances par ancienneté
// =====================================================================

pub fn lire_rapport_creances(
    conn: &rusqlite::Connection,
) -> Result<serde_json::Value, String> {

    // Tranches : <30j, 30-60j, 60-90j, >90j
    let tranches = vec![
        ("moins_30j", 0i64, 30i64),
        ("tranche_30_60", 30, 60),
        ("tranche_60_90", 60, 90),
        ("plus_90j", 90, 99999),
    ];

    let mut result = serde_json::json!({});

    for (cle, min_j, max_j) in &tranches {
        let (montant, nb): (i64, i64) = conn.query_row(
            &format!(
                "SELECT
                    CAST(COALESCE(SUM(
                      (SELECT COALESCE(SUM(prix_pratique * quantite), 0)
                       FROM ligne_vente WHERE vente_id = v.id) -
                      (SELECT COALESCE(SUM(montant), 0) FROM paiement WHERE vente_id = v.id)
                    ), 0) AS INTEGER),
                    COUNT(*)
                 FROM vente v
                 WHERE v.statut IN ('creance_ouverte','partiellement_payee')
                   AND CAST(julianday('now') - julianday(v.date_vente) AS INTEGER) >= {}
                   AND CAST(julianday('now') - julianday(v.date_vente) AS INTEGER) < {}",
                min_j, max_j
            ),
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        ).unwrap_or((0, 0));

        result[cle] = serde_json::json!({"montant": montant, "nb": nb});
    }

    Ok(result)
}

// =====================================================================
//  Rapport stock
// =====================================================================

pub fn lire_rapport_stock(
    conn: &rusqlite::Connection,
) -> Result<Vec<serde_json::Value>, String> {

    let mut stmt = conn.prepare(
        "SELECT a.nom, a.unite_base,
                COALESCE(sd.quantite, 0) as quantite,
                COALESCE(a.dernier_prix_achat, 0) as prix_achat,
                CAST(COALESCE(sd.quantite, 0) *
                     COALESCE(a.dernier_prix_achat, 0) AS INTEGER) as valeur_stock,
                d.nom as depot_nom
         FROM article a
         JOIN stock_depot sd ON sd.article_id = a.id
         JOIN depot d ON d.id = sd.depot_id
         WHERE a.actif = 1 AND a.gere_en_stock = 1
         ORDER BY valeur_stock DESC, a.nom"
    ).map_err(|e| e.to_string())?;

    let x = stmt.query_map([], |row| {
        let qte: f64 = row.get(2)?;
        Ok(serde_json::json!({
            "nom":          row.get::<_,String>(0)?,
            "unite":        row.get::<_,String>(1)?,
            "quantite":     qte,
            "prix_achat":   row.get::<_,i64>(3)?,
            "valeur_stock": row.get::<_,i64>(4)?,
            "depot":        row.get::<_,String>(5)?,
            "statut": if qte <= 0.0 { "rupture" }
                      else if qte < 5.0 { "alerte" }
                      else { "ok" },
        }))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
    Ok(x)
}

// =====================================================================
//  Rapport TVA collectée
// =====================================================================

pub fn lire_rapport_tva(
    conn: &rusqlite::Connection,
    date_debut: String,
    date_fin: String,
) -> Result<serde_json::Value, String> {

    let mut stmt = conn.prepare(
        "SELECT lv.taux_tva,
                CAST(SUM(lv.montant_tva) AS INTEGER) as total_tva,
                CAST(SUM(lv.prix_pratique * lv.quantite)
                     - SUM(lv.montant_tva) AS INTEGER) as total_ht,
                COUNT(DISTINCT v.id) as nb_ventes
         FROM ligne_vente lv
         JOIN vente v ON v.id = lv.vente_id
         WHERE v.date_vente BETWEEN ?1 AND ?2
           AND v.statut != 'annulee'
           AND lv.taux_tva > 0
         GROUP BY lv.taux_tva
         ORDER BY lv.taux_tva"
    ).map_err(|e| e.to_string())?;

    let par_taux: Vec<serde_json::Value> = stmt.query_map(
        rusqlite::params![date_debut, date_fin],
        |row| Ok(serde_json::json!({
            "taux":      row.get::<_,f64>(0)?,
            "total_tva": row.get::<_,i64>(1)?,
            "total_ht":  row.get::<_,i64>(2)?,
            "nb_ventes": row.get::<_,i64>(3)?,
        }))
    ).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    let total_tva: i64 = par_taux.iter()
        .filter_map(|v| v["total_tva"].as_i64()).sum();

    Ok(serde_json::json!({
        "par_taux":  par_taux,
        "total_tva": total_tva,
    }))
}
// =====================================================================
//  SUR L'UN OU L'AUTRE MOTEUR
// =====================================================================
//
// `strftime('%Y-%m', x)` devient `SUBSTR(x, 1, 7)` ; `date('now', '-N
// months')` se calcule ici ; les tranches d'anciennete des creances se
// rangent en Rust apres lecture (`utils::jours_depuis`).

use crate::base::Base;
use crate::parametres;

pub fn lire_rapport_ca_mensuel_sur_base(
    base: &mut Base,
    nb_mois: Option<i64>,
) -> Result<Vec<serde_json::Value>, String> {
    let dossier = base.dossier().to_string();
    let mois = nb_mois.unwrap_or(12).clamp(1, 24);
    let depuis = chrono::Local::now()
        .date_naive()
        .checked_sub_months(chrono::Months::new(mois as u32))
        .unwrap_or_else(|| chrono::Local::now().date_naive())
        .format("%Y-%m-%d")
        .to_string();

    base.lire_plusieurs(
        "SELECT SUBSTR(v.date_vente, 1, 7),
                CAST(COALESCE(SUM(
                  (SELECT COALESCE(SUM(prix_pratique * quantite), 0)
                   FROM ligne_vente WHERE vente_id = v.id)
                ), 0) AS BIGINT),
                COUNT(*),
                CAST(COALESCE(SUM(
                  (SELECT COALESCE(SUM(montant), 0) FROM paiement WHERE vente_id = v.id)
                ), 0) AS BIGINT)
         FROM vente v
         WHERE v.statut != 'annulee'
           AND SUBSTR(v.date_vente, 1, 10) >= ?1
           AND v.dossier_id = ?2
         GROUP BY SUBSTR(v.date_vente, 1, 7)
         ORDER BY SUBSTR(v.date_vente, 1, 7) DESC",
        &parametres![depuis, dossier],
        |r| {
            Ok(serde_json::json!({
                "mois":      r.get::<String>(0)?,
                "ca":        r.get::<i64>(1)?,
                "nb_ventes": r.get::<i64>(2)?,
                "encaisse":  r.get::<i64>(3)?,
            }))
        },
    )
    .map_err(|e| e.0)
}

pub fn lire_rapport_top_clients_sur_base(
    base: &mut Base,
    date_debut: String,
    date_fin: String,
    limite: Option<i64>,
) -> Result<Vec<serde_json::Value>, String> {
    let dossier = base.dossier().to_string();
    base.lire_plusieurs(
        "SELECT c.id, c.code, c.nom, c.telephone,
                CAST(COALESCE(SUM(
                  (SELECT COALESCE(SUM(prix_pratique * quantite), 0)
                   FROM ligne_vente WHERE vente_id = v.id)
                ), 0) AS BIGINT) AS ca,
                COUNT(v.id),
                CAST(COALESCE(SUM(
                  (SELECT COALESCE(SUM(prix_pratique * quantite), 0)
                   FROM ligne_vente WHERE vente_id = v.id) -
                  (SELECT COALESCE(SUM(montant), 0)
                   FROM paiement WHERE vente_id = v.id)
                ), 0) AS BIGINT)
         FROM vente v
         JOIN client c ON c.id = v.client_id
         WHERE v.date_vente BETWEEN ?1 AND ?2
           AND v.statut != 'annulee'
           AND c.est_generique = 0
           AND v.dossier_id = ?4
         GROUP BY c.id, c.code, c.nom, c.telephone
         ORDER BY ca DESC
         LIMIT ?3",
        &parametres![date_debut, date_fin, limite.unwrap_or(20), dossier],
        |r| {
            Ok(serde_json::json!({
                "id":        r.get::<String>(0)?,
                "code":      r.get::<String>(1)?,
                "nom":       r.get::<String>(2)?,
                "telephone": r.get::<Option<String>>(3)?,
                "ca":        r.get::<i64>(4)?,
                "nb_ventes": r.get::<i64>(5)?,
                "creances":  r.get::<i64>(6)?,
            }))
        },
    )
    .map_err(|e| e.0)
}

pub fn lire_rapport_top_articles_sur_base(
    base: &mut Base,
    date_debut: String,
    date_fin: String,
    limite: Option<i64>,
) -> Result<Vec<serde_json::Value>, String> {
    let dossier = base.dossier().to_string();
    base.lire_plusieurs(
        "SELECT a.id, a.nom, a.unite_base,
                CAST(COALESCE(SUM(lv.prix_pratique * lv.quantite), 0) AS BIGINT) AS ca,
                CAST(COALESCE(SUM(lv.quantite), 0) AS DOUBLE PRECISION),
                COUNT(DISTINCT lv.vente_id),
                CAST(COALESCE(SUM(COALESCE(a.dernier_prix_achat, 0) * lv.quantite), 0) AS BIGINT)
         FROM ligne_vente lv
         JOIN article a ON a.id = lv.article_id
         JOIN vente v ON v.id = lv.vente_id
         WHERE v.date_vente BETWEEN ?1 AND ?2
           AND v.statut != 'annulee'
           AND v.dossier_id = ?4
         GROUP BY a.id, a.nom, a.unite_base, a.dernier_prix_achat
         ORDER BY ca DESC
         LIMIT ?3",
        &parametres![date_debut, date_fin, limite.unwrap_or(20), dossier],
        |r| {
            let ca: i64 = r.get::<i64>(3)?;
            let cout: i64 = r.get::<i64>(6)?;
            Ok(serde_json::json!({
                "id":         r.get::<String>(0)?,
                "nom":        r.get::<String>(1)?,
                "unite":      r.get::<String>(2)?,
                "ca":         ca,
                "qte_vendue": r.get::<f64>(4)?,
                "nb_ventes":  r.get::<i64>(5)?,
                "marge":      ca - cout,
            }))
        },
    )
    .map_err(|e| e.0)
}

pub fn lire_rapport_creances_sur_base(base: &mut Base) -> Result<serde_json::Value, String> {
    let dossier = base.dossier().to_string();
    let creances: Vec<(String, i64)> = base
        .lire_plusieurs(
            "SELECT v.date_vente,
                    CAST(
                      (SELECT COALESCE(SUM(prix_pratique * quantite), 0)
                       FROM ligne_vente WHERE vente_id = v.id) -
                      (SELECT COALESCE(SUM(montant), 0) FROM paiement WHERE vente_id = v.id)
                    AS BIGINT)
             FROM vente v
             WHERE v.statut IN ('creance_ouverte','partiellement_payee')
               AND v.dossier_id = ?1",
            &parametres![dossier],
            |r| Ok((r.get::<String>(0)?, r.get::<i64>(1)?)),
        )
        .map_err(|e| e.0)?;

    // Tranches : <30j, 30-60j, 60-90j, >90j — rangees en Rust.
    let tranches = [("moins_30j", 0i64, 30i64), ("tranche_30_60", 30, 60), ("tranche_60_90", 60, 90), ("plus_90j", 90, i64::MAX)];
    let mut result = serde_json::json!({});
    for (cle, min_j, max_j) in tranches {
        let (montant, nb) = creances
            .iter()
            .filter(|(date, _)| {
                let j = crate::utils::jours_depuis(date);
                j >= min_j && j < max_j
            })
            .fold((0i64, 0i64), |(m, n), (_, reste)| (m + reste, n + 1));
        result[cle] = serde_json::json!({"montant": montant, "nb": nb});
    }
    Ok(result)
}

pub fn lire_rapport_stock_sur_base(base: &mut Base) -> Result<Vec<serde_json::Value>, String> {
    let dossier = base.dossier().to_string();
    base.lire_plusieurs(
        "SELECT a.nom, a.unite_base,
                COALESCE(sd.quantite, 0),
                COALESCE(a.dernier_prix_achat, 0),
                CAST(COALESCE(sd.quantite, 0) * COALESCE(a.dernier_prix_achat, 0) AS BIGINT) AS valeur_stock,
                d.nom
         FROM article a
         JOIN stock_depot sd ON sd.article_id = a.id
         JOIN depot d ON d.id = sd.depot_id
         WHERE a.actif = 1 AND a.gere_en_stock = 1 AND sd.dossier_id = ?1
         ORDER BY valeur_stock DESC, a.nom",
        &parametres![dossier],
        |r| {
            let qte: f64 = r.get::<f64>(2)?;
            Ok(serde_json::json!({
                "nom":          r.get::<String>(0)?,
                "unite":        r.get::<String>(1)?,
                "quantite":     qte,
                "prix_achat":   r.get::<i64>(3)?,
                "valeur_stock": r.get::<i64>(4)?,
                "depot":        r.get::<String>(5)?,
                "statut": if qte <= 0.0 { "rupture" } else if qte < 5.0 { "alerte" } else { "ok" },
            }))
        },
    )
    .map_err(|e| e.0)
}

pub fn lire_rapport_tva_sur_base(
    base: &mut Base,
    date_debut: String,
    date_fin: String,
) -> Result<serde_json::Value, String> {
    let dossier = base.dossier().to_string();
    let par_taux: Vec<serde_json::Value> = base
        .lire_plusieurs(
            "SELECT lv.taux_tva,
                    CAST(SUM(lv.montant_tva) AS BIGINT),
                    CAST(SUM(lv.prix_pratique * lv.quantite) - SUM(lv.montant_tva) AS BIGINT),
                    COUNT(DISTINCT v.id)
             FROM ligne_vente lv
             JOIN vente v ON v.id = lv.vente_id
             WHERE v.date_vente BETWEEN ?1 AND ?2
               AND v.statut != 'annulee'
               AND lv.taux_tva > 0
               AND v.dossier_id = ?3
             GROUP BY lv.taux_tva
             ORDER BY lv.taux_tva",
            &parametres![date_debut, date_fin, dossier],
            |r| {
                Ok(serde_json::json!({
                    "taux":      r.get::<f64>(0)?,
                    "total_tva": r.get::<i64>(1)?,
                    "total_ht":  r.get::<i64>(2)?,
                    "nb_ventes": r.get::<i64>(3)?,
                }))
            },
        )
        .map_err(|e| e.0)?;
    let total_tva: i64 = par_taux.iter().filter_map(|v| v["total_tva"].as_i64()).sum();
    Ok(serde_json::json!({ "par_taux": par_taux, "total_tva": total_tva }))
}
