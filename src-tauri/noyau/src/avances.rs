//! Les avances sur salaire en base (PLAN-EQUIPE, G-1 — D32). Version
//! `Base` seule (D29).
//!
//! Une avance est rattachee a la personne. Elle est **independante de
//! la caisse** (decision du proprietaire, 24/09) : l'argent de la paie
//! ne passe pas par le tiroir du jour, le moyen dit d'ou il vient. Ce
//! qui n'est pas retenu est « en cours » ; la fiche de paie le retient
//! (G-2, colonne `retenu`). Annuler une avance (une erreur de saisie) se
//! fait seulement si rien n'en a ete retenu.

use crate::base::Base;
use crate::coeur::avances as regles;
use crate::parametres;

struct Personne {
    nom: String,
    statut: String,
    plafond: Option<i64>,
}

fn personne(base: &mut Base, employe_id: &str) -> Result<Personne, String> {
    let dossier = base.dossier().to_string();
    base.lire_une(
        "SELECT nom, statut, avance_max FROM employe WHERE id = ?1 AND dossier_id = ?2",
        &parametres![employe_id, dossier],
        |r| Ok(Personne { nom: r.get(0)?, statut: r.get(1)?, plafond: r.get(2)? }),
    )
    .map_err(|e| e.0)?
    .ok_or_else(|| "Fiche introuvable.".to_string())
}

/// Ce que la personne doit encore en avances non retenues.
pub fn en_cours_sur(base: &mut Base, employe_id: &str) -> Result<i64, String> {
    let dossier = base.dossier().to_string();
    base.lire_une(
        "SELECT CAST(COALESCE(SUM(montant - retenu), 0) AS BIGINT) FROM avance
         WHERE employe_id = ?1 AND dossier_id = ?2 AND statut = 'ouverte'",
        &parametres![employe_id, dossier],
        |r| r.get::<i64>(0),
    )
    .map_err(|e| e.0)
    .map(|v| v.unwrap_or(0))
}

fn noter(
    acces: &mut impl crate::base::Acces,
    type_evenement: &str,
    avance_id: &str,
    auteur: &str,
    valeur: serde_json::Value,
) -> Result<(), String> {
    let dossier = acces.dossier().to_string();
    acces
        .executer(
            "INSERT INTO journal
               (id, type_evenement, entite_type, entite_id, auteur_id, nouveau_valeur, origine, date_evenement, dossier_id)
             VALUES (?1, ?2, 'avance', ?3, ?4, ?5, 'app', ?6, ?7)",
            &parametres![
                uuid::Uuid::new_v4().to_string(),
                type_evenement,
                avance_id,
                auteur,
                valeur.to_string(),
                crate::utils::maintenant_iso(),
                dossier
            ],
        )
        .map(|_| ())
        .map_err(|e| e.0)
}

/// Donne une avance, rattachee a la personne, hors caisse.
pub fn donner_sur(
    base: &mut Base,
    employe_id: String,
    montant: i64,
    moyen: Option<String>,
    motif: Option<String>,
) -> Result<serde_json::Value, String> {
    let p = personne(base, &employe_id)?;
    if p.statut != "actif" {
        return Err(format!("{} est partie : pas d'avance.", p.nom));
    }
    let moyen = moyen.map(|m| m.trim().to_string()).filter(|m| !m.is_empty()).unwrap_or_else(|| "especes".into());
    crate::coeur::saisie::verifier_mode_encaissement(&moyen)?;
    let en_cours = en_cours_sur(base, &employe_id)?;
    regles::verifier(montant, en_cours, p.plafond, &p.nom)?;
    let auteur = crate::argent::id_utilisateur_courant_sur(base);
    let dossier = base.dossier().to_string();
    let now = crate::utils::maintenant_iso();
    let motif = motif.map(|m| m.trim().to_string()).filter(|m| !m.is_empty());
    let id = uuid::Uuid::new_v4().to_string();
    let mut tx = base.transaction().map_err(|e| e.0)?;
    tx.executer(
        "INSERT INTO avance
           (id, dossier_id, employe_id, montant, retenu, moyen, motif, date_avance, statut,
            mouvement_caisse_id, cree_par, cree_le)
         VALUES (?1, ?2, ?3, ?4, 0, ?5, ?6, ?7, 'ouverte', NULL, ?8, ?7)",
        &parametres![id.clone(), dossier, employe_id.clone(), montant, moyen.clone(), motif.clone(), now.clone(), auteur.clone()],
    )
    .map_err(|e| e.0)?;
    noter(&mut tx, "avance_donnee", &id, &auteur, serde_json::json!({ "nom": p.nom, "montant": montant, "moyen": moyen }))?;
    tx.valider().map_err(|e| e.0)?;
    Ok(serde_json::json!({
        "id": id, "employe_id": employe_id, "nom": p.nom, "montant": montant,
        "en_cours": en_cours + montant, "date_avance": now,
    }))
}

