//! Pièces commerciales — Devis, Commande, BL, Facture, Avoir client.
//! Cycle : devis/proforma → commande_client → bon_livraison → facture → avoir_client

use tauri::State;
use crate::commandes::ventes::EtatApp;
use crate::utils::maintenant_iso;

// =====================================================================
//  Types de pièces et statuts
// =====================================================================
//
// type_piece :
//   devis | proforma | commande_client | bon_livraison
//   facture | facture_acompte | avoir_client
//
// statut :
//   brouillon | emis | accepte | refuse
//   partiellement_livre | livre | facture | paye | annule

// =====================================================================
//  Numérotation séquentielle par type et par an
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
//  Lire pièces d'un client
// =====================================================================

#[tauri::command]
pub fn lire_pieces_client(
    etat: State<EtatApp>,
    client_id: String,
    type_filtre: Option<String>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    // Toujours 2 paramètres — filtre = "%" si absent (LIKE '%' = tout)
    let filtre_val = type_filtre.unwrap_or_else(|| "%".to_string());
    let op = if filtre_val == "%" { "LIKE" } else { "=" };

    let sql = format!(
        "SELECT pc.id, pc.type_piece, pc.numero, pc.statut,
                pc.date_piece, pc.date_echeance, pc.remise_globale,
                pc.note, pc.cree_le,
                pc.piece_origine_id,
                CAST(COALESCE(
                  (SELECT SUM(lp.montant_ht) FROM ligne_piece lp WHERE lp.piece_id = pc.id)
                , 0) AS INTEGER) as total_ht,
                CAST(COALESCE(
                  (SELECT SUM(lp.montant_tva) FROM ligne_piece lp WHERE lp.piece_id = pc.id)
                , 0) AS INTEGER) as total_tva,
                u.nom as auteur_nom
         FROM piece_commerciale pc
         LEFT JOIN utilisateur u ON u.id = pc.auteur_id
         WHERE pc.tiers_id = ?1 AND pc.tiers_type = 'client'
           AND pc.type_piece {} ?2
         ORDER BY pc.date_piece DESC, pc.cree_le DESC",
        op
    );

    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;

    let rows: Vec<serde_json::Value> = stmt
        .query_map(rusqlite::params![client_id, filtre_val], |row| {
            mapper_piece(row)
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    Ok(rows)
}

fn mapper_piece(row: &rusqlite::Row) -> rusqlite::Result<serde_json::Value> {
    let total_ht: i64 = row.get(10)?;
    let total_tva: i64 = row.get(11)?;
    let remise_globale: f64 = row.get(6).unwrap_or(0.0);
    let remise_mt = (total_ht as f64 * remise_globale / 100.0).round() as i64;
    Ok(serde_json::json!({
        "id":              row.get::<_,String>(0)?,
        "type_piece":      row.get::<_,String>(1)?,
        "numero":          row.get::<_,String>(2)?,
        "statut":          row.get::<_,String>(3)?,
        "date_piece":      row.get::<_,String>(4)?,
        "date_echeance":   row.get::<_,Option<String>>(5)?,
        "remise_globale":  remise_globale,
        "note":            row.get::<_,Option<String>>(7)?,
        "cree_le":         row.get::<_,String>(8)?,
        "piece_origine_id":row.get::<_,Option<String>>(9)?,
        "total_ht":        total_ht,
        "total_tva":       total_tva,
        "remise_montant":  remise_mt,
        "total_net":       total_ht - remise_mt,
        "total_ttc":       total_ht - remise_mt + total_tva,
        "auteur_nom":      row.get::<_,Option<String>>(12)?,
    }))
}

// =====================================================================
//  Lire TOUTES les pièces client (page Pièces globale style Ciel)
// =====================================================================

#[tauri::command]
pub fn lire_toutes_pieces_client(
    etat: State<EtatApp>,
    type_filtre: Option<String>,
    statut: Option<String>,
    recherche: Option<String>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let mut conditions = vec!["pc.tiers_type = 'client'".to_string()];

    if let Some(ref tf) = type_filtre {
        // devis inclut proforma
        if tf == "devis" {
            conditions.push("pc.type_piece IN ('devis','proforma')".to_string());
        } else {
            conditions.push(format!("pc.type_piece = '{}'", tf.replace('\'', "''")));
        }
    }
    if let Some(ref s) = statut {
        conditions.push(format!("pc.statut = '{}'", s.replace('\'', "''")));
    }
    if let Some(ref r) = recherche {
        let r = r.replace('\'', "''");
        conditions.push(format!(
            "(pc.numero LIKE '%{}%' OR c.nom LIKE '%{}%' OR c.code LIKE '%{}%')",
            r, r, r
        ));
    }

    let where_clause = conditions.join(" AND ");

    let sql = format!(
        "SELECT pc.id, pc.type_piece, pc.numero, pc.statut,
                pc.date_piece, pc.date_echeance, pc.remise_globale, pc.note,
                c.id as tiers_id, c.nom as tiers_nom, c.code as tiers_code,
                CAST(COALESCE(
                  (SELECT SUM(lp.montant_ht) FROM ligne_piece lp WHERE lp.piece_id = pc.id)
                , 0) AS INTEGER) as total_ht,
                CAST(COALESCE(
                  (SELECT SUM(lp.montant_tva) FROM ligne_piece lp WHERE lp.piece_id = pc.id)
                , 0) AS INTEGER) as total_tva,
                u.nom as auteur_nom
         FROM piece_commerciale pc
         JOIN client c ON c.id = pc.tiers_id
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
            "id":              row.get::<_,String>(0)?,
            "type_piece":      row.get::<_,String>(1)?,
            "numero":          row.get::<_,String>(2)?,
            "statut":          row.get::<_,String>(3)?,
            "date_piece":      row.get::<_,String>(4)?,
            "date_echeance":   row.get::<_,Option<String>>(5)?,
            "remise_globale":  remise_g,
            "note":            row.get::<_,Option<String>>(7)?,
            "tiers_id":        row.get::<_,String>(8)?,
            "tiers_nom":       row.get::<_,String>(9)?,
            "tiers_code":      row.get::<_,String>(10)?,
            "total_ht":        total_ht,
            "total_tva":       total_tva,
            "remise_montant":  remise_mt,
            "total_net":       total_net,
            "total_ttc":       total_net + total_tva,
            "auteur_nom":      row.get::<_,Option<String>>(13)?,
        }))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    Ok(x)
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
        "SELECT lp.id, lp.article_id, a.nom as article_nom,
                lp.unite_vente_id, uv.libelle as unite_libelle,
                lp.quantite, lp.prix_unitaire,
                lp.remise_pct, lp.remise_montant,
                lp.taux_tva, lp.montant_tva, lp.montant_ht
         FROM ligne_piece lp
         JOIN article a ON a.id = lp.article_id
         JOIN unite_vente uv ON uv.id = lp.unite_vente_id
         WHERE lp.piece_id = ?1
         ORDER BY lp.cree_le"
    ).map_err(|e| e.to_string())?;

    let x = stmt.query_map(rusqlite::params![piece_id], |row| {
        Ok(serde_json::json!({
            "id":            row.get::<_,String>(0)?,
            "article_id":    row.get::<_,String>(1)?,
            "article_nom":   row.get::<_,String>(2)?,
            "unite_id":      row.get::<_,String>(3)?,
            "unite_libelle": row.get::<_,String>(4)?,
            "quantite":      row.get::<_,f64>(5)?,
            "prix_unitaire": row.get::<_,i64>(6)?,
            "remise_pct":    row.get::<_,f64>(7)?,
            "remise_montant":row.get::<_,i64>(8)?,
            "taux_tva":      row.get::<_,f64>(9)?,
            "montant_tva":   row.get::<_,i64>(10)?,
            "montant_ht":    row.get::<_,i64>(11)?,
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

    let statut = match type_piece.as_str() {
        "devis" | "proforma" => "brouillon",
        "commande_client"    => "accepte",
        "bon_livraison"      => "livre",
        "facture"            => "emis",
        "avoir_client"       => "emis",
        _                    => "brouillon",
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

    // Insérer les lignes
    for ligne in &lignes {
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

    Ok(serde_json::json!({
        "id":     piece_id,
        "numero": numero,
        "statut": statut,
    }))
}

// =====================================================================
//  Convertir une pièce vers la suivante
// =====================================================================

#[tauri::command]
pub fn convertir_piece(
    etat: State<EtatApp>,
    piece_id: String,
    nouveau_type: String,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    // Vérifier la pièce source
    let (client_id, type_src, remise_g, date_ech, note): (String, String, f64, Option<String>, Option<String>) =
        conn.query_row(
            "SELECT tiers_id, type_piece, remise_globale, date_echeance, note
             FROM piece_commerciale WHERE id = ?1",
            rusqlite::params![piece_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        ).map_err(|_| "Pièce introuvable".to_string())?;

    // Vérifier la transition autorisée
    let transitions_ok = [
        ("devis",           "commande_client"),
        ("proforma",        "commande_client"),
        ("commande_client", "bon_livraison"),
        ("bon_livraison",   "facture"),
        ("facture",         "avoir_client"),
    ];
    let ok = transitions_ok.iter().any(|(src, dst)| {
        *src == type_src.as_str() && *dst == nouveau_type.as_str()
    });
    if !ok {
        return Err(format!("Conversion {} → {} non autorisée", type_src, nouveau_type));
    }

    // Lire les lignes de la pièce source
    let mut stmt = conn.prepare(
        "SELECT article_id, unite_vente_id, quantite, prix_unitaire,
                remise_pct, taux_tva
         FROM ligne_piece WHERE piece_id = ?1"
    ).map_err(|e| e.to_string())?;

    let lignes: Vec<LignePieceInput> = stmt.query_map(
        rusqlite::params![piece_id], |row| {
            Ok(LignePieceInput {
                article_id:    row.get(0)?,
                unite_vente_id:row.get(1)?,
                quantite:      row.get(2)?,
                prix_unitaire: row.get(3)?,
                remise_pct:    row.get(4)?,
                taux_tva:      row.get(5)?,
            })
        }
    ).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    // Archiver la pièce source
    let now = maintenant_iso();
    let statut_archive = match type_src.as_str() {
        "devis" | "proforma" => "accepte",
        "commande_client"    => "livre",
        "bon_livraison"      => "facture",
        "facture"            => "annule",
        _                    => "annule",
    };
    conn.execute(
        "UPDATE piece_commerciale SET statut = ?1, modifie_le = ?2 WHERE id = ?3",
        rusqlite::params![statut_archive, now, piece_id],
    ).map_err(|e| e.to_string())?;

    // Créer la nouvelle pièce
    let auteur = crate::commandes::ventes::id_utilisateur_courant_pub(&conn);
    let nouvelle_id = uuid::Uuid::new_v4().to_string();
    let numero = prochain_numero(&conn, &nouveau_type);
    let statut_nouveau = match nouveau_type.as_str() {
        "commande_client" => "accepte",
        "bon_livraison"   => "livre",
        "facture"         => "emis",
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

    for ligne in &lignes {
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
                nouvelle_id, ligne.article_id, ligne.unite_vente_id,
                ligne.quantite, ligne.prix_unitaire,
                ligne.remise_pct, remise_mt,
                ligne.taux_tva, montant_tva, montant_ht, now
            ],
        ).map_err(|e| e.to_string())?;
    }

    Ok(serde_json::json!({
        "id":     nouvelle_id,
        "numero": numero,
        "statut": statut_nouveau,
        "type_piece": nouveau_type,
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
        "UPDATE piece_commerciale SET statut = ?1, modifie_le = ?2 WHERE id = ?3",
        rusqlite::params![nouveau_statut, maintenant_iso(), piece_id],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

// =====================================================================
//  Lire les données complètes d'une pièce (pour impression)
// =====================================================================

#[tauri::command]
pub fn lire_donnees_piece(
    etat: State<EtatApp>,
    piece_id: String,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    // Pièce
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
            "id":            row.get::<_,String>(0)?,
            "type_piece":    row.get::<_,String>(1)?,
            "numero":        row.get::<_,String>(2)?,
            "statut":        row.get::<_,String>(3)?,
            "date_piece":    row.get::<_,String>(4)?,
            "date_echeance": row.get::<_,Option<String>>(5)?,
            "remise_globale":row.get::<_,f64>(6).unwrap_or(0.0),
            "note":          row.get::<_,Option<String>>(7)?,
            "client_id":     row.get::<_,String>(8)?,
            "client_nom":    row.get::<_,String>(9)?,
            "client_code":   row.get::<_,String>(10)?,
            "client_telephone": row.get::<_,Option<String>>(11)?,
            "client_adresse":row.get::<_,Option<String>>(12)?,
            "client_nif":    row.get::<_,Option<String>>(13)?,
            "auteur_nom":    row.get::<_,Option<String>>(14)?,
        }))
    ).map_err(|e| e.to_string())?;

    // Lignes
    let mut stmt = conn.prepare(
        "SELECT a.nom, uv.libelle, lp.quantite, lp.prix_unitaire,
                lp.remise_pct, lp.remise_montant, lp.taux_tva,
                lp.montant_tva, lp.montant_ht
         FROM ligne_piece lp
         JOIN article a ON a.id = lp.article_id
         JOIN unite_vente uv ON uv.id = lp.unite_vente_id
         WHERE lp.piece_id = ?1 ORDER BY lp.cree_le"
    ).map_err(|e| e.to_string())?;

    let lignes: Vec<serde_json::Value> = stmt.query_map(
        rusqlite::params![piece_id], |row| {
            Ok(serde_json::json!({
                "article_nom":   row.get::<_,String>(0)?,
                "unite_libelle": row.get::<_,String>(1)?,
                "quantite":      row.get::<_,f64>(2)?,
                "prix_unitaire": row.get::<_,i64>(3)?,
                "remise_pct":    row.get::<_,f64>(4)?,
                "remise_montant":row.get::<_,i64>(5)?,
                "taux_tva":      row.get::<_,f64>(6)?,
                "montant_tva":   row.get::<_,i64>(7)?,
                "montant_ht":    row.get::<_,i64>(8)?,
            }))
        }
    ).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    // Totaux
    let total_ht: i64 = lignes.iter()
        .filter_map(|l| l["montant_ht"].as_i64()).sum();
    let total_tva: i64 = lignes.iter()
        .filter_map(|l| l["montant_tva"].as_i64()).sum();
    let remise_g: f64 = piece["remise_globale"].as_f64().unwrap_or(0.0);
    let remise_mt = (total_ht as f64 * remise_g / 100.0).round() as i64;
    let total_net = total_ht - remise_mt;
    let total_ttc = total_net + total_tva;

    // Société
    let societe = conn.query_row(
        "SELECT nom, adresse, telephone, telephone2, email, nif, rccm,
                pied_facture, devise
         FROM parametres_societe WHERE id = 1",
        [], |row| Ok(serde_json::json!({
            "nom":       row.get::<_,String>(0)?,
            "adresse":   row.get::<_,Option<String>>(1)?,
            "telephone": row.get::<_,Option<String>>(2)?,
            "telephone2":row.get::<_,Option<String>>(3)?,
            "email":     row.get::<_,Option<String>>(4)?,
            "nif":       row.get::<_,Option<String>>(5)?,
            "rccm":      row.get::<_,Option<String>>(6)?,
            "pied_facture":row.get::<_,Option<String>>(7)?,
            "devise":    row.get::<_,String>(8).unwrap_or("FCFA".to_string()),
        }))
    ).unwrap_or(serde_json::json!({"nom":"Ma Société","devise":"FCFA"}));

    Ok(serde_json::json!({
        "piece":     piece,
        "lignes":    lignes,
        "societe":   societe,
        "totaux": {
            "total_ht":        total_ht,
            "total_tva":       total_tva,
            "remise_globale":  remise_g,
            "remise_montant":  remise_mt,
            "total_net":       total_net,
            "total_ttc":       total_ttc,
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

    // Infos client
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

    // Stats ventes
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

    // Encours créances
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

    // Avoirs disponibles
    let avoirs_total: i64 = conn.query_row(
        "SELECT CAST(COALESCE(SUM(montant), 0) AS INTEGER)
         FROM avoir WHERE client_id = ?1 AND statut = 'ouvert'",
        rusqlite::params![client_id],
        |r| r.get(0),
    ).unwrap_or(0);

    // Nb pièces
    let nb_pieces: i64 = conn.query_row(
        "SELECT COUNT(*) FROM piece_commerciale
         WHERE tiers_id = ?1 AND tiers_type = 'client'",
        rusqlite::params![client_id],
        |r| r.get(0),
    ).unwrap_or(0);

    // Dernière vente
    let derniere_vente: Option<String> = conn.query_row(
        "SELECT date_vente FROM vente
         WHERE client_id = ?1 ORDER BY date_vente DESC LIMIT 1",
        rusqlite::params![client_id],
        |r| r.get(0),
    ).ok();

    Ok(serde_json::json!({
        "client":          client,
        "stats": {
            "ca_total":        ca_total,
            "nb_ventes":       nb_ventes,
            "encours":         encours,
            "avoirs_total":    avoirs_total,
            "nb_pieces":       nb_pieces,
            "derniere_vente":  derniere_vente,
        }
    }))
}

// =====================================================================
//  Imprimer une pièce — génère HTML et ouvre le navigateur
// =====================================================================

#[tauri::command]
pub fn imprimer_piece(
    html: String,
    nom_fichier: Option<String>,
) -> Result<String, String> {
    crate::commandes::impression::imprimer_facture(html, nom_fichier)
}