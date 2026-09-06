//! Suivi de livraison — un axe d'information, pas un evenement comptable.
//!
//! Ce module ne touche NI au stock NI a la caisse. La facture reste la
//! source : le stock sort a `valider_facture` (pieces.rs), l'argent aux
//! points d'argent habituels. Ici on ne repond qu'a une question que le
//! paiement ne pose pas — « qu'est-ce qui est deja parti chez le
//! client ? ».
//!
//! Paiement et livraison deviennent donc deux axes independants, ce qui
//! rend representable le cas courant « paye, pas encore livre » (et son
//! symetrique, « livre, pas encore paye »).
//!
//! Reglage desactive par defaut : la plupart des commercants vises
//! remettent la marchandise au comptoir, la livraison n'existe pas chez
//! eux et l'ecran ne doit pas s'encombrer. Meme principe que le bon de
//! sortie (parametres.rs).

use tauri::State;
use serde::Deserialize;
use crate::commandes::ventes::EtatApp;
use crate::utils::maintenant_iso;

/// Tolerance de comparaison sur des quantites en REAL.
///
/// `SUM(quantite_livree) >= SUM(quantite)` sur des flottants ne tombe
/// jamais juste apres quelques additions : sans marge, une commande
/// entierement livree resterait affichee « partiel » pour un
/// milliardieme d'unite.
const EPSILON: f64 = 0.0001;

/// Etat de livraison derive des quantites, jamais stocke.
///
/// Un statut stocke se desynchronise des lignes des qu'une quantite
/// change. Ici il se recalcule, donc il ne peut pas mentir.
pub fn etat(livree: f64, commandee: f64) -> &'static str {
    if commandee <= EPSILON {
        "sans_objet"
    } else if livree <= EPSILON {
        "non_livre"
    } else if livree >= commandee - EPSILON {
        "livre"
    } else {
        "partiel"
    }
}

// Le reglage `suivi_livraison_actif` vit dans `parametres.rs`, avec le
// scanner et le bon de sortie : les bascules d'ecran sont regroupees la,
// pas eparpillees dans chaque module metier.

// =====================================================================
//  LECTURE
// =====================================================================

