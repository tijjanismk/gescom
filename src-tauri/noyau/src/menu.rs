//! D37 — le menu d'un dossier : ce qu'il cache.
//!
//! Un reglage d'affichage, par dossier (`config_app`, cle
//! `menu_caches:<dossier>`, une liste JSON d'ecrans). Les regles —
//! quoi peut se cacher, « Boutique simple » — sont dans `coeur::menu`.
//! Version `Base` seule : la fenetre ne passe plus par les facades
//! `Connection` (D22 revise), comme les autres reglages de la v3.

use crate::base::{Acces, Base};
use crate::coeur::menu as regles;
use crate::parametres;

fn cle(dossier: &str) -> String {
    format!("menu_caches:{dossier}")
}

/// Ce que le menu de ce dossier cache, et de quoi regler l'ecran :
/// les entrees masquables, les toujours visibles, « Boutique simple ».
pub fn lire_menu_sur(acces: &mut impl Acces) -> Result<serde_json::Value, String> {
    let dossier = acces.dossier().to_string();
    let brut: Option<String> = acces
        .lire_une("SELECT valeur FROM config_app WHERE cle = ?1", &parametres![cle(&dossier)], |r| r.get::<String>(0))
        .map_err(|e| e.0)?;
    // Une valeur illisible ou une entree disparue d'une version a
    // l'autre : on n'en garde que ce qui se cache encore.
    let lus: Vec<String> = brut.and_then(|b| serde_json::from_str(&b).ok()).unwrap_or_default();
    let caches: Vec<String> = regles::MASQUABLES
        .iter()
        .map(|(h, _)| h.to_string())
        .filter(|h| lus.contains(h))
        .collect();
    let paires = |l: &[(&str, &str)]| -> Vec<serde_json::Value> {
        l.iter().map(|(h, n)| serde_json::json!({ "href": h, "nom": n })).collect()
    };
    Ok(serde_json::json!({
        "caches": caches,
        "masquables": paires(regles::MASQUABLES),
        "toujours": paires(regles::TOUJOURS_VISIBLES),
        "boutique_simple": regles::caches_boutique_simple(),
    }))
}

/// Regle ce que le menu de ce dossier cache. Le serveur juge la liste
/// (`coeur::menu::valider_caches`) ; le geste est trace.
pub fn definir_menu_sur(base: &mut Base, caches: Vec<String>) -> Result<serde_json::Value, String> {
    let caches = regles::valider_caches(&caches)?;
    let dossier = base.dossier().to_string();
    let avant = lire_menu_sur(base)?["caches"].clone();
    let auteur = crate::argent::id_utilisateur_courant_sur(base);
    let maintenant = crate::utils::maintenant_iso();
    let valeur = serde_json::to_string(&caches).map_err(|e| e.to_string())?;

    let mut tx = base.transaction().map_err(|e| e.0)?;
    tx.executer(
        "INSERT INTO config_app (cle, valeur) VALUES (?1, ?2)
         ON CONFLICT (cle) DO UPDATE SET valeur = excluded.valeur",
        &parametres![cle(&dossier), valeur.clone()],
    )
    .map_err(|e| e.0)?;
    tx.executer(
        "INSERT INTO journal
           (id, type_evenement, entite_type, entite_id, auteur_id,
            ancien_valeur, nouveau_valeur, origine, date_evenement, dossier_id)
         VALUES (?1,'menu_modifie','dossier',?2,?3,?4,?5,'app',?6,?2)",
        &parametres![
            uuid::Uuid::new_v4().to_string(),
            dossier,
            auteur,
            serde_json::json!({ "caches": avant }).to_string(),
            serde_json::json!({ "caches": caches }).to_string(),
            maintenant
        ],
    )
    .map_err(|e| e.0)?;
    tx.valider().map_err(|e| e.0)?;
    lire_menu_sur(base)
}
