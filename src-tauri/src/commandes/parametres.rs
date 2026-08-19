//! Commandes Tauri pour les paramètres (articles, catégories).

use tauri::State;
use crate::commandes::ventes::EtatApp;
use crate::utils::maintenant_iso;

#[tauri::command]
pub fn lire_categories(etat: State<EtatApp>) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(
        "SELECT id, nom FROM categorie WHERE actif = 1 ORDER BY nom"
    ).map_err(|e| e.to_string())?;
    let x = stmt.query_map([], |row| {
        Ok(serde_json::json!({"id": row.get::<_,String>(0)?, "nom": row.get::<_,String>(1)?}))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
    Ok(x)
}

#[tauri::command]
pub fn creer_categorie(etat: State<EtatApp>, nom: String) -> Result<String, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let id = uuid::Uuid::new_v4().to_string();
    let now = maintenant_iso();
    conn.execute(
        "INSERT INTO categorie (id, nom, schema_attributs, actif, cree_le, modifie_le, origine)
         VALUES (?1,?2,'[]',1,?3,?4,'app')",
        rusqlite::params![id, nom, now, now],
    ).map_err(|e| e.to_string())?;
    Ok(id)
}

#[tauri::command]
pub fn lire_articles_complets(etat: State<EtatApp>) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(
        "SELECT a.id, a.nom, a.unite_base, a.dernier_prix_achat,
                u.id, u.libelle, u.facteur, u.prix_reference
         FROM article a
         JOIN unite_vente u ON u.article_id = a.id AND u.actif = 1
         WHERE a.actif = 1 ORDER BY a.nom, u.facteur"
    ).map_err(|e| e.to_string())?;

    let mut articles: Vec<serde_json::Value> = Vec::new();
    let mut courant_id = String::new();

    stmt.query_map([], |row| {
        Ok((
            row.get::<_,String>(0)?,
            row.get::<_,String>(1)?,
            row.get::<_,String>(2)?,
            row.get::<_,Option<i64>>(3)?,
            row.get::<_,String>(4)?,
            row.get::<_,String>(5)?,
            row.get::<_,f64>(6)?,
            row.get::<_,i64>(7)?,
        ))
    }).map_err(|e| e.to_string())?
    .filter_map(|r| r.ok())
    .for_each(|(art_id, art_nom, unite_base, prix_achat,
                u_id, u_libelle, facteur, prix_ref)| {
        let unite = serde_json::json!({
            "id": u_id, "libelle": u_libelle,
            "facteur": facteur, "prix_reference": prix_ref,
        });
        if art_id != courant_id {
            courant_id = art_id.clone();
            articles.push(serde_json::json!({
                "id": art_id, "nom": art_nom,
                "unite_base": unite_base,
                "dernier_prix_achat": prix_achat,
                "unites": [unite],
            }));
        } else if let Some(last) = articles.last_mut() {
            if let Some(unites) = last["unites"].as_array_mut() {
                unites.push(unite);
            }
        }
    });
    Ok(articles)
}

#[tauri::command]
pub fn creer_article_complet(
    etat: State<EtatApp>,
    nom: String,
    categorie_id: Option<String>,
    unite_base: String,
    prix_reference: i64,
) -> Result<String, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let now = maintenant_iso();
    let art_id = uuid::Uuid::new_v4().to_string();
    let auteur = crate::commandes::ventes::id_utilisateur_courant_pub(&conn);

    conn.execute(
        "INSERT INTO article
         (id, nom, categorie_id, unite_base, gere_en_stock, attributs,
          actif, cree_le, modifie_le, cree_par, modifie_par, origine)
         VALUES (?1,?2,?3,?4,1,'{}',1,?5,?6,?7,?8,'app')",
        rusqlite::params![art_id, nom, categorie_id, unite_base,
                          now, now, auteur, auteur],
    ).map_err(|e| e.to_string())?;

    conn.execute(
        "INSERT INTO unite_vente
         (id, article_id, libelle, facteur, prix_reference, actif,
          cree_le, modifie_le, cree_par, modifie_par, origine)
         VALUES (?1,?2,?3,1.0,?4,1,?5,?6,?7,?8,'app')",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(), art_id, unite_base,
            prix_reference, now, now, auteur, auteur
        ],
    ).map_err(|e| e.to_string())?;

    let depot_id: String = conn.query_row(
        "SELECT id FROM depot WHERE est_defaut = 1 LIMIT 1",
        [], |r| r.get(0)
    ).map_err(|e| e.to_string())?;

    conn.execute(
        "INSERT OR IGNORE INTO stock_depot (id, article_id, depot_id, quantite)
         VALUES (?1,?2,?3,0)",
        rusqlite::params![uuid::Uuid::new_v4().to_string(), art_id, depot_id],
    ).ok();

    Ok(art_id)
}

#[tauri::command]
pub fn lire_stocks(etat: State<EtatApp>) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(
        "SELECT a.id, a.nom, a.unite_base, d.nom, sd.quantite, sd.depot_id
         FROM stock_depot sd
         JOIN article a ON a.id = sd.article_id
         JOIN depot d ON d.id = sd.depot_id
         WHERE a.actif = 1
         ORDER BY sd.quantite ASC, a.nom ASC"
    ).map_err(|e| e.to_string())?;
    let x = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "article_id": row.get::<_,String>(0)?,
            "article_nom": row.get::<_,String>(1)?,
            "unite_base": row.get::<_,String>(2)?,
            "depot_nom": row.get::<_,String>(3)?,
            "quantite": row.get::<_,f64>(4)?,
            "depot_id": row.get::<_,String>(5)?,
        }))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
    Ok(x)
}
