//! Les plafonds en base (v3, C-3) : ceux du role, le sur-mesure d'une
//! personne, et ce que vaut le plafond de celle qui agit.
//!
//! La regle (fusion, verification, message) est dans
//! `coeur::plafonds`. Ici : lire, ecrire, et `exiger_*` pour les gestes
//! dont le montant n'est connu qu'au fond du noyau (le remboursement
//! d'un retour, d'un reglement annule). Les gestes dont l'argument
//! suffit (remise, credit d'une vente, remboursement d'un avoir) se
//! jugent dans la poignee du serveur, comme `pieces:antidater`.

use rusqlite::Connection;

use crate::base::{Acces, Base};
use crate::coeur::plafonds::{self as regle, Plafonds};
use crate::parametres;
use crate::utils::maintenant_iso;

const LIRE: &str = "SELECT COALESCE(r.acces_total, 0), r.nom,
                r.remise_max_pct, r.remboursement_max, r.credit_max,
                up.remise_max_pct, up.remboursement_max, up.credit_max
         FROM utilisateur u
         JOIN role r ON r.id = u.role_id
         LEFT JOIN utilisateur_plafond up ON up.utilisateur_id = u.id
         WHERE u.id = ?1";

type Brut = (i64, String, Option<f64>, Option<i64>, Option<i64>, Option<f64>, Option<i64>, Option<i64>);

fn fondre(b: Brut) -> Plafonds {
    let (acces_total, role, rr, rb, rc, pr, pb, pc) = b;
    // Le patron et le compte de secours n'ont pas de plafond.
    if acces_total != 0 || role == crate::portes::SUPERADMIN {
        return Plafonds::default();
    }
    regle::fusionner(
        &Plafonds { remise_max_pct: rr, remboursement_max: rb, credit_max: rc },
        &Plafonds { remise_max_pct: pr, remboursement_max: pb, credit_max: pc },
    )
}

/// Les plafonds d'une personne (role + sur-mesure).
pub fn de(conn: &Connection, utilisateur_id: &str) -> Plafonds {
    conn.query_row(LIRE, rusqlite::params![utilisateur_id], |r| {
        Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?))
    })
    .map(fondre)
    .unwrap_or_default()
}

/// La meme chose, sur l'un ou l'autre moteur.
pub fn de_sur(acces: &mut impl Acces, utilisateur_id: &str) -> Plafonds {
    acces
        .lire_une(LIRE, &parametres![utilisateur_id], |r| {
            Ok((
                r.get::<i64>(0)?,
                r.get::<String>(1)?,
                r.get::<Option<f64>>(2)?,
                r.get::<Option<i64>>(3)?,
                r.get::<Option<i64>>(4)?,
                r.get::<Option<f64>>(5)?,
                r.get::<Option<i64>>(6)?,
                r.get::<Option<i64>>(7)?,
            ))
        })
        .ok()
        .flatten()
        .map(fondre)
        .unwrap_or_default()
}

/// Le remboursement de la personne qui agit (celle de la session, D26)
/// ne depasse pas son plafond. Hors serveur (fenetre monoposte, tests
/// sans auteur), personne n'est pose : pas de plafond.
pub fn exiger_remboursement(conn: &Connection, montant: i64) -> Result<(), String> {
    match crate::auteur::courant() {
        Some(id) => regle::verifier_remboursement(montant, &de(conn, &id)),
        None => Ok(()),
    }
}

pub fn exiger_remboursement_sur(acces: &mut impl Acces, montant: i64) -> Result<(), String> {
    match crate::auteur::courant() {
        Some(id) => {
            let p = de_sur(acces, &id);
            regle::verifier_remboursement(montant, &p)
        }
        None => Ok(()),
    }
}

fn plafonds_json(p: &Plafonds) -> serde_json::Value {
    serde_json::to_value(p).unwrap_or(serde_json::Value::Null)
}

/// Pour l'ecran : les plafonds de chaque role et le sur-mesure de
/// chaque personne qui en a un.
pub fn lire_sur(base: &mut Base) -> Result<serde_json::Value, String> {
    let roles = base
        .lire_plusieurs(
            "SELECT nom, COALESCE(acces_total, 0), remise_max_pct, remboursement_max, credit_max
             FROM role ORDER BY nom",
            &[],
            |r| {
                let p = Plafonds {
                    remise_max_pct: r.get::<Option<f64>>(2)?,
                    remboursement_max: r.get::<Option<i64>>(3)?,
                    credit_max: r.get::<Option<i64>>(4)?,
                };
                Ok(serde_json::json!({
                    "nom": r.get::<String>(0)?,
                    "acces_total": r.get::<i64>(1)? != 0,
                    "plafonds": plafonds_json(&p),
                }))
            },
        )
        .map_err(|e| e.0)?;
    let personnes = base
        .lire_plusieurs(
            "SELECT up.utilisateur_id, u.nom, up.remise_max_pct, up.remboursement_max, up.credit_max
             FROM utilisateur_plafond up JOIN utilisateur u ON u.id = up.utilisateur_id
             ORDER BY u.nom",
            &[],
            |r| {
                let p = Plafonds {
                    remise_max_pct: r.get::<Option<f64>>(2)?,
                    remboursement_max: r.get::<Option<i64>>(3)?,
                    credit_max: r.get::<Option<i64>>(4)?,
                };
                Ok(serde_json::json!({
                    "utilisateur_id": r.get::<String>(0)?,
                    "nom": r.get::<String>(1)?,
                    "plafonds": plafonds_json(&p),
                }))
            },
        )
        .map_err(|e| e.0)?;
    Ok(serde_json::json!({ "roles": roles, "personnes": personnes }))
}

