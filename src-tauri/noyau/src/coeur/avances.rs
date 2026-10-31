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

/// L'etalement d'une avance (D42) : vide, ou de 1 F au montant. Rend ce
/// qui s'ecrit : vide quand la mensualite couvre toute l'avance (« tout
/// sur la prochaine paie », la regle d'avant).
pub fn verifier_mensualite(mensualite: Option<i64>, montant: i64) -> Result<Option<i64>, String> {
    match mensualite {
        None => Ok(None),
        Some(m) if m <= 0 => Err("La retenue par paie doit être plus grande que zéro — ou vide pour tout retenir.".to_string()),
        Some(m) if m >= montant => Ok(None),
        Some(m) => Ok(Some(m)),
    }
}

/// L'argent pris dans le tiroir (D42) : des especes seulement — Orange
/// Money, Moov ou un virement ne sortent pas du tiroir.
pub fn verifier_tiroir(moyen: &str, du_tiroir: bool) -> Result<(), String> {
    if du_tiroir && moyen != "especes" {
        return Err("Seules les espèces se prennent dans le tiroir.".to_string());
    }
    Ok(())
}

/// La date d'une avance (D42) : aujourd'hui par defaut ; une date passee
/// se donne, jamais future, 31 jours en arriere au plus (la regle des
/// ventes, `coeur::dates`), jamais avant l'entree. Prise dans le tiroir,
/// elle est d'aujourd'hui : l'argent sort maintenant, de la caisse
/// ouverte. Rend le jour, `AAAA-MM-JJ`.
pub fn verifier_date(
    date: Option<&str>,
    aujourd_hui: chrono::NaiveDate,
    entree: Option<&str>,
    du_tiroir: bool,
) -> Result<String, String> {
    let jour = match date.map(str::trim).filter(|d| !d.is_empty()) {
        None => return Ok(aujourd_hui.format("%Y-%m-%d").to_string()),
        Some(d) => crate::coeur::dates::verifier_date_saisie(d, aujourd_hui)?,
    };
    let iso = jour.format("%Y-%m-%d").to_string();
    if du_tiroir && jour != aujourd_hui {
        return Err(
            "Une avance prise dans le tiroir est datée d'aujourd'hui : l'argent sort de la caisse ouverte. \
             Pour une avance d'un autre jour, décocher « pris dans le tiroir »."
                .to_string(),
        );
    }
    if let Some(e) = entree.filter(|e| iso.as_str() < &e[..e.len().min(10)]) {
        return Err(format!(
            "Entrée le {} : une avance ne peut pas venir avant.",
            crate::coeur::paie::date_dite(e)
        ));
    }
    Ok(iso)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jour(iso: &str) -> chrono::NaiveDate {
        chrono::NaiveDate::parse_from_str(iso, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn la_mensualite_va_de_1_f_au_montant() {
        assert_eq!(verifier_mensualite(None, 30_000), Ok(None));
        assert_eq!(verifier_mensualite(Some(10_000), 30_000), Ok(Some(10_000)));
        assert_eq!(verifier_mensualite(Some(30_000), 30_000), Ok(None), "tout : la regle d'avant");
        assert_eq!(verifier_mensualite(Some(50_000), 30_000), Ok(None));
        assert!(verifier_mensualite(Some(0), 30_000).is_err());
        assert!(verifier_mensualite(Some(-5), 30_000).is_err());
    }

    #[test]
    fn le_tiroir_ne_donne_que_des_especes() {
        assert!(verifier_tiroir("especes", true).is_ok());
        assert!(verifier_tiroir("orange_money", false).is_ok());
        assert!(verifier_tiroir("orange_money", true).unwrap_err().contains("espèces"));
    }

    #[test]
    fn la_date_d_une_avance() {
        let auj = jour("2026-10-28");
        assert_eq!(verifier_date(None, auj, None, true).unwrap(), "2026-10-28");
        assert_eq!(verifier_date(Some("2026-10-12"), auj, Some("2026-01-05"), false).unwrap(), "2026-10-12");
        assert!(verifier_date(Some("2026-10-29"), auj, None, false).unwrap_err().contains("futur"));
        assert!(verifier_date(Some("2026-09-20"), auj, None, false).unwrap_err().contains("31 jours"));
        let e = verifier_date(Some("2026-10-12"), auj, Some("2026-10-15"), false).unwrap_err();
        assert!(e.contains("15/10/2026"), "{e}");
        assert!(verifier_date(Some("2026-10-12"), auj, None, true).unwrap_err().contains("tiroir"));
        assert_eq!(verifier_date(Some("2026-10-28"), auj, None, true).unwrap(), "2026-10-28");
    }

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
