//! Initialisation et ouverture de la base de données SQLite.

use rusqlite::{Connection, Result};

pub fn ouvrir_base(chemin: &str) -> Result<Connection> {
    let conn = Connection::open(chemin)?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
    Ok(conn)
}

pub fn initialiser_tables(conn: &Connection) -> Result<()> {
    let schema = include_str!("schema.sql");
    conn.execute_batch(schema)?;

    // ---- Migrations colonnes (idempotentes) ----
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
    conn.execute(
        "ALTER TABLE parametres_societe ADD COLUMN logo_chemin TEXT", []
    ).ok();
    conn.execute(
        "ALTER TABLE article ADD COLUMN taux_tva_defaut REAL NOT NULL DEFAULT 0.0", []
    ).ok();
    // Lien vente -> piece_commerciale (facture POS automatique, D16).
    conn.execute(
        "ALTER TABLE vente ADD COLUMN piece_id TEXT", []
    ).ok();

    // ---- Nouvelles tables (une par une pour éviter stack overflow) ----

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS piece_commerciale (
            id               TEXT PRIMARY KEY,
            type_piece       TEXT NOT NULL,
            numero           TEXT NOT NULL UNIQUE,
            statut           TEXT NOT NULL DEFAULT 'brouillon',
            tiers_type       TEXT NOT NULL DEFAULT 'client',
            tiers_id         TEXT NOT NULL,
            depot_id         TEXT,
            piece_origine_id TEXT,
            auteur_id        TEXT,
            date_piece       TEXT NOT NULL,
            date_echeance    TEXT,
            remise_globale   REAL NOT NULL DEFAULT 0,
            note             TEXT,
            cree_le          TEXT NOT NULL,
            modifie_le       TEXT NOT NULL,
            origine          TEXT NOT NULL DEFAULT 'app'
        );"
    ).ok();

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS ligne_piece (
            id               TEXT PRIMARY KEY,
            piece_id         TEXT NOT NULL,
            article_id       TEXT NOT NULL,
            unite_vente_id   TEXT NOT NULL,
            quantite         REAL NOT NULL,
            prix_unitaire    INTEGER NOT NULL,
            remise_pct       REAL NOT NULL DEFAULT 0,
            remise_montant   INTEGER NOT NULL DEFAULT 0,
            taux_tva         REAL NOT NULL DEFAULT 0,
            montant_tva      INTEGER NOT NULL DEFAULT 0,
            montant_ht       INTEGER NOT NULL,
            cree_le          TEXT NOT NULL
        );"
    ).ok();

    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_piece_tiers
            ON piece_commerciale(tiers_id, tiers_type);"
    ).ok();

    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_ligne_piece ON ligne_piece(piece_id);"
    ).ok();

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS paiement_fournisseur (
            id              TEXT PRIMARY KEY,
            fournisseur_id  TEXT NOT NULL,
            montant         INTEGER NOT NULL,
            mode            TEXT NOT NULL DEFAULT 'especes',
            note            TEXT,
            auteur_id       TEXT,
            date_paiement   TEXT NOT NULL,
            cree_le         TEXT NOT NULL,
            origine         TEXT NOT NULL DEFAULT 'app'
        );"
    ).ok();

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS creance_irrecouvrable (
            id          TEXT PRIMARY KEY,
            vente_id    TEXT NOT NULL,
            motif       TEXT NOT NULL,
            auteur_id   TEXT,
            date_marque TEXT NOT NULL,
            cree_le     TEXT NOT NULL,
            origine     TEXT NOT NULL DEFAULT 'app'
        );"
    ).ok();

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS relance_creance (
            id           TEXT PRIMARY KEY,
            vente_id     TEXT NOT NULL,
            canal        TEXT NOT NULL DEFAULT 'whatsapp',
            note         TEXT,
            auteur_id    TEXT,
            date_relance TEXT NOT NULL,
            cree_le      TEXT NOT NULL,
            origine      TEXT NOT NULL DEFAULT 'app'
        );"
    ).ok();

    Ok(())
}