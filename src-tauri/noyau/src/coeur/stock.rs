#![allow(dead_code)]
//! Logique de stock — calculs purs.

// =====================================================================
//  TYPES DE MOUVEMENT
// =====================================================================
//
// `mouvement_stock.type_mouvement` est un TEXT libre : rien en base ne
// contraint la valeur. Chaque appelant ecrivait sa chaine a la main et
// le journal filtrait sur une liste codee en dur — les deux se sont
// desynchronises sans que rien ne le signale (echange, ajustement et
// transfert n'apparaissaient nulle part).
//
// Toute nouvelle valeur s'ajoute ICI, et nulle part ailleurs.

/// Achat facture : cree une FAF, une dette et un mouvement de caisse.
pub const ACHAT: &str = "achat";
/// Entree sans facture : marchandise qui rentre, rien a payer.
/// Distincte d'ACHAT depuis v1.3 — les confondre gonflait les achats
/// du jour d'un montant que personne ne doit.
pub const ENTREE: &str = "entree";
/// Sortie sur vente.
pub const VENTE: &str = "vente";
/// Retour client : la marchandise revient en stock.
pub const RETOUR: &str = "retour";
/// Retour fournisseur : la marchandise repart.
pub const RETOUR_FOURNISSEUR: &str = "retour_fournisseur";
/// Sortie de l'article de remplacement lors d'un echange.
pub const ECHANGE: &str = "echange";
/// Correction apres inventaire. Delta signe : peut aller dans les deux sens.
pub const AJUSTEMENT: &str = "ajustement";
/// Deplacement entre depots. Deux lignes par transfert, opposees.
pub const TRANSFERT: &str = "transfert";

/// Tous les types connus, pour les ecrans qui filtrent.
pub const TOUS: [&str; 8] = [
    ACHAT, ENTREE, VENTE, RETOUR, RETOUR_FOURNISSEUR,
    ECHANGE, AJUSTEMENT, TRANSFERT,
];

/// Le mouvement fait-il ENTRER de la marchandise ?
///
/// `None` pour AJUSTEMENT et TRANSFERT : le sens depend du signe de
/// `quantite_delta`, pas du type. Forcer une reponse ici ferait mentir
/// l'appelant une fois sur deux.
pub fn est_entrant(type_mouvement: &str) -> Option<bool> {
    match type_mouvement {
        ACHAT | ENTREE | RETOUR => Some(true),
        VENTE | RETOUR_FOURNISSEUR | ECHANGE => Some(false),
        _ => None,
    }
}

/// Ce mouvement correspond-il a de la marchandise achetee au sens
/// comptable — c'est-a-dire facturee par un fournisseur ?
///
/// ENTREE en est exclue : rien n'a ete facture ni paye.
pub fn est_achat_facture(type_mouvement: &str) -> bool {
    type_mouvement == ACHAT
}

/// Libelle affichable, pour le journal et l'historique de stock.
pub fn libelle(type_mouvement: &str) -> &'static str {
    match type_mouvement {
        ACHAT => "Achat",
        ENTREE => "Entrée",
        VENTE => "Vente",
        RETOUR => "Retour client",
        RETOUR_FOURNISSEUR => "Retour fournisseur",
        ECHANGE => "Échange",
        AJUSTEMENT => "Ajustement",
        TRANSFERT => "Transfert",
        _ => "Autre",
    }
}

// =====================================================================
//  CALCULS
// =====================================================================

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sens_des_mouvements_connus() {
        assert_eq!(est_entrant(ACHAT), Some(true));
        assert_eq!(est_entrant(ENTREE), Some(true));
        assert_eq!(est_entrant(RETOUR), Some(true));
        assert_eq!(est_entrant(VENTE), Some(false));
        assert_eq!(est_entrant(RETOUR_FOURNISSEUR), Some(false));
        assert_eq!(est_entrant(ECHANGE), Some(false));
    }

    #[test]
    fn ajustement_et_transfert_sans_sens_fixe() {
        // Le signe de quantite_delta tranche, pas le type.
        assert_eq!(est_entrant(AJUSTEMENT), None);
        assert_eq!(est_entrant(TRANSFERT), None);
    }

    #[test]
    fn entree_manuelle_n_est_pas_un_achat_facture() {
        // C'etait le bug : les deux ecrivaient 'achat' et le journal
        // les additionnait sous le meme total.
        assert!(est_achat_facture(ACHAT));
        assert!(!est_achat_facture(ENTREE));
    }

    #[test]
    fn tous_les_types_ont_un_libelle() {
        for t in TOUS {
            assert_ne!(libelle(t), "Autre", "libelle manquant : {}", t);
        }
    }

    #[test]
    fn tous_les_types_sont_distincts() {
        let mut v = TOUS.to_vec();
        v.sort();
        let n = v.len();
        v.dedup();
        assert_eq!(v.len(), n, "doublon dans TOUS");
    }

    #[test]
    fn type_inconnu_ne_panique_pas() {
        assert_eq!(est_entrant("inventaire_v2"), None);
        assert_eq!(libelle("inventaire_v2"), "Autre");
    }
}