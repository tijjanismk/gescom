//! Cœur de calcul — Groupes A (unités), B (prix), C (totaux / reste dû / statut).
//!
//! Fonctions PURES : elles prennent des nombres, rendent des nombres.
//! Aucune base de données, aucune interface, aucun Tauri. Testables seules.
//!
//! Conventions :
//! - Les MONTANTS sont des entiers `i64` (FCFA, pas de subdivision, jamais de flottant).
//! - Les QUANTITÉS sont des `f64` (pour kg, mètre ; les unités entières y tiennent aussi).
//! - Tout est en français : noms de fonctions, commentaires, tests.
//!
//! Pour lancer les tests sur ta machine :  cargo test

// =====================================================================
//  GROUPE A — Unités et conversions
// =====================================================================

/// Convertit une quantité exprimée dans une unité de vente vers l'unité de base.
///
/// Exemple : 3 cartons, où 1 carton = 12 unités de base  ->  36 unités de base.
///
/// `quantite` : la quantité dans l'unité de vente choisie (ex. 3 cartons).
/// `facteur`  : combien d'unités de base vaut 1 unité de vente (ex. 12).
pub fn en_unite_base(quantite: f64, facteur: f64) -> f64 {
    quantite * facteur
}

/// Ramène un coût d'achat exprimé pour une unité de vente au coût par unité de base.
///
/// Exemple : un carton acheté 5 400 F, facteur 12  ->  450 F par unité de base.
///
/// On travaille en entiers (FCFA). La division entière tronque : c'est acceptable
/// pour un coût unitaire indicatif servant au calcul de marge. Si `facteur` vaut 0
/// ou moins (donnée incohérente), on renvoie 0 plutôt que de planter.
pub fn cout_unitaire_base(cout_achat_unite_vente: i64, facteur: f64) -> i64 {
    if facteur <= 0.0 {
        return 0;
    }
    // cout / facteur, ramené à l'entier inférieur.
    (cout_achat_unite_vente as f64 / facteur).floor() as i64
}

// =====================================================================
//  GROUPE B — Prix et écart
// =====================================================================

/// Écart entre le prix de référence et le prix réellement pratiqué.
///
/// Rend `reference - pratique` :
///   - positif  -> remise accordée (à tracer au journal),
///   - zéro     -> prix plein,
///   - négatif  -> le vendeur a vendu AU-DESSUS du prix de référence (hausse).
///
/// On rend la valeur brute, sans la ramener à zéro : on n'invente rien, l'appelant
/// décide quoi en faire. La hausse (valeur négative) n'est pas traitée spécialement
/// ailleurs, mais elle n'est pas cachée non plus.
pub fn ecart_prix(prix_reference: i64, prix_pratique: i64) -> i64 {
    prix_reference - prix_pratique
}

/// Montant d'une ligne de vente = prix pratiqué × quantité.
///
/// La quantité est un `f64` (kg, mètre possibles). Le résultat est un montant entier
/// (FCFA) : on arrondit à l'entier le plus proche pour absorber les décimales de
/// quantité (ex. 2,5 kg à 800 F = 2000 F).
pub fn montant_ligne(prix_pratique: i64, quantite: f64) -> i64 {
    (prix_pratique as f64 * quantite).round() as i64
}

// =====================================================================
//  GROUPE C — Totaux, reste dû, statut
// =====================================================================

/// Total d'une vente = somme des montants de toutes ses lignes.
///
/// `montants_lignes` : les montants déjà calculés de chaque ligne (via `montant_ligne`).
pub fn total_vente(montants_lignes: &[i64]) -> i64 {
    montants_lignes.iter().sum()
}

/// Reste dû par le client sur une vente — LE nombre central du système.
///
/// reste = total − (somme des paiements) − (somme des avoirs appliqués).
///
/// Un reste strictement positif = créance vivante. On ne descend jamais sous zéro
/// ici : si le client a trop versé (paiements + avoirs > total), le reste dû est 0
/// (le trop-perçu est un autre sujet, géré ailleurs, pas un « reste dû négatif »).
pub fn reste_du(total_vente: i64, paiements: &[i64], avoirs_appliques: &[i64]) -> i64 {
    let paye: i64 = paiements.iter().sum();
    let avoirs: i64 = avoirs_appliques.iter().sum();
    let reste = total_vente - paye - avoirs;
    if reste < 0 {
        0
    } else {
        reste
    }
}

/// Statut d'une vente, dérivé du total et du reste dû.
///
///   - reste == 0            -> Payee
///   - 0 < reste < total     -> PartiellementPayee
///   - reste == total        -> CreanceOuverte (rien n'a été payé)
///
/// Note : une vente à total 0 (cas limite, ex. entièrement couverte par avoir) est
/// considérée Payee.
#[derive(Debug, PartialEq, Eq)]
pub enum StatutVente {
    Payee,
    PartiellementPayee,
    CreanceOuverte,
}

