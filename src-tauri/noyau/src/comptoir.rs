//! Le comptoir : ce que l'ecran de vente appelle en plus du catalogue.
//!
//! Creation rapide d'un client ou d'un article au moment de vendre,
//! stock consolide de tous les depots, reglage du scanner. Rien de
//! spectaculaire — mais sans ces quatre-la, un poste caisse ouvre
//! l'ecran de vente et bute des le premier client inconnu.
//!
//! Le code vient de `commandes/`, coupe et non recrit :
//! `creer_client_rapide` fabrique un code client, et deux formats de
//! code entre le comptoir et la caisse seraient une confusion durable
//! sur les pieces deja imprimees.

use crate::argent::id_utilisateur_courant_pub;
use crate::utils::maintenant_iso;

pub fn creer_client_rapide(
    conn: &rusqlite::Connection,
    nom: String,
    telephone: Option<String>,
) -> Result<serde_json::Value, String> {
    let now = maintenant_iso();
    let id = uuid::Uuid::new_v4().to_string();
    let auteur = id_utilisateur_courant_pub(&conn);

    // Générer un code unique
    let nb: i64 = conn.query_row(
        "SELECT COUNT(*) FROM client WHERE est_generique = 0", [], |r| r.get(0)
    ).unwrap_or(0);
    let code = format!("CLIENT{:05}", nb + 1);

    conn.execute(
        "INSERT INTO client
         (id, code, nom, telephone, est_generique, actif,
          cree_le, modifie_le, cree_par, modifie_par, origine)
         VALUES (?1,?2,?3,?4,0,1,?5,?6,?7,?8,'app')",
        rusqlite::params![id, code, nom, telephone, now, now, auteur, auteur],
    ).map_err(|e| e.to_string())?;

    Ok(serde_json::json!({"id": id, "code": code, "nom": nom, "telephone": telephone}))
}

pub fn creer_article_rapide(
    conn: &rusqlite::Connection,
    nom: String,
    unite_base: String,
    prix_reference: i64,
    // Renseigne depuis l'ecran Achats : on connait le prix d'ACHAT bien
    // avant le prix de vente. Le prix de vente peut alors rester a 0 et
    // se fixer plus tard, dans Parametres.
    prix_achat: Option<i64>,
) -> Result<serde_json::Value, String> {
    let nom = nom.trim().to_string();
    if nom.is_empty() {
        return Err("Le nom de l'article est obligatoire".to_string());
    }


    // Un doublon de nom casse l'import CSV, qui rapproche les articles
    // par `lower(nom)` et en mettrait deux a jour a la fois.
    let existant: Option<(String, String)> = conn.query_row(
        "SELECT id, nom FROM article WHERE lower(nom) = lower(?1) AND actif = 1",
        rusqlite::params![nom], |r| Ok((r.get(0)?, r.get(1)?)),
    ).ok();
    if let Some((_, nom_existant)) = existant {
        return Err(format!("L'article « {} » existe déjà.", nom_existant));
    }
    let now = maintenant_iso();
    let art_id = uuid::Uuid::new_v4().to_string();
    let auteur = id_utilisateur_courant_pub(&conn);

    conn.execute(
        "INSERT INTO article
         (id, nom, unite_base, gere_en_stock, attributs, actif,
          dernier_prix_achat,
          cree_le, modifie_le, cree_par, modifie_par, origine)
         VALUES (?1,?2,?3,1,'{}',1,?4,?5,?6,?7,?8,'app')",
        rusqlite::params![art_id, nom, unite_base, prix_achat,
                          now, now, auteur, auteur],
    ).map_err(|e| e.to_string())?;

    let unite_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO unite_vente
         (id, article_id, libelle, facteur, prix_reference, actif,
          cree_le, modifie_le, cree_par, modifie_par, origine)
         VALUES (?1,?2,?3,1.0,?4,1,?5,?6,?7,?8,'app')",
        rusqlite::params![unite_id, art_id, unite_base, prix_reference,
                          now, now, auteur, auteur],
    ).map_err(|e| e.to_string())?;

    // Initialiser le stock à 0 dans le dépôt par défaut
    let depot_id: String = conn.query_row(
        "SELECT id FROM depot WHERE est_defaut = 1 LIMIT 1",
        [], |r| r.get(0)
    ).or_else(|_| conn.query_row(
        "SELECT id FROM depot WHERE actif = 1 ORDER BY nom LIMIT 1",
        [], |r| r.get(0)
    )).map_err(|_| "Aucun dépôt actif".to_string())?;

    conn.execute(
        "INSERT OR IGNORE INTO stock_depot (id, article_id, depot_id, quantite)
         VALUES (?1,?2,?3,0)",
        rusqlite::params![uuid::Uuid::new_v4().to_string(), art_id, depot_id],
    ).ok();

    Ok(serde_json::json!({
        "id": art_id, "nom": nom, "unite_base": unite_base, "stock": 0.0,
        "unites": [{"id": unite_id, "libelle": unite_base,
                    "facteur": 1.0, "prix_reference": prix_reference}]
    }))
}

pub fn lire_stock_multi_depots_sur(
    conn: &rusqlite::Connection,
) -> Result<Vec<serde_json::Value>, String> {
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

/// Le scanner de codes-barres est-il actif ?
pub fn lire_config_scanner(conn: &rusqlite::Connection) -> Result<bool, String> {
    let actif: String = conn
        .query_row(
            "SELECT valeur FROM config_app WHERE cle = 'scanner_actif'",
            [],
            |r| r.get(0),
        )
        .unwrap_or_else(|_| "0".to_string());
    Ok(actif == "1")
}
