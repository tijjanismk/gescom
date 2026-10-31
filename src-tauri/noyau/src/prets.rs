//! Le pret au personnel sur plusieurs mois en base (PLAN-EQUIPE EQ-1 —
//! D47). Version `Base` seule (D29).
//!
//! Un pret est une avance d'une autre nature (`avance.nature = 'pret'`) :
//! la fiche de paie le retient comme une avance (ligne du genre `avance`,
//! source = le pret), la validation, la rectificative et le journal PA
//! marchent sans changer. Ce module porte ce qui lui est propre :
//! l'echeance par mois civil et le premier mois (`coeur::pret`), un seul
//! pret en cours, hors du plafond d'avance, et les reglements hors paie
//! (`pret_reglement`) — un remboursement direct, ou la remise du reste.
//! Ceux-ci montent dans `avance.retenu` : ce qui n'est plus du.
//!
//! Soldé se calcule (`montant - retenu = 0`), ne s'ecrit pas : une
//! rectificative qui rend ce qu'elle avait retenu rouvre le pret.

use crate::avances::{en_cours_de_nature_sur, noter_entite, MOTIF_CAISSE};
use crate::base::Base;
use crate::coeur::pret as regles;
use crate::parametres;

struct Personne {
    nom: String,
    statut: String,
    entree: Option<String>,
}

fn personne(base: &mut Base, employe_id: &str) -> Result<Personne, String> {
    let dossier = base.dossier().to_string();
    base.lire_une(
        "SELECT nom, statut, date_entree FROM employe WHERE id = ?1 AND dossier_id = ?2",
        &parametres![employe_id, dossier],
        |r| Ok(Personne { nom: r.get(0)?, statut: r.get(1)?, entree: r.get(2)? }),
    )
    .map_err(|e| e.0)?
    .ok_or_else(|| "Fiche introuvable.".to_string())
}

/// Ce que la personne doit encore en prets.
pub fn en_cours_sur(base: &mut Base, employe_id: &str) -> Result<i64, String> {
    en_cours_de_nature_sur(base, employe_id, regles::NATURE)
}

/// Ce qu'on dit d'un pret en le donnant, outre la personne et le montant.
#[derive(Debug, Clone, Default)]
pub struct Saisie {
    pub moyen: Option<String>,
    pub motif: Option<String>,
    /// Le jour du pret (`AAAA-MM-JJ`) ; vide : aujourd'hui.
    pub date: Option<String>,
    /// Ce qu'un mois en retient : obligatoire.
    pub mensualite: Option<i64>,
    /// Le premier mois retenu (`AAAA-MM`) ; vide : le mois suivant.
    pub premier_mois: Option<String>,
    /// L'argent sort du tiroir : la sortie de caisse s'ecrit avec lui.
    pub du_tiroir: bool,
}

