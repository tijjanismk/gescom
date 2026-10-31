//! D50 — les clients a relancer (PLAN-EQUIPE, EQ-4), en base. Version
//! `Base` seule (D29). Rien ne s'ecrit ici sauf le reglage N : la liste
//! se lit, et le clic de l'ecran passe par les commandes du CRM
//! (`crm::creer_echange_sur`, `crm::creer_rappel_sur`).
//!
//! Les ages se comptent en Rust (`utils::jours_depuis`), pas en SQL
//! (`julianday` n'existe pas sur PostgreSQL) ; la regle est dans
//! `coeur::crm_relances`.

use std::collections::HashMap;

use crate::base::{Acces, Base};
use crate::coeur::crm_relances as regles;
use crate::parametres;
use crate::utils::jours_depuis;

fn cle(dossier: &str) -> String {
    format!("crm_jours_dormant:{dossier}")
}

/// N du dossier : pas d'achat depuis N jours (60 d'office).
pub fn jours_dormant_sur(acces: &mut impl Acces) -> i64 {
    let dossier = acces.dossier().to_string();
    acces
        .lire_une("SELECT valeur FROM config_app WHERE cle = ?1", &parametres![cle(&dossier)], |r| r.get::<String>(0))
        .ok()
        .flatten()
        .and_then(|v| v.trim().parse::<i64>().ok())
        .and_then(|j| regles::valider_jours_dormant(j).ok())
        .unwrap_or(regles::JOURS_DORMANT_DEFAUT)
}

pub fn definir_jours_dormant_sur(base: &mut Base, jours: i64) -> Result<serde_json::Value, String> {
    let jours = regles::valider_jours_dormant(jours)?;
    let dossier = base.dossier().to_string();
    base.executer(
        "INSERT INTO config_app (cle, valeur) VALUES (?1, ?2)
         ON CONFLICT (cle) DO UPDATE SET valeur = excluded.valeur",
        &parametres![cle(&dossier), jours.to_string()],
    )
    .map_err(|e| e.0)?;
    Ok(serde_json::json!({ "jours_dormant": jours }))
}

/// Le plus recent de deux ages.
fn plus_recent(a: Option<i64>, b: i64) -> Option<i64> {
    Some(a.map_or(b, |x| x.min(b)))
}

