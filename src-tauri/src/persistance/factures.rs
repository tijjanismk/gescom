//! Persistance des factures.
//!
//! Une facture est immuable après validation. La numérotation est séquentielle
//! par année (2026-000001), sans trou, sans renumérotation possible.

use crate::persistance::journal::ecrire_evenement;
use crate::utils::maintenant_iso;
use rusqlite::{params, Connection, Result};
use uuid::Uuid;

#[derive(Debug)]
pub struct Facture {
    pub id: String,
    pub numero: String,
    pub vente_id: String,
    pub statut: String,
    pub total: i64,
    pub date_validation: Option<String>,
}

pub fn creer_facture_brouillon(
    conn: &Connection,
    vente_id: &str,
    total: i64,
    annee: i32,
    cree_par: &str,
    origine: &str,
) -> Result<String> {
    let id = Uuid::new_v4().to_string();
    let maintenant = maintenant_iso();

    let dernier: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM facture WHERE numero LIKE ?1",
            params![format!("{}-%", annee)],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let numero = format!("{}-{:06}", annee, dernier + 1);

    conn.execute(
        "INSERT INTO facture
         (id, numero, vente_id, statut, total,
          cree_le, modifie_le, cree_par, modifie_par, origine)
         VALUES (?1, ?2, ?3, 'brouillon', ?4, ?5, ?6, ?7, ?8, ?9)",
        params![id, numero, vente_id, total, maintenant, maintenant, cree_par, cree_par, origine],
    )?;

    Ok(id)
}

pub fn valider_facture(
    conn: &Connection,
    facture_id: &str,
    valide_par: &str,
    origine: &str,
) -> Result<()> {
    let maintenant = maintenant_iso();

    let modifie = conn.execute(
        "UPDATE facture
         SET statut = 'validee', date_validation = ?1, modifie_le = ?2, modifie_par = ?3
         WHERE id = ?4 AND statut = 'brouillon'",
        params![maintenant, maintenant, valide_par, facture_id],
    )?;

    if modifie == 0 {
        return Err(rusqlite::Error::InvalidParameterName(
            "Facture introuvable ou déjà validée".to_string(),
        ));
    }

    ecrire_evenement(
        conn,
        "facture_validee",
        "facture",
        facture_id,
        valide_par,
        Some("brouillon"),
        Some("validee"),
        origine,
    )?;

    Ok(())
}

pub fn lire_facture(conn: &Connection, facture_id: &str) -> Result<Option<Facture>> {
    let result = conn.query_row(
        "SELECT id, numero, vente_id, statut, total, date_validation
         FROM facture WHERE id = ?1",
        params![facture_id],
        |row| {
            Ok(Facture {
                id: row.get(0)?,
                numero: row.get(1)?,
                vente_id: row.get(2)?,
                statut: row.get(3)?,
                total: row.get(4)?,
                date_validation: row.get(5)?,
            })
        },
    );

    match result {
        Ok(f) => Ok(Some(f)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e),
    }
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

    fn preparer_vente(conn: &Connection) -> String {
        conn.execute(
            "INSERT INTO role (id,nom,permissions,cree_le,modifie_le,origine)
             VALUES ('r1','patron','[]','2024-01-01','2024-01-01','m1')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO utilisateur (id,nom,role_id,actif,cree_le,modifie_le,origine)
             VALUES ('u1','Patron','r1',1,'2024-01-01','2024-01-01','m1')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO client
             (id,code,nom,est_generique,actif,cree_le,modifie_le,cree_par,modifie_par,origine)
             VALUES ('cli1','C0001','Comptant',1,1,'2024-01-01','2024-01-01','u1','u1','m1')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO depot (id,nom,est_defaut,actif,cree_le,modifie_le,origine)
             VALUES ('d1','Principal',1,1,'2024-01-01','2024-01-01','m1')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO vente
             (id,client_id,depot_id,mode_reglement,auteur_id,statut,
              date_vente,cree_le,modifie_le,cree_par,modifie_par,origine)
             VALUES ('v1','cli1','d1','comptant','u1','payee',
                     '2024-01-01','2024-01-01','2024-01-01','u1','u1','m1')",
            [],
        )
        .unwrap();
        "v1".to_string()
    }

    #[test]
    fn numerotation_sequentielle() {
        let conn = base_test();
        let vente_id = preparer_vente(&conn);

        let id1 = creer_facture_brouillon(&conn, &vente_id, 5000, 2026, "u1", "m1").unwrap();
        let id2 = creer_facture_brouillon(&conn, &vente_id, 3000, 2026, "u1", "m1").unwrap();

        let f1 = lire_facture(&conn, &id1).unwrap().unwrap();
        let f2 = lire_facture(&conn, &id2).unwrap().unwrap();

        assert_eq!(f1.numero, "2026-000001");
        assert_eq!(f2.numero, "2026-000002");
    }

    #[test]
    fn validation_rend_immuable() {
        let conn = base_test();
        let vente_id = preparer_vente(&conn);

        let facture_id = creer_facture_brouillon(&conn, &vente_id, 5000, 2026, "u1", "m1").unwrap();

        valider_facture(&conn, &facture_id, "u1", "m1").unwrap();

        let resultat = valider_facture(&conn, &facture_id, "u1", "m1");
        assert!(resultat.is_err());

        let facture = lire_facture(&conn, &facture_id).unwrap().unwrap();
        assert_eq!(facture.statut, "validee");
        assert!(facture.date_validation.is_some());
    }
}
