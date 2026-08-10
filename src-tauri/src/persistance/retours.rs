//! Persistance des retours.
//!
//! Un retour est toujours tracé (stock + caisse), quel que soit le mode
//! de résolution. L'avoir n'est créé que si le client repart non soldé.
//!
//! Trois modes de résolution :
//!   - remboursement  : le crédit est rendu en argent (sortie de caisse)
//!   - echange        : le crédit finance une vente de remplacement
//!   - avoir_conserve : le client repart avec un crédit à utiliser plus tard

use crate::persistance::journal::ecrire_evenement;
use crate::utils::maintenant_iso;
use rusqlite::{params, Connection, Result};
use uuid::Uuid;

#[derive(Debug)]
pub struct Retour {
    pub id: String,
    pub vente_id: String,
    pub article_id: String,
    pub unite_vente_id: String,
    pub quantite: f64,
    pub depot_reintegration_id: String,
    pub mode_resolution: String,
    pub vente_remplacement_id: Option<String>,
    pub montant_credit: i64,
    pub reliquat: i64,
    pub reliquat_resolution: String,
}

/// Enregistre un retour complet dans une transaction unique.
///
/// `prix_pratique_origine` : le prix réellement payé à l'origine
///                           (on rembourse ce qui a été payé, pas le tarif).
/// `vente_remplacement_id` : rempli si mode = 'echange'.
/// `reliquat`              : positif = crédit au client, négatif = client complète.
/// `reliquat_resolution`   : 'especes'/'orange_money'/'moov_money'/'avoir'/'aucun'.
pub fn inserer_retour(
    conn: &mut Connection,
    vente_id: &str,
    article_id: &str,
    unite_vente_id: &str,
    quantite: f64,
    facteur: f64,
    depot_reintegration_id: &str,
    mode_resolution: &str,
    vente_remplacement_id: Option<&str>,
    prix_pratique_origine: i64,
    montant_remplacement: i64,
    reliquat_resolution: &str,
    auteur_id: &str,
    origine: &str,
) -> Result<String> {
    let tx = conn.transaction()?;
    let id = Uuid::new_v4().to_string();
    let maintenant = maintenant_iso();

    // Calcul du crédit et du reliquat via le cœur.
    let montant_credit = crate::coeur::stock::credit_retour(prix_pratique_origine, quantite);
    let reliquat = crate::coeur::stock::reliquat_echange(montant_credit, montant_remplacement);

    // Réintégration du stock (toujours, quel que soit le mode).
    let quantite_base = quantite * facteur;
    tx.execute(
        "INSERT INTO stock_depot (id, article_id, depot_id, quantite)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(article_id, depot_id)
         DO UPDATE SET quantite = quantite + ?4",
        params![
            Uuid::new_v4().to_string(),
            article_id,
            depot_reintegration_id,
            quantite_base
        ],
    )?;

    // Mouvement de stock (entrée = positif).
    tx.execute(
        "INSERT INTO mouvement_stock
         (id, article_id, depot_id, type_mouvement, quantite_delta,
          operation_id, auteur_id, date_mouvement, cree_le, cree_par, origine)
         VALUES (?1,?2,?3,'retour',?4,?5,?6,?7,?8,?9,?10)",
        params![
            Uuid::new_v4().to_string(),
            article_id,
            depot_reintegration_id,
            quantite_base,
            id,
            auteur_id,
            maintenant,
            maintenant,
            auteur_id,
            origine
        ],
    )?;

    // Insertion du retour.
    tx.execute(
        "INSERT INTO retour
         (id, vente_id, article_id, unite_vente_id, quantite,
          depot_reintegration_id, mode_resolution, vente_remplacement_id,
          montant_credit, reliquat, reliquat_resolution,
          auteur_id, date_retour, cree_le, cree_par, origine)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)",
        params![
            id,
            vente_id,
            article_id,
            unite_vente_id,
            quantite,
            depot_reintegration_id,
            mode_resolution,
            vente_remplacement_id,
            montant_credit,
            reliquat,
            reliquat_resolution,
            auteur_id,
            maintenant,
            maintenant,
            auteur_id,
            origine
        ],
    )?;

    // Journal.
    ecrire_evenement(
        &tx,
        "retour_enregistre",
        "retour",
        &id,
        auteur_id,
        None,
        Some(&format!(
            r#"{{"mode":"{}","credit":{},"reliquat":{}}}"#,
            mode_resolution, montant_credit, reliquat
        )),
        origine,
    )?;

    tx.commit()?;
    Ok(id)
}

