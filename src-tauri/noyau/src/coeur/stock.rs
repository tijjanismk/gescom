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
/// Sortie constatee par un bon de livraison.
///
/// Distincte de VENTE, et ce n'est pas un detail : l'historique de
/// stock retrouve le numero du document en joignant sur le TYPE et
/// `operation_id`. VENTE pointe vers la table `vente`, une livraison
/// vers `piece_commerciale`. Les confondre ferait chercher un
/// identifiant de piece dans les ventes — le mouvement s'afficherait
/// sans numero, donc invérifiable.
pub const LIVRAISON: &str = "livraison";
/// Entree constatee par un bon de reception.
///
/// Distincte d'ACHAT comme ENTREE l'est (D42) : a la reception, rien
/// n'est encore facture. C'est la facture fournisseur qui suit qui
/// cree la dette.
pub const RECEPTION: &str = "reception";

/// Tous les types connus, pour les ecrans qui filtrent.
pub const TOUS: [&str; 10] = [
    ACHAT, ENTREE, VENTE, RETOUR, RETOUR_FOURNISSEUR,
    ECHANGE, AJUSTEMENT, TRANSFERT, LIVRAISON, RECEPTION,
];

/// Le mouvement fait-il ENTRER de la marchandise ?
///
/// `None` pour AJUSTEMENT et TRANSFERT : le sens depend du signe de
/// `quantite_delta`, pas du type. Forcer une reponse ici ferait mentir
/// l'appelant une fois sur deux.
pub fn est_entrant(type_mouvement: &str) -> Option<bool> {
    match type_mouvement {
        ACHAT | ENTREE | RETOUR | RECEPTION => Some(true),
        VENTE | RETOUR_FOURNISSEUR | ECHANGE | LIVRAISON => Some(false),
        _ => None,
    }
}

/// Ce mouvement correspond-il a de la marchandise achetee au sens
/// comptable — c'est-a-dire facturee par un fournisseur ?
///
/// ENTREE et RECEPTION en sont exclues : rien n'a ete facture ni paye.
/// Pour une reception, c'est la facture fournisseur qui suit qui porte
/// la dette.
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
        LIVRAISON => "Livraison",
        RECEPTION => "Réception",
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

/// D36 — reprendre le suivi du stock d'un article exige un comptage
/// pour CHAQUE magasin actif : sinon son stock repartirait de la somme
/// de mouvements qu'on ne regardait plus — un chiffre faux. Rend les
/// comptages dans l'ordre des magasins ; refuse un magasin oublie, un
/// magasin inconnu, une quantite negative ou absurde.
pub fn verifier_comptages(
    depots_actifs: &[(String, String)],
    comptages: &[(String, f64)],
) -> Result<Vec<(String, f64)>, String> {
    for (depot, q) in comptages {
        if !depots_actifs.iter().any(|(id, _)| id == depot) {
            return Err(format!("Magasin inconnu ou fermé pour le comptage : « {depot} »."));
        }
        if !q.is_finite() || *q < 0.0 {
            return Err(format!("Quantité comptée invalide ({q}) : zéro ou plus."));
        }
    }
    depots_actifs
        .iter()
        .map(|(id, nom)| {
            comptages
                .iter()
                .find(|(d, _)| d == id)
                .map(|(_, q)| (id.clone(), *q))
                .ok_or_else(|| format!(
                    "Reprendre le suivi du stock demande de compter chaque magasin : « {nom} » manque."
                ))
        })
        .collect()
}

