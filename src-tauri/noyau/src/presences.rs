//! Les jours travailles en base (PLAN-EQUIPE, F-3). Version `Base`
//! seule (D29). Table `presence`, cloisonnee : une ligne par personne
//! et par jour marque ; un jour sans ligne n'est pas su.
//!
//! Un jour couvert par une fiche de paie validee ne se marque plus
//! (revue du 28/10, point 5) : la fiche est payee, un jour change apres
//! ne le serait nulle part. Une rectificative en brouillon le rouvre.

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
            "SELECT employe_id, jour, etat, motif FROM presence
             WHERE dossier_id = ?1 AND jour >= ?2 AND jour <= ?3",
            &parametres![dossier, premier, dernier],
            |r| Ok((r.get::<String>(0)?, r.get::<String>(1)?, r.get::<String>(2)?, r.get::<Option<String>>(3)?)),
        )
        .map_err(|e| e.0)?;
    let mut par_personne: HashMap<String, serde_json::Map<String, serde_json::Value>> = HashMap::new();
    // D52 : le motif des jours A et ½, a part pour que `etats` reste ce
    // qu'il etait ; un jour non travaille d'avant D52 se lit « absence ».
    let mut motifs: HashMap<String, serde_json::Map<String, serde_json::Value>> = HashMap::new();
    for (e, j, etat, motif) in marques {
        if etat == "absent" || etat == "demi" {
            let m = motif.unwrap_or_else(|| "absence".to_string());
            motifs.entry(e.clone()).or_default().insert(j.clone(), serde_json::Value::String(m));
        }
        par_personne.entry(e).or_default().insert(j, serde_json::Value::String(etat));
    }
    // Les fiches validees (non en cours de rectification) qui touchent
    // le mois : leurs jours ne se marquent plus (point 5).
    let dossier = base.dossier().to_string();
    let validees = base
        .lire_plusieurs(
            "SELECT f.employe_id, f.du, f.au, f.numero FROM fiche_paie f
             WHERE f.dossier_id = ?1 AND f.statut = 'validee' AND f.du <= ?3 AND f.au >= ?2
               AND NOT EXISTS (SELECT 1 FROM fiche_paie r
                               WHERE r.rectifie_id = f.id AND r.dossier_id = f.dossier_id AND r.statut = 'brouillon')",
            &parametres![dossier, jours[0].clone(), jours[jours.len() - 1].clone()],
            |r| Ok((r.get::<String>(0)?, r.get::<String>(1)?, r.get::<String>(2)?, r.get::<Option<String>>(3)?)),
        )
        .map_err(|e| e.0)?;
    let mut figes: HashMap<String, Vec<serde_json::Value>> = HashMap::new();
    for (e, du, au, numero) in validees {
        figes.entry(e).or_default().push(serde_json::json!({ "du": du, "au": au, "numero": numero }));
    }
    let lignes: Vec<serde_json::Value> = personnes
        .into_iter()
        .map(|(id, nom, fonction, entree, depart)| {
            let etats = par_personne.remove(&id).unwrap_or_default();
            let total = regles::jours_travailles(etats.values().filter_map(|v| v.as_str()));
            let fiches = figes.remove(&id).unwrap_or_default();
            let motifs = motifs.remove(&id).unwrap_or_default();
            serde_json::json!({
                "employe_id": id, "nom": nom, "fonction": fonction,
                "date_entree": entree, "date_depart": depart,
                "etats": etats, "motifs": motifs, "jours_travailles": total, "fiches_validees": fiches,
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
    marquer_avec_motif_sur(base, employe_id, jour, etat, None)
}

/// Marque un jour avec le motif d'un jour non travaille (D52) : maladie,
/// permission, conge, absence (ce que vaut un A ou un ½ sans motif). Le
/// motif ne change pas la paie ; sur un jour fige par une fiche validee,
/// il se corrige encore, l'etat non.
pub fn marquer_avec_motif_sur(
    base: &mut Base,
    employe_id: String,
    jour: String,
    etat: Option<String>,
    motif: Option<String>,
) -> Result<serde_json::Value, String> {
    let jour = jour_lisible(&jour)?;
    if let Some(e) = &etat {
        if regles::valeur(e).is_none() {
            return Err(format!("État inconnu : « {e} ». Attendu : {}.", regles::ETATS.join(", ")));
        }
    }
    let motif = regles::verifier_motif(etat.as_deref(), motif.as_deref())?;
    let nom = employe_marquable(base, &employe_id, &jour)?;
    let dossier = base.dossier().to_string();
    let auteur = crate::argent::id_utilisateur_courant_sur(base);
    let mut tx = base.transaction().map_err(|e| e.0)?;
    // Revue du 28/10, point 5 : un jour paye par une fiche validee est
    // fige — le marquer ensuite ne serait paye nulle part. Juge dans la
    // transaction qui ecrit. D52 : le motif seul se corrige encore (le
    // certificat arrive apres la paie), la paie ne le lit pas.
    if let Some(numero) = crate::paie::fiche_validee_du_jour_sur(&mut tx, &employe_id, &jour)? {
        let actuel = tx
            .lire_une(
                "SELECT etat FROM presence WHERE dossier_id = ?1 AND employe_id = ?2 AND jour = ?3",
                &parametres![dossier.clone(), employe_id.clone(), jour.clone()],
                |r| r.get::<String>(0),
            )
            .map_err(|e| e.0)?;
        if actuel.is_none() || actuel != etat {
            return Err(regles::refus_jour_fige(&nom, &numero));
        }
        tx.executer(
            "UPDATE presence SET motif = ?1, saisi_par = ?2, saisi_le = ?3
             WHERE dossier_id = ?4 AND employe_id = ?5 AND jour = ?6",
            &parametres![motif.clone(), auteur, crate::utils::maintenant_iso(), dossier, employe_id.clone(), jour.clone()],
        )
        .map_err(|e| e.0)?;
        tx.valider().map_err(|e| e.0)?;
        return Ok(serde_json::json!({ "employe_id": employe_id, "jour": jour, "etat": etat, "motif": motif }));
    }
    tx.executer(
        "DELETE FROM presence WHERE dossier_id = ?1 AND employe_id = ?2 AND jour = ?3",
        &parametres![dossier.clone(), employe_id.clone(), jour.clone()],
    )
    .map_err(|e| e.0)?;
    if let Some(e) = &etat {
        tx.executer(
            "INSERT INTO presence (id, dossier_id, employe_id, jour, etat, motif, saisi_par, saisi_le)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            &parametres![
                uuid::Uuid::new_v4().to_string(),
                dossier,
                employe_id.clone(),
                jour.clone(),
                e.clone(),
                motif.clone(),
                auteur,
                crate::utils::maintenant_iso()
            ],
        )
        .map_err(|e| e.0)?;
    }
    tx.valider().map_err(|e| e.0)?;
    Ok(serde_json::json!({ "employe_id": employe_id, "jour": jour, "etat": etat, "motif": motif }))
}

/// Le compte de l'annee civile `AAAA` par motif (D52), pour chaque
/// personne du dossier qui a au moins un jour A ou ½ : ce que la fiche
/// affiche. En jours, une demi-journee pour ½.
pub fn absences_annee_sur(base: &mut Base, annee: &str) -> Result<serde_json::Value, String> {
    let annee = annee.trim();
    if annee.len() != 4 || annee.parse::<u16>().is_err() {
        return Err(format!("Année illisible : « {annee} » (attendu AAAA)."));
    }
    let dossier = base.dossier().to_string();
    let lignes = base
        .lire_plusieurs(
            "SELECT employe_id, etat, motif FROM presence
             WHERE dossier_id = ?1 AND jour >= ?2 AND jour <= ?3 AND etat IN ('absent', 'demi')
             ORDER BY employe_id",
            &parametres![dossier, format!("{annee}-01-01"), format!("{annee}-12-31")],
            |r| Ok((r.get::<String>(0)?, r.get::<String>(1)?, r.get::<Option<String>>(2)?)),
        )
        .map_err(|e| e.0)?;
    let mut par_personne: Vec<(String, Vec<(String, Option<String>)>)> = Vec::new();
    for (e, etat, motif) in lignes {
        match par_personne.last_mut() {
            Some((id, v)) if *id == e => v.push((etat, motif)),
            _ => par_personne.push((e, vec![(etat, motif)])),
        }
    }
    let personnes: Vec<serde_json::Value> = par_personne
        .into_iter()
        .map(|(id, jours)| {
            let compte = regles::compter_motifs(jours.iter().map(|(e, m)| (e.as_str(), m.as_deref())));
            let total: f64 = compte.iter().map(|(_, n)| n).sum::<f64>() + 0.0;
            let motifs: serde_json::Map<String, serde_json::Value> =
                compte.into_iter().map(|(m, n)| (m.to_string(), serde_json::json!(n))).collect();
            serde_json::json!({ "employe_id": id, "motifs": motifs, "total": total })
        })
        .collect();
    Ok(serde_json::json!({ "annee": annee, "personnes": personnes }))
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
            "SELECT e.id, e.nom FROM employe e
             WHERE e.dossier_id = ?1 AND e.statut = 'actif'
               AND (e.date_entree IS NULL OR e.date_entree <= ?2)
               AND NOT EXISTS (SELECT 1 FROM presence p
                               WHERE p.dossier_id = ?1 AND p.employe_id = e.id AND p.jour = ?2)
             ORDER BY e.nom",
            &parametres![dossier.clone(), jour.clone()],
            |r| Ok((r.get::<String>(0)?, r.get::<String>(1)?)),
        )
        .map_err(|e| e.0)?;
    let auteur = crate::argent::id_utilisateur_courant_sur(base);
    let now = crate::utils::maintenant_iso();
    let mut tx = base.transaction().map_err(|e| e.0)?;
    let mut marques = 0;
    // Qui a deja ce jour sur une fiche validee n'est pas marque (point 5) :
    // l'ecran le dit, le reste de l'equipe l'est.
    let mut figes = Vec::new();
    for (id, nom) in &candidats {
        if let Some(numero) = crate::paie::fiche_validee_du_jour_sur(&mut tx, id, &jour)? {
            figes.push(serde_json::json!({ "nom": nom, "numero": numero }));
            continue;
        }
        tx.executer(
            "INSERT INTO presence (id, dossier_id, employe_id, jour, etat, saisi_par, saisi_le)
             VALUES (?1, ?2, ?3, ?4, 'present', ?5, ?6)",
            &parametres![uuid::Uuid::new_v4().to_string(), dossier.clone(), id.clone(), jour.clone(), auteur.clone(), now.clone()],
        )
        .map_err(|e| e.0)?;
        marques += 1;
    }
    tx.valider().map_err(|e| e.0)?;
    Ok(serde_json::json!({ "jour": jour, "marques": marques, "figes": figes }))
}