/// Les plafonds d'un role. Refuse sur un role a acces total : il n'en
/// aurait aucun effet, et l'ecran ferait croire le contraire.
pub fn definir_role_sur(base: &mut Base, role: &str, p: Plafonds) -> Result<serde_json::Value, String> {
    regle::valider(&p)?;
    let acces_total: i64 = base
        .lire_une("SELECT COALESCE(acces_total, 0) FROM role WHERE nom = ?1", &parametres![role], |r| r.get::<i64>(0))
        .map_err(|e| e.0)?
        .ok_or_else(|| format!("Rôle « {role} » introuvable."))?;
    if acces_total != 0 {
        return Err(format!("Le rôle « {role} » donne tout : un plafond n'y aurait aucun effet."));
    }
    let par = crate::argent::id_utilisateur_courant_sur(base);
    let maintenant = maintenant_iso();
    let dossier = base.dossier().to_string();
    let mut tx = base.transaction().map_err(|e| e.0)?;
    tx.executer(
        "UPDATE role SET remise_max_pct = ?1, remboursement_max = ?2, credit_max = ?3, modifie_le = ?4
         WHERE nom = ?5",
        &parametres![p.remise_max_pct, p.remboursement_max, p.credit_max, maintenant.clone(), role],
    )
    .map_err(|e| e.0)?;
    journaliser(&mut tx, "role", role, &par, &p, &maintenant, &dossier)?;
    tx.valider().map_err(|e| e.0)?;
    Ok(serde_json::json!({ "role": role, "plafonds": plafonds_json(&p) }))
}

/// Le sur-mesure d'une personne. Tout vide : la personne suit son role.
pub fn definir_utilisateur_sur(base: &mut Base, utilisateur_id: &str, p: Plafonds) -> Result<serde_json::Value, String> {
    regle::valider(&p)?;
    let existe = base
        .lire_une("SELECT 1 FROM utilisateur WHERE id = ?1", &parametres![utilisateur_id], |r| r.get::<i64>(0))
        .map_err(|e| e.0)?
        .is_some();
    if !existe {
        return Err("Compte introuvable.".to_string());
    }
    let par = crate::argent::id_utilisateur_courant_sur(base);
    let maintenant = maintenant_iso();
    let dossier = base.dossier().to_string();
    let mut tx = base.transaction().map_err(|e| e.0)?;
    if p.est_vide() {
        tx.executer("DELETE FROM utilisateur_plafond WHERE utilisateur_id = ?1", &parametres![utilisateur_id])
            .map_err(|e| e.0)?;
    } else {
        tx.executer(
            "INSERT INTO utilisateur_plafond
               (utilisateur_id, remise_max_pct, remboursement_max, credit_max, modifie_le, modifie_par)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT (utilisateur_id) DO UPDATE SET
               remise_max_pct = excluded.remise_max_pct,
               remboursement_max = excluded.remboursement_max,
               credit_max = excluded.credit_max,
               modifie_le = excluded.modifie_le,
               modifie_par = excluded.modifie_par",
            &parametres![
                utilisateur_id, p.remise_max_pct, p.remboursement_max, p.credit_max,
                maintenant.clone(), par.clone()
            ],
        )
        .map_err(|e| e.0)?;
    }
    journaliser(&mut tx, "utilisateur", utilisateur_id, &par, &p, &maintenant, &dossier)?;
    tx.valider().map_err(|e| e.0)?;
    Ok(serde_json::json!({ "utilisateur_id": utilisateur_id, "plafonds": plafonds_json(&p) }))
}

fn journaliser(
    acces: &mut impl Acces,
    entite_type: &str,
    entite_id: &str,
    par: &str,
    p: &Plafonds,
    maintenant: &str,
    dossier: &str,
) -> Result<(), String> {
    acces
        .executer(
            "INSERT INTO journal
               (id, type_evenement, entite_type, entite_id, auteur_id,
                nouveau_valeur, origine, date_evenement, dossier_id)
             VALUES (?1, 'plafonds_modifies', ?2, ?3, ?4, ?5, 'app', ?6, ?7)",
            &parametres![
                uuid::Uuid::new_v4().to_string(),
                entite_type,
                entite_id,
                par,
                plafonds_json(p).to_string(),
                maintenant,
                dossier
            ],
        )
        .map(|_| ())
        .map_err(|e| e.0)
}
