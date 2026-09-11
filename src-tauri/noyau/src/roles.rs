//! Gerer les roles et les permissions.
//!
//! Le catalogue des permissions vit dans `portes` — c'est du code, il
//! ne se modifie pas depuis l'ecran. Ce qui se modifie, c'est QUI a
//! QUOI : la liste d'un role, et les ajouts ou retraits personnels.
//!
//! ## Les trois garde-fous
//!
//! 1. Une permission hors catalogue est REFUSEE a l'ecriture. Ecrite
//!    en base, elle serait une case cochee sans effet : le role
//!    paraitrait complet et ne le serait pas.
//! 2. Un role `protege` ne se modifie ni ne se supprime. C'est le
//!    compte de secours ; sans lui, une mauvaise manipulation rendrait
//!    l'application inadministrable.
//! 3. Un role encore porte par quelqu'un ne se supprime pas. La
//!    colonne `utilisateur.role_id` est une cle etrangere : la base
//!    refuserait de toute facon, mais avec un message que personne ne
//!    comprend.

use crate::utils::maintenant_iso;

/// Le catalogue, tel qu'un ecran doit l'afficher.
pub fn lire_catalogue_permissions() -> serde_json::Value {
    serde_json::json!(crate::portes::CATALOGUE
        .iter()
        .map(|p| serde_json::json!({
            "code": p.code,
            "libelle": p.libelle,
            "groupe": p.groupe,
        }))
        .collect::<Vec<_>>())
}

