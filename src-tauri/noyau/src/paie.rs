//! La fiche de paie en base (PLAN-EQUIPE, G-2 — D31). Version `Base`
//! seule (D29).
//!
//! Contrairement aux journaux (D23), la fiche se STOCKE : c'est un
//! papier qu'on remet, et ce que la personne a touche ne doit pas
//! changer si l'on corrige un tarif le mois suivant — comme une vente.
//!
//! - **Brouillon** : se recalcule a volonte ; les lignes calculees
//!   (salaire, jours, commission, avances) se refont, les lignes saisies
//!   (tache, prime, retenue) restent.
//! - **Validee** : figee et numerotee (`PAIE-2026-00001`, par dossier),
//!   les avances retenues montent dans `avance.retenu`. On ne valide
//!   que ce qui est a jour : si les jours, les ventes ou les avances ont
//!   bouge depuis le calcul, la validation le dit au lieu de figer un
//!   chiffre perime. Ensuite, les jours de sa periode ne se marquent
//!   plus (revue du 28/10, point 5) — sauf pendant une rectificative.
//! - **Rectificative** : une erreur sur une fiche validee se corrige par
//!   une nouvelle fiche qui la remplace, jamais en reecrivant. Elle
//!   reprend les **conditions figees** de l'ancienne (salaire, tarif,
//!   taux, declaration, cotisations — point 4) et relit les faits de la
//!   periode (jours, ventes, avances, dates d'emploi). A sa validation,
//!   les avances que l'ancienne avait retenues sont rendues puis
//!   retenues de nouveau selon la nouvelle ; les versements la suivent ;
//!   ce qui a ete verse en trop devient une avance retenue sur la fiche
//!   suivante (point 6, D32).
//!
//! Une fiche ne paie que les jours d'emploi de la personne (entree,
//! depart — point 1) ; une personne au mois se paie sur un mois civil
//! entier (point 8).

use crate::base::{Acces, Base};
use crate::coeur::paie::{self as regles, AvanceARetenir, Conditions, Ligne, Periode};
use crate::coeur::personnel::Remuneration;
pub use crate::coeur::paie::Cotisation;
use crate::parametres;

struct Personne {
    nom: String,
    fonction: String,
    r: Remuneration,
    utilisateur_id: Option<String>,
    declare: bool,
    entree: Option<String>,
    depart: Option<String>,
}

fn personne(base: &mut impl Acces, employe_id: &str) -> Result<Personne, String> {
    let dossier = base.dossier().to_string();
    base.lire_une(
        "SELECT nom, fonction, salaire_mensuel, tarif_journalier, commission_pct, a_la_tache, utilisateur_id, declare,
                date_entree, date_depart
         FROM employe WHERE id = ?1 AND dossier_id = ?2",
        &parametres![employe_id, dossier],
        |r| {
            Ok(Personne {
                nom: r.get(0)?,
                fonction: r.get(1)?,
                r: Remuneration {
                    salaire_mensuel: r.get(2)?,
                    tarif_journalier: r.get(3)?,
                    commission_pct: r.get(4)?,
                    a_la_tache: r.get::<i64>(5)? != 0,
                },
                utilisateur_id: r.get(6)?,
                declare: r.get::<i64>(7)? != 0,
                entree: r.get(8)?,
                depart: r.get(9)?,
            })
        },
    )
    .map_err(|e| e.0)?
    .ok_or_else(|| "Fiche du personnel introuvable.".to_string())
}

struct Entete {
    id: String,
    employe_id: String,
    du: String,
    au: String,
    prorata: bool,
    statut: String,
    numero: Option<String>,
    rectifie_id: Option<String>,
    /// Ce qui a servi au calcul (point 4). Vide sur une fiche d'avant le
    /// 28/10 : elle se relit alors sur ses lignes.
    conditions: Option<Conditions>,
}

fn entete(base: &mut impl Acces, fiche_id: &str) -> Result<Entete, String> {
    let dossier = base.dossier().to_string();
    let (e, conditions) = base
        .lire_une(
            "SELECT id, employe_id, du, au, prorata, statut, numero, rectifie_id, conditions
             FROM fiche_paie WHERE id = ?1 AND dossier_id = ?2",
            &parametres![fiche_id, dossier],
            |r| {
                Ok((
                    Entete {
                        id: r.get(0)?,
                        employe_id: r.get(1)?,
                        du: r.get(2)?,
                        au: r.get(3)?,
                        prorata: r.get::<i64>(4)? != 0,
                        statut: r.get(5)?,
                        numero: r.get(6)?,
                        rectifie_id: r.get(7)?,
                        conditions: None,
                    },
                    r.get::<Option<String>>(8)?,
                ))
            },
        )
        .map_err(|e| e.0)?
        .ok_or_else(|| "Fiche de paie introuvable.".to_string())?;
    let conditions = match conditions {
        None => None,
        Some(t) => Some(serde_json::from_str::<Conditions>(&t).map_err(|_| "Les conditions de cette fiche ne se relisent pas.".to_string())?),
    };
    Ok(Entete { conditions, ..e })
}

fn brouillon(base: &mut impl Acces, fiche_id: &str) -> Result<Entete, String> {
    let e = entete(base, fiche_id)?;
    if e.statut != "brouillon" {
        return Err(format!(
            "La fiche {} est validée : elle ne change plus. Une erreur se corrige par une fiche rectificative.",
            e.numero.as_deref().unwrap_or("")
        ));
    }
    Ok(e)
}

/// Les lignes d'une fiche, dans l'ordre du bulletin, avec leur id.
fn lignes(base: &mut impl Acces, fiche_id: &str) -> Result<Vec<(String, Ligne)>, String> {
    let dossier = base.dossier().to_string();
    base.lire_plusieurs(
        "SELECT id, genre, libelle, quantite, prix, montant, source, saisie
         FROM ligne_paie WHERE fiche_id = ?1 AND dossier_id = ?2 ORDER BY rang",
        &parametres![fiche_id, dossier],
        |r| {
            Ok((
                r.get::<String>(0)?,
                Ligne {
                    genre: r.get(1)?,
                    libelle: r.get(2)?,
                    quantite: r.get(3)?,
                    prix: r.get(4)?,
                    montant: r.get(5)?,
                    source: r.get(6)?,
                    saisie: r.get::<i64>(7)? != 0,
                },
            ))
        },
    )
    .map_err(|e| e.0)
}

/// Les ventes signees par un compte entre deux jours (D26), TTC, hors
/// ventes annulees — la mesure du journal des ventes (E-3).
fn ventes_signees(base: &mut impl Acces, utilisateur_id: &str, du: &str, au: &str) -> Result<i64, String> {
    let dossier = base.dossier().to_string();
    base.lire_une(
        "SELECT CAST(COALESCE(SUM(CAST(lv.prix_pratique * lv.quantite AS BIGINT)), 0) AS BIGINT)
         FROM vente v JOIN ligne_vente lv ON lv.vente_id = v.id AND lv.dossier_id = v.dossier_id
         WHERE v.dossier_id = ?1 AND v.auteur_id = ?2 AND v.statut != 'annulee'
           AND SUBSTR(v.date_vente, 1, 10) >= ?3 AND SUBSTR(v.date_vente, 1, 10) <= ?4",
        &parametres![dossier, utilisateur_id, du, au],
        |r| r.get::<i64>(0),
    )
    .map_err(|e| e.0)
    .map(|v| v.unwrap_or(0))
}

