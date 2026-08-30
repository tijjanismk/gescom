//! Fournisseurs — liste, création, stock, fiche détail.

use tauri::State;
use crate::commandes::ventes::EtatApp;
use crate::utils::maintenant_iso;

// =====================================================================
//  Liste fournisseurs
// =====================================================================

#[tauri::command]
pub fn lire_fournisseurs(
    etat: State<EtatApp>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(
        "SELECT id, nom, telephone, adresse, est_voisin
         FROM fournisseur ORDER BY nom"
    ).map_err(|e| e.to_string())?;

    let x = stmt.query_map([], |r| {
        Ok(serde_json::json!({
            "id":        r.get::<_,String>(0)?,
            "nom":       r.get::<_,String>(1)?,
            "telephone": r.get::<_,Option<String>>(2)?,
            "adresse":   r.get::<_,Option<String>>(3)?,
            "est_voisin":r.get::<_,i64>(4)? != 0,
        }))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
    Ok(x)
}

// =====================================================================
//  Fournisseurs avec dettes (pour Chantiers)
// =====================================================================

#[tauri::command]
pub fn lire_fournisseurs_avec_dettes(
    etat: State<EtatApp>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let mut stmt = conn.prepare(
        "SELECT f.id, f.nom, f.telephone,
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
                     AND pc.statut <> 'annule')
                , 0) AS INTEGER) as total_achats,
                CAST(COALESCE(
                  (SELECT SUM(pf.montant) FROM paiement_fournisseur pf
                   WHERE pf.fournisseur_id = f.id)
                , 0) AS INTEGER) as total_paye
         FROM fournisseur f
         ORDER BY f.nom"
    ).map_err(|e| e.to_string())?;

    let x = stmt.query_map([], |r| {
        let total_achats: i64 = r.get(3)?;
        let total_paye: i64 = r.get(4)?;
        let dette = (total_achats - total_paye).max(0);
        Ok(serde_json::json!({
            "id":          r.get::<_,String>(0)?,
            "nom":         r.get::<_,String>(1)?,
            "telephone":   r.get::<_,Option<String>>(2)?,
            "total_achats":total_achats,
            "total_paye":  total_paye,
            "dette":       dette,
        }))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
    Ok(x)
}

// =====================================================================
//  Créer fournisseur
// =====================================================================

#[tauri::command]
pub fn creer_fournisseur(
    etat: State<EtatApp>,
    nom: String,
    telephone: Option<String>,
    adresse: Option<String>,
    nif: Option<String>,
    email: Option<String>,
    est_voisin: Option<bool>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let id = uuid::Uuid::new_v4().to_string();
    let now = maintenant_iso();
    let voisin = est_voisin.unwrap_or(false) as i64;

    conn.execute(
        "INSERT INTO fournisseur
         (id, nom, telephone, adresse, nif, email, est_voisin, cree_le, modifie_le)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        rusqlite::params![id, nom, telephone, adresse, nif, email, voisin, now, now],
    ).map_err(|e| e.to_string())?;

    Ok(serde_json::json!({"id": id, "nom": nom}))
}

// =====================================================================
//  Enregistrer entrée stock (achat)
// =====================================================================

#[tauri::command]
pub fn enregistrer_entree_stock(
    etat: State<EtatApp>,
    article_id: String,
    depot_id: Option<String>,  // null → dépôt par défaut
    quantite: f64,
    prix_achat: Option<i64>,
    fournisseur_id: Option<String>,
    utilisateur_role: Option<String>,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let now = maintenant_iso();
    let role = utilisateur_role.as_deref().unwrap_or("employe");
    let auteur = crate::commandes::ventes::id_utilisateur_par_role(&conn, role);

    // Résoudre le dépôt — utiliser le défaut si null
    let depot_id = match depot_id {
        Some(d) if !d.is_empty() => d,
        _ => conn.query_row(
            "SELECT id FROM depot WHERE est_defaut = 1 LIMIT 1",
            [], |r| r.get(0)
        ).map_err(|_| "Aucun dépôt par défaut configuré".to_string())?,
    };

    let op_id = uuid::Uuid::new_v4().to_string();

    // Mettre à jour le stock
    conn.execute(
        "INSERT INTO stock_depot (id, article_id, depot_id, quantite)
         VALUES (?1,?2,?3,?4)
         ON CONFLICT(article_id, depot_id)
         DO UPDATE SET quantite = quantite + ?4",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(), article_id, depot_id, quantite
        ],
    ).map_err(|e| e.to_string())?;

    // Mouvement stock
    conn.execute(
        "INSERT INTO mouvement_stock
         (id, article_id, depot_id, type_mouvement, quantite_delta,
          operation_id, auteur_id, date_mouvement, cree_le, cree_par, origine,
          fournisseur_id, prix_achat_unitaire)
         VALUES (?1,?2,?3,'achat',?4,?5,?6,?7,?8,?9,'app',?10,?11)",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(),
            article_id, depot_id, quantite,
            op_id, auteur, now, now, auteur,
            fournisseur_id, prix_achat
        ],
    ).map_err(|e| e.to_string())?;

    // Mettre à jour le dernier prix d'achat
    if let Some(px) = prix_achat {
        conn.execute(
            "UPDATE article SET dernier_prix_achat = ?1 WHERE id = ?2",
            rusqlite::params![px, article_id],
        ).ok();
    }

    Ok(())
}

