//! Persistance des sessions de caisse et mouvements de caisse.
//!
//! Une seule session ouverte à la fois. Le rapprochement (compté vs théorique)
//! ne porte que sur l'espèces — Orange/Moov sont tracés mais hors tiroir physique.

use crate::utils::maintenant_iso;
use rusqlite::{params, Connection, Result};
use uuid::Uuid;
#[derive(Debug)]
pub struct SessionCaisse {
    pub id: String,
    pub fond_initial: i64,
    pub date_ouverture: String,
    pub date_fermeture: Option<String>,
    pub montant_compte: Option<i64>,
    pub ecart: Option<i64>,
    pub statut: String,
}

#[derive(Debug)]
pub struct MouvementCaisse {
    pub id: String,
    pub session_id: String,
    pub sens: String,
    pub moyen: String,
    pub montant: i64,
    pub motif: String,
    pub operation_id: Option<String>,
}

/// Ouvre une nouvelle session de caisse.
/// Retourne une erreur si une session est déjà ouverte.
pub fn ouvrir_session(
    conn: &Connection,
    fond_initial: i64,
    cree_par: &str,
    origine: &str,
) -> Result<String> {
    // Vérifier qu'aucune session n'est déjà ouverte.
    let ouverte: i64 = conn.query_row(
        "SELECT COUNT(*) FROM session_caisse WHERE statut = 'ouverte'",
        [],
        |row| row.get(0),
    )?;

    if ouverte > 0 {
        return Err(rusqlite::Error::InvalidParameterName(
            "Une session de caisse est déjà ouverte".to_string(),
        ));
    }

    let id = Uuid::new_v4().to_string();
    let maintenant = maintenant_iso();

    conn.execute(
        "INSERT INTO session_caisse
         (id, fond_initial, date_ouverture, statut,
          cree_le, modifie_le, cree_par, modifie_par, origine)
         VALUES (?1, ?2, ?3, 'ouverte', ?4, ?5, ?6, ?7, ?8)",
        params![
            id,
            fond_initial,
            maintenant,
            maintenant,
            maintenant,
            cree_par,
            cree_par,
            origine
        ],
    )?;

    Ok(id)
}

