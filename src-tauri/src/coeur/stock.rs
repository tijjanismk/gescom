//! Cœur de calcul — Groupes D (stock) et E (retour / échange).
//!
//! Suite de `coeur.rs`. Toujours des fonctions PURES : nombres en entrée, nombres
//! en sortie. Aucune base, aucune interface, aucun Tauri.
//!
//! Rappels de convention :
//! - MONTANTS en `i64` (FCFA, jamais de flottant pour l'argent).
//! - QUANTITÉS en `f64` (kg, mètre ; les unités entières y tiennent).
//! - Français partout.
//!
//! Pour lancer les tests :  cargo test

// =====================================================================
//  GROUPE D — Stock
// =====================================================================

/// Nouveau niveau de stock après une sortie (vente, transfert sortant...).
///
/// `stock_actuel`  : quantité en stock avant l'opération, en unité de base.
/// `quantite_base` : quantité qui sort, DÉJÀ convertie en unité de base
///                   (utiliser `en_unite_base` du module précédent pour convertir).
///
/// Le résultat PEUT être négatif : la vente à découvert est autorisée (décision
/// terrain, comportement repris de Ciel). Un stock négatif est un signal de
/// régularisation, pas une erreur. On ne bloque pas ici.
pub fn stock_apres_sortie(stock_actuel: f64, quantite_base: f64) -> f64 {
    stock_actuel - quantite_base
}

/// Indique si une sortie va faire passer (ou laisser) le stock SOUS zéro.
///
/// Sert à marquer la ligne de vente `vente_a_decouvert = true` et à faire apparaître
/// l'article dans la vue « à régulariser ». On regarde le stock APRÈS l'opération.
pub fn est_a_decouvert(stock_actuel: f64, quantite_base: f64) -> bool {
    stock_apres_sortie(stock_actuel, quantite_base) < 0.0
}

/// Marge d'une ligne = (prix pratiqué − coût unitaire de base) × quantité.
///
/// `prix_pratique`  : prix de vente réellement appliqué, PAR UNITÉ DE VENTE.
/// `cout_base`      : coût d'achat PAR UNITÉ DE BASE. En vente à découvert, passer
///                    ici le `dernier_prix_achat` (fallback façon Ciel) pour que la
///                    marge reste calculable même sans stock.
/// `facteur`        : conversion unité de vente -> unité de base (pour ramener le
///                    coût de base au niveau de l'unité vendue).
/// `quantite`       : quantité vendue, dans l'unité de vente.
///
/// Note : le coût est exprimé par unité de base, le prix par unité de vente. On
/// remonte donc le coût au niveau de l'unité de vente (cout_base × facteur) avant
/// de soustraire. Résultat en FCFA (entier). Champ sensible : ne pas exposer sans
/// la permission de voir les coûts.
pub fn marge_ligne(prix_pratique: i64, cout_base: i64, facteur: f64, quantite: f64) -> i64 {
    let cout_unite_vente = cout_base as f64 * facteur;
    let marge_unitaire = prix_pratique as f64 - cout_unite_vente;
    (marge_unitaire * quantite).round() as i64
}

// =====================================================================
//  GROUPE E — Retour et échange
// =====================================================================

/// Valeur du crédit généré par un retour = ce que le client avait payé pour ce
/// qu'il rend.
///
/// `prix_pratique_origine` : le prix RÉELLEMENT payé à l'origine pour cet article
///                           (pas le prix de référence — on rembourse ce qui a été
///                           payé, pas le tarif affiché).
/// `quantite_retournee`    : combien d'unités (dans l'unité de vente d'origine) sont
///                           rendues.
pub fn credit_retour(prix_pratique_origine: i64, quantite_retournee: f64) -> i64 {
    (prix_pratique_origine as f64 * quantite_retournee).round() as i64
}

