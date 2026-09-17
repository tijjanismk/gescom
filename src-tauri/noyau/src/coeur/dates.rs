//! La date d'une affaire, saisie apres coup.
//!
//! Une boutique note sur papier et saisit le soir — parfois le samedi
//! pour la semaine, une coupure de courant suffit. Imposer la date de
//! saisie fait tomber la vente dans le mauvais mois, et le rapport ne
//! vaut plus rien. On accepte donc une date PASSEE. Mais pas n'importe
//! laquelle :
//!
//! - **jamais dans le futur** : une vente de demain n'existe pas ;
//! - **pas plus loin que `RECUL_MAX_JOURS`** : ce n'est pas contre la
//!   fraude, c'est contre la faute de frappe — `2025` au lieu de `2026`
//!   enverrait la vente dans un exercice clos, et on ne la retrouverait
//!   jamais.
//!
//! Ce que ce module NE decide pas : QUI a le droit d'antidater. C'est
//! une permission (`pieces:antidater`), verifiee par le serveur, parce
//! que la raison n'est pas comptable — antidater une vente en especes
//! est precisement la facon de masquer un trou dans le tiroir.
//!
//! Ce que ce module NE touche pas non plus : le mouvement de caisse.
//! L'argent est entre dans le tiroir quand il y est entre, et la caisse
//! se lit par session, jamais par date. Deux dates, deux faits.

use chrono::NaiveDate;

/// Le recul accepte, en jours. Un mois : assez pour rattraper une
/// semaine de saisie en retard, trop court pour changer d'annee par
/// erreur.
pub const RECUL_MAX_JOURS: i64 = 31;

/// Le jour porte par une date ISO (`AAAA-MM-JJ...`), ou rien.
pub fn jour(iso: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(iso.get(..10)?, "%Y-%m-%d").ok()
}

/// Une date saisie est-elle ANTERIEURE a aujourd'hui ?
///
/// C'est le cas qui exige la permission. Saisir la date du jour n'est
/// pas antidater — c'est ce que fait deja chaque vente.
pub fn est_antidatee(iso: &str, aujourd_hui: NaiveDate) -> bool {
    jour(iso).map(|d| d < aujourd_hui).unwrap_or(false)
}

/// Verifie une date saisie pour une vente, une piece ou un reglement.
///
/// `aujourd_hui` est passe et non lu sur l'horloge : la regle se teste
/// sans attendre minuit. Rend le jour lu, pour que l'appelant n'ait pas
/// a le relire.
pub fn verifier_date_saisie(iso: &str, aujourd_hui: NaiveDate) -> Result<NaiveDate, String> {
    let d = jour(iso).ok_or_else(|| format!("Date illisible : « {iso} »."))?;
    if d > aujourd_hui {
        return Err(format!(
            "La date {} est dans le futur — une affaire ne se date pas d'avance.",
            d.format("%d/%m/%Y")
        ));
    }
    let recul = (aujourd_hui - d).num_days();
    if recul > RECUL_MAX_JOURS {
        return Err(format!(
            "La date {} remonte a {recul} jours ; on n'antidate pas au-dela de \
             {RECUL_MAX_JOURS} jours. Verifier l'annee.",
            d.format("%d/%m/%Y")
        ));
    }
    Ok(d)
}

/// Meme verification, contre l'horloge de la machine.
pub fn verifier_date_saisie_maintenant(iso: &str) -> Result<NaiveDate, String> {
    verifier_date_saisie(iso, chrono::Local::now().date_naive())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn le(annee: i32, mois: u32, jour: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(annee, mois, jour).unwrap()
    }

    #[test]
    fn la_date_du_jour_passe_et_n_est_pas_antidatee() {
        let auj = le(2026, 9, 17);
        assert_eq!(verifier_date_saisie("2026-09-17T15:00:00", auj).unwrap(), auj);
        assert!(!est_antidatee("2026-09-17T15:00:00", auj));
    }

    #[test]
    fn une_date_passee_dans_la_fenetre_passe_et_est_antidatee() {
        let auj = le(2026, 9, 17);
        assert!(verifier_date_saisie("2026-09-03", auj).is_ok());
        assert!(est_antidatee("2026-09-03", auj));
    }

    #[test]
    fn le_futur_est_refuse() {
        let auj = le(2026, 9, 17);
        let e = verifier_date_saisie("2026-09-18", auj).unwrap_err();
        assert!(e.contains("futur"), "{e}");
    }

    #[test]
    fn la_limite_de_recul_tient_a_la_borne() {
        let auj = le(2026, 9, 17);
        // 31 jours : encore accepte.
        assert!(verifier_date_saisie("2026-08-17", auj).is_ok());
        // 32 jours : refuse, et le message dit de verifier l'annee.
        let e = verifier_date_saisie("2026-08-16", auj).unwrap_err();
        assert!(e.contains("annee"), "{e}");
    }

    #[test]
    fn une_faute_d_annee_est_attrapee() {
        let auj = le(2026, 9, 17);
        assert!(verifier_date_saisie("2025-09-17", auj).is_err());
    }

    #[test]
    fn une_date_illisible_est_refusee_et_n_est_pas_antidatee() {
        let auj = le(2026, 9, 17);
        assert!(verifier_date_saisie("hier", auj).is_err());
        assert!(!est_antidatee("hier", auj));
    }
}
