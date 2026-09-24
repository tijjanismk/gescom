//! Les fiches du personnel (PLAN-EQUIPE, F-2 — D30).
//!
//! Version `Base` seule (D29) : Gescom Equipe ne parle qu'au serveur.
//! Une fiche appartient au dossier courant (table cloisonnee). Un depart
//! ne supprime rien : la fiche passe « partie », son historique reste.
//! Ce que la personne gagne se masque a la lecture pour qui ne prepare
//! ni ne valide la paie (`coeur::lecture`).

use serde::Deserialize;

use crate::base::Base;
use crate::coeur::personnel::{self as regles, Remuneration};
use crate::parametres;

/// Ce que l'ecran envoie pour creer ou modifier une fiche.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Fiche {
    pub nom: String,
    pub fonction: String,
    #[serde(flatten)]
    pub remuneration: Remuneration,
    #[serde(default)]
    pub telephone: Option<String>,
    #[serde(default)]
    pub date_entree: Option<String>,
    #[serde(default)]
    pub piece_identite: Option<String>,
    #[serde(default)]
    pub contrat_ecrit: bool,
    #[serde(default)]
    pub contrat_date: Option<String>,
    #[serde(default)]
    pub declare: bool,
    #[serde(default)]
    pub numero_inps: Option<String>,
    #[serde(default)]
    pub utilisateur_id: Option<String>,
    #[serde(default)]
    pub depot_id: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
}

fn texte(v: &Option<String>) -> Option<String> {
    v.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string)
}

fn jour(v: &Option<String>, quoi: &str) -> Result<Option<String>, String> {
    match texte(v) {
        None => Ok(None),
        Some(d) => crate::coeur::dates::jour(&d)
            .map(|j| Some(j.format("%Y-%m-%d").to_string()))
            .ok_or_else(|| format!("{quoi} illisible : « {d} »")),
    }
}

/// La fiche nettoyee et jugee, prete a ecrire.
struct Propre {
    nom: String,
    fonction: String,
    r: Remuneration,
    telephone: Option<String>,
    date_entree: Option<String>,
    piece_identite: Option<String>,
    contrat_ecrit: bool,
    contrat_date: Option<String>,
    declare: bool,
    numero_inps: Option<String>,
    utilisateur_id: Option<String>,
    depot_id: Option<String>,
    note: Option<String>,
}

fn juger(base: &mut Base, f: &Fiche, soi: Option<&str>) -> Result<Propre, String> {
    let (nom, fonction) = regles::valider(&f.nom, &f.fonction, &f.remuneration)?;
    let dossier = base.dossier().to_string();
    let utilisateur_id = texte(&f.utilisateur_id);
    if let Some(u) = &utilisateur_id {
        let existe = base
            .lire_une("SELECT nom FROM utilisateur WHERE id = ?1", &parametres![u.clone()], |r| r.get::<String>(0))
            .map_err(|e| e.0)?;
        if existe.is_none() {
            return Err("Ce compte n'existe pas.".to_string());
        }
        // Un compte, une personne active dans le dossier : sinon ses
        // ventes compteraient deux fois dans les commissions.
        let deja: Option<String> = base
            .lire_une(
                "SELECT nom FROM employe
                 WHERE utilisateur_id = ?1 AND dossier_id = ?2 AND statut = 'actif'
                   AND (CAST(?3 AS TEXT) IS NULL OR id <> ?3)",
                &parametres![u.clone(), dossier.clone(), soi.map(str::to_string)],
                |r| r.get::<String>(0),
            )
            .map_err(|e| e.0)?;
        if let Some(autre) = deja {
            return Err(format!("Ce compte est déjà celui de {autre}."));
        }
    }
    let depot_id = texte(&f.depot_id);
    if let Some(d) = &depot_id {
        let existe = base
            .lire_une(
                "SELECT id FROM depot WHERE id = ?1 AND dossier_id = ?2",
                &parametres![d.clone(), dossier],
                |r| r.get::<String>(0),
            )
            .map_err(|e| e.0)?;
        if existe.is_none() {
            return Err("Ce magasin n'est pas dans ce dossier.".to_string());
        }
    }
    Ok(Propre {
        nom,
        fonction,
        r: f.remuneration.clone(),
        telephone: texte(&f.telephone),
        date_entree: jour(&f.date_entree, "Date d'entrée")?,
        piece_identite: texte(&f.piece_identite),
        contrat_ecrit: f.contrat_ecrit,
        contrat_date: if f.contrat_ecrit { jour(&f.contrat_date, "Date du contrat")? } else { None },
        declare: f.declare,
        numero_inps: texte(&f.numero_inps),
        utilisateur_id,
        depot_id,
        note: texte(&f.note),
    })
}

