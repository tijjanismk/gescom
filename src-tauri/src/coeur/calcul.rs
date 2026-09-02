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

/// Sous ce seuil, un reste dû n'est plus recouvrable : la plus petite
/// pièce en circulation vaut 5 F. `CAST` tronquant en SQLite (D31), un
/// résidu d'arrondi laisserait sinon la créance ouverte pour toujours.
pub const SEUIL_SOLDE: i64 = 5;

/// Reste dû réellement exigible. Absorbe les résidus d'arrondi (D41).
///
/// Ne modifie AUCUN montant : ferme un statut, rien de plus. Aucun
/// paiement n'est créé, la caisse ne bouge pas.
pub fn reste_exigible(total: i64, montant_paye: i64) -> i64 {
    let reste = total - montant_paye;
    if reste <= 0 {
        return 0; // soldee, ou trop-percu
    }
    // Un residu naît d'un arrondi SUR un encaissement. Sans encaissement,
    // une petite vente reste une creance : sinon un total de 3 F se
    // solderait tout seul, sans qu'un franc entre en caisse.
    if montant_paye > 0 && reste <= SEUIL_SOLDE { 0 } else { reste }
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
    if reste_exigible(total, montant_paye) == 0 {
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
    fn seuil_absorbe_les_residus_darrondi() {
        // Le cas qui motive le seuil : 1 F de troncature (D31).
        assert_eq!(reste_exigible(10_000, 9_999), 0);
        assert_eq!(reste_exigible(10_000, 9_995), 0);
        assert_eq!(statut_vente(10_000, 9_997), StatutVente::Payee);
    }

    #[test]
    fn seuil_ne_solde_jamais_un_impaye_reel() {
        // 6 F reste exigible : le seuil ne mord pas au-dela.
        assert_eq!(reste_exigible(10_000, 9_994), 6);
        assert_eq!(statut_vente(10_000, 9_994), StatutVente::PartiellementPayee);
        assert_eq!(reste_exigible(10_000, 0), 10_000);
        assert_eq!(statut_vente(10_000, 0), StatutVente::CreanceOuverte);
    }

    #[test]
    fn seuil_couvre_le_trop_percu() {
        // Reste negatif : deja absorbe, pas d'exigible negatif.
        assert_eq!(reste_exigible(10_000, 12_000), 0);
        assert_eq!(statut_vente(10_000, 12_000), StatutVente::Payee);
    }

    #[test]
    fn petite_vente_impayee_reste_une_creance() {
        // Sans encaissement, pas de residu : le seuil ne doit pas
        // solder une vente de 3 F dont rien n'est entre en caisse.
        assert_eq!(reste_exigible(3, 0), 3);
        assert_eq!(statut_vente(3, 0), StatutVente::CreanceOuverte);
        // Mais 3 F payes sur 5 laissent bien un residu absorbable.
        assert_eq!(reste_exigible(5, 3), 0);
    }

    #[test]
    fn ecart_prix_positif_si_remise() {
        assert_eq!(ecart_prix(800, 750), 50);
        assert_eq!(ecart_prix(800, 800), 0);
        assert!(ecart_prix(800, 900) < 0);
    }
}