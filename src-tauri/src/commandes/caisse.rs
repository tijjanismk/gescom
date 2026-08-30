//! Commandes Tauri pour la caisse.
//!
//! ⚠️ Le rapprochement ne porte que sur les ESPÈCES. Orange Money, Moov
//! Money et les chèques sont tracés en mouvement de caisse mais ne sont
//! pas dans le tiroir physique : les inclure dans le solde théorique
//! affiche un manque systématique à chaque clôture.

use tauri::State;
use crate::commandes::ventes::EtatApp;
use crate::utils::maintenant_iso;

// =====================================================================
//  RÉSUMÉ CAISSE
// =====================================================================

#[tauri::command]
pub fn lire_resume_caisse(
    etat: State<EtatApp>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    // Chercher une session ouverte.
    let session: Option<(String, i64, String)> = conn.query_row(
        "SELECT id, fond_ouverture, cree_le FROM session_caisse
         WHERE statut = 'ouverte' ORDER BY cree_le DESC LIMIT 1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).ok();

    match session {
        None => {
            // Vérifier s'il y a déjà eu des sessions.
            let nb: i64 = conn.query_row(
                "SELECT COUNT(*) FROM session_caisse", [], |r| r.get(0)
            ).unwrap_or(0);

            Ok(serde_json::json!({
                "session_id":       null,
                "statut":           if nb > 0 { "fermee" } else { "aucune" },
                "fond_ouverture":   0,
                "total_entrees":    0,
                "total_sorties":    0,
                "entrees_especes":  0,
                "sorties_especes":  0,
                "solde_theorique":  0,
                "nb_transactions":  0,
                "ouvert_le":        null,
            }))
        }
        Some((session_id, fond_ouverture, ouvert_le)) => {
            // Tous moyens confondus — pour les indicateurs d'activité.
            let total_entrees: i64 = conn.query_row(
                "SELECT CAST(COALESCE(SUM(montant), 0) AS INTEGER)
                 FROM mouvement_caisse
                 WHERE session_id = ?1 AND sens = 'entree'",
                rusqlite::params![session_id],
                |r| r.get(0),
            ).unwrap_or(0);

            let total_sorties: i64 = conn.query_row(
                "SELECT CAST(COALESCE(SUM(montant), 0) AS INTEGER)
                 FROM mouvement_caisse
                 WHERE session_id = ?1 AND sens = 'sortie'",
                rusqlite::params![session_id],
                |r| r.get(0),
            ).unwrap_or(0);

            // Espèces seules — pour le rapprochement du tiroir.
            let entrees_especes: i64 = conn.query_row(
                "SELECT CAST(COALESCE(SUM(montant), 0) AS INTEGER)
                 FROM mouvement_caisse
                 WHERE session_id = ?1 AND sens = 'entree' AND moyen = 'especes'",
                rusqlite::params![session_id],
                |r| r.get(0),
            ).unwrap_or(0);

            let sorties_especes: i64 = conn.query_row(
                "SELECT CAST(COALESCE(SUM(montant), 0) AS INTEGER)
                 FROM mouvement_caisse
                 WHERE session_id = ?1 AND sens = 'sortie' AND moyen = 'especes'",
                rusqlite::params![session_id],
                |r| r.get(0),
            ).unwrap_or(0);

            let nb_transactions: i64 = conn.query_row(
                "SELECT COUNT(*) FROM mouvement_caisse WHERE session_id = ?1",
                rusqlite::params![session_id],
                |r| r.get(0),
            ).unwrap_or(0);

            let solde_theorique = crate::coeur::caisse::solde_theorique(
                fond_ouverture, entrees_especes, sorties_especes
            );

            Ok(serde_json::json!({
                "session_id":      session_id,
                "statut":          "ouverte",
                "fond_ouverture":  fond_ouverture,
                "total_entrees":   total_entrees,
                "total_sorties":   total_sorties,
                "entrees_especes": entrees_especes,
                "sorties_especes": sorties_especes,
                "solde_theorique": solde_theorique,
                "nb_transactions": nb_transactions,
                "ouvert_le":       ouvert_le,
            }))
        }
    }
}

// =====================================================================
//  MOUVEMENTS DU JOUR
// =====================================================================