fn noter(
    acces: &mut impl crate::base::Acces,
    type_evenement: &str,
    id: &str,
    ancien: Option<serde_json::Value>,
    nouveau: serde_json::Value,
) -> Result<(), String> {
    let auteur = crate::argent::id_utilisateur_courant_sur(acces);
    let dossier = acces.dossier().to_string();
    acces
        .executer(
            "INSERT INTO journal
               (id, type_evenement, entite_type, entite_id, auteur_id, ancien_valeur, nouveau_valeur, origine, date_evenement, dossier_id)
             VALUES (?1, ?2, 'employe', ?3, ?4, ?5, ?6, 'app', ?7, ?8)",
            &parametres![
                uuid::Uuid::new_v4().to_string(),
                type_evenement,
                id,
                auteur,
                ancien.map(|v| v.to_string()),
                nouveau.to_string(),
                crate::utils::maintenant_iso(),
                dossier
            ],
        )
        .map(|_| ())
        .map_err(|e| e.0)
}

const COLONNES: &str = "e.id, e.nom, e.fonction, e.telephone, e.date_entree, e.piece_identite,
    e.contrat_ecrit, e.contrat_date, e.declare, e.numero_inps, e.salaire_mensuel,
    e.tarif_journalier, e.commission_pct, e.a_la_tache, e.utilisateur_id, u.nom, e.depot_id, d.nom,
    e.statut, e.date_depart, e.motif_depart, e.note, e.cree_le";

fn ligne(r: &crate::base::Ligne<'_>) -> crate::base::Resultat<serde_json::Value> {
    let remu = Remuneration {
        salaire_mensuel: r.get::<Option<i64>>(10)?,
        tarif_journalier: r.get::<Option<i64>>(11)?,
        commission_pct: r.get::<Option<f64>>(12)?,
        a_la_tache: r.get::<i64>(13)? != 0,
    };
    // Comment elle est payee, sans combien : lisible par tous ceux qui
    // voient la fiche. Les montants se masquent (`coeur::lecture`).
    let mut modes = Vec::new();
    if remu.salaire_mensuel.is_some() {
        modes.push("au mois");
    }
    if remu.tarif_journalier.is_some() {
        modes.push("à la journée");
    }
    if remu.commission_pct.is_some() {
        modes.push("à la commission");
    }
    if remu.a_la_tache {
        modes.push("à la tâche");
    }
    Ok(serde_json::json!({
        "id": r.get::<String>(0)?,
        "nom": r.get::<String>(1)?,
        "fonction": r.get::<String>(2)?,
        "telephone": r.get::<Option<String>>(3)?,
        "date_entree": r.get::<Option<String>>(4)?,
        "piece_identite": r.get::<Option<String>>(5)?,
        "contrat_ecrit": r.get::<i64>(6)? != 0,
        "contrat_date": r.get::<Option<String>>(7)?,
        "declare": r.get::<i64>(8)? != 0,
        "numero_inps": r.get::<Option<String>>(9)?,
        "salaire_mensuel": remu.salaire_mensuel,
        "tarif_journalier": remu.tarif_journalier,
        "commission_pct": remu.commission_pct,
        "a_la_tache": remu.a_la_tache,
        "modes": if modes.is_empty() { vec!["rien de fixe"] } else { modes },
        "remuneration_dite": regles::dire(&remu),
        "utilisateur_id": r.get::<Option<String>>(14)?,
        "utilisateur_nom": r.get::<Option<String>>(15)?,
        "depot_id": r.get::<Option<String>>(16)?,
        "depot_nom": r.get::<Option<String>>(17)?,
        "statut": r.get::<String>(18)?,
        "date_depart": r.get::<Option<String>>(19)?,
        "motif_depart": r.get::<Option<String>>(20)?,
        "note": r.get::<Option<String>>(21)?,
        "cree_le": r.get::<String>(22)?,
    }))
}

