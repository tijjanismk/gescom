//! Commandes dashboard — KPIs, top clients, top articles, ventes du jour.

use tauri::State;
use chrono::{Datelike, Timelike};
use crate::commandes::ventes::EtatApp;

// =====================================================================
//  Résumé dashboard principal
// =====================================================================

#[tauri::command]
pub fn lire_resume_dashboard(
    etat: State<EtatApp>,
    // Depot actif. None ou vide = tous les depots (vue consolidee).
    depot_id: Option<String>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    // Le depot vient d'une liste fermee cote Rust (lire_depots), jamais
    // d'une saisie libre. On le passe malgre tout en PARAMETRE lie et
    // non par interpolation : une clause construite par format! est la
    // porte ouverte a l'injection le jour ou la source change.
    //
    // Astuce : `(?N IS NULL OR colonne = ?N)` laisse passer tout quand
    // le parametre est NULL. Une seule requete couvre les deux cas.
    let dep: Option<String> = match depot_id {
        Some(d) if !d.is_empty() => Some(d),
        _ => None,
    };

    let debut_jour = chrono::Local::now().format("%Y-%m-%dT00:00:00").to_string();
    let debut_semaine = {
        let now = chrono::Local::now();
        let lundi = now - chrono::Duration::days(now.weekday().num_days_from_monday() as i64);
        lundi.format("%Y-%m-%dT00:00:00").to_string()
    };
    let debut_mois = chrono::Local::now().format("%Y-%m-01T00:00:00").to_string();
    let debut_mois_prec = {
        let now = chrono::Local::now();
        let mois = now.month();
        let annee = now.year();
        if mois == 1 {
            format!("{}-12-01T00:00:00", annee - 1)
        } else {
            format!("{}-{:02}-01T00:00:00", annee, mois - 1)
        }
    };
    let fin_mois_prec = chrono::Local::now().format("%Y-%m-01T00:00:00").to_string();

    // CA jour
    let ca_jour: i64 = conn.query_row(
        "SELECT CAST(COALESCE(SUM(
            (SELECT COALESCE(SUM(prix_pratique * quantite), 0)
             FROM ligne_vente WHERE vente_id = v.id)
         ), 0) AS INTEGER)
         FROM vente v
         WHERE v.date_vente >= ?1 AND v.statut != 'annulee'
           AND (?2 IS NULL OR v.depot_id = ?2)",
        rusqlite::params![debut_jour, dep],
        |r| r.get(0),
    ).unwrap_or(0);

    let nb_ventes_jour: i64 = conn.query_row(
        "SELECT COUNT(*) FROM vente WHERE date_vente >= ?1 AND statut != 'annulee'
           AND (?2 IS NULL OR depot_id = ?2)",
        rusqlite::params![debut_jour, dep], |r| r.get(0),
    ).unwrap_or(0);

    // CA semaine
    let ca_semaine: i64 = conn.query_row(
        "SELECT CAST(COALESCE(SUM(
            (SELECT COALESCE(SUM(prix_pratique * quantite), 0)
             FROM ligne_vente WHERE vente_id = v.id)
         ), 0) AS INTEGER)
         FROM vente v WHERE v.date_vente >= ?1 AND v.statut != 'annulee'
           AND (?2 IS NULL OR v.depot_id = ?2)",
        rusqlite::params![debut_semaine, dep], |r| r.get(0),
    ).unwrap_or(0);

    // CA mois
    let (ca_mois, nb_ventes_mois): (i64, i64) = conn.query_row(
        "SELECT
            CAST(COALESCE(SUM(
                (SELECT COALESCE(SUM(prix_pratique * quantite), 0)
                 FROM ligne_vente WHERE vente_id = v.id)
            ), 0) AS INTEGER),
            COUNT(*)
         FROM vente v WHERE v.date_vente >= ?1 AND v.statut != 'annulee'
           AND (?2 IS NULL OR v.depot_id = ?2)",
        rusqlite::params![debut_mois, dep],
        |r| Ok((r.get(0)?, r.get(1)?)),
    ).unwrap_or((0, 0));

    // CA mois précédent
    let ca_mois_precedent: i64 = conn.query_row(
        "SELECT CAST(COALESCE(SUM(
            (SELECT COALESCE(SUM(prix_pratique * quantite), 0)
             FROM ligne_vente WHERE vente_id = v.id)
         ), 0) AS INTEGER)
         FROM vente v
         WHERE v.date_vente >= ?1 AND v.date_vente < ?2
           AND v.statut != 'annulee'
           AND (?3 IS NULL OR v.depot_id = ?3)",
        rusqlite::params![debut_mois_prec, fin_mois_prec, dep],
        |r| r.get(0),
    ).unwrap_or(0);

    // Créances
    let (total_creances, nb_creances_ouvertes): (i64, i64) = conn.query_row(
        "SELECT
            CAST(COALESCE(SUM(
                (SELECT COALESCE(SUM(prix_pratique * quantite), 0)
                 FROM ligne_vente WHERE vente_id = v.id) -
                (SELECT COALESCE(SUM(montant), 0)
                 FROM paiement WHERE vente_id = v.id)
            ), 0) AS INTEGER),
            COUNT(DISTINCT v.id)
         FROM vente v
         WHERE v.statut IN ('creance_ouverte','partiellement_payee')
           AND (?1 IS NULL OR v.depot_id = ?1)",
        rusqlite::params![dep],
        |r| Ok((r.get(0)?, r.get(1)?)),
    ).unwrap_or((0, 0));

    // Créances en retard (pièces avec échéance dépassée)
    let nb_creances_en_retard: i64 = conn.query_row(
        "SELECT COUNT(*) FROM piece_commerciale
         WHERE type_piece = 'facture'
           AND statut IN ('brouillon','emis')
           AND date_echeance IS NOT NULL
           AND date_echeance < date('now')",
        [], |r| r.get(0),
    ).unwrap_or(0);

    // Avoirs ouverts
    let total_avoirs_ouverts: i64 = conn.query_row(
        "SELECT CAST(COALESCE(SUM(montant), 0) AS INTEGER)
         FROM avoir WHERE statut = 'ouvert'",
        [], |r| r.get(0),
    ).unwrap_or(0);

    // Stock
    let stock_ruptures: i64 = conn.query_row(
        "SELECT COUNT(*) FROM stock_depot sd
         JOIN article a ON a.id = sd.article_id
         WHERE sd.quantite <= 0 AND a.actif = 1 AND a.gere_en_stock = 1
           AND (?1 IS NULL OR sd.depot_id = ?1)",
        rusqlite::params![dep], |r| r.get(0),
    ).unwrap_or(0);

    let stock_alertes: i64 = conn.query_row(
        "SELECT COUNT(*) FROM stock_depot sd
         JOIN article a ON a.id = sd.article_id
         WHERE sd.quantite > 0 AND sd.quantite < 5
           AND a.actif = 1 AND a.gere_en_stock = 1
           AND (?1 IS NULL OR sd.depot_id = ?1)",
        rusqlite::params![dep], |r| r.get(0),
    ).unwrap_or(0);

    // Caisse
    let (caisse_solde, caisse_ouverte): (i64, bool) = conn.query_row(
        "SELECT
            COALESCE(fond_ouverture, 0) +
            COALESCE((SELECT SUM(CASE WHEN sens='entree' THEN montant ELSE -montant END)
                      FROM mouvement_caisse WHERE session_id = sc.id), 0),
            sc.statut = 'ouverte'
         FROM session_caisse sc
         WHERE sc.statut = 'ouverte'
         ORDER BY sc.cree_le DESC LIMIT 1",
        [], |r| Ok((r.get(0)?, r.get::<_,bool>(1)?)),
    ).unwrap_or((0, false));

    // Pièces en attente
    let factures_brouillon: i64 = conn.query_row(
        "SELECT COUNT(*) FROM piece_commerciale
         WHERE type_piece = 'facture' AND statut = 'brouillon'",
        [], |r| r.get(0),
    ).unwrap_or(0);

    let commandes_en_attente: i64 = conn.query_row(
        "SELECT COUNT(*) FROM piece_commerciale
         WHERE type_piece = 'commande_client'
           AND statut NOT IN ('transfere','annule')",
        [], |r| r.get(0),
    ).unwrap_or(0);

    Ok(serde_json::json!({
        "ca_jour":               ca_jour,
        "ca_semaine":            ca_semaine,
        "ca_mois":               ca_mois,
        "ca_mois_precedent":     ca_mois_precedent,
        "nb_ventes_jour":        nb_ventes_jour,
        "nb_ventes_mois":        nb_ventes_mois,
        "total_creances":        total_creances,
        "nb_creances_ouvertes":  nb_creances_ouvertes,
        "nb_creances_en_retard": nb_creances_en_retard,
        "total_avoirs_ouverts":  total_avoirs_ouverts,
        "stock_ruptures":        stock_ruptures,
        "stock_alertes":         stock_alertes,
        "caisse_solde":          caisse_solde,
        "caisse_session_ouverte":caisse_ouverte,
        "factures_brouillon":    factures_brouillon,
        "commandes_en_attente":  commandes_en_attente,
    }))
}

