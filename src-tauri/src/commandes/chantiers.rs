//! Chantiers §14 — TVA, dettes fournisseur, irrécouvrable, expiration avoirs.

use tauri::State;
use crate::commandes::ventes::EtatApp;
use crate::utils::maintenant_iso;

// =====================================================================
//  TVA
// =====================================================================

/// Lire le taux TVA de tous les articles.
#[tauri::command]
pub fn lire_taux_tva(etat: State<EtatApp>) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(
        "SELECT a.id, a.nom, a.unite_base,
                COALESCE(a.taux_tva_defaut, 0.0) as taux_tva
         FROM article a WHERE a.actif = 1 ORDER BY a.nom"
    ).map_err(|e| e.to_string())?;
    let x = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "id":         row.get::<_,String>(0)?,
            "nom":        row.get::<_,String>(1)?,
            "unite_base": row.get::<_,String>(2)?,
            "taux_tva":   row.get::<_,f64>(3)?,
        }))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
    Ok(x)
}

/// Sauvegarder le taux TVA d'un article.
#[tauri::command]
pub fn sauvegarder_tva_article(
    etat: State<EtatApp>,
    article_id: String,
    taux_tva: f64,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE article SET taux_tva_defaut = ?1, modifie_le = ?2 WHERE id = ?3",
        rusqlite::params![taux_tva, maintenant_iso(), article_id],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

/// Résumé TVA collectée sur une période.
#[tauri::command]
pub fn lire_resume_tva(
    etat: State<EtatApp>,
    date_debut: String,
    date_fin: String,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let mut stmt = conn.prepare(
        "SELECT lv.taux_tva,
                CAST(SUM(lv.montant_tva) AS INTEGER) as total_tva,
                CAST(SUM(lv.prix_pratique * lv.quantite)
                     - SUM(lv.montant_tva) AS INTEGER) as total_ht
         FROM ligne_vente lv
         JOIN vente v ON v.id = lv.vente_id
         WHERE v.date_vente BETWEEN ?1 AND ?2
           AND v.statut != 'annulee'
           AND lv.taux_tva > 0
         GROUP BY lv.taux_tva
         ORDER BY lv.taux_tva"
    ).map_err(|e| e.to_string())?;

    let par_taux: Vec<serde_json::Value> = stmt.query_map(
        rusqlite::params![date_debut, date_fin],
        |row| Ok(serde_json::json!({
            "taux":      row.get::<_,f64>(0)?,
            "total_tva": row.get::<_,i64>(1)?,
            "total_ht":  row.get::<_,i64>(2)?,
        }))
    ).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    let total_tva: i64 = par_taux.iter()
        .filter_map(|v| v["total_tva"].as_i64())
        .sum();

    Ok(serde_json::json!({
        "par_taux":   par_taux,
        "total_tva":  total_tva,
        "date_debut": date_debut,
        "date_fin":   date_fin,
    }))
}

// =====================================================================
//  DETTES FOURNISSEURS
// =====================================================================

#[tauri::command]
pub fn lire_dettes_fournisseurs(
    etat: State<EtatApp>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let mut stmt = conn.prepare(
        "SELECT f.id, f.nom, f.telephone, f.est_voisin,
                CAST(COALESCE(
                  (SELECT COALESCE(SUM(
                     CASE pc.type_piece
                       WHEN 'facture_fournisseur' THEN lp.montant_ht + lp.montant_tva
                       -- Un AVF deja rembourse en especes ('paye') ne
                       -- reduit pas la dette : la caisse l'a deja fait.
                       WHEN 'avoir_fournisseur'   THEN
                            CASE WHEN pc.statut = 'paye' THEN 0
                                 ELSE -(lp.montant_ht + lp.montant_tva) END
                       ELSE 0 END), 0)
                   FROM piece_commerciale pc
                   JOIN ligne_piece lp ON lp.piece_id = pc.id
                   WHERE pc.tiers_type = 'fournisseur'
                     AND pc.tiers_id = f.id
                     AND pc.type_piece IN ('facture_fournisseur','avoir_fournisseur')
                     AND pc.statut <> 'annule'), 0
                ) AS INTEGER) as total_achats,
                CAST(COALESCE(
                  (SELECT SUM(pf.montant)
                   FROM paiement_fournisseur pf
                   WHERE pf.fournisseur_id = f.id), 0
                ) AS INTEGER) as total_paye
         FROM fournisseur f
         WHERE f.actif = 1
         ORDER BY f.nom"
    ).map_err(|e| e.to_string())?;

    let x = stmt.query_map([], |row| {
        let total_achats: i64 = row.get(4)?;
        let total_paye: i64 = row.get(5)?;
        let dette = (total_achats - total_paye).max(0);
        Ok(serde_json::json!({
            "id":           row.get::<_,String>(0)?,
            "nom":          row.get::<_,String>(1)?,
            "telephone":    row.get::<_,Option<String>>(2)?,
            "est_voisin":   row.get::<_,i64>(3)? != 0,
            "total_achats": total_achats,
            "total_paye":   total_paye,
            "dette":        dette,
        }))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
    Ok(x)
}

#[tauri::command]
pub fn regler_dette_fournisseur(
    etat: State<EtatApp>,
    fournisseur_id: String,
    montant: i64,
    mode: String,
    note: Option<String>,
    // Facture imputee. NULL = reglement global, reparti de la plus
    // ancienne a la plus recente (voir imputer_paiements_fournisseur).
    piece_id: Option<String>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let auteur = crate::commandes::ventes::id_utilisateur_courant_pub(&conn);
    let now = maintenant_iso();

    conn.execute(
        "INSERT INTO paiement_fournisseur
         (id, fournisseur_id, montant, mode, note, auteur_id,
          date_paiement, cree_le, origine, piece_id)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,'app',?9)",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(),
            fournisseur_id, montant, mode, note, auteur, now, now, piece_id
        ],
    ).map_err(|e| e.to_string())?;

    // Recalculer les statuts des factures de ce fournisseur.
    let soldees = imputer_paiements_fournisseur(&conn, &fournisseur_id, &now)?;

    // SORTIE de caisse : le reglement d'une dette sort de l'argent du
    // tiroir. Sans ce mouvement, la cloture affiche un excedent.
    let session: Option<String> = conn.query_row(
        "SELECT id FROM session_caisse WHERE statut = 'ouverte' LIMIT 1",
        [], |r| r.get(0),
    ).ok();
    if let Some(sid) = session {
        conn.execute(
            "INSERT INTO mouvement_caisse
             (id, session_id, sens, moyen, montant, motif,
              operation_id, date_mouvement, cree_le, cree_par, origine)
             VALUES (?1,?2,'sortie',?3,?4,'reglement_fournisseur',?5,?6,?7,?8,'app')",
            rusqlite::params![
                uuid::Uuid::new_v4().to_string(),
                sid, mode, montant, fournisseur_id, now, now, auteur
            ],
        ).ok();
    }

    conn.execute(
        "INSERT INTO journal
         (id, type_evenement, entite_type, entite_id, auteur_id,
          nouveau_valeur, origine, date_evenement)
         VALUES (?1,'paiement_fournisseur','fournisseur',?2,?3,?4,'app',?5)",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(),
            fournisseur_id, auteur,
            format!(r#"{{"montant":{},"mode":"{}"}}"#, montant, mode),
            now
        ],
    ).ok();

    Ok(serde_json::json!({
        "montant":  montant,
        // Numeros des factures passees a "paye" par ce reglement.
        "soldees":  soldees,
    }))
}

/// Recalcule le statut des factures fournisseur d'apres les paiements.
///
/// Deux sources de reglement :
///   - paiements IMPUTES (piece_id renseigne) — affectes a leur facture
///   - paiements GLOBAUX (piece_id NULL) — repartis de la facture la
///     plus ancienne a la plus recente
///
/// Une facture passe a "paye" quand le cumul couvre son total, et
/// revient a "emis" sinon (annulation d'un paiement, avoir ajoute).
/// Retourne les numeros des factures desormais soldees.
fn imputer_paiements_fournisseur(
    conn: &rusqlite::Connection,
    fournisseur_id: &str,
    now: &str,
) -> Result<Vec<String>, String> {
    // Enveloppe globale disponible pour les factures non imputees.
    let mut enveloppe: i64 = conn.query_row(
        "SELECT CAST(COALESCE(SUM(montant), 0) AS INTEGER)
         FROM paiement_fournisseur
         WHERE fournisseur_id = ?1 AND piece_id IS NULL",
        rusqlite::params![fournisseur_id], |r| r.get(0),
    ).unwrap_or(0);

    let mut stmt = conn.prepare(
        "SELECT pc.id, pc.numero, pc.statut,
                CAST(COALESCE((SELECT SUM(lp.montant_ht + lp.montant_tva)
                   FROM ligne_piece lp WHERE lp.piece_id = pc.id), 0) AS INTEGER),
                CAST(COALESCE((SELECT SUM(pf.montant)
                   FROM paiement_fournisseur pf WHERE pf.piece_id = pc.id), 0) AS INTEGER)
         FROM piece_commerciale pc
         WHERE pc.tiers_type = 'fournisseur' AND pc.tiers_id = ?1
           AND pc.type_piece = 'facture_fournisseur'
           AND pc.statut <> 'annule'
         ORDER BY pc.date_piece ASC"
    ).map_err(|e| e.to_string())?;

    let factures: Vec<(String, String, String, i64, i64)> = stmt.query_map(
        rusqlite::params![fournisseur_id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        }
    ).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    let mut soldees = Vec::new();

    for (piece_id, numero, statut_actuel, total, impute) in factures {
        let mut couvert = impute;
        if couvert < total && enveloppe > 0 {
            let pris = enveloppe.min(total - couvert);
            couvert += pris;
            enveloppe -= pris;
        }

        // Meme seuil que le cote client : un residu d'arrondi ne doit
        // pas laisser une FAF eternellement "emis" (D41).
        let nouveau = if total > 0
            && crate::coeur::calcul::reste_exigible(total, couvert) == 0
            { "paye" } else { "emis" };
        if nouveau != statut_actuel {
            conn.execute(
                "UPDATE piece_commerciale SET statut = ?1, modifie_le = ?2
                 WHERE id = ?3",
                rusqlite::params![nouveau, now, piece_id],
            ).map_err(|e| e.to_string())?;
        }
        if nouveau == "paye" {
            soldees.push(numero);
        }
    }

    Ok(soldees)
}

// =====================================================================
//  IRRÉCOUVRABLE
// =====================================================================

#[tauri::command]
pub fn marquer_irrecouvrable(
    etat: State<EtatApp>,
    vente_id: String,
    motif: String,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let auteur = crate::commandes::ventes::id_utilisateur_courant_pub(&conn);
    let now = maintenant_iso();

    let statut: String = conn.query_row(
        "SELECT statut FROM vente WHERE id = ?1",
        rusqlite::params![vente_id],
        |r| r.get(0),
    ).map_err(|_| "Vente introuvable".to_string())?;

    if statut == "payee" {
        return Err("Cette vente est déjà entièrement payée".to_string());
    }
    if statut == "irrecouvrable" {
        return Err("Cette créance est déjà marquée irrécouvrable".to_string());
    }

    conn.execute(
        "UPDATE vente SET statut = 'irrecouvrable', modifie_le = ?1 WHERE id = ?2",
        rusqlite::params![now, vente_id],
    ).map_err(|e| e.to_string())?;

    conn.execute(
        "INSERT INTO creance_irrecouvrable
         (id, vente_id, motif, auteur_id, date_marque, cree_le, origine)
         VALUES (?1,?2,?3,?4,?5,?6,'app')",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(),
            vente_id, motif, auteur, now, now
        ],
    ).map_err(|e| e.to_string())?;

    conn.execute(
        "INSERT INTO journal
         (id, type_evenement, entite_type, entite_id, auteur_id,
          nouveau_valeur, origine, date_evenement)
         VALUES (?1,'creance_irrecouvrable','vente',?2,?3,?4,'app',?5)",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(),
            vente_id, auteur,
            format!(r#"{{"motif":"{}"}}"#, motif),
            now
        ],
    ).ok();
    Ok(())
}

