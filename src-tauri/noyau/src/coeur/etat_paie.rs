//! L'etat de paie du mois (PLAN-EQUIPE, EQ-2 — D48). Pur.
//!
//! Une ligne par personne : ses fiches validees du mois, ventilees par
//! genre de ligne — jamais recalculees : l'etat relit ce que la fiche a
//! fige. Les totaux s'additionnent ligne a ligne, sommes verifiees.
//!
//! Le mois d'une fiche est celui de la FIN de sa periode, comme le cout
//! des salaires de Rapports (`paie::salaires_par_mois_sur`) : une
//! quinzaine a cheval ne compte qu'une fois, et le cout du mois (brut +
//! charges patronales) est le meme chiffre des deux cotes.

use serde::Serialize;

use crate::coeur::paie::{AVANCE, CHARGE, COTISATION};
use crate::coeur::saisie::somme;

/// Le genre que l'etat donne a une ligne `avance` dont la source est un
/// pret (D47) : sur la fiche, la ligne reste du genre `avance` ; l'etat
/// la range dans sa colonne. La base le pose (`etat_paie::lire_sur`).
pub const PRET: &str = "pret";

/// Ce qu'une fiche (ou une personne, ou le mois) compte, en francs. Les
/// retenues sont POSITIVES ici : ce qui a ete retire du brut.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Montants {
    pub brut: i64,
    /// Les avances retenues (D32).
    pub avances: i64,
    /// Les echeances de pret retenues (D47).
    pub prets: i64,
    /// Les autres retenues : saisies (casse, absence, ITS), et tout genre
    /// que l'etat ne connait pas encore — le net reste juste.
    pub retenues: i64,
    /// Les cotisations salariales (D33).
    pub cotisations: i64,
    /// Les charges patronales : dites, hors du net.
    pub charges: i64,
    pub net: i64,
    pub verse: i64,
    /// Ce qui reste a verser (les restes positifs).
    pub reste: i64,
    /// Verse au-dela du net et pas repris en avance (les restes negatifs).
    pub trop_verse: i64,
}

impl Montants {
    /// Le cout du mois pour la boutique : brut + charges patronales.
    pub fn cout(&self) -> Result<i64, String> {
        somme([self.brut, self.charges])
    }

    /// Ajoute `autre` a ces montants, sommes verifiees.
    pub fn ajouter(&mut self, autre: &Montants) -> Result<(), String> {
        self.brut = somme([self.brut, autre.brut])?;
        self.avances = somme([self.avances, autre.avances])?;
        self.prets = somme([self.prets, autre.prets])?;
        self.retenues = somme([self.retenues, autre.retenues])?;
        self.cotisations = somme([self.cotisations, autre.cotisations])?;
        self.charges = somme([self.charges, autre.charges])?;
        self.net = somme([self.net, autre.net])?;
        self.verse = somme([self.verse, autre.verse])?;
        self.reste = somme([self.reste, autre.reste])?;
        self.trop_verse = somme([self.trop_verse, autre.trop_verse])?;
        Ok(())
    }
}

/// Ventile les lignes d'une fiche `(genre, montant signe)`, et ce qui a
/// ete verse : `reste` est le reste de la fiche (`paie::reste_a_verser`).
/// `net` stocke sur la fiche doit retomber sur ses lignes, sinon refus :
/// un etat qui ne colle pas a ses fiches ne vaut rien.
pub fn ventiler(numero: &str, net: i64, lignes: &[(String, i64)], verse: i64, reste: i64) -> Result<Montants, String> {
    let mut m = Montants { verse, ..Default::default() };
    let de = |genre: &str| somme(lignes.iter().filter(|(g, _)| g == genre).map(|(_, x)| *x));
    m.avances = de(AVANCE)?.saturating_neg();
    m.prets = de(PRET)?.saturating_neg();
    m.cotisations = de(COTISATION)?.saturating_neg();
    m.charges = de(CHARGE)?;
    let paie = || lignes.iter().filter(|(g, _)| g != CHARGE && g != AVANCE && g != PRET && g != COTISATION);
    m.brut = somme(paie().filter(|(_, x)| *x > 0).map(|(_, x)| *x))?;
    m.retenues = somme(paie().filter(|(_, x)| *x < 0).map(|(_, x)| *x))?.saturating_neg();
    m.net = m.brut - m.avances - m.prets - m.retenues - m.cotisations;
    if m.net != net {
        return Err(format!(
            "La fiche {numero} ne se relit pas : ses lignes donnent un net de {}, la fiche dit {net}. Ouvrir la fiche avant de se fier à l'état.",
            m.net
        ));
    }
    if reste >= 0 {
        m.reste = reste;
    } else {
        m.trop_verse = reste.saturating_neg();
    }
    Ok(m)
}

/// Le premier et le dernier jour d'un mois `AAAA-MM`, ou un refus.
pub fn bornes_du_mois(mois: &str) -> Result<(String, String), String> {
    let refus = || format!("Mois illisible : « {mois} » (attendu AAAA-MM).");
    let (a, m) = mois.split_once('-').ok_or_else(refus)?;
    let (a, m): (i32, u32) = (a.parse().map_err(|_| refus())?, m.parse().map_err(|_| refus())?);
    if a.to_string().len() != 4 || mois.len() != 7 {
        return Err(refus());
    }
    let premier = chrono::NaiveDate::from_ymd_opt(a, m, 1).ok_or_else(refus)?;
    let suivant = premier.checked_add_months(chrono::Months::new(1)).ok_or_else(refus)?;
    let dernier = suivant.pred_opt().ok_or_else(refus)?;
    Ok((premier.format("%Y-%m-%d").to_string(), dernier.format("%Y-%m-%d").to_string()))
}

