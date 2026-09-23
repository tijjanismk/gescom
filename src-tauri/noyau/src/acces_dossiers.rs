//! Les droits par dossier (v3, C-2 — decision C2).
//!
//! > Ton frere est patron de sa boutique et n'a rien a faire dans la
//! > tienne. Ta comptable voit les deux.
//!
//! Une personne a un role GLOBAL (`utilisateur.role_id`, comme avant) et
//! peut avoir des lignes `utilisateur_dossier (utilisateur, dossier,
//! role)`. La regle, pure (`role_dans`) :
//!
//! - `superadmin` : partout, avec son role — il ne se restreint pas ;
//! - des lignes : EXACTEMENT ces dossiers, avec ces roles — meme si son
//!   role global est `patron` (c'est le cas du frere) ;
//! - aucune ligne : comme avant la v3 — partout s'il a l'acces total,
//!   sinon le dossier d'origine seulement. Tant qu'il n'y a qu'un
//!   dossier, rien ne change.
//!
//! Le sur-mesure (`utilisateur_permission`) reste par personne, tous
//! dossiers : « plus de droits dans A que dans B » se regle par deux
//! roles. Le serveur relit le role a CHAQUE requete (`sessions::etat_sur`)
//! : retirer un dossier ferme la session qui y travaillait.

use crate::base::Base;
use crate::dossiers::DOSSIER_DEFAUT;
use crate::parametres;

/// Le role d'une personne dans un dossier, ou `None` : elle n'y entre
/// pas. `lignes` = (dossier_id, nom du role). Pure.
pub fn role_dans(role_global: &str, acces_total: bool, lignes: &[(String, String)], dossier_id: &str) -> Option<String> {
    if role_global == crate::portes::SUPERADMIN {
        return Some(role_global.to_string());
    }
    if !lignes.is_empty() {
        return lignes.iter().find(|(d, _)| d == dossier_id).map(|(_, r)| r.clone());
    }
    (acces_total || dossier_id == DOSSIER_DEFAUT).then(|| role_global.to_string())
}

struct Personne {
    nom: String,
    role: String,
    acces_total: bool,
    protege: bool,
}

fn personne(base: &mut Base, utilisateur_id: &str) -> Result<Personne, String> {
    base.lire_une(
        "SELECT u.nom, r.nom, COALESCE(r.acces_total, 0), COALESCE(r.protege, 0)
         FROM utilisateur u JOIN role r ON r.id = u.role_id
         WHERE u.id = ?1",
        &parametres![utilisateur_id],
        |r| {
            Ok(Personne {
                nom: r.get(0)?,
                role: r.get(1)?,
                acces_total: r.get::<i64>(2)? != 0,
                protege: r.get::<i64>(3)? != 0,
            })
        },
    )
    .map_err(|e| e.0)?
    .ok_or_else(|| "Compte introuvable.".to_string())
}

fn lignes(base: &mut Base, utilisateur_id: &str) -> Result<Vec<(String, String)>, String> {
    base.lire_plusieurs(
        "SELECT ud.dossier_id, r.nom FROM utilisateur_dossier ud
         JOIN role r ON r.id = ud.role_id
         WHERE ud.utilisateur_id = ?1",
        &parametres![utilisateur_id],
        |r| Ok((r.get::<String>(0)?, r.get::<String>(1)?)),
    )
    .map_err(|e| e.0)
}

/// Le role de cette personne dans ce dossier, lu en base. `None` : pas
/// d'entree. Un compte introuvable n'entre nulle part.
pub fn role_dans_sur(base: &mut Base, utilisateur_id: &str, dossier_id: &str) -> Result<Option<String>, String> {
    let p = match personne(base, utilisateur_id) {
        Ok(p) => p,
        Err(_) => return Ok(None),
    };
    let l = lignes(base, utilisateur_id)?;
    Ok(role_dans(&p.role, p.acces_total, &l, dossier_id))
}

/// Les dossiers ou cette personne entre, parmi `ids`.
pub fn dossiers_ouverts_a_sur(base: &mut Base, utilisateur_id: &str, ids: &[String]) -> Result<Vec<String>, String> {
    let p = personne(base, utilisateur_id)?;
    let l = lignes(base, utilisateur_id)?;
    Ok(ids.iter().filter(|d| role_dans(&p.role, p.acces_total, &l, d).is_some()).cloned().collect())
}

