//! L'etat de paie du mois en base (PLAN-EQUIPE, EQ-2 — D48). Version
//! `Base` seule (D29). Lecture seule : on paie et on corrige dans les
//! fiches du mois.
//!
//! - **Une ligne par personne** : ses fiches VALIDEES dont la periode
//!   finit dans le mois (la regle de `paie::salaires_par_mois_sur`),
//!   ventilees par genre (`coeur::etat_paie::ventiler`). Une fiche
//!   remplacee ne compte pas : sa rectificative la remplace, et ses
//!   versements l'ont suivie.
//! - **Hors des totaux, signales** : les brouillons du mois (« a valider »)
//!   et les personnes employees au moins un jour du mois sans aucune
//!   fiche (la liste « a preparer » de `paie::lister_sur`) — la personne
//!   oubliee a la paie est le trou le plus probable.
//!
//! Qui lit : `paie:preparer` ou `paie:valider`, sinon refus
//! (`coeur::lecture`) ; le CSV de meme.

use std::collections::HashMap;

use crate::base::Base;
use crate::coeur::etat_paie::{self as regles, LigneEtat, Montants};
use crate::coeur::paie::reste_a_verser;
use crate::parametres;

struct FicheDuMois {
    id: String,
    employe_id: String,
    nom: String,
    fonction: String,
    numero: String,
    net: i64,
    verse: i64,
    repris: i64,
}

/// Les lignes du mois `AAAA-MM` et leur total.
fn lignes_du_mois(base: &mut Base, mois: &str) -> Result<(Vec<LigneEtat>, Montants), String> {
    regles::bornes_du_mois(mois)?;
    let dossier = base.dossier().to_string();
    let fiches = base
        .lire_plusieurs(
            "SELECT f.id, f.employe_id, f.nom, f.fonction, f.numero, f.net,
                    (SELECT CAST(COALESCE(SUM(v.montant), 0) AS BIGINT) FROM versement_paie v
                      WHERE v.fiche_id = f.id AND v.dossier_id = f.dossier_id),
                    (SELECT CAST(COALESCE(SUM(a.montant), 0) AS BIGINT) FROM avance a
                      WHERE a.trop_verse_fiche_id = f.id AND a.dossier_id = f.dossier_id AND a.statut = 'ouverte')
             FROM fiche_paie f
             WHERE f.dossier_id = ?1 AND f.statut = 'validee' AND SUBSTR(f.au, 1, 7) = ?2
             ORDER BY f.nom, f.employe_id, f.du",
            &parametres![dossier.clone(), mois],
            |r| {
                Ok(FicheDuMois {
                    id: r.get(0)?,
                    employe_id: r.get(1)?,
                    nom: r.get(2)?,
                    fonction: r.get(3)?,
                    numero: r.get::<Option<String>>(4)?.unwrap_or_default(),
                    net: r.get(5)?,
                    verse: r.get(6)?,
                    repris: r.get(7)?,
                })
            },
        )
        .map_err(|e| e.0)?;
    let mut par_fiche: HashMap<String, Vec<(String, i64)>> = HashMap::new();
    // D47 : la ligne d'un pret est du genre `avance` sur la fiche ; sa
    // source dit sa nature, et l'etat la range dans la colonne des prets.
    for (fiche, genre, montant, nature) in base
        .lire_plusieurs(
            "SELECT l.fiche_id, l.genre, l.montant, a.nature FROM ligne_paie l
             JOIN fiche_paie f ON f.id = l.fiche_id AND f.dossier_id = l.dossier_id
             LEFT JOIN avance a ON a.id = l.source AND a.dossier_id = l.dossier_id
             WHERE l.dossier_id = ?1 AND f.statut = 'validee' AND SUBSTR(f.au, 1, 7) = ?2",
            &parametres![dossier, mois],
            |r| Ok((r.get::<String>(0)?, r.get::<String>(1)?, r.get::<i64>(2)?, r.get::<Option<String>>(3)?)),
        )
        .map_err(|e| e.0)?
    {
        let genre = if genre == crate::coeur::paie::AVANCE && nature.as_deref() == Some(crate::coeur::pret::NATURE) {
            regles::PRET.to_string()
        } else {
            genre
        };
        par_fiche.entry(fiche).or_default().push((genre, montant));
    }

    // Une ligne par personne, dans l'ordre des noms.
    let mut lignes: Vec<LigneEtat> = Vec::new();
    let mut total = Montants::default();
    for f in fiches {
        let reste = reste_a_verser(f.net, f.verse, f.repris);
        let m = regles::ventiler(&f.numero, f.net, par_fiche.get(&f.id).map(Vec::as_slice).unwrap_or(&[]), f.verse, reste)?;
        total.ajouter(&m)?;
        match lignes.iter_mut().find(|l| l.employe_id == f.employe_id) {
            Some(l) => {
                l.montants.ajouter(&m)?;
                l.numeros.push(f.numero);
            }
            None => lignes.push(LigneEtat { employe_id: f.employe_id, nom: f.nom, fonction: f.fonction, numeros: vec![f.numero], montants: m }),
        }
    }
    Ok((lignes, total))
}

/// L'etat du mois `AAAA-MM` du dossier courant.
pub fn lire_sur(base: &mut Base, mois: String) -> Result<serde_json::Value, String> {
    let (du, au) = regles::bornes_du_mois(&mois)?;
    let (lignes, total) = lignes_du_mois(base, &mois)?;
    let dossier = base.dossier().to_string();

    let brouillons = base
        .lire_plusieurs(
            "SELECT id, employe_id, nom, fonction, du, au, net FROM fiche_paie
             WHERE dossier_id = ?1 AND statut = 'brouillon' AND SUBSTR(au, 1, 7) = ?2
             ORDER BY nom, du",
            &parametres![dossier, mois.clone()],
            |r| {
                Ok(serde_json::json!({
                    "id": r.get::<String>(0)?,
                    "employe_id": r.get::<String>(1)?,
                    "nom": r.get::<String>(2)?,
                    "fonction": r.get::<String>(3)?,
                    "du": r.get::<String>(4)?,
                    "au": r.get::<String>(5)?,
                    "net": r.get::<i64>(6)?,
                }))
            },
        )
        .map_err(|e| e.0)?;
    let sans_fiche = crate::paie::lister_sur(base, du.clone(), au.clone())?["a_preparer"].take();

    let auteur = crate::argent::id_utilisateur_courant_sur(base);
    let etabli_par = base
        .lire_une("SELECT nom FROM utilisateur WHERE id = ?1", &parametres![auteur], |r| r.get::<String>(0))
        .map_err(|e| e.0)?;
    let cout = total.cout()?;
    Ok(serde_json::json!({
        "mois": mois, "du": du, "au": au,
        "lignes": lignes,
        "total": total,
        // Le cout pour la boutique : le chiffre de Rapports -> CA mensuel.
        "cout": cout,
        "brouillons": brouillons,
        "sans_fiche": sans_fiche,
        "etabli_le": crate::utils::maintenant_iso(),
        "etabli_par": etabli_par,
        "societe": crate::creances::societe_sur(base),
    }))
}

/// L'export CSV de l'etat du mois, et un nom de fichier qui le dit.
pub fn csv_sur(base: &mut Base, mois: String) -> Result<serde_json::Value, String> {
    let (lignes, total) = lignes_du_mois(base, &mois)?;
    Ok(serde_json::json!({
        "nom_fichier": format!("etat_paie_{mois}.csv"),
        "contenu": regles::csv(&lignes, &total),
        "lignes": lignes.len(),
    }))
}
