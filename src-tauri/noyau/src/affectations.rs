//! L'affectation comptable en base (v3, E-2 — decision D23).
//!
//! `affectation_comptable` ne garde que ce qui a ete CHANGE, par
//! dossier ; le reste vaut le defaut livre (`coeur::affectations`). La
//! table est cloisonnee : chaque dossier a ses affectations.

use crate::base::Base;
use crate::coeur::affectations as regles;
use crate::parametres;

fn changees(base: &mut Base) -> Result<Vec<(String, String)>, String> {
    let dossier = base.dossier().to_string();
    base.lire_plusieurs(
        "SELECT operation, compte FROM affectation_comptable WHERE dossier_id = ?1",
        &parametres![dossier],
        |r| Ok((r.get::<String>(0)?, r.get::<String>(1)?)),
    )
    .map_err(|e| e.0)
}

/// Le compte de cette operation dans le dossier courant : le reglage,
/// sinon le defaut.
pub fn compte_sur(base: &mut Base, operation: &str) -> Result<String, String> {
    let op = regles::operation(operation).ok_or_else(|| format!("Opération inconnue : « {operation} »."))?;
    let dossier = base.dossier().to_string();
    let regle: Option<String> = base
        .lire_une(
            "SELECT compte FROM affectation_comptable WHERE dossier_id = ?1 AND operation = ?2",
            &parametres![dossier, operation],
            |r| r.get::<String>(0),
        )
        .map_err(|e| e.0)?;
    Ok(regle.unwrap_or_else(|| op.defaut.to_string()))
}

/// Toutes les affectations du dossier courant, pour l'ecran et pour les
/// journaux (E-3) : (cle -> compte).
pub fn table_sur(base: &mut Base) -> Result<std::collections::HashMap<String, String>, String> {
    let mut t: std::collections::HashMap<String, String> =
        regles::OPERATIONS.iter().map(|o| (o.cle.to_string(), o.defaut.to_string())).collect();
    for (op, compte) in changees(base)? {
        if t.contains_key(&op) {
            t.insert(op, compte);
        }
    }
    Ok(t)
}

pub fn lire_sur(base: &mut Base) -> Result<Vec<serde_json::Value>, String> {
    let reglees: std::collections::HashMap<String, String> = changees(base)?.into_iter().collect();
    let mut v = Vec::new();
    for o in regles::OPERATIONS {
        let compte = reglees.get(o.cle).cloned().unwrap_or_else(|| o.defaut.to_string());
        let libelle_compte = crate::plan_comptable::libelle_sur(base, &compte)?;
        v.push(serde_json::json!({
            "operation": o.cle,
            "libelle": o.libelle,
            "groupe": o.groupe,
            "compte": compte,
            "libelle_compte": libelle_compte,
            "defaut": o.defaut,
            "modifiee": reglees.contains_key(o.cle),
            "prefixes": o.prefixes,
        }));
    }
    Ok(v)
}

/// Pose le compte d'une operation dans le dossier courant ; `None` : le
/// defaut revient. Le compte doit exister (plan commun ou sous-compte du
/// dossier) et convenir a l'operation. Au journal.
pub fn definir_sur(base: &mut Base, operation: String, compte: Option<String>) -> Result<serde_json::Value, String> {
    let op = regles::operation(&operation).ok_or_else(|| format!("Opération inconnue : « {operation} »."))?;
    let compte = compte.map(|c| c.trim().to_string()).filter(|c| !c.is_empty());
    if let Some(c) = &compte {
        regles::verifier(&operation, c)?;
        if crate::plan_comptable::libelle_sur(base, c)?.is_none() {
            return Err(format!("Le compte {c} n'existe pas dans le plan : l'ajouter d'abord comme sous-compte."));
        }
    }
    let avant = compte_sur(base, &operation)?;
    let dossier = base.dossier().to_string();
    let auteur = crate::argent::id_utilisateur_courant_sur(base);
    let now = crate::utils::maintenant_iso();
    let mut tx = base.transaction().map_err(|e| e.0)?;
    tx.executer(
        "DELETE FROM affectation_comptable WHERE dossier_id = ?1 AND operation = ?2",
        &parametres![dossier.clone(), operation.clone()],
    )
    .map_err(|e| e.0)?;
    // Le defaut ne s'ecrit pas : il suivra les versions suivantes.
    if let Some(c) = compte.as_ref().filter(|c| c.as_str() != op.defaut) {
        tx.executer(
            "INSERT INTO affectation_comptable (dossier_id, operation, compte, modifie_le, modifie_par)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            &parametres![dossier.clone(), operation.clone(), c.clone(), now.clone(), auteur.clone()],
        )
        .map_err(|e| e.0)?;
    }
    let apres = compte.clone().unwrap_or_else(|| op.defaut.to_string());
    tx.executer(
        "INSERT INTO journal
           (id, type_evenement, entite_type, entite_id, auteur_id, ancien_valeur, nouveau_valeur, origine, date_evenement, dossier_id)
         VALUES (?1, 'affectation_modifiee', 'affectation_comptable', ?2, ?3, ?4, ?5, 'app', ?6, ?7)",
        &parametres![
            uuid::Uuid::new_v4().to_string(),
            operation.clone(),
            auteur,
            serde_json::json!({ "compte": avant }).to_string(),
            serde_json::json!({ "compte": apres }).to_string(),
            now,
            dossier
        ],
    )
    .map_err(|e| e.0)?;
    tx.valider().map_err(|e| e.0)?;
    Ok(serde_json::json!({ "operation": operation, "compte": apres, "modifiee": apres != op.defaut }))
}
