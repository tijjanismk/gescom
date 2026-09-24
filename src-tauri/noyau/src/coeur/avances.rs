//! Les avances sur salaire (PLAN-EQUIPE, G-1 — D32). Pur.
//!
//! Le trou le plus courant n'est pas le calcul du salaire : c'est
//! l'avance donnee au milieu du mois, sortie de la caisse sur un bout de
//! papier, et oubliee a la paie. Une avance est une sortie de caisse
//! rattachee a la personne ; ce qui n'est pas encore retenu est « en
//! cours » et la fiche de paie suivante le retient (G-2).

/// Ce qui reste a retenir d'une avance.
pub fn reste(montant: i64, retenu: i64) -> i64 {
    (montant - retenu).max(0)
}

/// Peut-on donner cette avance ? `en_cours` : ce que la personne doit
/// deja en avances non retenues ; `plafond` : son plafond, s'il y en a
/// un (vide = pas de plafond).
pub fn verifier(montant: i64, en_cours: i64, plafond: Option<i64>, nom: &str) -> Result<(), String> {
    if montant <= 0 {
        return Err("Le montant d'une avance doit être plus grand que zéro.".to_string());
    }
    if let Some(p) = plafond {
        if en_cours + montant > p {
            return Err(format!(
                "{nom} a déjà {} d'avances en cours ; avec {} on dépasserait son plafond de {}.",
                crate::coeur::plafonds::francs(en_cours),
                crate::coeur::plafonds::francs(montant),
                crate::coeur::plafonds::francs(p)
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ce_qui_reste_a_retenir() {
        assert_eq!(reste(10_000, 4_000), 6_000);
        assert_eq!(reste(10_000, 12_000), 0);
    }

    #[test]
    fn le_plafond_compte_ce_qui_est_deja_en_cours() {
        assert!(verifier(5_000, 0, None, "Awa").is_ok(), "sans plafond");
        assert!(verifier(5_000, 10_000, Some(15_000), "Awa").is_ok(), "pile au plafond");
        let e = verifier(6_000, 10_000, Some(15_000), "Awa").unwrap_err();
        assert_eq!(e, "Awa a déjà 10 000 F d'avances en cours ; avec 6 000 F on dépasserait son plafond de 15 000 F.");
        assert!(verifier(0, 0, None, "Awa").is_err());
    }
}
