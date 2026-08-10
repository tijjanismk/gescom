//! Commandes Tauri pour les paramètres (articles, catégories, unités).

use tauri::State;
use crate::commandes::ventes::EtatApp;
use crate::utils::maintenant_iso;

// =====================================================================
//  CATÉGORIES
// =====================================================================

#[tauri::command]
pub fn lire_categories(
    etat: State<EtatApp>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let mut stmt = conn.prepare(
        "SELECT c.id, c.nom,
                COUNT(a.id) as nb_articles
         FROM categorie c
         LEFT JOIN article a ON a.categorie_id = c.id AND a.actif = 1
         WHERE c.actif = 1
         GROUP BY c.id
         ORDER BY c.nom ASC"
    ).map_err(|e| e.to_string())?;

    let x = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "id":          row.get::<_, String>(0)?,
            "nom":         row.get::<_, String>(1)?,
            "nb_articles": row.get::<_, i64>(2)?,
        }))
    })
    .map_err(|e| e.to_string())?
    .filter_map(|r| r.ok())
    .collect();
    Ok(x)
}

#[tauri::command]
pub fn creer_categorie(
    etat: State<EtatApp>,
    nom: String,
) -> Result<String, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let id = uuid::Uuid::new_v4().to_string();
    let maintenant = maintenant_iso();

    conn.execute(
        "INSERT INTO categorie (id, nom, schema_attributs, actif, cree_le, modifie_le, origine)
         VALUES (?1, ?2, '[]', 1, ?3, ?4, 'app')",
        rusqlite::params![id, nom, maintenant, maintenant],
    ).map_err(|e| e.to_string())?;

    Ok(id)
}

// =====================================================================
//  ARTICLES COMPLETS
// =====================================================================

#[tauri::command]
pub fn lire_articles_complets(
    etat: State<EtatApp>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    // Lire les articles.
    let mut stmt_art = conn.prepare(
        "SELECT a.id, a.nom, a.reference, a.categorie_id, c.nom,
                a.unite_base, a.gere_en_stock, a.actif,
                COALESCE(SUM(sd.quantite), 0) as stock_total
         FROM article a
         JOIN categorie c ON c.id = a.categorie_id
         LEFT JOIN stock_depot sd ON sd.article_id = a.id
         WHERE a.actif = 1
         GROUP BY a.id
         ORDER BY a.nom ASC"
    ).map_err(|e| e.to_string())?;

    let articles_base: Vec<(String, String, Option<String>, String, String,
                             String, bool, bool, f64)> =
    {
        let x = stmt_art.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, i64>(6)? != 0,
                row.get::<_, i64>(7)? != 0,
                row.get::<_, f64>(8)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
        x
    };

    // Pour chaque article, charger ses unités.
    let mut articles = Vec::new();
    for (id, nom, reference, cat_id, cat_nom,
         unite_base, gere_en_stock, actif, stock_total) in articles_base {

        let mut stmt_u = conn.prepare(
            "SELECT id, libelle, facteur, prix_reference
             FROM unite_vente WHERE article_id = ?1 AND actif = 1
             ORDER BY facteur ASC"
        ).map_err(|e| e.to_string())?;

        let unites: Vec<serde_json::Value> = {
            let x = stmt_u.query_map(rusqlite::params![id], |row| {
                Ok(serde_json::json!({
                    "id":             row.get::<_, String>(0)?,
                    "libelle":        row.get::<_, String>(1)?,
                    "facteur":        row.get::<_, f64>(2)?,
                    "prix_reference": row.get::<_, i64>(3)?,
                }))
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();
            x
        };

        articles.push(serde_json::json!({
            "id":            id,
            "nom":           nom,
            "reference":     reference,
            "categorie_id":  cat_id,
            "categorie_nom": cat_nom,
            "unite_base":    unite_base,
            "gere_en_stock": gere_en_stock,
            "actif":         actif,
            "stock_total":   stock_total,
            "unites":        unites,
        }));
    }

    Ok(articles)
}

#[tauri::command]
pub fn creer_article_complet(
    etat: State<EtatApp>,
    nom: String,
    reference: Option<String>,
    categorie_id: String,
    unite_base: String,
    prix_reference: i64,
    stock_initial: f64,
) -> Result<String, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let utilisateur_id = crate::commandes::ventes::id_utilisateur_courant_pub(&conn);

    let depot_id: String = conn.query_row(
        "SELECT id FROM depot WHERE est_defaut = 1 LIMIT 1",
        [],
        |row| row.get(0),
    ).map_err(|e| e.to_string())?;

    let article_id = uuid::Uuid::new_v4().to_string();
    let unite_id   = uuid::Uuid::new_v4().to_string();
    let stock_id   = uuid::Uuid::new_v4().to_string();
    let maintenant = maintenant_iso();

    conn.execute(
        "INSERT INTO article
         (id, nom, reference, categorie_id, unite_base, gere_en_stock,
          attributs, actif, cree_le, modifie_le, cree_par, modifie_par, origine)
         VALUES (?1, ?2, ?3, ?4, ?5, 1, '{}', 1, ?6, ?7, ?8, ?9, 'app')",
        rusqlite::params![
            article_id, nom, reference, categorie_id, unite_base,
            maintenant, maintenant, utilisateur_id, utilisateur_id
        ],
    ).map_err(|e| e.to_string())?;

    conn.execute(
        "INSERT INTO unite_vente
         (id, article_id, libelle, facteur, prix_reference, actif,
          cree_le, modifie_le, cree_par, modifie_par, origine)
         VALUES (?1, ?2, ?3, 1.0, ?4, 1, ?5, ?6, ?7, ?8, 'app')",
        rusqlite::params![
            unite_id, article_id, unite_base, prix_reference,
            maintenant, maintenant, utilisateur_id, utilisateur_id
        ],
    ).map_err(|e| e.to_string())?;

    conn.execute(
        "INSERT INTO stock_depot (id, article_id, depot_id, quantite)
         VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![stock_id, article_id, depot_id, stock_initial],
    ).map_err(|e| e.to_string())?;

    Ok(article_id)
}

// =====================================================================
//  UNITÉS DE VENTE
// =====================================================================

#[tauri::command]
pub fn creer_unite_vente(
    etat: State<EtatApp>,
    article_id: String,
    libelle: String,
    facteur: f64,
    prix_reference: i64,
) -> Result<String, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let utilisateur_id = crate::commandes::ventes::id_utilisateur_courant_pub(&conn);

    let id = uuid::Uuid::new_v4().to_string();
    let maintenant = maintenant_iso();

    conn.execute(
        "INSERT INTO unite_vente
         (id, article_id, libelle, facteur, prix_reference, actif,
          cree_le, modifie_le, cree_par, modifie_par, origine)
         VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?7, ?8, ?9, 'app')",
        rusqlite::params![
            id, article_id, libelle, facteur, prix_reference,
            maintenant, maintenant, utilisateur_id, utilisateur_id
        ],
    ).map_err(|e| e.to_string())?;

    Ok(id)
}
