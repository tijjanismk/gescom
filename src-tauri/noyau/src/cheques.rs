//! Chèques reçus — suivi de l'encaissement.
//!
//! Un paiement par chèque n'est pas de l'argent : c'est une promesse.
//! Le commerçant doit savoir lesquels sont déposés, encaissés, rejetés,
//! et lesquels dorment dans un tiroir depuis trois semaines.
//!
//! Le chèque N'ENTRE PAS dans le rapprochement de caisse — comme le
//! mobile money, il n'est pas dans le tiroir (cf. D29). Il figure
//! seulement dans les totaux par moyen.
//!
//! Cycle :
//!   recu -> depose -> encaisse
//!               \\-> rejete  (le paiement est annulé, la créance rouvre)

use crate::utils::maintenant_iso;

/// Enregistre un chèque, rattaché au paiement qui l'a créé.
pub fn enregistrer_cheque(
    conn: &rusqlite::Connection,
    paiement_id: Option<String>,
    vente_id: Option<String>,
    numero: String,
    banque: String,
    tireur: Option<String>,
    montant: i64,
    date_emission: Option<String>,
    date_echeance: Option<String>,
) -> Result<String, String> {
    if numero.trim().is_empty() {
        return Err("Le numéro du chèque est obligatoire".to_string());
    }
    if banque.trim().is_empty() {
        return Err("La banque est obligatoire".to_string());
    }
    if montant <= 0 {
        return Err("Le montant doit être positif".to_string());
    }
    let now = maintenant_iso();
    let id = uuid::Uuid::new_v4().to_string();

    // Un meme numero peut revenir d'une banque a l'autre : c'est le
    // couple numero + banque qui identifie le cheque.
    let doublon: Option<String> = conn.query_row(
        "SELECT id FROM cheque_recu
         WHERE numero = ?1 AND banque = ?2 AND statut <> 'rejete'",
        rusqlite::params![numero.trim(), banque.trim()], |r| r.get(0),
    ).ok();
    if doublon.is_some() {
        return Err(format!(
            "Le chèque n° {} de {} est déjà enregistré.",
            numero.trim(), banque.trim()
        ));
    }

    conn.execute(
        "INSERT INTO cheque_recu
         (id, paiement_id, vente_id, numero, banque, tireur, montant,
          date_emission, date_echeance, statut, cree_le, modifie_le, origine)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,'recu',?10,?11,'app')",
        rusqlite::params![
            id, paiement_id, vente_id, numero.trim(), banque.trim(),
            tireur, montant, date_emission, date_echeance, now, now
        ],
    ).map_err(|e| e.to_string())?;

    Ok(id)
}

/// Chèques, filtrables par statut.
pub fn lire_cheques(
    conn: &rusqlite::Connection,
    statut: Option<String>,
) -> Result<serde_json::Value, String> {

    let mut st = conn.prepare(
        "SELECT ch.id, ch.numero, ch.banque, COALESCE(ch.tireur,''),
                ch.montant, ch.date_emission, ch.date_echeance, ch.statut,
                ch.cree_le, COALESCE(c.nom, '—'),
                CAST(julianday('now','localtime') - julianday(ch.cree_le) AS INTEGER)
         FROM cheque_recu ch
         LEFT JOIN vente v ON v.id = ch.vente_id
         LEFT JOIN client c ON c.id = v.client_id
         WHERE (?1 IS NULL OR ch.statut = ?1)
         ORDER BY
           CASE ch.statut WHEN 'recu' THEN 0 WHEN 'depose' THEN 1 ELSE 2 END,
           COALESCE(ch.date_echeance, ch.cree_le)"
    ).map_err(|e| e.to_string())?;

    let dep: Option<String> = match statut {
        Some(s) if !s.is_empty() && s != "tous" => Some(s),
        _ => None,
    };

    let cheques: Vec<serde_json::Value> = st.query_map(
        rusqlite::params![dep], |r| {
            Ok(serde_json::json!({
                "id":            r.get::<_, String>(0)?,
                "numero":        r.get::<_, String>(1)?,
                "banque":        r.get::<_, String>(2)?,
                "tireur":        r.get::<_, String>(3)?,
                "montant":       r.get::<_, i64>(4)?,
                "date_emission": r.get::<_, Option<String>>(5)?,
                "date_echeance": r.get::<_, Option<String>>(6)?,
                "statut":        r.get::<_, String>(7)?,
                "cree_le":       r.get::<_, String>(8)?,
                "client":        r.get::<_, String>(9)?,
                "jours_detention": r.get::<_, i64>(10)?,
            }))
        }
    ).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    let en_attente: i64 = conn.query_row(
        "SELECT CAST(COALESCE(SUM(montant),0) AS INTEGER) FROM cheque_recu
         WHERE statut IN ('recu','depose')",
        [], |r| r.get(0),
    ).unwrap_or(0);

    let nb_dormants: i64 = conn.query_row(
        "SELECT COUNT(*) FROM cheque_recu
         WHERE statut = 'recu'
           AND julianday('now','localtime') - julianday(cree_le) > 15",
        [], |r| r.get(0),
    ).unwrap_or(0);

    Ok(serde_json::json!({
        "cheques": cheques,
        "total_en_attente": en_attente,
        // Un cheque garde plus de 15 jours perd sa valeur de recours.
        "nb_dormants": nb_dormants,
    }))
}

