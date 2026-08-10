//! Persistance des articles : insertion et lecture depuis SQLite.

use crate::utils::maintenant_iso;
use rusqlite::{params, Connection, Result};
use uuid::Uuid;
/// Un article tel qu'il vit en b
/// ase.
#[derive(Debug)]
pub struct Article {
    pub id: String,
    pub nom: String,
    pub reference: Option<String>,
    pub categorie_id: String,
    pub prix_achat: Option<i64>,
    pub dernier_prix_achat: Option<i64>,
    pub unite_base: String,
    pub gere_en_stock: bool,
    pub attributs: String,
    pub actif: bool,
}

/// Insère un nouvel article en base.
/// L'UUID et les horodatages sont générés ici — l'appelant ne les fournit pas.
pub fn inserer_article(
    conn: &Connection,
    nom: &str,
    categorie_id: &str,
    unite_base: &str,
    gere_en_stock: bool,
    cree_par: &str,
    origine: &str,
) -> Result<String> {
    let id = Uuid::new_v4().to_string();
    let maintenant = maintenant_iso();

    conn.execute(
        "INSERT INTO article (
            id, nom, categorie_id, unite_base, gere_en_stock,
            attributs, actif,
            cree_le, modifie_le, cree_par, modifie_par, origine
        ) VALUES (?1, ?2, ?3, ?4, ?5, '{}', 1, ?6, ?7, ?8, ?9, ?10)",
        params![
            id,
            nom,
            categorie_id,
            unite_base,
            gere_en_stock as i64,
            maintenant,
            maintenant,
            cree_par,
            cree_par,
            origine
        ],
    )?;

    Ok(id) // on retourne l'UUID créé
}

/// Lit tous les articles actifs.
pub fn lire_articles(conn: &Connection) -> Result<Vec<Article>> {
    let mut stmt = conn.prepare(
        "SELECT id, nom, reference, categorie_id,
                prix_achat, dernier_prix_achat,
                unite_base, gere_en_stock, attributs, actif
         FROM article
         WHERE actif = 1
         ORDER BY nom",
    )?;

    let articles = stmt
        .query_map([], |row| {
            Ok(Article {
                id: row.get(0)?,
                nom: row.get(1)?,
                reference: row.get(2)?,
                categorie_id: row.get(3)?,
                prix_achat: row.get(4)?,
                dernier_prix_achat: row.get(5)?,
                unite_base: row.get(6)?,
                gere_en_stock: row.get::<_, i64>(7)? != 0,
                attributs: row.get(8)?,
                actif: row.get::<_, i64>(9)? != 0,
            })
        })?
        .collect::<Result<Vec<_>>>()?;

    Ok(articles)
}

// =====================================================================
//  TESTS
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistance::ouvrir_base;

    fn base_en_memoire() -> Connection {
        // On ouvre une base SQLite EN MÉMOIRE pour les tests —
        // rien n'est écrit sur le disque, chaque test repart de zéro.
        let conn = Connection::open_in_memory().unwrap();
        crate::persistance::initialiser_tables(&conn).unwrap();
        conn
    }

    #[test]
    fn inserer_et_lire_un_article() {
        let conn = base_en_memoire();

        // On a besoin d'une catégorie d'abord (contrainte FK).
        conn.execute(
            "INSERT INTO categorie (id, nom, schema_attributs, actif, cree_le, modifie_le, origine)
             VALUES ('cat-001', 'Alimentation', '[]', 1, '2024-01-01', '2024-01-01', 'machine-1')",
            [],
        )
        .unwrap();

        // Insertion d'un article.
        let id =
            inserer_article(&conn, "Sucre", "cat-001", "kg", true, "user-1", "machine-1").unwrap();

        // L'UUID retourné ne doit pas être vide.
        assert!(!id.is_empty());

        // On relit les articles.
        let articles = lire_articles(&conn).unwrap();
        assert_eq!(articles.len(), 1);
        assert_eq!(articles[0].nom, "Sucre");
        assert_eq!(articles[0].unite_base, "kg");
        assert!(articles[0].gere_en_stock);
    }

    #[test]
    fn article_inactif_non_retourne() {
        let conn = base_en_memoire();

        conn.execute(
            "INSERT INTO categorie (id, nom, schema_attributs, actif, cree_le, modifie_le, origine)
             VALUES ('cat-001', 'Alimentation', '[]', 1, '2024-01-01', '2024-01-01', 'machine-1')",
            [],
        )
        .unwrap();

        // Insérer puis désactiver un article.
        let id =
            inserer_article(&conn, "Riz", "cat-001", "kg", true, "user-1", "machine-1").unwrap();

        conn.execute("UPDATE article SET actif = 0 WHERE id = ?1", params![id])
            .unwrap();

        // lire_articles ne doit pas le retourner (actif = 0).
        let articles = lire_articles(&conn).unwrap();
        assert_eq!(articles.len(), 0);
    }
}