/// Annule une avance donnee par erreur : elle ne se retiendra pas.
/// Refuse si une fiche de paie en a deja retenu une partie.
pub fn annuler_sur(base: &mut Base, avance_id: String, motif: Option<String>) -> Result<serde_json::Value, String> {
    let dossier = base.dossier().to_string();
    let (employe_id, montant, retenu, statut): (String, i64, i64, String) = base
        .lire_une(
            "SELECT employe_id, montant, retenu, statut FROM avance WHERE id = ?1 AND dossier_id = ?2",
            &parametres![avance_id.clone(), dossier.clone()],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .map_err(|e| e.0)?
        .ok_or_else(|| "Avance introuvable.".to_string())?;
    if statut != "ouverte" {
        return Err("Cette avance n'est plus en cours.".to_string());
    }
    if retenu > 0 {
        return Err("Une fiche de paie en a déjà retenu une partie : elle ne s'annule plus.".to_string());
    }
    let p = personne(base, &employe_id)?;
    let auteur = crate::argent::id_utilisateur_courant_sur(base);
    let now = crate::utils::maintenant_iso();
    let motif = motif.map(|m| m.trim().to_string()).filter(|m| !m.is_empty());
    let mut tx = base.transaction().map_err(|e| e.0)?;
    tx.executer(
        "UPDATE avance SET statut = 'annulee', annule_le = ?1 WHERE id = ?2 AND dossier_id = ?3",
        &parametres![now, avance_id.clone(), dossier],
    )
    .map_err(|e| e.0)?;
    noter(&mut tx, "avance_annulee", &avance_id, &auteur, serde_json::json!({ "nom": p.nom, "montant": montant, "motif": motif }))?;
    tx.valider().map_err(|e| e.0)?;
    Ok(serde_json::json!({ "id": avance_id, "nom": p.nom, "montant": montant }))
}

/// Les avances du dossier courant (d'une personne, ou de toutes), les
/// plus recentes d'abord. `en_cours_seulement` : ce qui reste a retenir.
pub fn lister_sur(base: &mut Base, employe_id: Option<String>, en_cours_seulement: bool) -> Result<Vec<serde_json::Value>, String> {
    let dossier = base.dossier().to_string();
    base.lire_plusieurs(
        "SELECT a.id, a.employe_id, e.nom, a.montant, a.retenu, a.moyen, a.motif, a.date_avance, a.statut
         FROM avance a JOIN employe e ON e.id = a.employe_id AND e.dossier_id = a.dossier_id
         WHERE a.dossier_id = ?1
           AND (CAST(?2 AS TEXT) IS NULL OR a.employe_id = ?2)
           AND (CAST(?3 AS BIGINT) = 0 OR (a.statut = 'ouverte' AND a.montant > a.retenu))
         ORDER BY a.date_avance DESC",
        &parametres![dossier, employe_id, en_cours_seulement as i64],
        |r| {
            let montant: i64 = r.get(3)?;
            let retenu: i64 = r.get(4)?;
            Ok(serde_json::json!({
                "id": r.get::<String>(0)?,
                "employe_id": r.get::<String>(1)?,
                "nom": r.get::<String>(2)?,
                "montant": montant,
                "retenu": retenu,
                "reste": regles::reste(montant, retenu),
                "moyen": r.get::<String>(5)?,
                "motif": r.get::<Option<String>>(6)?,
                "date_avance": r.get::<String>(7)?,
                "statut": r.get::<String>(8)?,
            }))
        },
    )
    .map_err(|e| e.0)
}
