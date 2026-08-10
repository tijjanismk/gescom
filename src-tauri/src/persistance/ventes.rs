//! Persistance des ventes et lignes de vente.
//!
//! La vente est l'acte atomique central du système. Elle décrémente le stock
//! et crée une créance si non payée immédiatement.
//!
//! Deux étapes distinctes et séparées :
//!   1. inserer_vente_complete  — crée la vente, décrémente le stock, journalise.
//!   2. enregistrer_paiement    — encaisse, met à jour le statut, journalise.
//!
//! Pour une vente comptant : enchaîner les deux immédiatement.
//! Pour une vente à crédit : s'arrêter après l'étape 1.

use crate::coeur::calcul::{montant_ligne, total_vente};
use crate::coeur::stock::est_a_decouvert;
use crate::persistance::journal::ecrire_evenement;
use crate::utils::maintenant_iso;
use rusqlite::{params, Connection, Result};
use uuid::Uuid;

/// Une vente telle qu'elle vit en base.
#[derive(Debug)]
pub struct Vente {
    pub id: String,
    pub client_id: String,
    pub depot_id: String,
    pub mode_reglement: String,
    pub auteur_id: String,
    pub statut: String,
    pub date_vente: String,
}

/// Une ligne de vente.
#[derive(Debug)]
pub struct LigneVente {
    pub id: String,
    pub vente_id: String,
    pub article_id: String,
    pub unite_vente_id: String,
    pub depot_source_id: String,
    pub source_approvisionnement: String,
    pub vente_a_decouvert: bool,
    pub quantite: f64,
    pub prix_reference: i64,
    pub prix_pratique: i64,
}

/// Paramètres d'une ligne pour la création d'une vente.
pub struct ParamsLigne<'a> {
    pub article_id: &'a str,
    pub unite_vente_id: &'a str,
    pub depot_source_id: &'a str,
    pub source_approvisionnement: &'a str, // 'stock' / 'voisin'
    pub quantite: f64,
    pub facteur: f64,
    pub prix_reference: i64,
    pub prix_pratique: i64,
}