/// Le personnel du dossier courant, actifs d'abord, par nom.
pub fn lister_sur(base: &mut Base, avec_partis: bool) -> Result<Vec<serde_json::Value>, String> {
    let dossier = base.dossier().to_string();
    base.lire_plusieurs(
        &format!(
            "SELECT {COLONNES} FROM employe e
             LEFT JOIN utilisateur u ON u.id = e.utilisateur_id
             LEFT JOIN depot d ON d.id = e.depot_id AND d.dossier_id = e.dossier_id
             WHERE e.dossier_id = ?1 AND (CAST(?2 AS BIGINT) = 1 OR e.statut = 'actif')
             ORDER BY CASE WHEN e.statut = 'actif' THEN 0 ELSE 1 END, LOWER(e.nom)"
        ),
        &parametres![dossier, avec_partis as i64],
        ligne,
    )
    .map_err(|e| e.0)
}

/// Une fiche du dossier courant.
pub fn lire_sur(base: &mut Base, id: &str) -> Result<serde_json::Value, String> {
    let dossier = base.dossier().to_string();
    base.lire_une(
        &format!(
            "SELECT {COLONNES} FROM employe e
             LEFT JOIN utilisateur u ON u.id = e.utilisateur_id
             LEFT JOIN depot d ON d.id = e.depot_id AND d.dossier_id = e.dossier_id
             WHERE e.id = ?1 AND e.dossier_id = ?2"
        ),
        &parametres![id, dossier],
        ligne,
    )
    .map_err(|e| e.0)?
    .ok_or_else(|| "Fiche introuvable.".to_string())
}

pub fn creer_sur(base: &mut Base, f: Fiche) -> Result<serde_json::Value, String> {
    let p = juger(base, &f, None)?;
    let id = uuid::Uuid::new_v4().to_string();
    let dossier = base.dossier().to_string();
    let now = crate::utils::maintenant_iso();
    let mut tx = base.transaction().map_err(|e| e.0)?;
    tx.executer(
        "INSERT INTO employe
           (id, dossier_id, nom, fonction, telephone, date_entree, piece_identite, contrat_ecrit,
            contrat_date, declare, numero_inps, salaire_mensuel, tarif_journalier, commission_pct,
            a_la_tache, utilisateur_id, depot_id, statut, note, cree_le, modifie_le)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,'actif',?18,?19,?19)",
        &parametres![
            id.clone(),
            dossier,
            p.nom.clone(),
            p.fonction.clone(),
            p.telephone,
            p.date_entree,
            p.piece_identite,
            p.contrat_ecrit as i64,
            p.contrat_date,
            p.declare as i64,
            p.numero_inps,
            p.r.salaire_mensuel,
            p.r.tarif_journalier,
            p.r.commission_pct,
            p.r.a_la_tache as i64,
            p.utilisateur_id,
            p.depot_id,
            p.note,
            now
        ],
    )
    .map_err(|e| e.0)?;
    noter(&mut tx, "employe_cree", &id, None, serde_json::json!({ "nom": p.nom, "fonction": p.fonction }))?;
    tx.valider().map_err(|e| e.0)?;
    lire_sur(base, &id)
}

