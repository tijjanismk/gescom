//! Pièces commerciales — cycle complet client.
//!
//! Cycle :
//!   Devis/Proforma → Commande → Facture brouillon → Facture validée (= Vente)
//!   Commande → BL (optionnel, rare) → Facture brouillon → Facture validée
//!
//! Règles :
//!   - Une pièce ne peut être transférée qu'une seule fois (statut → 'transfere')
//!   - BL n'est jamais une vente
//!   - Facture créée toujours en 'brouillon'
//!   - Valider une facture = créer la vente (stock + caisse)

use tauri::State;
use crate::commandes::ventes::EtatApp;
use crate::utils::maintenant_iso;

// =====================================================================
//  Numérotation
// =====================================================================

fn prochain_numero(conn: &rusqlite::Connection, type_piece: &str) -> String {
    let annee = chrono::Local::now().format("%Y").to_string();
    let prefix = match type_piece {
        "devis"            => "DEV",
        "proforma"         => "PRO",
        "commande_client"  => "CMD",
        "bon_livraison"    => "BL",
        "facture"          => "FAC",
        "facture_acompte"  => "ACP",
        "avoir_client"     => "AVC",
        _                  => "PIE",
    };
    let nb: i64 = conn.query_row(
        "SELECT COUNT(*) FROM piece_commerciale
         WHERE type_piece = ?1 AND numero LIKE ?2",
        rusqlite::params![type_piece, format!("{}-{}-%" , prefix, annee)],
        |r| r.get(0),
    ).unwrap_or(0);
    format!("{}-{}-{:05}", prefix, annee, nb + 1)
}

// =====================================================================
//  Lire TOUTES les pièces client (page Pièces style Ciel)
// =====================================================================

#[tauri::command]
pub fn lire_toutes_pieces_client(
    etat: State<EtatApp>,
    type_filtre: Option<String>,
    statut: Option<String>,
    recherche: Option<String>,
    date_debut: Option<String>,
    date_fin: Option<String>,
    montant_min: Option<i64>,
    montant_max: Option<i64>,
    impaye_seulement: Option<bool>,
    en_retard_seulement: Option<bool>,
    client_id: Option<String>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let mut conditions = vec!["pc.tiers_type = 'client'".to_string()];

    if let Some(ref tf) = type_filtre {
        if tf == "devis" {
            conditions.push("pc.type_piece IN ('devis','proforma')".to_string());
        } else {
            conditions.push(format!("pc.type_piece = '{}'",
                tf.replace('\'', "''")));
        }
    }
    if let Some(ref s) = statut {
        conditions.push(format!("pc.statut = '{}'", s.replace('\'', "''")));
    }
    if let Some(ref r) = recherche {
        let r = r.replace('\'', "''");
        conditions.push(format!(
            "(pc.numero LIKE '%{r}%' OR c.nom LIKE '%{r}%' OR c.code LIKE '%{r}%')"
        ));
    }
    if let Some(ref dd) = date_debut {
        conditions.push(format!("pc.date_piece >= '{}'", dd.replace('\'', "''")));
    }
    if let Some(ref df) = date_fin {
        conditions.push(format!("pc.date_piece <= '{}'", df.replace('\'', "''")));
    }
    if let Some(ref cid) = client_id {
        conditions.push(format!("pc.tiers_id = '{}'", cid.replace('\'', "''")));
    }
    if impaye_seulement == Some(true) {
        conditions.push("pc.statut NOT IN ('paye','annule','transfere')".to_string());
    }
    if en_retard_seulement == Some(true) {
        conditions.push(
            "pc.date_echeance IS NOT NULL AND pc.date_echeance < date('now') \
             AND pc.statut NOT IN ('paye','annule','transfere')".to_string()
        );
    }

    // Filtre montant appliqué après (HAVING ou sous-requête)
    let where_clause = conditions.join(" AND ");

    let sql = format!(
        "SELECT pc.id, pc.type_piece, pc.numero, pc.statut,
                pc.date_piece, pc.date_echeance, pc.remise_globale, pc.note,
                c.id as tiers_id, c.nom as tiers_nom, c.code as tiers_code,
                CAST(COALESCE(
                  (SELECT SUM(lp.montant_ht)
                   FROM ligne_piece lp WHERE lp.piece_id = pc.id), 0
                ) AS INTEGER) as total_ht,
                CAST(COALESCE(
                  (SELECT SUM(lp.montant_tva)
                   FROM ligne_piece lp WHERE lp.piece_id = pc.id), 0
                ) AS INTEGER) as total_tva,
                u.nom as auteur_nom,
                pc.piece_origine_id
         FROM piece_commerciale pc
         JOIN client c ON c.id = pc.tiers_id
         LEFT JOIN utilisateur u ON u.id = pc.auteur_id
         WHERE {}
         ORDER BY pc.date_piece DESC, pc.cree_le DESC",
        where_clause
    );

    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;

    let rows: Vec<serde_json::Value> = stmt.query_map([], |row| {
        let total_ht: i64 = row.get(11)?;
        let total_tva: i64 = row.get(12)?;
        let remise_g: f64 = row.get(6).unwrap_or(0.0);
        let remise_mt = (total_ht as f64 * remise_g / 100.0).round() as i64;
        let total_net = total_ht - remise_mt;
        Ok(serde_json::json!({
            "id":               row.get::<_,String>(0)?,
            "type_piece":       row.get::<_,String>(1)?,
            "numero":           row.get::<_,String>(2)?,
            "statut":           row.get::<_,String>(3)?,
            "date_piece":       row.get::<_,String>(4)?,
            "date_echeance":    row.get::<_,Option<String>>(5)?,
            "remise_globale":   remise_g,
            "note":             row.get::<_,Option<String>>(7)?,
            "tiers_id":         row.get::<_,String>(8)?,
            "tiers_nom":        row.get::<_,String>(9)?,
            "tiers_code":       row.get::<_,String>(10)?,
            "total_ht":         total_ht,
            "total_tva":        total_tva,
            "remise_montant":   remise_mt,
            "total_net":        total_net,
            "total_ttc":        total_net + total_tva,
            "auteur_nom":       row.get::<_,Option<String>>(13)?,
            "piece_origine_id": row.get::<_,Option<String>>(14)?,
        }))
    }).map_err(|e| e.to_string())?
    .filter_map(|r| r.ok())
    .filter(|p| {
        // Filtre montant post-requête
        let net = p["total_net"].as_i64().unwrap_or(0);
        let ok_min = montant_min.map(|m| net >= m).unwrap_or(true);
        let ok_max = montant_max.map(|m| net <= m).unwrap_or(true);
        ok_min && ok_max
    })
    .collect();

    Ok(rows)
}