/// Fait avancer un chèque dans son cycle.
///
/// Un rejet ANNULE le paiement : la créance du client se rouvre. C'est
/// le seul cas où de l'argent déjà compté disparaît.
pub fn changer_statut_cheque(
    conn: &mut rusqlite::Connection,
    cheque_id: String,
    statut: String,
    motif: Option<String>,
) -> Result<serde_json::Value, String> {
    if !matches!(statut.as_str(), "recu" | "depose" | "encaisse" | "rejete") {
        return Err(format!("Statut inconnu : {}", statut));
    }
    let now = maintenant_iso();

    let (ancien, paiement_id, vente_id, montant):
        (String, Option<String>, Option<String>, i64) =
        conn.query_row(
            "SELECT statut, paiement_id, vente_id, montant
             FROM cheque_recu WHERE id = ?1",
            rusqlite::params![cheque_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        ).map_err(|_| "Chèque introuvable".to_string())?;

    if ancien == "encaisse" {
        return Err(
            "Ce chèque est encaissé : l'argent est sur le compte. \
             Pour le corriger, saisir un mouvement de caisse.".to_string()
        );
    }

    let tx = conn.transaction().map_err(|e| e.to_string())?;

    tx.execute(
        "UPDATE cheque_recu
         SET statut = ?1, motif_rejet = ?2, modifie_le = ?3 WHERE id = ?4",
        rusqlite::params![statut, motif, now, cheque_id],
    ).map_err(|e| e.to_string())?;

    let mut creance_rouverte = false;

    if statut == "rejete" {
        // Le paiement n'a jamais eu lieu : on le supprime et on
        // recalcule le statut de la vente.
        if let Some(pid) = &paiement_id {
            tx.execute("DELETE FROM paiement WHERE id = ?1",
                rusqlite::params![pid]).map_err(|e| e.to_string())?;
        }

        if let Some(vid) = &vente_id {
            let total: i64 = tx.query_row(
                "SELECT CAST(COALESCE(SUM(prix_pratique*quantite),0) AS INTEGER)
                 FROM ligne_vente WHERE vente_id = ?1",
                rusqlite::params![vid], |r| r.get(0),
            ).unwrap_or(0);
            let paye: i64 = tx.query_row(
                "SELECT CAST(COALESCE(SUM(montant),0) AS INTEGER)
                 FROM paiement WHERE vente_id = ?1",
                rusqlite::params![vid], |r| r.get(0),
            ).unwrap_or(0);

            let st = match crate::coeur::calcul::statut_vente(total, paye) {
                crate::coeur::calcul::StatutVente::Payee => "payee",
                crate::coeur::calcul::StatutVente::PartiellementPayee
                    => "partiellement_payee",
                crate::coeur::calcul::StatutVente::CreanceOuverte
                    => "creance_ouverte",
            };
            tx.execute(
                "UPDATE vente SET statut = ?1, modifie_le = ?2 WHERE id = ?3",
                rusqlite::params![st, now, vid],
            ).map_err(|e| e.to_string())?;

            // La piece liee redevient 'emis' si elle etait soldee.
            tx.execute(
                "UPDATE piece_commerciale SET statut = 'emis', modifie_le = ?1
                 WHERE id = (SELECT piece_id FROM vente WHERE id = ?2)
                   AND statut = 'paye'",
                rusqlite::params![now, vid],
            ).ok();

            creance_rouverte = st != "payee";
        }
    }

    tx.execute(
        "INSERT INTO journal
         (id, type_evenement, entite_type, entite_id, auteur_id,
          ancien_valeur, nouveau_valeur, origine, date_evenement)
         VALUES (?1,'cheque_statut','cheque_recu',?2,NULL,?3,?4,'app',?5)",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(), cheque_id,
            ancien, statut, now
        ],
    ).ok();

    tx.commit().map_err(|e| e.to_string())?;

    Ok(serde_json::json!({
        "statut": statut,
        "montant": montant,
        "creance_rouverte": creance_rouverte,
    }))
}

