//! Persistance des dépôts et du stock.
//!
//! STOCK_DEPOT est la table la plus sensible du système : elle est modifiée
//! à chaque vente, retour, transfert ou ajustement. Elle est la seule source
//! de vérité sur les quantités disponibles.
//!
//! Règle absolue : ne jamais modifier stock_depot directement depuis l'extérieur
//! de ce module. Toute modification passe par les fonctions ici — c'est le début
//! de la porte d'écriture unique.

use crate::utils::maintenant_iso;
use rusqlite::{params, Connection, Result};
use uuid::Uuid;

/// Un dépôt tel qu'il vit en base.
#[derive(Debug)]
pub struct Depot {
    pub id: String,
    pub nom: String,
    pub est_defaut: bool,
    pub actif: bool,
}

/// Le stock d'un article dans un dépôt.
#[derive(Debug)]
pub struct StockDepot {
    pub id: String,
    pub article_id: String,
    pub depot_id: String,
    pub quantite: f64, // peut être négative (vente à découvert)
}

/// Insère un nouveau dépôt.
pub fn inserer_depot(
    conn: &Connection,
    nom: &str,
    est_defaut: bool,
    origine: &str,
) -> Result<String> {
    let id = Uuid::new_v4().to_string();
    let maintenant = maintenant_iso();

    conn.execute(
        "INSERT INTO depot (id, nom, est_defaut, actif, cree_le, modifie_le, origine)
         VALUES (?1, ?2, ?3, 1, ?4, ?5, ?6)",
        params![id, nom, est_defaut as i64, maintenant, maintenant, origine],
    )?;

    Ok(id)
}

/// Initialise le stock d'un article dans un dépôt à zéro.
/// Appelée quand on crée un article ou qu'on l'affecte à un nouveau dépôt.
pub fn initialiser_stock(conn: &Connection, article_id: &str, depot_id: &str) -> Result<()> {
    let id = Uuid::new_v4().to_string();

    conn.execute(
        "INSERT OR IGNORE INTO stock_depot (id, article_id, depot_id, quantite)
         VALUES (?1, ?2, ?3, 0.0)",
        params![id, article_id, depot_id],
    )?;

    Ok(())
}

/// Lit le stock actuel d'un article dans un dépôt donné.
/// Retourne 0.0 si aucun enregistrement n'existe encore.
pub fn lire_stock(conn: &Connection, article_id: &str, depot_id: &str) -> Result<f64> {
    let quantite = conn
        .query_row(
            "SELECT quantite FROM stock_depot
         WHERE article_id = ?1 AND depot_id = ?2",
            params![article_id, depot_id],
            |row| row.get(0),
        )
        .unwrap_or(0.0); // si pas de ligne, stock = 0

    Ok(quantite)
}

/// Modifie le stock d'un article dans un dépôt.
///
/// `delta` est SIGNÉ :
///   - positif -> entrée (achat, retour, ajustement +)
///   - négatif -> sortie (vente, transfert sortant, ajustement -)
///
/// Le résultat PEUT être négatif (vente à découvert autorisée).
/// Cette fonction ne journalise pas — c'est la porte d'écriture unique
/// qui appellera modifier_stock ET écrira au journal dans la même transaction.
pub fn modifier_stock(
    conn: &Connection,
    article_id: &str,
    depot_id: &str,
    delta: f64,
) -> Result<f64> {
    // S'assurer que la ligne existe avant de la modifier.
    initialiser_stock(conn, article_id, depot_id)?;

    conn.execute(
        "UPDATE stock_depot
         SET quantite = quantite + ?1
         WHERE article_id = ?2 AND depot_id = ?3",
        params![delta, article_id, depot_id],
    )?;

    // Relire le nouveau stock pour le retourner.
    lire_stock(conn, article_id, depot_id)
}

