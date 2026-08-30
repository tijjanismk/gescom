//! Gestion des dépôts — scénario « un poste, plusieurs magasins ».
//!
//! Le patron gère tous ses points de vente depuis un seul ordinateur.
//! Chaque dépôt a son stock ; les ventes, achats et pièces portent le
//! dépôt sur lequel ils ont eu lieu.
//!
//! Ce n'est PAS du multi-poste : une seule installation, une seule base.
//! Le multi-poste (chaque magasin avec son ordinateur) demande l'API
//! réseau de la v2.

use tauri::State;
use crate::commandes::ventes::EtatApp;
use crate::utils::maintenant_iso;

/// Tous les dépôts, avec leur nombre d'articles et la valeur du stock.
#[tauri::command]
pub fn lire_depots_detail(
    etat: State<EtatApp>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let mut st = conn.prepare(
        "SELECT d.id, d.nom, d.est_defaut, d.actif,
                (SELECT COUNT(*) FROM stock_depot sd
                  JOIN article a ON a.id = sd.article_id
                  WHERE sd.depot_id = d.id AND sd.quantite > 0 AND a.actif = 1),
                CAST(COALESCE((SELECT SUM(sd.quantite * COALESCE(a.dernier_prix_achat,0))
                  FROM stock_depot sd JOIN article a ON a.id = sd.article_id
                  WHERE sd.depot_id = d.id AND sd.quantite > 0), 0) AS INTEGER),
                (SELECT COUNT(*) FROM vente v WHERE v.depot_id = d.id)
         FROM depot d
         ORDER BY d.est_defaut DESC, d.nom"
    ).map_err(|e| e.to_string())?;

    let x = st.query_map([], |r| {
        Ok(serde_json::json!({
            "id":            r.get::<_, String>(0)?,
            "nom":           r.get::<_, String>(1)?,
            "est_defaut":    r.get::<_, i64>(2)? != 0,
            "actif":         r.get::<_, i64>(3)? != 0,
            "nb_articles":   r.get::<_, i64>(4)?,
            "valeur_stock":  r.get::<_, i64>(5)?,
            "nb_ventes":     r.get::<_, i64>(6)?,
        }))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    Ok(x)
}

#[tauri::command]
pub fn creer_depot(
    etat: State<EtatApp>,
    nom: String,
    est_defaut: Option<bool>,
) -> Result<serde_json::Value, String> {
    if nom.trim().is_empty() {
        return Err("Le nom du dépôt est obligatoire".to_string());
    }

    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let now = maintenant_iso();
    let id = uuid::Uuid::new_v4().to_string();
    let defaut = est_defaut.unwrap_or(false);

    // Un seul dépôt par défaut : l'ancien perd son statut.
    if defaut {
        conn.execute("UPDATE depot SET est_defaut = 0", [])
            .map_err(|e| e.to_string())?;
    }

    conn.execute(
        "INSERT INTO depot (id, nom, est_defaut, actif, cree_le, modifie_le, origine)
         VALUES (?1, ?2, ?3, 1, ?4, ?5, 'app')",
        rusqlite::params![id, nom.trim(), defaut as i64, now, now],
    ).map_err(|e| e.to_string())?;

    // Initialiser le stock à zéro pour tous les articles actifs, sinon
    // le nouveau dépôt n'apparaît dans aucun état de stock tant qu'il
    // n'a rien reçu.
    conn.execute(
        "INSERT OR IGNORE INTO stock_depot (id, article_id, depot_id, quantite)
         SELECT lower(hex(randomblob(16))), a.id, ?1, 0
         FROM article a WHERE a.actif = 1",
        rusqlite::params![id],
    ).ok();

    Ok(serde_json::json!({ "id": id, "nom": nom.trim() }))
}

#[tauri::command]
pub fn renommer_depot(
    etat: State<EtatApp>,
    depot_id: String,
    nom: String,
) -> Result<(), String> {
    if nom.trim().is_empty() {
        return Err("Le nom du dépôt est obligatoire".to_string());
    }
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE depot SET nom = ?1, modifie_le = ?2 WHERE id = ?3",
        rusqlite::params![nom.trim(), maintenant_iso(), depot_id],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn definir_depot_defaut(
    etat: State<EtatApp>,
    depot_id: String,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    conn.execute("UPDATE depot SET est_defaut = 0", [])
        .map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE depot SET est_defaut = 1, modifie_le = ?1 WHERE id = ?2",
        rusqlite::params![maintenant_iso(), depot_id],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

/// Désactive un dépôt. Refusé s'il contient encore du stock : la
/// marchandise disparaîtrait des états sans jamais avoir été
/// transférée ni vendue.
#[tauri::command]
pub fn desactiver_depot(
    etat: State<EtatApp>,
    depot_id: String,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let est_defaut: i64 = conn.query_row(
        "SELECT est_defaut FROM depot WHERE id = ?1",
        rusqlite::params![depot_id], |r| r.get(0),
    ).map_err(|_| "Dépôt introuvable".to_string())?;

    if est_defaut != 0 {
        return Err(
            "Impossible de désactiver le dépôt par défaut. \
             Désigner un autre dépôt par défaut d'abord.".to_string()
        );
    }

    let reste: f64 = conn.query_row(
        "SELECT COALESCE(SUM(quantite), 0) FROM stock_depot
         WHERE depot_id = ?1 AND quantite > 0",
        rusqlite::params![depot_id], |r| r.get(0),
    ).unwrap_or(0.0);

    if reste > 0.0 {
        return Err(format!(
            "Ce dépôt contient encore {} unité(s) en stock. \
             Transférer la marchandise avant de le désactiver.",
            reste
        ));
    }

    conn.execute(
        "UPDATE depot SET actif = 0, est_defaut = 0, modifie_le = ?1 WHERE id = ?2",
        rusqlite::params![maintenant_iso(), depot_id],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

/// Stock d'un dépôt précis — l'écran Transferts en a besoin pour
/// afficher le stock de la SOURCE et non celui du dépôt par défaut.
#[tauri::command]
pub fn lire_stock_depot(
    etat: State<EtatApp>,
    depot_id: String,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let mut st = conn.prepare(
        "SELECT a.id, a.nom, a.unite_base, COALESCE(sd.quantite, 0)
         FROM article a
         LEFT JOIN stock_depot sd ON sd.article_id = a.id AND sd.depot_id = ?1
         WHERE a.actif = 1
         ORDER BY a.nom"
    ).map_err(|e| e.to_string())?;

    let x = st.query_map(rusqlite::params![depot_id], |r| {
        Ok(serde_json::json!({
            "article_id": r.get::<_, String>(0)?,
            "nom":        r.get::<_, String>(1)?,
            "unite_base": r.get::<_, String>(2)?,
            "quantite":   r.get::<_, f64>(3)?,
        }))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    Ok(x)
}

/// Comparatif entre magasins — la vue que le cahier Excel appelait
/// « créance magasin » et « acompte magasin », mais fondée sur les
/// ventes réelles plutôt que sur des transferts facturés.
#[tauri::command]
pub fn lire_resume_par_depot(
    etat: State<EtatApp>,
    date_debut: Option<String>,
    date_fin: Option<String>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let d1 = date_debut.unwrap_or_else(||
        chrono::Local::now().format("%Y-%m-01").to_string());
    let d2 = date_fin.unwrap_or_else(||
        chrono::Local::now().format("%Y-%m-%d").to_string());

    let mut st = conn.prepare(
        "SELECT d.id, d.nom,
                CAST(COALESCE(SUM(
                  (SELECT SUM(lv.prix_pratique * lv.quantite)
                   FROM ligne_vente lv WHERE lv.vente_id = v.id)), 0) AS INTEGER),
                COUNT(DISTINCT v.id),
                CAST(COALESCE(SUM(
                  (SELECT COALESCE(SUM(p.montant), 0)
                   FROM paiement p WHERE p.vente_id = v.id)), 0) AS INTEGER)
         FROM depot d
         LEFT JOIN vente v ON v.depot_id = d.id
              AND DATE(v.date_vente) BETWEEN ?1 AND ?2
              AND v.statut <> 'annulee'
         WHERE d.actif = 1
         GROUP BY d.id
         ORDER BY d.est_defaut DESC, d.nom"
    ).map_err(|e| e.to_string())?;

    let x = st.query_map(rusqlite::params![d1, d2], |r| {
        let ca: i64 = r.get(2)?;
        let paye: i64 = r.get(4)?;
        Ok(serde_json::json!({
            "depot_id":  r.get::<_, String>(0)?,
            "nom":       r.get::<_, String>(1)?,
            "ca":        ca,
            "nb_ventes": r.get::<_, i64>(3)?,
            "encaisse":  paye,
            "impaye":    (ca - paye).max(0),
        }))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    Ok(x)
}

/// Stock d'un article dans TOUS les dépôts.
///
/// Le vendeur doit voir ce qu'il a sous la main et ce qui dort à côté.
/// Sans cette vue, il vend à l'aveugle : soit il refuse une vente qu'il
/// pouvait honorer, soit il promet une quantité qu'il n'a pas.
#[tauri::command]
pub fn lire_stock_article_depots(
    etat: State<EtatApp>,
    article_id: String,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let mut st = conn.prepare(
        "SELECT d.id, d.nom, d.est_defaut, COALESCE(sd.quantite, 0)
         FROM depot d
         LEFT JOIN stock_depot sd
                ON sd.depot_id = d.id AND sd.article_id = ?1
         WHERE d.actif = 1
         ORDER BY d.est_defaut DESC, d.nom"
    ).map_err(|e| e.to_string())?;

    let x = st.query_map(rusqlite::params![article_id], |r| {
        Ok(serde_json::json!({
            "depot_id":   r.get::<_, String>(0)?,
            "nom":        r.get::<_, String>(1)?,
            "est_defaut": r.get::<_, i64>(2)? != 0,
            "quantite":   r.get::<_, f64>(3)?,
        }))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    Ok(x)
}

/// Stock de plusieurs articles dans tous les dépôts, en un appel.
///
/// Le POS charge ses articles une fois au démarrage ; interroger dépôt
/// par dépôt à chaque frappe serait intenable.
#[tauri::command]
pub fn lire_stock_multi_depots(
    etat: State<EtatApp>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let mut st = conn.prepare(
        "SELECT sd.article_id, sd.depot_id, d.nom, d.est_defaut, sd.quantite
         FROM stock_depot sd
         JOIN depot d ON d.id = sd.depot_id
         WHERE d.actif = 1 AND sd.quantite <> 0"
    ).map_err(|e| e.to_string())?;

    let x = st.query_map([], |r| {
        Ok(serde_json::json!({
            "article_id": r.get::<_, String>(0)?,
            "depot_id":   r.get::<_, String>(1)?,
            "depot_nom":  r.get::<_, String>(2)?,
            "est_defaut": r.get::<_, i64>(3)? != 0,
            "quantite":   r.get::<_, f64>(4)?,
        }))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    Ok(x)
}
