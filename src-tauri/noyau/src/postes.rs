//! Les postes : quelles machines parlent au serveur.

use rusqlite::{Connection, Result, params};
use serde::{Deserialize, Serialize};
use crate::utils::maintenant_iso;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Poste {
    pub id: String,
    pub nom: String,
    pub empreinte: String,
    /// `serveur` (le poste principal, qui heberge la base) ou `caisse`.
    pub genre: String,
    pub actif: bool,
    pub dernier_contact: Option<String>,
    pub derniere_ip: Option<String>,
}

/// Retrouve le poste par son empreinte, ou l'inscrit.
///
/// L'inscription est automatique et non l'inverse : refuser un poste
/// inconnu obligerait le commercant a declarer chaque machine avant de
/// pouvoir s'en servir, un vendredi de marche, sans savoir ou. Le
/// controle d'acces est le mot de passe ; le poste, lui, est une trace.
pub fn inscrire_ou_retrouver(
    conn: &Connection,
    nom: &str,
    empreinte: &str,
    genre: &str,
    ip: Option<&str>,
) -> Result<Poste> {
    let maintenant = maintenant_iso();

    let existant: Option<String> = conn
        .query_row(
            "SELECT id FROM poste WHERE empreinte = ?1",
            params![empreinte],
            |r| r.get(0),
        )
        .ok();

    let id = match existant {
        Some(id) => {
            conn.execute(
                "UPDATE poste SET nom = ?1, dernier_contact = ?2,
                        derniere_ip = ?3, modifie_le = ?2
                 WHERE id = ?4",
                params![nom, maintenant, ip, id],
            )?;
            id
        }
        None => {
            let id = uuid::Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO poste
                   (id, nom, empreinte, genre, actif,
                    dernier_contact, derniere_ip, cree_le, modifie_le)
                 VALUES (?1,?2,?3,?4,1,?5,?6,?5,?5)",
                params![id, nom, empreinte, genre, maintenant, ip],
            )?;
            id
        }
    };

    lire(conn, &id)
}

pub fn lire(conn: &Connection, id: &str) -> Result<Poste> {
    conn.query_row(
        "SELECT id, nom, empreinte, genre, actif, dernier_contact, derniere_ip
         FROM poste WHERE id = ?1",
        params![id],
        ligne_poste,
    )
}

pub fn lister(conn: &Connection) -> Result<Vec<Poste>> {
    let mut stmt = conn.prepare(
        "SELECT id, nom, empreinte, genre, actif, dernier_contact, derniere_ip
         FROM poste ORDER BY genre DESC, nom ASC",
    )?;
    let v = stmt
        .query_map([], ligne_poste)?
        .collect::<Result<Vec<_>>>()?;
    Ok(v)
}

/// Coupe l'acces d'un poste : ses sessions tombent, et il ne peut plus
/// en ouvrir. Le geste qu'on veut avoir sous la main quand une machine
/// disparait du magasin.
pub fn desactiver(conn: &Connection, id: &str, par: &str) -> Result<()> {
    let maintenant = maintenant_iso();
    conn.execute(
        "UPDATE poste SET actif = 0, modifie_le = ?1 WHERE id = ?2",
        params![maintenant, id],
    )?;
    conn.execute(
        "UPDATE session_reseau SET revoque_le = ?1, revoque_par = ?2
         WHERE poste_id = ?3 AND revoque_le IS NULL",
        params![maintenant, par, id],
    )?;
    Ok(())
}

pub fn reactiver(conn: &Connection, id: &str) -> Result<()> {
    conn.execute(
        "UPDATE poste SET actif = 1, modifie_le = ?1 WHERE id = ?2",
        params![maintenant_iso(), id],
    )?;
    Ok(())
}

fn ligne_poste(r: &rusqlite::Row) -> Result<Poste> {
    Ok(Poste {
        id: r.get(0)?,
        nom: r.get(1)?,
        empreinte: r.get(2)?,
        genre: r.get(3)?,
        actif: r.get::<_, i64>(4)? != 0,
        dernier_contact: r.get(5)?,
        derniere_ip: r.get(6)?,
    })
}