#[tauri::command]
pub fn lire_mouvements_caisse_du_jour(
    etat: State<EtatApp>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let mut stmt = conn.prepare(
        "SELECT mc.id, mc.sens, mc.moyen, mc.montant, mc.motif,
                mc.date_mouvement, mc.libelle
         FROM mouvement_caisse mc
         JOIN session_caisse sc ON sc.id = mc.session_id
         WHERE DATE(mc.date_mouvement) = DATE('now', 'localtime')
         ORDER BY mc.date_mouvement ASC"
    ).map_err(|e| e.to_string())?;

    let x = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "id":             row.get::<_, String>(0)?,
            "sens":           row.get::<_, String>(1)?,
            "moyen":          row.get::<_, String>(2)?,
            "montant":        row.get::<_, i64>(3)?,
            "motif":          row.get::<_, String>(4)?,
            "date_mouvement": row.get::<_, String>(5)?,
            "libelle":        row.get::<_, Option<String>>(6)?,
        }))
    })
    .map_err(|e| e.to_string())?
    .filter_map(|r| r.ok())
    .collect();
    Ok(x)
}

// =====================================================================
//  OUVERTURE DE SESSION
// =====================================================================

#[tauri::command]
pub fn ouvrir_session_caisse(
    etat: State<EtatApp>,
    fond_ouverture: i64,
    utilisateur_role: String,
) -> Result<String, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let utilisateur_id = crate::commandes::ventes::id_utilisateur_par_role(
        &conn, &utilisateur_role
    );
    let maintenant = maintenant_iso();

    // Vérifier qu'il n'y a pas déjà une session ouverte.
    let session_ouverte: i64 = conn.query_row(
        "SELECT COUNT(*) FROM session_caisse WHERE statut = 'ouverte'",
        [], |r| r.get(0),
    ).unwrap_or(0);

    if session_ouverte > 0 {
        return Err("Une session est déjà ouverte".to_string());
    }

    let session_id = uuid::Uuid::new_v4().to_string();

    conn.execute(
        "INSERT INTO session_caisse
         (id, statut, fond_ouverture, ouvert_par, cree_le, modifie_le, origine)
         VALUES (?1, 'ouverte', ?2, ?3, ?4, ?5, 'app')",
        rusqlite::params![
            session_id, fond_ouverture, utilisateur_id,
            maintenant, maintenant
        ],
    ).map_err(|e| e.to_string())?;

    // Mouvement d'ouverture si fond > 0.
    if fond_ouverture > 0 {
        conn.execute(
            "INSERT INTO mouvement_caisse
             (id, session_id, sens, moyen, montant, motif,
              date_mouvement, cree_le, cree_par, origine)
             VALUES (?1, ?2, 'entree', 'especes', ?3, 'ouverture', ?4, ?5, ?6, 'app')",
            rusqlite::params![
                uuid::Uuid::new_v4().to_string(),
                session_id, fond_ouverture,
                maintenant, maintenant, utilisateur_id
            ],
        ).map_err(|e| e.to_string())?;
    }

    Ok(session_id)
}

// =====================================================================
//  FERMETURE DE SESSION
// =====================================================================

#[tauri::command]
pub fn fermer_session_caisse(
    etat: State<EtatApp>,
    session_id: String,
    especes_comptees: i64,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let maintenant = maintenant_iso();

    let fond: i64 = conn.query_row(
        "SELECT fond_ouverture FROM session_caisse WHERE id = ?1",
        rusqlite::params![session_id], |r| r.get(0),
    ).map_err(|e| e.to_string())?;

    // ⚠️ ESPÈCES UNIQUEMENT. Sans ce filtre, l'écart intègre l'Orange
    // Money et la caisse paraît en déficit à chaque clôture.
    let entrees: i64 = conn.query_row(
        "SELECT CAST(COALESCE(SUM(montant), 0) AS INTEGER)
         FROM mouvement_caisse
         WHERE session_id = ?1 AND sens = 'entree' AND moyen = 'especes'",
        rusqlite::params![session_id], |r| r.get(0),
    ).unwrap_or(0);

    let sorties: i64 = conn.query_row(
        "SELECT CAST(COALESCE(SUM(montant), 0) AS INTEGER)
         FROM mouvement_caisse
         WHERE session_id = ?1 AND sens = 'sortie' AND moyen = 'especes'",
        rusqlite::params![session_id], |r| r.get(0),
    ).unwrap_or(0);

    let solde_theorique = crate::coeur::caisse::solde_theorique(fond, entrees, sorties);
    let ecart = crate::coeur::caisse::ecart_caisse(especes_comptees, solde_theorique);

    conn.execute(
        "UPDATE session_caisse SET
           statut = 'fermee',
           solde_theorique = ?1,
           especes_comptees = ?2,
           ecart = ?3,
           ferme_le = ?4,
           modifie_le = ?5
         WHERE id = ?6",
        rusqlite::params![
            solde_theorique, especes_comptees, ecart,
            maintenant, maintenant, session_id
        ],
    ).map_err(|e| e.to_string())?;

    Ok(())
}