/// Tous les roles, avec ce qu'ils permettent.
pub fn lire_roles(conn: &rusqlite::Connection) -> Result<serde_json::Value, String> {
    let mut st = conn
        .prepare(
            "SELECT r.id, r.nom, COALESCE(r.permissions, '[]'),
                    COALESCE(r.acces_total, 0), COALESCE(r.protege, 0),
                    COALESCE(r.description, ''),
                    (SELECT COUNT(*) FROM utilisateur u WHERE u.role_id = r.id)
             FROM role r ORDER BY r.acces_total DESC, r.nom",
        )
        .map_err(|e| e.to_string())?;

    let v: Vec<serde_json::Value> = st
        .query_map([], |r| {
            let json: String = r.get(2)?;
            let acces_total: i64 = r.get(3)?;
            // Un role a acces total rend TOUT le catalogue : afficher sa
            // liste vide laisserait croire qu'il ne peut rien.
            let permissions: Vec<String> = if acces_total != 0 {
                crate::portes::CATALOGUE
                    .iter()
                    .map(|p| p.code.to_string())
                    .collect()
            } else {
                serde_json::from_str::<Vec<String>>(&json)
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|c| crate::portes::existe(c))
                    .collect()
            };
            Ok(serde_json::json!({
                "id":          r.get::<_, String>(0)?,
                "nom":         r.get::<_, String>(1)?,
                "permissions": permissions,
                "acces_total": acces_total != 0,
                "protege":     r.get::<_, i64>(4)? != 0,
                "description": r.get::<_, String>(5)?,
                "nb_utilisateurs": r.get::<_, i64>(6)?,
            }))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    Ok(serde_json::json!(v))
}

/// Verifie que chaque code existe, et rend le JSON a stocker.
fn valider(permissions: &[String]) -> Result<String, String> {
    let inconnues: Vec<&String> = permissions
        .iter()
        .filter(|c| !crate::portes::existe(c))
        .collect();
    if !inconnues.is_empty() {
        return Err(format!(
            "Permission inconnue : {}. Elle ne correspond à aucune action.",
            inconnues
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    let mut uniques: Vec<String> = permissions.to_vec();
    uniques.sort();
    uniques.dedup();
    Ok(serde_json::json!(uniques).to_string())
}

pub fn creer_role(
    conn: &rusqlite::Connection,
    nom: String,
    description: Option<String>,
    permissions: Vec<String>,
) -> Result<serde_json::Value, String> {
    let nom = nom.trim().to_lowercase();
    if nom.is_empty() {
        return Err("Le nom du rôle est obligatoire".to_string());
    }
    let existe: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM role WHERE nom = ?1",
            rusqlite::params![nom],
            |r| r.get(0),
        )
        .unwrap_or(0);
    if existe > 0 {
        return Err(format!("Le rôle « {nom} » existe déjà"));
    }

    let liste = valider(&permissions)?;
    let id = uuid::Uuid::new_v4().to_string();
    let now = maintenant_iso();
    conn.execute(
        "INSERT INTO role
           (id, nom, permissions, acces_total, protege, description,
            cree_le, modifie_le, origine)
         VALUES (?1, ?2, ?3, 0, 0, ?4, ?5, ?5, 'app')",
        rusqlite::params![id, nom, liste, description, now],
    )
    .map_err(|e| e.to_string())?;

    Ok(serde_json::json!({ "id": id, "nom": nom }))
}

pub fn modifier_role(
    conn: &rusqlite::Connection,
    role_id: String,
    description: Option<String>,
    permissions: Vec<String>,
) -> Result<serde_json::Value, String> {
    let (nom, protege): (String, i64) = conn
        .query_row(
            "SELECT nom, COALESCE(protege, 0) FROM role WHERE id = ?1",
            rusqlite::params![role_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|_| "Rôle introuvable".to_string())?;

    if protege != 0 {
        return Err(format!(
            "Le rôle « {nom} » ne se modifie pas : c'est le compte de secours, \
             celui qui permet de réparer les autres."
        ));
    }

    let liste = valider(&permissions)?;
    conn.execute(
        "UPDATE role SET permissions = ?1, description = ?2, modifie_le = ?3
         WHERE id = ?4",
        rusqlite::params![liste, description, maintenant_iso(), role_id],
    )
    .map_err(|e| e.to_string())?;

    Ok(serde_json::json!({ "id": role_id, "nom": nom }))
}

pub fn supprimer_role(
    conn: &rusqlite::Connection,
    role_id: String,
) -> Result<serde_json::Value, String> {
    let (nom, protege): (String, i64) = conn
        .query_row(
            "SELECT nom, COALESCE(protege, 0) FROM role WHERE id = ?1",
            rusqlite::params![role_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|_| "Rôle introuvable".to_string())?;

    if protege != 0 {
        return Err(format!("Le rôle « {nom} » ne se supprime pas."));
    }

    let porteurs: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM utilisateur WHERE role_id = ?1",
            rusqlite::params![role_id],
            |r| r.get(0),
        )
        .unwrap_or(0);
    if porteurs > 0 {
        return Err(format!(
            "{porteurs} utilisateur(s) ont encore le rôle « {nom} ». \
             Leur en donner un autre d'abord."
        ));
    }

    conn.execute("DELETE FROM role WHERE id = ?1", rusqlite::params![role_id])
        .map_err(|e| e.to_string())?;
    Ok(serde_json::json!({ "supprime": nom }))
}

// =====================================================================
//  LES REGLAGES PERSONNELS
// =====================================================================

/// Ce qu'une personne peut, et d'ou ca vient.
///
/// L'origine compte pour l'ecran : « vient du rôle » ne se decoche pas
/// de la meme facon qu'« ajouté à cette personne ».
pub fn lire_permissions_utilisateur(
    conn: &rusqlite::Connection,
    utilisateur_id: String,
) -> Result<serde_json::Value, String> {
    let role: String = conn
        .query_row(
            "SELECT r.nom FROM utilisateur u
             JOIN role r ON r.id = u.role_id WHERE u.id = ?1",
            rusqlite::params![utilisateur_id],
            |r| r.get(0),
        )
        .map_err(|_| "Utilisateur introuvable".to_string())?;

    let effectives = crate::portes::permissions_de(conn, &utilisateur_id, &role);

    let mut st = conn
        .prepare(
            "SELECT permission, accorde FROM utilisateur_permission
             WHERE utilisateur_id = ?1",
        )
        .map_err(|e| e.to_string())?;
    let reglages: Vec<serde_json::Value> = st
        .query_map(rusqlite::params![utilisateur_id], |r| {
            Ok(serde_json::json!({
                "permission": r.get::<_, String>(0)?,
                "accorde":    r.get::<_, i64>(1)? != 0,
            }))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    let mut liste: Vec<String> = effectives.into_iter().collect();
    liste.sort();

    Ok(serde_json::json!({
        "utilisateur_id": utilisateur_id,
        "role": role,
        "effectives": liste,
        "reglages": reglages,
    }))
}

/// Ajoute, retire, ou revient au role.
///
/// `accorde = None` efface le reglage personnel : la permission
/// redevient ce que le role en dit. Sans ce troisieme etat, « revenir a
/// la normale » demanderait de savoir ce que le role accordait.
pub fn definir_permission_utilisateur(
    conn: &rusqlite::Connection,
    utilisateur_id: String,
    permission: String,
    accorde: Option<bool>,
    par: Option<String>,
) -> Result<serde_json::Value, String> {
    if !crate::portes::existe(&permission) {
        return Err(format!(
            "Permission inconnue : « {permission} ». Elle ne correspond à \
             aucune action."
        ));
    }
    let existe: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM utilisateur WHERE id = ?1",
            rusqlite::params![utilisateur_id],
            |r| r.get(0),
        )
        .unwrap_or(0);
    if existe == 0 {
        return Err("Utilisateur introuvable".to_string());
    }

    match accorde {
        None => {
            conn.execute(
                "DELETE FROM utilisateur_permission
                 WHERE utilisateur_id = ?1 AND permission = ?2",
                rusqlite::params![utilisateur_id, permission],
            )
            .map_err(|e| e.to_string())?;
        }
        Some(valeur) => {
            conn.execute(
                "INSERT INTO utilisateur_permission
                   (utilisateur_id, permission, accorde, cree_le, cree_par)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(utilisateur_id, permission)
                 DO UPDATE SET accorde = ?3, cree_le = ?4, cree_par = ?5",
                rusqlite::params![
                    utilisateur_id,
                    permission,
                    valeur as i64,
                    maintenant_iso(),
                    par
                ],
            )
            .map_err(|e| e.to_string())?;
        }
    }

    lire_permissions_utilisateur(conn, utilisateur_id)
}
