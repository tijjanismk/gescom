//! Le pret au personnel sur plusieurs mois (PLAN-EQUIPE EQ-1 — D47). Pur.
//!
//! 60 000 F pretes pour la Tabaski, retenus 10 000 F par mois : un pret
//! est une avance d'une autre nature (`avance.nature = 'pret'`). Ce qui
//! le distingue d'une avance etalee (D42) :
//! - l'echeance est fixe et **par mois civil** : un journalier paye a la
//!   quinzaine ne la rembourse qu'une fois par mois ;
//! - la premiere retenue tombe le mois suivant le pret, reculable de
//!   trois mois au plus ;
//! - une echeance manquee (net trop maigre) **allonge** le pret, elle ne
//!   se double pas le mois d'apres ;
//! - sur une fiche maigre, les avances passent d'abord ;
//! - la fiche du depart retient tout le reste, dans la limite du net.

use crate::coeur::paie::{date_dite, Ligne, AVANCE};
use crate::coeur::plafonds::francs;

/// La nature d'une avance qui est un pret.
pub const NATURE: &str = "pret";
/// Les deux reglements d'un pret hors paie.
pub const REMBOURSEMENT: &str = "remboursement";
pub const REMISE: &str = "remise";
/// La premiere retenue se recule de trois mois au plus.
pub const DIFFERE_MAX: i64 = 3;

const MOIS: [&str; 12] = [
    "janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août", "septembre", "octobre", "novembre", "décembre",
];

/// Un pret a retenir sur une fiche : ce qui en reste (en rendant ce que
/// la fiche remplacee en avait retenu), l'echeance, le premier mois, et
/// ce que les autres fiches validees du meme mois en ont deja retenu.
#[derive(Debug, Clone)]
pub struct PretARetenir {
    pub id: String,
    pub date: String,
    pub reste: i64,
    pub mensualite: i64,
    /// `AAAA-MM`.
    pub retenir_des: String,
    pub deja_ce_mois: i64,
}

/// Le mois `AAAA-MM` d'un jour `AAAA-MM-JJ…`.
pub fn mois_de(jour: &str) -> Result<String, String> {
    let (a, m) = lire_mois(jour.get(..7).unwrap_or(jour))?;
    Ok(format!("{a:04}-{m:02}"))
}

fn lire_mois(mois: &str) -> Result<(i64, i64), String> {
    let illisible = || format!("Mois illisible : « {mois} ».");
    let (a, m) = mois.split_once('-').ok_or_else(illisible)?;
    let a: i64 = a.parse().map_err(|_| illisible())?;
    let m: i64 = m.get(..2).unwrap_or(m).parse().map_err(|_| illisible())?;
    if !(1..=12).contains(&m) || !(1900..=9999).contains(&a) {
        return Err(illisible());
    }
    Ok((a, m))
}

/// `mois` + `n` mois.
pub fn mois_plus(mois: &str, n: i64) -> Result<String, String> {
    let (a, m) = lire_mois(mois)?;
    let t = a * 12 + (m - 1) + n;
    Ok(format!("{:04}-{:02}", t.div_euclid(12), t.rem_euclid(12) + 1))
}

/// « juillet 2026 ».
pub fn mois_dit(mois: &str) -> String {
    match lire_mois(mois) {
        Ok((a, m)) => format!("{} {a}", MOIS[(m - 1) as usize]),
        Err(_) => mois.to_string(),
    }
}

/// Peut-on preter ? Rend l'echeance. `deja` : la personne a deja un pret
/// en cours (un seul a la fois).
pub fn verifier(montant: i64, mensualite: Option<i64>, deja: bool, nom: &str) -> Result<i64, String> {
    if montant <= 0 {
        return Err("Le montant d'un prêt doit être plus grand que zéro.".to_string());
    }
    crate::coeur::saisie::borner_montant(montant, "le prêt")?;
    let m = mensualite.ok_or("Un prêt dit combien se retient chaque mois.")?;
    if m <= 0 {
        return Err("La retenue par mois doit être plus grande que zéro.".to_string());
    }
    if m >= montant {
        return Err(format!(
            "Une retenue de {} rembourse tout en une fois : c'est une avance, pas un prêt.",
            francs(m)
        ));
    }
    if deja {
        return Err(format!("{nom} a déjà un prêt en cours : un seul à la fois. Le solder d'abord."));
    }
    Ok(m)
}