/// Prete a une personne (D47). Pris dans le tiroir, la sortie de caisse
/// s'ecrit dans la meme transaction (caisse ouverte exigee, D42).
pub fn preter_sur(base: &mut Base, employe_id: String, montant: i64, s: Saisie) -> Result<serde_json::Value, String> {
    let p = personne(base, &employe_id)?;
    if p.statut != "actif" {
        return Err(format!("{} est partie : pas de prêt.", p.nom));
    }
    let moyen = s.moyen.map(|m| m.trim().to_string()).filter(|m| !m.is_empty()).unwrap_or_else(|| "especes".into());
    crate::coeur::saisie::verifier_mode_encaissement(&moyen)?;
    crate::coeur::avances::verifier_tiroir(&moyen, s.du_tiroir)?;
    let deja = en_cours_sur(base, &employe_id)? > 0;
    let mensualite = regles::verifier(montant, s.mensualite, deja, &p.nom)?;
    let jour = crate::coeur::avances::verifier_date(s.date.as_deref(), chrono::Local::now().date_naive(), p.entree.as_deref(), s.du_tiroir)?;
    crate::dossiers::verifier_date_sur(base, &jour)?;
    let retenir_des = regles::premier_mois(&jour, s.premier_mois.as_deref())?;
    let auteur = crate::argent::id_utilisateur_courant_sur(base);
    let dossier = base.dossier().to_string();
    let now = crate::utils::maintenant_iso();
    let date_pret = if now.starts_with(&jour) { now.clone() } else { jour.clone() };
    let motif = s.motif.map(|m| m.trim().to_string()).filter(|m| !m.is_empty());
    let id = uuid::Uuid::new_v4().to_string();
    let mut tx = base.transaction().map_err(|e| e.0)?;
    // La caisse d'abord : fermee, rien ne s'ecrit (regle 4).
    let mouvement = if s.du_tiroir {
        Some(crate::caisses::mouvement_especes_sur(
            &mut tx,
            "sortie",
            montant,
            MOTIF_CAISSE,
            &format!("Prêt au personnel — {}", p.nom),
            &id,
            &auteur,
            &now,
        )?)
    } else {
        None
    };
    tx.executer(
        "INSERT INTO avance
           (id, dossier_id, employe_id, montant, retenu, moyen, motif, date_avance, statut,
            mouvement_caisse_id, cree_par, cree_le, mensualite, nature, retenir_des)
         VALUES (?1, ?2, ?3, ?4, 0, ?5, ?6, ?7, 'ouverte', ?8, ?9, ?10, ?11, ?12, ?13)",
        &parametres![
            id.clone(),
            dossier,
            employe_id.clone(),
            montant,
            moyen.clone(),
            motif.clone(),
            date_pret,
            mouvement,
            auteur.clone(),
            now,
            mensualite,
            regles::NATURE,
            retenir_des.clone()
        ],
    )
    .map_err(|e| e.0)?;
    noter_entite(
        &mut tx,
        "pret_accorde",
        regles::NATURE,
        &id,
        &auteur,
        serde_json::json!({
            "nom": p.nom, "montant": montant, "mensualite": mensualite, "moyen": moyen, "date_pret": jour,
            "retenir_des": retenir_des, "motif": motif, "du_tiroir": s.du_tiroir,
        }),
    )?;
    tx.valider().map_err(|e| e.0)?;
    lire_sur(base, &id)
}

/// Un remboursement direct, hors paie : une partie ou tout (D47).
pub fn rembourser_sur(base: &mut Base, pret_id: String, montant: i64, moyen: Option<String>, motif: Option<String>) -> Result<serde_json::Value, String> {
    regler(base, pret_id, regles::REMBOURSEMENT, Some(montant), moyen, motif)
}

/// La remise du reste : la societe y renonce (une charge, D47).
pub fn remettre_sur(base: &mut Base, pret_id: String, motif: Option<String>) -> Result<serde_json::Value, String> {
    regler(base, pret_id, regles::REMISE, None, None, motif)
}

