//! Initialisation et ouverture de la base de données SQLite.

use rusqlite::{Connection, Result};

pub fn ouvrir_base(chemin: &str) -> Result<Connection> {
    let conn = Connection::open(chemin)?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
    Ok(conn)
}

fn colonne_existe(conn: &Connection, table: &str, colonne: &str) -> bool {
    conn.prepare(&format!("PRAGMA table_info({})", table))
        .ok()
        .and_then(|mut stmt| {
            stmt.query_map([], |row| row.get::<_, String>(1))
                .ok()
                .map(|rows| {
                    rows.filter_map(|r| r.ok())
                        .any(|nom| nom == colonne)
                })
        })
        .unwrap_or(false)
}

fn ajouter_colonne_si_absente(
    conn: &Connection,
    table: &str,
    colonne: &str,
    definition: &str,
) {
    if !colonne_existe(conn, table, colonne) {
        conn.execute(
            &format!("ALTER TABLE {} ADD COLUMN {} {}", table, colonne, definition),
            [],
        ).ok();
    }
}

pub fn initialiser_tables(conn: &Connection) -> Result<()> {
    let schema = include_str!("schema.sql");
    conn.execute_batch(schema)?;

    // ---- Migrations pour bases existantes ----
    // Ordre important : ajouter les colonnes AVANT de créer les index qui les référencent.

    ajouter_colonne_si_absente(conn, "ligne_vente", "taux_tva",
        "REAL NOT NULL DEFAULT 0.0");
    ajouter_colonne_si_absente(conn, "ligne_vente", "montant_tva",
        "INTEGER NOT NULL DEFAULT 0");
    ajouter_colonne_si_absente(conn, "session_caisse", "solde_theorique", "INTEGER");
    ajouter_colonne_si_absente(conn, "session_caisse", "especes_comptees", "INTEGER");
    ajouter_colonne_si_absente(conn, "session_caisse", "ecart", "INTEGER");
    ajouter_colonne_si_absente(conn, "session_caisse", "ferme_le", "TEXT");
    ajouter_colonne_si_absente(conn, "parametres_societe", "logo_chemin", "TEXT");
    ajouter_colonne_si_absente(conn, "article", "code_barre", "TEXT");
    ajouter_colonne_si_absente(conn, "avoir", "vente_utilisation_id", "TEXT");

    // ---- Index créés APRÈS les migrations ----
    // (évite l'erreur "no such column" sur les bases existantes)
    conn.execute_batch("
        CREATE INDEX IF NOT EXISTS idx_avoir_client
            ON avoir(client_id, statut);
        CREATE INDEX IF NOT EXISTS idx_article_code_barre
            ON article(code_barre);
    ")?;

    Ok(())
}