/// Une ligne de l'etat : une personne et ses fiches du mois.
#[derive(Debug, Clone, Serialize)]
pub struct LigneEtat {
    pub employe_id: String,
    pub nom: String,
    pub fonction: String,
    pub numeros: Vec<String>,
    #[serde(flatten)]
    pub montants: Montants,
}

fn champ(s: &str) -> String {
    if s.contains([';', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// L'export CSV (D48) : le format des journaux (point-virgule pour Excel
/// francais, francs entiers), une ligne par personne, une ligne TOTAL.
/// Le BOM est ajoute par l'ecran, comme pour les journaux.
pub fn csv(lignes: &[LigneEtat], total: &Montants) -> String {
    let mut s = String::from(
        "Nom;Fonction;Fiches;Brut;Avances retenues;Prêts retenus;Autres retenues;Cotisations salariales;Net;Déjà versé;Reste dû;Versé en trop;Charges patronales\r\n",
    );
    let chiffres = |m: &Montants| {
        format!(
            "{};{};{};{};{};{};{};{};{};{}",
            m.brut, m.avances, m.prets, m.retenues, m.cotisations, m.net, m.verse, m.reste, m.trop_verse, m.charges
        )
    };
    for l in lignes {
        s.push_str(&format!(
            "{};{};{};{}\r\n",
            champ(&l.nom),
            champ(&l.fonction),
            champ(&l.numeros.join(" ")),
            chiffres(&l.montants)
        ));
    }
    s.push_str(&format!("TOTAL;;;{}\r\n", chiffres(total)));
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn l(g: &str, m: i64) -> (String, i64) {
        (g.to_string(), m)
    }

    #[test]
    fn chaque_genre_dans_sa_colonne() {
        let lignes = [
            l("base", 60_000),
            l("prime", 5_000),
            l("retenue", -2_000),
            l("avance", -10_000),
            l("cotisation", -2_340),
            l("charge_patronale", 10_400),
            l("pret", -5_000),
            l("genre_inconnu", -1_000),
        ];
        let m = ventiler("PAIE-1", 44_660, &lignes, 20_000, 24_660).unwrap();
        assert_eq!(
            (m.brut, m.avances, m.prets, m.retenues, m.cotisations, m.charges),
            (65_000, 10_000, 5_000, 3_000, 2_340, 10_400)
        );
        assert_eq!(m.net, 44_660, "un genre inconnu tombe dans les autres retenues : le net reste juste");
        assert_eq!((m.verse, m.reste, m.trop_verse), (20_000, 24_660, 0));
        assert_eq!(m.cout().unwrap(), 75_400, "brut + charges");
    }

    #[test]
    fn un_net_qui_ne_colle_pas_est_un_refus() {
        let e = ventiler("PAIE-7", 50_000, &[l("base", 60_000)], 0, 50_000).unwrap_err();
        assert!(e.contains("PAIE-7") && e.contains("60000"), "{e}");
    }

    #[test]
    fn un_verse_en_trop_se_dit_a_part() {
        let m = ventiler("P", 40_000, &[l("base", 40_000)], 45_000, -5_000).unwrap();
        assert_eq!((m.reste, m.trop_verse), (0, 5_000));
        let mut t = Montants::default();
        t.ajouter(&m).unwrap();
        t.ajouter(&ventiler("Q", 10_000, &[l("base", 10_000)], 0, 10_000).unwrap()).unwrap();
        assert_eq!((t.net, t.reste, t.trop_verse), (50_000, 10_000, 5_000), "le trop-versé n'est pas soustrait en silence");
    }

    #[test]
    fn les_bornes_du_mois() {
        assert_eq!(bornes_du_mois("2026-02").unwrap(), ("2026-02-01".into(), "2026-02-28".into()));
        assert_eq!(bornes_du_mois("2028-02").unwrap().1, "2028-02-29");
        assert_eq!(bornes_du_mois("2026-12").unwrap().1, "2026-12-31");
        for faux in ["2026-13", "2026-1", "26-10", "octobre", "2026-10-01", ""] {
            assert!(bornes_du_mois(faux).is_err(), "{faux}");
        }
    }

    #[test]
    fn le_csv_se_lit_dans_excel() {
        let m = ventiler("P", 50_000, &[l("base", 60_000), l("avance", -10_000)], 0, 50_000).unwrap();
        let lignes = vec![LigneEtat {
            employe_id: "e".into(),
            nom: "Diarra; Awa".into(),
            fonction: "vendeuse".into(),
            numeros: vec!["PAIE-2026-00001".into()],
            montants: m.clone(),
        }];
        let s = csv(&lignes, &m);
        let l: Vec<&str> = s.split("\r\n").collect();
        assert!(l[0].starts_with("Nom;Fonction;Fiches;Brut"));
        assert_eq!(l[1], "\"Diarra; Awa\";vendeuse;PAIE-2026-00001;60000;10000;0;0;0;50000;0;50000;0;0");
        assert_eq!(l[2], "TOTAL;;;60000;10000;0;0;0;50000;0;50000;0;0");
    }
}