/// Les retours DATES entre deux jours sur les ventes de ce compte faites
/// pendant l'emploi de la personne (`vendu_du..vendu_au`, ouvertes si
/// vides) — comme le chiffre d'affaires net du lot B : a la date du
/// retour, l'echange mis a part (`coeur::calcul::RETOUR_HORS_CA`), une
/// vente annulee deja sortie (revue du 28/10, point 7).
fn retours_signes(
    base: &mut impl Acces,
    utilisateur_id: &str,
    du: &str,
    au: &str,
    vendu_du: Option<&str>,
    vendu_au: Option<&str>,
) -> Result<i64, String> {
    let dossier = base.dossier().to_string();
    base.lire_une(
        "SELECT CAST(COALESCE(SUM(rt.montant_credit), 0) AS BIGINT)
         FROM retour rt JOIN vente v ON v.id = rt.vente_id AND v.dossier_id = rt.dossier_id
         WHERE rt.dossier_id = ?1 AND v.auteur_id = ?2 AND v.statut != 'annulee'
           AND rt.mode_resolution <> ?5
           AND SUBSTR(rt.date_retour, 1, 10) >= ?3 AND SUBSTR(rt.date_retour, 1, 10) <= ?4
           AND (CAST(?6 AS TEXT) IS NULL OR SUBSTR(v.date_vente, 1, 10) >= ?6)
           AND (CAST(?7 AS TEXT) IS NULL OR SUBSTR(v.date_vente, 1, 10) <= ?7)",
        &parametres![
            dossier,
            utilisateur_id,
            du,
            au,
            crate::coeur::calcul::RETOUR_HORS_CA,
            vendu_du.map(str::to_string),
            vendu_au.map(str::to_string)
        ],
        |r| r.get::<i64>(0),
    )
    .map_err(|e| e.0)
    .map(|v| v.unwrap_or(0))
}

/// Les departs des AUTRES personnes liees au meme compte.
fn departs_des_autres(base: &mut impl Acces, utilisateur_id: &str, employe_id: &str) -> Result<Vec<String>, String> {
    let dossier = base.dossier().to_string();
    base.lire_plusieurs(
        "SELECT date_depart FROM employe
         WHERE dossier_id = ?1 AND utilisateur_id = ?2 AND id <> ?3 AND date_depart IS NOT NULL",
        &parametres![dossier, utilisateur_id, employe_id],
        |r| r.get::<String>(0),
    )
    .map_err(|e| e.0)
}

/// Les marques de presence d'une personne entre deux jours.
fn marques(base: &mut impl Acces, employe_id: &str, du: &str, au: &str) -> Result<(i64, i64, i64), String> {
    let dossier = base.dossier().to_string();
    let etats = base
        .lire_plusieurs(
            "SELECT etat FROM presence WHERE dossier_id = ?1 AND employe_id = ?2 AND jour >= ?3 AND jour <= ?4",
            &parametres![dossier, employe_id, du, au],
            |r| r.get::<String>(0),
        )
        .map_err(|e| e.0)?;
    Ok(crate::coeur::presences::compter(etats.iter().map(String::as_str)))
}

/// Les avances a retenir : ouvertes, donnees avant la fin de la
/// periode. Pour une rectificative, ce que la fiche remplacee en avait
/// retenu compte comme disponible (il sera rendu a la validation).
fn avances_a_retenir(base: &mut impl Acces, employe_id: &str, au: &str, remplace: Option<&str>) -> Result<Vec<AvanceARetenir>, String> {
    let dossier = base.dossier().to_string();
    base.lire_plusieurs(
        "SELECT a.id, a.date_avance,
                CAST(a.montant - a.retenu + COALESCE((SELECT SUM(-lp.montant) FROM ligne_paie lp
                     WHERE lp.dossier_id = a.dossier_id AND lp.fiche_id = ?4
                       AND lp.genre = 'avance' AND lp.source = a.id), 0) AS BIGINT)
         FROM avance a
         WHERE a.dossier_id = ?1 AND a.employe_id = ?2 AND a.statut = 'ouverte'
           AND SUBSTR(a.date_avance, 1, 10) <= ?3",
        &parametres![dossier, employe_id, au, remplace],
        |r| Ok(AvanceARetenir { id: r.get(0)?, date: r.get(1)?, reste: r.get(2)? }),
    )
    .map_err(|e| e.0)
}

/// Les conditions de la fiche du personnel, aujourd'hui : ce qu'un
/// brouillon ordinaire suit.
fn conditions_actuelles(base: &mut impl Acces, p: &Personne) -> Result<Conditions, String> {
    Ok(Conditions {
        remuneration: p.r.clone(),
        declare: p.declare,
        utilisateur_id: p.utilisateur_id.clone(),
        // D33 : les cotisations ne touchent que les personnes declarees.
        cotisations: if p.declare { cotisations_sur(base)? } else { Vec::new() },
    })
}

/// Les conditions figees d'une fiche validee : celles qu'elle porte, ou,
/// pour une fiche d'avant le 28/10, celles qu'on relit sur ses lignes
/// (`coeur::paie::conditions_depuis_lignes` — le choix le plus prudent :
/// ce sont les lignes remises a la personne).
fn conditions_figees(base: &mut impl Acces, fiche_id: &str, p: &Personne) -> Result<(Conditions, bool), String> {
    let e = entete(base, fiche_id)?;
    match e.conditions {
        Some(c) => Ok((c, false)),
        None => {
            let l: Vec<Ligne> = lignes(base, fiche_id)?.into_iter().map(|(_, x)| x).collect();
            Ok((regles::conditions_depuis_lignes(&l, p.utilisateur_id.clone())?, true))
        }
    }
}

/// Les conditions qui calculent cette fiche : figees pour une
/// rectificative, celles d'aujourd'hui pour un brouillon ordinaire.
fn conditions_de(base: &mut impl Acces, e: &Entete, p: &Personne) -> Result<Conditions, String> {
    match (&e.rectifie_id, &e.conditions) {
        (Some(_), Some(c)) => Ok(c.clone()),
        // Une rectificative ouverte avant le 28/10 : celles de la fiche
        // qu'elle remplace.
        (Some(r), None) => Ok(conditions_figees(base, r, p)?.0),
        (None, _) => conditions_actuelles(base, p),
    }
}

/// Ce que la base sait de la periode, borne aux jours d'emploi.
fn periode(base: &mut impl Acces, e: &Entete, p: &Personne, c: &Conditions) -> Result<Periode, String> {
    let (debut, fin) = regles::bornes_d_emploi(&e.du, &e.au, p.entree.as_deref(), p.depart.as_deref())
        .ok_or_else(|| regles::hors_emploi(&p.nom, &e.du, &e.au, p.entree.as_deref(), p.depart.as_deref()))?;
    if c.remuneration.salaire_mensuel.is_some() {
        regles::verifier_un_seul_mois(&e.du, &e.au)?;
    }
    let (presents, demis, absents) = marques(base, &e.employe_id, &debut, &fin)?;
    let (ventes, retours) = match (&c.remuneration.commission_pct, &c.utilisateur_id) {
        (Some(_), Some(u)) => {
            // Le compte ne compte pour elle que depuis son entree, ou
            // depuis le depart de qui l'avait avant elle (point 1).
            let autres = departs_des_autres(base, u, &e.employe_id)?;
            let debut_compte = regles::debut_du_compte(p.entree.as_deref(), p.depart.as_deref(), &autres);
            let depuis = match &debut_compte {
                Some(d) if d.as_str() > debut.as_str() => d.clone(),
                _ => debut.clone(),
            };
            let ventes = if depuis <= fin { ventes_signees(base, u, &depuis, &fin)? } else { 0 };
            let retours = retours_signes(base, u, &debut, &fin, debut_compte.as_deref(), p.depart.as_deref())?;
            (Some(ventes), retours)
        }
        _ => (None, 0),
    };
    Ok(Periode {
        jours_du_mois: regles::jours_du_mois_de(&e.du)?,
        jours_emploi: regles::nombre_de_jours(&debut, &fin)?,
        presents,
        demis,
        absents,
        ventes,
        retours,
    })
}

