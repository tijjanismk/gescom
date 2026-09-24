//! La fiche de paie, calculee (PLAN-EQUIPE, G-2 — D31). Pur.
//!
//! Une fiche part de la facon dont la personne est payee (D30) et de ce
//! que la base sait de la periode — jours travailles, ventes signees par
//! son compte, avances en cours — plus ce qu'on y saisit : lignes a la
//! tache, primes, retenues (casse, absence). Chaque ligne dit d'ou elle
//! vient ; une ligne calculee se refait a chaque calcul, une ligne
//! saisie reste.
//!
//! Les avances se retiennent d'office, les plus anciennes d'abord, dans
//! la limite de ce que la personne a gagne : ce qui depasse se reporte
//! sur la fiche suivante (D32). Les montants sont en francs entiers ;
//! un seul arrondi par ligne.

use serde::{Deserialize, Serialize};

use crate::coeur::personnel::Remuneration;
use crate::coeur::plafonds::francs;

/// Les genres de ligne. Les trois premiers se calculent, les autres se
/// saisissent, sauf `avance` (calculee aussi).
pub const BASE: &str = "base";
pub const JOURS: &str = "jours";
pub const COMMISSION: &str = "commission";
pub const TACHE: &str = "tache";
pub const PRIME: &str = "prime";
pub const RETENUE: &str = "retenue";
pub const AVANCE: &str = "avance";

/// Ce qu'on peut saisir sur une fiche.
pub const SAISISSABLES: &[&str] = &[TACHE, PRIME, RETENUE];

/// Une ligne de fiche. `montant` est signe : un gain est positif, une
/// retenue negative.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ligne {
    pub genre: String,
    pub libelle: String,
    pub quantite: Option<f64>,
    pub prix: Option<i64>,
    pub montant: i64,
    /// L'avance retenue, pour une ligne `avance`.
    pub source: Option<String>,
    pub saisie: bool,
}

/// Une avance a retenir : son identifiant, sa date, ce qui peut encore
/// en etre retenu.
#[derive(Debug, Clone)]
pub struct AvanceARetenir {
    pub id: String,
    pub date: String,
    pub reste: i64,
}

/// Ce que la base sait de la periode.
#[derive(Debug, Clone, Default)]
pub struct Periode {
    /// Jours travailles (presents = 1, demi = 0,5).
    pub jours_travailles: f64,
    /// Jours marques, quelle que soit la marque (le prorata s'y mesure).
    pub jours_marques: i64,
    /// Les ventes signees par son compte ; `None` : pas de compte lie.
    pub ventes: Option<i64>,
}

fn jours_dits(j: f64) -> String {
    let s = if j.fract() == 0.0 { format!("{j:.0}") } else { format!("{j}").replace('.', ",") };
    if j <= 1.0 { format!("{s} jour") } else { format!("{s} jours") }
}

fn pct_dit(p: f64) -> String {
    if p.fract() == 0.0 { format!("{p:.0}") } else { format!("{p}").replace('.', ",") }
}

fn calculee(genre: &str, libelle: String, quantite: Option<f64>, prix: Option<i64>, montant: i64) -> Ligne {
    Ligne { genre: genre.to_string(), libelle, quantite, prix, montant, source: None, saisie: false }
}

/// Les lignes que la remuneration donne pour la periode (sans les
/// saisies ni les avances). `prorata` : le mois au prorata des jours
/// travailles sur les jours marques.
pub fn gains_calcules(r: &Remuneration, p: &Periode, prorata: bool) -> Vec<Ligne> {
    let mut l = Vec::new();
    if let Some(m) = r.salaire_mensuel {
        if prorata && p.jours_marques > 0 {
            let montant = ((m as f64) * p.jours_travailles / p.jours_marques as f64).round() as i64;
            l.push(calculee(
                BASE,
                format!(
                    "Salaire du mois, {} {} sur {} marqués",
                    jours_dits(p.jours_travailles),
                    if p.jours_travailles <= 1.0 { "travaillé" } else { "travaillés" },
                    p.jours_marques
                ),
                Some(p.jours_travailles),
                Some(m),
                montant.min(m),
            ));
        } else if prorata {
            l.push(calculee(BASE, "Salaire du mois (aucun jour marqué : le mois entier)".to_string(), None, Some(m), m));
        } else {
            l.push(calculee(BASE, "Salaire du mois".to_string(), None, Some(m), m));
        }
    }
    if let Some(t) = r.tarif_journalier {
        let montant = ((t as f64) * p.jours_travailles).round() as i64;
        l.push(calculee(
            JOURS,
            format!("{} × {}", jours_dits(p.jours_travailles), francs(t)),
            Some(p.jours_travailles),
            Some(t),
            montant,
        ));
    }
    if let Some(c) = r.commission_pct {
        match p.ventes {
            Some(v) => {
                let montant = ((v as f64) * c / 100.0).round() as i64;
                l.push(calculee(COMMISSION, format!("{} % de {} de ventes signées", pct_dit(c), francs(v)), None, Some(v), montant));
            }
            None => l.push(calculee(COMMISSION, format!("{} % des ventes : pas de compte lié, aucune vente à compter", pct_dit(c)), None, None, 0)),
        }
    }
    l
}

