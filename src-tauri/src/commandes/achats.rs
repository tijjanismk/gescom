//! Achats fournisseur — entrée de stock + facture fournisseur (FAF).
//!
//! Un achat produit UN document : une pièce `facture_fournisseur`.
//! C'est elle qui porte la dette — plus aucun calcul dérivé de
//! `mouvement_stock`. Bons de commande et bons de réception restent du
//! ressort du fournisseur, ils ne sont pas générés ici.
//!
//! Tout se fait dans une transaction unique : si une ligne échoue,
//! ni le stock ni la facture ne bougent.

use tauri::State;
use serde::Deserialize;
use crate::commandes::ventes::EtatApp;
use crate::utils::maintenant_iso;

#[derive(Deserialize)]
pub struct LigneAchat {
    pub article_id: String,
    pub unite_vente_id: String,
    /// Quantité dans l'unité de vente choisie (carton, sac...).
    pub quantite: f64,
    /// Combien d'unités de base vaut cette unité.
    pub facteur: f64,
    /// Prix d'achat unitaire, pour l'unité choisie.
    pub prix_achat: i64,
}

/// Enregistre un achat complet : stock, mouvements, facture fournisseur.
///
/// - `mode_reglement` = "comptant" → un paiement du total est enregistré,
///   la dette retombe à zéro immédiatement.
/// - `mode_reglement` = "credit"   → la dette reste ouverte.
///
/// Sans `fournisseur_id`, le stock est mis à jour mais aucune facture
/// n'est créée (régularisation interne).
#[tauri::command]
pub fn enregistrer_achat(
    etat: State<EtatApp>,
    fournisseur_id: Option<String>,
    depot_id: Option<String>,
    lignes: Vec<LigneAchat>,
    mode_reglement: Option<String>,
    // especes | orange_money | moov_money | cheque — pour le comptant
    mode_paiement: Option<String>,
    note: Option<String>,
    utilisateur_role: Option<String>,
) -> Result<serde_json::Value, String> {
    if lignes.is_empty() {
        return Err("Aucune ligne à enregistrer".to_string());
    }

    let mut conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let now = maintenant_iso();
    let role = utilisateur_role.as_deref().unwrap_or("employe");
    let auteur = crate::commandes::ventes::id_utilisateur_par_role(&conn, role);

    // Résoudre le dépôt avant d'ouvrir la transaction.
    let depot_id = match depot_id {
        Some(d) if !d.is_empty() => d,
        _ => conn.query_row(
            "SELECT id FROM depot WHERE est_defaut = 1 LIMIT 1",
            [], |r| r.get(0),
        ).map_err(|_| "Aucun dépôt par défaut configuré".to_string())?,
    };

    // Numéro réservé avant la transaction (lecture seule).
    let numero = if fournisseur_id.is_some() {
        Some(crate::commandes::pieces::prochain_numero(&conn, "facture_fournisseur"))
    } else {
        None
    };

    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let op_id = uuid::Uuid::new_v4().to_string();
    let mut total: i64 = 0;

    // ---- 1. Stock et mouvements ----
    for l in &lignes {
        let quantite_base = l.quantite * l.facteur;
        let montant = (l.prix_achat as f64 * l.quantite).round() as i64;
        total += montant;

        tx.execute(
            "INSERT INTO stock_depot (id, article_id, depot_id, quantite)
             VALUES (?1,?2,?3,?4)
             ON CONFLICT(article_id, depot_id)
             DO UPDATE SET quantite = quantite + ?4",
            rusqlite::params![
                uuid::Uuid::new_v4().to_string(),
                l.article_id, depot_id, quantite_base
            ],
        ).map_err(|e| e.to_string())?;

        // Prix ramené à l'unité de base, pour rester homogène avec
        // quantite_delta qui est lui aussi en unité de base.
        let prix_base = if l.facteur > 0.0 {
            (l.prix_achat as f64 / l.facteur).round() as i64
        } else { l.prix_achat };

        tx.execute(
            "INSERT INTO mouvement_stock
             (id, article_id, depot_id, type_mouvement, quantite_delta,
              operation_id, auteur_id, date_mouvement, cree_le, cree_par, origine,
              fournisseur_id, prix_achat_unitaire)
             VALUES (?1,?2,?3,'achat',?4,?5,?6,?7,?8,?9,'app',?10,?11)",
            rusqlite::params![
                uuid::Uuid::new_v4().to_string(),
                l.article_id, depot_id, quantite_base,
                op_id, auteur, now, now, auteur,
                fournisseur_id, prix_base
            ],
        ).map_err(|e| e.to_string())?;

        tx.execute(
            "UPDATE article SET dernier_prix_achat = ?1, modifie_le = ?2 WHERE id = ?3",
            rusqlite::params![prix_base, now, l.article_id],
        ).ok();
    }

    // ---- 2. Facture fournisseur ----
    let mut piece_id_retour = serde_json::Value::Null;
    let mut numero_retour = serde_json::Value::Null;

    if let (Some(f_id), Some(num)) = (fournisseur_id.as_ref(), numero.as_ref()) {
        let piece_id = uuid::Uuid::new_v4().to_string();

        // Une facture fournisseur arrive du fournisseur : elle est ferme
        // des sa reception. Son statut reflete donc le REGLEMENT, pas un
        // cycle de validation interne.
        //   comptant -> "paye"  (soldee immediatement)
        //   credit   -> "emis"  (recue, non reglee)
        // "validee" n'a de sens que cote client (brouillon -> vente creee).
        let statut_piece = if mode_reglement.as_deref() == Some("comptant") {
            "paye"
        } else {
            "emis"
        };

        tx.execute(
            "INSERT INTO piece_commerciale
             (id, type_piece, numero, statut, tiers_type, tiers_id, depot_id,
              auteur_id, date_piece, remise_globale, note,
              cree_le, modifie_le, origine)
             VALUES (?1,'facture_fournisseur',?2,?3,'fournisseur',?4,?5,
                     ?6,?7,0.0,?8,?9,?10,'achat')",
            rusqlite::params![
                piece_id, num, statut_piece, f_id, depot_id,
                auteur, now, note, now, now
            ],
        ).map_err(|e| e.to_string())?;

        for l in &lignes {
            let montant_ht = (l.prix_achat as f64 * l.quantite).round() as i64;
            tx.execute(
                "INSERT INTO ligne_piece
                 (id, piece_id, article_id, unite_vente_id, quantite,
                  prix_unitaire, remise_pct, remise_montant,
                  taux_tva, montant_tva, montant_ht, cree_le)
                 VALUES (?1,?2,?3,?4,?5,?6,0.0,0,0.0,0,?7,?8)",
                rusqlite::params![
                    uuid::Uuid::new_v4().to_string(),
                    piece_id, l.article_id, l.unite_vente_id,
                    l.quantite, l.prix_achat, montant_ht, now
                ],
            ).map_err(|e| e.to_string())?;
        }

        // ---- 3. Solde de la dette (comptant) ----
        // Uniquement ici : sans fournisseur, il n'y a pas de dette a solder.
        if mode_reglement.as_deref() == Some("comptant") && total > 0 {
            tx.execute(
                "INSERT INTO paiement_fournisseur
                 (id, fournisseur_id, montant, mode, note,
                  auteur_id, date_paiement, cree_le, origine)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,'achat')",
                rusqlite::params![
                    uuid::Uuid::new_v4().to_string(),
                    f_id, total,
                    mode_paiement.as_deref().unwrap_or("especes"),
                    format!("Achat {}", num),
                    auteur, now, now
                ],
            ).map_err(|e| e.to_string())?;
        }

        piece_id_retour = serde_json::json!(piece_id);
        numero_retour = serde_json::json!(num);
    }

    // ---- 4. Sortie de caisse ----
    // HORS du bloc fournisseur : l'argent quitte le tiroir meme sans
    // tiers identifie (regularisation, achat de depannage). Sans ce
    // mouvement, la cloture affiche un excedent egal aux achats comptant.
    if mode_reglement.as_deref() == Some("comptant") && total > 0 {
        let mode_p = mode_paiement.as_deref().unwrap_or("especes");
        let session: Option<String> = tx.query_row(
            "SELECT id FROM session_caisse WHERE statut = 'ouverte' LIMIT 1",
            [], |r| r.get(0),
        ).ok();
        if let Some(sid) = session {
            tx.execute(
                "INSERT INTO mouvement_caisse
                 (id, session_id, sens, moyen, montant, motif,
                  operation_id, date_mouvement, cree_le, cree_par, origine)
                 VALUES (?1,?2,'sortie',?3,?4,'achat',?5,?6,?7,?8,'app')",
                rusqlite::params![
                    uuid::Uuid::new_v4().to_string(),
                    sid, mode_p, total, op_id, now, now, auteur
                ],
            ).map_err(|e| e.to_string())?;
        }
    }

    tx.execute(
        "INSERT INTO journal
         (id, type_evenement, entite_type, entite_id, auteur_id,
          nouveau_valeur, origine, date_evenement)
         VALUES (?1,'achat_enregistre','operation',?2,?3,?4,'app',?5)",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(), op_id, auteur,
            format!(r#"{{"total":{},"nb_lignes":{}}}"#, total, lignes.len()),
            now
        ],
    ).ok();

    tx.commit().map_err(|e| e.to_string())?;

    Ok(serde_json::json!({
        "piece_id": piece_id_retour,
        "numero":   numero_retour,
        "total":    total,
        "statut":   if mode_reglement.as_deref() == Some("comptant")
                    { "paye" } else { "emis" },
    }))
}