/// Calcule une fiche depuis la base : ce que la personne gagne sur ses
/// jours d'emploi de la periode, les saisies, les avances. Rend aussi
/// les conditions qui ont servi.
fn calculer(base: &mut impl Acces, e: &Entete, p: &Personne, saisies: &[Ligne]) -> Result<(Vec<Ligne>, i64, Conditions), String> {
    let c = conditions_de(base, e, p)?;
    let periode = periode(base, e, p, &c)?;
    let avances = avances_a_retenir(base, &e.employe_id, &e.au, e.rectifie_id.as_deref())?;
    let cotisations: &[Cotisation] = if c.declare { &c.cotisations } else { &[] };
    let (lignes, reporte) = regles::calculer(&c.remuneration, &periode, e.prorata, saisies, &avances, cotisations)?;
    Ok((lignes, reporte, c))
}

fn conditions_json(c: &Conditions) -> String {
    serde_json::to_string(c).unwrap_or_default()
}

fn ecrire_lignes(acces: &mut impl Acces, fiche_id: &str, lignes: &[Ligne], reporte: i64) -> Result<(), String> {
    let dossier = acces.dossier().to_string();
    let (brut, retenues, net) = regles::totaux(lignes)?;
    acces
        .executer("DELETE FROM ligne_paie WHERE fiche_id = ?1 AND dossier_id = ?2", &parametres![fiche_id, dossier.clone()])
        .map_err(|e| e.0)?;
    for (rang, l) in lignes.iter().enumerate() {
        acces
            .executer(
                "INSERT INTO ligne_paie (id, dossier_id, fiche_id, rang, genre, libelle, quantite, prix, montant, source, saisie)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                &parametres![
                    uuid::Uuid::new_v4().to_string(),
                    dossier.clone(),
                    fiche_id,
                    rang as i64,
                    l.genre.clone(),
                    l.libelle.clone(),
                    l.quantite,
                    l.prix,
                    l.montant,
                    l.source.clone(),
                    l.saisie as i64
                ],
            )
            .map_err(|e| e.0)?;
    }
    acces
        .executer(
            "UPDATE fiche_paie SET brut = ?1, retenues = ?2, net = ?3, reporte = ?4, modifie_le = ?5
             WHERE id = ?6 AND dossier_id = ?7",
            &parametres![brut, retenues, net, reporte, crate::utils::maintenant_iso(), fiche_id, dossier],
        )
        .map_err(|e| e.0)?;
    Ok(())
}

fn noter(acces: &mut impl Acces, type_evenement: &str, entite: &str, id: &str, auteur: &str, valeur: serde_json::Value) -> Result<(), String> {
    let dossier = acces.dossier().to_string();
    acces
        .executer(
            "INSERT INTO journal
               (id, type_evenement, entite_type, entite_id, auteur_id, nouveau_valeur, origine, date_evenement, dossier_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'app', ?7, ?8)",
            &parametres![
                uuid::Uuid::new_v4().to_string(),
                type_evenement,
                entite,
                id,
                auteur,
                valeur.to_string(),
                crate::utils::maintenant_iso(),
                dossier
            ],
        )
        .map(|_| ())
        .map_err(|e| e.0)
}

/// Une fiche vivante (brouillon ou validee) de la personne qui touche
/// deja cette periode ? `sauf` : la fiche qu'une rectificative remplace.
fn chevauchement(base: &mut Base, employe_id: &str, du: &str, au: &str, sauf: Option<&str>) -> Result<Option<String>, String> {
    let dossier = base.dossier().to_string();
    let fiches = base
        .lire_plusieurs(
            "SELECT id, du, au, statut, numero FROM fiche_paie
             WHERE dossier_id = ?1 AND employe_id = ?2 AND statut IN ('brouillon', 'validee')",
            &parametres![dossier, employe_id],
            |r| Ok((r.get::<String>(0)?, r.get::<String>(1)?, r.get::<String>(2)?, r.get::<String>(3)?, r.get::<Option<String>>(4)?)),
        )
        .map_err(|e| e.0)?;
    Ok(fiches.into_iter().find(|f| Some(f.0.as_str()) != sauf && regles::se_chevauchent(du, au, &f.1, &f.2)).map(|f| {
        let quoi = match (f.3.as_str(), f.4) {
            ("validee", Some(n)) => format!("la fiche {n}"),
            _ => "un brouillon".to_string(),
        };
        format!("du {} au {} ({quoi})", regles::date_dite(&f.1), regles::date_dite(&f.2))
    }))
}

/// La fiche validee (et pas en cours de rectification) qui couvre ce
/// jour de la personne, s'il y en a une : son numero. Un tel jour ne se
/// marque plus (point 5) ; une rectificative en brouillon le rouvre —
/// elle relira les jours.
pub fn fiche_validee_du_jour_sur(acces: &mut impl Acces, employe_id: &str, jour: &str) -> Result<Option<String>, String> {
    let dossier = acces.dossier().to_string();
    acces
        .lire_une(
            "SELECT f.numero FROM fiche_paie f
             WHERE f.dossier_id = ?1 AND f.employe_id = ?2 AND f.statut = 'validee'
               AND f.du <= ?3 AND f.au >= ?3
               AND NOT EXISTS (SELECT 1 FROM fiche_paie r
                               WHERE r.rectifie_id = f.id AND r.dossier_id = f.dossier_id AND r.statut = 'brouillon')",
            &parametres![dossier, employe_id, jour],
            |r| r.get::<Option<String>>(0),
        )
        .map_err(|e| e.0)
        .map(Option::flatten)
}

/// Les fiches validees (non remplacees) d'une personne qui couvrent des
/// jours apres une date : leurs numeros.
pub fn fiches_validees_apres_sur(acces: &mut impl Acces, employe_id: &str, jour: &str) -> Result<Vec<String>, String> {
    let dossier = acces.dossier().to_string();
    acces
        .lire_plusieurs(
            "SELECT numero FROM fiche_paie
             WHERE dossier_id = ?1 AND employe_id = ?2 AND statut = 'validee' AND au > ?3 AND numero IS NOT NULL
             ORDER BY du",
            &parametres![dossier, employe_id, jour],
            |r| r.get::<String>(0),
        )
        .map_err(|e| e.0)
}

/// Une fiche validee couvre-t-elle ce jour de la personne ?
pub fn jour_paye_sur(acces: &mut impl Acces, employe_id: &str, jour: &str) -> Result<bool, String> {
    let dossier = acces.dossier().to_string();
    Ok(acces
        .lire_une(
            "SELECT CAST(COUNT(*) AS BIGINT) FROM fiche_paie
             WHERE dossier_id = ?1 AND employe_id = ?2 AND statut = 'validee' AND du <= ?3 AND au >= ?3",
            &parametres![dossier, employe_id, jour],
            |r| r.get::<i64>(0),
        )
        .map_err(|e| e.0)?
        .unwrap_or(0)
        > 0)
}

fn saisies_de(l: &[(String, Ligne)]) -> Vec<Ligne> {
    l.iter().filter(|(_, x)| x.saisie).map(|(_, x)| x.clone()).collect()
}

