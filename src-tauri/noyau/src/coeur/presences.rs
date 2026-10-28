//! Les jours travailles (PLAN-EQUIPE, F-3). Pur.
//!
//! Une grille du mois : present, absent, demi-journee. Un jour non
//! marque n'est ni present ni absent — il n'est pas su. La paie d'un
//! journalier lit le compte (G-2) ; le salaire au mois au prorata, lui,
//! compte un jour non marque comme present (choix du proprietaire,
//! 28/10) : seules les absences et les demi-journees retirent.

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

/// (presents, demi-journees, absences) d'une liste d'etats, en jours
/// entiers : la paie compte en demi-journees, jamais en flottant.
pub fn compter<'a>(etats: impl IntoIterator<Item = &'a str>) -> (i64, i64, i64) {
    etats.into_iter().fold((0, 0, 0), |(p, d, a), e| match e {
        "present" => (p + 1, d, a),
        "demi" => (p, d + 1, a),
        "absent" => (p, d, a + 1),
        _ => (p, d, a),
    })
}

/// Un jour couvert par une fiche de paie validee ne se marque plus
/// (revue du 28/10, point 5, choix du proprietaire : on peut valider
/// avant la fin du mois, ensuite les jours de la periode sont figes).
pub fn refus_jour_fige(nom: &str, numero: &str) -> String {
    format!("{nom} : ce jour est sur la fiche {numero} déjà validée : faire une rectificative de cette fiche pour le changer.")
}

/// Peut-on enregistrer un depart a cette date ? Pas si des jours sont
/// marques apres (revue du 28/10, point 1) : ils seraient payes a qui
/// est deja parti, ou perdus sans qu'on le sache. `apres` : les jours
/// marques apres `depart`, dans l'ordre.
pub fn verifier_depart(nom: &str, depart: &str, apres: &[String]) -> Result<(), String> {
    let Some(dernier) = apres.last() else { return Ok(()) };
    let liste: Vec<String> = apres.iter().take(3).map(|j| crate::coeur::paie::date_dite(j)).collect();
    let suite = if apres.len() > 3 { "…" } else { "" };
    Err(format!(
        "{nom} a des jours marqués après le {} ({}{suite}) : les effacer d'abord dans Personnel → Jours travaillés, \
         ou choisir un départ le {} ou après.",
        crate::coeur::paie::date_dite(depart),
        liste.join(", "),
        crate::coeur::paie::date_dite(dernier)
    ))
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
    fn la_paie_compte_en_jours_entiers() {
        assert_eq!(compter(["present", "demi", "absent", "present", "inconnu"]), (2, 1, 1));
        assert_eq!(compter([]), (0, 0, 0));
    }

    #[test]
    fn un_depart_avant_des_jours_marques_se_refuse_en_disant_quoi_faire() {
        assert!(verifier_depart("Moussa", "2026-09-15", &[]).is_ok());
        let apres: Vec<String> = (16..=20).map(|j| format!("2026-09-{j}")).collect();
        let e = verifier_depart("Moussa", "2026-09-15", &apres).unwrap_err();
        assert!(e.starts_with("Moussa a des jours marqués après le 15/09/2026 (16/09/2026, 17/09/2026, 18/09/2026…)"), "{e}");
        assert!(e.contains("les effacer d'abord") && e.contains("un départ le 20/09/2026 ou après"), "{e}");
        assert!(refus_jour_fige("Moussa", "PAIE-2026-00003").contains("sur la fiche PAIE-2026-00003 déjà validée : faire une rectificative"));
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
