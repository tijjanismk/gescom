//! Lecture des mouvements de stock.
//! L'écriture se fait via la porte d'écriture unique (ventes.rs).

use crate::utils::maintenant_iso;
use rusqlite::{params, Connection, Result};
use uuid::Uuid;
#[derive(Debug)]
pub struct MouvementStock {
    pub id: String,
    pub article_id: String,
    pub depot_id: String,
    pub type_mouvement: String,
    pub quantite_delta: f64,
    pub motif: Option<String>,
    pub operation_id: Option<String>,
}

/// Lit l'historique des mouvements d'un article dans un dépôt.
pub fn lire_mouvements(
    conn: &Connection,
    article_id: &str,
    depot_id: &str,
) -> Result<Vec<MouvementStock>> {
    let mut stmt = conn.prepare(
        "SELECT id, article_id, depot_id, type_mouvement,
                quantite_delta, motif, operation_id
         FROM mouvement_stock
         WHERE article_id = ?1 AND depot_id = ?2
         ORDER BY date_mouvement DESC",
    )?;

    let mouvements = stmt
        .query_map(params![article_id, depot_id], |row| {
            Ok(MouvementStock {
                id: row.get(0)?,
                article_id: row.get(1)?,
                depot_id: row.get(2)?,
                type_mouvement: row.get(3)?,
                quantite_delta: row.get(4)?,
                motif: row.get(5)?,
                operation_id: row.get(6)?,
            })
        })?
        .collect::<Result<Vec<_>>>()?;

    Ok(mouvements)
}

/// Enregistre un ajustement ou une régularisation de stock.
/// Réservé au patron — motif obligatoire.
pub fn enregistrer_ajustement(
    conn: &mut Connection,
    article_id: &str,
    depot_id: &str,
    delta: f64,
    type_mouvement: &str,
    motif: &str,
    auteur_id: &str,
    origine: &str,
) -> Result<()> {
    let tx = conn.transaction()?;
    let maintenant = maintenant_iso();

    tx.execute(
        "UPDATE stock_depot SET quantite = quantite + ?1
         WHERE article_id = ?2 AND depot_id = ?3",
        params![delta, article_id, depot_id],
    )?;

    tx.execute(
        "INSERT INTO mouvement_stock
         (id, article_id, depot_id, type_mouvement, quantite_delta,
          motif, auteur_id, date_mouvement, cree_le, cree_par, origine)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
        params![
            Uuid::new_v4().to_string(),
            article_id,
            depot_id,
            type_mouvement,
            delta,
            motif,
            auteur_id,
            maintenant,
            maintenant,
            auteur_id,
            origine
        ],
    )?;

    crate::persistance::journal::ecrire_evenement(
        &tx,
        type_mouvement,
        "stock_depot",
        &format!("{}-{}", article_id, depot_id),
        auteur_id,
        None,
        Some(motif),
        origine,
    )?;

    tx.commit()?;
    Ok(())
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

    fn preparer(conn: &Connection) {
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
            "INSERT INTO stock_depot (id,article_id,depot_id,quantite)
             VALUES ('sd1','art1','dep1',10.0)",
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
    }

    #[test]
    fn ajustement_modifie_le_stock() {
        let mut conn = base_test();
        preparer(&conn);

        enregistrer_ajustement(
            &mut conn,
            "art1",
            "dep1",
            -2.0,
            "ajustement",
            "casse",
            "u1",
            "m1",
        )
        .unwrap();

        let stock: f64 = conn
            .query_row(
                "SELECT quantite FROM stock_depot WHERE article_id = 'art1'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(stock, 8.0); // 10 - 2
    }

    #[test]
    fn regularisation_remet_stock_a_zero() {
        let mut conn = base_test();
        preparer(&conn);

        // Stock à -3 (vente à découvert simulée).
        conn.execute(
            "UPDATE stock_depot SET quantite = -3.0 WHERE article_id = 'art1'",
            [],
        )
        .unwrap();

        enregistrer_ajustement(
            &mut conn,
            "art1",
            "dep1",
            3.0,
            "regularisation",
            "achat manquant saisi",
            "u1",
            "m1",
        )
        .unwrap();

        let stock: f64 = conn
            .query_row(
                "SELECT quantite FROM stock_depot WHERE article_id = 'art1'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(stock, 0.0);
    }
}