// =====================================================================
//  DÉPENSES
// =====================================================================
//
//  Une dépense est une sortie de caisse qui n'est ni un achat ni un
//  règlement fournisseur : loyer, transport, carburant, salaire
//  d'appoint. Elle n'a pas de tiers ni de document commercial.
//
//  Elle exige une session ouverte : sans cela le montant sortirait du
//  tiroir sans jamais entrer dans un rapprochement.

#[tauri::command]
pub fn enregistrer_depense(
    etat: State<EtatApp>,
    montant: i64,
    libelle: String,
    // transport | loyer | salaire | carburant | electricite | eau
    // | fourniture | entretien | taxe | autre
    categorie: Option<String>,
    // especes | orange_money | moov_money | cheque
    moyen: Option<String>,
    utilisateur_role: Option<String>,
) -> Result<String, String> {
    if montant <= 0 {
        return Err("Le montant doit être positif".to_string());
    }
    if libelle.trim().is_empty() {
        return Err("Le libellé est obligatoire".to_string());
    }

    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let maintenant = maintenant_iso();
    let role = utilisateur_role.as_deref().unwrap_or("employe");
    let auteur = crate::commandes::ventes::id_utilisateur_par_role(&conn, role);

    let session_id: String = conn.query_row(
        "SELECT id FROM session_caisse WHERE statut = 'ouverte' LIMIT 1",
        [], |r| r.get(0),
    ).map_err(|_| {
        "Aucune session de caisse ouverte — ouvrir la caisse d'abord."
            .to_string()
    })?;

    let id = uuid::Uuid::new_v4().to_string();

    conn.execute(
        "INSERT INTO mouvement_caisse
         (id, session_id, sens, moyen, montant, motif, libelle, categorie,
          date_mouvement, cree_le, cree_par, origine)
         VALUES (?1,?2,'sortie',?3,?4,'depense',?5,?6,?7,?8,?9,'app')",
        rusqlite::params![
            id, session_id,
            moyen.as_deref().unwrap_or("especes"),
            montant, libelle.trim(),
            categorie.as_deref().unwrap_or("autre"),
            maintenant, maintenant, auteur
        ],
    ).map_err(|e| e.to_string())?;

    conn.execute(
        "INSERT INTO journal
         (id, type_evenement, entite_type, entite_id, auteur_id,
          nouveau_valeur, origine, date_evenement)
         VALUES (?1,'depense','mouvement_caisse',?2,?3,?4,'app',?5)",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(), id, auteur,
            format!(r#"{{"montant":{},"libelle":"{}"}}"#,
                    montant, libelle.trim().replace('"', "'")),
            maintenant
        ],
    ).ok();

    Ok(id)
}

