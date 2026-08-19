#![allow(dead_code)]
//! Calculs métier purs — sans I/O, sans état.
//! §13 : montants en i64, jamais Float pour l'argent.

pub fn montant_ligne(prix_pratique: i64, quantite: f64) -> i64 {
    (prix_pratique as f64 * quantite).round() as i64
}

pub fn total_vente(montants: &[i64]) -> i64 {
    montants.iter().sum()
}

pub fn reste_du(total: i64, paiements: &[i64]) -> i64 {
    let paye: i64 = paiements.iter().sum();
    total - paye
}

#[derive(Debug, PartialEq)]
pub enum StatutVente {
    Payee,
    PartielementPayee,
    CreanceOuverte,
}

pub fn statut_vente(total: i64, paye: i64) -> StatutVente {
    if paye >= total { StatutVente::Payee }
    else if paye > 0 { StatutVente::PartielementPayee }
    else { StatutVente::CreanceOuverte }
}

pub fn ecart_prix(prix_reference: i64, prix_pratique: i64) -> i64 {
    prix_reference - prix_pratique
}