// =====================================================================
//  SUR L'UN OU L'AUTRE MOTEUR
// =====================================================================
//
// `cheque_recu` n'est pas cloisonnee ; `vente`, `paiement`, `journal`
// le sont. L'age d'un cheque se calcule en Rust (`utils::jours_depuis`)
// : `julianday` n'existe pas sur PostgreSQL.

use crate::base::Base;
use crate::parametres;

#[allow(clippy::too_many_arguments)]
pub fn enregistrer_cheque_sur_base(
    base: &mut Base,
    paiement_id: Option<String>,
    vente_id: Option<String>,
    numero: String,
    banque: String,
    tireur: Option<String>,
    montant: i64,
    date_emission: Option<String>,
    date_echeance: Option<String>,
) -> Result<String, String> {
    if numero.trim().is_empty() {
        return Err("Le numéro du chèque est obligatoire".to_string());
    }
    if banque.trim().is_empty() {
        return Err("La banque est obligatoire".to_string());
    }
    if montant <= 0 {
        return Err("Le montant doit être positif".to_string());
    }
    let now = maintenant_iso();
    let id = uuid::Uuid::new_v4().to_string();

    let doublon: Option<String> = base
        .lire_une(
            "SELECT id FROM cheque_recu
             WHERE numero = ?1 AND banque = ?2 AND statut <> 'rejete'",
            &parametres![numero.trim(), banque.trim()],
            |r| r.get::<String>(0),
        )
        .map_err(|e| e.0)?;
    if doublon.is_some() {
        return Err(format!(
            "Le chèque n° {} de {} est déjà enregistré.",
            numero.trim(),
            banque.trim()
        ));
    }

    base.executer(
        "INSERT INTO cheque_recu
         (id, paiement_id, vente_id, numero, banque, tireur, montant,
          date_emission, date_echeance, statut, cree_le, modifie_le, origine)
         VALUES (?1,CAST(?2 AS TEXT),CAST(?3 AS TEXT),?4,?5,CAST(?6 AS TEXT),?7,
                 CAST(?8 AS TEXT),CAST(?9 AS TEXT),'recu',?10,?10,'app')",
        &parametres![
            id.clone(), paiement_id, vente_id, numero.trim(), banque.trim(),
            tireur, montant, date_emission, date_echeance, now
        ],
    )
    .map_err(|e| e.0)?;
    Ok(id)
}

pub fn lire_cheques_sur_base(base: &mut Base, statut: Option<String>) -> Result<serde_json::Value, String> {
    let dossier = base.dossier().to_string();
    let dep: Option<String> = match statut {
        Some(s) if !s.is_empty() && s != "tous" => Some(s),
        _ => None,
    };

    let cheques: Vec<serde_json::Value> = base
        .lire_plusieurs(
            "SELECT ch.id, ch.numero, ch.banque, COALESCE(ch.tireur,''),
                    ch.montant, ch.date_emission, ch.date_echeance, ch.statut,
                    ch.cree_le, COALESCE(c.nom, '—')
             FROM cheque_recu ch
             LEFT JOIN vente v ON v.id = ch.vente_id AND v.dossier_id = ?2
             LEFT JOIN client c ON c.id = v.client_id
             WHERE (CAST(?1 AS TEXT) IS NULL OR ch.statut = ?1)
             ORDER BY
               CASE ch.statut WHEN 'recu' THEN 0 WHEN 'depose' THEN 1 ELSE 2 END,
               COALESCE(ch.date_echeance, ch.cree_le)",
            &parametres![dep, dossier],
            |r| {
                let cree_le: String = r.get::<String>(8)?;
                Ok(serde_json::json!({
                    "id":            r.get::<String>(0)?,
                    "numero":        r.get::<String>(1)?,
                    "banque":        r.get::<String>(2)?,
                    "tireur":        r.get::<String>(3)?,
                    "montant":       r.get::<i64>(4)?,
                    "date_emission": r.get::<Option<String>>(5)?,
                    "date_echeance": r.get::<Option<String>>(6)?,
                    "statut":        r.get::<String>(7)?,
                    "jours_detention": crate::utils::jours_depuis(&cree_le),
                    "cree_le":       cree_le,
                    "client":        r.get::<String>(9)?,
                }))
            },
        )
        .map_err(|e| e.0)?;

    let en_attente: i64 = base
        .lire_une(
            "SELECT CAST(COALESCE(SUM(montant),0) AS BIGINT) FROM cheque_recu
             WHERE statut IN ('recu','depose')",
            &[],
            |r| r.get::<i64>(0),
        )
        .map_err(|e| e.0)?
        .unwrap_or(0);

    // Un cheque garde plus de 15 jours perd sa valeur de recours.
    let nb_dormants = cheques
        .iter()
        .filter(|c| c["statut"] == "recu" && c["jours_detention"].as_i64().unwrap_or(0) > 15)
        .count();

    Ok(serde_json::json!({
        "cheques": cheques,
        "total_en_attente": en_attente,
        "nb_dormants": nb_dormants,
    }))
}