const DEJA_PLUS_UN_BROUILLON: &str = "Cette fiche vient d'être validée ou jetée par quelqu'un d'autre : la relire.";

/// Refait les lignes calculees d'un brouillon, garde les saisies.
fn refaire(base: &mut Base, e: &Entete, saisies: Vec<Ligne>) -> Result<(), String> {
    let p = personne(base, &e.employe_id)?;
    let (calcul, reporte, c) = calculer(base, e, &p, &saisies)?;
    let dossier = base.dossier().to_string();
    let mut tx = base.transaction().map_err(|e| e.0)?;
    let n = tx
        .executer(
            "UPDATE fiche_paie SET nom = ?1, fonction = ?2, prorata = ?3, conditions = ?4
             WHERE id = ?5 AND dossier_id = ?6 AND statut = 'brouillon'",
            &parametres![p.nom, p.fonction, e.prorata as i64, conditions_json(&c), e.id.clone(), dossier],
        )
        .map_err(|e| e.0)?;
    if n != 1 {
        return Err(DEJA_PLUS_UN_BROUILLON.to_string());
    }
    ecrire_lignes(&mut tx, &e.id, &calcul, reporte)?;
    tx.valider().map_err(|e| e.0)
}

/// Prepare le brouillon d'une personne pour une periode. Refuse si une
/// fiche vivante la couvre deja, si la personne n'etait pas employee un
/// seul jour de la periode, et une periode qui n'est pas un mois civil
/// entier pour qui est paye au mois.
pub fn preparer_sur(base: &mut Base, employe_id: String, du: String, au: String, prorata: bool) -> Result<serde_json::Value, String> {
    regles::verifier_periode(&du, &au)?;
    let p = personne(base, &employe_id)?;
    if p.r.salaire_mensuel.is_some() {
        regles::verifier_mois_entier(&du, &au)?;
    }
    if regles::bornes_d_emploi(&du, &au, p.entree.as_deref(), p.depart.as_deref()).is_none() {
        return Err(regles::hors_emploi(&p.nom, &du, &au, p.entree.as_deref(), p.depart.as_deref()));
    }
    if let Some(deja) = chevauchement(base, &employe_id, &du, &au, None)? {
        return Err(format!("{} a déjà une fiche {deja}.", p.nom));
    }
    let id = uuid::Uuid::new_v4().to_string();
    let e = Entete {
        id: id.clone(),
        employe_id: employe_id.clone(),
        du: du.clone(),
        au: au.clone(),
        prorata,
        statut: "brouillon".into(),
        numero: None,
        rectifie_id: None,
        conditions: None,
    };
    let (calcul, reporte, c) = calculer(base, &e, &p, &[])?;
    let dossier = base.dossier().to_string();
    let auteur = crate::argent::id_utilisateur_courant_sur(base);
    let now = crate::utils::maintenant_iso();
    let mut tx = base.transaction().map_err(|e| e.0)?;
    tx.executer(
        "INSERT INTO fiche_paie (id, dossier_id, employe_id, nom, fonction, du, au, prorata, statut, conditions, cree_par, cree_le, modifie_le)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'brouillon', ?9, ?10, ?11, ?11)",
        &parametres![
            id.clone(),
            dossier,
            employe_id,
            p.nom.clone(),
            p.fonction,
            du.clone(),
            au.clone(),
            prorata as i64,
            conditions_json(&c),
            auteur.clone(),
            now
        ],
    )
    .map_err(|e| e.0)?;
    ecrire_lignes(&mut tx, &id, &calcul, reporte)?;
    noter(&mut tx, "paie_preparee", "fiche_paie", &id, &auteur, serde_json::json!({ "nom": p.nom, "du": du, "au": au }))?;
    tx.valider().map_err(|e| e.0)?;
    lire_sur(base, &id)
}

/// Recalcule un brouillon (jours, ventes, avances ont pu bouger) ;
/// `prorata` le change s'il est donne.
pub fn recalculer_sur(base: &mut Base, fiche_id: String, prorata: Option<bool>) -> Result<serde_json::Value, String> {
    let mut e = brouillon(base, &fiche_id)?;
    if let Some(p) = prorata {
        e.prorata = p;
    }
    let saisies = saisies_de(&lignes(base, &fiche_id)?);
    refaire(base, &e, saisies)?;
    lire_sur(base, &fiche_id)
}

/// Ajoute une ligne saisie (tache, prime, retenue) a un brouillon.
pub fn ajouter_ligne_sur(
    base: &mut Base,
    fiche_id: String,
    genre: String,
    libelle: String,
    quantite: Option<f64>,
    prix: Option<i64>,
    montant: Option<i64>,
) -> Result<serde_json::Value, String> {
    let e = brouillon(base, &fiche_id)?;
    let nouvelle = regles::ligne_saisie(&genre, &libelle, quantite, prix, montant)?;
    let mut saisies = saisies_de(&lignes(base, &fiche_id)?);
    saisies.push(nouvelle);
    refaire(base, &e, saisies)?;
    lire_sur(base, &fiche_id)
}

/// Retire une ligne saisie d'un brouillon. Une ligne calculee ne se
/// retire pas : elle vient de la fiche du personnel ou de la caisse.
pub fn retirer_ligne_sur(base: &mut Base, fiche_id: String, ligne_id: String) -> Result<serde_json::Value, String> {
    let e = brouillon(base, &fiche_id)?;
    let tout = lignes(base, &fiche_id)?;
    let Some((_, l)) = tout.iter().find(|(id, _)| *id == ligne_id) else {
        return Err("Ligne introuvable.".to_string());
    };
    if !l.saisie {
        return Err(format!("« {} » est calculée : elle se corrige sur la fiche de la personne, les jours ou la caisse.", l.libelle));
    }
    let saisies: Vec<Ligne> = tout.iter().filter(|(id, x)| x.saisie && *id != ligne_id).map(|(_, x)| x.clone()).collect();
    refaire(base, &e, saisies)?;
    lire_sur(base, &fiche_id)
}

/// Jette un brouillon. Une fiche validee ne se jette pas.
pub fn supprimer_sur(base: &mut Base, fiche_id: String) -> Result<serde_json::Value, String> {
    let e = brouillon(base, &fiche_id)?;
    let dossier = base.dossier().to_string();
    let auteur = crate::argent::id_utilisateur_courant_sur(base);
    let nom = personne(base, &e.employe_id).map(|p| p.nom).unwrap_or_default();
    let mut tx = base.transaction().map_err(|e| e.0)?;
    // Encore un brouillon au moment d'ecrire : sinon rien ne se jette.
    let n = tx
        .executer(
            "UPDATE fiche_paie SET modifie_le = ?1 WHERE id = ?2 AND dossier_id = ?3 AND statut = 'brouillon'",
            &parametres![crate::utils::maintenant_iso(), fiche_id.clone(), dossier.clone()],
        )
        .map_err(|e| e.0)?;
    if n != 1 {
        return Err(DEJA_PLUS_UN_BROUILLON.to_string());
    }
    tx.executer("DELETE FROM ligne_paie WHERE fiche_id = ?1 AND dossier_id = ?2", &parametres![fiche_id.clone(), dossier.clone()])
        .map_err(|e| e.0)?;
    tx.executer("DELETE FROM fiche_paie WHERE id = ?1 AND dossier_id = ?2 AND statut = 'brouillon'", &parametres![fiche_id.clone(), dossier])
        .map_err(|e| e.0)?;
    noter(&mut tx, "paie_supprimee", "fiche_paie", &fiche_id, &auteur, serde_json::json!({ "nom": nom, "du": e.du, "au": e.au }))?;
    tx.valider().map_err(|e| e.0)?;
    Ok(serde_json::json!({ "id": fiche_id, "nom": nom }))
}