/// D36, lot D.11 — la colonne « Suivre le stock » de l'import CSV des
/// articles. Vide : `None`, l'import decide (oui pour un article cree,
/// inchange pour un article existant). Oui/non sous leurs formes
/// courantes — dont VRAI/FAUX, ce qu'Excel francophone ecrit pour une
/// case. Une valeur illisible est refusee, jamais devinee : un article
/// qu'on croit suivi et qui ne l'est pas cache ses manques.
pub fn lire_suivi_csv(valeur: &str) -> Result<Option<bool>, String> {
    let v = valeur.trim().to_lowercase();
    match v.as_str() {
        "" => Ok(None),
        "oui" | "o" | "1" | "vrai" => Ok(Some(true)),
        "non" | "n" | "0" | "faux" => Ok(Some(false)),
        _ => Err(format!(
            "« Suivre le stock » attend oui ou non, pas « {} »",
            valeur.trim()
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_colonne_suivre_le_stock_se_lit_sans_deviner() {
        assert_eq!(lire_suivi_csv(""), Ok(None));
        assert_eq!(lire_suivi_csv("  "), Ok(None));
        for oui in ["oui", "OUI", " Oui ", "o", "1", "VRAI"] {
            assert_eq!(lire_suivi_csv(oui), Ok(Some(true)), "{oui}");
        }
        for non in ["non", "NON", "n", "0", "FAUX"] {
            assert_eq!(lire_suivi_csv(non), Ok(Some(false)), "{non}");
        }
        for illisible in ["peut-être", "2", "yes", "ouii", "-", "x"] {
            let e = lire_suivi_csv(illisible).unwrap_err();
            assert!(e.contains("oui ou non") && e.contains(illisible), "{e}");
        }
    }

    fn magasins() -> Vec<(String, String)> {
        vec![("d1".into(), "Boutique".into()), ("d2".into(), "Dépôt".into())]
    }

    #[test]
    fn reprendre_le_suivi_compte_chaque_magasin() {
        let c = verifier_comptages(&magasins(), &[("d2".into(), 3.0), ("d1".into(), 0.0)]).unwrap();
        assert_eq!(c, vec![("d1".to_string(), 0.0), ("d2".to_string(), 3.0)]);
        let e = verifier_comptages(&magasins(), &[("d1".into(), 5.0)]).unwrap_err();
        assert!(e.contains("Dépôt"), "{e}");
    }

    #[test]
    fn un_comptage_absurde_ou_d_un_autre_magasin_est_refuse() {
        assert!(verifier_comptages(&magasins(), &[("d1".into(), -1.0), ("d2".into(), 1.0)]).is_err());
        assert!(verifier_comptages(&magasins(), &[("d1".into(), f64::NAN), ("d2".into(), 1.0)]).is_err());
        assert!(verifier_comptages(&magasins(), &[("d1".into(), 1.0), ("d2".into(), 1.0), ("x".into(), 1.0)])
            .unwrap_err()
            .contains("inconnu"));
    }

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

// =====================================================================
//  D45 — le seuil d'alerte d'un article
// =====================================================================

/// Le seuil d'un dossier qui n'en a pas regle : 5, la valeur ecrite en
/// dur avant D45 (tableau de bord, rapports).
pub const SEUIL_ALERTE_DEFAUT: f64 = 5.0;

/// Au-dela, une faute de frappe (des zeros de trop).
pub const SEUIL_ALERTE_MAX: f64 = 1_000_000.0;

/// Un seuil saisi : positif ou nul, borne. 0 = pas d'alerte, seulement
/// la rupture.
pub fn verifier_seuil(seuil: f64) -> Result<(), String> {
    if !seuil.is_finite() || seuil < 0.0 {
        return Err("Le seuil d'alerte est un nombre positif (0 : pas d'alerte).".to_string());
    }
    if seuil > SEUIL_ALERTE_MAX {
        return Err("Un seuil d'alerte de plus d'un million : vérifier le nombre de zéros.".to_string());
    }
    Ok(())
}

/// L'etat d'un stock face a son seuil : « rupture » a 0 ou moins,
/// « alerte » sous le seuil (strictement, comme le stock minimum de
/// Ciel), « ok » sinon.
pub fn statut_stock(quantite: f64, seuil: f64) -> &'static str {
    if quantite <= 0.0 {
        "rupture"
    } else if quantite < seuil {
        "alerte"
    } else {
        "ok"
    }
}

#[cfg(test)]
mod tests_seuil {
    use super::*;

    #[test]
    fn le_statut_suit_le_seuil_et_zero_coupe_l_alerte() {
        assert_eq!(statut_stock(0.0, 5.0), "rupture");
        assert_eq!(statut_stock(-2.0, 5.0), "rupture");
        assert_eq!(statut_stock(4.5, 5.0), "alerte");
        assert_eq!(statut_stock(5.0, 5.0), "ok", "strictement sous le seuil");
        assert_eq!(statut_stock(1.0, 0.0), "ok", "0 : pas d'alerte");
        assert_eq!(statut_stock(15.0, 20.0), "alerte");
    }

    #[test]
    fn un_seuil_se_juge() {
        assert!(verifier_seuil(0.0).is_ok());
        assert!(verifier_seuil(12.5).is_ok());
        assert!(verifier_seuil(-1.0).is_err());
        assert!(verifier_seuil(f64::NAN).is_err());
        assert!(verifier_seuil(2_000_000.0).is_err());
    }
}
