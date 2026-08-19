//! Persistance des ventes et lignes de vente.
//!
//! RÈGLES DOCUMENTÉES (module-facturation-stock.md) :
//!   §0  — Créance naît d'une vente, jamais saisie manuellement.
//!   §1  — Vente atomique : stock décrémenté + facture créée dans la même transaction.
//!   §2  — Toute vente produit une facture numérotée et validée immédiatement.
//!   §7  — Remise (baisse) tracée au journal. Hausse ignorée.
//!   §13 — Montants en i64, jamais Float.

use rusqlite::{Connection, Result, params};
use uuid::Uuid;
use chrono::Datelike;

use crate::coeur::calcul::{montant_ligne, total_vente};
use crate::coeur::stock::est_a_decouvert;
use crate::persistance::journal::ecrire_evenement;
use crate::utils::maintenant_iso;

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
    pub taux_tva: f64,
    pub montant_tva: i64,
}

pub struct ParamsLigne<'a> {
    pub article_id: &'a str,
    pub unite_vente_id: &'a str,
    pub depot_source_id: &'a str,
    pub source_approvisionnement: &'a str,
    pub quantite: f64,
    pub facteur: f64,
    pub prix_reference: i64,
    pub prix_pratique: i64,
    pub taux_tva: f64,  // 0.0 par défaut
}