pub fn modifier_sur(base: &mut Base, id: String, f: Fiche) -> Result<serde_json::Value, String> {
    let avant = lire_sur(base, &id)?;
    let p = juger(base, &f, Some(&id))?;
    let dossier = base.dossier().to_string();
    let now = crate::utils::maintenant_iso();
    let mut tx = base.transaction().map_err(|e| e.0)?;
    tx.executer(
        "UPDATE employe SET nom = ?1, fonction = ?2, telephone = ?3, date_entree = ?4, piece_identite = ?5,
                contrat_ecrit = ?6, contrat_date = ?7, declare = ?8, numero_inps = ?9, salaire_mensuel = ?10,
                tarif_journalier = ?11, commission_pct = ?12, a_la_tache = ?13, utilisateur_id = ?14,
                depot_id = ?15, note = ?16, modifie_le = ?17
         WHERE id = ?18 AND dossier_id = ?19",
        &parametres![
            p.nom.clone(),
            p.fonction.clone(),
            p.telephone,
            p.date_entree,
            p.piece_identite,
            p.contrat_ecrit as i64,
            p.contrat_date,
            p.declare as i64,
            p.numero_inps,
            p.r.salaire_mensuel,
            p.r.tarif_journalier,
            p.r.commission_pct,
            p.r.a_la_tache as i64,
            p.utilisateur_id,
            p.depot_id,
            p.note,
            now,
            id.clone(),
            dossier
        ],
    )
    .map_err(|e| e.0)?;
    noter(
        &mut tx,
        "employe_modifie",
        &id,
        // Pas de montant au journal : l'Historique se lit avec
        // `journal:lire`, pas avec les droits de la paie.
        Some(serde_json::json!({ "nom": avant["nom"], "fonction": avant["fonction"] })),
        serde_json::json!({
            "nom": p.nom, "fonction": p.fonction,
            "remuneration_changee": avant["remuneration_dite"].as_str() != Some(regles::dire(&p.r).as_str()),
        }),
    )?;
    tx.valider().map_err(|e| e.0)?;
    lire_sur(base, &id)
}

/// La personne part : sa fiche reste, avec la date et le motif.
pub fn faire_partir_sur(base: &mut Base, id: String, date: Option<String>, motif: Option<String>) -> Result<serde_json::Value, String> {
    let avant = lire_sur(base, &id)?;
    if avant["statut"] == "partie" {
        return Err(format!("{} est déjà partie.", avant["nom"].as_str().unwrap_or("")));
    }
    let date = jour(&date, "Date de départ")?.unwrap_or_else(|| crate::utils::maintenant_iso()[..10].to_string());
    let motif = texte(&motif);
    let dossier = base.dossier().to_string();
    let mut tx = base.transaction().map_err(|e| e.0)?;
    tx.executer(
        "UPDATE employe SET statut = 'partie', date_depart = ?1, motif_depart = ?2, modifie_le = ?3
         WHERE id = ?4 AND dossier_id = ?5",
        &parametres![date.clone(), motif.clone(), crate::utils::maintenant_iso(), id.clone(), dossier],
    )
    .map_err(|e| e.0)?;
    noter(&mut tx, "employe_parti", &id, None, serde_json::json!({ "nom": avant["nom"], "le": date, "motif": motif }))?;
    tx.valider().map_err(|e| e.0)?;
    lire_sur(base, &id)
}

/// La personne revient : la meme fiche, active de nouveau.
pub fn faire_revenir_sur(base: &mut Base, id: String) -> Result<serde_json::Value, String> {
    let avant = lire_sur(base, &id)?;
    if avant["statut"] == "actif" {
        return Err(format!("{} est déjà là.", avant["nom"].as_str().unwrap_or("")));
    }
    // Son compte a pu etre donne a quelqu'un d'autre entre-temps.
    if let Some(u) = avant["utilisateur_id"].as_str() {
        let dossier = base.dossier().to_string();
        let autre: Option<String> = base
            .lire_une(
                "SELECT nom FROM employe WHERE utilisateur_id = ?1 AND dossier_id = ?2 AND statut = 'actif' AND id <> ?3",
                &parametres![u, dossier, id.clone()],
                |r| r.get::<String>(0),
            )
            .map_err(|e| e.0)?;
        if let Some(a) = autre {
            return Err(format!("Son compte est maintenant celui de {a} : le retirer de sa fiche d'abord."));
        }
    }
    let dossier = base.dossier().to_string();
    let mut tx = base.transaction().map_err(|e| e.0)?;
    tx.executer(
        "UPDATE employe SET statut = 'actif', date_depart = NULL, motif_depart = NULL, modifie_le = ?1
         WHERE id = ?2 AND dossier_id = ?3",
        &parametres![crate::utils::maintenant_iso(), id.clone(), dossier],
    )
    .map_err(|e| e.0)?;
    noter(&mut tx, "employe_revenu", &id, None, serde_json::json!({ "nom": avant["nom"] }))?;
    tx.valider().map_err(|e| e.0)?;
    lire_sur(base, &id)
}
