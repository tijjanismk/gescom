//! Persistance des unités de vente : insertion et lecture depuis SQLite.
//!
//! Une unité de vente définit comment un article se vend (à l'unité, au carton,
//! au kg...) avec son facteur de conversion vers l'unité de base et son prix
//! de référence. Un article a toujours au moins une unité de vente.

use crate::utils::maintenant_iso;
use rusqlite::{params, Connection, Result};
use uuid::Uuid;
/// Une unité de vente telle qu'elle vit en base.
#[derive(Debug)]
pub struct UniteVente {
    pub id: String,
    pub article_id: String,
    pub libelle: String,
    pub facteur: f64,
    pub prix_reference: i64,
    pub actif: bool,
}

/// Insère une nouvelle unité de vente pour un article.
///
/// `facteur` : combien d'unités de base vaut cette unité.
///   - unité simple  -> facteur 1.0
///   - carton de 12  -> facteur 12.0
///   - demi-unité    -> facteur 0.5
///
/// `prix_reference` : le prix de vente par défaut pour CETTE unité.
///   Permet le prix de gros dégressif : carton à 5400, unité à 500.
pub fn inserer_unite_vente(
    conn: &Connection,
    article_id: &str,
    libelle: &str,
    facteur: f64,
    prix_reference: i64,
    cree_par: &str,
    origine: &str,
) -> Result<String> {
    let id = Uuid::new_v4().to_string();
    let maintenant = maintenant_iso();

    conn.execute(
        "INSERT INTO unite_vente (
            id, article_id, libelle, facteur, prix_reference, actif,
            cree_le, modifie_le, cree_par, modifie_par, origine
        ) VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?7, ?8, ?9, ?10)",
        params![
            id,
            article_id,
            libelle,
            facteur,
            prix_reference,
            maintenant,
            maintenant,
            cree_par,
            cree_par,
            origine
        ],
    )?;

    Ok(id)
}

/// Lit toutes les unités de vente actives d'un article.
pub fn lire_unites_vente(conn: &Connection, article_id: &str) -> Result<Vec<UniteVente>> {
    let mut stmt = conn.prepare(
        "SELECT id, article_id, libelle, facteur, prix_reference, actif
         FROM unite_vente
         WHERE article_id = ?1 AND actif = 1
         ORDER BY facteur ASC",
    )?;

    let unites = stmt
        .query_map(params![article_id], |row| {
            Ok(UniteVente {
                id: row.get(0)?,
                article_id: row.get(1)?,
                libelle: row.get(2)?,
                facteur: row.get(3)?,
                prix_reference: row.get(4)?,
                actif: row.get::<_, i64>(5)? != 0,
            })
        })?
        .collect::<Result<Vec<_>>>()?;

    Ok(unites)
}

// =====================================================================
//  TESTS
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistance::initialiser_tables;
    use rusqlite::Connection;

    /// Base en mémoire — rien sur le disque, repart de zéro à chaque test.
    fn base_test() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        initialiser_tables(&conn).unwrap();
        conn
    }

    /// Insère une catégorie et un article de base pour les tests.
    /// On a besoin des deux avant de pouvoir créer une unité de vente
    /// (contrainte de clé étrangère : unite_vente.article_id -> article.id).
    fn preparer_article(conn: &Connection) -> String {
        conn.execute(
            "INSERT INTO categorie
             (id, nom, schema_attributs, actif, cree_le, modifie_le, origine)
             VALUES ('cat-1', 'Alimentation', '[]', 1,
                     '2024-01-01', '2024-01-01', 'machine-1')",
            [],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO article
             (id, nom, categorie_id, unite_base, gere_en_stock,
              attributs, actif, cree_le, modifie_le,
              cree_par, modifie_par, origine)
             VALUES ('art-1', 'Sucre', 'cat-1', 'kg', 1,
                     '{}', 1, '2024-01-01', '2024-01-01',
                     'user-1', 'user-1', 'machine-1')",
            [],
        )
        .unwrap();

        "art-1".to_string()
    }

    #[test]
    fn inserer_et_lire_unite_simple() {
        let conn = base_test();
        let article_id = preparer_article(&conn);

        // Unité simple : vente au kg, facteur 1, prix 800 F/kg.
        let id =
            inserer_unite_vente(&conn, &article_id, "kg", 1.0, 800, "user-1", "machine-1").unwrap();

        assert!(!id.is_empty());

        let unites = lire_unites_vente(&conn, &article_id).unwrap();
        assert_eq!(unites.len(), 1);
        assert_eq!(unites[0].libelle, "kg");
        assert_eq!(unites[0].facteur, 1.0);
        assert_eq!(unites[0].prix_reference, 800);
    }

    #[test]
    fn plusieurs_unites_par_article() {
        let conn = base_test();
        let article_id = preparer_article(&conn);

        // Un article vendu à l'unité ET au carton — prix de gros dégressif.
        inserer_unite_vente(&conn, &article_id, "unité", 1.0, 500, "user-1", "machine-1").unwrap();

        inserer_unite_vente(
            &conn,
            &article_id,
            "carton",
            12.0,
            5400,
            "user-1",
            "machine-1",
        )
        .unwrap();

        let unites = lire_unites_vente(&conn, &article_id).unwrap();

        // Les deux unités sont là, triées par facteur (unité avant carton).
        assert_eq!(unites.len(), 2);
        assert_eq!(unites[0].libelle, "unité"); // facteur 1.0
        assert_eq!(unites[1].libelle, "carton"); // facteur 12.0

        // Vérif du prix de gros : le carton à 5400 est moins cher
        // que 12 unités à 500 (= 6000). C'est la décision U3.
        assert!(unites[1].prix_reference < unites[0].prix_reference * 12);
    }

    #[test]
    fn unites_d_un_autre_article_non_retournees() {
        let conn = base_test();
        let article_id = preparer_article(&conn);

        // Créer un deuxième article.
        conn.execute(
            "INSERT INTO article
             (id, nom, categorie_id, unite_base, gere_en_stock,
              attributs, actif, cree_le, modifie_le,
              cree_par, modifie_par, origine)
             VALUES ('art-2', 'Riz', 'cat-1', 'kg', 1,
                     '{}', 1, '2024-01-01', '2024-01-01',
                     'user-1', 'user-1', 'machine-1')",
            [],
        )
        .unwrap();

        // Une unité pour chaque article.
        inserer_unite_vente(&conn, &article_id, "kg", 1.0, 800, "user-1", "machine-1").unwrap();
        inserer_unite_vente(&conn, "art-2", "kg", 1.0, 600, "user-1", "machine-1").unwrap();

        // On lit les unités du premier article seulement.
        let unites = lire_unites_vente(&conn, &article_id).unwrap();
        assert_eq!(unites.len(), 1);
        assert_eq!(unites[0].prix_reference, 800); // pas 600 (le riz)
    }
}
