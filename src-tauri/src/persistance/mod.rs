//! Initialisation et ouverture de la base de données SQLite.

use rusqlite::{Connection, Result};

/// Ouvre la base de données SQLite dans le répertoire de données de l'app.
pub fn ouvrir_base(chemin: &str) -> Result<Connection> {
    let conn = Connection::open(chemin)?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
    Ok(conn)
}

/// Crée toutes les tables à partir du schema.sql embarqué.
pub fn initialiser_tables(conn: &Connection) -> Result<()> {
    let schema = include_str!("schema.sql");
    conn.execute_batch(schema)?;

    // Migrations additionnelles — colonnes ajoutées après la version initiale.
    // .ok() = erreur ignorée si la colonne existe déjà (SQLite ne supporte pas
    // ALTER TABLE ADD COLUMN IF NOT EXISTS).
    conn.execute(
        "ALTER TABLE ligne_vente ADD COLUMN taux_tva REAL NOT NULL DEFAULT 0.0", []
    ).ok();
    conn.execute(
        "ALTER TABLE ligne_vente ADD COLUMN montant_tva INTEGER NOT NULL DEFAULT 0", []
    ).ok();
    conn.execute(
        "ALTER TABLE session_caisse ADD COLUMN solde_theorique INTEGER", []
    ).ok();
    conn.execute(
        "ALTER TABLE session_caisse ADD COLUMN especes_comptees INTEGER", []
    ).ok();
    conn.execute(
        "ALTER TABLE session_caisse ADD COLUMN ecart INTEGER", []
    ).ok();
    conn.execute(
        "ALTER TABLE session_caisse ADD COLUMN ferme_le TEXT", []
    ).ok();

    Ok(())
}