/// Ferme la session ouverte avec le montant physiquement compté.
/// Calcule et enregistre l'écart (compté - théorique espèces).
pub fn fermer_session(
    conn: &Connection,
    session_id: &str,
    montant_compte: i64,
    modifie_par: &str,
) -> Result<i64> {
    // Calculer le solde théorique espèces.
    let fond_initial: i64 = conn.query_row(
        "SELECT fond_initial FROM session_caisse WHERE id = ?1",
        params![session_id],
        |row| row.get(0),
    )?;

    let entrees: i64 = conn
        .query_row(
            "SELECT COALESCE(SUM(montant), 0) FROM mouvement_caisse
             WHERE session_id = ?1 AND sens = 'entree' AND moyen = 'especes'",
            params![session_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let sorties: i64 = conn
        .query_row(
            "SELECT COALESCE(SUM(montant), 0) FROM mouvement_caisse
             WHERE session_id = ?1 AND sens = 'sortie' AND moyen = 'especes'",
            params![session_id],
            |row| row.get(0),
        )
        .unwrap_or(0);

    let solde_theorique = fond_initial + entrees - sorties;
    let ecart = montant_compte - solde_theorique;
    let maintenant = maintenant_iso();

    conn.execute(
        "UPDATE session_caisse
         SET statut = 'fermee', date_fermeture = ?1,
             montant_compte = ?2, ecart = ?3,
             modifie_le = ?4, modifie_par = ?5
         WHERE id = ?6",
        params![
            maintenant,
            montant_compte,
            ecart,
            maintenant,
            modifie_par,
            session_id
        ],
    )?;

    Ok(ecart)
}

/// Enregistre un mouvement de caisse dans la session ouverte.
pub fn enregistrer_mouvement(
    conn: &Connection,
    session_id: &str,
    sens: &str,  // 'entree' / 'sortie'
    moyen: &str, // 'especes' / 'orange_money' / 'moov_money' / 'cheque'
    montant: i64,
    motif: &str, // 'vente' / 'remboursement' / 'depense' / 'divers'
    operation_id: Option<&str>,
    cree_par: &str,
    origine: &str,
) -> Result<String> {
    let id = Uuid::new_v4().to_string();
    let maintenant = maintenant_iso();

    conn.execute(
        "INSERT INTO mouvement_caisse
         (id, session_id, sens, moyen, montant, motif, operation_id,
          date_mouvement, cree_le, cree_par, origine)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            id,
            session_id,
            sens,
            moyen,
            montant,
            motif,
            operation_id,
            maintenant,
            maintenant,
            cree_par,
            origine
        ],
    )?;

    Ok(id)
}

/// Retourne la session de caisse actuellement ouverte, ou None.
pub fn session_ouverte(conn: &Connection) -> Result<Option<SessionCaisse>> {
    let result = conn.query_row(
        "SELECT id, fond_initial, date_ouverture, date_fermeture,
                montant_compte, ecart, statut
         FROM session_caisse WHERE statut = 'ouverte' LIMIT 1",
        [],
        |row| {
            Ok(SessionCaisse {
                id: row.get(0)?,
                fond_initial: row.get(1)?,
                date_ouverture: row.get(2)?,
                date_fermeture: row.get(3)?,
                montant_compte: row.get(4)?,
                ecart: row.get(5)?,
                statut: row.get(6)?,
            })
        },
    );

    match result {
        Ok(session) => Ok(Some(session)),
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

    #[test]
    fn ouvrir_et_trouver_session() {
        let conn = base_test();

        ouvrir_session(&conn, 10_000, "patron", "m1").unwrap();

        let session = session_ouverte(&conn).unwrap();
        assert!(session.is_some());
        assert_eq!(session.unwrap().fond_initial, 10_000);
    }

    #[test]
    fn double_ouverture_interdite() {
        let conn = base_test();

        ouvrir_session(&conn, 10_000, "patron", "m1").unwrap();
        let resultat = ouvrir_session(&conn, 5_000, "patron", "m1");

        // La deuxième ouverture doit échouer.
        assert!(resultat.is_err());
    }

    #[test]
    fn rapprochement_especes_seulement() {
        let conn = base_test();

        let session_id = ouvrir_session(&conn, 10_000, "patron", "m1").unwrap();

        // Deux ventes espèces + une Orange Money.
        enregistrer_mouvement(
            &conn,
            &session_id,
            "entree",
            "especes",
            5_000,
            "vente",
            None,
            "u1",
            "m1",
        )
        .unwrap();
        enregistrer_mouvement(
            &conn,
            &session_id,
            "entree",
            "especes",
            3_000,
            "vente",
            None,
            "u1",
            "m1",
        )
        .unwrap();
        enregistrer_mouvement(
            &conn,
            &session_id,
            "entree",
            "orange_money",
            10_000,
            "vente",
            None,
            "u1",
            "m1",
        )
        .unwrap();

        // Fermeture : on compte 18 000 dans le tiroir (espèces seulement).
        // Théorique espèces = 10000 + 5000 + 3000 = 18000. Écart = 0.
        let ecart = fermer_session(&conn, &session_id, 18_000, "patron").unwrap();
        assert_eq!(ecart, 0);
    }

    #[test]
    fn ecart_negatif_signale_manque() {
        let conn = base_test();

        let session_id = ouvrir_session(&conn, 10_000, "patron", "m1").unwrap();

        enregistrer_mouvement(
            &conn,
            &session_id,
            "entree",
            "especes",
            8_000,
            "vente",
            None,
            "u1",
            "m1",
        )
        .unwrap();

        // Théorique = 18000, compté = 17500 -> écart -500.
        let ecart = fermer_session(&conn, &session_id, 17_500, "patron").unwrap();
        assert_eq!(ecart, -500);
        assert!(ecart < 0);
    }
}