/// La liste a relancer du dossier. `avec_creances` : faux pour qui ne
/// lit pas les soldes — `coeur::lecture` le neutralise (D50, point 8).
pub fn lire_clients_a_relancer_sur(base: &mut Base, avec_creances: bool) -> Result<serde_json::Value, String> {
    let dossier = base.dossier().to_string();
    let jours_dormant = jours_dormant_sur(base);

    // Les clients nommes et actifs : ni prospect, ni fiche de passage,
    // ni « client divers ».
    // Les partis (D50, point 13) en sont : ils restent pour leur creance.
    let clients: Vec<(String, String, String, Option<String>, Option<String>, Option<String>)> = base
        .lire_plusieurs(
            "SELECT id, code, nom, telephone, parti_le, raison_sortie FROM client
             WHERE dossier_id = ?1 AND est_generique = 0 AND actif = 1 AND statut = 'client'",
            &parametres![dossier.clone()],
            |r| {
                Ok((
                    r.get::<String>(0)?, r.get::<String>(1)?, r.get::<String>(2)?,
                    r.get::<Option<String>>(3)?, r.get::<Option<String>>(4)?, r.get::<Option<String>>(5)?,
                ))
            },
        )
        .map_err(|e| e.0)?;
    let mut releves: Vec<regles::Releve> = Vec::with_capacity(clients.len());
    let mut index: HashMap<String, usize> = HashMap::new();
    let mut fiches: HashMap<String, (String, Option<String>, Option<String>)> = HashMap::new();
    let mut partis: Vec<serde_json::Value> = Vec::new();
    let mut par_raison: std::collections::BTreeMap<String, i64> = std::collections::BTreeMap::new();
    for (id, code, nom, telephone, parti_le, raison) in clients {
        if let Some(le) = &parti_le {
            let raison = raison.unwrap_or_else(|| "autre".to_string());
            *par_raison.entry(raison.clone()).or_default() += 1;
            partis.push(serde_json::json!({
                "client_id": id, "code": code, "nom": nom, "parti_le": le, "raison": raison,
            }));
        }
        index.insert(id.clone(), releves.len());
        fiches.insert(id.clone(), (code, telephone, None));
        releves.push(regles::Releve { id, nom, parti: parti_le.is_some(), ..Default::default() });
    }
    partis.sort_by(|a, b| b["parti_le"].as_str().cmp(&a["parti_le"].as_str()));
    // Les reconquis du mois (point 14) : revenus par une vente ce mois.
    let mois = crate::utils::maintenant_iso()[..7].to_string();
    let reconquis_mois: i64 = base
        .lire_une(
            "SELECT COUNT(*) FROM client WHERE dossier_id = ?1 AND SUBSTR(revenu_le, 1, 7) = ?2",
            &parametres![dossier.clone(), mois],
            |r| r.get::<i64>(0),
        )
        .map_err(|e| e.0)?
        .unwrap_or(0);

    // Le dernier achat de chacun.
    let derniers: Vec<(String, String)> = base
        .lire_plusieurs(
            "SELECT client_id, MAX(date_vente) FROM vente
             WHERE dossier_id = ?1 AND statut <> 'annulee'
             GROUP BY client_id",
            &parametres![dossier.clone()],
            |r| Ok((r.get::<String>(0)?, r.get::<String>(1)?)),
        )
        .map_err(|e| e.0)?;
    for (client_id, date) in derniers {
        if let Some(&i) = index.get(&client_id) {
            releves[i].jours_dernier_achat = Some(jours_depuis(&date));
            if let Some(f) = fiches.get_mut(&client_id) {
                f.2 = Some(date);
            }
        }
    }

    // Les achats de la reference et du recent. La borne se tire de
    // l'age du jour : `maintenant_iso` moins 360 jours, compare par
    // `SUBSTR(…, 1, 10)` ; l'age exact se recompte en Rust.
    let borne = crate::coeur::dates::jour(&crate::utils::maintenant_iso())
        .map(|j| (j - chrono::Duration::days(regles::JOURS_REFERENCE_FIN)).format("%Y-%m-%d").to_string())
        .unwrap_or_default();
    let achats: Vec<(String, String, i64)> = base
        .lire_plusieurs(
            "SELECT v.client_id, v.date_vente,
                    CAST(COALESCE(SUM(lv.prix_pratique * lv.quantite), 0) AS BIGINT)
             FROM vente v LEFT JOIN ligne_vente lv ON lv.vente_id = v.id AND lv.dossier_id = v.dossier_id
             WHERE v.dossier_id = ?1 AND v.statut <> 'annulee'
               AND SUBSTR(v.date_vente, 1, 10) >= ?2
             GROUP BY v.id, v.client_id, v.date_vente",
            &parametres![dossier.clone(), borne],
            |r| Ok((r.get::<String>(0)?, r.get::<String>(1)?, r.get::<i64>(2)?)),
        )
        .map_err(|e| e.0)?;
    for (client_id, date, montant) in achats {
        if let Some(&i) = index.get(&client_id) {
            releves[i].achats.push((jours_depuis(&date), montant));
        }
    }

    // Les ventes non soldees — seulement pour qui lit les soldes : sans
    // le droit, elles ne sont meme pas lues.
    if avec_creances {
        let creances: Vec<(String, String, i64, i64)> = base
            .lire_plusieurs(
                "SELECT v.client_id, v.date_vente,
                        CAST(COALESCE((SELECT SUM(prix_pratique * quantite) FROM ligne_vente WHERE vente_id = v.id AND dossier_id = v.dossier_id), 0) AS BIGINT),
                        CAST(COALESCE((SELECT SUM(montant) FROM paiement WHERE vente_id = v.id AND dossier_id = v.dossier_id), 0) AS BIGINT)
                 FROM vente v
                 WHERE v.dossier_id = ?1 AND v.statut IN ('creance_ouverte', 'partiellement_payee')",
                &parametres![dossier.clone()],
                |r| Ok((r.get::<String>(0)?, r.get::<String>(1)?, r.get::<i64>(2)?, r.get::<i64>(3)?)),
            )
            .map_err(|e| e.0)?;
        for (client_id, date, total, paye) in creances {
            if let Some(&i) = index.get(&client_id) {
                releves[i].creances.push((jours_depuis(&date), total - paye));
            }
        }
    }

    // Le dernier contact : un echange, ou une relance de creance (D34 :
    // un echange comme un autre).
    let echanges: Vec<(String, String)> = base
        .lire_plusieurs(
            "SELECT client_id, MAX(date_echange) FROM echange WHERE dossier_id = ?1 GROUP BY client_id",
            &parametres![dossier.clone()],
            |r| Ok((r.get::<String>(0)?, r.get::<String>(1)?)),
        )
        .map_err(|e| e.0)?;
    let relances: Vec<(String, String)> = base
        .lire_plusieurs(
            "SELECT v.client_id, MAX(rc.date_relance)
             FROM relance_creance rc JOIN vente v ON v.id = rc.vente_id AND v.dossier_id = rc.dossier_id
             WHERE rc.dossier_id = ?1
             GROUP BY v.client_id",
            &parametres![dossier.clone()],
            |r| Ok((r.get::<String>(0)?, r.get::<String>(1)?)),
        )
        .map_err(|e| e.0)?;
    for (client_id, date) in echanges.into_iter().chain(relances) {
        if let Some(&i) = index.get(&client_id) {
            releves[i].jours_dernier_contact = plus_recent(releves[i].jours_dernier_contact, jours_depuis(&date));
        }
    }
    let rappels: Vec<String> = base
        .lire_plusieurs(
            "SELECT DISTINCT client_id FROM rappel WHERE dossier_id = ?1 AND fait = 0",
            &parametres![dossier],
            |r| r.get::<String>(0),
        )
        .map_err(|e| e.0)?;
    for client_id in rappels {
        if let Some(&i) = index.get(&client_id) {
            releves[i].rappel_actif = true;
        }
    }

    let verdicts = regles::juger(&releves, jours_dormant, avec_creances);
    let mis_de_cote = verdicts.iter().filter(|v| v.mis_de_cote).count();
    let lignes: Vec<serde_json::Value> = verdicts
        .iter()
        .filter(|v| !v.mis_de_cote)
        .map(|v| {
            let r = &releves[index[&v.id]];
            let (code, telephone, dernier_achat) = fiches.get(&v.id).cloned().unwrap_or_default();
            serde_json::json!({
                "client_id": v.id,
                "code": code,
                "nom": r.nom,
                "telephone": telephone,
                "parti": r.parti,
                "motifs": v.motifs,
                "dernier_achat": dernier_achat,
                "jours_sans_achat": r.jours_dernier_achat,
                "nb_achats_reference": v.nb_achats_reference,
                "ca_reference": v.ca_reference,
                "ca_recent": v.ca_recent,
                "reste_en_retard": if avec_creances { serde_json::json!(v.reste_en_retard) } else { serde_json::Value::Null },
                "nb_creances_en_retard": v.nb_creances_en_retard,
                "jours_plus_vieille_creance": v.jours_plus_vieille_creance,
                "texte_propose": regles::texte_propose(v, r.jours_dernier_achat),
            })
        })
        .collect();

    Ok(serde_json::json!({
        "jours_dormant": jours_dormant,
        "avec_creances": avec_creances,
        "lignes": lignes,
        "mis_de_cote": mis_de_cote,
        // D50, point 16 : aucun montant ici.
        "partis": {
            "nombre": partis.len(),
            "par_raison": par_raison.into_iter().map(|(raison, nombre)| serde_json::json!({ "raison": raison, "nombre": nombre })).collect::<Vec<_>>(),
            "reconquis_mois": reconquis_mois,
            "liste": partis,
        },
    }))
}