/// Juge une ligne saisie et la rend prete. Tache : quantite × prix ;
/// prime : un montant ; retenue : un montant, rendu negatif.
pub fn ligne_saisie(genre: &str, libelle: &str, quantite: Option<f64>, prix: Option<i64>, montant: Option<i64>) -> Result<Ligne, String> {
    let libelle = libelle.trim().to_string();
    match genre {
        TACHE => {
            if libelle.is_empty() {
                return Err("Quelle tâche ? (livraisons, déchargement…)".to_string());
            }
            let q = quantite.ok_or("Combien de fois ?")?;
            if !q.is_finite() || q <= 0.0 {
                return Err("La quantité doit être plus grande que zéro.".to_string());
            }
            let p = prix.ok_or("Combien pour chacune ?")?;
            if p <= 0 {
                return Err("Le prix d'une tâche doit être plus grand que zéro.".to_string());
            }
            Ok(Ligne { genre: TACHE.into(), libelle, quantite: Some(q), prix: Some(p), montant: ((p as f64) * q).round() as i64, source: None, saisie: true })
        }
        PRIME | RETENUE => {
            let m = montant.ok_or("Quel montant ?")?;
            if m <= 0 {
                return Err("Le montant doit être plus grand que zéro.".to_string());
            }
            let (libelle, montant) = if genre == PRIME {
                (if libelle.is_empty() { "Prime".to_string() } else { libelle }, m)
            } else {
                if libelle.is_empty() {
                    return Err("Une retenue dit pourquoi (casse, absence…).".to_string());
                }
                (libelle, -m)
            };
            Ok(Ligne { genre: genre.into(), libelle, quantite: None, prix: None, montant, source: None, saisie: true })
        }
        _ => Err(format!("« {genre} » ne se saisit pas : prime, tâche ou retenue.")),
    }
}

fn date_dite(iso: &str) -> String {
    let j = &iso[..iso.len().min(10)];
    let p: Vec<&str> = j.split('-').collect();
    if p.len() == 3 { format!("{}/{}/{}", p[2], p[1], p[0]) } else { j.to_string() }
}

/// Retient les avances sur ce qui reste, les plus anciennes d'abord.
/// Rend les lignes et ce qui se reporte.
pub fn retenir_avances(disponible: i64, avances: &[AvanceARetenir]) -> (Vec<Ligne>, i64) {
    let mut avances: Vec<&AvanceARetenir> = avances.iter().filter(|a| a.reste > 0).collect();
    avances.sort_by(|a, b| a.date.cmp(&b.date).then(a.id.cmp(&b.id)));
    let mut reste_dispo = disponible.max(0);
    let mut lignes = Vec::new();
    let mut reporte = 0;
    for a in avances {
        let pris = a.reste.min(reste_dispo);
        reste_dispo -= pris;
        reporte += a.reste - pris;
        if pris == 0 {
            continue;
        }
        let libelle = if pris < a.reste {
            format!("Avance du {} ({} ; {} reportés)", date_dite(&a.date), francs(a.reste), francs(a.reste - pris))
        } else {
            format!("Avance du {}", date_dite(&a.date))
        };
        lignes.push(Ligne { genre: AVANCE.into(), libelle, quantite: None, prix: None, montant: -pris, source: Some(a.id.clone()), saisie: false });
    }
    (lignes, reporte)
}

