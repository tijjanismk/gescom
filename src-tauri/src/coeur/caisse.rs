//! Cœur de calcul — Groupe F (caisse).
//!
//! Dernier module du cœur. Fonctions PURES, comme les précédentes.
//! Rappel : MONTANTS en `i64` (FCFA). Français partout.
//!
//! Pour lancer les tests :  cargo test  (ou mode « Test » dans le Playground)

// =====================================================================
//  GROUPE F — Caisse
// =====================================================================

/// Solde théorique ESPÈCES d'une session de caisse.
///
/// solde = fond initial + (somme des entrées espèces) − (somme des sorties espèces)
///
/// IMPORTANT : ne concerne QUE l'espèces. Les paiements Orange Money / Moov / chèque
/// sont tracés ailleurs mais n'entrent PAS dans le tiroir physique, donc pas ici.
/// C'est ce solde qu'on compare au comptage physique à la fermeture.
///
/// `fond_initial`   : la monnaie de départ mise dans le tiroir à l'ouverture.
/// `entrees_especes`: montants des mouvements espèces entrants (ventes cash...).
/// `sorties_especes`: montants des mouvements espèces sortants (remboursements,
///                    dépenses cash...).
pub fn solde_theorique_especes(
    fond_initial: i64,
    entrees_especes: &[i64],
    sorties_especes: &[i64],
) -> i64 {
    let entrees: i64 = entrees_especes.iter().sum();
    let sorties: i64 = sorties_especes.iter().sum();
    fond_initial + entrees - sorties
}

/// Écart de caisse à la fermeture = ce qui est physiquement compté − le théorique.
///
///   - zéro     -> la caisse tombe juste (idéal).
///   - positif  -> il y a PLUS d'argent que prévu (trop-perçu, erreur de rendu...).
///   - négatif  -> il MANQUE de l'argent (le cas qui doit alerter : erreur ou vol).
///
/// On rend la valeur brute signée : c'est LE chiffre de confiance. Un écart négatif
/// récurrent est le signal à surveiller.
pub fn ecart_caisse(montant_compte: i64, solde_theorique: i64) -> i64 {
    montant_compte - solde_theorique
}

// =====================================================================
//  TESTS
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solde_theorique_journee_simple() {
        // Fond de 10000, trois ventes cash (5000, 3000, 2000), aucune sortie.
        // 10000 + 10000 - 0 = 20000
        assert_eq!(
            solde_theorique_especes(10000, &[5000, 3000, 2000], &[]),
            20000
        );
    }

    #[test]
    fn solde_theorique_avec_sorties() {
        // Fond 10000, entrées 8000, un remboursement de 1500 et une dépense de 500.
        // 10000 + 8000 - 2000 = 16000
        assert_eq!(solde_theorique_especes(10000, &[8000], &[1500, 500]), 16000);
    }

    #[test]
    fn solde_theorique_sans_mouvement() {
        // Juste le fond initial, rien d'autre.
        assert_eq!(solde_theorique_especes(10000, &[], &[]), 10000);
    }

    #[test]
    fn ecart_caisse_juste() {
        // Compté = théorique -> écart 0
        assert_eq!(ecart_caisse(20000, 20000), 0);
    }

    #[test]
    fn ecart_caisse_manquant() {
        // Il manque 500 dans le tiroir -> écart négatif (à surveiller)
        assert_eq!(ecart_caisse(19500, 20000), -500);
        assert!(ecart_caisse(19500, 20000) < 0);
    }

    #[test]
    fn ecart_caisse_excedent() {
        // 300 de trop -> écart positif
        assert_eq!(ecart_caisse(20300, 20000), 300);
    }

    // ---- Scénario complet : ouverture -> journée -> fermeture ----

    #[test]
    fn scenario_journee_de_caisse() {
        // Ouverture avec 10000 de fond.
        // Journée : ventes cash 5000 + 12000 + 3000, un remboursement de 2000.
        let theorique = solde_theorique_especes(10000, &[5000, 12000, 3000], &[2000]);
        // 10000 + 20000 - 2000 = 28000
        assert_eq!(theorique, 28000);

        // Le soir, on compte 27500 dans le tiroir.
        let ecart = ecart_caisse(27500, theorique);
        // Il manque 500 -> à expliquer.
        assert_eq!(ecart, -500);
        assert!(ecart < 0);
    }
}