/// Reliquat après un échange : ce qu'il reste à régler, du point de vue du CLIENT.
///
/// reliquat = crédit du retour − montant de l'article pris en remplacement.
///
///   - positif  -> il reste un crédit EN FAVEUR DU CLIENT (à rendre en espèces,
///                 ou à conserver en avoir). C'est ton cas « parfois payé, parfois non ».
///   - négatif  -> le client DOIT COMPLÉTER (le nouvel article vaut plus cher).
///   - zéro     -> échange à valeur égale, rien à régler.
///
/// On rend la valeur brute (signée) : l'appelant interprète le signe pour décider
/// remboursement / avoir / complément.
pub fn reliquat_echange(credit_retour: i64, montant_remplacement: i64) -> i64 {
    credit_retour - montant_remplacement
}

// =====================================================================
//  TESTS
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Groupe D ----

    #[test]
    fn stock_apres_une_vente_normale() {
        // 10 en stock, on sort 3 -> 7
        assert_eq!(stock_apres_sortie(10.0, 3.0), 7.0);
        // Stock en kg : 5,5 - 2,0 = 3,5
        assert_eq!(stock_apres_sortie(5.5, 2.0), 3.5);
    }

    #[test]
    fn stock_peut_passer_a_decouvert() {
        // 1 en stock, on sort 3 -> -2 (autorisé, signal de régularisation)
        assert_eq!(stock_apres_sortie(1.0, 3.0), -2.0);
    }

    #[test]
    fn detection_du_decouvert() {
        // Assez de stock -> pas à découvert
        assert_eq!(est_a_decouvert(10.0, 3.0), false);
        // Pile à zéro -> pas à découvert (0 n'est pas < 0)
        assert_eq!(est_a_decouvert(3.0, 3.0), false);
        // Pas assez -> à découvert
        assert_eq!(est_a_decouvert(1.0, 3.0), true);
    }

    #[test]
    fn marge_d_une_ligne() {
        // Vendu 500/unité, coût 450/unité de base, facteur 1, 4 unités.
        // marge unitaire = 500 - 450 = 50 ; × 4 = 200
        assert_eq!(marge_ligne(500, 450, 1.0, 4.0), 200);

        // Vente au carton : prix carton 5400, coût 450/unité de base, facteur 12.
        // coût du carton = 450 × 12 = 5400 ; marge = 5400 - 5400 = 0 ; × 2 = 0
        assert_eq!(marge_ligne(5400, 450, 12.0, 2.0), 0);

        // Carton vendu 6000, coût 450/base, facteur 12 -> marge carton 600 ; × 1 = 600
        assert_eq!(marge_ligne(6000, 450, 12.0, 1.0), 600);
    }

    // ---- Groupe E ----

    #[test]
    fn credit_d_un_retour() {
        // 2 unités rendues, payées 450 chacune -> crédit 900
        assert_eq!(credit_retour(450, 2.0), 900);
        // Retour d'1 kg payé 700 -> 700
        assert_eq!(credit_retour(700, 1.0), 700);
    }

    #[test]
    fn reliquat_apres_echange() {
        // Rend pour 5000, reprend un article à 3000 -> +2000 en faveur du client
        assert_eq!(reliquat_echange(5000, 3000), 2000);
        // Rend pour 3000, reprend à 5000 -> -2000 : le client complète 2000
        assert_eq!(reliquat_echange(3000, 5000), -2000);
        // Échange à valeur égale -> 0
        assert_eq!(reliquat_echange(4000, 4000), 0);
    }

    // ---- Scénario complet : un échange avec reliquat ----

    #[test]
    fn scenario_echange_avec_reliquat() {
        // Le client rapporte 2 unités payées 450 chacune.
        let credit = credit_retour(450, 2.0); // 900
        assert_eq!(credit, 900);

        // Il repart avec 1 article à 600.
        let reliquat = reliquat_echange(credit, 600); // 900 - 600 = 300
        assert_eq!(reliquat, 300);

        // reliquat positif -> il reste 300 en faveur du client :
        // soit rendu en espèces, soit conservé en avoir (décision au comptoir).
        assert!(reliquat > 0);
    }

    #[test]
    fn scenario_echange_avec_complement() {
        // Rend 1 article payé 500 -> crédit 500
        let credit = credit_retour(500, 1.0);
        // Reprend un article à 1200 -> reliquat -700 : le client complète 700
        let reliquat = reliquat_echange(credit, 1200);
        assert_eq!(reliquat, -700);
        assert!(reliquat < 0); // complément à payer par le client
    }
}
