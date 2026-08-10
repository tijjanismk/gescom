//! Persistance des avoirs.
//!
//! Un avoir est créé uniquement quand le client repart non soldé sur place.
//! Il est nominatif (rattaché à un client), sans expiration, consommé par
//! remboursement ou par paiement d'une vente (mode = 'avoir').

use crate::persistance::journal::ecrire_evenement;
use crate::utils::maintenant_iso;
use rusqlite::{params, Connection, Result};
use uuid::Uuid;

#[derive(Debug)]
pub struct Avoir {
    pub id: String,
    pub client_id: String,
    pub retour_id: String,
    pub montant: i64,
    pub statut: String,
}

/// Crée un avoir pour un client.
pub fn creer_avoir(
    conn: &Connection,
    client_id: &str,
    retour_id: &str,
    montant: i64,
    auteur_id: &str,
    origine: &str,
) -> Result<String> {
    let id = Uuid::new_v4().to_string();
    let maintenant = maintenant_iso();

    conn.execute(
        "INSERT INTO avoir (id, client_id, retour_id, montant, statut, cree_le, origine)
         VALUES (?1, ?2, ?3, ?4, 'ouvert', ?5, ?6)",
        params![id, client_id, retour_id, montant, maintenant, origine],
    )?;

    ecrire_evenement(
        conn,
        "avoir_cree",
        "avoir",
        &id,
        auteur_id,
        None,
        Some(&format!(
            r#"{{"montant":{}, "client":"{}"}}"#,
            montant, client_id
        )),
        origine,
    )?;

    Ok(id)
}

/// Consomme un avoir — le marque comme utilisé.
pub fn consommer_avoir(
    conn: &Connection,
    avoir_id: &str,
    auteur_id: &str,
    origine: &str,
) -> Result<()> {
    let modifie = conn.execute(
        "UPDATE avoir SET statut = 'consomme' WHERE id = ?1 AND statut = 'ouvert'",
        params![avoir_id],
    )?;

    if modifie == 0 {
        return Err(rusqlite::Error::InvalidParameterName(
            "Avoir introuvable ou déjà consommé".to_string(),
        ));
    }

    ecrire_evenement(
        conn,
        "avoir_consomme",
        "avoir",
        avoir_id,
        auteur_id,
        Some("ouvert"),
        Some("consomme"),
        origine,
    )?;

    Ok(())
}

