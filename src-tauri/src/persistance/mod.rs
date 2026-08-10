pub mod articles;
pub mod avoirs;
pub mod clients;
pub mod depots;
pub mod factures;
pub mod fournisseurs;
pub mod journal;
pub mod mouvements_stock;
pub mod paiements;
pub mod retours;
pub mod sessions_caisse;
pub mod transferts;
pub mod unites_vente;
pub mod ventes;

use rusqlite::Connection;
use std::path::Path;

pub fn ouvrir_base(chemin: &Path) -> Result<Connection, rusqlite::Error> {
    let conn = Connection::open(chemin)?;
    initialiser_tables(&conn)?;
    Ok(conn)
}

pub(crate) fn initialiser_tables(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(include_str!("schema.sql"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn base_ouvre_et_schema_cree() {
        // Base en mémoire — vérifie que toutes les tables sont créées.
        let conn = Connection::open_in_memory().unwrap();
        initialiser_tables(&conn).unwrap();

        // Vérifier que les tables principales existent.
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
             WHERE type='table'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        // On a 20 tables dans le schéma.
        assert_eq!(count, 20);
    }
}
