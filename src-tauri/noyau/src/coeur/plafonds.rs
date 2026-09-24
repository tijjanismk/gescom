//! Des plafonds, pas seulement des portes (v3, C-3).
//!
//! > Le caissier peut faire une remise, mais pas 40 %. Il peut
//! > rembourser un retour, mais au-dela de 50 000 F c'est le patron qui
//! > valide.
//!
//! Trois plafonds, portes par le ROLE et ajustables par personne :
//! `remise_max_pct` (la remise d'une ligne ou d'une piece),
//! `remboursement_max` (l'argent qui ressort du tiroir en un geste),
//! `credit_max` (ce qu'une vente laisse a credit). Vide = pas de
//! plafond. Un role a acces total n'en a jamais.
//!
//! Le depassement est un refus clair, qui dit quoi faire : pas de
//! « validation a distance » dans cette v3 — le patron est dans la
//! boutique, il fait le geste avec son compte.

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct Plafonds {
    #[serde(default)]
    pub remise_max_pct: Option<f64>,
    #[serde(default)]
    pub remboursement_max: Option<i64>,
    #[serde(default)]
    pub credit_max: Option<i64>,
}

impl Plafonds {
    pub fn est_vide(&self) -> bool {
        self.remise_max_pct.is_none() && self.remboursement_max.is_none() && self.credit_max.is_none()
    }
}

/// Le sur-mesure d'une personne l'emporte, plafond par plafond, sur
/// celui de son role.
pub fn fusionner(role: &Plafonds, personne: &Plafonds) -> Plafonds {
    Plafonds {
        remise_max_pct: personne.remise_max_pct.or(role.remise_max_pct),
        remboursement_max: personne.remboursement_max.or(role.remboursement_max),
        credit_max: personne.credit_max.or(role.credit_max),
    }
}

/// Un plafond se saisit : on le juge comme une saisie (D27).
pub fn valider(p: &Plafonds) -> Result<(), String> {
    if let Some(r) = p.remise_max_pct {
        if !(0.0..=100.0).contains(&r) || r.is_nan() {
            return Err(format!("Plafond de remise impossible : {r} % (entre 0 et 100)."));
        }
    }
    for (nom, v) in [("remboursement", p.remboursement_max), ("crédit", p.credit_max)] {
        if v.is_some_and(|m| m < 0) {
            return Err(format!("Plafond de {nom} négatif : refusé."));
        }
    }
    Ok(())
}

/// La remise d'une ligne, en pourcentage du prix de reference. Une
/// hausse n'est pas une remise.
pub fn remise_pct(prix_reference: i64, prix_pratique: i64) -> f64 {
    if prix_reference <= 0 || prix_pratique >= prix_reference {
        return 0.0;
    }
    (prix_reference - prix_pratique) as f64 * 100.0 / prix_reference as f64
}

/// Ce qu'une vente laisse a credit : son total, moins ce qui est paye
/// maintenant et l'avoir applique.
pub fn credit_de_la_vente(montants_lignes: &[i64], paye: i64, avoir: i64) -> i64 {
    (montants_lignes.iter().sum::<i64>() - paye.max(0) - avoir.max(0)).max(0)
}

fn pct(v: f64) -> String {
    let arrondi = (v * 10.0).round() / 10.0;
    if arrondi.fract() == 0.0 {
        format!("{}", arrondi as i64)
    } else {
        format!("{arrondi}").replace('.', ",")
    }
}

/// `50000` → `50 000 F`.
pub fn francs(n: i64) -> String {
    let brut = n.abs().to_string();
    let mut groupes = Vec::new();
    let mut fin = brut.len();
    while fin > 3 {
        groupes.push(&brut[fin - 3..fin]);
        fin -= 3;
    }
    groupes.push(&brut[..fin]);
    groupes.reverse();
    format!("{}{} F", if n < 0 { "-" } else { "" }, groupes.join(" "))
}

const DEMANDER: &str = "Demander au patron.";

/// Une remise au-dela du plafond est refusee. La tolerance d'un
/// centieme evite de refuser 15 % a cause d'un arrondi au franc.
pub fn verifier_remise(remise: f64, p: &Plafonds) -> Result<(), String> {
    match p.remise_max_pct {
        Some(max) if remise > max + 0.01 => Err(format!(
            "Remise de {} % — votre plafond est {} %. {DEMANDER}",
            pct(remise),
            pct(max)
        )),
        _ => Ok(()),
    }
}

pub fn verifier_remboursement(montant: i64, p: &Plafonds) -> Result<(), String> {
    match p.remboursement_max {
        Some(max) if montant > max => Err(format!(
            "Remboursement de {} — votre plafond est {}. {DEMANDER}",
            francs(montant),
            francs(max)
        )),
        _ => Ok(()),
    }
}

pub fn verifier_credit(montant: i64, p: &Plafonds) -> Result<(), String> {
    match p.credit_max {
        Some(max) if montant > max => Err(format!(
            "Crédit de {} sur cette vente — votre plafond est {}. {DEMANDER}",
            francs(montant),
            francs(max)
        )),
        _ => Ok(()),
    }
}

