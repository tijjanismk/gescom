//! Les journaux comptables, LUS (v3, E-3 — decision D23).
//!
//! Rien n'est stocke : une ecriture enregistree a cote de la vente
//! serait une seconde verite, et les deux finiraient par se contredire.
//! Les ecritures se fabriquent a la lecture, depuis les ventes, les
//! achats, les paiements et la caisse, avec les comptes de
//! l'affectation (E-2). Ici, la forme d'une ecriture, sa regle
//! (debit = credit) et l'export pour le logiciel du comptable.

use serde::Serialize;

/// Les quatre journaux : (code, libelle).
pub const JOURNAUX: &[(&str, &str)] = &[
    ("VT", "Journal des ventes"),
    ("AC", "Journal des achats"),
    ("RG", "Journal des règlements"),
    ("CA", "Journal de caisse"),
];

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Ligne {
    pub compte: String,
    pub debit: i64,
    pub credit: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Ecriture {
    pub journal: &'static str,
    /// `AAAA-MM-JJ`.
    pub date: String,
    pub piece: String,
    pub libelle: String,
    pub lignes: Vec<Ligne>,
}

/// Un mode de paiement, comme on le dit.
pub fn libelle_mode(mode: &str) -> &str {
    match mode {
        "especes" => "espèces",
        "orange_money" => "Orange Money",
        "moov_money" => "Moov Money",
        "cheque" => "chèque",
        "virement" => "virement",
        autre => autre,
    }
}

pub fn debit(compte: &str, montant: i64) -> Ligne {
    Ligne { compte: compte.to_string(), debit: montant, credit: 0 }
}

pub fn credit(compte: &str, montant: i64) -> Ligne {
    Ligne { compte: compte.to_string(), debit: 0, credit: montant }
}

/// Une ecriture a deux comptes : `montant` au debit du premier et au
/// credit du second. Un montant negatif (une annulation, un cheque
/// rejete) se contre-passe : les cotes s'inversent, le montant reste
/// positif — un journal n'a pas de debit negatif.
pub fn simple(
    journal: &'static str,
    date: &str,
    piece: String,
    libelle: String,
    au_debit: &str,
    au_credit: &str,
    montant: i64,
) -> Option<Ecriture> {
    if montant == 0 {
        return None;
    }
    let (d, c) = if montant > 0 { (au_debit, au_credit) } else { (au_credit, au_debit) };
    Some(Ecriture {
        journal,
        date: date.chars().take(10).collect(),
        piece,
        libelle,
        lignes: vec![debit(d, montant.abs()), credit(c, montant.abs())],
    })
}

/// Une ecriture est-elle equilibree ? Les lignes a zero s'en vont.
pub fn equilibrer(mut e: Ecriture) -> Result<Ecriture, String> {
    e.lignes.retain(|l| l.debit != 0 || l.credit != 0);
    let d: i64 = e.lignes.iter().map(|l| l.debit).sum();
    let c: i64 = e.lignes.iter().map(|l| l.credit).sum();
    if d != c {
        return Err(format!("Écriture {} du {} déséquilibrée : débit {d}, crédit {c}.", e.piece, e.date));
    }
    Ok(e)
}

pub fn totaux(ecritures: &[Ecriture]) -> (i64, i64) {
    ecritures.iter().flat_map(|e| e.lignes.iter()).fold((0, 0), |(d, c), l| (d + l.debit, c + l.credit))
}

fn champ(s: &str) -> String {
    if s.contains([';', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// L'export pour le logiciel du comptable : point-virgule (Excel
/// francais), une ligne par ligne d'ecriture, montants en francs
/// entiers, date `JJ/MM/AAAA`. Pure.
pub fn csv(ecritures: &[Ecriture], libelles: &dyn Fn(&str) -> String) -> String {
    let mut s = String::from("Date;Journal;Compte;Libellé du compte;Libellé;Débit;Crédit;Pièce\r\n");
    for e in ecritures {
        let date = match (e.date.get(0..4), e.date.get(5..7), e.date.get(8..10)) {
            (Some(a), Some(m), Some(j)) => format!("{j}/{m}/{a}"),
            _ => e.date.clone(),
        };
        for l in &e.lignes {
            s.push_str(&format!(
                "{};{};{};{};{};{};{};{}\r\n",
                date,
                e.journal,
                champ(&l.compte),
                champ(&libelles(&l.compte)),
                champ(&e.libelle),
                if l.debit != 0 { l.debit.to_string() } else { String::new() },
                if l.credit != 0 { l.credit.to_string() } else { String::new() },
                champ(&e.piece),
            ));
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn une_annulation_se_contre_passe_sans_montant_negatif() {
        let e = simple("RG", "2026-09-24T10:00:00", "P1".into(), "x".into(), "571", "411", -5000).unwrap();
        assert_eq!(e.date, "2026-09-24");
        assert_eq!(e.lignes, vec![debit("411", 5000), credit("571", 5000)]);
        assert!(simple("RG", "2026-09-24", "P".into(), "x".into(), "571", "411", 0).is_none());
    }

    #[test]
    fn debit_egal_credit_ou_refus() {
        let bonne = Ecriture {
            journal: "VT",
            date: "2026-09-24".into(),
            piece: "V1".into(),
            libelle: "Vente".into(),
            lignes: vec![debit("411", 11800), credit("701", 10000), credit("4431", 1800), credit("7019", 0)],
        };
        let e = equilibrer(bonne.clone()).unwrap();
        assert_eq!(e.lignes.len(), 3, "la ligne à zéro s'en va");
        let mut fausse = bonne;
        fausse.lignes[0].debit = 11700;
        assert!(equilibrer(fausse).unwrap_err().contains("déséquilibrée"));
    }

    #[test]
    fn le_csv_se_lit_dans_excel() {
        let e = simple("VT", "2026-09-24", "FAC;1".into(), "Vente \"Awa\"".into(), "411", "701", 1500).unwrap();
        let c = csv(&[e], &|n| if n == "411" { "Clients".into() } else { "Ventes".into() });
        let l: Vec<&str> = c.split("\r\n").collect();
        assert_eq!(l[0], "Date;Journal;Compte;Libellé du compte;Libellé;Débit;Crédit;Pièce");
        assert_eq!(l[1], "24/09/2026;VT;411;Clients;\"Vente \"\"Awa\"\"\";1500;;\"FAC;1\"");
        assert_eq!(l[2], "24/09/2026;VT;701;Ventes;\"Vente \"\"Awa\"\"\";;1500;\"FAC;1\"");
    }
}
