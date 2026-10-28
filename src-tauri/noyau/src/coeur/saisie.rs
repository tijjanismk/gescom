//! Ce que le serveur refuse d'une saisie, quel que soit l'ecran.
//!
//! L'ecran ne propose que des valeurs sensees, mais une commande du
//! serveur s'appelle aussi sans lui. La revue du 23/09/2026 (D27) a
//! montre trois gestes que rien ne gardait :
//!
//! - regler une creance « par avoir » sans qu'aucun avoir soit consomme
//!   — la dette s'effacait, ni argent ni credit ;
//! - un paiement negatif — une « entree » de caisse negative, donc un
//!   tiroir qui peut etre vide sans que le comptage le voie ;
//! - une vente a quantite negative — le stock remontait, la vente
//!   passait « payee », sans retour ni avoir.

/// Les moyens par lesquels un client paie une creance. `avoir` n'en
/// est pas un : consommer un avoir passe par l'avoir lui-meme
/// (`appliquer_avoir_vente`, ou le reglement par avoir du point de
/// vente), qui le marque consomme.
pub const MODES_ENCAISSEMENT: &[&str] =
    &["especes", "orange_money", "moov_money", "cheque", "virement"];

pub fn verifier_mode_encaissement(mode: &str) -> Result<(), String> {
    if mode == "avoir" {
        return Err(
            "Un avoir n'est pas un mode de paiement : il doit être consommé \
             (au point de vente, ou appliqué à la vente), pas déclaré."
                .to_string(),
        );
    }
    if MODES_ENCAISSEMENT.contains(&mode) {
        Ok(())
    } else {
        Err(format!(
            "Mode de paiement inconnu : « {mode} ». Attendu : {}.",
            MODES_ENCAISSEMENT.join(", ")
        ))
    }
}

/// Un montant encaisse : strictement positif.
pub fn verifier_montant(montant: i64) -> Result<(), String> {
    if montant <= 0 {
        return Err("Le montant doit être positif".to_string());
    }
    Ok(())
}

/// Le plus grand montant qu'une saisie de la paie accepte (salaire,
/// tarif, prime, retenue, prix d'une tache, avance, versement, plafond) :
/// un milliard de francs. Aucune boutique n'en paie autant en une ligne ;
/// la borne existe pour qu'une valeur tapee de travers (ou envoyee sans
/// l'ecran) ne fasse pas deborder une somme — le serveur de debogage
/// paniquait et la base devenait « indisponible » (revue du 28/10).
pub const MONTANT_MAX: i64 = 1_000_000_000;

/// La plus grande quantite d'une ligne a la tache.
pub const QUANTITE_MAX: f64 = 100_000.0;

/// Un montant saisi : pas au-dela de `MONTANT_MAX`. `quoi` le nomme
/// dans le refus (« la prime », « l'avance »…).
pub fn borner_montant(montant: i64, quoi: &str) -> Result<(), String> {
    if montant > MONTANT_MAX {
        return Err(format!(
            "Montant trop grand pour {quoi} : au plus {} F par saisie. Vérifier le nombre de zéros.",
            francs_bruts(MONTANT_MAX)
        ));
    }
    Ok(())
}

/// Une quantite saisie : finie, positive, pas au-dela de `QUANTITE_MAX`.
pub fn borner_quantite(quantite: f64) -> Result<(), String> {
    if !quantite.is_finite() || quantite <= 0.0 {
        return Err("La quantité doit être plus grande que zéro.".to_string());
    }
    if quantite > QUANTITE_MAX {
        return Err(format!("Quantité trop grande : au plus {} par ligne.", francs_bruts(QUANTITE_MAX as i64)));
    }
    Ok(())
}

/// Une somme de montants, verifiee : un depassement est refuse au lieu
/// de faire paniquer le serveur (et de bloquer la base).
pub fn somme(montants: impl IntoIterator<Item = i64>) -> Result<i64, String> {
    montants
        .into_iter()
        .try_fold(0i64, |acc, m| acc.checked_add(m))
        .ok_or_else(|| "Montants hors limites : la somme dépasse ce que Gescom sait compter.".to_string())
}

/// « 1 000 000 000 », sans le « F ».
fn francs_bruts(n: i64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(' ');
        }
        out.push(c);
    }
    out
}

/// Une ligne de vente, d'achat ou de piece. Le sens du stock est porte
/// par le GESTE (vente, retour, reception), jamais par le signe de la
/// quantite : une quantite negative ferait d'une vente un retour qui ne
/// passe par aucun controle de retour.
pub fn verifier_ligne(quantite: f64, facteur: f64, prix: i64) -> Result<(), String> {
    if !quantite.is_finite() || quantite <= 0.0 {
        return Err(format!("Quantité invalide ({quantite}) : elle doit être positive."));
    }
    if !facteur.is_finite() || facteur <= 0.0 {
        return Err(format!("Facteur d'unité invalide ({facteur})."));
    }
    if prix < 0 {
        return Err(format!("Prix négatif refusé ({prix} F)."));
    }
    Ok(())
}

/// Une remise en pourcentage : entre 0 et 100.
pub fn verifier_remise_pct(pct: f64) -> Result<(), String> {
    if !pct.is_finite() || !(0.0..=100.0).contains(&pct) {
        return Err(format!("Remise invalide ({pct} %) : entre 0 et 100."));
    }
    Ok(())
}

