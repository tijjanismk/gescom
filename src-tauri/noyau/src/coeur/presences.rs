//! Les jours travailles (PLAN-EQUIPE, F-3). Pur.
//!
//! Une grille du mois : present, absent, demi-journee. Un jour non
//! marque n'est ni present ni absent — il n'est pas su. La paie d'un
//! journalier lit le compte (G-2).

use chrono::{Datelike, NaiveDate};

pub const ETATS: &[&str] = &["present", "absent", "demi"];

/// Ce que vaut un etat, en jours.
pub fn valeur(etat: &str) -> Option<f64> {
    match etat {
        "present" => Some(1.0),
        "demi" => Some(0.5),
        "absent" => Some(0.0),
        _ => None,
    }
}

/// Les jours travailles d'une liste d'etats.
pub fn jours_travailles<'a>(etats: impl IntoIterator<Item = &'a str>) -> f64 {
    // La somme d'une liste vide de f64 vaut -0.0 (element neutre de
    // Rust) : l'ecran affichait « -0 ». `+ 0.0` le ramene a zero.
    etats.into_iter().filter_map(valeur).sum::<f64>() + 0.0
}

/// Les jours d'un mois `AAAA-MM`, en `AAAA-MM-JJ`.
pub fn jours_du_mois(mois: &str) -> Result<Vec<String>, String> {
    let premier = NaiveDate::parse_from_str(&format!("{}-01", mois.trim()), "%Y-%m-%d")
        .map_err(|_| format!("Mois illisible : « {} » (attendu AAAA-MM).", mois.trim()))?;
    let mut v = Vec::new();
    let mut d = premier;
    while d.month() == premier.month() {
        v.push(d.format("%Y-%m-%d").to_string());
        d = d.succ_opt().ok_or("Date hors limites")?;
    }
    Ok(v)
}

/// Peut-on marquer ce jour pour cette personne ? Ni avant son entree,
/// ni apres son depart, ni un jour qui n'est pas encore arrive.
pub fn marquable(jour: &str, aujourd_hui: &str, entree: Option<&str>, depart: Option<&str>) -> Result<(), String> {
    if jour > aujourd_hui {
        return Err(format!("Le {} n'est pas encore arrivé.", crate::coeur::dates::en_lettres(jour)));
    }
    if let Some(e) = entree.filter(|e| jour < *e) {
        return Err(format!("Entrée le {} : ce jour est avant.", crate::coeur::dates::en_lettres(e)));
    }
    if let Some(d) = depart.filter(|d| jour > *d) {
        return Err(format!("Partie le {} : ce jour est après.", crate::coeur::dates::en_lettres(d)));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn present_un_jour_demi_la_moitie_absent_rien() {
        assert_eq!(jours_travailles(["present", "demi", "absent", "present", "inconnu"]), 2.5);
        let rien = jours_travailles([]);
        assert!(rien == 0.0 && rien.is_sign_positive(), "pas de « -0 » à l'écran");
    }

    #[test]
    fn un_mois_a_ses_jours() {
        assert_eq!(jours_du_mois("2026-02").unwrap().len(), 28);
        assert_eq!(jours_du_mois("2028-02").unwrap().len(), 29);
        let s = jours_du_mois("2026-09").unwrap();
        assert_eq!((s.first().unwrap().as_str(), s.last().unwrap().as_str()), ("2026-09-01", "2026-09-30"));
        assert!(jours_du_mois("septembre").is_err());
    }

    #[test]
    fn ni_avant_l_entree_ni_apres_le_depart_ni_demain() {
        assert!(marquable("2026-09-10", "2026-09-24", Some("2026-09-01"), None).is_ok());
        assert!(marquable("2026-09-25", "2026-09-24", None, None).unwrap_err().contains("pas encore arrivé"));
        assert!(marquable("2026-08-31", "2026-09-24", Some("2026-09-01"), None).unwrap_err().contains("avant"));
        assert!(marquable("2026-09-21", "2026-09-24", None, Some("2026-09-20")).unwrap_err().contains("après"));
    }
}