/// Lit tous les articles en stock négatif dans un dépôt —
/// la vue « à régulariser » que le patron doit surveiller.
pub fn articles_a_regulariser(conn: &Connection, depot_id: &str) -> Result<Vec<StockDepot>> {
    let mut stmt = conn.prepare(
        "SELECT id, article_id, depot_id, quantite
         FROM stock_depot
         WHERE depot_id = ?1 AND quantite < 0
         ORDER BY quantite ASC",
    )?;

    let stocks = stmt
        .query_map(params![depot_id], |row| {
            Ok(StockDepot {
                id: row.get(0)?,
                article_id: row.get(1)?,
                depot_id: row.get(2)?,
                quantite: row.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>>>()?;

    Ok(stocks)
}

// =====================================================================
//  TESTS
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistance::initialiser_tables;
    use rusqlite::Connection;

    fn base_test() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        initialiser_tables(&conn).unwrap();
        conn
    }

    /// Prépare un article et un dépôt pour les tests.
    fn preparer(conn: &Connection) -> (String, String) {
        conn.execute(
            "INSERT INTO categorie
             (id, nom, schema_attributs, actif, cree_le, modifie_le, origine)
             VALUES ('cat-1', 'Alim', '[]', 1, '2024-01-01', '2024-01-01', 'm1')",
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
                     'user-1', 'user-1', 'm1')",
            [],
        )
        .unwrap();

        let depot_id = inserer_depot(conn, "Dépôt principal", true, "m1").unwrap();

        ("art-1".to_string(), depot_id)
    }

    #[test]
    fn stock_initial_est_zero() {
        let conn = base_test();
        let (article_id, depot_id) = preparer(&conn);

        let stock = lire_stock(&conn, &article_id, &depot_id).unwrap();
        assert_eq!(stock, 0.0);
    }

    #[test]
    fn entree_de_stock() {
        let conn = base_test();
        let (article_id, depot_id) = preparer(&conn);

        // Réception de 50 kg.
        let nouveau = modifier_stock(&conn, &article_id, &depot_id, 50.0).unwrap();
        assert_eq!(nouveau, 50.0);
    }

    #[test]
    fn sortie_de_stock_normale() {
        let conn = base_test();
        let (article_id, depot_id) = preparer(&conn);

        modifier_stock(&conn, &article_id, &depot_id, 50.0).unwrap();

        // Vente de 12 kg.
        let nouveau = modifier_stock(&conn, &article_id, &depot_id, -12.0).unwrap();
        assert_eq!(nouveau, 38.0);
    }

    #[test]
    fn vente_a_decouvert_autorisee() {
        let conn = base_test();
        let (article_id, depot_id) = preparer(&conn);

        // Stock à 0, on vend 3 kg -> stock passe à -3 (signal de régularisation).
        let nouveau = modifier_stock(&conn, &article_id, &depot_id, -3.0).unwrap();
        assert_eq!(nouveau, -3.0);
        assert!(nouveau < 0.0);
    }

    #[test]
    fn vue_articles_a_regulariser() {
        let conn = base_test();
        let (article_id, depot_id) = preparer(&conn);

        // Mettre le stock en négatif.
        modifier_stock(&conn, &article_id, &depot_id, -5.0).unwrap();

        let a_regulariser = articles_a_regulariser(&conn, &depot_id).unwrap();
        assert_eq!(a_regulariser.len(), 1);
        assert_eq!(a_regulariser[0].article_id, article_id);
        assert_eq!(a_regulariser[0].quantite, -5.0);
    }

    #[test]
    fn regularisation_remet_stock_positif() {
        let conn = base_test();
        let (article_id, depot_id) = preparer(&conn);

        // Stock à -3, le patron régularise en ajoutant 3.
        modifier_stock(&conn, &article_id, &depot_id, -3.0).unwrap();
        let apres = modifier_stock(&conn, &article_id, &depot_id, 3.0).unwrap();

        assert_eq!(apres, 0.0);

        // Plus rien à régulariser.
        let a_regulariser = articles_a_regulariser(&conn, &depot_id).unwrap();
        assert_eq!(a_regulariser.len(), 0);
    }
}