/// Le premier mois retenu : le mois suivant le pret par defaut, reculable
/// de trois mois au plus (`DIFFERE_MAX`), jamais le mois du pret.
pub fn premier_mois(date_pret: &str, choisi: Option<&str>) -> Result<String, String> {
    let suivant = mois_plus(&mois_de(date_pret)?, 1)?;
    let Some(c) = choisi.map(str::trim).filter(|c| !c.is_empty()) else {
        return Ok(suivant);
    };
    let c = mois_de(c)?;
    let dernier = mois_plus(&suivant, DIFFERE_MAX)?;
    if c < suivant || c > dernier {
        return Err(format!(
            "La première retenue tombe entre {} et {}.",
            mois_dit(&suivant),
            mois_dit(&dernier)
        ));
    }
    Ok(c)
}

/// Ce que la fiche du mois `mois` doit retenir de ce pret, avant de
/// regarder le net. `depart` : la fiche contient le jour du depart — tout
/// le reste (solde de tout compte).
pub fn echeance(p: &PretARetenir, mois: &str, depart: bool) -> i64 {
    let reste = p.reste.max(0);
    if depart {
        return reste;
    }
    if mois < p.retenir_des.as_str() {
        return 0;
    }
    p.mensualite.saturating_sub(p.deja_ce_mois).max(0).min(reste)
}

/// Retient les prets sur ce qui reste du net apres les avances (D47 :
/// les avances d'abord), les plus anciens d'abord. Rend les lignes (du
/// genre `avance` : leur source est le pret) et ce qui manque aux
/// echeances — un pret en avance sur son echeancier ne « reporte » rien.
pub fn retenir_prets(disponible: i64, prets: &[PretARetenir], mois: &str, depart: bool) -> (Vec<Ligne>, i64) {
    let mut prets: Vec<&PretARetenir> = prets.iter().filter(|p| p.reste > 0).collect();
    prets.sort_by(|a, b| a.date.cmp(&b.date).then(a.id.cmp(&b.id)));
    let mut dispo = disponible.max(0);
    let mut lignes = Vec::new();
    let mut manque: i64 = 0;
    for p in prets {
        let du = echeance(p, mois, depart);
        let pris = du.min(dispo);
        dispo -= pris;
        manque = manque.saturating_add(du - pris);
        if pris == 0 {
            continue;
        }
        let apres = p.reste - pris;
        let quand = date_dite(&p.date);
        let libelle = match (depart, pris < du, apres) {
            (true, false, _) => format!("Prêt du {quand} : soldé au départ"),
            (true, true, _) => format!("Prêt du {quand} : {} retenus au départ ; {} restent dus", francs(pris), francs(apres)),
            (false, true, _) => format!(
                "Prêt du {quand} (échéance {}, {} faute de net ; reste {})",
                francs(du),
                francs(du - pris),
                francs(apres)
            ),
            (false, false, 0) => format!("Prêt du {quand} : dernière échéance"),
            (false, false, _) => format!("Prêt du {quand} (reste {} après cette paie)", francs(apres)),
        };
        lignes.push(Ligne { genre: AVANCE.into(), libelle, quantite: None, prix: None, montant: -pris, source: Some(p.id.clone()), saisie: false });
    }
    (lignes, manque)
}

/// Le mois de la derniere echeance, si tout se passe comme prevu : a
/// partir du premier mois, du mois qui suit la derniere retenue, et du
/// mois en cours (le plus tard des trois). `None` : soldé.
pub fn fin_prevue(reste: i64, mensualite: i64, retenir_des: &str, dernier_retenu: Option<&str>, mois_courant: &str) -> Option<String> {
    if reste <= 0 || mensualite <= 0 {
        return None;
    }
    let mut prochain = retenir_des.to_string();
    if let Some(d) = dernier_retenu.and_then(|d| mois_plus(d, 1).ok()) {
        prochain = prochain.max(d);
    }
    prochain = prochain.max(mois_courant.to_string());
    let echeances = (reste + mensualite - 1) / mensualite;
    mois_plus(&prochain, echeances - 1).ok()
}

