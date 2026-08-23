//! Création automatique de facture depuis le POS.
//! Appelé juste après creer_vente pour lier vente ↔ pièce commerciale.

use tauri::State;
use crate::commandes::ventes::EtatApp;
use crate::utils::maintenant_iso;

/// Crée automatiquement une pièce commerciale "facture" liée à une vente POS.
/// - Comptant  → statut "validee" (non modifiable)
/// - Crédit    → statut "emis"    (modifiable jusqu'au règlement)
#[tauri::command]
pub fn creer_facture_depuis_vente(
    etat: State<EtatApp>,
    vente_id: String,
    client_id: String,
    mode_reglement: String,
    utilisateur_role: Option<String>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let role = utilisateur_role.as_deref().unwrap_or("employe");
    let auteur_id = crate::commandes::ventes::id_utilisateur_par_role(&conn, role);
    let now = maintenant_iso();

    // Numérotation — meme fonction que les pieces saisies manuellement.
    // Avant : COUNT(*) local, en parallele de prochain_numero. Deux
    // compteurs pour la meme sequence FAC- = collision sur numero UNIQUE
    // des qu'une facture est creee des deux cotes.
    let numero = crate::commandes::pieces::prochain_numero(&conn, "facture");
    let piece_id = uuid::Uuid::new_v4().to_string();

    // Statut selon mode règlement
    let statut = if mode_reglement == "comptant" { "validee" } else { "emis" };

    // Créer la pièce
    conn.execute(
        "INSERT INTO piece_commerciale
         (id, type_piece, numero, statut, tiers_type, tiers_id,
          auteur_id, date_piece, remise_globale, note, cree_le, modifie_le, origine)
         VALUES (?1,'facture',?2,?3,'client',?4,?5,?6,0.0,'Vente POS',?7,?8,'pos')",
        rusqlite::params![
            piece_id, numero, statut,
            client_id, auteur_id, now, now, now
        ],
    ).map_err(|e| e.to_string())?;

    // Copier les lignes de vente → lignes de pièce
    let mut stmt = conn.prepare(
        "SELECT lv.article_id, lv.unite_vente_id, lv.quantite,
                lv.prix_pratique,
                CAST(COALESCE(lv.montant_tva, 0) AS INTEGER),
                CAST(COALESCE(lv.taux_tva, 0.0) AS REAL)
         FROM ligne_vente lv
         WHERE lv.vente_id = ?1"
    ).map_err(|e| e.to_string())?;

    let lignes: Vec<(String, String, f64, i64, i64, f64)> = stmt
        .query_map(rusqlite::params![vente_id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?,
                r.get(3)?, r.get(4)?, r.get(5)?))
        }).map_err(|e| e.to_string())?
        .filter_map(|r| r.ok()).collect();

    for (art_id, uv_id, qte, prix, tva, taux_tva) in &lignes {
        // prix_pratique est du TTC (D8). Une ligne_piece stocke du HT,
        // comme les pieces saisies manuellement -> on convertit.
        let montant_ttc = (*prix as f64 * qte).round() as i64;
        let montant_ht  = montant_ttc - *tva;
        let prix_ht = if *qte > 0.0 {
            (montant_ht as f64 / qte).round() as i64
        } else {
            *prix
        };
        conn.execute(
            "INSERT INTO ligne_piece
             (id, piece_id, article_id, unite_vente_id, quantite,
              prix_unitaire, remise_pct, remise_montant,
              taux_tva, montant_tva, montant_ht, cree_le)
             VALUES (?1,?2,?3,?4,?5,?6,0.0,0,?7,?8,?9,?10)",
            rusqlite::params![
                uuid::Uuid::new_v4().to_string(),
                piece_id, art_id, uv_id, qte,
                prix_ht, taux_tva, tva, montant_ht, now
            ],
        ).map_err(|e| e.to_string())?;
    }

    // Lier la vente à la pièce via colonne optionnelle
    if let Err(e) = conn.execute(
        "UPDATE vente SET piece_id = ?1 WHERE id = ?2",
        rusqlite::params![piece_id, vente_id],
    ) {
        // Non bloquant : la piece est creee, seul le lien manque.
        // Si ce message apparait, la migration vente.piece_id n'est pas passee.
        eprintln!("[gescom] lien vente->piece non etabli : {}", e);
    }

    Ok(serde_json::json!({
        "piece_id": piece_id,
        "numero":   numero,
        "statut":   statut,
    }))
}

/// Modifier une facture POS en statut "emis" (crédit non encore réglé).
/// Interdit si statut = "validee".
#[tauri::command]
pub fn modifier_facture_pos(
    etat: State<EtatApp>,
    piece_id: String,
    note: Option<String>,
    date_echeance: Option<String>,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    // Exception assumee a l'immuabilite de coeur::pieces.
    //
    // Une facture emise est normalement figee. On tolere ici la retouche
    // de la NOTE et de la DATE D'ECHEANCE sur une facture credit non
    // encore validee : ces deux champs ne portent ni montant, ni TVA,
    // ni article. Ils n'ont donc aucun effet comptable.
    //
    // Toute modification touchant les lignes ou les montants doit passer
    // par modifier_piece, qui applique la regle complete.
    let statut: String = conn.query_row(
        "SELECT statut FROM piece_commerciale WHERE id = ?1",
        rusqlite::params![piece_id],
        |r| r.get(0),
    ).map_err(|_| "Facture introuvable".to_string())?;

    if matches!(statut.as_str(), "validee" | "paye" | "annule" | "transfere") {
        return Err(format!(
            "Facture en statut '{}' — non modifiable. Émettre un avoir.",
            statut
        ));
    }

    conn.execute(
        "UPDATE piece_commerciale
         SET note = ?1, date_echeance = ?2, modifie_le = ?3
         WHERE id = ?4",
        rusqlite::params![note, date_echeance, maintenant_iso(), piece_id],
    ).map_err(|e| e.to_string())?;

    Ok(())
}

/// Valider manuellement une facture crédit → devient "validee".
#[tauri::command]
pub fn valider_facture_credit(
    etat: State<EtatApp>,
    piece_id: String,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let statut: String = conn.query_row(
        "SELECT statut FROM piece_commerciale WHERE id = ?1",
        rusqlite::params![piece_id],
        |r| r.get(0),
    ).map_err(|_| "Facture introuvable".to_string())?;

    if statut == "validee" {
        return Err("Facture déjà validée".to_string());
    }

    conn.execute(
        "UPDATE piece_commerciale
         SET statut = 'validee', modifie_le = ?1 WHERE id = ?2",
        rusqlite::params![maintenant_iso(), piece_id],
    ).map_err(|e| e.to_string())?;

    Ok(())
}
