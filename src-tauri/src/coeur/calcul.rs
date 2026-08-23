//! Calculs métier purs — sans I/O, sans état.
//! §13 : montants en i64, jamais Float pour l'argent.

/// Montant d'une ligne = prix unitaire × quantité, arrondi au franc.
pub fn montant_ligne(prix_pratique: i64, quantite: f64) -> i64 {
    (prix_pratique as f64 * quantite).round() as i64
}

/// Total d'une vente = somme des montants de ses lignes.
pub fn total_vente(montants: &[i64]) -> i64 {
    montants.iter().sum()
}

/// Somme des paiements enregistrés sur une vente.
pub fn total_paye(paiements: &[i64]) -> i64 {
    paiements.iter().sum()
}

/// Reste dû = total de la vente - somme des paiements.
/// Peut être négatif en cas de surpaiement (à traiter par l'appelant).
pub fn reste_du(total: i64, paiements: &[i64]) -> i64 {
    total - total_paye(paiements)
}

#[derive(Debug, PartialEq)]
pub enum StatutVente {
    Payee,
    PartiellementPayee,
    CreanceOuverte,
}

/// Statut d'une vente à partir de son total et du **montant déjà encaissé**.
///
/// ⚠️ Le second argument est le montant PAYÉ, pas le reste dû.
/// Pour partir d'un reste, utiliser `statut_vente_depuis_reste`.
pub fn statut_vente(total: i64, montant_paye: i64) -> StatutVente {
    if montant_paye >= total {
        StatutVente::Payee
    } else if montant_paye > 0 {
        StatutVente::PartiellementPayee
    } else {
        StatutVente::CreanceOuverte
    }
}

/// Variante prenant le reste dû — évite l'inversion d'argument.
pub fn statut_vente_depuis_reste(total: i64, reste: i64) -> StatutVente {
    statut_vente(total, total - reste)
}

/// Écart entre prix de référence et prix pratiqué.
/// Positif = remise accordée. Négatif = hausse (ignorée, cf. §7).
pub fn ecart_prix(prix_reference: i64, prix_pratique: i64) -> i64 {
    prix_reference - prix_pratique
}

// =====================================================================
//  TESTS
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn montant_ligne_arrondit_au_franc() {
        assert_eq!(montant_ligne(800, 2.5), 2000);
        assert_eq!(montant_ligne(333, 3.0), 999);
        // 1200 × 0.333 = 399.6 -> 400
        assert_eq!(montant_ligne(1200, 0.333), 400);
    }

    #[test]
    fn total_vente_somme_les_lignes() {
        assert_eq!(total_vente(&[2000, 3500, 500]), 6000);
        assert_eq!(total_vente(&[]), 0);
    }

    #[test]
    fn reste_du_deduit_les_paiements() {
        assert_eq!(reste_du(10_000, &[4_000, 1_000]), 5_000);
        assert_eq!(reste_du(10_000, &[]), 10_000);
        assert_eq!(reste_du(10_000, &[10_000]), 0);
    }

    #[test]
    fn statut_vente_prend_le_montant_paye() {
        // Vente entièrement réglée.
        assert_eq!(statut_vente(10_000, 10_000), StatutVente::Payee);
        // Surpaiement : reste payée.
        assert_eq!(statut_vente(10_000, 12_000), StatutVente::Payee);
        // Acompte.
        assert_eq!(statut_vente(10_000, 4_000), StatutVente::PartiellementPayee);
        // Rien encaissé.
        assert_eq!(statut_vente(10_000, 0), StatutVente::CreanceOuverte);
    }

    #[test]
    fn statut_vente_depuis_reste_ne_sinverse_pas() {
        // Reste 0 = tout payé.
        assert_eq!(statut_vente_depuis_reste(10_000, 0), StatutVente::Payee);
        // Reste = total = rien payé.
        assert_eq!(
            statut_vente_depuis_reste(10_000, 10_000),
            StatutVente::CreanceOuverte
        );
        // Reste partiel.
        assert_eq!(
            statut_vente_depuis_reste(10_000, 6_000),
            StatutVente::PartiellementPayee
        );
    }

    #[test]
    fn ecart_prix_positif_si_remise() {
        assert_eq!(ecart_prix(800, 750), 50);
        assert_eq!(ecart_prix(800, 800), 0);
        assert!(ecart_prix(800, 900) < 0);
    }
}
