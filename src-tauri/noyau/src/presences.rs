//! Les jours travailles en base (PLAN-EQUIPE, F-3). Version `Base`
//! seule (D29). Table `presence`, cloisonnee : une ligne par personne
//! et par jour marque ; un jour sans ligne n'est pas su.

use std::collections::HashMap;

use crate::base::Base;
use crate::coeur::presences as regles;
use crate::parametres;

fn aujourd_hui() -> String {
    crate::utils::maintenant_iso()[..10].to_string()
}

/// La grille d'un mois `AAAA-MM` : les jours, et chaque personne qui a
/// ete la pendant ce mois (active, ou partie dedans), avec ses etats et
/// son compte de jours.
pub fn lire_mois_sur(base: &mut Base, mois: &str) -> Result<serde_json::Value, String> {
    let jours = regles::jours_du_mois(mois)?;
    let (premier, dernier) = (jours[0].clone(), jours[jours.len() - 1].clone());
    let dossier = base.dossier().to_string();
    let personnes = base
        .lire_plusieurs(
            "SELECT id, nom, fonction, date_entree, date_depart FROM employe
             WHERE dossier_id = ?1
               AND (date_entree IS NULL OR date_entree <= ?3)
               AND (statut = 'actif' OR date_depart >= ?2)
             ORDER BY LOWER(nom)",
            &parametres![dossier.clone(), premier.clone(), dernier.clone()],
            |r| {
                Ok((
                    r.get::<String>(0)?,
                    r.get::<String>(1)?,
                    r.get::<String>(2)?,
                    r.get::<Option<String>>(3)?,
                    r.get::<Option<String>>(4)?,
                ))
            },
        )
        .map_err(|e| e.0)?;
    let marques = base
        .lire_plusieurs(
            "SELECT employe_id, jour, etat FROM presence
             WHERE dossier_id = ?1 AND jour >= ?2 AND jour <= ?3",
            &parametres![dossier, premier, dernier],
            |r| Ok((r.get::<String>(0)?, r.get::<String>(1)?, r.get::<String>(2)?)),
        )
        .map_err(|e| e.0)?;
    let mut par_personne: HashMap<String, serde_json::Map<String, serde_json::Value>> = HashMap::new();
    for (e, j, etat) in marques {
        par_personne.entry(e).or_default().insert(j, serde_json::Value::String(etat));
    }
    let lignes: Vec<serde_json::Value> = personnes
        .into_iter()
        .map(|(id, nom, fonction, entree, depart)| {
            let etats = par_personne.remove(&id).unwrap_or_default();
            let total = regles::jours_travailles(etats.values().filter_map(|v| v.as_str()));
            serde_json::json!({
                "employe_id": id, "nom": nom, "fonction": fonction,
                "date_entree": entree, "date_depart": depart,
                "etats": etats, "jours_travailles": total,
            })
        })
        .collect();
    Ok(serde_json::json!({ "mois": mois.trim(), "jours": jours, "aujourd_hui": aujourd_hui(), "personnes": lignes }))
}

/// Le compte de jours travailles d'une personne sur une periode — ce que
/// la paie d'un journalier lira (G-2).
pub fn jours_travailles_sur(base: &mut Base, employe_id: &str, du: &str, au: &str) -> Result<f64, String> {
    let dossier = base.dossier().to_string();
    let etats = base
        .lire_plusieurs(
            "SELECT etat FROM presence WHERE dossier_id = ?1 AND employe_id = ?2 AND jour >= ?3 AND jour <= ?4",
            &parametres![dossier, employe_id, du, au],
            |r| r.get::<String>(0),
        )
        .map_err(|e| e.0)?;
    Ok(regles::jours_travailles(etats.iter().map(String::as_str)))
}