pub fn statut_vente(total_vente: i64, reste_du: i64) -> StatutVente {
    if reste_du <= 0 {
        StatutVente::Payee
    } else if reste_du >= total_vente {
        StatutVente::CreanceOuverte
    } else {
        StatutVente::PartiellementPayee
    }
}

// =====================================================================
//  TESTS — chaque fonction est vérifiée sur des cas concrets.
//  Lance-les avec :  cargo test
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Groupe A ----

    #[test]
    fn conversion_cartons_vers_unites() {
        // 3 cartons de 12 = 36 unités de base
        assert_eq!(en_unite_base(3.0, 12.0), 36.0);
        // 1 unité (facteur 1) reste 1
        assert_eq!(en_unite_base(1.0, 1.0), 1.0);
        // 2,5 kg (facteur 1) reste 2,5
        assert_eq!(en_unite_base(2.5, 1.0), 2.5);
    }

    #[test]
    fn cout_ramene_a_l_unite_de_base() {
        // Carton acheté 5400, facteur 12 -> 450 / unité
        assert_eq!(cout_unitaire_base(5400, 12.0), 450);
        // Facteur 1 -> coût inchangé
        assert_eq!(cout_unitaire_base(800, 1.0), 800);
        // Facteur incohérent (0) -> 0, pas de plantage
        assert_eq!(cout_unitaire_base(5400, 0.0), 0);
    }

    // ---- Groupe B ----

    #[test]
    fn ecart_de_prix() {
        // Remise : référence 500, pratiqué 450 -> écart +50
        assert_eq!(ecart_prix(500, 450), 50);
        // Prix plein -> 0
        assert_eq!(ecart_prix(500, 500), 0);
        // Hausse : pratiqué au-dessus -> écart négatif (non caché)
        assert_eq!(ecart_prix(500, 600), -100);
    }

    #[test]
    fn montant_d_une_ligne() {
        // 4 unités à 500 = 2000
        assert_eq!(montant_ligne(500, 4.0), 2000);
        // 2,5 kg à 800 = 2000
        assert_eq!(montant_ligne(800, 2.5), 2000);
        // 3,7 mètres à 1500 = 5550
        assert_eq!(montant_ligne(1500, 3.7), 5550);
    }

    // ---- Groupe C ----

    #[test]
    fn total_d_une_vente() {
        assert_eq!(total_vente(&[2000, 3000, 500]), 5500);
        // Vente vide -> 0
        assert_eq!(total_vente(&[]), 0);
    }

    #[test]
    fn reste_du_cas_courants() {
        // Total 10000, payé 6000 -> reste 4000
        assert_eq!(reste_du(10000, &[6000], &[]), 4000);
        // Payé en deux fois : 6000 + 4000 -> reste 0
        assert_eq!(reste_du(10000, &[6000, 4000], &[]), 0);
        // Un avoir de 2000 appliqué + 3000 payés sur 10000 -> reste 5000
        assert_eq!(reste_du(10000, &[3000], &[2000]), 5000);
        // Trop versé (12000 sur 10000) -> reste 0, jamais négatif
        assert_eq!(reste_du(10000, &[12000], &[]), 0);
        // Rien payé -> reste = total
        assert_eq!(reste_du(10000, &[], &[]), 10000);
    }

    #[test]
    fn statut_d_une_vente() {
        // Soldée
        assert_eq!(statut_vente(10000, 0), StatutVente::Payee);
        // Partielle
        assert_eq!(statut_vente(10000, 4000), StatutVente::PartiellementPayee);
        // Créance ouverte (rien payé)
        assert_eq!(statut_vente(10000, 10000), StatutVente::CreanceOuverte);
        // Cas limite : total 0 -> Payee
        assert_eq!(statut_vente(0, 0), StatutVente::Payee);
    }

    // ---- Un scénario complet, de bout en bout ----

    #[test]
    fn scenario_vente_complete() {
        // Le client achète :
        //   - 2 cartons de sucre (facteur 12), prix carton pratiqué 5400
        //   - 3,5 kg de riz, prix au kg pratiqué 700
        let ligne_sucre = montant_ligne(5400, 2.0); // 10800
        let ligne_riz = montant_ligne(700, 3.5); // 2450
        let total = total_vente(&[ligne_sucre, ligne_riz]); // 13250

        assert_eq!(total, 13250);

        // Il paie 10000 comptant, le reste à crédit.
        let reste = reste_du(total, &[10000], &[]);
        assert_eq!(reste, 3250);
        assert_eq!(statut_vente(total, reste), StatutVente::PartiellementPayee);

        // Vérif conversion stock : 2 cartons = 24 unités de base à décrémenter.
        assert_eq!(en_unite_base(2.0, 12.0), 24.0);
    }
}