/// Une vente du comptoir : chaque ligne sous le plafond de remise
/// (prix pratique contre prix de reference), et, a credit, ce qu'elle
/// laisse a credit sous le plafond de credit. `lignes` :
/// (prix de reference, prix pratique, quantite).
pub fn verifier_vente(
    lignes: &[(i64, i64, f64)],
    a_credit: bool,
    paye: i64,
    avoir: i64,
    p: &Plafonds,
) -> Result<(), String> {
    if p.est_vide() {
        return Ok(());
    }
    for (reference, pratique, _) in lignes {
        verifier_remise(remise_pct(*reference, *pratique), p)?;
    }
    if a_credit {
        let montants: Vec<i64> = lignes
            .iter()
            .map(|(_, pratique, qte)| crate::coeur::calcul::montant_ligne(*pratique, *qte))
            .collect();
        verifier_credit(credit_de_la_vente(&montants, paye, avoir), p)?;
    }
    Ok(())
}

/// Une piece : la remise de chaque ligne et la remise globale.
pub fn verifier_piece(remises_lignes: &[f64], remise_globale: Option<f64>, p: &Plafonds) -> Result<(), String> {
    for r in remises_lignes.iter().copied().chain(remise_globale) {
        verifier_remise(r, p)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn une_vente_se_juge_ligne_par_ligne_et_le_credit_seulement_a_credit() {
        let p = caissier();
        // 1000 -> 850 : 15 %, permis ; 1000 -> 800 : 20 %, refuse.
        assert!(verifier_vente(&[(1000, 850, 2.0)], false, 0, 0, &p).is_ok());
        let e = verifier_vente(&[(1000, 850, 1.0), (1000, 800, 1.0)], false, 0, 0, &p).unwrap_err();
        assert!(e.starts_with("Remise de 20 %"), "{e}");
        // 120 000 a credit, dont 30 000 payes : 90 000 laisses, permis.
        assert!(verifier_vente(&[(60_000, 60_000, 2.0)], true, 30_000, 0, &p).is_ok());
        assert!(verifier_vente(&[(60_000, 60_000, 2.0)], true, 0, 0, &p).is_err());
        // Au comptant, le credit ne se juge pas.
        assert!(verifier_vente(&[(60_000, 60_000, 2.0)], false, 0, 0, &p).is_ok());
        // Sans plafond, rien.
        assert!(verifier_vente(&[(1000, 1, 1.0)], true, 0, 0, &Plafonds::default()).is_ok());
    }

    #[test]
    fn une_piece_juge_ses_lignes_et_sa_remise_globale() {
        let p = caissier();
        assert!(verifier_piece(&[10.0, 15.0], Some(5.0), &p).is_ok());
        assert!(verifier_piece(&[10.0], Some(30.0), &p).is_err());
        assert!(verifier_piece(&[16.0], None, &p).is_err());
    }

    fn caissier() -> Plafonds {
        Plafonds { remise_max_pct: Some(15.0), remboursement_max: Some(50_000), credit_max: Some(100_000) }
    }

    #[test]
    fn le_refus_dit_le_chiffre_le_plafond_et_quoi_faire() {
        assert_eq!(
            verifier_remise(40.0, &caissier()).unwrap_err(),
            "Remise de 40 % — votre plafond est 15 %. Demander au patron."
        );
        assert_eq!(
            verifier_remboursement(60_000, &caissier()).unwrap_err(),
            "Remboursement de 60 000 F — votre plafond est 50 000 F. Demander au patron."
        );
        assert!(verifier_credit(150_000, &caissier()).unwrap_err().starts_with("Crédit de 150 000 F"));
    }

    #[test]
    fn au_plafond_c_est_permis_au_dela_non() {
        let p = caissier();
        assert!(verifier_remise(15.0, &p).is_ok());
        assert!(verifier_remise(remise_pct(1000, 850), &p).is_ok(), "15 % pile");
        assert!(verifier_remise(remise_pct(1000, 849), &p).is_err());
        assert!(verifier_remboursement(50_000, &p).is_ok());
        assert!(verifier_remboursement(50_001, &p).is_err());
        assert!(verifier_credit(100_000, &p).is_ok());
    }

    #[test]
    fn sans_plafond_tout_passe() {
        let p = Plafonds::default();
        assert!(verifier_remise(99.0, &p).is_ok());
        assert!(verifier_remboursement(10_000_000, &p).is_ok());
        assert!(verifier_credit(10_000_000, &p).is_ok());
    }

    #[test]
    fn la_personne_l_emporte_plafond_par_plafond() {
        let perso = Plafonds { remise_max_pct: Some(25.0), ..Default::default() };
        let f = fusionner(&caissier(), &perso);
        assert_eq!(f.remise_max_pct, Some(25.0));
        assert_eq!(f.remboursement_max, Some(50_000), "le reste suit le rôle");
    }

    #[test]
    fn une_hausse_n_est_pas_une_remise_et_le_credit_ne_descend_pas_sous_zero() {
        assert_eq!(remise_pct(1000, 1200), 0.0);
        assert_eq!(remise_pct(0, 0), 0.0);
        assert!((remise_pct(1000, 875) - 12.5).abs() < 1e-9);
        assert_eq!(credit_de_la_vente(&[1000, 500], 300, 200), 1000);
        assert_eq!(credit_de_la_vente(&[1000], 5000, 0), 0);
        assert_eq!(pct(12.5), "12,5");
    }

    #[test]
    fn un_plafond_saisi_se_juge() {
        assert!(valider(&Plafonds { remise_max_pct: Some(120.0), ..Default::default() }).is_err());
        assert!(valider(&Plafonds { credit_max: Some(-1), ..Default::default() }).is_err());
        assert!(valider(&caissier()).is_ok());
        assert_eq!(francs(0), "0 F");
        assert_eq!(francs(1_234_567), "1 234 567 F");
    }
}