/// Un remboursement direct (une partie ou tout) ou une remise (tout le
/// reste). Rend le montant a noter.
pub fn verifier_reglement(genre: &str, montant: Option<i64>, reste: i64) -> Result<i64, String> {
    if reste <= 0 {
        return Err("Ce prêt est soldé : plus rien à régler.".to_string());
    }
    match genre {
        REMISE => Ok(reste),
        REMBOURSEMENT => {
            let m = montant.ok_or("Combien est remboursé ?")?;
            if m <= 0 {
                return Err("Le remboursement doit être plus grand que zéro.".to_string());
            }
            crate::coeur::saisie::borner_montant(m, "le remboursement")?;
            if m > reste {
                return Err(format!("Il ne reste que {} à rembourser.", francs(reste)));
            }
            Ok(m)
        }
        _ => Err(format!("« {genre} » : un remboursement ou une remise.")),
    }
}

/// L'etat d'un pret, calcule (D47 point 4) : `annule`, `solde` ou
/// `en_cours`.
pub fn etat(statut: &str, montant: i64, retenu: i64) -> &'static str {
    if statut == "annulee" {
        "annule"
    } else if montant - retenu <= 0 {
        "solde"
    } else {
        "en_cours"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pret(reste: i64, mensualite: i64, des: &str, deja: i64) -> PretARetenir {
        PretARetenir { id: "p".into(), date: "2026-06-12".into(), reste, mensualite, retenir_des: des.into(), deja_ce_mois: deja }
    }

    #[test]
    fn les_mois_se_comptent() {
        assert_eq!(mois_de("2026-06-12T10:00:00").unwrap(), "2026-06");
        assert_eq!(mois_plus("2026-11", 2).unwrap(), "2027-01");
        assert_eq!(mois_plus("2026-01", -1).unwrap(), "2025-12");
        assert_eq!(mois_dit("2026-08"), "août 2026");
        assert!(mois_de("juin").is_err());
    }

    #[test]
    fn un_pret_a_une_echeance_plus_petite_que_lui() {
        assert_eq!(verifier(60_000, Some(10_000), false, "Awa"), Ok(10_000));
        assert!(verifier(60_000, None, false, "Awa").unwrap_err().contains("chaque mois"));
        assert!(verifier(60_000, Some(60_000), false, "Awa").unwrap_err().contains("avance"));
        assert!(verifier(60_000, Some(0), false, "Awa").is_err());
        assert!(verifier(0, Some(1), false, "Awa").is_err());
        assert!(verifier(60_000, Some(10_000), true, "Awa").unwrap_err().contains("un seul"));
    }

    #[test]
    fn la_premiere_retenue_tombe_le_mois_suivant_reculable_de_trois_mois() {
        assert_eq!(premier_mois("2026-06-12", None).unwrap(), "2026-07");
        assert_eq!(premier_mois("2026-06-12", Some("2026-09")).unwrap(), "2026-09");
        assert_eq!(premier_mois("2026-06-12", Some("2026-10-01")).unwrap(), "2026-10");
        let e = premier_mois("2026-06-12", Some("2026-11")).unwrap_err();
        assert!(e.contains("juillet 2026") && e.contains("octobre 2026"), "{e}");
        assert!(premier_mois("2026-06-12", Some("2026-06")).is_err(), "pas le mois du prêt");
        assert_eq!(premier_mois("2026-12-20", None).unwrap(), "2027-01");
    }

    #[test]
    fn l_echeance_du_mois() {
        assert_eq!(echeance(&pret(60_000, 10_000, "2026-07", 0), "2026-06", false), 0, "pas encore");
        assert_eq!(echeance(&pret(60_000, 10_000, "2026-07", 0), "2026-07", false), 10_000);
        assert_eq!(echeance(&pret(60_000, 10_000, "2026-07", 10_000), "2026-07", false), 0, "la quinzaine a déjà payé");
        assert_eq!(echeance(&pret(60_000, 10_000, "2026-07", 4_000), "2026-07", false), 6_000);
        assert_eq!(echeance(&pret(5_000, 10_000, "2026-07", 0), "2026-08", false), 5_000, "la dernière");
        assert_eq!(echeance(&pret(45_000, 10_000, "2026-09", 0), "2026-07", true), 45_000, "au départ, tout");
    }

    #[test]
    fn une_fiche_maigre_allonge_le_pret_sans_doubler() {
        let p = vec![pret(60_000, 10_000, "2026-07", 0)];
        let (l, manque) = retenir_prets(4_000, &p, "2026-07", false);
        assert_eq!(l[0].montant, -4_000);
        assert_eq!(manque, 6_000);
        assert!(l[0].libelle.contains("6 000 F faute de net") && l[0].libelle.contains("reste 56 000 F"), "{}", l[0].libelle);
        // Le mois suivant : l'echeance, pas l'echeance plus le manque.
        let p = vec![pret(56_000, 10_000, "2026-07", 0)];
        let (l, manque) = retenir_prets(200_000, &p, "2026-08", false);
        assert_eq!((l[0].montant, manque), (-10_000, 0));
        let (l, manque) = retenir_prets(0, &p, "2026-08", false);
        assert!(l.is_empty());
        assert_eq!(manque, 10_000);
    }

    #[test]
    fn les_libelles_disent_ou_en_est_le_pret() {
        let (l, _) = retenir_prets(100_000, &[pret(10_000, 10_000, "2026-07", 0)], "2026-12", false);
        assert_eq!(l[0].libelle, "Prêt du 12/06/2026 : dernière échéance");
        let (l, _) = retenir_prets(100_000, &[pret(40_000, 10_000, "2026-07", 0)], "2026-09", true);
        assert_eq!(l[0].libelle, "Prêt du 12/06/2026 : soldé au départ");
        assert_eq!(l[0].montant, -40_000);
        let (l, manque) = retenir_prets(15_000, &[pret(40_000, 10_000, "2026-07", 0)], "2026-09", true);
        assert_eq!(manque, 25_000);
        assert!(l[0].libelle.contains("25 000 F restent dus"), "{}", l[0].libelle);
    }

    #[test]
    fn la_fin_prevue_recule_avec_les_echeances_manquees() {
        assert_eq!(fin_prevue(60_000, 10_000, "2026-07", None, "2026-06").as_deref(), Some("2026-12"));
        assert_eq!(fin_prevue(50_000, 10_000, "2026-07", Some("2026-07"), "2026-07").as_deref(), Some("2026-12"));
        // Aout manque (rien retenu) : en octobre, il reste 5 echeances.
        assert_eq!(fin_prevue(50_000, 10_000, "2026-07", Some("2026-07"), "2026-10").as_deref(), Some("2027-02"));
        assert_eq!(fin_prevue(5_000, 10_000, "2026-07", Some("2026-11"), "2026-11").as_deref(), Some("2026-12"));
        assert_eq!(fin_prevue(0, 10_000, "2026-07", None, "2026-07"), None);
    }

    #[test]
    fn rembourser_une_partie_remettre_tout() {
        assert_eq!(verifier_reglement(REMISE, None, 25_000), Ok(25_000));
        assert_eq!(verifier_reglement(REMBOURSEMENT, Some(5_000), 25_000), Ok(5_000));
        assert!(verifier_reglement(REMBOURSEMENT, Some(30_000), 25_000).unwrap_err().contains("25 000 F"));
        assert!(verifier_reglement(REMBOURSEMENT, None, 25_000).is_err());
        assert!(verifier_reglement(REMISE, None, 0).unwrap_err().contains("soldé"));
        assert!(verifier_reglement("don", None, 10).is_err());
    }

    #[test]
    fn solde_se_calcule() {
        assert_eq!(etat("ouverte", 60_000, 60_000), "solde");
        assert_eq!(etat("ouverte", 60_000, 50_000), "en_cours");
        assert_eq!(etat("annulee", 60_000, 0), "annule");
    }
}