// =====================================================================
//  Ajustement inventaire
// =====================================================================

#[tauri::command]
pub fn enregistrer_ajustement_inventaire(
    etat: State<EtatApp>,
    article_id: String,
    depot_id: String,
    // `quantite_reelle` et non `nouvelle_quantite` : c'est ce qui a ete
    // COMPTE physiquement dans le depot. Le front envoyait deja ce nom,
    // d'ou l'erreur « invalid args ».
    quantite_reelle: f64,
    motif: Option<String>,
    utilisateur_role: Option<String>,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let _ = motif; // réservé pour journal des ajustements v1.1

    let quantite_actuelle: f64 = conn.query_row(
        "SELECT COALESCE(quantite, 0) FROM stock_depot
         WHERE article_id = ?1 AND depot_id = ?2",
        rusqlite::params![article_id, depot_id],
        |r| r.get(0),
    ).unwrap_or(0.0);

    let delta = quantite_reelle - quantite_actuelle;
    let now = maintenant_iso();
    let role = utilisateur_role.as_deref().unwrap_or("employe");
    let auteur = crate::commandes::ventes::id_utilisateur_par_role(&conn, role);
    let op_id = uuid::Uuid::new_v4().to_string();

    conn.execute(
        "INSERT INTO stock_depot (id, article_id, depot_id, quantite)
         VALUES (?1,?2,?3,?4)
         ON CONFLICT(article_id, depot_id)
         DO UPDATE SET quantite = ?4",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(), article_id, depot_id, quantite_reelle
        ],
    ).map_err(|e| e.to_string())?;

    conn.execute(
        "INSERT INTO mouvement_stock
         (id, article_id, depot_id, type_mouvement, quantite_delta,
          operation_id, auteur_id, date_mouvement, cree_le, cree_par, origine)
         VALUES (?1,?2,?3,'ajustement',?4,?5,?6,?7,?8,?9,'app')",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(),
            article_id, depot_id, delta,
            op_id, auteur, now, now, auteur
        ],
    ).map_err(|e| e.to_string())?;

    Ok(())
}

// =====================================================================
//  Fiche fournisseur — détail
// =====================================================================

#[tauri::command]
pub fn lire_fournisseur_detail(
    etat: State<EtatApp>,
    fournisseur_id: String,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let f = conn.query_row(
        "SELECT id, nom, telephone, adresse, nif, email, est_voisin, cree_le
         FROM fournisseur WHERE id = ?1",
        rusqlite::params![fournisseur_id],
        |r| Ok(serde_json::json!({
            "id":        r.get::<_,String>(0)?,
            "nom":       r.get::<_,String>(1)?,
            "telephone": r.get::<_,Option<String>>(2)?,
            "adresse":   r.get::<_,Option<String>>(3)?,
            "nif":       r.get::<_,Option<String>>(4)?,
            "email":     r.get::<_,Option<String>>(5)?,
            "est_voisin":r.get::<_,i64>(6)? != 0,
            "cree_le":   r.get::<_,String>(7)?,
        }))
    ).map_err(|e| e.to_string())?;
    Ok(f)
}