// =====================================================================
//  Ventes du jour par heure
// =====================================================================

#[tauri::command]
pub fn lire_ventes_du_jour(
    etat: State<EtatApp>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let debut_jour = chrono::Local::now().format("%Y-%m-%dT00:00:00").to_string();

    // Générer les 24 heures
    let mut heures: Vec<serde_json::Value> = (0..24).map(|h| {
        serde_json::json!({"heure": h, "montant": 0_i64, "nb": 0_i64})
    }).collect();

    let mut stmt = conn.prepare(
        "SELECT
            CAST(strftime('%H', date_vente) AS INTEGER) as heure,
            CAST(COALESCE(SUM(
                (SELECT COALESCE(SUM(prix_pratique * quantite), 0)
                 FROM ligne_vente WHERE vente_id = v.id)
            ), 0) AS INTEGER) as total,
            COUNT(*) as nb
         FROM vente v
         WHERE v.date_vente >= ?1 AND v.statut != 'annulee'
         GROUP BY heure ORDER BY heure"
    ).map_err(|e| e.to_string())?;

    stmt.query_map(rusqlite::params![debut_jour], |row| {
        Ok((row.get::<_,i64>(0)?, row.get::<_,i64>(1)?, row.get::<_,i64>(2)?))
    }).map_err(|e| e.to_string())?
    .filter_map(|r| r.ok())
    .for_each(|(h, montant, nb)| {
        if h >= 0 && h < 24 {
            heures[h as usize] = serde_json::json!({
                "heure": h, "montant": montant, "nb": nb
            });
        }
    });

    // Retourner les heures de 6h à maintenant (ou toutes si avant 6h)
    let heure_actuelle = chrono::Local::now().hour() as usize;
    // Avant 6h on part de 0 : `heure.min(6)` donnait `heures[3..=3]`,
    // soit une seule barre au lieu de la matinee complete.
    let debut = if heure_actuelle < 6 { 0 } else { 6 };
    let fin = heure_actuelle.min(23);
    let heures_filtrees = if debut <= fin {
        heures[debut..=fin].to_vec()
    } else {
        heures.clone()
    };

    Ok(heures_filtrees)
}