/// Pour l'ecran : chaque dossier, le role qu'y a la personne (ou rien),
/// et si ce sont des lignes posees (`par_dossier`) ou la regle d'avant.
pub fn lire_sur(base: &mut Base, utilisateur_id: &str) -> Result<serde_json::Value, String> {
    let p = personne(base, utilisateur_id)?;
    let l = lignes(base, utilisateur_id)?;
    let dossiers = crate::dossiers::lire_dossiers_sur(base)?;
    let liste: Vec<serde_json::Value> = dossiers
        .iter()
        .map(|d| {
            let id = d["id"].as_str().unwrap_or("");
            serde_json::json!({
                "id": id,
                "code": d["code"],
                "societe": d["societe"],
                "role": role_dans(&p.role, p.acces_total, &l, id),
            })
        })
        .collect();
    Ok(serde_json::json!({
        "role_global": p.role,
        "acces_total": p.acces_total,
        "superadmin": p.role == crate::portes::SUPERADMIN,
        "par_dossier": !l.is_empty(),
        "dossiers": liste,
    }))
}

/// Pose les dossiers d'une personne : `Some(lignes)` = exactement ces
/// dossiers avec ces roles ; `None` = retour a la regle d'avant (plus
/// aucune ligne). Refuse ce qui enfermerait tout le monde dehors.
pub fn definir_sur(
    base: &mut Base,
    utilisateur_id: &str,
    choix: Option<Vec<(String, String)>>,
) -> Result<serde_json::Value, String> {
    let par = crate::argent::id_utilisateur_courant_sur(base);
    let p = personne(base, utilisateur_id)?;
    if utilisateur_id == par {
        return Err("On ne règle pas ses propres dossiers : un autre compte doit le faire.".to_string());
    }
    if p.protege || p.role == crate::portes::SUPERADMIN {
        return Err(format!("« {} » porte le rôle protégé {} : il entre partout.", p.nom, p.role));
    }

    // Chaque ligne : un dossier qui existe, un role qui existe, jamais
    // superadmin (il ne se donne pas par dossier).
    let mut resolues: Vec<(String, String, String)> = Vec::new();
    if let Some(choix) = &choix {
        if choix.is_empty() {
            return Err(format!(
                "Sans aucun dossier, « {} » ne pourrait plus se connecter. Le désactiver plutôt.",
                p.nom
            ));
        }
        for (dossier, role) in choix {
            let d = crate::dossiers::dossier_ouvert_sur(base, dossier)?;
            if role == crate::portes::SUPERADMIN {
                return Err("Le rôle superadmin ne se donne pas par dossier.".to_string());
            }
            let role_id: String = base
                .lire_une("SELECT id FROM role WHERE nom = ?1", &parametres![role.clone()], |r| r.get::<String>(0))
                .map_err(|e| e.0)?
                .ok_or_else(|| format!("Rôle introuvable : {role}"))?;
            if resolues.iter().any(|(x, ..)| *x == d.id) {
                return Err(format!("Le dossier {} est donné deux fois.", d.code));
            }
            resolues.push((d.id, role_id, role.clone()));
        }
    }

    // Qu'il reste quelqu'un qui voit TOUS les dossiers : sans lui, plus
    // personne ne pourrait redonner l'acces a un dossier oublie.
    if p.acces_total && choix.is_some() {
        let autres: i64 = base
            .lire_une(
                "SELECT CAST(COUNT(*) AS BIGINT) FROM utilisateur u JOIN role r ON r.id = u.role_id
                 WHERE u.actif = 1 AND u.id <> ?1
                   AND (r.nom = ?2 OR (r.acces_total = 1 AND NOT EXISTS
                        (SELECT 1 FROM utilisateur_dossier ud WHERE ud.utilisateur_id = u.id)))",
                &parametres![utilisateur_id, crate::portes::SUPERADMIN],
                |r| r.get::<i64>(0),
            )
            .map_err(|e| e.0)?
            .unwrap_or(0);
        if autres == 0 {
            return Err(format!(
                "« {} » est le dernier compte qui voit tous les dossiers : le restreindre \
                 laisserait des dossiers sans personne pour les gérer.",
                p.nom
            ));
        }
    }

    let now = crate::utils::maintenant_iso();
    let dossier_courant = base.dossier().to_string();
    let mut tx = base.transaction().map_err(|e| e.0)?;
    tx.executer("DELETE FROM utilisateur_dossier WHERE utilisateur_id = ?1", &parametres![utilisateur_id])
        .map_err(|e| e.0)?;
    for (dossier, role_id, _) in &resolues {
        tx.executer(
            "INSERT INTO utilisateur_dossier (utilisateur_id, dossier_id, role_id, cree_le, cree_par)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            &parametres![utilisateur_id, dossier.clone(), role_id.clone(), now.clone(), par.clone()],
        )
        .map_err(|e| e.0)?;
    }
    let nouveau = serde_json::json!({
        "nom": p.nom,
        "par_dossier": choix.is_some(),
        "dossiers": resolues.iter().map(|(d, _, r)| serde_json::json!({ "dossier_id": d, "role": r })).collect::<Vec<_>>(),
    });
    tx.executer(
        "INSERT INTO journal
           (id, type_evenement, entite_type, entite_id, auteur_id, nouveau_valeur, origine, date_evenement, dossier_id)
         VALUES (?1, 'droits_dossiers_modifies', 'utilisateur', ?2, ?3, ?4, 'app', ?5, ?6)",
        &parametres![uuid::Uuid::new_v4().to_string(), utilisateur_id, par, nouveau.to_string(), now, dossier_courant],
    )
    .map_err(|e| e.0)?;
    tx.valider().map_err(|e| e.0)?;
    Ok(nouveau)
}