// =====================================================================
//  Fiche fournisseur — stats, paiements, achats
// =====================================================================

#[tauri::command]
pub fn lire_fiche_fournisseur(
    etat: State<EtatApp>,
    fournisseur_id: String,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let (_qte, nb_achats, derniere_cmd): (f64, i64, Option<String>) =
        conn.query_row(
            "SELECT COALESCE(SUM(quantite_delta), 0), COUNT(*), MAX(date_mouvement)
             FROM mouvement_stock
             WHERE type_mouvement = 'achat' AND quantite_delta > 0
               AND fournisseur_id = ?1",
            rusqlite::params![fournisseur_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        ).unwrap_or((0.0, 0, None));

    let total_achats: i64 = conn.query_row(
        "SELECT CAST(COALESCE(SUM(
             CASE pc.type_piece
               WHEN 'facture_fournisseur' THEN lp.montant_ht + lp.montant_tva
               WHEN 'avoir_fournisseur'   THEN
                    CASE WHEN pc.statut = 'paye' THEN 0
                         ELSE -(lp.montant_ht + lp.montant_tva) END
               ELSE 0 END), 0) AS INTEGER)
         FROM piece_commerciale pc
         JOIN ligne_piece lp ON lp.piece_id = pc.id
         WHERE pc.tiers_type = 'fournisseur'
           AND pc.tiers_id = ?1
           AND pc.type_piece IN ('facture_fournisseur','avoir_fournisseur')
           AND pc.statut <> 'annule'",
        rusqlite::params![fournisseur_id], |r| r.get(0),
    ).unwrap_or(0);

    let total_paye: i64 = conn.query_row(
        "SELECT CAST(COALESCE(SUM(montant), 0) AS INTEGER)
         FROM paiement_fournisseur WHERE fournisseur_id = ?1",
        rusqlite::params![fournisseur_id], |r| r.get(0),
    ).unwrap_or(0);

    let dette = (total_achats - total_paye).max(0);

    let mut stmt_p = conn.prepare(
        "SELECT pf.id, pf.montant, pf.mode, pf.note, pf.date_paiement, u.nom
         FROM paiement_fournisseur pf
         LEFT JOIN utilisateur u ON u.id = pf.auteur_id
         WHERE pf.fournisseur_id = ?1 ORDER BY pf.date_paiement DESC"
    ).map_err(|e| e.to_string())?;

    let paiements: Vec<serde_json::Value> = stmt_p.query_map(
        rusqlite::params![fournisseur_id], |r| {
            Ok(serde_json::json!({
                "id":            r.get::<_,String>(0)?,
                "montant":       r.get::<_,i64>(1)?,
                "mode":          r.get::<_,String>(2)?,
                "note":          r.get::<_,Option<String>>(3)?,
                "date_paiement": r.get::<_,String>(4)?,
                "auteur_nom":    r.get::<_,Option<String>>(5)?,
            }))
        }
    ).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    let mut stmt_a = conn.prepare(
        "SELECT ms.id, a.nom, ms.quantite_delta,
                COALESCE(ms.prix_achat_unitaire, a.dernier_prix_achat, 0),
                ms.date_mouvement
         FROM mouvement_stock ms
         JOIN article a ON a.id = ms.article_id
         WHERE ms.type_mouvement = 'achat' AND ms.quantite_delta > 0
           AND ms.fournisseur_id = ?1
         ORDER BY ms.date_mouvement DESC LIMIT 50"
    ).map_err(|e| e.to_string())?;

    let achats: Vec<serde_json::Value> = stmt_a.query_map(
        rusqlite::params![fournisseur_id], |r| {
        Ok(serde_json::json!({
            "id":            r.get::<_,String>(0)?,
            "article_nom":   r.get::<_,String>(1)?,
            "quantite":      r.get::<_,f64>(2)?,
            "prix_achat":    r.get::<_,i64>(3)?,
            "date_mouvement":r.get::<_,String>(4)?,
        }))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    Ok(serde_json::json!({
        "stats": {
            "total_achats":      total_achats,
            "nb_achats":         nb_achats,
            "dette":             dette,
            "total_paye":        total_paye,
            "derniere_commande": derniere_cmd,
        },
        "paiements": paiements,
        "achats":    achats,
    }))
}