/// D35 — une vente a un client de passage (le « Comptant » ou une
/// fiche passage) se paie en entier, a la vente.
///
/// Personne a relancer : une dette au nom d'un passant n'apparait dans
/// aucune relance ni aucun rapport (ils ecartent les fiches generiques),
/// la marchandise sortait et rien n'entrait. Pas de credit, et
/// l'encaissement couvre le total ; un client qui paie en plusieurs
/// fois se cree (ou se « passe en client ordinaire ») d'abord.
pub fn verifier_vente_passage(mode_reglement: &str, total: i64, encaisse: i64) -> Result<(), String> {
    if mode_reglement != "comptant" {
        return Err("Pas de crédit pour un client de passage. \
                    Créer ou sélectionner le client d'abord."
            .to_string());
    }
    if encaisse < total {
        return Err(format!(
            "Un client de passage paie en entier : {total} F à encaisser, {encaisse} F saisis. \
             Pour un paiement en plusieurs fois, créer ou sélectionner le client d'abord."
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_passant_paie_tout_comptant() {
        assert!(verifier_vente_passage("comptant", 5000, 5000).is_ok());
        // Rendre la monnaie est l'affaire de l'ecran : payer plus passe.
        assert!(verifier_vente_passage("comptant", 5000, 10000).is_ok());
    }

    #[test]
    fn le_passant_n_a_ni_credit_ni_paiement_partiel() {
        assert!(verifier_vente_passage("credit", 5000, 5000).unwrap_err().contains("Pas de crédit"));
        let e = verifier_vente_passage("comptant", 5000, 3000).unwrap_err();
        assert!(e.contains("5000 F") && e.contains("3000 F"), "{e}");
        assert!(verifier_vente_passage("comptant", 5000, 0).is_err());
    }

    #[test]
    fn les_modes_de_l_ecran_passent() {
        for m in ["especes", "orange_money", "moov_money", "cheque", "virement"] {
            assert!(verifier_mode_encaissement(m).is_ok(), "{m}");
        }
    }

    #[test]
    fn l_avoir_n_est_pas_un_mode() {
        let e = verifier_mode_encaissement("avoir").unwrap_err();
        assert!(e.contains("avoir"), "{e}");
    }

    #[test]
    fn un_mode_invente_est_refuse() {
        assert!(verifier_mode_encaissement("cadeau").is_err());
        assert!(verifier_mode_encaissement("").is_err());
    }

    #[test]
    fn un_montant_nul_ou_negatif_est_refuse() {
        assert!(verifier_montant(0).is_err());
        assert!(verifier_montant(-50_000).is_err());
        assert!(verifier_montant(1).is_ok());
    }

    #[test]
    fn une_ligne_saine_passe() {
        assert!(verifier_ligne(2.0, 1.0, 800).is_ok());
        assert!(verifier_ligne(0.5, 12.0, 0).is_ok(), "prix nul = cadeau, accepte");
    }

    #[test]
    fn une_quantite_negative_ou_nulle_est_refusee() {
        assert!(verifier_ligne(-5.0, 1.0, 800).is_err());
        assert!(verifier_ligne(0.0, 1.0, 800).is_err());
        assert!(verifier_ligne(f64::NAN, 1.0, 800).is_err());
        assert!(verifier_ligne(f64::INFINITY, 1.0, 800).is_err());
    }

    #[test]
    fn un_facteur_ou_un_prix_absurde_est_refuse() {
        assert!(verifier_ligne(1.0, 0.0, 800).is_err());
        assert!(verifier_ligne(1.0, -1.0, 800).is_err());
        assert!(verifier_ligne(1.0, 1.0, -1).is_err());
    }

    #[test]
    fn un_montant_ou_une_quantite_enorme_est_refuse() {
        assert!(borner_montant(MONTANT_MAX, "la prime").is_ok());
        let e = borner_montant(MONTANT_MAX + 1, "la prime").unwrap_err();
        assert_eq!(e, "Montant trop grand pour la prime : au plus 1 000 000 000 F par saisie. Vérifier le nombre de zéros.");
        assert!(borner_montant(i64::MAX, "l'avance").is_err());
        assert!(borner_quantite(3.0).is_ok());
        assert!(borner_quantite(QUANTITE_MAX + 1.0).is_err());
        assert!(borner_quantite(f64::INFINITY).is_err());
        assert!(borner_quantite(0.0).is_err());
    }

    #[test]
    fn une_somme_qui_deborde_est_refusee_sans_paniquer() {
        assert_eq!(somme([1, 2, 3]), Ok(6));
        assert_eq!(somme([]), Ok(0));
        assert_eq!(somme([-5, 2]), Ok(-3));
        assert!(somme([i64::MAX, 1]).unwrap_err().contains("hors limites"));
    }

    #[test]
    fn la_remise_reste_entre_0_et_100() {
        assert!(verifier_remise_pct(0.0).is_ok());
        assert!(verifier_remise_pct(100.0).is_ok());
        assert!(verifier_remise_pct(-1.0).is_err());
        assert!(verifier_remise_pct(150.0).is_err());
    }
}