fn regler(
    base: &mut Base,
    pret_id: String,
    genre: &str,
    montant: Option<i64>,
    moyen: Option<String>,
    motif: Option<String>,
) -> Result<serde_json::Value, String> {
    let dossier = base.dossier().to_string();
    let (employe_id, du, retenu, statut, nature): (String, i64, i64, String, String) = base
        .lire_une(
            "SELECT employe_id, montant, retenu, statut, nature FROM avance WHERE id = ?1 AND dossier_id = ?2",
            &parametres![pret_id.clone(), dossier.clone()],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .map_err(|e| e.0)?
        .ok_or_else(|| "Prêt introuvable.".to_string())?;
    if nature != regles::NATURE {
        return Err("Ce n'est pas un prêt.".to_string());
    }
    if statut != "ouverte" {
        return Err("Ce prêt est annulé.".to_string());
    }
    let m = regles::verifier_reglement(genre, montant, du - retenu)?;
    let moyen = if genre == regles::REMBOURSEMENT {
        let m = moyen.map(|m| m.trim().to_string()).filter(|m| !m.is_empty()).unwrap_or_else(|| "especes".into());
        crate::coeur::saisie::verifier_mode_encaissement(&m)?;
        Some(m)
    } else {
        None
    };
    let now = crate::utils::maintenant_iso();
    crate::dossiers::verifier_date_sur(base, &now[..10])?;
    let p = personne(base, &employe_id)?;
    let auteur = crate::argent::id_utilisateur_courant_sur(base);
    let motif = motif.map(|m| m.trim().to_string()).filter(|m| !m.is_empty());
    let id = uuid::Uuid::new_v4().to_string();
    let mut tx = base.transaction().map_err(|e| e.0)?;
    // Une fiche validee en meme temps a pu retenir : on ne descend pas
    // sous zero.
    let n = tx
        .executer(
            "UPDATE avance SET retenu = retenu + ?1
             WHERE id = ?2 AND dossier_id = ?3 AND statut = 'ouverte' AND retenu + ?1 <= montant",
            &parametres![m, pret_id.clone(), dossier.clone()],
        )
        .map_err(|e| e.0)?;
    if n != 1 {
        return Err("Ce prêt vient de changer (une fiche de paie l'a retenu) : relire avant de régler.".to_string());
    }
    tx.executer(
        "INSERT INTO pret_reglement (id, dossier_id, pret_id, genre, montant, moyen, motif, date_reglement, cree_par, cree_le)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?8)",
        &parametres![id, dossier, pret_id.clone(), genre, m, moyen.clone(), motif.clone(), now, auteur.clone()],
    )
    .map_err(|e| e.0)?;
    noter_entite(
        &mut tx,
        if genre == regles::REMISE { "pret_remis" } else { "pret_rembourse" },
        regles::NATURE,
        &pret_id,
        &auteur,
        serde_json::json!({ "nom": p.nom, "montant": m, "moyen": moyen, "motif": motif, "reste": du - retenu - m }),
    )?;
    tx.valider().map_err(|e| e.0)?;
    lire_sur(base, &pret_id)
}

/// Annule un pret donne par erreur, si rien n'en a ete retenu ni
/// rembourse. Pris dans le tiroir, l'argent y revient (caisse ouverte).
pub fn annuler_sur(base: &mut Base, pret_id: String, motif: Option<String>) -> Result<serde_json::Value, String> {
    crate::avances::annuler_de_nature_sur(base, pret_id, motif, regles::NATURE)
}

const COLONNES: &str = "a.id, a.employe_id, e.nom, e.statut, a.montant, a.retenu, a.mensualite, a.retenir_des,
    a.date_avance, a.statut, a.moyen, a.motif, a.mouvement_caisse_id,
    CAST(COALESCE((SELECT SUM(-lp.montant) FROM ligne_paie lp
         JOIN fiche_paie f ON f.id = lp.fiche_id AND f.dossier_id = lp.dossier_id
         WHERE lp.dossier_id = a.dossier_id AND lp.genre = 'avance' AND lp.source = a.id AND f.statut = 'validee'), 0) AS BIGINT),
    (SELECT MAX(SUBSTR(f.du, 1, 7)) FROM ligne_paie lp
         JOIN fiche_paie f ON f.id = lp.fiche_id AND f.dossier_id = lp.dossier_id
         WHERE lp.dossier_id = a.dossier_id AND lp.genre = 'avance' AND lp.source = a.id AND f.statut = 'validee'),
    CAST(COALESCE((SELECT SUM(r.montant) FROM pret_reglement r
         WHERE r.dossier_id = a.dossier_id AND r.pret_id = a.id AND r.genre = 'remboursement'), 0) AS BIGINT),
    CAST(COALESCE((SELECT SUM(r.montant) FROM pret_reglement r
         WHERE r.dossier_id = a.dossier_id AND r.pret_id = a.id AND r.genre = 'remise'), 0) AS BIGINT)";

fn pret_json(r: &crate::base::Ligne<'_>, mois_courant: &str) -> crate::base::Resultat<serde_json::Value> {
    let montant: i64 = r.get(4)?;
    let retenu: i64 = r.get(5)?;
    let mensualite = r.get::<Option<i64>>(6)?.unwrap_or(0);
    let retenir_des = r.get::<Option<String>>(7)?.unwrap_or_default();
    let statut: String = r.get(9)?;
    let dernier: Option<String> = r.get(14)?;
    let reste = (montant - retenu).max(0);
    let etat = regles::etat(&statut, montant, retenu);
    Ok(serde_json::json!({
        "id": r.get::<String>(0)?,
        "employe_id": r.get::<String>(1)?,
        "nom": r.get::<String>(2)?,
        "partie": r.get::<String>(3)? == "partie",
        "montant": montant,
        "mensualite": mensualite,
        "retenir_des": retenir_des,
        "date_pret": r.get::<String>(8)?,
        "etat": etat,
        "moyen": r.get::<String>(10)?,
        "motif": r.get::<Option<String>>(11)?,
        "du_tiroir": r.get::<Option<String>>(12)?.is_some(),
        "retenu_paie": r.get::<i64>(13)?,
        "dernier_mois_retenu": dernier,
        "rembourse": r.get::<i64>(15)?,
        "remis": r.get::<i64>(16)?,
        "reste": reste,
        "fin_prevue": if etat == "en_cours" {
            regles::fin_prevue(reste, mensualite, &retenir_des, dernier.as_deref(), mois_courant)
        } else {
            None
        },
    }))
}

fn mois_courant() -> String {
    chrono::Local::now().format("%Y-%m").to_string()
}

/// Les prets du dossier (d'une personne, ou de tous), les plus recents
/// d'abord. `en_cours_seulement` : ce qui reste du.
pub fn lister_sur(base: &mut Base, employe_id: Option<String>, en_cours_seulement: bool) -> Result<Vec<serde_json::Value>, String> {
    let dossier = base.dossier().to_string();
    let mois = mois_courant();
    base.lire_plusieurs(
        &format!(
            "SELECT {COLONNES}
             FROM avance a JOIN employe e ON e.id = a.employe_id AND e.dossier_id = a.dossier_id
             WHERE a.dossier_id = ?1 AND a.nature = ?4
               AND (CAST(?2 AS TEXT) IS NULL OR a.employe_id = ?2)
               AND (CAST(?3 AS BIGINT) = 0 OR (a.statut = 'ouverte' AND a.montant > a.retenu))
             ORDER BY a.date_avance DESC"
        ),
        &parametres![dossier, employe_id, en_cours_seulement as i64, regles::NATURE],
        |r| pret_json(r, &mois),
    )
    .map_err(|e| e.0)
}

/// Un pret et son histoire : les retenues des fiches validees, les
/// remboursements et la remise, dans l'ordre.
pub fn lire_sur(base: &mut Base, pret_id: &str) -> Result<serde_json::Value, String> {
    let dossier = base.dossier().to_string();
    let mois = mois_courant();
    let mut v = base
        .lire_une(
            &format!(
                "SELECT {COLONNES}
                 FROM avance a JOIN employe e ON e.id = a.employe_id AND e.dossier_id = a.dossier_id
                 WHERE a.id = ?1 AND a.dossier_id = ?2 AND a.nature = ?3"
            ),
            &parametres![pret_id, dossier.clone(), regles::NATURE],
            |r| pret_json(r, &mois),
        )
        .map_err(|e| e.0)?
        .ok_or_else(|| "Prêt introuvable.".to_string())?;
    let mut mouvements = base
        .lire_plusieurs(
            "SELECT COALESCE(f.valide_le, f.au), f.numero, f.du, -lp.montant FROM ligne_paie lp
             JOIN fiche_paie f ON f.id = lp.fiche_id AND f.dossier_id = lp.dossier_id
             WHERE lp.dossier_id = ?1 AND lp.genre = 'avance' AND lp.source = ?2 AND f.statut = 'validee'",
            &parametres![dossier.clone(), pret_id],
            |r| {
                let du: String = r.get(2)?;
                Ok(serde_json::json!({
                    "date": r.get::<String>(0)?,
                    "genre": "paie",
                    "numero": r.get::<Option<String>>(1)?,
                    "mois": du.get(..7).unwrap_or(&du),
                    "montant": r.get::<i64>(3)?,
                }))
            },
        )
        .map_err(|e| e.0)?;
    mouvements.extend(
        base.lire_plusieurs(
            "SELECT date_reglement, genre, montant, moyen, motif FROM pret_reglement
             WHERE dossier_id = ?1 AND pret_id = ?2",
            &parametres![dossier, pret_id],
            |r| {
                Ok(serde_json::json!({
                    "date": r.get::<String>(0)?,
                    "genre": r.get::<String>(1)?,
                    "montant": r.get::<i64>(2)?,
                    "moyen": r.get::<Option<String>>(3)?,
                    "motif": r.get::<Option<String>>(4)?,
                }))
            },
        )
        .map_err(|e| e.0)?,
    );
    mouvements.sort_by(|a, b| a["date"].as_str().cmp(&b["date"].as_str()));
    v["mouvements"] = mouvements.into();
    Ok(v)
}
