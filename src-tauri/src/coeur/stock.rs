#![allow(dead_code)]
//! Logique de stock — calculs purs.

/// Vérifie si une vente entraîne un découvert.
pub fn est_a_decouvert(stock_actuel: f64, quantite_demandee: f64) -> bool {
    quantite_demandee > stock_actuel
}

/// Marge brute sur une vente.
pub fn marge(prix_vente: i64, prix_achat: i64, quantite: f64) -> i64 {
    ((prix_vente - prix_achat) as f64 * quantite).round() as i64
}

/// Crédit client après retour.
pub fn credit_retour(prix_pratique: i64, quantite_retournee: f64) -> i64 {
    (prix_pratique as f64 * quantite_retournee).round() as i64
}