/// Crée une vente complète dans une transaction unique.
///
/// Le statut initial est toujours 'creance_ouverte' — c'est enregistrer_paiement
/// qui le fera évoluer vers 'partiellement_payee' ou 'payee'.
/// Pour une vente comptant, enchaîner immédiatement avec enregistrer_paiement.
pub fn inserer_vente_complete(
    conn: &mut Connection,
    client_id: &str,
    depot_id: &str,
    mode_reglement: &str,
    lignes: &[ParamsLigne],
    auteur_id: &str,
    origine: &str,
) -> Result<String> {
    let tx = conn.transaction()?;
    let vente_id = Uuid::new_v4().to_string();
    let maintenant = maintenant_iso();

    // Calcul du total via le cœur.
    let montants: Vec<i64> = lignes
        .iter()
        .map(|l| montant_ligne(l.prix_pratique, l.quantite))
        .collect();

    let total = total_vente(&montants);

    // Statut initial toujours creance_ouverte.
    // enregistrer_paiement le fera évoluer.
    let statut = "creance_ouverte";

    // Insertion de la vente.
    tx.execute(
        "INSERT INTO vente
         (id, client_id, depot_id, mode_reglement, auteur_id, statut,
          date_vente, cree_le, modifie_le, cree_par, modifie_par, origine)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
        params![
            vente_id,
            client_id,
            depot_id,
            mode_reglement,
            auteur_id,
            statut,
            maintenant,
            maintenant,
            maintenant,
            auteur_id,
            auteur_id,
            origine
        ],
    )?;

    // Insertion des lignes + décrément stock.
    for ligne in lignes.iter() {
        let ligne_id = Uuid::new_v4().to_string();

        // Détection du découvert avant décrément.
        let a_decouvert = if ligne.source_approvisionnement == "stock" {
            let stock_actuel: f64 = tx
                .query_row(
                    "SELECT COALESCE(quantite, 0) FROM stock_depot
                     WHERE article_id = ?1 AND depot_id = ?2",
                    params![ligne.article_id, ligne.depot_source_id],
                    |row| row.get(0),
                )
                .unwrap_or(0.0);

            est_a_decouvert(stock_actuel, ligne.quantite * ligne.facteur)
        } else {
            false
        };

        tx.execute(
            "INSERT INTO ligne_vente
             (id, vente_id, article_id, unite_vente_id, depot_source_id,
              source_approvisionnement, vente_a_decouvert, quantite,
              prix_reference, prix_pratique, cree_le, origine)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
            params![
                ligne_id,
                vente_id,
                ligne.article_id,
                ligne.unite_vente_id,
                ligne.depot_source_id,
                ligne.source_approvisionnement,
                a_decouvert as i64,
                ligne.quantite,
                ligne.prix_reference,
                ligne.prix_pratique,
                maintenant,
                origine
            ],
        )?;

        // Décrément stock (seulement si source = stock).
        if ligne.source_approvisionnement == "stock" {
            let quantite_base = ligne.quantite * ligne.facteur;

            let stock_actuel: f64 = tx
                .query_row(
                    "SELECT COALESCE(quantite, 0) FROM stock_depot
                     WHERE article_id = ?1 AND depot_id = ?2",
                    params![ligne.article_id, ligne.depot_source_id],
                    |row| row.get(0),
                )
                .unwrap_or(0.0);

            let nouveau_stock = stock_actuel - quantite_base;

            tx.execute(
                "INSERT INTO stock_depot (id, article_id, depot_id, quantite)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(article_id, depot_id)
                 DO UPDATE SET quantite = ?4",
                params![
                    Uuid::new_v4().to_string(),
                    ligne.article_id,
                    ligne.depot_source_id,
                    nouveau_stock
                ],
            )?;

            // Mouvement de stock.
            tx.execute(
                "INSERT INTO mouvement_stock
                 (id, article_id, depot_id, type_mouvement, quantite_delta,
                  operation_id, auteur_id, date_mouvement, cree_le, cree_par, origine)
                 VALUES (?1,?2,?3,'vente',?4,?5,?6,?7,?8,?9,?10)",
                params![
                    Uuid::new_v4().to_string(),
                    ligne.article_id,
                    ligne.depot_source_id,
                    -quantite_base,
                    vente_id,
                    auteur_id,
                    maintenant,
                    maintenant,
                    auteur_id,
                    origine
                ],
            )?;
        }

        // Tracer la remise si prix négocié en dessous du référence.
        if ligne.prix_pratique < ligne.prix_reference {
            ecrire_evenement(
                &tx,
                "remise_accordee",
                "ligne_vente",
                &ligne_id,
                auteur_id,
                Some(&ligne.prix_reference.to_string()),
                Some(&ligne.prix_pratique.to_string()),
                origine,
            )?;
        }
    }

    // Journal de la vente.
    ecrire_evenement(
        &tx,
        "vente_creee",
        "vente",
        &vente_id,
        auteur_id,
        None,
        Some(&format!(r#"{{"total":{}}}"#, total)),
        origine,
    )?;

    tx.commit()?;
    Ok(vente_id)
}

/// Lit une vente par son id.
pub fn lire_vente(conn: &Connection, vente_id: &str) -> Result<Option<Vente>> {
    let result = conn.query_row(
        "SELECT id, client_id, depot_id, mode_reglement,
                auteur_id, statut, date_vente
         FROM vente WHERE id = ?1",
        params![vente_id],
        |row| {
            Ok(Vente {
                id: row.get(0)?,
                client_id: row.get(1)?,
                depot_id: row.get(2)?,
                mode_reglement: row.get(3)?,
                auteur_id: row.get(4)?,
                statut: row.get(5)?,
                date_vente: row.get(6)?,
            })
        },
    );

    match result {
        Ok(v) => Ok(Some(v)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e),
    }
}

/// Lit les lignes d'une vente.
pub fn lire_lignes_vente(conn: &Connection, vente_id: &str) -> Result<Vec<LigneVente>> {
    let mut stmt = conn.prepare(
        "SELECT id, vente_id, article_id, unite_vente_id, depot_source_id,
                source_approvisionnement, vente_a_decouvert, quantite,
                prix_reference, prix_pratique
         FROM ligne_vente WHERE vente_id = ?1",
    )?;

    let lignes = stmt
        .query_map(params![vente_id], |row| {
            Ok(LigneVente {
                id: row.get(0)?,
                vente_id: row.get(1)?,
                article_id: row.get(2)?,
                unite_vente_id: row.get(3)?,
                depot_source_id: row.get(4)?,
                source_approvisionnement: row.get(5)?,
                vente_a_decouvert: row.get::<_, i64>(6)? != 0,
                quantite: row.get(7)?,
                prix_reference: row.get(8)?,
                prix_pratique: row.get(9)?,
            })
        })?
        .collect::<Result<Vec<_>>>()?;

    Ok(lignes)
}

/// Calcule le total d'une vente depuis ses lignes en base.
pub fn total_vente_depuis_base(conn: &Connection, vente_id: &str) -> Result<i64> {
    let total: i64 = conn.query_row(
        "SELECT COALESCE(SUM(CAST(prix_pratique AS REAL) * quantite), 0)
         FROM ligne_vente WHERE vente_id = ?1",
        params![vente_id],
        |row| row.get(0),
    )?;
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistance::initialiser_tables;
    use crate::persistance::paiements::enregistrer_paiement;
    use rusqlite::Connection;

    fn base_test() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        initialiser_tables(&conn).unwrap();
        conn
    }

    fn preparer(conn: &mut Connection) -> (String, String, String, String, String) {
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
             VALUES ('sd1','art1','dep1',100.0)",
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
             VALUES ('u1','Amadou','r1',1,'2024-01-01','2024-01-01','m1')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO client
             (id,code,nom,est_generique,actif,cree_le,modifie_le,
              cree_par,modifie_par,origine)
             VALUES ('cli1','CLIENT00000','Comptant',1,1,
                     '2024-01-01','2024-01-01','system','system','m1')",
            [],
        )
        .unwrap();
        (
            "art1".into(),
            "uv1".into(),
            "dep1".into(),
            "u1".into(),
            "cli1".into(),
        )
    }

    #[test]
    fn vente_creee_en_creance_ouverte() {
        let mut conn = base_test();
        let (art, uv, dep, user, cli) = preparer(&mut conn);

        let ligne = ParamsLigne {
            article_id: &art,
            unite_vente_id: &uv,
            depot_source_id: &dep,
            source_approvisionnement: "stock",
            quantite: 5.0,
            facteur: 1.0,
            prix_reference: 800,
            prix_pratique: 800,
        };

        let vente_id =
            inserer_vente_complete(&mut conn, &cli, &dep, "comptant", &[ligne], &user, "m1")
                .unwrap();

        // Statut initial = creance_ouverte, peu importe le mode_reglement.
        let vente = lire_vente(&conn, &vente_id).unwrap().unwrap();
        assert_eq!(vente.statut, "creance_ouverte");
    }

    #[test]
    fn vente_comptant_payee_apres_paiement() {
        let mut conn = base_test();
        let (art, uv, dep, user, cli) = preparer(&mut conn);

        let ligne = ParamsLigne {
            article_id: &art,
            unite_vente_id: &uv,
            depot_source_id: &dep,
            source_approvisionnement: "stock",
            quantite: 5.0,
            facteur: 1.0,
            prix_reference: 800,
            prix_pratique: 800,
        };

        // Étape 1 : créer la vente.
        let vente_id =
            inserer_vente_complete(&mut conn, &cli, &dep, "comptant", &[ligne], &user, "m1")
                .unwrap();

        // Étape 2 : encaisser le total (5 × 800 = 4000).
        enregistrer_paiement(&mut conn, &vente_id, 4000, "especes", None, &user, "m1").unwrap();

        let vente = lire_vente(&conn, &vente_id).unwrap().unwrap();
        assert_eq!(vente.statut, "payee");
    }

    #[test]
    fn vente_credit_reste_creance_sans_paiement() {
        let mut conn = base_test();
        let (art, uv, dep, user, cli) = preparer(&mut conn);

        let ligne = ParamsLigne {
            article_id: &art,
            unite_vente_id: &uv,
            depot_source_id: &dep,
            source_approvisionnement: "stock",
            quantite: 10.0,
            facteur: 1.0,
            prix_reference: 800,
            prix_pratique: 800,
        };

        let vente_id =
            inserer_vente_complete(&mut conn, &cli, &dep, "credit", &[ligne], &user, "m1").unwrap();

        // Aucun paiement -> reste creance_ouverte.
        let vente = lire_vente(&conn, &vente_id).unwrap().unwrap();
        assert_eq!(vente.statut, "creance_ouverte");
    }

    #[test]
    fn paiement_partiel_statut_partiel() {
        let mut conn = base_test();
        let (art, uv, dep, user, cli) = preparer(&mut conn);

        let ligne = ParamsLigne {
            article_id: &art,
            unite_vente_id: &uv,
            depot_source_id: &dep,
            source_approvisionnement: "stock",
            quantite: 10.0,
            facteur: 1.0,
            prix_reference: 800,
            prix_pratique: 800,
        };

        // Total = 8000.
        let vente_id =
            inserer_vente_complete(&mut conn, &cli, &dep, "credit", &[ligne], &user, "m1").unwrap();

        // Paiement partiel de 5000.
        enregistrer_paiement(
            &mut conn,
            &vente_id,
            5000,
            "orange_money",
            None,
            &user,
            "m1",
        )
        .unwrap();

        let vente = lire_vente(&conn, &vente_id).unwrap().unwrap();
        assert_eq!(vente.statut, "partiellement_payee");
    }

    #[test]
    fn vente_decremente_le_stock() {
        let mut conn = base_test();
        let (art, uv, dep, user, cli) = preparer(&mut conn);

        let ligne = ParamsLigne {
            article_id: &art,
            unite_vente_id: &uv,
            depot_source_id: &dep,
            source_approvisionnement: "stock",
            quantite: 12.0,
            facteur: 1.0,
            prix_reference: 800,
            prix_pratique: 800,
        };

        inserer_vente_complete(&mut conn, &cli, &dep, "comptant", &[ligne], &user, "m1").unwrap();

        let stock: f64 = conn
            .query_row(
                "SELECT quantite FROM stock_depot WHERE article_id = 'art1'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(stock, 88.0); // 100 - 12
    }

    #[test]
    fn vente_a_decouvert_marquee() {
        let mut conn = base_test();
        let (art, uv, dep, user, cli) = preparer(&mut conn);

        // Vider le stock.
        conn.execute(
            "UPDATE stock_depot SET quantite = 0 WHERE article_id = 'art1'",
            [],
        )
        .unwrap();

        let ligne = ParamsLigne {
            article_id: &art,
            unite_vente_id: &uv,
            depot_source_id: &dep,
            source_approvisionnement: "stock",
            quantite: 3.0,
            facteur: 1.0,
            prix_reference: 800,
            prix_pratique: 800,
        };

        let vente_id =
            inserer_vente_complete(&mut conn, &cli, &dep, "comptant", &[ligne], &user, "m1")
                .unwrap();

        let lignes = lire_lignes_vente(&conn, &vente_id).unwrap();
        assert!(lignes[0].vente_a_decouvert);

        let stock: f64 = conn
            .query_row(
                "SELECT quantite FROM stock_depot WHERE article_id = 'art1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(stock, -3.0);
    }

    #[test]
    fn remise_tracee_au_journal() {
        let mut conn = base_test();
        let (art, uv, dep, user, cli) = preparer(&mut conn);

        let ligne = ParamsLigne {
            article_id: &art,
            unite_vente_id: &uv,
            depot_source_id: &dep,
            source_approvisionnement: "stock",
            quantite: 1.0,
            facteur: 1.0,
            prix_reference: 800,
            prix_pratique: 700, // remise de 100
        };

        inserer_vente_complete(&mut conn, &cli, &dep, "comptant", &[ligne], &user, "m1").unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM journal WHERE type_evenement = 'remise_accordee'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(count, 1);
    }
}
