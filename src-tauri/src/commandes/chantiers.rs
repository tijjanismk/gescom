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
                CAST(SUM(lv.prix_pratique * lv.quantite) AS INTEGER) as total_ht
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
                  (SELECT SUM(CAST(ms.quantite_delta AS REAL) *
                              COALESCE(a.dernier_prix_achat, 0))
                   FROM mouvement_stock ms
                   JOIN article a ON a.id = ms.article_id
                   WHERE ms.type_mouvement = 'achat'
                     AND ms.quantite_delta > 0
                   ), 0
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
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let auteur = crate::commandes::ventes::id_utilisateur_courant_pub(&conn);
    let now = maintenant_iso();

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS paiement_fournisseur (
            id TEXT PRIMARY KEY, fournisseur_id TEXT NOT NULL,
            montant INTEGER NOT NULL, mode TEXT NOT NULL DEFAULT 'especes',
            note TEXT, auteur_id TEXT, date_paiement TEXT NOT NULL,
            cree_le TEXT NOT NULL, origine TEXT NOT NULL DEFAULT 'app'
        )"
    ).map_err(|e| e.to_string())?;

    conn.execute(
        "INSERT INTO paiement_fournisseur
         (id, fournisseur_id, montant, mode, note, auteur_id, date_paiement, cree_le, origine)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,'app')",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(),
            fournisseur_id, montant, mode, note, auteur, now, now
        ],
    ).map_err(|e| e.to_string())?;

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
    Ok(())
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

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS creance_irrecouvrable (
            id TEXT PRIMARY KEY, vente_id TEXT NOT NULL,
            motif TEXT NOT NULL, auteur_id TEXT,
            date_marque TEXT NOT NULL, cree_le TEXT NOT NULL,
            origine TEXT NOT NULL DEFAULT 'app'
        )"
    ).map_err(|e| e.to_string())?;

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

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS creance_irrecouvrable (
            id TEXT PRIMARY KEY, vente_id TEXT, motif TEXT,
            auteur_id TEXT, date_marque TEXT, cree_le TEXT,
            origine TEXT DEFAULT 'app'
        )"
    ).ok();

    let mut stmt = conn.prepare(
        "SELECT ci.id, ci.vente_id, ci.motif, ci.date_marque,
                c.nom as client_nom,
                f.numero as facture_num,
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
         LEFT JOIN facture f ON f.vente_id = ci.vente_id
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