// =====================================================================
//  RETOUR FOURNISSEUR
// =====================================================================
//
//  Symetrique du retour client, mais les flux sont inverses :
//  le stock SORT et la dette DIMINUE.
//
//  Deux modes :
//    "avoir"         — le fournisseur credite le compte. Piece AVF,
//                      dette reduite, caisse inchangee. Cas normal.
//    "remboursement" — le fournisseur rend l'argent. Entree de caisse,
//                      et l'AVF est marque 'paye' pour ne pas reduire
//                      la dette une seconde fois.
// =====================================================================

#[derive(Deserialize)]
pub struct LigneRetourFournisseur {
    pub article_id: String,
    pub unite_vente_id: String,
    pub quantite: f64,
    pub facteur: f64,
    pub prix_achat: i64,
}

#[tauri::command]
pub fn enregistrer_retour_fournisseur(
    etat: State<EtatApp>,
    fournisseur_id: String,
    depot_id: Option<String>,
    lignes: Vec<LigneRetourFournisseur>,
    // Facture d'achat d'origine — permet de calculer le reliquat PAR
    // FACTURE et non par article, seul moyen de rester juste quand le
    // meme article a ete achete sur plusieurs factures.
    piece_origine_id: Option<String>,
    // "avoir" (defaut) | "remboursement"
    mode_resolution: Option<String>,
    // especes | orange_money | moov_money | cheque — si remboursement
    mode_encaissement: Option<String>,
    motif: Option<String>,
    utilisateur_role: Option<String>,
) -> Result<serde_json::Value, String> {
    if lignes.is_empty() {
        return Err("Aucune ligne a retourner".to_string());
    }

    let mut conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let now = maintenant_iso();
    let role = utilisateur_role.as_deref().unwrap_or("employe");
    let auteur = crate::commandes::ventes::id_utilisateur_par_role(&conn, role);

    let depot_id = match depot_id {
        Some(d) if !d.is_empty() => d,
        _ => conn.query_row(
            "SELECT id FROM depot WHERE est_defaut = 1 LIMIT 1",
            [], |r| r.get(0),
        ).map_err(|_| "Aucun depot par defaut configure".to_string())?,
    };

    let rembourse = mode_resolution.as_deref() == Some("remboursement");
    let numero = crate::commandes::pieces::prochain_numero(&conn, "avoir_fournisseur");

    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let op_id = uuid::Uuid::new_v4().to_string();
    let piece_id = uuid::Uuid::new_v4().to_string();
    let mut total: i64 = 0;

    // Un AVF rembourse en especes est deja solde : statut 'paye', il ne
    // doit pas reduire la dette en plus de l'entree de caisse.
    let statut_piece = if rembourse { "paye" } else { "emis" };

    tx.execute(
        "INSERT INTO piece_commerciale
         (id, type_piece, numero, statut, tiers_type, tiers_id, depot_id,
          piece_origine_id, auteur_id, date_piece, remise_globale, note,
          cree_le, modifie_le, origine)
         VALUES (?1,'avoir_fournisseur',?2,?3,'fournisseur',?4,?5,?6,
                 ?7,?8,0.0,?9,?10,?11,'retour')",
        rusqlite::params![
            piece_id, numero, statut_piece, fournisseur_id, depot_id,
            piece_origine_id, auteur, now, motif, now, now
        ],
    ).map_err(|e| e.to_string())?;

    for l in &lignes {
        let quantite_base = l.quantite * l.facteur;
        let montant = (l.prix_achat as f64 * l.quantite).round() as i64;
        total += montant;

        // Sortie de stock — ON CONFLICT, jamais un UPDATE nu.
        tx.execute(
            "INSERT INTO stock_depot (id, article_id, depot_id, quantite)
             VALUES (?1,?2,?3,0 - ?4)
             ON CONFLICT(article_id, depot_id)
             DO UPDATE SET quantite = quantite - ?4",
            rusqlite::params![
                uuid::Uuid::new_v4().to_string(),
                l.article_id, depot_id, quantite_base
            ],
        ).map_err(|e| e.to_string())?;

        let prix_base = if l.facteur > 0.0 {
            (l.prix_achat as f64 / l.facteur).round() as i64
        } else { l.prix_achat };

        tx.execute(
            "INSERT INTO mouvement_stock
             (id, article_id, depot_id, type_mouvement, quantite_delta,
              operation_id, auteur_id, date_mouvement, cree_le, cree_par, origine,
              fournisseur_id, prix_achat_unitaire)
             VALUES (?1,?2,?3,'retour_fournisseur',?4,?5,?6,?7,?8,?9,'app',?10,?11)",
            rusqlite::params![
                uuid::Uuid::new_v4().to_string(),
                l.article_id, depot_id, -quantite_base,
                op_id, auteur, now, now, auteur,
                fournisseur_id, prix_base
            ],
        ).map_err(|e| e.to_string())?;

        tx.execute(
            "INSERT INTO ligne_piece
             (id, piece_id, article_id, unite_vente_id, quantite,
              prix_unitaire, remise_pct, remise_montant,
              taux_tva, montant_tva, montant_ht, cree_le)
             VALUES (?1,?2,?3,?4,?5,?6,0.0,0,0.0,0,?7,?8)",
            rusqlite::params![
                uuid::Uuid::new_v4().to_string(),
                piece_id, l.article_id, l.unite_vente_id,
                l.quantite, l.prix_achat, montant, now
            ],
        ).map_err(|e| e.to_string())?;
    }

    // Remboursement : l'argent revient dans le tiroir.
    if rembourse && total > 0 {
        let mode_e = mode_encaissement.as_deref().unwrap_or("especes");
        let session: Option<String> = tx.query_row(
            "SELECT id FROM session_caisse WHERE statut = 'ouverte' LIMIT 1",
            [], |r| r.get(0),
        ).ok();
        if let Some(sid) = session {
            tx.execute(
                "INSERT INTO mouvement_caisse
                 (id, session_id, sens, moyen, montant, motif,
                  operation_id, date_mouvement, cree_le, cree_par, origine)
                 VALUES (?1,?2,'entree',?3,?4,'retour_fournisseur',?5,?6,?7,?8,'app')",
                rusqlite::params![
                    uuid::Uuid::new_v4().to_string(),
                    sid, mode_e, total, op_id, now, now, auteur
                ],
            ).map_err(|e| e.to_string())?;
        }
    }

    tx.execute(
        "INSERT INTO journal
         (id, type_evenement, entite_type, entite_id, auteur_id,
          nouveau_valeur, origine, date_evenement)
         VALUES (?1,'retour_fournisseur','piece',?2,?3,?4,'app',?5)",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(), piece_id, auteur,
            format!(r#"{{"total":{},"mode":"{}"}}"#, total,
                    if rembourse { "remboursement" } else { "avoir" }),
            now
        ],
    ).ok();

    tx.commit().map_err(|e| e.to_string())?;

    Ok(serde_json::json!({
        "piece_id": piece_id,
        "numero":   numero,
        "total":    total,
        "statut":   statut_piece,
    }))
}