// =====================================================================
//  Lire pièces d'un client spécifique (fiche client)
// =====================================================================

#[tauri::command]
pub fn lire_pieces_client(
    etat: State<EtatApp>,
    client_id: String,
    type_filtre: Option<String>,
) -> Result<Vec<serde_json::Value>, String> {
    lire_toutes_pieces_client(
        etat, type_filtre, None, None,
        None, None, None, None, None, None,
        Some(client_id),
    )
}

// =====================================================================
//  Lire les lignes d'une pièce
// =====================================================================

#[tauri::command]
pub fn lire_lignes_piece(
    etat: State<EtatApp>,
    piece_id: String,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(
        "SELECT lp.id, lp.article_id, a.nom,
                lp.unite_vente_id, uv.libelle, uv.facteur,
                lp.quantite, lp.prix_unitaire,
                lp.remise_pct, lp.remise_montant,
                lp.taux_tva, lp.montant_tva, lp.montant_ht
         FROM ligne_piece lp
         JOIN article a ON a.id = lp.article_id
         JOIN unite_vente uv ON uv.id = lp.unite_vente_id
         WHERE lp.piece_id = ?1 ORDER BY lp.cree_le"
    ).map_err(|e| e.to_string())?;

    let x = stmt.query_map(rusqlite::params![piece_id], |row| {
        Ok(serde_json::json!({
            "id":             row.get::<_,String>(0)?,
            "article_id":     row.get::<_,String>(1)?,
            "article_nom":    row.get::<_,String>(2)?,
            "unite_id":       row.get::<_,String>(3)?,
            "unite_libelle":  row.get::<_,String>(4)?,
            "facteur":        row.get::<_,f64>(5)?,
            "quantite":       row.get::<_,f64>(6)?,
            "prix_unitaire":  row.get::<_,i64>(7)?,
            "remise_pct":     row.get::<_,f64>(8)?,
            "remise_montant": row.get::<_,i64>(9)?,
            "taux_tva":       row.get::<_,f64>(10)?,
            "montant_tva":    row.get::<_,i64>(11)?,
            "montant_ht":     row.get::<_,i64>(12)?,
        }))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
    Ok(x)
}

// =====================================================================
//  Créer une pièce
// =====================================================================

#[derive(serde::Deserialize)]
pub struct LignePieceInput {
    pub article_id: String,
    pub unite_vente_id: String,
    pub quantite: f64,
    pub prix_unitaire: i64,
    pub remise_pct: f64,
    pub taux_tva: f64,
}

#[tauri::command]
pub fn creer_piece(
    etat: State<EtatApp>,
    client_id: String,
    type_piece: String,
    lignes: Vec<LignePieceInput>,
    remise_globale: Option<f64>,
    date_echeance: Option<String>,
    note: Option<String>,
    piece_origine_id: Option<String>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let auteur = crate::commandes::ventes::id_utilisateur_courant_pub(&conn);
    let now = maintenant_iso();
    let piece_id = uuid::Uuid::new_v4().to_string();
    let numero = prochain_numero(&conn, &type_piece);
    let remise_g = remise_globale.unwrap_or(0.0);

    // Statut initial selon le type
    let statut = match type_piece.as_str() {
        "devis" | "proforma"   => "brouillon",
        "commande_client"      => "brouillon",
        "bon_livraison"        => "emis",
        "facture"              => "brouillon", // toujours brouillon
        "avoir_client"         => "emis",
        _                      => "brouillon",
    };

    conn.execute(
        "INSERT INTO piece_commerciale
         (id, type_piece, numero, statut, tiers_type, tiers_id,
          piece_origine_id, auteur_id, date_piece, date_echeance,
          remise_globale, note, cree_le, modifie_le, origine)
         VALUES (?1,?2,?3,?4,'client',?5,?6,?7,?8,?9,?10,?11,?12,?13,'app')",
        rusqlite::params![
            piece_id, type_piece, numero, statut,
            client_id, piece_origine_id, auteur,
            now, date_echeance, remise_g, note, now, now
        ],
    ).map_err(|e| e.to_string())?;

    inserer_lignes(&conn, &piece_id, &lignes, &now)?;

    Ok(serde_json::json!({
        "id": piece_id, "numero": numero, "statut": statut,
    }))
}

fn inserer_lignes(
    conn: &rusqlite::Connection,
    piece_id: &str,
    lignes: &[LignePieceInput],
    now: &str,
) -> Result<(), String> {
    for ligne in lignes {
        let montant_brut = (ligne.prix_unitaire as f64 * ligne.quantite).round() as i64;
        let remise_mt = (montant_brut as f64 * ligne.remise_pct / 100.0).round() as i64;
        let montant_ht = montant_brut - remise_mt;
        let montant_tva = (montant_ht as f64 * ligne.taux_tva).round() as i64;

        conn.execute(
            "INSERT INTO ligne_piece
             (id, piece_id, article_id, unite_vente_id, quantite,
              prix_unitaire, remise_pct, remise_montant,
              taux_tva, montant_tva, montant_ht, cree_le)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
            rusqlite::params![
                uuid::Uuid::new_v4().to_string(),
                piece_id, ligne.article_id, ligne.unite_vente_id,
                ligne.quantite, ligne.prix_unitaire,
                ligne.remise_pct, remise_mt,
                ligne.taux_tva, montant_tva, montant_ht, now
            ],
        ).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// =====================================================================
//  Convertir une pièce → pièce suivante (1 seul transfert autorisé)
// =====================================================================

#[tauri::command]
pub fn convertir_piece(
    etat: State<EtatApp>,
    piece_id: String,
    nouveau_type: String,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    // Vérifier la pièce source
    let (client_id, type_src, statut_src, remise_g, date_ech, note):
        (String, String, String, f64, Option<String>, Option<String>) =
        conn.query_row(
            "SELECT tiers_id, type_piece, statut, remise_globale, date_echeance, note
             FROM piece_commerciale WHERE id = ?1",
            rusqlite::params![piece_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
        ).map_err(|_| "Pièce introuvable".to_string())?;

    // Vérifier qu'elle n'a pas déjà été transférée
    if statut_src == "transfere" {
        return Err("Cette pièce a déjà été transférée".to_string());
    }

    // Transitions autorisées
    let ok = match (type_src.as_str(), nouveau_type.as_str()) {
        ("devis",           "commande_client") => true,
        ("proforma",        "commande_client") => true,
        ("devis",           "facture")         => true, // raccourci direct
        ("proforma",        "facture")         => true,
        ("commande_client", "bon_livraison")   => true,
        ("commande_client", "facture")         => true,
        ("bon_livraison",   "facture")         => true,
        _ => false,
    };

    if !ok {
        return Err(format!(
            "Conversion {} → {} non autorisée", type_src, nouveau_type
        ));
    }

    // Lire les lignes source
    let lignes = lire_lignes_raw(&conn, &piece_id)?;

    let now = maintenant_iso();
    let auteur = crate::commandes::ventes::id_utilisateur_courant_pub(&conn);

    // Archiver la pièce source → statut 'transfere'
    conn.execute(
        "UPDATE piece_commerciale SET statut = 'transfere', modifie_le = ?1
         WHERE id = ?2",
        rusqlite::params![now, piece_id],
    ).map_err(|e| e.to_string())?;

    // Créer la nouvelle pièce
    let nouvelle_id = uuid::Uuid::new_v4().to_string();
    let numero = prochain_numero(&conn, &nouveau_type);

    // La facture est toujours créée en brouillon
    let statut_nouveau = match nouveau_type.as_str() {
        "commande_client" => "brouillon",
        "bon_livraison"   => "emis",
        "facture"         => "brouillon", // jamais validee automatiquement
        "avoir_client"    => "emis",
        _                 => "brouillon",
    };

    conn.execute(
        "INSERT INTO piece_commerciale
         (id, type_piece, numero, statut, tiers_type, tiers_id,
          piece_origine_id, auteur_id, date_piece, date_echeance,
          remise_globale, note, cree_le, modifie_le, origine)
         VALUES (?1,?2,?3,?4,'client',?5,?6,?7,?8,?9,?10,?11,?12,?13,'app')",
        rusqlite::params![
            nouvelle_id, nouveau_type, numero, statut_nouveau,
            client_id, piece_id, auteur,
            now, date_ech, remise_g, note, now, now
        ],
    ).map_err(|e| e.to_string())?;

    inserer_lignes_raw(&conn, &nouvelle_id, &lignes, &now)?;

    // Journaliser
    conn.execute(
        "INSERT INTO journal
         (id, type_evenement, entite_type, entite_id, auteur_id,
          nouveau_valeur, origine, date_evenement)
         VALUES (?1,'piece_convertie','piece_commerciale',?2,?3,?4,'app',?5)",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(),
            nouvelle_id, auteur,
            format!(r#"{{"source":"{}","destination":"{}","numero":"{}"}}"#,
                piece_id, nouvelle_id, numero),
            now
        ],
    ).ok();

    Ok(serde_json::json!({
        "id":         nouvelle_id,
        "numero":     numero,
        "statut":     statut_nouveau,
        "type_piece": nouveau_type,
    }))
}

// Helpers lignes raw pour la conversion
fn lire_lignes_raw(
    conn: &rusqlite::Connection,
    piece_id: &str,
) -> Result<Vec<(String, String, f64, i64, f64, f64)>, String> {
    let mut stmt = conn.prepare(
        "SELECT article_id, unite_vente_id, quantite, prix_unitaire,
                remise_pct, taux_tva
         FROM ligne_piece WHERE piece_id = ?1"
    ).map_err(|e| e.to_string())?;

    let x = stmt.query_map(rusqlite::params![piece_id], |row| {
        Ok((
            row.get::<_,String>(0)?, row.get::<_,String>(1)?,
            row.get::<_,f64>(2)?,   row.get::<_,i64>(3)?,
            row.get::<_,f64>(4)?,   row.get::<_,f64>(5)?,
        ))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
    Ok(x)
}

fn inserer_lignes_raw(
    conn: &rusqlite::Connection,
    piece_id: &str,
    lignes: &[(String, String, f64, i64, f64, f64)],
    now: &str,
) -> Result<(), String> {
    for (art_id, uv_id, qte, prix, remise_pct, taux_tva) in lignes {
        let montant_brut = (*prix as f64 * qte).round() as i64;
        let remise_mt = (montant_brut as f64 * remise_pct / 100.0).round() as i64;
        let montant_ht = montant_brut - remise_mt;
        let montant_tva = (montant_ht as f64 * taux_tva).round() as i64;

        conn.execute(
            "INSERT INTO ligne_piece
             (id, piece_id, article_id, unite_vente_id, quantite,
              prix_unitaire, remise_pct, remise_montant,
              taux_tva, montant_tva, montant_ht, cree_le)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
            rusqlite::params![
                uuid::Uuid::new_v4().to_string(),
                piece_id, art_id, uv_id, qte, prix,
                remise_pct, remise_mt, taux_tva, montant_tva, montant_ht, now
            ],
        ).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// =====================================================================
//  Valider une facture → crée la vente (stock + caisse)
// =====================================================================

#[tauri::command]
pub fn valider_facture(
    etat: State<EtatApp>,
    piece_id: String,
    mode_reglement: String,     // comptant | credit
    mode_paiement: Option<String>, // especes | orange_money | moov_money
    acompte: Option<i64>,
    utilisateur_role: Option<String>,
) -> Result<serde_json::Value, String> {
    let mut conn = etat.conn.lock().map_err(|e| e.to_string())?;

    // Vérifier que c'est bien une facture brouillon
    let (client_id, type_p, statut, remise_g, depot_id_opt):
        (String, String, String, f64, Option<String>) =
        conn.query_row(
            "SELECT tiers_id, type_piece, statut, remise_globale, depot_id
             FROM piece_commerciale WHERE id = ?1",
            rusqlite::params![piece_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        ).map_err(|_| "Pièce introuvable".to_string())?;

    if type_p != "facture" {
        return Err("Seules les factures peuvent être validées".to_string());
    }
    if statut != "brouillon" {
        return Err(format!("Facture déjà en statut '{}'", statut));
    }

    // Lire les lignes
    let lignes_raw = lire_lignes_raw(&conn, &piece_id)?;
    if lignes_raw.is_empty() {
        return Err("La facture ne contient aucune ligne".to_string());
    }

    // Dépôt par défaut si non spécifié
    let depot_id = match depot_id_opt {
        Some(d) if !d.is_empty() => d,
        _ => conn.query_row(
            "SELECT id FROM depot WHERE est_defaut = 1 LIMIT 1",
            [], |r| r.get(0)
        ).map_err(|_| "Aucun dépôt par défaut".to_string())?,
    };

    let role = utilisateur_role.as_deref().unwrap_or("employe");
    let auteur_id = crate::commandes::ventes::id_utilisateur_par_role(&conn, role);
    let now = maintenant_iso();

    // Calculer les montants ligne par ligne.
    // La remise globale est REPARTIE au prorata sur chaque ligne, sinon
    // SUM(prix_pratique * quantite) ne correspond pas au montant du et la
    // creance reapparait apres reglement.
    // La TVA est AJOUTEE au HT : le client doit le TTC.
    struct MontantsLigne {
        montant_tva: i64,
        prix_pratique: i64,
    }
    let montants: Vec<MontantsLigne> = lignes_raw.iter()
        .map(|(_, _, qte, prix, remise_pct, taux_tva)| {
            let brut = (*prix as f64 * qte).round() as i64;
            let remise = (brut as f64 * remise_pct / 100.0).round() as i64;
            let ht_ligne = brut - remise;
            // Quote-part de remise globale (meme pourcentage sur chaque ligne).
            let remise_g_ligne = (ht_ligne as f64 * remise_g / 100.0).round() as i64;
            let montant_ht = ht_ligne - remise_g_ligne;
            let montant_tva = (montant_ht as f64 * taux_tva).round() as i64;
            let montant_ttc = montant_ht + montant_tva;
            // Prix unitaire TTC arrondi au franc : c'est LUI qui est stocke.
            let prix_pratique = if *qte > 0.0 {
                (montant_ttc as f64 / qte).round() as i64
            } else { *prix };
            MontantsLigne { montant_tva, prix_pratique }
        }).collect();

    // Le montant du doit etre derive des prix_pratique REELLEMENT stockes,
    // pas de la somme arithmetique des TTC de lignes : toutes les requetes
    // recalculent CAST(SUM(prix_pratique * quantite) AS INTEGER), et
    // l'arrondi du prix unitaire fait diverger les deux. Une divergence
    // d'un seul franc laisse une creance residuelle apres reglement.
    let total_net: i64 = lignes_raw.iter().enumerate()
        .map(|(i, (_, _, qte, _, _, _))| montants[i].prix_pratique as f64 * qte)
        .sum::<f64>() as i64;   // `as i64` tronque, comme le CAST de SQLite

    let tx = conn.transaction().map_err(|e| e.to_string())?;

    // 1. Créer la vente
    let vente_id = uuid::Uuid::new_v4().to_string();
    tx.execute(
        "INSERT INTO vente
         (id, client_id, depot_id, mode_reglement, auteur_id, statut,
          date_vente, cree_le, modifie_le, cree_par, modifie_par, origine)
         VALUES (?1,?2,?3,?4,?5,'creance_ouverte',?6,?7,?8,?9,?10,'app')",
        rusqlite::params![
            vente_id, client_id, depot_id, mode_reglement,
            auteur_id, now, now, now, auteur_id, auteur_id
        ],
    ).map_err(|e| e.to_string())?;

    // 2. Insérer les lignes de vente + décrémenter le stock
    for (i, (art_id, uv_id, qte, prix_u, _remise_pct, taux_tva))
        in lignes_raw.iter().enumerate()
    {
        let montant_tva   = montants[i].montant_tva;
        let prix_pratique = montants[i].prix_pratique;

        // Facteur de l'unité de vente
        let facteur: f64 = tx.query_row(
            "SELECT facteur FROM unite_vente WHERE id = ?1",
            rusqlite::params![uv_id], |r| r.get(0),
        ).unwrap_or(1.0);

        tx.execute(
            "INSERT INTO ligne_vente
             (id, vente_id, article_id, unite_vente_id, depot_source_id,
              source_approvisionnement, quantite, prix_reference,
              prix_pratique, taux_tva, montant_tva, cree_le, origine)
             VALUES (?1,?2,?3,?4,?5,'stock',?6,?7,?8,?9,?10,?11,'app')",
            rusqlite::params![
                uuid::Uuid::new_v4().to_string(),
                vente_id, art_id, uv_id, depot_id,
                qte, prix_u, prix_pratique,
                taux_tva, montant_tva, now
            ],
        ).map_err(|e| e.to_string())?;

        // Décrémenter stock
        let qte_base = qte * facteur;
        tx.execute(
            "INSERT INTO stock_depot (id, article_id, depot_id, quantite)
             VALUES (?1,?2,?3,0 - ?4)
             ON CONFLICT(article_id, depot_id)
             DO UPDATE SET quantite = quantite - ?4",
            rusqlite::params![
                uuid::Uuid::new_v4().to_string(), art_id, depot_id, qte_base
            ],
        ).map_err(|e| e.to_string())?;

        tx.execute(
            "INSERT INTO mouvement_stock
             (id, article_id, depot_id, type_mouvement, quantite_delta,
              operation_id, auteur_id, date_mouvement, cree_le, cree_par, origine)
             VALUES (?1,?2,?3,'vente',?4,?5,?6,?7,?8,?9,'app')",
            rusqlite::params![
                uuid::Uuid::new_v4().to_string(),
                art_id, depot_id, -qte_base,
                vente_id, auteur_id, now, now, auteur_id
            ],
        ).map_err(|e| e.to_string())?;
    }

    // 3. Numéro de facture lié à la vente
    let annee = chrono::Local::now().format("%Y").to_string();
    let nb_fac: i64 = tx.query_row(
        "SELECT COUNT(*) FROM facture WHERE numero LIKE ?1",
        rusqlite::params![format!("GESCOM-{}-%" , annee)],
        |r| r.get(0),
    ).unwrap_or(0);
    let numero_fac = format!("GESCOM-{}-{:06}", annee, nb_fac + 1);
    let facture_id = uuid::Uuid::new_v4().to_string();

    tx.execute(
        "INSERT INTO facture
         (id, numero, vente_id, statut, total, date_validation,
          cree_le, modifie_le, cree_par, modifie_par, origine)
         VALUES (?1,?2,?3,'validee',?4,?5,?6,?7,?8,?9,'app')",
        rusqlite::params![
            facture_id, numero_fac, vente_id, total_net,
            now, now, now, auteur_id, auteur_id
        ],
    ).map_err(|e| e.to_string())?;

    // 4. Paiement si comptant
    if mode_reglement == "comptant" {
        let mode_p = mode_paiement.as_deref().unwrap_or("especes");
        tx.execute(
            "INSERT INTO paiement
             (id, vente_id, montant, mode, date_paiement,
              auteur_id, cree_le, cree_par, origine)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,'app')",
            rusqlite::params![
                uuid::Uuid::new_v4().to_string(),
                vente_id, total_net, mode_p, now,
                auteur_id, now, auteur_id
            ],
        ).map_err(|e| e.to_string())?;

        tx.execute(
            "UPDATE vente SET statut = 'payee', modifie_le = ?1 WHERE id = ?2",
            rusqlite::params![now, vente_id],
        ).map_err(|e| e.to_string())?;

        // Caisse
        if let Ok(session_id) = tx.query_row::<String, _, _>(
            "SELECT id FROM session_caisse WHERE statut = 'ouverte' LIMIT 1",
            [], |r| r.get(0),
        ) {
            tx.execute(
                "INSERT INTO mouvement_caisse
                 (id, session_id, sens, moyen, montant, motif,
                  operation_id, date_mouvement, cree_le, cree_par, origine)
                 VALUES (?1,?2,'entree',?3,?4,'vente_facture',?5,?6,?7,?8,'app')",
                rusqlite::params![
                    uuid::Uuid::new_v4().to_string(), session_id,
                    mode_p, total_net, vente_id, now, now, auteur_id
                ],
            ).ok();
        }
    } else if let Some(ac) = acompte {
        if ac > 0 {
            let mode_p = mode_paiement.as_deref().unwrap_or("especes");
            tx.execute(
                "INSERT INTO paiement
                 (id, vente_id, montant, mode, date_paiement,
                  auteur_id, cree_le, cree_par, origine)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,'app')",
                rusqlite::params![
                    uuid::Uuid::new_v4().to_string(),
                    vente_id, ac, mode_p, now,
                    auteur_id, now, auteur_id
                ],
            ).map_err(|e| e.to_string())?;

            let statut_v = if ac >= total_net { "payee" } else { "partiellement_payee" };
            tx.execute(
                "UPDATE vente SET statut = ?1, modifie_le = ?2 WHERE id = ?3",
                rusqlite::params![statut_v, now, vente_id],
            ).map_err(|e| e.to_string())?;
        }
    }

    // 5. Marquer la facture pièce comme validée + lier à la vente
    tx.execute(
        "UPDATE piece_commerciale
         SET statut = 'validee', modifie_le = ?1 WHERE id = ?2",
        rusqlite::params![now, piece_id],
    ).map_err(|e| e.to_string())?;

    tx.commit().map_err(|e| e.to_string())?;

    Ok(serde_json::json!({
        "vente_id":      vente_id,
        "numero_facture": numero_fac,
        "total_net":     total_net,
        "statut_vente":  if mode_reglement == "comptant" { "payee" } else { "creance_ouverte" },
    }))
}

// =====================================================================
//  Changer le statut d'une pièce
// =====================================================================

#[tauri::command]
pub fn changer_statut_piece(
    etat: State<EtatApp>,
    piece_id: String,
    nouveau_statut: String,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE piece_commerciale SET statut = ?1, modifie_le = ?2
         WHERE id = ?3",
        rusqlite::params![nouveau_statut, maintenant_iso(), piece_id],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

// =====================================================================
//  Données complètes d'une pièce (pour impression)
// =====================================================================

#[tauri::command]
pub fn lire_donnees_piece(
    etat: State<EtatApp>,
    piece_id: String,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let piece = conn.query_row(
        "SELECT pc.id, pc.type_piece, pc.numero, pc.statut,
                pc.date_piece, pc.date_echeance, pc.remise_globale, pc.note,
                c.id, c.nom, c.code, c.telephone, c.adresse, c.nif,
                u.nom as auteur_nom
         FROM piece_commerciale pc
         JOIN client c ON c.id = pc.tiers_id
         LEFT JOIN utilisateur u ON u.id = pc.auteur_id
         WHERE pc.id = ?1",
        rusqlite::params![piece_id],
        |row| Ok(serde_json::json!({
            "id":             row.get::<_,String>(0)?,
            "type_piece":     row.get::<_,String>(1)?,
            "numero":         row.get::<_,String>(2)?,
            "statut":         row.get::<_,String>(3)?,
            "date_piece":     row.get::<_,String>(4)?,
            "date_echeance":  row.get::<_,Option<String>>(5)?,
            "remise_globale": row.get::<_,f64>(6).unwrap_or(0.0),
            "note":           row.get::<_,Option<String>>(7)?,
            "client_id":      row.get::<_,String>(8)?,
            "client_nom":     row.get::<_,String>(9)?,
            "client_code":    row.get::<_,String>(10)?,
            "client_telephone": row.get::<_,Option<String>>(11)?,
            "client_adresse": row.get::<_,Option<String>>(12)?,
            "client_nif":     row.get::<_,Option<String>>(13)?,
            "auteur_nom":     row.get::<_,Option<String>>(14)?,
        }))
    ).map_err(|e| e.to_string())?;

    let mut stmt = conn.prepare(
        "SELECT a.nom, uv.libelle, lp.quantite, lp.prix_unitaire,
                lp.remise_pct, lp.remise_montant,
                lp.taux_tva, lp.montant_tva, lp.montant_ht
         FROM ligne_piece lp
         JOIN article a ON a.id = lp.article_id
         JOIN unite_vente uv ON uv.id = lp.unite_vente_id
         WHERE lp.piece_id = ?1 ORDER BY lp.cree_le"
    ).map_err(|e| e.to_string())?;

    let lignes: Vec<serde_json::Value> = stmt.query_map(
        rusqlite::params![piece_id], |row| {
            let taux_tva: f64 = row.get(6)?;
            let montant_ht: i64 = row.get(8)?;
            // Recalculer montant_tva depuis taux si montant_tva = 0
            let montant_tva_raw: i64 = row.get(7)?;
            let montant_tva = if montant_tva_raw == 0 && taux_tva > 0.0 {
                // Filet de securite : TVA ajoutee au HT.
                (montant_ht as f64 * taux_tva).round() as i64
            } else {
                montant_tva_raw
            };
            Ok(serde_json::json!({
                "article_nom":    row.get::<_,String>(0)?,
                "unite_libelle":  row.get::<_,String>(1)?,
                "quantite":       row.get::<_,f64>(2)?,
                "prix_unitaire":  row.get::<_,i64>(3)?,
                "remise_pct":     row.get::<_,f64>(4)?,
                "remise_montant": row.get::<_,i64>(5)?,
                "taux_tva":       taux_tva,
                "montant_tva":    montant_tva,
                "montant_ht":     montant_ht,
            }))
        }
    ).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    let total_ht: i64 = lignes.iter()
        .filter_map(|l| l["montant_ht"].as_i64()).sum();
    let remise_g = piece["remise_globale"].as_f64().unwrap_or(0.0);
    let remise_mt = (total_ht as f64 * remise_g / 100.0).round() as i64;
    let total_net = total_ht - remise_mt;

    // Le TTC imprime doit etre STRICTEMENT celui que valider_facture
    // enregistrera comme montant du. On reproduit donc ici la meme suite
    // d'operations : remise globale par ligne -> TVA -> prix unitaire TTC
    // arrondi -> troncature de la somme (comme le CAST de SQLite).
    // Toute divergence, meme d'un franc, laisse une creance residuelle.
    let mut total_tva: i64 = 0;
    let mut somme_ttc: f64 = 0.0;
    for l in &lignes {
        let qte      = l["quantite"].as_f64().unwrap_or(0.0);
        let ht_ligne = l["montant_ht"].as_i64().unwrap_or(0);
        let taux     = l["taux_tva"].as_f64().unwrap_or(0.0);

        let remise_g_ligne = (ht_ligne as f64 * remise_g / 100.0).round() as i64;
        let ht  = ht_ligne - remise_g_ligne;
        let tva = (ht as f64 * taux).round() as i64;
        total_tva += tva;

        let ttc = ht + tva;
        let pu_ttc = if qte > 0.0 { (ttc as f64 / qte).round() as i64 } else { ttc };
        somme_ttc += pu_ttc as f64 * qte;
    }
    let total_ttc = somme_ttc as i64;

    let societe = conn.query_row(
        "SELECT nom, adresse, telephone, telephone2, email, nif, rccm,
                pied_facture, devise
         FROM parametres_societe WHERE id = 1",
        [], |row| Ok(serde_json::json!({
            "nom":        row.get::<_,String>(0)?,
            "adresse":    row.get::<_,Option<String>>(1)?,
            "telephone":  row.get::<_,Option<String>>(2)?,
            "telephone2": row.get::<_,Option<String>>(3)?,
            "email":      row.get::<_,Option<String>>(4)?,
            "nif":        row.get::<_,Option<String>>(5)?,
            "rccm":       row.get::<_,Option<String>>(6)?,
            "pied_facture": row.get::<_,Option<String>>(7)?,
            "devise":     row.get::<_,String>(8).unwrap_or("FCFA".to_string()),
        }))
    ).unwrap_or(serde_json::json!({"nom":"Ma Société","devise":"FCFA"}));

    Ok(serde_json::json!({
        "piece":   piece,
        "lignes":  lignes,
        "societe": societe,
        "totaux": {
            "total_ht":       total_ht,
            "total_tva":      total_tva,
            "remise_globale": remise_g,
            "remise_montant": remise_mt,
            "total_net":      total_net,
            "total_ttc":      total_ttc,
        }
    }))
}

// =====================================================================
//  Fiche client — résumé complet
// =====================================================================

#[tauri::command]
pub fn lire_fiche_client(
    etat: State<EtatApp>,
    client_id: String,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let client = conn.query_row(
        "SELECT id, code, nom, telephone, adresse, nif, email, cree_le
         FROM client WHERE id = ?1",
        rusqlite::params![client_id],
        |row| Ok(serde_json::json!({
            "id":        row.get::<_,String>(0)?,
            "code":      row.get::<_,String>(1)?,
            "nom":       row.get::<_,String>(2)?,
            "telephone": row.get::<_,Option<String>>(3)?,
            "adresse":   row.get::<_,Option<String>>(4)?,
            "nif":       row.get::<_,Option<String>>(5)?,
            "email":     row.get::<_,Option<String>>(6)?,
            "cree_le":   row.get::<_,String>(7)?,
        }))
    ).map_err(|e| e.to_string())?;

    let (ca_total, nb_ventes): (i64, i64) = conn.query_row(
        "SELECT
            CAST(COALESCE(SUM(
              (SELECT COALESCE(SUM(prix_pratique * quantite), 0)
               FROM ligne_vente WHERE vente_id = v.id)
            ), 0) AS INTEGER),
            COUNT(*)
         FROM vente v WHERE v.client_id = ?1 AND v.statut != 'annulee'",
        rusqlite::params![client_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    ).unwrap_or((0, 0));

    let encours: i64 = conn.query_row(
        "SELECT CAST(COALESCE(SUM(
            (SELECT COALESCE(SUM(prix_pratique * quantite), 0)
             FROM ligne_vente WHERE vente_id = v.id) -
            (SELECT COALESCE(SUM(montant), 0)
             FROM paiement WHERE vente_id = v.id)
         ), 0) AS INTEGER)
         FROM vente v
         WHERE v.client_id = ?1
           AND v.statut IN ('creance_ouverte','partiellement_payee')",
        rusqlite::params![client_id],
        |r| r.get(0),
    ).unwrap_or(0);

    let avoirs_total: i64 = conn.query_row(
        "SELECT CAST(COALESCE(SUM(montant), 0) AS INTEGER)
         FROM avoir WHERE client_id = ?1 AND statut = 'ouvert'",
        rusqlite::params![client_id],
        |r| r.get(0),
    ).unwrap_or(0);

    let nb_pieces: i64 = conn.query_row(
        "SELECT COUNT(*) FROM piece_commerciale
         WHERE tiers_id = ?1 AND tiers_type = 'client'",
        rusqlite::params![client_id],
        |r| r.get(0),
    ).unwrap_or(0);

    let derniere_vente: Option<String> = conn.query_row(
        "SELECT date_vente FROM vente
         WHERE client_id = ?1 ORDER BY date_vente DESC LIMIT 1",
        rusqlite::params![client_id],
        |r| r.get(0),
    ).ok();

    Ok(serde_json::json!({
        "client": client,
        "stats": {
            "ca_total":       ca_total,
            "nb_ventes":      nb_ventes,
            "encours":        encours,
            "avoirs_total":   avoirs_total,
            "nb_pieces":      nb_pieces,
            "derniere_vente": derniere_vente,
        }
    }))
}

// =====================================================================
//  Proxy impression
// =====================================================================

#[tauri::command]
pub fn imprimer_piece(
    html: String,
    nom_fichier: Option<String>,
) -> Result<String, String> {
    crate::commandes::impression::imprimer_facture(html, nom_fichier)
}

// =====================================================================
//  Lire TOUTES les pièces fournisseur (page Pièces)
// =====================================================================

#[tauri::command]
pub fn lire_toutes_pieces_fournisseur(
    etat: State<EtatApp>,
    type_filtre: Option<String>,
    statut: Option<String>,
    recherche: Option<String>,
    fournisseur_id: Option<String>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let mut conditions = vec!["pc.tiers_type = 'fournisseur'".to_string()];

    if let Some(ref tf) = type_filtre {
        conditions.push(format!("pc.type_piece = '{}'", tf.replace('\'', "''")));
    }
    if let Some(ref s) = statut {
        conditions.push(format!("pc.statut = '{}'", s.replace('\'', "''")));
    }
    if let Some(ref r) = recherche {
        let r = r.replace('\'', "''");
        conditions.push(format!(
            "(pc.numero LIKE '%{r}%' OR f.nom LIKE '%{r}%')"
        ));
    }
    if let Some(ref fid) = fournisseur_id {
        conditions.push(format!("pc.tiers_id = '{}'", fid.replace('\'', "''")));
    }

    let where_clause = conditions.join(" AND ");

    let sql = format!(
        "SELECT pc.id, pc.type_piece, pc.numero, pc.statut,
                pc.date_piece, pc.date_echeance, pc.remise_globale, pc.note,
                f.id as tiers_id, f.nom as tiers_nom,
                COALESCE(f.telephone, '') as tiers_code,
                CAST(COALESCE(
                  (SELECT SUM(lp.montant_ht) FROM ligne_piece lp WHERE lp.piece_id = pc.id), 0
                ) AS INTEGER) as total_ht,
                CAST(COALESCE(
                  (SELECT SUM(lp.montant_tva) FROM ligne_piece lp WHERE lp.piece_id = pc.id), 0
                ) AS INTEGER) as total_tva,
                u.nom as auteur_nom,
                pc.piece_origine_id
         FROM piece_commerciale pc
         JOIN fournisseur f ON f.id = pc.tiers_id
         LEFT JOIN utilisateur u ON u.id = pc.auteur_id
         WHERE {}
         ORDER BY pc.date_piece DESC, pc.cree_le DESC",
        where_clause
    );

    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;

    let x = stmt.query_map([], |row| {
        let total_ht: i64 = row.get(11)?;
        let total_tva: i64 = row.get(12)?;
        let remise_g: f64 = row.get(6).unwrap_or(0.0);
        let remise_mt = (total_ht as f64 * remise_g / 100.0).round() as i64;
        let total_net = total_ht - remise_mt;
        Ok(serde_json::json!({
            "id":               row.get::<_,String>(0)?,
            "type_piece":       row.get::<_,String>(1)?,
            "numero":           row.get::<_,String>(2)?,
            "statut":           row.get::<_,String>(3)?,
            "date_piece":       row.get::<_,String>(4)?,
            "date_echeance":    row.get::<_,Option<String>>(5)?,
            "remise_globale":   remise_g,
            "note":             row.get::<_,Option<String>>(7)?,
            "tiers_id":         row.get::<_,String>(8)?,
            "tiers_nom":        row.get::<_,String>(9)?,
            "tiers_code":       row.get::<_,String>(10)?,
            "total_ht":         total_ht,
            "total_tva":        total_tva,
            "remise_montant":   remise_mt,
            "total_net":        total_net,
            "total_ttc":        total_net + total_tva,
            "auteur_nom":       row.get::<_,Option<String>>(13)?,
            "piece_origine_id": row.get::<_,Option<String>>(14)?,
        }))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
    Ok(x)
}

// =====================================================================
//  Créer une pièce fournisseur
// =====================================================================

#[tauri::command]
pub fn creer_piece_fournisseur(
    etat: State<EtatApp>,
    fournisseur_id: String,
    type_piece: String,
    lignes: Vec<LignePieceInput>,
    remise_globale: Option<f64>,
    date_echeance: Option<String>,
    note: Option<String>,
    piece_origine_id: Option<String>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let auteur = crate::commandes::ventes::id_utilisateur_courant_pub(&conn);
    let now = crate::utils::maintenant_iso();
    let piece_id = uuid::Uuid::new_v4().to_string();
    let numero = prochain_numero(&conn, &type_piece);
    let remise_g = remise_globale.unwrap_or(0.0);

    let statut = match type_piece.as_str() {
        "bon_commande_fournisseur" => "emis",
        "bon_reception"            => "emis",
        "facture_fournisseur"      => "brouillon",
        "avoir_fournisseur"        => "emis",
        _                          => "brouillon",
    };

    conn.execute(
        "INSERT INTO piece_commerciale
         (id, type_piece, numero, statut, tiers_type, tiers_id,
          piece_origine_id, auteur_id, date_piece, date_echeance,
          remise_globale, note, cree_le, modifie_le, origine)
         VALUES (?1,?2,?3,?4,'fournisseur',?5,?6,?7,?8,?9,?10,?11,?12,?13,'app')",
        rusqlite::params![
            piece_id, type_piece, numero, statut,
            fournisseur_id, piece_origine_id, auteur,
            now, date_echeance, remise_g, note, now, now
        ],
    ).map_err(|e| e.to_string())?;

    inserer_lignes(&conn, &piece_id, &lignes, &now)?;

    Ok(serde_json::json!({
        "id": piece_id, "numero": numero, "statut": statut,
    }))
}
// Patch à ajouter à la fin de pieces.rs

// =====================================================================
//  Modifier une pièce (seulement si statut != validee/transfere/annule)
// =====================================================================

#[tauri::command]
pub fn modifier_piece(
    etat: State<EtatApp>,
    piece_id: String,
    note: Option<String>,
    date_echeance: Option<String>,
    remise_globale: Option<f64>,
    lignes: Option<Vec<LignePieceInput>>,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    // Vérifier statut
    let statut: String = conn.query_row(
        "SELECT statut FROM piece_commerciale WHERE id = ?1",
        rusqlite::params![piece_id],
        |r| r.get(0),
    ).map_err(|_| "Pièce introuvable".to_string())?;

    match statut.as_str() {
        "validee" => return Err("Pièce validée — non modifiable".to_string()),
        "transfere" => return Err("Pièce transférée — non modifiable".to_string()),
        "annule" => return Err("Pièce annulée — non modifiable".to_string()),
        _ => {}
    }

    let now = maintenant_iso();

    // Mettre à jour les champs
    conn.execute(
        "UPDATE piece_commerciale
         SET note = ?1, date_echeance = ?2,
             remise_globale = COALESCE(?3, remise_globale),
             modifie_le = ?4
         WHERE id = ?5",
        rusqlite::params![note, date_echeance, remise_globale, now, piece_id],
    ).map_err(|e| e.to_string())?;

    // Si nouvelles lignes fournies → remplacer
    if let Some(nouvelles_lignes) = lignes {
        conn.execute(
            "DELETE FROM ligne_piece WHERE piece_id = ?1",
            rusqlite::params![piece_id],
        ).map_err(|e| e.to_string())?;
        inserer_lignes(&conn, &piece_id, &nouvelles_lignes, &now)?;
    }

    Ok(())
}

// =====================================================================
//  Annuler une pièce
// =====================================================================

#[tauri::command]
pub fn annuler_piece(
    etat: State<EtatApp>,
    piece_id: String,
    motif: Option<String>,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let (statut, type_piece): (String, String) = conn.query_row(
        "SELECT statut, type_piece FROM piece_commerciale WHERE id = ?1",
        rusqlite::params![piece_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    ).map_err(|_| "Pièce introuvable".to_string())?;

    match statut.as_str() {
        "validee" => return Err("Pièce validée — impossible d'annuler".to_string()),
        "annule"  => return Err("Pièce déjà annulée".to_string()),
        _ => {}
    }

    let now = maintenant_iso();
    let auteur = crate::commandes::ventes::id_utilisateur_courant_pub(&conn);

    conn.execute(
        "UPDATE piece_commerciale
         SET statut = 'annule', note = COALESCE(?1, note), modifie_le = ?2
         WHERE id = ?3",
        rusqlite::params![motif, now, piece_id],
    ).map_err(|e| e.to_string())?;

    // Journaliser
    conn.execute(
        "INSERT INTO journal
         (id, type_evenement, entite_type, entite_id, auteur_id,
          nouveau_valeur, origine, date_evenement)
         VALUES (?1,'piece_annulee','piece_commerciale',?2,?3,?4,'app',?5)",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(),
            piece_id, auteur,
            format!(r#"{{"type":"{}","motif":"{}"}}"#,
                type_piece,
                motif.as_deref().unwrap_or("")),
            now
        ],
    ).ok();

    Ok(())
}

// =====================================================================
//  Dupliquer une pièce (créer une copie en brouillon)
// =====================================================================

#[tauri::command]
pub fn dupliquer_piece(
    etat: State<EtatApp>,
    piece_id: String,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let (client_id, type_piece, remise_g, date_ech, note): 
        (String, String, f64, Option<String>, Option<String>) =
        conn.query_row(
            "SELECT tiers_id, type_piece, remise_globale, date_echeance, note
             FROM piece_commerciale WHERE id = ?1",
            rusqlite::params![piece_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        ).map_err(|_| "Pièce introuvable".to_string())?;

    let lignes = lire_lignes_raw(&conn, &piece_id)?;
    let now = maintenant_iso();
    let auteur = crate::commandes::ventes::id_utilisateur_courant_pub(&conn);
    let nouvelle_id = uuid::Uuid::new_v4().to_string();
    let numero = prochain_numero(&conn, &type_piece);

    conn.execute(
        "INSERT INTO piece_commerciale
         (id, type_piece, numero, statut, tiers_type, tiers_id,
          piece_origine_id, auteur_id, date_piece, date_echeance,
          remise_globale, note, cree_le, modifie_le, origine)
         VALUES (?1,?2,?3,'brouillon','client',?4,?5,?6,?7,?8,?9,?10,?11,?12,'app')",
        rusqlite::params![
            nouvelle_id, type_piece, numero,
            client_id, piece_id, auteur,
            now, date_ech, remise_g,
            note.map(|n| format!("[Copie] {}", n)).or(Some("[Copie]".to_string())),
            now, now
        ],
    ).map_err(|e| e.to_string())?;

    inserer_lignes_raw(&conn, &nouvelle_id, &lignes, &now)?;

    Ok(serde_json::json!({
        "id": nouvelle_id, "numero": numero, "statut": "brouillon"
    }))
}