fn memes(a: &[Ligne], b: &[Ligne]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(x, y)| x.genre == y.genre && x.montant == y.montant && x.source == y.source && x.libelle == y.libelle)
}

fn somme_sql(acces: &mut impl Acces, sql: &str, fiche_id: &str) -> Result<i64, String> {
    let dossier = acces.dossier().to_string();
    acces
        .lire_une(sql, &parametres![fiche_id, dossier], |r| r.get::<i64>(0))
        .map_err(|e| e.0)
        .map(|v| v.unwrap_or(0))
}

const VERSE: &str = "SELECT CAST(COALESCE(SUM(montant), 0) AS BIGINT) FROM versement_paie WHERE fiche_id = ?1 AND dossier_id = ?2";
const REPRIS: &str = "SELECT CAST(COALESCE(SUM(montant), 0) AS BIGINT) FROM avance
                      WHERE trop_verse_fiche_id = ?1 AND dossier_id = ?2 AND statut = 'ouverte'";

/// Valide un brouillon : numero, fige, avances retenues. Refuse ce qui
/// n'est plus a jour et un net negatif. Une rectificative remplace
/// l'ancienne, reprend ses versements, et ce qui a ete verse en trop
/// devient une avance retenue sur la fiche suivante (D32, point 6).
pub fn valider_sur(base: &mut Base, fiche_id: String) -> Result<serde_json::Value, String> {
    let e = brouillon(base, &fiche_id)?;
    let stockees = lignes(base, &fiche_id)?;
    let actuelles: Vec<Ligne> = stockees.iter().map(|(_, l)| l.clone()).collect();
    let p = personne(base, &e.employe_id)?;
    let (frais, _, conditions) = calculer(base, &e, &p, &saisies_de(&stockees))?;
    if !memes(&actuelles, &frais) {
        return Err(
            "Quelque chose a changé depuis le calcul (fiche de la personne, jours, ventes, avances ou cotisations) : \
             recalculer la fiche et la relire avant de la valider."
                .to_string(),
        );
    }
    regles::validable(&actuelles)?;
    let (brut, retenues, net) = regles::totaux(&actuelles)?;

    let dossier = base.dossier().to_string();
    let auteur = crate::argent::id_utilisateur_courant_sur(base);
    let now = crate::utils::maintenant_iso();
    let ancien = match &e.rectifie_id {
        Some(r) => Some((r.clone(), lignes(base, r)?, entete(base, r)?.numero)),
        None => None,
    };
    let mut tx = base.transaction().map_err(|e| e.0)?;
    let numero = crate::argent::reserver_numero_sur(&mut tx, "fiche_paie")?;
    // Encore un brouillon au moment d'ecrire (point 5) : deux validations
    // en meme temps n'ecrivent pas deux fois. Sinon, rien ne s'ecrit.
    let n = tx
        .executer(
            "UPDATE fiche_paie SET statut = 'validee', numero = ?1, valide_par = ?2, valide_le = ?3, modifie_le = ?3, conditions = ?4
             WHERE id = ?5 AND dossier_id = ?6 AND statut = 'brouillon'",
            &parametres![numero.clone(), auteur.clone(), now.clone(), conditions_json(&conditions), fiche_id.clone(), dossier.clone()],
        )
        .map_err(|e| e.0)?;
    if n != 1 {
        return Err(DEJA_PLUS_UN_BROUILLON.to_string());
    }
    // La fiche remplacee rend d'abord ce qu'elle avait retenu.
    if let Some((ancien_id, anciennes, _)) = &ancien {
        let n = tx
            .executer(
                "UPDATE fiche_paie SET statut = 'remplacee', modifie_le = ?1 WHERE id = ?2 AND dossier_id = ?3 AND statut = 'validee'",
                &parametres![now.clone(), ancien_id.clone(), dossier.clone()],
            )
            .map_err(|e| e.0)?;
        if n != 1 {
            return Err("La fiche à rectifier vient d'être remplacée par une autre rectificative : la relire.".to_string());
        }
        for (_, l) in anciennes.iter().filter(|(_, l)| l.genre == regles::AVANCE) {
            tx.executer(
                "UPDATE avance SET retenu = retenu - ?1 WHERE id = ?2 AND dossier_id = ?3",
                &parametres![-l.montant, l.source.clone(), dossier.clone()],
            )
            .map_err(|e| e.0)?;
        }
        // Ce qui a deja ete verse sur l'ancienne l'a ete pour la meme
        // periode : les versements suivent la fiche qui vaut, et ce qui
        // en avait deja ete repris en avance aussi.
        tx.executer(
            "UPDATE versement_paie SET fiche_id = ?1 WHERE fiche_id = ?2 AND dossier_id = ?3",
            &parametres![fiche_id.clone(), ancien_id.clone(), dossier.clone()],
        )
        .map_err(|e| e.0)?;
        tx.executer(
            "UPDATE avance SET trop_verse_fiche_id = ?1 WHERE trop_verse_fiche_id = ?2 AND dossier_id = ?3",
            &parametres![fiche_id.clone(), ancien_id.clone(), dossier.clone()],
        )
        .map_err(|e| e.0)?;
    }
    // Deux fiches validees en meme temps ne retiennent pas deux fois la
    // meme avance : la ligne ne monte que si elle ne depasse pas.
    for l in actuelles.iter().filter(|l| l.genre == regles::AVANCE) {
        let n = tx
            .executer(
                "UPDATE avance SET retenu = retenu + ?1
                 WHERE id = ?2 AND dossier_id = ?3 AND statut = 'ouverte' AND retenu + ?1 <= montant",
                &parametres![-l.montant, l.source.clone(), dossier.clone()],
            )
            .map_err(|e| e.0)?;
        if n != 1 {
            return Err(format!("« {} » a déjà été retenue ailleurs : recalculer la fiche.", l.libelle));
        }
    }
    // Le trop-verse d'une rectificative devient une avance, retenue sur
    // la fiche suivante (D32) : dans cette transaction, au journal.
    let mut trop_verse = 0;
    if let Some((_, _, ancien_numero)) = &ancien {
        let verse = somme_sql(&mut tx, VERSE, &fiche_id)?;
        let repris = somme_sql(&mut tx, REPRIS, &fiche_id)?;
        trop_verse = regles::trop_verse_a_reprendre(net, verse, repris);
        if trop_verse > 0 {
            let avance_id = uuid::Uuid::new_v4().to_string();
            let motif = format!(
                "Versé en trop sur {} (rectifiée par {numero}) : retenu sur la paie suivante",
                ancien_numero.as_deref().unwrap_or("la fiche rectifiée")
            );
            tx.executer(
                "INSERT INTO avance
                   (id, dossier_id, employe_id, montant, retenu, moyen, motif, date_avance, statut,
                    mouvement_caisse_id, cree_par, cree_le, trop_verse_fiche_id)
                 VALUES (?1, ?2, ?3, ?4, 0, ?5, ?6, ?7, 'ouverte', NULL, ?8, ?7, ?9)",
                &parametres![
                    avance_id.clone(),
                    dossier.clone(),
                    e.employe_id.clone(),
                    trop_verse,
                    regles::MOYEN_TROP_VERSE,
                    motif,
                    now.clone(),
                    auteur.clone(),
                    fiche_id.clone()
                ],
            )
            .map_err(|e| e.0)?;
            noter(
                &mut tx,
                "avance_trop_verse",
                "avance",
                &avance_id,
                &auteur,
                serde_json::json!({ "nom": p.nom, "montant": trop_verse, "fiche": numero, "remplace": ancien_numero }),
            )?;
        }
    }
    let ancien_numero = ancien.as_ref().and_then(|a| a.2.clone());
    noter(
        &mut tx,
        "paie_validee",
        "fiche_paie",
        &fiche_id,
        &auteur,
        serde_json::json!({
            "nom": p.nom, "numero": numero, "du": e.du, "au": e.au, "brut": brut, "retenues": retenues, "net": net,
            "remplace": ancien_numero, "trop_verse": (trop_verse > 0).then_some(trop_verse),
        }),
    )?;
    tx.valider().map_err(|e| e.0)?;
    lire_sur(base, &fiche_id)
}

