//! Journal append-only — §10.

use rusqlite::{Connection, Result, params};
use crate::utils::maintenant_iso;

/// Écrit un événement dans le journal. Ne lève jamais d'erreur bloquante.
pub fn ecrire_evenement(
    conn: &Connection,
    type_evenement: &str,
    entite_type: &str,
    entite_id: &str,
    auteur_id: &str,
    ancien_valeur: Option<&str>,
    nouveau_valeur: Option<&str>,
    origine: &str,
) -> Result<()> {
    conn.execute(
        "INSERT INTO journal
         (id, type_evenement, entite_type, entite_id, auteur_id,
          ancien_valeur, nouveau_valeur, origine, date_evenement)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        params![
            uuid::Uuid::new_v4().to_string(),
            type_evenement, entite_type, entite_id, auteur_id,
            ancien_valeur, nouveau_valeur, origine,
            maintenant_iso()
        ],
    )?;
    Ok(())
}
