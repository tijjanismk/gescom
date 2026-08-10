//! Journal d'audit immuable — colonne vertébrale de la confiance.
//!
//! Chaque opération sensible (vente, modification de prix, ajustement de stock,
//! validation de facture...) écrit un événement ici. Jamais de suppression,
//! jamais de modification. Écrit par la porte d'écriture unique dans la même
//! transaction que la modification d'état.

use crate::utils::maintenant_iso;
use rusqlite::{params, Connection, Result};
use uuid::Uuid;
#[derive(Debug)]
pub struct EvenementJournal {
    pub id: String,
    pub type_evenement: String,
    pub entite_type: String,
    pub entite_id: String,
    pub auteur_id: String,
    pub ancien_valeur: Option<String>,
    pub nouveau_valeur: Option<String>,
    pub origine: String,
    pub date_evenement: String,
}

/// Écrit un événement dans le journal.
/// Appelée par la porte d'écriture unique — jamais directement par le code métier.
///
/// `type_evenement` : ce qui s'est passé (ex. 'vente_creee', 'prix_modifie',
///                    'stock_ajuste', 'facture_validee'...).
/// `entite_type`    : quelle table est concernée ('vente', 'article', 'stock_depot'...).
/// `entite_id`      : l'UUID de l'entité concernée.
/// `ancien_valeur`  : état avant, sérialisé en JSON (None si création).
/// `nouveau_valeur` : état après, sérialisé en JSON (None si suppression logique).
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
    let id = Uuid::new_v4().to_string();
    let maintenant = maintenant_iso();

    conn.execute(
        "INSERT INTO journal
         (id, type_evenement, entite_type, entite_id, auteur_id,
          ancien_valeur, nouveau_valeur, origine, date_evenement)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            id,
            type_evenement,
            entite_type,
            entite_id,
            auteur_id,
            ancien_valeur,
            nouveau_valeur,
            origine,
            maintenant
        ],
    )?;

    Ok(())
}

/// Lit les événements d'une entité donnée — l'historique complet.
pub fn lire_historique(conn: &Connection, entite_id: &str) -> Result<Vec<EvenementJournal>> {
    let mut stmt = conn.prepare(
        "SELECT id, type_evenement, entite_type, entite_id,
                auteur_id, ancien_valeur, nouveau_valeur, origine, date_evenement
         FROM journal
         WHERE entite_id = ?1
         ORDER BY date_evenement ASC",
    )?;

    let evenements = stmt
        .query_map(params![entite_id], |row| {
            Ok(EvenementJournal {
                id: row.get(0)?,
                type_evenement: row.get(1)?,
                entite_type: row.get(2)?,
                entite_id: row.get(3)?,
                auteur_id: row.get(4)?,
                ancien_valeur: row.get(5)?,
                nouveau_valeur: row.get(6)?,
                origine: row.get(7)?,
                date_evenement: row.get(8)?,
            })
        })?
        .collect::<Result<Vec<_>>>()?;

    Ok(evenements)
}

/// Lit tous les événements d'un type donné — utile pour les rapports d'audit.
/// Exemple : tous les 'prix_modifie' pour voir les changements de prix.
pub fn lire_evenements_par_type(
    conn: &Connection,
    type_evenement: &str,
) -> Result<Vec<EvenementJournal>> {
    let mut stmt = conn.prepare(
        "SELECT id, type_evenement, entite_type, entite_id,
                auteur_id, ancien_valeur, nouveau_valeur, origine, date_evenement
         FROM journal
         WHERE type_evenement = ?1
         ORDER BY date_evenement DESC",
    )?;

    let evenements = stmt
        .query_map(params![type_evenement], |row| {
            Ok(EvenementJournal {
                id: row.get(0)?,
                type_evenement: row.get(1)?,
                entite_type: row.get(2)?,
                entite_id: row.get(3)?,
                auteur_id: row.get(4)?,
                ancien_valeur: row.get(5)?,
                nouveau_valeur: row.get(6)?,
                origine: row.get(7)?,
                date_evenement: row.get(8)?,
            })
        })?
        .collect::<Result<Vec<_>>>()?;

    Ok(evenements)
}

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

    #[test]
    fn ecrire_et_lire_evenement() {
        let conn = base_test();

        ecrire_evenement(
            &conn,
            "vente_creee",
            "vente",
            "vente-uuid-001",
            "user-1",
            None,
            Some(r#"{"total": 5000, "statut": "payee"}"#),
            "m1",
        )
        .unwrap();

        let historique = lire_historique(&conn, "vente-uuid-001").unwrap();
        assert_eq!(historique.len(), 1);
        assert_eq!(historique[0].type_evenement, "vente_creee");
        assert_eq!(historique[0].entite_type, "vente");
        assert!(historique[0].ancien_valeur.is_none());
        assert!(historique[0].nouveau_valeur.is_some());
    }

    #[test]
    fn lire_par_type_evenement() {
        let conn = base_test();

        ecrire_evenement(
            &conn,
            "prix_modifie",
            "article",
            "art-001",
            "user-1",
            Some("500"),
            Some("600"),
            "m1",
        )
        .unwrap();

        ecrire_evenement(
            &conn,
            "vente_creee",
            "vente",
            "vente-001",
            "user-1",
            None,
            Some("{}"),
            "m1",
        )
        .unwrap();

        // On filtre par type — seul le changement de prix doit apparaître.
        let prix = lire_evenements_par_type(&conn, "prix_modifie").unwrap();
        assert_eq!(prix.len(), 1);
        assert_eq!(prix[0].entite_id, "art-001");
        assert_eq!(prix[0].ancien_valeur, Some("500".to_string()));
        assert_eq!(prix[0].nouveau_valeur, Some("600".to_string()));
    }

    #[test]
    fn journal_immuable_aucune_suppression() {
        let conn = base_test();

        ecrire_evenement(
            &conn,
            "stock_ajuste",
            "stock_depot",
            "stock-001",
            "patron",
            Some("10"),
            Some("8"),
            "m1",
        )
        .unwrap();

        // On vérifie qu'il n'y a pas de fonction de suppression —
        // le journal n'expose aucun DELETE. Ce test documente l'intention.
        let historique = lire_historique(&conn, "stock-001").unwrap();
        assert_eq!(historique.len(), 1);
        // Un événement écrit ne peut pas disparaître.
    }
}