/// La fiche entiere : gains calcules, saisies (gains puis retenues),
/// avances retenues. Rend les lignes dans l'ordre du bulletin et ce qui
/// se reporte.
pub fn calculer(r: &Remuneration, p: &Periode, prorata: bool, saisies: &[Ligne], avances: &[AvanceARetenir]) -> (Vec<Ligne>, i64) {
    let mut lignes = gains_calcules(r, p, prorata);
    lignes.extend(saisies.iter().filter(|l| l.montant >= 0).cloned());
    lignes.extend(saisies.iter().filter(|l| l.montant < 0).cloned());
    let avant_avances: i64 = lignes.iter().map(|l| l.montant).sum();
    let (retenues, reporte) = retenir_avances(avant_avances, avances);
    lignes.extend(retenues);
    (lignes, reporte)
}

/// Brut (les gains), retenues (en positif, avances comprises), net.
pub fn totaux(lignes: &[Ligne]) -> (i64, i64, i64) {
    let brut: i64 = lignes.iter().filter(|l| l.montant > 0).map(|l| l.montant).sum();
    let retenues: i64 = lignes.iter().filter(|l| l.montant < 0).map(|l| -l.montant).sum();
    (brut, retenues, brut - retenues)
}

/// Peut-on valider ? Un net negatif veut dire que les retenues saisies
/// depassent ce que la personne a gagne.
pub fn validable(lignes: &[Ligne]) -> Result<(), String> {
    let (brut, retenues, net) = totaux(lignes);
    if net < 0 {
        return Err(format!(
            "Les retenues ({}) dépassent ce que la personne a gagné ({}) : corriger avant de valider.",
            francs(retenues),
            francs(brut)
        ));
    }
    Ok(())
}

/// Deux periodes se chevauchent-elles ? Dates ISO `AAAA-MM-JJ`.
pub fn se_chevauchent(du1: &str, au1: &str, du2: &str, au2: &str) -> bool {
    du1 <= au2 && du2 <= au1
}

