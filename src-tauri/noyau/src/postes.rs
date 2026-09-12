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
                        derniere_ip = ?3, genre = ?4, modifie_le = ?2
                 WHERE id = ?5",
                params![nom, maintenant, ip, genre, id],
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

/// Coupe l'acces d'une caisse : ses sessions tombent, et elle ne peut
/// plus en ouvrir. Le geste qu'on veut avoir sous la main quand une
/// machine disparait de la boutique.
///
/// Rend `Err` avec un message destine au commercant, pas une erreur
/// SQL : ce texte s'affiche tel quel dans la console du serveur.
pub fn desactiver(conn: &Connection, id: &str, par: &str) -> std::result::Result<(), String> {
    // Seule une caisse se desactive. Le poste « serveur » est la
    // boutique elle-meme, et le poste « console » est l'ecran d'ou
    // vient le clic : les eteindre enferme dehors celui qui les
    // eteint, sans aucun chemin de retour.
    let genre: String = conn
        .query_row("SELECT genre FROM poste WHERE id = ?1", params![id], |r| {
            r.get(0)
        })
        .map_err(|_| "Ce poste n'existe plus.".to_string())?;
    if genre != "caisse" {
        return Err(format!(
            "Le poste « {genre} » ne se désactive pas : plus rien ne permettrait de le rallumer."
        ));
    }

    let maintenant = maintenant_iso();
    conn.execute(
        "UPDATE poste SET actif = 0, modifie_le = ?1 WHERE id = ?2",
        params![maintenant, id],
    )
    .map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE session_reseau SET revoque_le = ?1, revoque_par = ?2
         WHERE poste_id = ?3 AND revoque_le IS NULL",
        params![maintenant, par, id],
    )
    .map_err(|e| e.to_string())?;
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

// =====================================================================
//  LA MEME CHOSE, SUR L'UN OU L'AUTRE MOTEUR
// =====================================================================
//
// Pour authentifier une caisse sur PostgreSQL (D11) : `connexion`
// inscrit le poste avant d'ouvrir la session, sur les deux moteurs
// desormais. `poste` n'est pas cloisonne — une machine reste la meme
// machine, quel que soit le dossier ouvert depuis elle.

use crate::base::Base;
use crate::parametres;

pub fn inscrire_ou_retrouver_sur(
    base: &mut Base,
    nom: &str,
    empreinte: &str,
    genre: &str,
    ip: Option<&str>,
) -> std::result::Result<Poste, String> {
    let maintenant = maintenant_iso();

    let existant = base
        .lire_une(
            "SELECT id FROM poste WHERE empreinte = ?1",
            &parametres![empreinte],
            |r| r.get::<String>(0),
        )
        .map_err(|e| e.0)?;

    let id = match existant {
        Some(id) => {
            base.executer(
                "UPDATE poste SET nom = ?1, dernier_contact = ?2,
                        derniere_ip = ?3, genre = ?4, modifie_le = ?2
                 WHERE id = ?5",
                &parametres![nom, maintenant.clone(), ip, genre, id.clone()],
            )
            .map_err(|e| e.0)?;
            id
        }
        None => {
            let id = uuid::Uuid::new_v4().to_string();
            base.executer(
                "INSERT INTO poste
                   (id, nom, empreinte, genre, actif,
                    dernier_contact, derniere_ip, cree_le, modifie_le)
                 VALUES (?1,?2,?3,?4,1,?5,?6,?5,?5)",
                &parametres![id.clone(), nom, empreinte, genre, maintenant, ip],
            )
            .map_err(|e| e.0)?;
            id
        }
    };

    lire_sur(base, &id)
}

pub fn lire_sur(base: &mut Base, id: &str) -> std::result::Result<Poste, String> {
    base.lire_une(
        "SELECT id, nom, empreinte, genre, actif, dernier_contact, derniere_ip
         FROM poste WHERE id = ?1",
        &parametres![id],
        |r| {
            Ok(Poste {
                id: r.get::<String>(0)?,
                nom: r.get::<String>(1)?,
                empreinte: r.get::<String>(2)?,
                genre: r.get::<String>(3)?,
                actif: r.get::<i64>(4)? != 0,
                dernier_contact: r.get::<Option<String>>(5)?,
                derniere_ip: r.get::<Option<String>>(6)?,
            })
        },
    )
    .map_err(|e| e.0)?
    .ok_or_else(|| "Poste introuvable".to_string())
}

pub fn lister_sur_base(base: &mut Base) -> std::result::Result<Vec<Poste>, String> {
    base.lire_plusieurs(
        "SELECT id, nom, empreinte, genre, actif, dernier_contact, derniere_ip
         FROM poste ORDER BY genre DESC, nom ASC",
        &[],
        |r| {
            Ok(Poste {
                id: r.get::<String>(0)?,
                nom: r.get::<String>(1)?,
                empreinte: r.get::<String>(2)?,
                genre: r.get::<String>(3)?,
                actif: r.get::<i64>(4)? != 0,
                dernier_contact: r.get::<Option<String>>(5)?,
                derniere_ip: r.get::<Option<String>>(6)?,
            })
        },
    )
    .map_err(|e| e.0)
}

/// Coupe l'acces d'une caisse et fait tomber ses sessions — les deux
/// dans une transaction, ou rien.
pub fn desactiver_sur_base(base: &mut Base, id: &str, par: &str) -> std::result::Result<(), String> {
    let genre: String = base
        .lire_une("SELECT genre FROM poste WHERE id = ?1", &parametres![id], |r| r.get::<String>(0))
        .map_err(|e| e.0)?
        .ok_or_else(|| "Ce poste n'existe plus.".to_string())?;
    if genre != "caisse" {
        return Err(format!(
            "Le poste « {genre} » ne se désactive pas : plus rien ne permettrait de le rallumer."
        ));
    }
    let maintenant = maintenant_iso();
    let mut tx = base.transaction().map_err(|e| e.0)?;
    tx.executer(
        "UPDATE poste SET actif = 0, modifie_le = ?1 WHERE id = ?2",
        &parametres![maintenant.clone(), id],
    )
    .map_err(|e| e.0)?;
    tx.executer(
        "UPDATE session_reseau SET revoque_le = ?1, revoque_par = ?2
         WHERE poste_id = ?3 AND revoque_le IS NULL",
        &parametres![maintenant, par, id],
    )
    .map_err(|e| e.0)?;
    tx.valider().map_err(|e| e.0)
}

pub fn reactiver_sur_base(base: &mut Base, id: &str) -> std::result::Result<(), String> {
    base.executer(
        "UPDATE poste SET actif = 1, modifie_le = ?1 WHERE id = ?2",
        &parametres![maintenant_iso(), id],
    )
    .map_err(|e| e.0)?;
    Ok(())
}
