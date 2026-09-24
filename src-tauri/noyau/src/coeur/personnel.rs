//! Une personne, pas un contrat (PLAN-EQUIPE, D30).
//!
//! La plupart des boutiques n'ont ni contrat ecrit ni declaration. Une
//! fiche exige le nom, ce que la personne fait, et comment elle est
//! payee — qui peut etre « rien de fixe ». Les modes s'ajoutent : un
//! vendeur peut avoir un fixe au mois ET une commission.

use serde::{Deserialize, Serialize};

/// Comment la personne est payee. Tout vide : rien de fixe (des
/// primes saisies sur la fiche de paie).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Remuneration {
    /// Un montant au mois, en francs.
    #[serde(default)]
    pub salaire_mensuel: Option<i64>,
    /// Un tarif par jour travaille, en francs.
    #[serde(default)]
    pub tarif_journalier: Option<i64>,
    /// Un pourcentage des ventes signees par son compte (D26).
    #[serde(default)]
    pub commission_pct: Option<f64>,
    /// Paye a la tache : des lignes saisies sur la fiche de paie.
    #[serde(default)]
    pub a_la_tache: bool,
}

impl Remuneration {
    pub fn rien_de_fixe(&self) -> bool {
        self.salaire_mensuel.is_none() && self.tarif_journalier.is_none() && self.commission_pct.is_none() && !self.a_la_tache
    }
}

fn francs(n: i64) -> String {
    let s = n.abs().to_string();
    let mut groupes: Vec<&str> = Vec::new();
    let mut fin = s.len();
    while fin > 3 {
        groupes.push(&s[fin - 3..fin]);
        fin -= 3;
    }
    groupes.push(&s[..fin]);
    groupes.reverse();
    format!("{}{} F", if n < 0 { "-" } else { "" }, groupes.join(" "))
}

/// « 60 000 F par mois + 2 % des ventes », « rien de fixe ». Pure.
pub fn dire(r: &Remuneration) -> String {
    let mut parts = Vec::new();
    if let Some(m) = r.salaire_mensuel {
        parts.push(format!("{} par mois", francs(m)));
    }
    if let Some(j) = r.tarif_journalier {
        parts.push(format!("{} par jour", francs(j)));
    }
    if let Some(c) = r.commission_pct {
        let c = if c.fract() == 0.0 { format!("{c:.0}") } else { format!("{c}").replace('.', ",") };
        parts.push(format!("{c} % des ventes"));
    }
    if r.a_la_tache {
        parts.push("à la tâche".to_string());
    }
    if parts.is_empty() {
        "rien de fixe".to_string()
    } else {
        parts.join(" + ")
    }
}

/// Juge une fiche : le nom, ce que fait la personne, une remuneration
/// qui tient debout. Rend le nom et la fonction nettoyes. Pure.
pub fn valider(nom: &str, fonction: &str, r: &Remuneration) -> Result<(String, String), String> {
    let nom = nom.trim().to_string();
    let fonction = fonction.trim().to_string();
    if nom.is_empty() {
        return Err("Le nom est vide.".to_string());
    }
    if fonction.is_empty() {
        return Err(format!("Que fait {nom} ? (vendeuse, magasinier, livreur…)"));
    }
    if r.salaire_mensuel.is_some_and(|m| m <= 0) {
        return Err("Le salaire au mois doit être plus grand que zéro — ou vide.".to_string());
    }
    if r.tarif_journalier.is_some_and(|j| j <= 0) {
        return Err("Le tarif du jour doit être plus grand que zéro — ou vide.".to_string());
    }
    if r.commission_pct.is_some_and(|c| !c.is_finite() || c <= 0.0 || c > 100.0) {
        return Err("La commission est un pourcentage entre 0 et 100.".to_string());
    }
    Ok((nom, fonction))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn une_fiche_sans_contrat_ni_salaire_fixe_est_permise() {
        let r = Remuneration::default();
        assert!(r.rien_de_fixe());
        assert_eq!(valider(" Awa ", " vendeuse ", &r).unwrap(), ("Awa".into(), "vendeuse".into()));
        assert_eq!(dire(&r), "rien de fixe");
    }

    #[test]
    fn les_modes_s_ajoutent_et_se_disent() {
        let r = Remuneration { salaire_mensuel: Some(60_000), commission_pct: Some(2.5), ..Default::default() };
        assert_eq!(dire(&r), "60 000 F par mois + 2,5 % des ventes");
        let j = Remuneration { tarif_journalier: Some(2_500), a_la_tache: true, ..Default::default() };
        assert_eq!(dire(&j), "2 500 F par jour + à la tâche");
    }

    #[test]
    fn ce_qui_ne_tient_pas_debout_est_refuse() {
        let r = Remuneration::default();
        assert!(valider("", "vendeuse", &r).unwrap_err().contains("nom"));
        assert!(valider("Awa", " ", &r).unwrap_err().contains("Que fait Awa"));
        let z = Remuneration { salaire_mensuel: Some(0), ..Default::default() };
        assert!(valider("Awa", "vendeuse", &z).is_err());
        let c = Remuneration { commission_pct: Some(150.0), ..Default::default() };
        assert!(valider("Awa", "vendeuse", &c).unwrap_err().contains("0 et 100"));
    }
}