/// Lit les avoirs ouverts d'un client.
pub fn lire_avoirs_ouverts(conn: &Connection, client_id: &str) -> Result<Vec<Avoir>> {
    let mut stmt = conn.prepare(
        "SELECT id, client_id, retour_id, montant, statut
         FROM avoir
         WHERE client_id = ?1 AND statut = 'ouvert'
         ORDER BY cree_le ASC",
    )?;

    let avoirs = stmt
        .query_map(params![client_id], |row| {
            Ok(Avoir {
                id: row.get(0)?,
                client_id: row.get(1)?,
                retour_id: row.get(2)?,
                montant: row.get(3)?,
                statut: row.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>>>()?;

    Ok(avoirs)
}

/// Somme des avoirs ouverts d'un client.
pub fn total_avoirs_ouverts(conn: &Connection, client_id: &str) -> Result<i64> {
    let total: i64 = conn.query_row(
        "SELECT COALESCE(SUM(montant), 0) FROM avoir
         WHERE client_id = ?1 AND statut = 'ouvert'",
        params![client_id],
        |row| row.get(0),
    )?;

    Ok(total)
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

    fn preparer(conn: &Connection) -> (String, String) {
        conn.execute(
            "INSERT INTO role (id,nom,permissions,cree_le,modifie_le,origine)
             VALUES ('r1','patron','[]','2024-01-01','2024-01-01','m1')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO utilisateur
             (id,nom,role_id,actif,cree_le,modifie_le,origine)
             VALUES ('u1','Patron','r1',1,'2024-01-01','2024-01-01','m1')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO client
             (id,code,nom,est_generique,actif,cree_le,modifie_le,
              cree_par,modifie_par,origine)
             VALUES ('cli1','CLIENT00001','Amadou',0,1,
                     '2024-01-01','2024-01-01','u1','u1','m1')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO categorie (id,nom,schema_attributs,actif,cree_le,modifie_le,origine)
             VALUES ('cat1','Alim','[]',1,'2024-01-01','2024-01-01','m1')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO article
             (id,nom,categorie_id,unite_base,gere_en_stock,attributs,actif,
              cree_le,modifie_le,cree_par,modifie_par,origine)
             VALUES ('art1','Sucre','cat1','kg',1,'{}',1,
                     '2024-01-01','2024-01-01','u1','u1','m1')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO unite_vente
             (id,article_id,libelle,facteur,prix_reference,actif,
              cree_le,modifie_le,cree_par,modifie_par,origine)
             VALUES ('uv1','art1','kg',1.0,800,1,
                     '2024-01-01','2024-01-01','u1','u1','m1')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO depot (id,nom,est_defaut,actif,cree_le,modifie_le,origine)
             VALUES ('dep1','Principal',1,1,'2024-01-01','2024-01-01','m1')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO vente
             (id,client_id,depot_id,mode_reglement,auteur_id,statut,
              date_vente,cree_le,modifie_le,cree_par,modifie_par,origine)
             VALUES ('v1','cli1','dep1','comptant','u1','payee',
                     '2024-01-01','2024-01-01','2024-01-01','u1','u1','m1')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO retour
             (id,vente_id,article_id,unite_vente_id,quantite,
              depot_reintegration_id,mode_resolution,montant_credit,
              reliquat,reliquat_resolution,auteur_id,date_retour,
              cree_le,cree_par,origine)
             VALUES ('ret1','v1','art1','uv1',1.0,'dep1',
                     'avoir_conserve',800,0,'aucun',
                     'u1','2024-01-01','2024-01-01','u1','m1')",
            [],
        )
        .unwrap();
        ("cli1".into(), "ret1".into())
    }

    #[test]
    fn creer_et_lire_avoir() {
        let conn = base_test();
        let (client_id, retour_id) = preparer(&conn);

        creer_avoir(&conn, &client_id, &retour_id, 800, "u1", "m1").unwrap();

        let avoirs = lire_avoirs_ouverts(&conn, &client_id).unwrap();
        assert_eq!(avoirs.len(), 1);
        assert_eq!(avoirs[0].montant, 800);
        assert_eq!(avoirs[0].statut, "ouvert");
    }

    #[test]
    fn consommer_avoir_fonctionne() {
        let conn = base_test();
        let (client_id, retour_id) = preparer(&conn);

        let avoir_id = creer_avoir(&conn, &client_id, &retour_id, 800, "u1", "m1").unwrap();

        consommer_avoir(&conn, &avoir_id, "u1", "m1").unwrap();

        let avoirs = lire_avoirs_ouverts(&conn, &client_id).unwrap();
        assert_eq!(avoirs.len(), 0);
    }

    #[test]
    fn double_consommation_interdite() {
        let conn = base_test();
        let (client_id, retour_id) = preparer(&conn);

        let avoir_id = creer_avoir(&conn, &client_id, &retour_id, 800, "u1", "m1").unwrap();

        consommer_avoir(&conn, &avoir_id, "u1", "m1").unwrap();

        let resultat = consommer_avoir(&conn, &avoir_id, "u1", "m1");
        assert!(resultat.is_err());
    }

    #[test]
    fn total_avoirs_ouverts_somme_correctement() {
        let conn = base_test();
        let (client_id, retour_id) = preparer(&conn);

        creer_avoir(&conn, &client_id, &retour_id, 800, "u1", "m1").unwrap();
        creer_avoir(&conn, &client_id, &retour_id, 1200, "u1", "m1").unwrap();

        let total = total_avoirs_ouverts(&conn, &client_id).unwrap();
        assert_eq!(total, 2000);
    }
}