/// Crée une vente complète dans une transaction unique.
///
/// §2 — Crée ET valide la facture dans la même transaction.
/// §1 — Atomique : tout ou rien.
/// Statut initial toujours `creance_ouverte` — enregistrer_paiement le fera évoluer.
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

    // Calcul des montants via le cœur.
    let montants: Vec<i64> = lignes
        .iter()
        .map(|l| montant_ligne(l.prix_pratique, l.quantite))
        .collect();
    let total = total_vente(&montants);

    // Statut initial toujours creance_ouverte.
    tx.execute(
        "INSERT INTO vente
         (id, client_id, depot_id, mode_reglement, auteur_id, statut,
          date_vente, cree_le, modifie_le, cree_par, modifie_par, origine)
         VALUES (?1,?2,?3,?4,?5,'creance_ouverte',?6,?7,?8,?9,?10,?11)",
        params![
            vente_id, client_id, depot_id, mode_reglement,
            auteur_id, maintenant,
            maintenant, maintenant, auteur_id, auteur_id, origine
        ],
    )?;

    // Lignes + décrément stock.
    for ligne in lignes.iter() {
        let ligne_id = Uuid::new_v4().to_string();

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

        // Calcul TVA.
        let montant_ligne = montant_ligne(ligne.prix_pratique, ligne.quantite);
        let montant_tva = (montant_ligne as f64 * ligne.taux_tva).round() as i64;

        tx.execute(
            "INSERT INTO ligne_vente
             (id, vente_id, article_id, unite_vente_id, depot_source_id,
              source_approvisionnement, vente_a_decouvert, quantite,
              prix_reference, prix_pratique, taux_tva, montant_tva,
              cree_le, origine)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
            params![
                ligne_id, vente_id, ligne.article_id, ligne.unite_vente_id,
                ligne.depot_source_id, ligne.source_approvisionnement,
                a_decouvert as i64, ligne.quantite,
                ligne.prix_reference, ligne.prix_pratique,
                ligne.taux_tva, montant_tva,
                maintenant, origine
            ],
        )?;

        // Décrément stock.
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

            tx.execute(
                "INSERT INTO stock_depot (id, article_id, depot_id, quantite)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(article_id, depot_id)
                 DO UPDATE SET quantite = ?4",
                params![
                    Uuid::new_v4().to_string(),
                    ligne.article_id,
                    ligne.depot_source_id,
                    stock_actuel - quantite_base
                ],
            )?;

            tx.execute(
                "INSERT INTO mouvement_stock
                 (id, article_id, depot_id, type_mouvement, quantite_delta,
                  operation_id, auteur_id, date_mouvement, cree_le, cree_par, origine)
                 VALUES (?1,?2,?3,'vente',?4,?5,?6,?7,?8,?9,?10)",
                params![
                    Uuid::new_v4().to_string(),
                    ligne.article_id, ligne.depot_source_id, -(quantite_base),
                    vente_id, auteur_id,
                    maintenant, maintenant, auteur_id, origine
                ],
            )?;
        }

        // §5 — Fournisseur secondaire : créer une dette automatique.
        if ligne.source_approvisionnement == "fournisseur_secondaire" {
            // Chercher ou créer un fournisseur secondaire générique.
            let fournisseur_id: String = tx.query_row(
                "SELECT id FROM fournisseur WHERE est_voisin = 1
                 AND nom = 'Fournisseur secondaire' LIMIT 1",
                [],
                |row| row.get(0),
            ).unwrap_or_else(|_| {
                let fid = Uuid::new_v4().to_string();
                tx.execute(
                    "INSERT OR IGNORE INTO fournisseur
                     (id, nom, est_voisin, actif, cree_le, modifie_le, origine)
                     VALUES (?1, 'Fournisseur secondaire', 1, 1, ?2, ?3, 'auto')",
                    params![fid, maintenant, maintenant],
                ).ok();
                fid
            });

            // Journaliser la dette (la gestion complète des dettes fournisseur
            // est un chantier ouvert §14 — on trace pour l'instant).
            ecrire_evenement(
                &tx,
                "dette_fournisseur_secondaire",
                "ligne_vente",
                &ligne_id,
                auteur_id,
                None,
                Some(&format!(
                    r#"{{"fournisseur_id":"{}","montant":{}}}"#,
                    fournisseur_id,
                    montant_ligne(ligne.prix_pratique, ligne.quantite)
                )),
                origine,
            )?;
        }

        // §7 — Tracer la remise si prix négocié en dessous du référence.
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
        // §7 — Hausse ignorée : aucune logique, aucun log.
    }

    // §2 — Créer et valider la facture dans la même transaction.
    // Format : GESCOM-2026-000001
    let annee = chrono::Local::now().year();
    let dernier: i64 = tx.query_row(
        "SELECT COUNT(*) FROM facture WHERE numero LIKE ?1",
        params![format!("GESCOM-{}-%", annee)],
        |row| row.get(0),
    ).unwrap_or(0);

    let numero_facture = format!("GESCOM-{}-{:06}", annee, dernier + 1);
    let facture_id = Uuid::new_v4().to_string();

    tx.execute(
        "INSERT INTO facture
         (id, numero, vente_id, statut, total,
          date_validation, cree_le, modifie_le,
          cree_par, modifie_par, origine)
         VALUES (?1, ?2, ?3, 'validee', ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            facture_id, numero_facture, vente_id, total,
            maintenant, maintenant, maintenant,
            auteur_id, auteur_id, origine
        ],
    )?;

    // Journal vente + facture.
    ecrire_evenement(
        &tx, "vente_creee", "vente", &vente_id, auteur_id,
        None,
        Some(&format!(
            r#"{{"total":{},"facture":"{}"}}"#,
            total, numero_facture
        )),
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
        |row| Ok(Vente {
            id:             row.get(0)?,
            client_id:      row.get(1)?,
            depot_id:       row.get(2)?,
            mode_reglement: row.get(3)?,
            auteur_id:      row.get(4)?,
            statut:         row.get(5)?,
            date_vente:     row.get(6)?,
        }),
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
                prix_reference, prix_pratique,
                COALESCE(taux_tva, 0.0), COALESCE(montant_tva, 0)
         FROM ligne_vente WHERE vente_id = ?1",
    )?;

    let x = stmt.query_map(params![vente_id], |row| {
        Ok(LigneVente {
            id:                       row.get(0)?,
            vente_id:                 row.get(1)?,
            article_id:               row.get(2)?,
            unite_vente_id:           row.get(3)?,
            depot_source_id:          row.get(4)?,
            source_approvisionnement: row.get(5)?,
            vente_a_decouvert:        row.get::<_, i64>(6)? != 0,
            quantite:                 row.get(7)?,
            prix_reference:           row.get(8)?,
            prix_pratique:            row.get(9)?,
            taux_tva:                 row.get(10)?,
            montant_tva:              row.get(11)?,
        })
    })?
    .collect::<Result<Vec<_>>>()?;
    Ok(x)
}

/// Calcule le total d'une vente depuis ses lignes en base.
pub fn total_vente_depuis_base(conn: &Connection, vente_id: &str) -> Result<i64> {
    let total: i64 = conn.query_row(
        "SELECT CAST(COALESCE(SUM(prix_pratique * quantite), 0) AS INTEGER)
         FROM ligne_vente WHERE vente_id = ?1",
        params![vente_id],
        |row| row.get(0),
    )?;
    Ok(total)
}
