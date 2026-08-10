//! Persistance des transferts inter-dépôts.
//!
//! Un transfert est neutre (sans prix) entre dépôts du même propriétaire.
//! Il est atomique : sortie source + entrée destination dans la même transaction.

use crate::utils::maintenant_iso;
use rusqlite::{params, Connection, Result};
use uuid::Uuid;
pub fn inserer_transfert(
    conn: &mut Connection,
    depot_source_id: &str,
    depot_dest_id: &str,
    article_id: &str,
    quantite: f64,
    auteur_id: &str,
    origine: &str,
) -> Result<String> {
    let tx = conn.transaction()?;
    let id = Uuid::new_v4().to_string();
    let maintenant = maintenant_iso();

    // Sortie du dépôt source.
    tx.execute(
        "UPDATE stock_depot SET quantite = quantite - ?1
         WHERE article_id = ?2 AND depot_id = ?3",
        params![quantite, article_id, depot_source_id],
    )?;

    // Entrée dans le dépôt destination.
    tx.execute(
        "INSERT INTO stock_depot (id, article_id, depot_id, quantite)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(article_id, depot_id)
         DO UPDATE SET quantite = quantite + ?4",
        params![
            Uuid::new_v4().to_string(),
            article_id,
            depot_dest_id,
            quantite
        ],
    )?;

    // Enregistrement du transfert.
    tx.execute(
        "INSERT INTO transfert
         (id, depot_source_id, depot_dest_id, article_id, quantite,
          auteur_id, date_transfert, cree_le, cree_par, origine)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
        params![
            id,
            depot_source_id,
            depot_dest_id,
            article_id,
            quantite,
            auteur_id,
            maintenant,
            maintenant,
            auteur_id,
            origine
        ],
    )?;

    // Deux mouvements de stock (sortie + entrée).
    for (depot, delta) in [(depot_source_id, -quantite), (depot_dest_id, quantite)] {
        tx.execute(
            "INSERT INTO mouvement_stock
             (id, article_id, depot_id, type_mouvement, quantite_delta,
              operation_id, auteur_id, date_mouvement, cree_le, cree_par, origine)
             VALUES (?1,?2,?3,'transfert',?4,?5,?6,?7,?8,?9,?10)",
            params![
                Uuid::new_v4().to_string(),
                article_id,
                depot,
                delta,
                id,
                auteur_id,
                maintenant,
                maintenant,
                auteur_id,
                origine
            ],
        )?;
    }

    crate::persistance::journal::ecrire_evenement(
        &tx,
        "transfert_effectue",
        "transfert",
        &id,
        auteur_id,
        None,
        Some(&format!(
            r#"{{"article":"{}","quantite":{},"de":"{}","vers":"{}"}}"#,
            article_id, quantite, depot_source_id, depot_dest_id
        )),
        origine,
    )?;

    tx.commit()?;
    Ok(id)
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

    fn preparer(conn: &Connection) -> (String, String, String) {
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
            "INSERT INTO depot (id,nom,est_defaut,actif,cree_le,modifie_le,origine)
             VALUES ('dep1','Principal',1,1,'2024-01-01','2024-01-01','m1')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO depot (id,nom,est_defaut,actif,cree_le,modifie_le,origine)
             VALUES ('dep2','Secondaire',0,1,'2024-01-01','2024-01-01','m1')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO stock_depot (id,article_id,depot_id,quantite)
             VALUES ('sd1','art1','dep1',50.0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO stock_depot (id,article_id,depot_id,quantite)
             VALUES ('sd2','art1','dep2',0.0)",
            [],
        )
        .unwrap();
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
        ("art1".into(), "dep1".into(), "dep2".into())
    }

    #[test]
    fn transfert_deplace_le_stock() {
        let mut conn = base_test();
        let (art, dep1, dep2) = preparer(&conn);

        inserer_transfert(&mut conn, &dep1, &dep2, &art, 20.0, "u1", "m1").unwrap();

        let stock1: f64 = conn
            .query_row(
                "SELECT quantite FROM stock_depot WHERE depot_id = 'dep1'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        let stock2: f64 = conn
            .query_row(
                "SELECT quantite FROM stock_depot WHERE depot_id = 'dep2'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(stock1, 30.0);
        assert_eq!(stock2, 20.0);
    }

    #[test]
    fn transfert_genere_deux_mouvements() {
        let mut conn = base_test();
        let (art, dep1, dep2) = preparer(&conn);

        inserer_transfert(&mut conn, &dep1, &dep2, &art, 10.0, "u1", "m1").unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM mouvement_stock WHERE type_mouvement = 'transfert'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(count, 2);
    }
}