/// Dépenses de la journée, avec leur total.
#[tauri::command]
pub fn lire_depenses_du_jour(
    etat: State<EtatApp>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let mut stmt = conn.prepare(
        "SELECT mc.id, mc.libelle, mc.moyen, mc.montant, mc.date_mouvement,
                u.nom, COALESCE(mc.categorie, 'autre')
         FROM mouvement_caisse mc
         LEFT JOIN utilisateur u ON u.id = mc.cree_par
         WHERE mc.motif = 'depense'
           AND DATE(mc.date_mouvement) = DATE('now', 'localtime')
         ORDER BY mc.date_mouvement DESC"
    ).map_err(|e| e.to_string())?;

    let lignes: Vec<serde_json::Value> = stmt.query_map([], |r| {
        Ok(serde_json::json!({
            "id":             r.get::<_, String>(0)?,
            "libelle":        r.get::<_, Option<String>>(1)?,
            "moyen":          r.get::<_, String>(2)?,
            "montant":        r.get::<_, i64>(3)?,
            "date_mouvement": r.get::<_, String>(4)?,
            "auteur_nom":     r.get::<_, Option<String>>(5)?,
            "categorie":      r.get::<_, String>(6)?,
        }))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    let total: i64 = lignes.iter()
        .filter_map(|l| l["montant"].as_i64()).sum();

    // Ventilation par poste, pour le journal.
    let mut par_categorie: std::collections::BTreeMap<String, i64> =
        std::collections::BTreeMap::new();
    for l in &lignes {
        let c = l["categorie"].as_str().unwrap_or("autre").to_string();
        *par_categorie.entry(c).or_insert(0) += l["montant"].as_i64().unwrap_or(0);
    }
    let ventilation: Vec<serde_json::Value> = par_categorie.into_iter()
        .map(|(c, m)| serde_json::json!({ "categorie": c, "montant": m }))
        .collect();

    Ok(serde_json::json!({
        "depenses":      lignes,
        "total":         total,
        "par_categorie": ventilation,
    }))
}

// =====================================================================
//  HISTORIQUE DES SESSIONS
// =====================================================================
//
//  L'ecart de cloture est la seule mesure qui confronte le logiciel au
//  monde reel. Un ecart isole ne dit rien ; c'est la SUITE des ecarts
//  qui parle :
//    - toujours nul          -> les saisies sont completes
//    - manque regulier       -> une sortie d'argent n'est jamais saisie
//    - excedent              -> une vente echappe au systeme
//    - manque brutal, isole  -> la question du vol se pose

#[tauri::command]
pub fn lire_sessions_caisse(
    etat: State<EtatApp>,
    limite: Option<i64>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let lim = limite.unwrap_or(60);

    let mut st = conn.prepare(
        "SELECT sc.id, sc.statut, sc.fond_ouverture, sc.cree_le, sc.ferme_le,
                sc.solde_theorique, sc.especes_comptees, sc.ecart,
                COALESCE(uo.nom, '—'),
                CAST(COALESCE((SELECT SUM(mc.montant) FROM mouvement_caisse mc
                   WHERE mc.session_id = sc.id AND mc.sens = 'entree'
                     AND mc.moyen = 'especes' AND mc.motif <> 'ouverture'), 0) AS INTEGER),
                CAST(COALESCE((SELECT SUM(mc.montant) FROM mouvement_caisse mc
                   WHERE mc.session_id = sc.id AND mc.sens = 'sortie'
                     AND mc.moyen = 'especes'), 0) AS INTEGER),
                (SELECT COUNT(*) FROM mouvement_caisse mc WHERE mc.session_id = sc.id)
         FROM session_caisse sc
         LEFT JOIN utilisateur uo ON uo.id = sc.ouvert_par
         ORDER BY sc.cree_le DESC
         LIMIT ?1"
    ).map_err(|e| e.to_string())?;

    let x = st.query_map(rusqlite::params![lim], |r| {
        Ok(serde_json::json!({
            "id":               r.get::<_, String>(0)?,
            "statut":           r.get::<_, String>(1)?,
            "fond_ouverture":   r.get::<_, i64>(2)?,
            "ouvert_le":        r.get::<_, String>(3)?,
            "ferme_le":         r.get::<_, Option<String>>(4)?,
            "solde_theorique":  r.get::<_, Option<i64>>(5)?,
            "especes_comptees": r.get::<_, Option<i64>>(6)?,
            "ecart":            r.get::<_, Option<i64>>(7)?,
            "ouvert_par":       r.get::<_, String>(8)?,
            "entrees_especes":  r.get::<_, i64>(9)?,
            "sorties_especes":  r.get::<_, i64>(10)?,
            "nb_mouvements":    r.get::<_, i64>(11)?,
        }))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    Ok(x)
}

/// Mouvements d'une session precise — pour comprendre un ecart.
#[tauri::command]
pub fn lire_mouvements_session(
    etat: State<EtatApp>,
    session_id: String,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let mut st = conn.prepare(
        "SELECT mc.id, mc.sens, mc.moyen, mc.montant, mc.motif,
                COALESCE(mc.libelle, ''), COALESCE(mc.categorie, ''),
                mc.date_mouvement
         FROM mouvement_caisse mc
         WHERE mc.session_id = ?1
         ORDER BY mc.date_mouvement"
    ).map_err(|e| e.to_string())?;

    let x = st.query_map(rusqlite::params![session_id], |r| {
        Ok(serde_json::json!({
            "id":             r.get::<_, String>(0)?,
            "sens":           r.get::<_, String>(1)?,
            "moyen":          r.get::<_, String>(2)?,
            "montant":        r.get::<_, i64>(3)?,
            "motif":          r.get::<_, String>(4)?,
            "libelle":        r.get::<_, String>(5)?,
            "categorie":      r.get::<_, String>(6)?,
            "date_mouvement": r.get::<_, String>(7)?,
        }))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    Ok(x)
}

/// Synthese des ecarts sur une periode — c'est la tendance qui compte.
#[tauri::command]
pub fn lire_rapport_ecarts(
    etat: State<EtatApp>,
    jours: Option<i64>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let n = jours.unwrap_or(30);

    let (nb, nuls, manques, excedents, cumul, pire): (i64, i64, i64, i64, i64, i64) =
        conn.query_row(
            "SELECT COUNT(*),
                    SUM(CASE WHEN ecart = 0 THEN 1 ELSE 0 END),
                    SUM(CASE WHEN ecart < 0 THEN 1 ELSE 0 END),
                    SUM(CASE WHEN ecart > 0 THEN 1 ELSE 0 END),
                    CAST(COALESCE(SUM(ecart), 0) AS INTEGER),
                    CAST(COALESCE(MIN(ecart), 0) AS INTEGER)
             FROM session_caisse
             WHERE statut = 'fermee' AND ecart IS NOT NULL
               AND DATE(ferme_le) >= DATE('now', 'localtime', '-' || ?1 || ' days')",
            rusqlite::params![n],
            |r| Ok((r.get(0)?, r.get(1).unwrap_or(0), r.get(2).unwrap_or(0),
                    r.get(3).unwrap_or(0), r.get(4)?, r.get(5)?)),
        ).unwrap_or((0, 0, 0, 0, 0, 0));

    // Interpretation, pour que le patron sache quoi en faire.
    let diagnostic = if nb == 0 {
        "Aucune clôture sur la période."
    } else if nuls == nb {
        "Toutes les caisses tombent juste. Les saisies sont complètes."
    } else if manques > nb / 2 && cumul < 0 {
        "Manques réguliers : une sortie d'argent n'est probablement          jamais saisie (transport, monnaie prêtée, petites dépenses)."
    } else if excedents > nb / 2 {
        "Excédents réguliers : des ventes échappent au système.          Le stock devient faux et les clients n'ont pas de facture."
    } else {
        "Écarts irréguliers : erreurs de monnaie rendue ou saisies          approximatives."
    };

    Ok(serde_json::json!({
        "jours": n, "nb_clotures": nb,
        "nuls": nuls, "manques": manques, "excedents": excedents,
        "cumul": cumul, "pire_manque": pire,
        "moyenne": if nb > 0 { cumul / nb } else { 0 },
        "diagnostic": diagnostic,
    }))
}

/// Corriger une depense mal saisie. Uniquement sur la session OUVERTE :
/// une session close a ete rapprochee, la modifier invaliderait l'ecart
/// deja constate.
#[tauri::command]
pub fn modifier_depense(
    etat: State<EtatApp>,
    mouvement_id: String,
    montant: Option<i64>,
    libelle: Option<String>,
    categorie: Option<String>,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let (motif, statut): (String, String) = conn.query_row(
        "SELECT mc.motif, sc.statut FROM mouvement_caisse mc
         JOIN session_caisse sc ON sc.id = mc.session_id
         WHERE mc.id = ?1",
        rusqlite::params![mouvement_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    ).map_err(|_| "Mouvement introuvable".to_string())?;

    if motif != "depense" {
        return Err(
            "Seule une dépense se corrige. Les mouvements liés à une              vente, un achat ou un retour suivent leur document."
                .to_string()
        );
    }
    if statut != "ouverte" {
        return Err(
            "Cette session est clôturée : l'écart a déjà été constaté.              Saisir une dépense corrective sur la session du jour."
                .to_string()
        );
    }

    if let Some(m) = montant {
        if m <= 0 { return Err("Le montant doit être positif".to_string()); }
        conn.execute("UPDATE mouvement_caisse SET montant = ?1 WHERE id = ?2",
            rusqlite::params![m, mouvement_id]).map_err(|e| e.to_string())?;
    }
    if let Some(l) = libelle {
        conn.execute("UPDATE mouvement_caisse SET libelle = ?1 WHERE id = ?2",
            rusqlite::params![l.trim(), mouvement_id]).map_err(|e| e.to_string())?;
    }
    if let Some(c) = categorie {
        conn.execute("UPDATE mouvement_caisse SET categorie = ?1 WHERE id = ?2",
            rusqlite::params![c, mouvement_id]).map_err(|e| e.to_string())?;
    }
    Ok(())
}
