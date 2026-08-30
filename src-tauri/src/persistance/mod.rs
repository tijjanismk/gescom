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
    // L3 — rattachement des achats a leur fournisseur.
    // Sans cette colonne, toutes les dettes fournisseurs sont identiques.
    conn.execute(
        "ALTER TABLE mouvement_stock ADD COLUMN fournisseur_id TEXT", []
    ).ok();
    // Prix d'achat AU MOMENT du mouvement : sinon tout l'historique est
    // recalcule au dernier prix connu (article.dernier_prix_achat).
    conn.execute(
        "ALTER TABLE mouvement_stock ADD COLUMN prix_achat_unitaire INTEGER", []
    ).ok();
    // Imputation : a quelle facture fournisseur ce paiement se rapporte.
    // NULL = paiement global non impute (anciens reglements).
    conn.execute(
        "ALTER TABLE paiement_fournisseur ADD COLUMN piece_id TEXT", []
    ).ok();
    // Depenses : un mouvement de caisse libre a besoin d'un libelle.
    // motif reste la categorie technique ('vente', 'achat', 'depense'),
    // libelle porte le texte saisi par le commercant.
    conn.execute(
        "ALTER TABLE mouvement_caisse ADD COLUMN libelle TEXT", []
    ).ok();
    // Poste de depense, pour ventiler le journal (transport, loyer...).
    conn.execute(
        "ALTER TABLE mouvement_caisse ADD COLUMN categorie TEXT", []
    ).ok();

    // ---- v1.2 : transferts inter-depots ----
    // `bon` regroupe les lignes d'un meme bon numerote BTR-AAAA-NNNNN.
    // La table transfert existait depuis l'origine mais n'etait pas
    // utilisee ; elle n'avait ni bon, ni unite, ni motif.
    conn.execute(
        "ALTER TABLE transfert ADD COLUMN bon TEXT", []
    ).ok();
    conn.execute(
        "ALTER TABLE transfert ADD COLUMN unite_vente_id TEXT", []
    ).ok();
    conn.execute(
        "ALTER TABLE transfert ADD COLUMN motif TEXT", []
    ).ok();
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_transfert_bon ON transfert(bon)"
    ).ok();

    // ---- v1.2 : suivi des cheques recus ----
    // Un cheque est une promesse, pas de l'argent. Il n'entre PAS dans
    // le rapprochement de caisse (D29) — comme le mobile money.
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS cheque_recu (
            id            TEXT PRIMARY KEY,
            paiement_id   TEXT,
            vente_id      TEXT,
            numero        TEXT NOT NULL,
            banque        TEXT NOT NULL,
            tireur        TEXT,
            montant       INTEGER NOT NULL,
            date_emission TEXT,
            date_echeance TEXT,
            statut        TEXT NOT NULL DEFAULT 'recu',
            motif_rejet   TEXT,
            cree_le       TEXT NOT NULL,
            modifie_le    TEXT NOT NULL,
            origine       TEXT NOT NULL DEFAULT 'app'
        )"
    ).ok();
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_cheque_statut ON cheque_recu(statut)"
    ).ok();
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_cheque_vente ON cheque_recu(vente_id)"
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