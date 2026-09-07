//! Commandes dashboard — KPIs, top clients, top articles, ventes du jour.

use tauri::State;
use chrono::Datelike;
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
    // Compte des ARTICLES, pas des lignes de stock : creer un depot
    // insere une ligne a 0 pour chaque article, ce qui doublait le
    // compteur de ruptures le jour de l'ouverture d'un magasin.
    // En vue consolidee, un article present ailleurs n'est pas en
    // rupture — c'est le total qui tranche.
    let stock_ruptures: i64 = conn.query_row(
        "SELECT COUNT(*) FROM (
           SELECT sd.article_id
           FROM stock_depot sd
           JOIN article a ON a.id = sd.article_id
           JOIN depot d ON d.id = sd.depot_id AND d.actif = 1
           WHERE a.actif = 1 AND a.gere_en_stock = 1
             AND (?1 IS NULL OR sd.depot_id = ?1)
           GROUP BY sd.article_id
           HAVING SUM(sd.quantite) <= 0
         )",
        rusqlite::params![dep], |r| r.get(0),
    ).unwrap_or(0);

    let stock_alertes: i64 = conn.query_row(
        "SELECT COUNT(*) FROM (
           SELECT sd.article_id
           FROM stock_depot sd
           JOIN article a ON a.id = sd.article_id
           JOIN depot d ON d.id = sd.depot_id AND d.actif = 1
           WHERE a.actif = 1 AND a.gere_en_stock = 1
             AND (?1 IS NULL OR sd.depot_id = ?1)
           GROUP BY sd.article_id
           HAVING SUM(sd.quantite) > 0 AND SUM(sd.quantite) < 5
         )",
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
//  Ventes par periode — jour (heures), semaine (jours),
//  mois (semaines), annee (mois)
// =====================================================================
//
// Une seule commande pour les quatre echelles : la difference tient au
// decoupage et au libelle, pas au calcul. Quatre commandes auraient fait
// quatre fois la meme somme, avec quatre occasions de diverger.

const JOURS_COURTS: [&str; 7] = ["Lun", "Mar", "Mer", "Jeu", "Ven", "Sam", "Dim"];
const MOIS_COURTS: [&str; 12] = [
    "Jan", "Fév", "Mar", "Avr", "Mai", "Juin",
    "Juil", "Août", "Sep", "Oct", "Nov", "Déc",
];

#[tauri::command]
pub fn lire_ventes_periode(
    etat: State<EtatApp>,
    // jour | semaine | mois | annee. Defaut : jour.
    periode: Option<String>,
    // Depot actif. Sans ce filtre, les barres additionnaient TOUS les
    // depots alors que le total affiche a cote vient de
    // `lire_resume_dashboard`, lui filtre : sur deux depots, les barres
    // depassaient visiblement le total annonce.
    depot_id: Option<String>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let dep: Option<String> = depot_id.filter(|d| !d.is_empty());
    let p = periode.as_deref().unwrap_or("jour");
    let maintenant = chrono::Local::now();

    // `cle` est l'expression SQL qui range une vente dans sa case, et
    // `debut`/`fin` bornent la periode. Le reste du code est commun.
    let (debut, fin, cle, nb_cases): (String, String, &str, usize) = match p {
        "semaine" => {
            let d = maintenant - chrono::Duration::days(6);
            (
                d.format("%Y-%m-%dT00:00:00").to_string(),
                maintenant.format("%Y-%m-%dT23:59:59").to_string(),
                // Jours ecoules depuis le debut de la fenetre : 0..6.
                "CAST(julianday(DATE(v.date_vente)) - julianday(DATE(?1)) AS INTEGER)",
                7,
            )
        }
        "mois" => {
            let d = maintenant.with_day(1).unwrap_or(maintenant);
            (
                d.format("%Y-%m-01T00:00:00").to_string(),
                maintenant.format("%Y-%m-%dT23:59:59").to_string(),
                // Semaine dans le mois : (jour - 1) / 7, donc 0..4.
                "(CAST(strftime('%d', v.date_vente) AS INTEGER) - 1) / 7",
                5,
            )
        }
        "annee" => (
            maintenant.format("%Y-01-01T00:00:00").to_string(),
            maintenant.format("%Y-12-31T23:59:59").to_string(),
            "CAST(strftime('%m', v.date_vente) AS INTEGER) - 1",
            12,
        ),
        // jour
        _ => (
            maintenant.format("%Y-%m-%dT00:00:00").to_string(),
            maintenant.format("%Y-%m-%dT23:59:59").to_string(),
            "CAST(strftime('%H', v.date_vente) AS INTEGER)",
            24,
        ),
    };

    let mut cases: Vec<(i64, i64)> = vec![(0, 0); nb_cases];

    let sql = format!(
        "SELECT {cle} AS case_idx,
                CAST(COALESCE(SUM(
                    (SELECT COALESCE(SUM(prix_pratique * quantite), 0)
                     FROM ligne_vente WHERE vente_id = v.id)
                ), 0) AS INTEGER) AS total,
                COUNT(*) AS nb
         FROM vente v
         WHERE v.date_vente >= ?1 AND v.date_vente <= ?2
           AND v.statut != 'annulee'
           AND (?3 IS NULL OR v.depot_id = ?3)
         GROUP BY case_idx"
    );

    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    stmt.query_map(rusqlite::params![debut, fin, dep], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)?))
    }).map_err(|e| e.to_string())?
    .filter_map(|r| r.ok())
    .for_each(|(i, montant, nb)| {
        if i >= 0 && (i as usize) < nb_cases {
            cases[i as usize] = (montant, nb);
        }
    });

    // Libelles, et bornes d'affichage.
    let mut points: Vec<serde_json::Value> = Vec::new();
    let (de, a) = match p {
        // Une boutique n'ouvre pas a minuit : on part de 6h, sauf si une
        // vente a eu lieu avant — auquel cas la masquer serait mentir.
        "jour" | _ if p == "jour" => {
            let tot = if cases[..6].iter().any(|(m, n)| *m != 0 || *n != 0) { 0 } else { 6 };
            (tot, 23)
        }
        _ => (0, nb_cases - 1),
    };

    for i in de..=a {
        let (montant, nb) = cases[i];
        let label = match p {
            "semaine" => {
                let d = maintenant - chrono::Duration::days((6 - i) as i64);
                JOURS_COURTS[d.weekday().num_days_from_monday() as usize].to_string()
            }
            "mois" => format!("S{}", i + 1),
            "annee" => MOIS_COURTS[i].to_string(),
            _ => format!("{}h", i),
        };
        points.push(serde_json::json!({
            "label": label, "montant": montant, "nb": nb,
        }));
    }

    let total: i64 = points.iter().filter_map(|p| p["montant"].as_i64()).sum();
    let nb_total: i64 = points.iter().filter_map(|p| p["nb"].as_i64()).sum();

    Ok(serde_json::json!({
        "points":   points,
        "total":    total,
        "nb":       nb_total,
        "periode":  p,
    }))
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