fn employe_marquable(base: &mut Base, employe_id: &str, jour: &str) -> Result<String, String> {
    let dossier = base.dossier().to_string();
    let (nom, entree, depart): (String, Option<String>, Option<String>) = base
        .lire_une(
            "SELECT nom, date_entree, date_depart FROM employe WHERE id = ?1 AND dossier_id = ?2",
            &parametres![employe_id, dossier],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .map_err(|e| e.0)?
        .ok_or_else(|| "Fiche introuvable.".to_string())?;
    regles::marquable(jour, &aujourd_hui(), entree.as_deref(), depart.as_deref()).map_err(|e| format!("{nom} : {e}"))?;
    Ok(nom)
}

fn jour_lisible(jour: &str) -> Result<String, String> {
    crate::coeur::dates::jour(jour.trim())
        .map(|d| d.format("%Y-%m-%d").to_string())
        .ok_or_else(|| format!("Jour illisible : « {} »", jour.trim()))
}

/// Marque (ou efface, `etat = None`) un jour d'une personne.
pub fn marquer_sur(base: &mut Base, employe_id: String, jour: String, etat: Option<String>) -> Result<serde_json::Value, String> {
    let jour = jour_lisible(&jour)?;
    if let Some(e) = &etat {
        if regles::valeur(e).is_none() {
            return Err(format!("État inconnu : « {e} ». Attendu : {}.", regles::ETATS.join(", ")));
        }
    }
    employe_marquable(base, &employe_id, &jour)?;
    let dossier = base.dossier().to_string();
    let auteur = crate::argent::id_utilisateur_courant_sur(base);
    let mut tx = base.transaction().map_err(|e| e.0)?;
    tx.executer(
        "DELETE FROM presence WHERE dossier_id = ?1 AND employe_id = ?2 AND jour = ?3",
        &parametres![dossier.clone(), employe_id.clone(), jour.clone()],
    )
    .map_err(|e| e.0)?;
    if let Some(e) = &etat {
        tx.executer(
            "INSERT INTO presence (id, dossier_id, employe_id, jour, etat, saisi_par, saisi_le)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            &parametres![
                uuid::Uuid::new_v4().to_string(),
                dossier,
                employe_id.clone(),
                jour.clone(),
                e.clone(),
                auteur,
                crate::utils::maintenant_iso()
            ],
        )
        .map_err(|e| e.0)?;
    }
    tx.valider().map_err(|e| e.0)?;
    Ok(serde_json::json!({ "employe_id": employe_id, "jour": jour, "etat": etat }))
}

/// « Tout le monde est venu » : marque present, pour ce jour, chaque
/// personne active qui ne l'est pas encore et qui peut l'etre. Ne
/// touche pas a un jour deja marque (une absence reste une absence).
pub fn tous_presents_sur(base: &mut Base, jour: String) -> Result<serde_json::Value, String> {
    let jour = jour_lisible(&jour)?;
    if jour > aujourd_hui() {
        return Err(format!("Le {} n'est pas encore arrivé.", crate::coeur::dates::en_lettres(&jour)));
    }
    let dossier = base.dossier().to_string();
    let candidats = base
        .lire_plusieurs(
            "SELECT e.id FROM employe e
             WHERE e.dossier_id = ?1 AND e.statut = 'actif'
               AND (e.date_entree IS NULL OR e.date_entree <= ?2)
               AND NOT EXISTS (SELECT 1 FROM presence p
                               WHERE p.dossier_id = ?1 AND p.employe_id = e.id AND p.jour = ?2)",
            &parametres![dossier.clone(), jour.clone()],
            |r| r.get::<String>(0),
        )
        .map_err(|e| e.0)?;
    let auteur = crate::argent::id_utilisateur_courant_sur(base);
    let now = crate::utils::maintenant_iso();
    let mut tx = base.transaction().map_err(|e| e.0)?;
    for id in &candidats {
        tx.executer(
            "INSERT INTO presence (id, dossier_id, employe_id, jour, etat, saisi_par, saisi_le)
             VALUES (?1, ?2, ?3, ?4, 'present', ?5, ?6)",
            &parametres![uuid::Uuid::new_v4().to_string(), dossier.clone(), id.clone(), jour.clone(), auteur.clone(), now.clone()],
        )
        .map_err(|e| e.0)?;
    }
    tx.valider().map_err(|e| e.0)?;
    Ok(serde_json::json!({ "jour": jour, "marques": candidats.len() }))
}
