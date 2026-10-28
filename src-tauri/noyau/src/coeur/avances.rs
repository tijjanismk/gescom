//! Les avances sur salaire (PLAN-EQUIPE, G-1 — D32). Pur.
//!
//! Le trou le plus courant n'est pas le calcul du salaire : c'est
//! l'avance donnee au milieu du mois, sortie de la caisse sur un bout de
//! papier, et oubliee a la paie. Une avance est rattachee a la
//! personne ; ce qui n'est pas encore retenu est « en cours » et la
//! fiche de paie suivante le retient (G-2).

use crate::coeur::plafonds::francs;

/// Ce qui reste a retenir d'une avance.
pub fn reste(montant: i64, retenu: i64) -> i64 {
    montant.saturating_sub(retenu).max(0)
}

/// Peut-on donner cette avance ? `en_cours` : ce que la personne doit
/// deja en avances non retenues ; `plafond` : son plafond, s'il y en a
/// un (vide = pas de plafond). `voit_plafond` : l'appelant a-t-il le
/// droit de lire le plafond (`paie:preparer` ou `paie:valider`, comme a
/// la lecture de la fiche) ? Sans lui, le refus ne le chiffre pas
/// (revue du 28/10, point 3) : le plafond se masque partout ailleurs.
pub fn verifier(montant: i64, en_cours: i64, plafond: Option<i64>, nom: &str, voit_plafond: bool) -> Result<(), String> {
    if montant <= 0 {
        return Err("Le montant d'une avance doit être plus grand que zéro.".to_string());
    }
    crate::coeur::saisie::borner_montant(montant, "l'avance")?;
    if let Some(p) = plafond {
        // Additions verifiees : `en_cours` vient de la base.
        let apres = en_cours.checked_add(montant).ok_or("Montants hors limites.")?;
        if apres > p {
            return Err(if voit_plafond {
                format!(
                    "{nom} a déjà {} d'avances en cours ; avec {} on dépasserait son plafond de {}.",
                    francs(en_cours),
                    francs(montant),
                    francs(p)
                )
            } else {
                format!(
                    "{nom} a déjà {} d'avances en cours ; avec {} on dépasserait son plafond. \
                     Voir avec qui prépare la paie.",
                    francs(en_cours),
                    francs(montant)
                )
            });
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
        assert!(verifier(5_000, 0, None, "Awa", true).is_ok(), "sans plafond");
        assert!(verifier(5_000, 10_000, Some(15_000), "Awa", true).is_ok(), "pile au plafond");
        let e = verifier(6_000, 10_000, Some(15_000), "Awa", true).unwrap_err();
        assert_eq!(e, "Awa a déjà 10 000 F d'avances en cours ; avec 6 000 F on dépasserait son plafond de 15 000 F.");
        assert!(verifier(0, 0, None, "Awa", true).is_err());
    }

    #[test]
    fn le_refus_ne_chiffre_pas_le_plafond_a_qui_ne_le_voit_pas() {
        let e = verifier(6_000, 10_000, Some(15_000), "Awa", false).unwrap_err();
        assert!(e.contains("dépasserait son plafond") && !e.contains("15 000"), "{e}");
    }

    #[test]
    fn une_avance_enorme_est_refusee_sans_deborder() {
        assert!(verifier(i64::MAX, 0, None, "Awa", true).unwrap_err().contains("trop grand"));
        assert!(verifier(1_000, i64::MAX, Some(10), "Awa", true).unwrap_err().contains("hors limites"));
    }
}