// =====================================================================
//  Top clients du mois (patron seulement)
// =====================================================================

#[tauri::command]
pub fn lire_top_clients(
    etat: State<EtatApp>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let debut_mois = chrono::Local::now().format("%Y-%m-01T00:00:00").to_string();

    let mut stmt = conn.prepare(
        "SELECT c.nom, c.code,
                CAST(COALESCE(SUM(
                    (SELECT COALESCE(SUM(prix_pratique * quantite), 0)
                     FROM ligne_vente WHERE vente_id = v.id)
                ), 0) AS INTEGER) as ca,
                COUNT(v.id) as nb_ventes
         FROM vente v
         JOIN client c ON c.id = v.client_id
         WHERE v.date_vente >= ?1 AND v.statut != 'annulee'
           AND c.est_generique = 0
         GROUP BY c.id
         ORDER BY ca DESC
         LIMIT 10"
    ).map_err(|e| e.to_string())?;

    let x = stmt.query_map(rusqlite::params![debut_mois], |row| {
        Ok(serde_json::json!({
            "nom":       row.get::<_,String>(0)?,
            "code":      row.get::<_,String>(1)?,
            "ca":        row.get::<_,i64>(2)?,
            "nb_ventes": row.get::<_,i64>(3)?,
        }))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
    Ok(x)
}

// =====================================================================
//  Top articles du mois (patron seulement)
// =====================================================================

#[tauri::command]
pub fn lire_top_articles(
    etat: State<EtatApp>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let debut_mois = chrono::Local::now().format("%Y-%m-01T00:00:00").to_string();

    let mut stmt = conn.prepare(
        "SELECT a.nom, a.unite_base,
                CAST(COALESCE(SUM(lv.quantite * lv.prix_pratique), 0) AS INTEGER) as ca,
                COALESCE(SUM(lv.quantite), 0) as qte_vendue
         FROM ligne_vente lv
         JOIN article a ON a.id = lv.article_id
         JOIN vente v ON v.id = lv.vente_id
         WHERE v.date_vente >= ?1 AND v.statut != 'annulee'
         GROUP BY a.id
         ORDER BY ca DESC
         LIMIT 10"
    ).map_err(|e| e.to_string())?;

    let x = stmt.query_map(rusqlite::params![debut_mois], |row| {
        Ok(serde_json::json!({
            "nom":        row.get::<_,String>(0)?,
            "unite":      row.get::<_,String>(1)?,
            "ca":         row.get::<_,i64>(2)?,
            "qte_vendue": row.get::<_,f64>(3)?,
        }))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
    Ok(x)
}