pub fn changer_statut_cheque_sur_base(
    base: &mut Base,
    cheque_id: String,
    statut: String,
    motif: Option<String>,
) -> Result<serde_json::Value, String> {
    if !matches!(statut.as_str(), "recu" | "depose" | "encaisse" | "rejete") {
        return Err(format!("Statut inconnu : {}", statut));
    }
    let dossier = base.dossier().to_string();
    let now = maintenant_iso();

    let (ancien, paiement_id, vente_id, montant): (String, Option<String>, Option<String>, i64) = base
        .lire_une(
            "SELECT statut, paiement_id, vente_id, montant FROM cheque_recu WHERE id = ?1",
            &parametres![cheque_id.clone()],
            |r| Ok((r.get::<String>(0)?, r.get::<Option<String>>(1)?, r.get::<Option<String>>(2)?, r.get::<i64>(3)?)),
        )
        .map_err(|e| e.0)?
        .ok_or_else(|| "Chèque introuvable".to_string())?;

    if ancien == "encaisse" {
        return Err(
            "Ce chèque est encaissé : l'argent est sur le compte. \
             Pour le corriger, saisir un mouvement de caisse."
                .to_string(),
        );
    }

    let mut tx = base.transaction().map_err(|e| e.0)?;
    tx.executer(
        "UPDATE cheque_recu SET statut = ?1, motif_rejet = CAST(?2 AS TEXT), modifie_le = ?3 WHERE id = ?4",
        &parametres![statut.clone(), motif, now.clone(), cheque_id.clone()],
    )
    .map_err(|e| e.0)?;

    let mut creance_rouverte = false;
    if statut == "rejete" {
        // Le paiement n'a jamais eu lieu : on le supprime et on
        // recalcule le statut de la vente.
        if let Some(pid) = &paiement_id {
            tx.executer(
                "DELETE FROM paiement WHERE id = ?1 AND dossier_id = ?2",
                &parametres![pid.clone(), dossier.clone()],
            )
            .map_err(|e| e.0)?;
        }
        if let Some(vid) = &vente_id {
            let total: i64 = tx
                .lire_une(
                    "SELECT CAST(COALESCE(SUM(prix_pratique*quantite),0) AS BIGINT)
                     FROM ligne_vente WHERE vente_id = ?1 AND dossier_id = ?2",
                    &parametres![vid.clone(), dossier.clone()],
                    |r| r.get::<i64>(0),
                )
                .map_err(|e| e.0)?
                .unwrap_or(0);
            let paye: i64 = tx
                .lire_une(
                    "SELECT CAST(COALESCE(SUM(montant),0) AS BIGINT)
                     FROM paiement WHERE vente_id = ?1 AND dossier_id = ?2",
                    &parametres![vid.clone(), dossier.clone()],
                    |r| r.get::<i64>(0),
                )
                .map_err(|e| e.0)?
                .unwrap_or(0);
            let st = match crate::coeur::calcul::statut_vente(total, paye) {
                crate::coeur::calcul::StatutVente::Payee => "payee",
                crate::coeur::calcul::StatutVente::PartiellementPayee => "partiellement_payee",
                crate::coeur::calcul::StatutVente::CreanceOuverte => "creance_ouverte",
            };
            tx.executer(
                "UPDATE vente SET statut = ?1, modifie_le = ?2 WHERE id = ?3 AND dossier_id = ?4",
                &parametres![st, now.clone(), vid.clone(), dossier.clone()],
            )
            .map_err(|e| e.0)?;
            // La piece liee redevient 'emis' si elle etait soldee.
            let _ = tx.executer(
                "UPDATE piece_commerciale SET statut = 'emis', modifie_le = ?1
                 WHERE id = (SELECT piece_id FROM vente WHERE id = ?2)
                   AND statut = 'paye' AND dossier_id = ?3",
                &parametres![now.clone(), vid.clone(), dossier.clone()],
            );
            creance_rouverte = st != "payee";
        }
    }

    let _ = tx.executer(
        "INSERT INTO journal
         (id, type_evenement, entite_type, entite_id, auteur_id,
          ancien_valeur, nouveau_valeur, origine, date_evenement, dossier_id)
         VALUES (?1,'cheque_statut','cheque_recu',?2,NULL,?3,?4,'app',?5,?6)",
        &parametres![uuid::Uuid::new_v4().to_string(), cheque_id, ancien, statut.clone(), now, dossier],
    );
    tx.valider().map_err(|e| e.0)?;

    Ok(serde_json::json!({
        "statut": statut,
        "montant": montant,
        "creance_rouverte": creance_rouverte,
    }))
}
