//! Prevenir AVANT la fin des dates de travail (D21).
//!
//! Trouve le 27/10/2026 en relisant : chaque dossier amorce en 2026 a
//! un exercice qui finit le 31/12/2026. Le lendemain, toute ecriture
//! datee est refusee, et seul le patron (`dossiers:gerer`) peut
//! prolonger ou ouvrir la suite. Rien ne le disait avant : un caissier
//! seul le 2 janvier au matin ne pouvait plus vendre.
//!
//! On ne touche pas au refus (D21) : on l'annonce. Ce module dit QUAND
//! l'annoncer, a partir du nombre de jours qui restent. Quelle est la
//! derniere date ecrivable, c'est `dossiers::derniere_date_ecrivable`
//! qui le dit — elle juge avec `dossier_accepte`, le meme juge que le
//! serveur, pour que l'avis annonce exactement le refus.
//!
//! Pure : la date du jour est passee, la regle se teste sans attendre
//! decembre.

use chrono::NaiveDate;

/// L'avis commence un mois avant : de quoi joindre un patron en voyage
/// ou a la Tabaski. Plus tot, le bandeau resterait affiche deux mois et
/// on cesserait de le lire.
pub const PREAVIS_JOURS: i64 = 30;

/// La derniere semaine, l'avis passe au rouge : un bandeau orange
/// affiche depuis un mois ne se voit plus.
pub const URGENCE_JOURS: i64 = 7;

/// Des dates de travail longues se donnent parfois — plus de deux ans
/// pour une boutique qui ne veut pas y repenser. Mais une prolongation
/// ne se raccourcit pas, et l'exercice suivant ne commence que le
/// lendemain : une faute d'annee (2062 pour 2026) bloquerait le
/// dossier des dizaines d'annees, sans remede. Cinq ans couvrent le
/// besoin et arretent la faute de frappe.
pub const DUREE_MAX_ANS: u32 = 5;

/// Le dernier jour accepte pour une fin comptee depuis `depuis` :
/// `DUREE_MAX_ANS` ans plus tard, ce jour inclus. `None` sur une date
/// illisible.
pub fn fin_au_plus_tard(depuis: &str) -> Option<NaiveDate> {
    crate::coeur::dates::jour(depuis)?.checked_add_months(chrono::Months::new(12 * DUREE_MAX_ANS))
}

/// Ce que le tableau de bord doit dire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Avis {
    /// Plus d'un mois : rien a dire.
    Rien,
    /// Dans le mois.
    Proche,
    /// Dans la semaine, ou c'est le dernier jour.
    Urgent,
    /// Aujourd'hui ne s'ecrit deja plus.
    Bloque,
}

impl Avis {
    /// Le mot que lit l'ecran.
    pub fn code(self) -> &'static str {
        match self {
            Avis::Rien => "rien",
            Avis::Proche => "proche",
            Avis::Urgent => "urgent",
            Avis::Bloque => "bloque",
        }
    }
}

/// Les jours qui restent APRES aujourd'hui jusqu'a `fin`, incluse :
/// 0, c'est le dernier jour ou l'on ecrit. `None` sur une date
/// illisible.
pub fn jours_restants(fin: &str, aujourd_hui: NaiveDate) -> Option<i64> {
    let fin = crate::coeur::dates::jour(fin)?;
    Some((fin - aujourd_hui).num_days())
}

/// L'avis, vu les jours qui restent. `None` : aujourd'hui n'est deja
/// plus ecrivable.
pub fn avis(jours_restants: Option<i64>) -> Avis {
    match jours_restants {
        Some(j) if j > PREAVIS_JOURS => Avis::Rien,
        Some(j) if j > URGENCE_JOURS => Avis::Proche,
        Some(j) if j >= 0 => Avis::Urgent,
        _ => Avis::Bloque,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn le(annee: i32, mois: u32, jour: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(annee, mois, jour).unwrap()
    }

    #[test]
    fn les_jours_se_comptent_apres_aujourd_hui() {
        assert_eq!(jours_restants("2026-12-31", le(2026, 12, 31)), Some(0), "le dernier jour");
        assert_eq!(jours_restants("2026-12-31", le(2026, 12, 1)), Some(30));
        assert_eq!(jours_restants("2026-12-31", le(2026, 10, 27)), Some(65));
        assert_eq!(jours_restants("2026-12-31", le(2027, 1, 2)), Some(-2));
        assert_eq!(jours_restants("31/12/2026", le(2026, 12, 1)), None);
    }

    #[test]
    fn l_avis_commence_un_mois_avant_et_rougit_la_derniere_semaine() {
        assert_eq!(avis(Some(65)), Avis::Rien);
        assert_eq!(avis(Some(PREAVIS_JOURS + 1)), Avis::Rien);
        assert_eq!(avis(Some(PREAVIS_JOURS)), Avis::Proche, "le 1er décembre pour un 31 décembre");
        assert_eq!(avis(Some(URGENCE_JOURS + 1)), Avis::Proche);
        assert_eq!(avis(Some(URGENCE_JOURS)), Avis::Urgent);
        assert_eq!(avis(Some(0)), Avis::Urgent, "le dernier jour on écrit encore");
    }

    #[test]
    fn une_fin_passee_ou_absente_bloque() {
        assert_eq!(avis(Some(-1)), Avis::Bloque);
        assert_eq!(avis(None), Avis::Bloque);
    }

    #[test]
    fn une_fin_va_au_plus_cinq_ans_plus_loin() {
        assert_eq!(fin_au_plus_tard("2026-12-31"), Some(le(2031, 12, 31)));
        assert_eq!(fin_au_plus_tard("2027-01-01T08:00:00"), Some(le(2032, 1, 1)));
        // Un 29 fevrier sans 29 fevrier cinq ans plus tard : le dernier
        // jour du mois.
        assert_eq!(fin_au_plus_tard("2028-02-29"), Some(le(2033, 2, 28)));
        assert_eq!(fin_au_plus_tard("fin d'année"), None);
    }

    #[test]
    fn les_codes_que_lit_l_ecran() {
        let codes: Vec<_> = [Avis::Rien, Avis::Proche, Avis::Urgent, Avis::Bloque].iter().map(|a| a.code()).collect();
        assert_eq!(codes, ["rien", "proche", "urgent", "bloque"]);
    }
}