/// Juge une periode : deux dates ISO, dans l'ordre, pas plus d'un an.
pub fn verifier_periode(du: &str, au: &str) -> Result<(), String> {
    let d = chrono::NaiveDate::parse_from_str(du, "%Y-%m-%d").map_err(|_| format!("Date de début illisible : {du}"))?;
    let a = chrono::NaiveDate::parse_from_str(au, "%Y-%m-%d").map_err(|_| format!("Date de fin illisible : {au}"))?;
    if a < d {
        return Err("La période finit avant de commencer.".to_string());
    }
    if (a - d).num_days() > 366 {
        return Err("Une fiche de paie couvre au plus un an.".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(m: Option<i64>, j: Option<i64>, c: Option<f64>) -> Remuneration {
        Remuneration { salaire_mensuel: m, tarif_journalier: j, commission_pct: c, a_la_tache: false }
    }

    #[test]
    fn chaque_mode_donne_sa_ligne() {
        let p = Periode { jours_travailles: 22.0, jours_marques: 24, ventes: Some(250_000) };
        let l = gains_calcules(&r(Some(60_000), None, None), &p, false);
        assert_eq!((l[0].montant, l[0].libelle.as_str()), (60_000, "Salaire du mois"));
        let l = gains_calcules(&r(Some(60_000), None, None), &p, true);
        assert_eq!(l[0].montant, 55_000, "60 000 × 22 / 24");
        assert_eq!(l[0].libelle, "Salaire du mois, 22 jours travaillés sur 24 marqués");
        let l = gains_calcules(&r(None, Some(2_500), None), &Periode { jours_travailles: 21.5, ..p.clone() }, false);
        assert_eq!((l[0].montant, l[0].libelle.as_str()), (53_750, "21,5 jours × 2 500 F"));
        let l = gains_calcules(&r(Some(25_000), None, Some(2.5)), &p, false);
        assert_eq!(l.len(), 2, "les modes s'ajoutent");
        assert_eq!((l[1].montant, l[1].libelle.as_str()), (6_250, "2,5 % de 250 000 F de ventes signées"));
        let l = gains_calcules(&r(None, None, Some(3.0)), &Periode { ventes: None, ..p }, false);
        assert_eq!(l[0].montant, 0);
        assert!(l[0].libelle.contains("pas de compte lié"));
        assert!(gains_calcules(&Remuneration::default(), &Periode::default(), false).is_empty(), "rien de fixe");
    }

    #[test]
    fn le_prorata_sans_jour_marque_donne_le_mois() {
        let l = gains_calcules(&r(Some(60_000), None, None), &Periode::default(), true);
        assert_eq!(l[0].montant, 60_000);
        assert!(l[0].libelle.contains("aucun jour marqué"));
    }

    #[test]
    fn les_saisies_se_jugent() {
        assert_eq!(ligne_saisie(TACHE, "Livraisons", Some(3.0), Some(1_000), None).unwrap().montant, 3_000);
        assert!(ligne_saisie(TACHE, "", Some(3.0), Some(1_000), None).is_err());
        assert_eq!(ligne_saisie(PRIME, "", None, None, Some(5_000)).unwrap().libelle, "Prime");
        let casse = ligne_saisie(RETENUE, "Casse d'un carton", None, None, Some(2_000)).unwrap();
        assert_eq!(casse.montant, -2_000);
        assert!(ligne_saisie(RETENUE, "", None, None, Some(2_000)).unwrap_err().contains("pourquoi"));
        assert!(ligne_saisie(PRIME, "x", None, None, Some(0)).is_err());
        assert!(ligne_saisie(AVANCE, "x", None, None, Some(1)).is_err(), "une avance ne se saisit pas ici");
    }

    #[test]
    fn les_avances_se_retiennent_les_plus_anciennes_d_abord_et_le_reste_se_reporte() {
        let av = vec![
            AvanceARetenir { id: "b".into(), date: "2026-09-20".into(), reste: 30_000 },
            AvanceARetenir { id: "a".into(), date: "2026-09-05".into(), reste: 10_000 },
        ];
        let (l, reporte) = retenir_avances(25_000, &av);
        assert_eq!(l.len(), 2);
        assert_eq!((l[0].source.as_deref(), l[0].montant), (Some("a"), -10_000));
        assert_eq!((l[1].source.as_deref(), l[1].montant), (Some("b"), -15_000));
        assert_eq!(l[1].libelle, "Avance du 20/09/2026 (30 000 F ; 15 000 F reportés)");
        assert_eq!(reporte, 15_000);
        let (l, reporte) = retenir_avances(0, &av);
        assert!(l.is_empty());
        assert_eq!(reporte, 40_000, "rien gagné : tout se reporte");
    }

    #[test]
    fn la_fiche_entiere() {
        let p = Periode { jours_travailles: 0.0, jours_marques: 0, ventes: None };
        let saisies = vec![
            ligne_saisie(RETENUE, "Casse", None, None, Some(2_000)).unwrap(),
            ligne_saisie(PRIME, "Fin d'année", None, None, Some(5_000)).unwrap(),
        ];
        let av = vec![AvanceARetenir { id: "a".into(), date: "2026-09-05".into(), reste: 10_000 }];
        let (l, reporte) = calculer(&r(Some(60_000), None, None), &p, false, &saisies, &av);
        let genres: Vec<&str> = l.iter().map(|x| x.genre.as_str()).collect();
        assert_eq!(genres, vec![BASE, PRIME, RETENUE, AVANCE], "gains, puis retenues, puis avances");
        assert_eq!(totaux(&l), (65_000, 12_000, 53_000));
        assert_eq!(reporte, 0);
        assert!(validable(&l).is_ok());
        let trop = vec![ligne_saisie(RETENUE, "Casse", None, None, Some(70_000)).unwrap()];
        let (l, _) = calculer(&r(Some(60_000), None, None), &p, false, &trop, &av);
        assert_eq!(l.iter().filter(|x| x.genre == AVANCE).count(), 0, "rien à retenir");
        assert!(validable(&l).unwrap_err().contains("dépassent"));
    }

    #[test]
    fn les_periodes() {
        assert!(se_chevauchent("2026-09-01", "2026-09-30", "2026-09-15", "2026-10-15"));
        assert!(!se_chevauchent("2026-09-01", "2026-09-15", "2026-09-16", "2026-09-30"));
        assert!(verifier_periode("2026-09-01", "2026-09-30").is_ok());
        assert!(verifier_periode("2026-09-30", "2026-09-01").is_err());
        assert!(verifier_periode("2026-09", "2026-09-30").is_err());
    }
}