#[tauri::command]
pub fn lire_irrecouvrable(
    etat: State<EtatApp>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let mut stmt = conn.prepare(
        "SELECT ci.id, ci.vente_id, ci.motif, ci.date_marque,
                c.nom as client_nom,
                p.numero as facture_num,
                CAST(COALESCE(
                  (SELECT SUM(prix_pratique * quantite)
                   FROM ligne_vente WHERE vente_id = ci.vente_id), 0
                ) AS INTEGER) as total,
                CAST(COALESCE(
                  (SELECT SUM(montant) FROM paiement WHERE vente_id = ci.vente_id), 0
                ) AS INTEGER) as total_paye
         FROM creance_irrecouvrable ci
         JOIN vente v ON v.id = ci.vente_id
         JOIN client c ON c.id = v.client_id
         LEFT JOIN piece_commerciale p ON p.id = v.piece_id
         ORDER BY ci.date_marque DESC"
    ).map_err(|e| e.to_string())?;

    let x = stmt.query_map([], |row| {
        let total: i64 = row.get(6)?;
        let paye: i64 = row.get(7)?;
        Ok(serde_json::json!({
            "id":            row.get::<_,String>(0)?,
            "vente_id":      row.get::<_,String>(1)?,
            "motif":         row.get::<_,String>(2)?,
            "date_marque":   row.get::<_,String>(3)?,
            "client_nom":    row.get::<_,String>(4)?,
            "facture_num":   row.get::<_,Option<String>>(5)?,
            "montant_perdu": total - paye,
        }))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
    Ok(x)
}

// =====================================================================
//  EXPIRATION AVOIRS
// =====================================================================

#[tauri::command]
pub fn lire_config_avoirs(
    etat: State<EtatApp>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let active: Option<String> = conn.query_row(
        "SELECT valeur FROM config_app WHERE cle = 'avoirs_expiration_active'",
        [], |r| r.get(0),
    ).ok();

    let duree: Option<String> = conn.query_row(
        "SELECT valeur FROM config_app WHERE cle = 'avoirs_expiration_jours'",
        [], |r| r.get(0),
    ).ok();

    Ok(serde_json::json!({
        "active":      active.as_deref() == Some("1"),
        "duree_jours": duree.and_then(|d| d.parse::<i64>().ok()).unwrap_or(90),
    }))
}

#[tauri::command]
pub fn sauvegarder_config_avoirs(
    etat: State<EtatApp>,
    active: bool,
    duree_jours: i64,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    if duree_jours < 30 {
        return Err("La durée minimale est de 30 jours".to_string());
    }

    conn.execute(
        "INSERT INTO config_app (cle, valeur) VALUES ('avoirs_expiration_active', ?1)
         ON CONFLICT(cle) DO UPDATE SET valeur = ?1",
        rusqlite::params![if active { "1" } else { "0" }],
    ).map_err(|e| e.to_string())?;

    conn.execute(
        "INSERT INTO config_app (cle, valeur) VALUES ('avoirs_expiration_jours', ?1)
         ON CONFLICT(cle) DO UPDATE SET valeur = ?1",
        rusqlite::params![duree_jours.to_string()],
    ).map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub fn expirer_avoirs(etat: State<EtatApp>) -> Result<i64, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let active: Option<String> = conn.query_row(
        "SELECT valeur FROM config_app WHERE cle = 'avoirs_expiration_active'",
        [], |r| r.get(0),
    ).ok();

    if active.as_deref() != Some("1") {
        return Ok(0);
    }

    let duree: i64 = conn.query_row(
        "SELECT CAST(valeur AS INTEGER) FROM config_app
         WHERE cle = 'avoirs_expiration_jours'",
        [], |r| r.get(0),
    ).unwrap_or(90);

    let nb = conn.execute(
        "UPDATE avoir SET statut = 'expire'
         WHERE statut = 'ouvert'
           AND (julianday('now') - julianday(cree_le)) > ?1",
        rusqlite::params![duree],
    ).map_err(|e| e.to_string())?;

    if nb > 0 {
        let auteur = crate::commandes::ventes::id_utilisateur_courant_pub(&conn);
        conn.execute(
            "INSERT INTO journal
             (id, type_evenement, entite_type, entite_id, auteur_id,
              nouveau_valeur, origine, date_evenement)
             VALUES (?1,'expiration_avoirs','avoir','batch',?2,?3,'app',?4)",
            rusqlite::params![
                uuid::Uuid::new_v4().to_string(), auteur,
                format!(r#"{{"nb_expires":{},"duree_jours":{}}}"#, nb, duree),
                maintenant_iso()
            ],
        ).ok();
    }

    Ok(nb as i64)
}

/// Factures fournisseur non soldees, avec leur reste du.
///
/// Sert a l'ecran de reglement : on impute sur une facture precise au
/// lieu d'un montant global qui ne dit pas ce qu'il paie.
#[tauri::command]
pub fn lire_factures_fournisseur_ouvertes(
    etat: State<EtatApp>,
    fournisseur_id: String,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let mut stmt = conn.prepare(
        "SELECT pc.id, pc.numero, pc.date_piece, pc.statut,
                CAST(COALESCE((SELECT SUM(lp.montant_ht + lp.montant_tva)
                   FROM ligne_piece lp WHERE lp.piece_id = pc.id), 0) AS INTEGER) as total,
                CAST(COALESCE((SELECT SUM(pf.montant)
                   FROM paiement_fournisseur pf WHERE pf.piece_id = pc.id), 0) AS INTEGER) as paye
         FROM piece_commerciale pc
         WHERE pc.tiers_type = 'fournisseur' AND pc.tiers_id = ?1
           AND pc.type_piece = 'facture_fournisseur'
           AND pc.statut NOT IN ('annule','paye')
         ORDER BY pc.date_piece ASC"
    ).map_err(|e| e.to_string())?;

    let x = stmt.query_map(rusqlite::params![fournisseur_id], |r| {
        let total: i64 = r.get(4)?;
        let paye: i64 = r.get(5)?;
        Ok(serde_json::json!({
            "piece_id":   r.get::<_,String>(0)?,
            "numero":     r.get::<_,String>(1)?,
            "date_piece": r.get::<_,String>(2)?,
            "statut":     r.get::<_,String>(3)?,
            "total":      total,
            "paye":       paye,
            "reste":      crate::coeur::calcul::reste_exigible(total, paye),
        }))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    Ok(x)
}