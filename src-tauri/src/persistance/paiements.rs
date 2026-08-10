//! Persistance des paiements.
//!
//! Un paiement peut utiliser un avoir comme moyen de règlement.
//! Chaque paiement met à jour le statut de la vente ET alimente la caisse
//! si une session est ouverte.

use rusqlite::{Connection, Result, params};
use uuid::Uuid;

use crate::coeur::calcul::{reste_du, statut_vente, StatutVente};
use crate::persistance::journal::ecrire_evenement;

#[derive(Debug)]
pub struct Paiement {
    pub id: String,
    pub vente_id: String,
    pub montant: i64,
    pub mode: String,
    pub avoir_id: Option<String>,
}

/// Enregistre un paiement et met à jour le statut de la vente.
/// Si une session de caisse est ouverte, crée automatiquement un mouvement de caisse.
pub fn enregistrer_paiement(
    conn: &mut Connection,
    vente_id: &str,
    montant: i64,
    mode: &str,
    avoir_id: Option<&str>,
    auteur_id: &str,
    origine: &str,
) -> Result<String> {
    let tx = conn.transaction()?;
    let id = Uuid::new_v4().to_string();
    let maintenant = crate::utils::maintenant_iso();

    // Si mode = avoir, marquer l'avoir comme consommé.
    if mode == "avoir" {
        if let Some(aid) = avoir_id {
            tx.execute(
                "UPDATE avoir SET statut = 'consomme' WHERE id = ?1",
                params![aid],
            )?;
        }
    }

    // Insérer le paiement.
    tx.execute(
        "INSERT INTO paiement
         (id, vente_id, montant, mode, avoir_id, auteur_id,
          date_paiement, cree_le, cree_par, origine)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
        params![
            id, vente_id, montant, mode, avoir_id,
            auteur_id, maintenant, maintenant, auteur_id, origine
        ],
    )?;

    // Recalculer le statut de la vente.
    let total: i64 = tx.query_row(
        "SELECT CAST(COALESCE(SUM(prix_pratique * quantite), 0) AS INTEGER)
         FROM ligne_vente WHERE vente_id = ?1",
        params![vente_id],
        |row| row.get(0),
    )?;

    let paiements: Vec<i64> = {
        let mut stmt = tx.prepare(
            "SELECT montant FROM paiement WHERE vente_id = ?1",
        )?;
        let x = stmt.query_map(params![vente_id], |row| row.get(0))?
            .collect::<Result<Vec<_>>>()?;
        x
    };

    let reste = reste_du(total, &paiements, &[]);
    let nouveau_statut = match statut_vente(total, reste) {
        StatutVente::Payee              => "payee",
        StatutVente::PartiellementPayee => "partiellement_payee",
        StatutVente::CreanceOuverte     => "creance_ouverte",
    };

    tx.execute(
        "UPDATE vente SET statut = ?1, modifie_le = ?2, modifie_par = ?3
         WHERE id = ?4",
        params![nouveau_statut, maintenant, auteur_id, vente_id],
    )?;

    // Alimenter la caisse si une session est ouverte.
    // On n'alimente la caisse que pour les paiements réels (pas les avoirs).
    if mode != "avoir" {
        let session_id: Option<String> = tx.query_row(
            "SELECT id FROM session_caisse WHERE statut = 'ouverte' LIMIT 1",
            [],
            |row| row.get(0),
        ).ok();

        if let Some(sid) = session_id {
            tx.execute(
                "INSERT INTO mouvement_caisse
                 (id, session_id, sens, moyen, montant, motif,
                  operation_id, date_mouvement, cree_le, cree_par, origine)
                 VALUES (?1, ?2, 'entree', ?3, ?4, 'vente', ?5, ?6, ?7, ?8, ?9)",
                params![
                    Uuid::new_v4().to_string(),
                    sid, mode, montant,
                    vente_id,
                    maintenant, maintenant, auteur_id, origine
                ],
            )?;
        }
    }

    // Journaliser.
    ecrire_evenement(
        &tx,
        "paiement_enregistre",
        "vente",
        vente_id,
        auteur_id,
        None,
        Some(&format!(r#"{{"montant":{}, "mode":"{}"}}"#, montant, mode)),
        origine,
    )?;

    tx.commit()?;
    Ok(id)
}

pub fn lire_paiements(conn: &Connection, vente_id: &str) -> Result<Vec<Paiement>> {
    let mut stmt = conn.prepare(
        "SELECT id, vente_id, montant, mode, avoir_id
         FROM paiement WHERE vente_id = ?1 ORDER BY date_paiement",
    )?;

    let x = stmt
        .query_map(params![vente_id], |row| {
            Ok(Paiement {
                id:       row.get(0)?,
                vente_id: row.get(1)?,
                montant:  row.get(2)?,
                mode:     row.get(3)?,
                avoir_id: row.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>>>()?;
    Ok(x)
}