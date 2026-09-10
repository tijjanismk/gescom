//! Sessions reseau — qui est connecte, depuis quand, jusqu'a quand.
//!
//! Remplace la session « 8 heures dans le localStorage » du v1 (D1),
//! qui n'etait verifiable que par le poste lui-meme : effacer une cle
//! de navigateur suffisait a se prolonger, et rien ne permettait de
//! deconnecter une machine a distance.
//!
//! ## Ou vit le jeton
//!
//! En base : un hash bcrypt, jamais le jeton en clair — une sauvegarde
//! egaree sur une cle USB donnerait sinon huit heures d'acces a chaque
//! poste connecte.
//!
//! En memoire du serveur : la correspondance jeton -> session, pour ne
//! pas payer un bcrypt (~250 ms) a chaque appel de commande. La
//! consequence assumee : au redemarrage du serveur, les postes se
//! reconnectent. C'est le bon sens du compromis — un serveur qui
//! redemarre en pleine journee est un evenement, pas une routine.

use rusqlite::{Connection, Result as ResSql, params};
use serde::{Deserialize, Serialize};
use crate::utils::maintenant_iso;
use crate::protocole::DUREE_JETON_HEURES;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionReseau {
    pub id: String,
    pub poste_id: String,
    pub poste_nom: String,
    pub utilisateur_id: String,
    pub utilisateur_nom: String,
    pub role: String,
    pub ouvert_le: String,
    pub expire_le: String,
    pub derniere_vue: Option<String>,
}

/// Un jeton d'authentification : 256 bits d'alea.
///
/// Deux UUID v4 concatenes. `uuid` tire son alea de `getrandom`, donc
/// du generateur du systeme — pas d'un `rand` amorce sur l'horloge,
/// qui rendrait le jeton devinable pour qui connait la seconde de
/// connexion.
pub fn engendrer_jeton() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

pub fn ouvrir(
    conn: &Connection,
    poste_id: &str,
    utilisateur_id: &str,
) -> Result<(String, String, String), String> {
    let jeton = engendrer_jeton();
    let hash = bcrypt::hash(&jeton, bcrypt::DEFAULT_COST).map_err(|e| e.to_string())?;
    let id = uuid::Uuid::new_v4().to_string();
    let ouvert = chrono::Local::now();
    let expire = ouvert + chrono::Duration::hours(DUREE_JETON_HEURES);
    let f = |d: chrono::DateTime<chrono::Local>| d.format("%Y-%m-%dT%H:%M:%S%.3f").to_string();

    conn.execute(
        "INSERT INTO session_reseau
           (id, jeton_hash, poste_id, utilisateur_id,
            ouvert_le, expire_le, derniere_vue)
         VALUES (?1,?2,?3,?4,?5,?6,?5)",
        params![id, hash, poste_id, utilisateur_id, f(ouvert), f(expire)],
    )
    .map_err(|e| e.to_string())?;

    Ok((id, jeton, f(expire)))
}

/// L'etat d'une session, relu en base a chaque appel.
///
/// On ne se fie pas au cache memoire pour la validite : c'est la
/// revocation qui doit prendre effet immediatement, sinon le bouton
/// « deconnecter ce poste » ne veut rien dire.
pub enum Etat {
    Valide { utilisateur_id: String, role: String, poste_id: String },
    Expiree,
    Revoquee,
    PosteFerme,
    Inconnue,
}

pub fn etat(conn: &Connection, session_id: &str) -> Etat {
    let ligne: Option<(String, String, Option<String>, String, i64)> = conn
        .query_row(
            "SELECT s.utilisateur_id, s.expire_le, s.revoque_le,
                    s.poste_id, p.actif
             FROM session_reseau s
             JOIN poste p ON p.id = s.poste_id
             WHERE s.id = ?1",
            params![session_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .ok();

    let Some((utilisateur_id, expire_le, revoque_le, poste_id, poste_actif)) = ligne else {
        return Etat::Inconnue;
    };
    if revoque_le.is_some() {
        return Etat::Revoquee;
    }
    if poste_actif == 0 {
        return Etat::PosteFerme;
    }
    // Comparaison lexicographique : l'ISO 8601 local se compare comme
    // du texte tant que le fuseau ne change pas, et il ne change pas —
    // le serveur et les caisses sont dans la meme boutique.
    if expire_le.as_str() < maintenant_iso().as_str() {
        return Etat::Expiree;
    }

    let role: String = conn
        .query_row(
            "SELECT r.nom FROM utilisateur u
             JOIN role r ON r.id = u.role_id
             WHERE u.id = ?1 AND u.actif = 1",
            params![utilisateur_id],
            |r| r.get(0),
        )
        .unwrap_or_default();

    if role.is_empty() {
        // L'utilisateur a ete desactive pendant sa session.
        return Etat::Revoquee;
    }

    Etat::Valide { utilisateur_id, role, poste_id }
}

pub fn toucher(conn: &Connection, session_id: &str) {
    conn.execute(
        "UPDATE session_reseau SET derniere_vue = ?1 WHERE id = ?2",
        params![maintenant_iso(), session_id],
    )
    .ok();
}

pub fn revoquer(conn: &Connection, session_id: &str, par: &str) -> ResSql<usize> {
    conn.execute(
        "UPDATE session_reseau SET revoque_le = ?1, revoque_par = ?2
         WHERE id = ?3 AND revoque_le IS NULL",
        params![maintenant_iso(), par, session_id],
    )
}

/// Ferme toutes les sessions. Appele au demarrage du serveur : les
/// jetons d'avant le redemarrage ne sont plus verifiables, les laisser
/// « actives » en base ferait mentir la liste des postes connectes.
pub fn revoquer_toutes(conn: &Connection, motif: &str) -> ResSql<usize> {
    conn.execute(
        "UPDATE session_reseau SET revoque_le = ?1, revoque_par = ?2
         WHERE revoque_le IS NULL",
        params![maintenant_iso(), motif],
    )
}

pub fn lister_actives(conn: &Connection) -> ResSql<Vec<SessionReseau>> {
    let mut stmt = conn.prepare(
        "SELECT s.id, s.poste_id, p.nom, s.utilisateur_id, u.nom, r.nom,
                s.ouvert_le, s.expire_le, s.derniere_vue
         FROM session_reseau s
         JOIN poste p       ON p.id = s.poste_id
         JOIN utilisateur u ON u.id = s.utilisateur_id
         JOIN role r        ON r.id = u.role_id
         WHERE s.revoque_le IS NULL AND s.expire_le > ?1
         ORDER BY s.ouvert_le DESC",
    )?;
    let v = stmt
        .query_map(params![maintenant_iso()], |r| {
            Ok(SessionReseau {
                id: r.get(0)?,
                poste_id: r.get(1)?,
                poste_nom: r.get(2)?,
                utilisateur_id: r.get(3)?,
                utilisateur_nom: r.get(4)?,
                role: r.get(5)?,
                ouvert_le: r.get(6)?,
                expire_le: r.get(7)?,
                derniere_vue: r.get(8)?,
            })
        })?
        .collect::<ResSql<Vec<_>>>()?;
    Ok(v)
}