/// Ouvre une rectificative d'une fiche validee : un brouillon de la
/// meme periode, avec les memes saisies et les conditions figees de
/// l'ancienne (point 4), recalcule sur les faits d'aujourd'hui (jours,
/// ventes, avances, dates d'emploi). L'ancienne reste valable jusqu'a la
/// validation de la nouvelle ; ses jours se rouvrent pendant ce temps.
pub fn rectifier_sur(base: &mut Base, fiche_id: String) -> Result<serde_json::Value, String> {
    let ancienne = entete(base, &fiche_id)?;
    if ancienne.statut != "validee" {
        return Err(match ancienne.statut.as_str() {
            "brouillon" => "C'est un brouillon : il se corrige directement.".to_string(),
            _ => "Cette fiche a déjà été remplacée.".to_string(),
        });
    }
    let p = personne(base, &ancienne.employe_id)?;
    if let Some(deja) = chevauchement(base, &ancienne.employe_id, &ancienne.du, &ancienne.au, Some(&fiche_id))? {
        return Err(format!("{} a déjà une fiche {deja} : la finir ou la jeter d'abord.", p.nom));
    }
    let (figees, relues) = conditions_figees(base, &fiche_id, &p)?;
    let saisies = saisies_de(&lignes(base, &fiche_id)?);
    let ancien_numero = ancienne.numero.clone();
    let id = uuid::Uuid::new_v4().to_string();
    let e = Entete {
        id: id.clone(),
        rectifie_id: Some(fiche_id.clone()),
        statut: "brouillon".into(),
        numero: None,
        conditions: Some(figees),
        ..ancienne
    };
    let (calcul, reporte, c) = calculer(base, &e, &p, &saisies)?;
    let dossier = base.dossier().to_string();
    let auteur = crate::argent::id_utilisateur_courant_sur(base);
    let now = crate::utils::maintenant_iso();
    let mut tx = base.transaction().map_err(|e| e.0)?;
    tx.executer(
        "INSERT INTO fiche_paie (id, dossier_id, employe_id, nom, fonction, du, au, prorata, statut, rectifie_id, conditions, cree_par, cree_le, modifie_le)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'brouillon', ?9, ?10, ?11, ?12, ?12)",
        &parametres![
            id.clone(),
            dossier,
            e.employe_id.clone(),
            p.nom.clone(),
            p.fonction,
            e.du.clone(),
            e.au.clone(),
            e.prorata as i64,
            fiche_id.clone(),
            conditions_json(&c),
            auteur.clone(),
            now
        ],
    )
    .map_err(|e| e.0)?;
    ecrire_lignes(&mut tx, &id, &calcul, reporte)?;
    noter(
        &mut tx,
        "paie_rectificative",
        "fiche_paie",
        &id,
        &auteur,
        serde_json::json!({
            "nom": p.nom, "remplace": ancien_numero,
            // Une fiche d'avant le 28/10 ne portait pas ses conditions :
            // on les a relues sur ses lignes. Le journal le dit.
            "conditions": if relues { "relues sur les lignes de l'ancienne fiche" } else { "celles de l'ancienne fiche" },
        }),
    )?;
    tx.valider().map_err(|e| e.0)?;
    lire_sur(base, &id)
}

fn ligne_json(id: &str, l: &Ligne) -> serde_json::Value {
    serde_json::json!({
        "id": id, "genre": l.genre, "libelle": l.libelle, "quantite": l.quantite, "prix": l.prix,
        "montant": l.montant, "source": l.source, "saisie": l.saisie,
    })
}

const COLONNES: &str = "f.id, f.employe_id, f.nom, f.fonction, f.du, f.au, f.prorata, f.statut, f.numero,
    f.brut, f.retenues, f.net, f.reporte, f.rectifie_id, r.numero, f.cree_le, f.valide_le, u.nom,
    (SELECT n.numero FROM fiche_paie n WHERE n.rectifie_id = f.id AND n.dossier_id = f.dossier_id AND n.statut IN ('validee', 'remplacee')),
    (SELECT CAST(COALESCE(SUM(v.montant), 0) AS BIGINT) FROM versement_paie v WHERE v.fiche_id = f.id AND v.dossier_id = f.dossier_id),
    (SELECT CAST(COALESCE(SUM(a.montant), 0) AS BIGINT) FROM avance a
      WHERE a.trop_verse_fiche_id = f.id AND a.dossier_id = f.dossier_id AND a.statut = 'ouverte')";

fn fiche_json(r: &crate::base::Ligne<'_>) -> crate::base::Resultat<serde_json::Value> {
    let net = r.get::<i64>(11)?;
    let verse = r.get::<i64>(19)?;
    let repris = r.get::<i64>(20)?;
    Ok(serde_json::json!({
        "id": r.get::<String>(0)?,
        "employe_id": r.get::<String>(1)?,
        "nom": r.get::<String>(2)?,
        "fonction": r.get::<String>(3)?,
        "du": r.get::<String>(4)?,
        "au": r.get::<String>(5)?,
        "prorata": r.get::<i64>(6)? != 0,
        "statut": r.get::<String>(7)?,
        "numero": r.get::<Option<String>>(8)?,
        "brut": r.get::<i64>(9)?,
        "retenues": r.get::<i64>(10)?,
        "net": net,
        "reporte": r.get::<i64>(12)?,
        "rectifie_id": r.get::<Option<String>>(13)?,
        "rectifie_numero": r.get::<Option<String>>(14)?,
        "cree_le": r.get::<String>(15)?,
        "valide_le": r.get::<Option<String>>(16)?,
        "valide_par_nom": r.get::<Option<String>>(17)?,
        "remplacee_par": r.get::<Option<String>>(18)?,
        "verse": verse,
        // Verse en trop et repris en avance sur la fiche suivante.
        "trop_verse": repris,
        // Negatif : verse en trop, et pas (ou plus) repris en avance.
        "reste": regles::reste_a_verser(net, verse, repris),
    }))
}

/// Une fiche et ses lignes.
pub fn lire_sur(base: &mut Base, fiche_id: &str) -> Result<serde_json::Value, String> {
    let dossier = base.dossier().to_string();
    let mut f = base
        .lire_une(
            &format!(
                "SELECT {COLONNES} FROM fiche_paie f
                 LEFT JOIN fiche_paie r ON r.id = f.rectifie_id AND r.dossier_id = f.dossier_id
                 LEFT JOIN utilisateur u ON u.id = f.valide_par
                 WHERE f.id = ?1 AND f.dossier_id = ?2"
            ),
            &parametres![fiche_id, dossier],
            fiche_json,
        )
        .map_err(|e| e.0)?
        .ok_or_else(|| "Fiche de paie introuvable.".to_string())?;
    let l = lignes(base, fiche_id)?;
    f["lignes"] = l.iter().map(|(id, x)| ligne_json(id, x)).collect();
    f["versements"] = versements(base, fiche_id)?.into();
    Ok(f)
}

