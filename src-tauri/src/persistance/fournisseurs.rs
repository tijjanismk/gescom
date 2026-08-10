//! Persistance des fournisseurs.
//!
//! Deux types : fournisseur normal (achat régulier) et fournisseur voisin
//! (créé automatiquement quand on vend un article hors-stock).

use crate::utils::maintenant_iso;
use rusqlite::{params, Connection, Result};
use uuid::Uuid;

#[derive(Debug)]
pub struct Fournisseur {
    pub id: String,
    pub nom: String,
    pub telephone: Option<String>,
    pub nif: Option<String>,
    pub adresse: Option<String>,
    pub est_voisin: bool,
    pub actif: bool,
}

/// Insère un fournisseur normal.
pub fn inserer_fournisseur(
    conn: &Connection,
    nom: &str,
    telephone: Option<&str>,
    nif: Option<&str>,
    adresse: Option<&str>,
    origine: &str,
) -> Result<String> {
    let id = Uuid::new_v4().to_string();
    let maintenant = maintenant_iso();

    conn.execute(
        "INSERT INTO fournisseur
         (id, nom, telephone, nif, adresse, est_voisin, actif,
          cree_le, modifie_le, origine)
         VALUES (?1, ?2, ?3, ?4, ?5, 0, 1, ?6, ?7, ?8)",
        params![id, nom, telephone, nif, adresse, maintenant, maintenant, origine],
    )?;

    Ok(id)
}

/// Crée un fournisseur-voisin automatiquement (article hors-stock).
/// Si un voisin du même nom existe déjà, retourne son id existant.
pub fn inserer_fournisseur_voisin(conn: &Connection, nom: &str, origine: &str) -> Result<String> {
    // Vérifier si un voisin de ce nom existe déjà.
    let existant: Option<String> = conn
        .query_row(
            "SELECT id FROM fournisseur WHERE nom = ?1 AND est_voisin = 1 LIMIT 1",
            params![nom],
            |row| row.get(0),
        )
        .ok();

    if let Some(id) = existant {
        return Ok(id);
    }

    // Sinon on le crée.
    let id = Uuid::new_v4().to_string();
    let maintenant = maintenant_iso();

    conn.execute(
        "INSERT INTO fournisseur
         (id, nom, est_voisin, actif, cree_le, modifie_le, origine)
         VALUES (?1, ?2, 1, 1, ?3, ?4, ?5)",
        params![id, nom, maintenant, maintenant, origine],
    )?;

    Ok(id)
}

/// Lit tous les fournisseurs actifs (hors voisins).
pub fn lire_fournisseurs(conn: &Connection) -> Result<Vec<Fournisseur>> {
    let mut stmt = conn.prepare(
        "SELECT id, nom, telephone, nif, adresse, est_voisin, actif
         FROM fournisseur
         WHERE actif = 1 AND est_voisin = 0
         ORDER BY nom",
    )?;

    let fournisseurs = stmt
        .query_map([], |row| {
            Ok(Fournisseur {
                id: row.get(0)?,
                nom: row.get(1)?,
                telephone: row.get(2)?,
                nif: row.get(3)?,
                adresse: row.get(4)?,
                est_voisin: row.get::<_, i64>(5)? != 0,
                actif: row.get::<_, i64>(6)? != 0,
            })
        })?
        .collect::<Result<Vec<_>>>()?;

    Ok(fournisseurs)
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
    fn inserer_et_lire_fournisseur() {
        let conn = base_test();

        inserer_fournisseur(
            &conn,
            "Grossiste Bamako",
            Some("76111111"),
            Some("NIF-001"),
            None,
            "m1",
        )
        .unwrap();

        let fournisseurs = lire_fournisseurs(&conn).unwrap();
        assert_eq!(fournisseurs.len(), 1);
        assert_eq!(fournisseurs[0].nom, "Grossiste Bamako");
        assert!(!fournisseurs[0].est_voisin);
    }

    #[test]
    fn voisin_meme_nom_non_duplique() {
        let conn = base_test();

        let id1 = inserer_fournisseur_voisin(&conn, "Boutique Koné", "m1").unwrap();
        let id2 = inserer_fournisseur_voisin(&conn, "Boutique Koné", "m1").unwrap();

        // Même nom = même id, pas de doublon.
        assert_eq!(id1, id2);
    }

    #[test]
    fn voisins_non_retournes_dans_lire_fournisseurs() {
        let conn = base_test();

        inserer_fournisseur_voisin(&conn, "Voisin A", "m1").unwrap();
        inserer_fournisseur(&conn, "Vrai fournisseur", None, None, None, "m1").unwrap();

        let fournisseurs = lire_fournisseurs(&conn).unwrap();
        // Seul le vrai fournisseur apparaît.
        assert_eq!(fournisseurs.len(), 1);
        assert_eq!(fournisseurs[0].nom, "Vrai fournisseur");
    }
}