/// Lignes d'une piece avec ce qui reste a livrer.
#[tauri::command]
pub fn lire_livraison_piece(
    etat_app: State<EtatApp>,
    piece_id: String,
) -> Result<serde_json::Value, String> {
    let conn = etat_app.conn.lock().map_err(|e| e.to_string())?;

    let (numero, type_piece): (String, String) = conn.query_row(
        "SELECT numero, type_piece FROM piece_commerciale WHERE id = ?1",
        rusqlite::params![piece_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    ).map_err(|_| "Piece introuvable".to_string())?;

    let mut st = conn.prepare(
        "SELECT lp.id, a.nom, COALESCE(u.libelle, a.unite_base),
                lp.quantite, COALESCE(lp.quantite_livree, 0)
         FROM ligne_piece lp
         JOIN article a ON a.id = lp.article_id
         LEFT JOIN unite_vente u ON u.id = lp.unite_vente_id
         WHERE lp.piece_id = ?1
         ORDER BY a.nom"
    ).map_err(|e| e.to_string())?;

    let lignes: Vec<serde_json::Value> = st.query_map(
        rusqlite::params![piece_id], |r| {
            let qte: f64 = r.get(3)?;
            let livree: f64 = r.get(4)?;
            Ok(serde_json::json!({
                "id":              r.get::<_, String>(0)?,
                "article_nom":     r.get::<_, String>(1)?,
                "unite":           r.get::<_, String>(2)?,
                "quantite":        qte,
                "quantite_livree": livree,
                "reste":           (qte - livree).max(0.0),
            }))
        }
    ).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    let commandee: f64 = lignes.iter()
        .filter_map(|l| l["quantite"].as_f64()).sum();
    let livree: f64 = lignes.iter()
        .filter_map(|l| l["quantite_livree"].as_f64()).sum();

    Ok(serde_json::json!({
        "piece_id":   piece_id,
        "numero":     numero,
        "type_piece": type_piece,
        "lignes":     lignes,
        "etat":       etat(livree, commandee),
    }))
}

// =====================================================================
//  ECRITURE
// =====================================================================

#[derive(Deserialize)]
pub struct LigneLivraison {
    pub ligne_id: String,
    /// Quantite TOTALE livree a ce jour, pas l'increment.
    ///
    /// Un increment obligerait l'ecran a connaitre l'etat courant pour
    /// calculer la difference, et deux enregistrements rapproches
    /// doubleraient la quantite. Ici l'ecran envoie ce qu'il affiche.
    pub quantite_livree: f64,
}

/// Enregistre l'etat de livraison d'une piece.
///
/// Aucun mouvement de stock, aucun mouvement de caisse, aucune session
/// de caisse exigee : livrer n'est pas encaisser, et refuser une
/// livraison parce que le tiroir est ferme n'aurait aucun sens.
#[tauri::command]
pub fn enregistrer_livraison(
    etat_app: State<EtatApp>,
    piece_id: String,
    lignes: Vec<LigneLivraison>,
) -> Result<serde_json::Value, String> {
    let mut conn = etat_app.conn.lock().map_err(|e| e.to_string())?;
    let now = maintenant_iso();
    let auteur = crate::commandes::ventes::id_utilisateur_courant_pub(&conn);

    let tx = conn.transaction().map_err(|e| e.to_string())?;

    for l in &lignes {
        // Quantite commandee de CETTE ligne, pour plafonner. Livrer plus
        // que commande n'est pas une livraison, c'est une saisie fausse.
        let commandee: f64 = tx.query_row(
            "SELECT quantite FROM ligne_piece WHERE id = ?1 AND piece_id = ?2",
            rusqlite::params![l.ligne_id, piece_id], |r| r.get(0),
        ).map_err(|_| format!("Ligne {} introuvable sur cette piece", l.ligne_id))?;

        let valeur = l.quantite_livree.clamp(0.0, commandee);

        tx.execute(
            "UPDATE ligne_piece SET quantite_livree = ?1 WHERE id = ?2",
            rusqlite::params![valeur, l.ligne_id],
        ).map_err(|e| e.to_string())?;
    }

    // Etat recalcule DANS la transaction : c'est lui qu'on journalise.
    let (livree, commandee): (f64, f64) = tx.query_row(
        "SELECT COALESCE(SUM(quantite_livree), 0), COALESCE(SUM(quantite), 0)
         FROM ligne_piece WHERE piece_id = ?1",
        rusqlite::params![piece_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    ).unwrap_or((0.0, 0.0));

    let e = etat(livree, commandee);

    // Le journal est append-only (invariant 11) : on trace l'etat
    // atteint, pas la ligne modifiee. C'est ce qui permet de repondre
    // « quand est-ce parti ? » des semaines plus tard.
    tx.execute(
        "INSERT INTO journal
         (id, type_evenement, entite_type, entite_id, auteur_id,
          nouveau_valeur, origine, date_evenement)
         VALUES (?1,'livraison_enregistree','piece_commerciale',?2,?3,?4,'app',?5)",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(), piece_id, auteur,
            format!(r#"{{"etat":"{}","livree":{},"commandee":{}}}"#,
                e, livree, commandee),
            now
        ],
    ).ok();

    tx.commit().map_err(|e| e.to_string())?;

    Ok(serde_json::json!({
        "etat":      e,
        "livree":    livree,
        "commandee": commandee,
    }))
}

// =====================================================================
//  TESTS
// =====================================================================

#[cfg(test)]
mod tests {
    use super::etat;

    #[test]
    fn rien_livre() {
        assert_eq!(etat(0.0, 10.0), "non_livre");
    }

    #[test]
    fn partiellement_livre() {
        assert_eq!(etat(3.0, 10.0), "partiel");
    }

    #[test]
    fn entierement_livre() {
        assert_eq!(etat(10.0, 10.0), "livre");
    }

    #[test]
    fn arrondi_flottant_compte_comme_livre() {
        // 0.1 * 3 != 0.3 en binaire. Sans EPSILON, cette commande
        // resterait « partiel » pour toujours.
        assert_eq!(etat(0.1 + 0.1 + 0.1, 0.3), "livre");
    }

    #[test]
    fn piece_sans_ligne() {
        assert_eq!(etat(0.0, 0.0), "sans_objet");
    }

    #[test]
    fn surlivraison_compte_comme_livre() {
        // `enregistrer_livraison` plafonne, mais une donnee ancienne ou
        // importee peut depasser : elle ne doit pas retomber en partiel.
        assert_eq!(etat(12.0, 10.0), "livre");
    }
}