/// Un dossier cree par quelqu'un qui n'a que certains dossiers : il y
/// entre, avec le role qu'il a la ou il l'a cree. Sans cela il
/// creerait un dossier qu'il ne peut pas ouvrir.
pub fn donner_au_createur_sur(
    acces: &mut impl crate::base::Acces,
    createur_id: &str,
    nouveau_dossier: &str,
    role_ici: Option<&str>,
) -> Result<(), String> {
    let Some(role) = role_ici else { return Ok(()) };
    let a_des_lignes: i64 = acces
        .lire_une(
            "SELECT CAST(COUNT(*) AS BIGINT) FROM utilisateur_dossier WHERE utilisateur_id = ?1",
            &parametres![createur_id],
            |r| r.get::<i64>(0),
        )
        .map_err(|e| e.0)?
        .unwrap_or(0);
    if a_des_lignes == 0 {
        return Ok(());
    }
    acces
        .executer(
            "INSERT INTO utilisateur_dossier (utilisateur_id, dossier_id, role_id, cree_le, cree_par)
             SELECT ?1, ?2, r.id, ?3, ?1 FROM role r WHERE r.nom = ?4
             ON CONFLICT (utilisateur_id, dossier_id) DO NOTHING",
            &parametres![createur_id, nouveau_dossier, crate::utils::maintenant_iso(), role],
        )
        .map(|_| ())
        .map_err(|e| e.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn l(v: &[(&str, &str)]) -> Vec<(String, String)> {
        v.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect()
    }

    #[test]
    fn sans_ligne_comme_avant() {
        assert_eq!(role_dans("patron", true, &[], "quinc").as_deref(), Some("patron"), "l'accès total voit tout");
        assert_eq!(role_dans("caissier", false, &[], DOSSIER_DEFAUT).as_deref(), Some("caissier"));
        assert_eq!(role_dans("caissier", false, &[], "quinc"), None, "un dossier neuf ne s'ouvre pas tout seul");
    }

    #[test]
    fn des_lignes_disent_exactement_ou_et_avec_quoi() {
        let frere = l(&[("quinc", "patron")]);
        assert_eq!(role_dans("patron", true, &frere, "quinc").as_deref(), Some("patron"));
        assert_eq!(role_dans("patron", true, &frere, DOSSIER_DEFAUT), None, "le frère n'a rien à faire chez toi");
        let comptable = l(&[(DOSSIER_DEFAUT, "comptable"), ("quinc", "caissier")]);
        assert_eq!(role_dans("comptable", false, &comptable, "quinc").as_deref(), Some("caissier"), "un rôle par dossier");
    }

    #[test]
    fn le_superadmin_entre_partout() {
        assert_eq!(role_dans("superadmin", true, &l(&[("quinc", "caissier")]), "autre").as_deref(), Some("superadmin"));
    }
}