/// Les fiches dont la periode touche `du..au`, et les personnes
/// employees au moins un jour de la periode (entree <= fin, depart
/// absent ou >= debut — point 1) qui n'en ont pas encore : a preparer,
/// parties dans le mois comprises (leur derniere fiche). Les fiches
/// remplacees restent lisibles, marquees.
pub fn lister_sur(base: &mut Base, du: String, au: String) -> Result<serde_json::Value, String> {
    regles::verifier_periode(&du, &au)?;
    let dossier = base.dossier().to_string();
    let fiches = base
        .lire_plusieurs(
            &format!(
                "SELECT {COLONNES} FROM fiche_paie f
                 LEFT JOIN fiche_paie r ON r.id = f.rectifie_id AND r.dossier_id = f.dossier_id
                 LEFT JOIN utilisateur u ON u.id = f.valide_par
                 WHERE f.dossier_id = ?1 AND f.du <= ?3 AND f.au >= ?2
                 ORDER BY f.nom, f.du, f.cree_le"
            ),
            &parametres![dossier.clone(), du.clone(), au.clone()],
            fiche_json,
        )
        .map_err(|e| e.0)?;
    let personnes = base
        .lire_plusieurs(
            "SELECT id, nom, fonction, salaire_mensuel, date_entree, date_depart FROM employe
             WHERE dossier_id = ?1
               AND (date_entree IS NULL OR SUBSTR(date_entree, 1, 10) <= ?3)
               AND (date_depart IS NULL OR SUBSTR(date_depart, 1, 10) >= ?2)
             ORDER BY nom",
            &parametres![dossier, du.clone(), au.clone()],
            |r| {
                Ok((
                    r.get::<String>(0)?,
                    r.get::<String>(1)?,
                    r.get::<String>(2)?,
                    r.get::<Option<i64>>(3)?,
                    r.get::<Option<String>>(4)?,
                    r.get::<Option<String>>(5)?,
                ))
            },
        )
        .map_err(|e| e.0)?;
    let a_preparer: Vec<serde_json::Value> = personnes
        .into_iter()
        .filter(|(id, ..)| {
            !fiches.iter().any(|f| f["employe_id"] == id.as_str() && (f["statut"] == "brouillon" || f["statut"] == "validee"))
        })
        .map(|(id, nom, fonction, mois, entree, depart)| {
            serde_json::json!({
                "employe_id": id, "nom": nom, "fonction": fonction, "au_mois": mois.is_some(),
                "date_entree": entree, "date_depart": depart,
            })
        })
        .collect();
    Ok(serde_json::json!({ "du": du, "au": au, "fiches": fiches, "a_preparer": a_preparer }))
}

// =====================================================================
//  G-3 : payer (D31) — des versements, hors caisse (24/09)
// =====================================================================

/// Verse tout ou partie de ce qui reste du sur une fiche validee.
/// Plusieurs versements possibles ; pas plus que ce qui reste du.
/// **Hors caisse** (decision du proprietaire, 24/09) : la paie ne passe
/// pas par le tiroir du jour, le moyen dit d'ou vient l'argent.
pub fn verser_sur(base: &mut Base, fiche_id: String, montant: i64, moyen: Option<String>) -> Result<serde_json::Value, String> {
    let f = lire_sur(base, &fiche_id)?;
    match f["statut"].as_str() {
        Some("validee") => {}
        Some("brouillon") => return Err("Un brouillon ne se paie pas : le valider d'abord.".to_string()),
        _ => {
            return Err(format!(
                "Cette fiche a été remplacée par {} : verser sur celle-ci.",
                f["remplacee_par"].as_str().unwrap_or("sa rectificative")
            ))
        }
    }
    let nom = f["nom"].as_str().unwrap_or_default().to_string();
    let numero = f["numero"].as_str().unwrap_or_default().to_string();
    let net = f["net"].as_i64().unwrap_or(0);
    regles::verifier_versement(montant, f["reste"].as_i64().unwrap_or(0), &nom)?;
    let moyen = moyen.map(|m| m.trim().to_string()).filter(|m| !m.is_empty()).unwrap_or_else(|| "especes".into());
    crate::coeur::saisie::verifier_mode_encaissement(&moyen)?;
    let auteur = crate::argent::id_utilisateur_courant_sur(base);
    let dossier = base.dossier().to_string();
    let now = crate::utils::maintenant_iso();
    let id = uuid::Uuid::new_v4().to_string();
    let mut tx = base.transaction().map_err(|e| e.0)?;
    tx.executer(
        "INSERT INTO versement_paie (id, dossier_id, fiche_id, montant, moyen, date_versement, mouvement_caisse_id, cree_par, cree_le)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7, ?6)",
        &parametres![id.clone(), dossier, fiche_id.clone(), montant, moyen.clone(), now, auteur.clone()],
    )
    .map_err(|e| e.0)?;
    // Deux versements en meme temps ne paient pas plus que ce qui reste.
    let verse = somme_sql(&mut tx, VERSE, &fiche_id)?;
    let repris = somme_sql(&mut tx, REPRIS, &fiche_id)?;
    if regles::reste_a_verser(net, verse, repris) < 0 {
        return Err("Un autre versement vient d'être fait sur cette fiche : relire avant de verser.".to_string());
    }
    noter(&mut tx, "paie_versee", "fiche_paie", &fiche_id, &auteur, serde_json::json!({ "nom": nom, "numero": numero, "montant": montant, "moyen": moyen }))?;
    tx.valider().map_err(|e| e.0)?;
    lire_sur(base, &fiche_id)
}

/// Les versements d'une fiche, les plus anciens d'abord.
fn versements(base: &mut Base, fiche_id: &str) -> Result<Vec<serde_json::Value>, String> {
    let dossier = base.dossier().to_string();
    base.lire_plusieurs(
        "SELECT v.id, v.montant, v.moyen, v.date_versement, u.nom
         FROM versement_paie v LEFT JOIN utilisateur u ON u.id = v.cree_par
         WHERE v.fiche_id = ?1 AND v.dossier_id = ?2 ORDER BY v.date_versement",
        &parametres![fiche_id, dossier],
        |r| {
            Ok(serde_json::json!({
                "id": r.get::<String>(0)?,
                "montant": r.get::<i64>(1)?,
                "moyen": r.get::<String>(2)?,
                "date": r.get::<String>(3)?,
                "par": r.get::<Option<String>>(4)?,
            }))
        },
    )
    .map_err(|e| e.0)
}

/// Tout ce que le bulletin imprime : la fiche, ses lignes, ses
/// versements, ce que l'on sait de la personne, la societe.
pub fn donnees_bulletin_sur(base: &mut Base, fiche_id: String) -> Result<serde_json::Value, String> {
    let mut f = lire_sur(base, &fiche_id)?;
    if f["statut"] == "brouillon" {
        return Err("Un brouillon ne s'imprime pas : le bulletin est le papier d'une fiche validée.".to_string());
    }
    let dossier = base.dossier().to_string();
    let employe_id = f["employe_id"].as_str().unwrap_or_default().to_string();
    let personne = base
        .lire_une(
            "SELECT telephone, date_entree, declare, numero_inps, contrat_ecrit FROM employe WHERE id = ?1 AND dossier_id = ?2",
            &parametres![employe_id, dossier],
            |r| {
                Ok(serde_json::json!({
                    "telephone": r.get::<Option<String>>(0)?,
                    "date_entree": r.get::<Option<String>>(1)?,
                    "declare": r.get::<i64>(2)? != 0,
                    "numero_inps": r.get::<Option<String>>(3)?,
                    "contrat_ecrit": r.get::<i64>(4)? != 0,
                }))
            },
        )
        .map_err(|e| e.0)?
        .unwrap_or(serde_json::Value::Null);
    f["personne"] = personne;
    f["societe"] = crate::creances::societe_sur(base);
    Ok(f)
}

