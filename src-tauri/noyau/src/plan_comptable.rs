//! Le plan comptable en base (v3, E-1 — decision D23).
//!
//! Le plan SYSCOHADA (`coeur::plan_comptable::SYSCOHADA`) est seme une
//! fois, commun a tous les dossiers (`dossier_id` vide). Un sous-compte
//! (`4111 Client Coulibaly`) appartient au dossier ou on l'ajoute. La
//! table n'est pas dans `TABLES_CLOISONNEES` : chaque lecture filtre
//! elle-meme `dossier_id IN ('', dossier courant)`.

use crate::base::Base;
use crate::coeur::plan_comptable as regles;
use crate::parametres;

/// Seme le plan commun. Idempotent (`ON CONFLICT DO NOTHING`) : une
/// base installee le recoit au premier demarrage v3, et un compte du
/// plan dont le patron aurait change le libelle n'est pas ecrase.
pub fn semer_sur(base: &mut Base) -> Result<usize, String> {
    let now = crate::utils::maintenant_iso();
    let numeros: Vec<&str> = regles::SYSCOHADA.iter().map(|(n, _)| *n).collect();
    let mut tx = base.transaction().map_err(|e| e.0)?;
    let mut poses = 0;
    for (numero, libelle) in regles::SYSCOHADA {
        let parent = regles::parent_de(numero, numeros.iter().copied());
        let classe = regles::classe(numero).unwrap_or(0) as i64;
        poses += tx
            .executer(
                "INSERT INTO compte_comptable (numero, dossier_id, libelle, classe, parent, origine, cree_le)
                 VALUES (?1, '', ?2, ?3, ?4, 'syscohada', ?5)
                 ON CONFLICT (numero, dossier_id) DO NOTHING",
                &parametres![*numero, *libelle, classe, parent, now.clone()],
            )
            .map_err(|e| e.0)? as usize;
    }
    tx.valider().map_err(|e| e.0)?;
    Ok(poses)
}

fn numeros_visibles(base: &mut Base) -> Result<Vec<String>, String> {
    let dossier = base.dossier().to_string();
    base.lire_plusieurs(
        "SELECT numero FROM compte_comptable WHERE dossier_id IN ('', ?1)",
        &parametres![dossier],
        |r| r.get::<String>(0),
    )
    .map_err(|e| e.0)
}

/// Le plan du dossier courant : le commun et ses sous-comptes, par
/// numero (l'ordre du plan : `4`, `40`, `401`, `4011`, `41`…).
pub fn lire_sur(base: &mut Base) -> Result<Vec<serde_json::Value>, String> {
    let dossier = base.dossier().to_string();
    base.lire_plusieurs(
        "SELECT numero, libelle, classe, parent, dossier_id FROM compte_comptable
         WHERE dossier_id IN ('', ?1) ORDER BY numero ASC",
        &parametres![dossier],
        |r| {
            let d: String = r.get(4)?;
            Ok(serde_json::json!({
                "numero": r.get::<String>(0)?,
                "libelle": r.get::<String>(1)?,
                "classe": r.get::<i64>(2)?,
                "parent": r.get::<Option<String>>(3)?,
                "sous_compte": !d.is_empty(),
            }))
        },
    )
    .map_err(|e| e.0)
}

/// Le libelle d'un compte visible depuis le dossier courant.
pub fn libelle_sur(base: &mut Base, numero: &str) -> Result<Option<String>, String> {
    let dossier = base.dossier().to_string();
    base.lire_une(
        "SELECT libelle FROM compte_comptable WHERE numero = ?1 AND dossier_id IN ('', ?2)
         ORDER BY dossier_id DESC",
        &parametres![numero, dossier],
        |r| r.get::<String>(0),
    )
    .map_err(|e| e.0)
}

/// Ajoute un sous-compte AU DOSSIER COURANT. Au journal.
pub fn ajouter_sous_compte_sur(base: &mut Base, numero: String, libelle: String) -> Result<serde_json::Value, String> {
    let connus = numeros_visibles(base)?;
    let (numero, libelle, parent) = regles::valider_sous_compte(&numero, &libelle, connus.iter().map(String::as_str))?;
    let dossier = base.dossier().to_string();
    let auteur = crate::argent::id_utilisateur_courant_sur(base);
    let now = crate::utils::maintenant_iso();
    let classe = regles::classe(&numero).unwrap_or(0) as i64;
    let mut tx = base.transaction().map_err(|e| e.0)?;
    tx.executer(
        "INSERT INTO compte_comptable (numero, dossier_id, libelle, classe, parent, origine, cree_le)
         VALUES (?1, ?2, ?3, ?4, ?5, 'dossier', ?6)",
        &parametres![numero.clone(), dossier.clone(), libelle.clone(), classe, parent.clone(), now.clone()],
    )
    .map_err(|e| e.0)?;
    tx.executer(
        "INSERT INTO journal
           (id, type_evenement, entite_type, entite_id, auteur_id, nouveau_valeur, origine, date_evenement, dossier_id)
         VALUES (?1, 'sous_compte_cree', 'compte_comptable', ?2, ?3, ?4, 'app', ?5, ?6)",
        &parametres![
            uuid::Uuid::new_v4().to_string(),
            numero.clone(),
            auteur,
            serde_json::json!({ "numero": numero, "libelle": libelle, "parent": parent }).to_string(),
            now,
            dossier
        ],
    )
    .map_err(|e| e.0)?;
    tx.valider().map_err(|e| e.0)?;
    Ok(serde_json::json!({ "numero": numero, "libelle": libelle, "parent": parent, "classe": classe }))
}
