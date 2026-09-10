//! Création automatique de facture depuis le POS.
//! Appelé juste après creer_vente pour lier vente ↔ pièce commerciale.

use crate::utils::maintenant_iso;

/// Crée automatiquement une pièce commerciale "facture" liée à une vente POS.
/// - Comptant  → statut "validee" (non modifiable)
/// - Crédit    → statut "emis"    (modifiable jusqu'au règlement)
pub fn creer_facture_depuis_vente(
    conn: &rusqlite::Connection,
    vente_id: String,
    client_id: String,
    mode_reglement: String,
    utilisateur_role: Option<String>,
) -> Result<serde_json::Value, String> {
    creer_facture_depuis_vente_sur(
        &conn, vente_id, client_id, mode_reglement, utilisateur_role,
    )
}

/// Logique de `creer_facture_depuis_vente`, sur une connexion quelconque.
///
/// Separee de la commande pour etre jouable sur une base de test :
/// les scenarios de `tests_multi_depot` verifient ce que le SQL fait
/// reellement a la base, ce qu'aucun test de formule ne montre.
// Le corps a demenage dans `noyau::argent` : la facture POS est
// une ecriture d'argent, le serveur v2 doit pouvoir la produire.
pub(crate) use crate::argent::creer_facture_depuis_vente_sur;

/// Modifier une facture POS en statut "emis" (crédit non encore réglé).
/// Interdit si statut = "validee".
pub fn modifier_facture_pos(
    conn: &rusqlite::Connection,
    piece_id: String,
    note: Option<String>,
    date_echeance: Option<String>,
) -> Result<(), String> {

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
pub fn valider_facture_credit(
    conn: &rusqlite::Connection,
    piece_id: String,
) -> Result<(), String> {

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
