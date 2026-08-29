//! Journal de la journée — reprend la structure du cahier tenu sous Excel.
//!
//! Sections :
//!   1. Détail des ventes du jour (date, client, article, PU, qté, montant)
//!   2. Hors du jour — encaissements sur des ventes antérieures
//!      (acomptes et soldes), qui ne sont PAS du chiffre d'affaires
//!      du jour mais bien de l'argent reçu aujourd'hui
//!   3. Situation non payée — reste dû par client
//!   4. Entrées / achats marchandises
//!   5. Retours marchandises (client et fournisseur)
//!   6. Dépenses, ventilées par poste
//!   7. Récapitulatif de caisse
//!
//! La distinction ventes du jour / encaissements du jour est le point
//! central : additionner les deux compterait deux fois une vente à
//! crédit encaissée plus tard.

use tauri::State;
use crate::commandes::ventes::EtatApp;

/// Date au format AAAA-MM-JJ. `None` = aujourd'hui.
fn jour(date: &Option<String>) -> String {
    match date {
        Some(d) if !d.is_empty() => d.clone(),
        _ => chrono::Local::now().format("%Y-%m-%d").to_string(),
    }
}

#[tauri::command]
pub fn lire_journal_du_jour(
    etat: State<EtatApp>,
    date: Option<String>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let j = jour(&date);

    // -----------------------------------------------------------------
    //  1. Ventes du jour, ligne par ligne
    // -----------------------------------------------------------------
    let mut st = conn.prepare(
        "SELECT v.date_vente, c.nom, a.nom, u.libelle,
                lv.prix_pratique, lv.quantite,
                CAST(lv.prix_pratique * lv.quantite AS INTEGER),
                COALESCE(lv.montant_tva, 0),
                v.mode_reglement, p.numero
         FROM ligne_vente lv
         JOIN vente v ON v.id = lv.vente_id
         JOIN client c ON c.id = v.client_id
         JOIN article a ON a.id = lv.article_id
         JOIN unite_vente u ON u.id = lv.unite_vente_id
         LEFT JOIN piece_commerciale p ON p.id = v.piece_id
         WHERE DATE(v.date_vente) = ?1 AND v.statut != 'annulee'
         ORDER BY v.date_vente"
    ).map_err(|e| e.to_string())?;

    let ventes: Vec<serde_json::Value> = st.query_map(
        rusqlite::params![j], |r| {
            let ttc: i64 = r.get(6)?;
            let tva: i64 = r.get(7)?;
            Ok(serde_json::json!({
                "date":        r.get::<_, String>(0)?,
                "client":      r.get::<_, String>(1)?,
                "description": r.get::<_, String>(2)?,
                "unite":       r.get::<_, String>(3)?,
                "prix_unitaire": r.get::<_, i64>(4)?,
                "quantite":    r.get::<_, f64>(5)?,
                "montant_ttc": ttc,
                "montant_ht":  ttc - tva,
                "mode":        r.get::<_, String>(8)?,
                "numero":      r.get::<_, Option<String>>(9)?,
            }))
        }
    ).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    let ca_jour: i64 = ventes.iter()
        .filter_map(|v| v["montant_ttc"].as_i64()).sum();
    let ca_jour_ht: i64 = ventes.iter()
        .filter_map(|v| v["montant_ht"].as_i64()).sum();

    // -----------------------------------------------------------------
    //  2. Hors du jour — encaissements sur ventes antérieures
    // -----------------------------------------------------------------
    let mut st = conn.prepare(
        "SELECT c.nom, p.montant, p.mode, p.date_paiement, v.date_vente,
                CAST(COALESCE((SELECT SUM(lv.prix_pratique * lv.quantite)
                   FROM ligne_vente lv WHERE lv.vente_id = v.id), 0) AS INTEGER),
                CAST(COALESCE((SELECT SUM(p2.montant)
                   FROM paiement p2 WHERE p2.vente_id = v.id), 0) AS INTEGER)
         FROM paiement p
         JOIN vente v ON v.id = p.vente_id
         JOIN client c ON c.id = v.client_id
         WHERE DATE(p.date_paiement) = ?1
           AND DATE(v.date_vente) <> ?1
         ORDER BY p.date_paiement"
    ).map_err(|e| e.to_string())?;

    let hors_jour: Vec<serde_json::Value> = st.query_map(
        rusqlite::params![j], |r| {
            let total: i64 = r.get(5)?;
            let paye: i64 = r.get(6)?;
            Ok(serde_json::json!({
                "client":     r.get::<_, String>(0)?,
                "montant":    r.get::<_, i64>(1)?,
                "mode":       r.get::<_, String>(2)?,
                "date":       r.get::<_, String>(3)?,
                "date_vente": r.get::<_, String>(4)?,
                // Solde si la vente est desormais entierement reglee,
                // acompte sinon — c'est la distinction du cahier.
                "type":       if paye >= total { "solde" } else { "acompte" },
            }))
        }
    ).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    let total_hors_jour: i64 = hors_jour.iter()
        .filter_map(|h| h["montant"].as_i64()).sum();

    // -----------------------------------------------------------------
    //  3. Situation non payée (toutes créances ouvertes)
    // -----------------------------------------------------------------
    let mut st = conn.prepare(
        "SELECT c.nom, v.date_vente,
                CAST(COALESCE((SELECT SUM(lv.prix_pratique * lv.quantite)
                   FROM ligne_vente lv WHERE lv.vente_id = v.id), 0) AS INTEGER),
                CAST(COALESCE((SELECT SUM(p.montant)
                   FROM paiement p WHERE p.vente_id = v.id), 0) AS INTEGER)
         FROM vente v
         JOIN client c ON c.id = v.client_id
         WHERE v.statut IN ('creance_ouverte','partiellement_payee')
           AND c.est_generique = 0
         ORDER BY v.date_vente"
    ).map_err(|e| e.to_string())?;

    let impayes: Vec<serde_json::Value> = st.query_map([], |r| {
        let total: i64 = r.get(2)?;
        let paye: i64 = r.get(3)?;
        Ok(serde_json::json!({
            "client":     r.get::<_, String>(0)?,
            "date_vente": r.get::<_, String>(1)?,
            "total":      total,
            "paye":       paye,
            "reste":      total - paye,
        }))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    let total_impayes: i64 = impayes.iter()
        .filter_map(|i| i["reste"].as_i64()).sum();

    // -----------------------------------------------------------------
    //  4. Entrées / achats marchandises
    // -----------------------------------------------------------------
    let mut st = conn.prepare(
        "SELECT COALESCE(f.nom, 'Sans fournisseur'), a.nom,
                ms.quantite_delta,
                COALESCE(ms.prix_achat_unitaire, a.dernier_prix_achat, 0),
                ms.date_mouvement
         FROM mouvement_stock ms
         JOIN article a ON a.id = ms.article_id
         LEFT JOIN fournisseur f ON f.id = ms.fournisseur_id
         WHERE ms.type_mouvement = 'achat'
           AND DATE(ms.date_mouvement) = ?1
         ORDER BY ms.date_mouvement"
    ).map_err(|e| e.to_string())?;

    let achats: Vec<serde_json::Value> = st.query_map(
        rusqlite::params![j], |r| {
            let qte: f64 = r.get(2)?;
            let pu: i64 = r.get(3)?;
            Ok(serde_json::json!({
                "fournisseur": r.get::<_, String>(0)?,
                "description": r.get::<_, String>(1)?,
                "quantite":    qte,
                "prix_unitaire": pu,
                "montant":     (pu as f64 * qte).round() as i64,
                "date":        r.get::<_, String>(4)?,
            }))
        }
    ).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    let total_achats: i64 = achats.iter()
        .filter_map(|a| a["montant"].as_i64()).sum();

    // -----------------------------------------------------------------
    //  5. Retours marchandises — client et fournisseur
    // -----------------------------------------------------------------
    let mut st = conn.prepare(
        "SELECT CASE WHEN ms.type_mouvement = 'retour' THEN 'client'
                     ELSE 'fournisseur' END,
                COALESCE(f.nom, c.nom, '—'), a.nom,
                ABS(ms.quantite_delta),
                COALESCE(ms.prix_achat_unitaire, a.dernier_prix_achat, 0),
                ms.date_mouvement
         FROM mouvement_stock ms
         JOIN article a ON a.id = ms.article_id
         LEFT JOIN fournisseur f ON f.id = ms.fournisseur_id
         LEFT JOIN retour ret ON ret.id = ms.operation_id
         LEFT JOIN vente v ON v.id = ret.vente_id
         LEFT JOIN client c ON c.id = v.client_id
         WHERE ms.type_mouvement IN ('retour','retour_fournisseur')
           AND DATE(ms.date_mouvement) = ?1
         ORDER BY ms.date_mouvement"
    ).map_err(|e| e.to_string())?;

    let retours: Vec<serde_json::Value> = st.query_map(
        rusqlite::params![j], |r| {
            let qte: f64 = r.get(3)?;
            let pu: i64 = r.get(4)?;
            Ok(serde_json::json!({
                "sens":        r.get::<_, String>(0)?,
                "tiers":       r.get::<_, String>(1)?,
                "description": r.get::<_, String>(2)?,
                "quantite":    qte,
                "prix_unitaire": pu,
                "montant":     (pu as f64 * qte).round() as i64,
                "date":        r.get::<_, String>(5)?,
            }))
        }
    ).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    // -----------------------------------------------------------------
    //  6. Dépenses du jour, ventilées
    // -----------------------------------------------------------------
    let mut st = conn.prepare(
        "SELECT COALESCE(libelle, '—'), COALESCE(categorie, 'autre'),
                moyen, montant, date_mouvement
         FROM mouvement_caisse
         WHERE motif = 'depense' AND DATE(date_mouvement) = ?1
         ORDER BY date_mouvement"
    ).map_err(|e| e.to_string())?;

    let depenses: Vec<serde_json::Value> = st.query_map(
        rusqlite::params![j], |r| {
            Ok(serde_json::json!({
                "libelle":   r.get::<_, String>(0)?,
                "categorie": r.get::<_, String>(1)?,
                "moyen":     r.get::<_, String>(2)?,
                "montant":   r.get::<_, i64>(3)?,
                "date":      r.get::<_, String>(4)?,
            }))
        }
    ).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    let total_depenses: i64 = depenses.iter()
        .filter_map(|d| d["montant"].as_i64()).sum();

    let mut par_cat: std::collections::BTreeMap<String, i64> =
        std::collections::BTreeMap::new();
    for d in &depenses {
        *par_cat.entry(d["categorie"].as_str().unwrap_or("autre").to_string())
            .or_insert(0) += d["montant"].as_i64().unwrap_or(0);
    }
    let depenses_par_categorie: Vec<serde_json::Value> = par_cat.into_iter()
        .map(|(c, m)| serde_json::json!({ "categorie": c, "montant": m }))
        .collect();

    // -----------------------------------------------------------------
    //  7. Règlements fournisseur du jour
    // -----------------------------------------------------------------
    let reglements_fournisseur: i64 = conn.query_row(
        "SELECT CAST(COALESCE(SUM(montant), 0) AS INTEGER)
         FROM paiement_fournisseur WHERE DATE(date_paiement) = ?1",
        rusqlite::params![j], |r| r.get(0),
    ).unwrap_or(0);

    // -----------------------------------------------------------------
    //  8. Caisse : encaissements du jour par moyen
    // -----------------------------------------------------------------
    let mut st = conn.prepare(
        "SELECT moyen,
                CAST(COALESCE(SUM(CASE WHEN sens='entree' THEN montant END),0) AS INTEGER),
                CAST(COALESCE(SUM(CASE WHEN sens='sortie' THEN montant END),0) AS INTEGER)
         FROM mouvement_caisse
         WHERE DATE(date_mouvement) = ?1
         GROUP BY moyen ORDER BY moyen"
    ).map_err(|e| e.to_string())?;

    let caisse_par_moyen: Vec<serde_json::Value> = st.query_map(
        rusqlite::params![j], |r| {
            Ok(serde_json::json!({
                "moyen":   r.get::<_, String>(0)?,
                "entrees": r.get::<_, i64>(1)?,
                "sorties": r.get::<_, i64>(2)?,
            }))
        }
    ).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    // Encaissements du jour, toutes ventes confondues.
    let encaisse_jour: i64 = conn.query_row(
        "SELECT CAST(COALESCE(SUM(montant), 0) AS INTEGER)
         FROM paiement WHERE DATE(date_paiement) = ?1 AND mode <> 'avoir'",
        rusqlite::params![j], |r| r.get(0),
    ).unwrap_or(0);

    Ok(serde_json::json!({
        "date": j,
        "ventes":     ventes,
        "hors_jour":  hors_jour,
        "impayes":    impayes,
        "achats":     achats,
        "retours":    retours,
        "depenses":   depenses,
        "depenses_par_categorie": depenses_par_categorie,
        "caisse_par_moyen": caisse_par_moyen,
        "totaux": {
            // CA du jour = ventes emises aujourd'hui, payees ou non.
            "ca_jour":        ca_jour,
            "ca_jour_ht":     ca_jour_ht,
            // Encaisse du jour = argent reellement recu aujourd'hui,
            // y compris sur des ventes anterieures. Les deux ne se
            // confondent pas et ne s'additionnent pas.
            "encaisse_jour":  encaisse_jour,
            "hors_jour":      total_hors_jour,
            "impayes":        total_impayes,
            "achats":         total_achats,
            "depenses":       total_depenses,
            "reglement_fournisseur": reglements_fournisseur,
            "nb_ventes":      ventes.len(),
        }
    }))
}