// =====================================================================
//  G-4 : les cotisations du dossier (D33) — facultatives, jamais devinees
// =====================================================================

/// Les cotisations reglees du dossier courant, dans l'ordre de saisie.
/// Vide par defaut : tant que personne ne les a remplies, aucune n'est
/// retenue — un taux faux sur un bulletin est pire qu'aucun.
pub fn cotisations_sur(acces: &mut impl Acces) -> Result<Vec<regles::Cotisation>, String> {
    let dossier = acces.dossier().to_string();
    acces
        .lire_plusieurs(
            "SELECT id, libelle, qui, taux, plafond, compte FROM cotisation WHERE dossier_id = ?1 ORDER BY rang, cree_le",
            &parametres![dossier],
            |r| {
                Ok(regles::Cotisation {
                    id: r.get(0)?,
                    libelle: r.get(1)?,
                    qui: r.get(2)?,
                    taux: r.get(3)?,
                    plafond: r.get(4)?,
                    compte: r.get(5)?,
                })
            },
        )
        .map_err(|e| e.0)
}

fn noter_cotisation(acces: &mut impl Acces, type_evenement: &str, c: &regles::Cotisation, auteur: &str) -> Result<(), String> {
    let dossier = acces.dossier().to_string();
    acces
        .executer(
            "INSERT INTO journal
               (id, type_evenement, entite_type, entite_id, auteur_id, nouveau_valeur, origine, date_evenement, dossier_id)
             VALUES (?1, ?2, 'cotisation', ?3, ?4, ?5, 'app', ?6, ?7)",
            &parametres![
                uuid::Uuid::new_v4().to_string(),
                type_evenement,
                c.id.clone(),
                auteur,
                serde_json::to_string(c).unwrap_or_default(),
                crate::utils::maintenant_iso(),
                dossier
            ],
        )
        .map(|_| ())
        .map_err(|e| e.0)
}

/// Ajoute (sans `id`) ou modifie une cotisation. Le compte, s'il est
/// donne, doit etre au plan du dossier. Les fiches deja calculees ne
/// bougent pas : un brouillon se recalcule, une fiche validee reste.
/// La cotisation et sa ligne d'historique : une seule transaction
/// (revue du 28/10, point 10).
pub fn enregistrer_cotisation_sur(base: &mut Base, mut c: regles::Cotisation) -> Result<serde_json::Value, String> {
    c.libelle = c.libelle.trim().to_string();
    c.compte = c.compte.map(|n| n.trim().to_string()).filter(|n| !n.is_empty());
    regles::valider_cotisation(&c)?;
    if let Some(n) = &c.compte {
        if crate::plan_comptable::libelle_sur(base, n)?.is_none() {
            return Err(format!("Le compte {n} n'est pas au plan comptable de ce dossier."));
        }
    }
    let dossier = base.dossier().to_string();
    let auteur = crate::argent::id_utilisateur_courant_sur(base);
    let now = crate::utils::maintenant_iso();
    let mut tx = base.transaction().map_err(|e| e.0)?;
    if c.id.is_empty() {
        c.id = uuid::Uuid::new_v4().to_string();
        let rang = tx
            .lire_une("SELECT CAST(COUNT(*) AS BIGINT) FROM cotisation WHERE dossier_id = ?1", &parametres![dossier.clone()], |r| r.get::<i64>(0))
            .map_err(|e| e.0)?
            .unwrap_or(0);
        tx.executer(
            "INSERT INTO cotisation (id, dossier_id, libelle, qui, taux, plafond, compte, rang, cree_le, modifie_le)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
            &parametres![c.id.clone(), dossier, c.libelle.clone(), c.qui.clone(), c.taux, c.plafond, c.compte.clone(), rang, now],
        )
        .map_err(|e| e.0)?;
    } else {
        let n = tx
            .executer(
                "UPDATE cotisation SET libelle = ?1, qui = ?2, taux = ?3, plafond = ?4, compte = ?5, modifie_le = ?6
                 WHERE id = ?7 AND dossier_id = ?8",
                &parametres![c.libelle.clone(), c.qui.clone(), c.taux, c.plafond, c.compte.clone(), now, c.id.clone(), dossier],
            )
            .map_err(|e| e.0)?;
        if n == 0 {
            return Err("Cotisation introuvable.".to_string());
        }
    }
    noter_cotisation(&mut tx, "cotisation_modifiee", &c, &auteur)?;
    tx.valider().map_err(|e| e.0)?;
    serde_json::to_value(&c).map_err(|e| e.to_string())
}

/// Retire une cotisation du reglage. Les fiches deja validees gardent
/// leurs lignes. Le retrait et sa ligne d'historique : une transaction.
pub fn retirer_cotisation_sur(base: &mut Base, id: String) -> Result<serde_json::Value, String> {
    let c = cotisations_sur(base)?.into_iter().find(|c| c.id == id).ok_or_else(|| "Cotisation introuvable.".to_string())?;
    let dossier = base.dossier().to_string();
    let auteur = crate::argent::id_utilisateur_courant_sur(base);
    let mut tx = base.transaction().map_err(|e| e.0)?;
    let n = tx
        .executer("DELETE FROM cotisation WHERE id = ?1 AND dossier_id = ?2", &parametres![id, dossier])
        .map_err(|e| e.0)?;
    if n != 1 {
        return Err("Cotisation introuvable.".to_string());
    }
    noter_cotisation(&mut tx, "cotisation_retiree", &c, &auteur)?;
    tx.valider().map_err(|e| e.0)?;
    serde_json::to_value(&c).map_err(|e| e.to_string())
}

/// Le cout des salaires par mois (`AAAA-MM`), depuis un mois donne :
/// le brut des fiches validees et leurs charges patronales, rangees au
/// mois de la fin de leur periode (le mois pour lequel on paie). Une
/// fiche remplacee ne compte pas : sa rectificative la remplace. Lu par
/// Rapports -> CA mensuel.
pub fn salaires_par_mois_sur(acces: &mut impl Acces, depuis_mois: &str) -> Result<std::collections::HashMap<String, i64>, String> {
    let dossier = acces.dossier().to_string();
    let lignes = acces
        .lire_plusieurs(
            "SELECT SUBSTR(f.au, 1, 7),
                    CAST(SUM(f.brut + COALESCE((SELECT SUM(l.montant) FROM ligne_paie l
                        WHERE l.fiche_id = f.id AND l.dossier_id = f.dossier_id AND l.genre = ?3), 0)) AS BIGINT)
             FROM fiche_paie f
             WHERE f.dossier_id = ?1 AND f.statut = 'validee' AND SUBSTR(f.au, 1, 7) >= ?2
             GROUP BY SUBSTR(f.au, 1, 7)",
            &parametres![dossier, depuis_mois, regles::CHARGE],
            |r| Ok((r.get::<String>(0)?, r.get::<i64>(1)?)),
        )
        .map_err(|e| e.0)?;
    Ok(lignes.into_iter().collect())
}