/// Lit un retour par son id.
pub fn lire_retour(conn: &Connection, retour_id: &str) -> Result<Option<Retour>> {
    let result = conn.query_row(
        "SELECT id, vente_id, article_id, unite_vente_id, quantite,
                depot_reintegration_id, mode_resolution, vente_remplacement_id,
                montant_credit, reliquat, reliquat_resolution
         FROM retour WHERE id = ?1",
        params![retour_id],
        |row| {
            Ok(Retour {
                id: row.get(0)?,
                vente_id: row.get(1)?,
                article_id: row.get(2)?,
                unite_vente_id: row.get(3)?,
                quantite: row.get(4)?,
                depot_reintegration_id: row.get(5)?,
                mode_resolution: row.get(6)?,
                vente_remplacement_id: row.get(7)?,
                montant_credit: row.get(8)?,
                reliquat: row.get(9)?,
                reliquat_resolution: row.get(10)?,
            })
        },
    );

    match result {
        Ok(r) => Ok(Some(r)),
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

    fn preparer(conn: &Connection) -> (String, String, String, String) {
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
        conn.execute(
            "INSERT INTO client
             (id,code,nom,est_generique,actif,cree_le,modifie_le,
              cree_par,modifie_par,origine)
             VALUES ('cli1','C0001','Comptant',1,1,
                     '2024-01-01','2024-01-01','u1','u1','m1')",
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
        ("art1".into(), "uv1".into(), "dep1".into(), "v1".into())
    }

    #[test]
    fn retour_reinteagre_le_stock() {
        let mut conn = base_test();
        let (art, uv, dep, vente) = preparer(&conn);

        inserer_retour(
            &mut conn,
            &vente,
            &art,
            &uv,
            2.0, // 2 kg rendus
            1.0, // facteur 1
            &dep,
            "remboursement",
            None,
            800, // prix payé à l'origine
            0,   // pas de remplacement
            "aucun",
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

        // Stock passe de 10 à 12 (retour de 2 kg).
        assert_eq!(stock, 12.0);
    }

    #[test]
    fn credit_retour_calcule_correctement() {
        let mut conn = base_test();
        let (art, uv, dep, vente) = preparer(&conn);

        let retour_id = inserer_retour(
            &mut conn,
            &vente,
            &art,
            &uv,
            3.0,
            1.0,
            &dep,
            "remboursement",
            None,
            800,
            0,
            "aucun",
            "u1",
            "m1",
        )
        .unwrap();

        let retour = lire_retour(&conn, &retour_id).unwrap().unwrap();
        // 3 kg × 800 = 2400
        assert_eq!(retour.montant_credit, 2400);
    }

    #[test]
    fn echange_avec_reliquat_positif() {
        let mut conn = base_test();
        let (art, uv, dep, vente) = preparer(&conn);

        let retour_id = inserer_retour(
            &mut conn,
            &vente,
            &art,
            &uv,
            2.0,
            1.0,
            &dep,
            "echange",
            Some("v1"), // vente de remplacement
            800,        // crédit = 2 × 800 = 1600
            1000,       // article repris à 1000
            "especes",  // reliquat de 600 rendu en espèces
            "u1",
            "m1",
        )
        .unwrap();

        let retour = lire_retour(&conn, &retour_id).unwrap().unwrap();
        assert_eq!(retour.montant_credit, 1600);
        assert_eq!(retour.reliquat, 600); // 1600 - 1000
        assert_eq!(retour.reliquat_resolution, "especes");
    }

    #[test]
    fn echange_avec_complement_negatif() {
        let mut conn = base_test();
        let (art, uv, dep, vente) = preparer(&conn);

        let retour_id = inserer_retour(
            &mut conn,
            &vente,
            &art,
            &uv,
            1.0,
            1.0,
            &dep,
            "echange",
            Some("v1"),
            800,  // crédit = 800
            1500, // article repris plus cher
            "aucun",
            "u1",
            "m1",
        )
        .unwrap();

        let retour = lire_retour(&conn, &retour_id).unwrap().unwrap();
        assert_eq!(retour.reliquat, -700); // le client complète 700
        assert!(retour.reliquat < 0);
    }
}
