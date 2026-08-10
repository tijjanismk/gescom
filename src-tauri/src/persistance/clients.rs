//! Persistance des clients.
//!
//! Le client générique « Comptant » (est_generique = true) est unique —
//! réutilisé pour toutes les ventes anonymes. Les vrais clients ont un
//! code séquentiel (CLIENT00001...) et un nom.

use crate::utils::maintenant_iso;
use rusqlite::{params, Connection, Result};
use uuid::Uuid;

#[derive(Debug)]
pub struct Client {
    pub id: String,
    pub code: String,
    pub nom: String,
    pub telephone: Option<String>,
    pub nif: Option<String>,
    pub adresse: Option<String>,
    pub plafond_credit: Option<i64>,
    pub est_generique: bool,
    pub actif: bool,
}

/// Insère le client générique « Comptant » — appelé une seule fois à l'init.
/// Si le client générique existe déjà, ne fait rien (INSERT OR IGNORE).
pub fn inserer_client_generique(conn: &Connection, origine: &str) -> Result<String> {
    let id = Uuid::new_v4().to_string();
    let maintenant = maintenant_iso();

    conn.execute(
        "INSERT OR IGNORE INTO client
         (id, code, nom, est_generique, actif,
          cree_le, modifie_le, cree_par, modifie_par, origine)
         VALUES (?1, 'CLIENT00000', 'Comptant', 1, 1,
                 ?2, ?3, 'system', 'system', ?4)",
        params![id, maintenant, maintenant, origine],
    )?;

    // Retourner l'id du générique (qu'il vienne d'être créé ou qu'il existait).
    let id_generique: String = conn.query_row(
        "SELECT id FROM client WHERE est_generique = 1 LIMIT 1",
        [],
        |row| row.get(0),
    )?;

    Ok(id_generique)
}

/// Insère un vrai client identifié.
/// Le code est généré automatiquement en séquence (CLIENT00001, CLIENT00002...).
pub fn inserer_client(
    conn: &Connection,
    nom: &str,
    telephone: Option<&str>,
    nif: Option<&str>,
    adresse: Option<&str>,
    plafond_credit: Option<i64>,
    cree_par: &str,
    origine: &str,
) -> Result<String> {
    let id = Uuid::new_v4().to_string();
    let maintenant = maintenant_iso();

    // Calcul du prochain code séquentiel.
    // On exclut le générique (CLIENT00000) du comptage.
    let dernier_numero: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM client WHERE est_generique = 0",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let code = format!("CLIENT{:05}", dernier_numero + 1);

    conn.execute(
        "INSERT INTO client
         (id, code, nom, telephone, nif, adresse, plafond_credit,
          est_generique, actif, cree_le, modifie_le, cree_par, modifie_par, origine)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, 1, ?8, ?9, ?10, ?11, ?12)",
        params![
            id,
            code,
            nom,
            telephone,
            nif,
            adresse,
            plafond_credit,
            maintenant,
            maintenant,
            cree_par,
            cree_par,
            origine
        ],
    )?;

    Ok(id)
}

/// Lit tous les clients actifs (hors générique).
pub fn lire_clients(conn: &Connection) -> Result<Vec<Client>> {
    let mut stmt = conn.prepare(
        "SELECT id, code, nom, telephone, nif, adresse,
                plafond_credit, est_generique, actif
         FROM client
         WHERE actif = 1 AND est_generique = 0
         ORDER BY code",
    )?;

    let clients = stmt
        .query_map([], |row| {
            Ok(Client {
                id: row.get(0)?,
                code: row.get(1)?,
                nom: row.get(2)?,
                telephone: row.get(3)?,
                nif: row.get(4)?,
                adresse: row.get(5)?,
                plafond_credit: row.get(6)?,
                est_generique: row.get::<_, i64>(7)? != 0,
                actif: row.get::<_, i64>(8)? != 0,
            })
        })?
        .collect::<Result<Vec<_>>>()?;

    Ok(clients)
}

/// Retourne l'id du client générique « Comptant ».
/// Utilisé pour les ventes sans client identifié.
pub fn id_client_generique(conn: &Connection) -> Result<String> {
    conn.query_row(
        "SELECT id FROM client WHERE est_generique = 1 LIMIT 1",
        [],
        |row| row.get(0),
    )
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
    fn client_generique_cree_une_seule_fois() {
        let conn = base_test();

        // Deux appels — le deuxième ne crée pas de doublon.
        inserer_client_generique(&conn, "m1").unwrap();
        inserer_client_generique(&conn, "m1").unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM client WHERE est_generique = 1",
                [],
                |r| r.get(0),
            )
            .unwrap();

        assert_eq!(count, 1);
    }

    #[test]
    fn inserer_et_lire_client() {
        let conn = base_test();
        inserer_client_generique(&conn, "m1").unwrap();

        inserer_client(
            &conn,
            "Amadou Diarra",
            Some("76000001"),
            None,
            None,
            Some(500_000),
            "user-1",
            "m1",
        )
        .unwrap();

        let clients = lire_clients(&conn).unwrap();
        assert_eq!(clients.len(), 1);
        assert_eq!(clients[0].nom, "Amadou Diarra");
        assert_eq!(clients[0].code, "CLIENT00001");
        assert_eq!(clients[0].plafond_credit, Some(500_000));
    }

    #[test]
    fn codes_sequentiels() {
        let conn = base_test();
        inserer_client_generique(&conn, "m1").unwrap();

        inserer_client(&conn, "Client A", None, None, None, None, "u1", "m1").unwrap();
        inserer_client(&conn, "Client B", None, None, None, None, "u1", "m1").unwrap();
        inserer_client(&conn, "Client C", None, None, None, None, "u1", "m1").unwrap();

        let clients = lire_clients(&conn).unwrap();
        assert_eq!(clients[0].code, "CLIENT00001");
        assert_eq!(clients[1].code, "CLIENT00002");
        assert_eq!(clients[2].code, "CLIENT00003");
    }

    #[test]
    fn generique_non_retourne_dans_lire_clients() {
        let conn = base_test();
        inserer_client_generique(&conn, "m1").unwrap();

        let clients = lire_clients(&conn).unwrap();
        // Le générique n'apparaît pas dans la liste des clients.
        assert_eq!(clients.len(), 0);
    }
}