/// Factures d'achat d'un fournisseur, avec le reliquat retournable par
/// ligne (quantite achetee moins quantite deja retournee).
///
/// Sans ce calcul, rien n'empeche de retourner deux fois la meme
/// marchandise.
#[tauri::command]
pub fn lire_factures_fournisseur_retournables(
    etat: State<EtatApp>,
    fournisseur_id: String,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let mut st_f = conn.prepare(
        "SELECT id, numero, date_piece, statut
         FROM piece_commerciale
         WHERE tiers_type = 'fournisseur' AND tiers_id = ?1
           AND type_piece = 'facture_fournisseur'
           AND statut <> 'annule'
         ORDER BY date_piece DESC LIMIT 50"
    ).map_err(|e| e.to_string())?;

    let factures: Vec<(String, String, String, String)> = st_f.query_map(
        rusqlite::params![fournisseur_id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        }
    ).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    let mut resultat = Vec::new();

    for (piece_id, numero, date_piece, statut) in factures {
        let mut st_l = conn.prepare(
            "SELECT lp.id, lp.article_id, a.nom, lp.unite_vente_id,
                    u.libelle, u.facteur, lp.quantite, lp.prix_unitaire
             FROM ligne_piece lp
             JOIN article a ON a.id = lp.article_id
             JOIN unite_vente u ON u.id = lp.unite_vente_id
             WHERE lp.piece_id = ?1
             ORDER BY a.nom"
        ).map_err(|e| e.to_string())?;

        let lignes: Vec<serde_json::Value> = st_l.query_map(
            rusqlite::params![piece_id], |r| {
                Ok(serde_json::json!({
                    "ligne_id":       r.get::<_,String>(0)?,
                    "article_id":     r.get::<_,String>(1)?,
                    "article_nom":    r.get::<_,String>(2)?,
                    "unite_vente_id": r.get::<_,String>(3)?,
                    "unite_libelle":  r.get::<_,String>(4)?,
                    "facteur":        r.get::<_,f64>(5)?,
                    "quantite":       r.get::<_,f64>(6)?,
                    "prix_achat":     r.get::<_,i64>(7)?,
                }))
            }
        ).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

        // Quantites deja retournees SUR CETTE FACTURE (piece_origine_id).
        // Agreger par article tous AVF confondus serait faux des que le
        // meme article apparait sur deux factures d'achat.
        //
        // Les AVF anterieurs a cette colonne ont piece_origine_id NULL :
        // ils ne sont rattaches a aucune facture et n'entrent donc dans
        // aucun reliquat. Sans objet en dev, a savoir en production.
        let mut st_r = conn.prepare(
            "SELECT lp.article_id, COALESCE(SUM(lp.quantite), 0)
             FROM piece_commerciale pc
             JOIN ligne_piece lp ON lp.piece_id = pc.id
             WHERE pc.type_piece = 'avoir_fournisseur'
               AND pc.statut <> 'annule'
               AND pc.piece_origine_id = ?1
             GROUP BY lp.article_id"
        ).map_err(|e| e.to_string())?;

        let retournes: std::collections::HashMap<String, f64> = st_r.query_map(
            rusqlite::params![piece_id], |r| {
                Ok((r.get::<_,String>(0)?, r.get::<_,f64>(1)?))
            }
        ).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

        let lignes: Vec<serde_json::Value> = lignes.into_iter().map(|mut l| {
            let art = l["article_id"].as_str().unwrap_or("").to_string();
            let qte = l["quantite"].as_f64().unwrap_or(0.0);
            let deja = *retournes.get(&art).unwrap_or(&0.0);
            let restant = (qte - deja).max(0.0);
            l["deja_retourne"] = serde_json::json!(deja);
            l["quantite_restante"] = serde_json::json!(restant);
            l
        }).collect();

        let total: i64 = lignes.iter().map(|l| {
            let q = l["quantite"].as_f64().unwrap_or(0.0);
            let p = l["prix_achat"].as_i64().unwrap_or(0);
            (p as f64 * q).round() as i64
        }).sum();

        resultat.push(serde_json::json!({
            "piece_id":   piece_id,
            "numero":     numero,
            "date_piece": date_piece,
            "statut":     statut,
            "total":      total,
            "lignes":     lignes,
        }));
    }

    Ok(resultat)
}
