//! Une horloge qui recule (lot C.5, 27/10/2026).
//!
//! La pile du BIOS meurt, une coupure remet la machine a zero : le PC
//! qui garde la base repart a une date passee. Les ventes du jour
//! s'ecrivent alors a hier (la caisse du jour est faussee), ou tombent
//! hors de l'exercice. Rien ne le voyait.
//!
//! La garde : le serveur retient la date-heure de sa derniere ecriture
//! reussie (globale a la base, `config_app`). Si l'heure de la machine
//! est en arriere de PLUS de `TOLERANCE_RECUL_SECONDES` sur cette
//! reference, toute ecriture est refusee ; les lectures continuent. Le
//! remede est de corriger l'heure de Windows — pas de passe-droit.
//!
//! Le seul geste prevu (le patron, `parametres:modifier`) : l'horloge
//! avait ete AVANCEE par erreur (2030), puis corrigee. La reference est
//! alors dans le futur et tout resterait bloque jusqu'a cette date. Le
//! patron confirme que l'heure actuelle de Windows est juste ; la
//! reference revient a maintenant, et le journal garde l'ancienne et la
//! nouvelle valeur. Ce geste n'est permis QUE quand l'ecriture est
//! effectivement bloquee : hors de ce cas il n'a rien a corriger.
//!
//! Ici, la regle seule : comparer, borner, dire. Pure — les heures sont
//! passees, jamais lues sur l'horloge. La lecture et l'ecriture de la
//! reference sont dans `noyau::horloge`, le controle dans `api::rpc`.

use chrono::NaiveDateTime;

/// Le recul tolere, en secondes. Cinq minutes : une resynchronisation
/// de Windows, deux postes legerement decales, une seconde intercalaire
/// ne bloquent pas la boutique ; une machine repartie a hier, si.
pub const TOLERANCE_RECUL_SECONDES: i64 = 5 * 60;

/// Les commandes qui ecrivent mais que la garde laisse passer : le seul
/// geste qui la leve. Sans lui, une reference dans le futur bloquerait
/// tout, y compris sa propre correction.
pub const EXEMPTEES: &[&str] = &["confirmer_horloge"];

/// Le prefixe du refus, comme `CAISSE_FERMEE` : l'ecran peut le
/// reconnaitre sans lire la phrase.
pub const CODE_REFUS: &str = "HORLOGE_RECULE";

/// Une date-heure ISO telle que `utils::maintenant_iso` l'ecrit
/// (`AAAA-MM-JJTHH:MM:SS.mmm`), avec ou sans les millisecondes, avec un
/// `T` ou une espace. Autre chose : rien.
pub fn lire(iso: &str) -> Option<NaiveDateTime> {
    let s = iso.trim().replacen(' ', "T", 1);
    NaiveDateTime::parse_from_str(&s, "%Y-%m-%dT%H:%M:%S%.f").ok()
}

/// « le 27/10/2026 à 14:05 » — pour un message qu'un commercant lit.
/// Illisible : rendue telle quelle.
pub fn en_clair(iso: &str) -> String {
    match lire(iso) {
        Some(d) => d.format("le %d/%m/%Y à %H:%M").to_string(),
        None => iso.to_string(),
    }
}

/// De combien l'heure `maintenant` est en arriere sur `reference`, en
/// secondes (negatif : en avance). `None` si l'une est illisible.
pub fn recul_secondes(reference: &str, maintenant: &str) -> Option<i64> {
    Some((lire(reference)? - lire(maintenant)?).num_seconds())
}

/// La commande echappe-t-elle a la garde ?
pub fn exemptee(commande: &str) -> bool {
    EXEMPTEES.contains(&commande)
}

/// Le refus, mot pour mot : ce qui s'est passe, ce qu'il faut faire, et
/// le seul recours quand l'heure de Windows est deja juste.
pub fn message_refus(reference: &str, maintenant: &str) -> String {
    format!(
        "{CODE_REFUS} — L'horloge de l'ordinateur qui garde la base recule : \
         dernière écriture {}, il est maintenant {}. Rien ne s'enregistre tant \
         que l'heure est en arrière : corriger la date et l'heure de Windows \
         sur cet ordinateur. Si l'heure de Windows est juste (elle avait été \
         avancée par erreur), le patron le confirme dans Paramètres → Horloge.",
        en_clair(reference),
        en_clair(maintenant),
    )
}

/// Une ecriture peut-elle passer a `maintenant`, la derniere ayant eu
/// lieu a `reference` ?
///
/// - pas de reference (base neuve, ou d'avant cette garde) : oui ;
/// - reference illisible : oui — elle sera remplacee par la prochaine
///   ecriture (`nouvelle_reference`). Elle ne s'ecrit que par Gescom ;
///   qui la falsifie a la main a deja la base entre les mains ;
/// - en arriere de plus de la tolerance : non, avec le message ;
/// - `maintenant` illisible : non. C'est l'heure de la machine, mise en
///   forme par Gescom : si elle ne se lit pas, on ne sait pas dater.
pub fn juger(reference: Option<&str>, maintenant: &str) -> Result<(), String> {
    if lire(maintenant).is_none() {
        return Err(format!("{CODE_REFUS} — Heure de l'ordinateur illisible : « {maintenant} »."));
    }
    let Some(reference) = reference else { return Ok(()) };
    match recul_secondes(reference, maintenant) {
        Some(recul) if recul > TOLERANCE_RECUL_SECONDES => Err(message_refus(reference, maintenant)),
        _ => Ok(()),
    }
}

/// La reference apres une ecriture reussie a `maintenant` : la plus
/// recente des deux. `None` quand elle ne change pas (rien a ecrire).
///
/// La plus recente, et non « maintenant » : une horloge qui recule de
/// quatre minutes a chaque ecriture finirait sinon par reculer d'une
/// heure sans jamais etre arretee.
pub fn nouvelle_reference(reference: Option<&str>, maintenant: &str) -> Option<String> {
    let m = lire(maintenant)?;
    match reference.and_then(lire) {
        Some(r) if r >= m => None,
        _ => Some(maintenant.to_string()),
    }
}

/// Le geste du patron est-il permis ? Seulement quand l'ecriture est
/// bloquee : il sert a lever une reference restee dans le futur, pas a
/// effacer une trace quand rien ne l'exige.
pub fn confirmation_permise(reference: Option<&str>, maintenant: &str) -> Result<(), String> {
    match juger(reference, maintenant) {
        Err(_) if lire(maintenant).is_some() => Ok(()),
        Err(e) => Err(e),
        Ok(()) => Err(
            "L'horloge n'est pas en arrière sur la dernière écriture : rien à confirmer, \
             les écritures passent."
                .to_string(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REF: &str = "2026-10-27T14:05:00.000";

    #[test]
    fn une_heure_se_lit_avec_ou_sans_millisecondes() {
        assert!(lire("2026-10-27T14:05:00.123").is_some());
        assert!(lire("2026-10-27T14:05:00").is_some());
        assert!(lire("2026-10-27 14:05:00").is_some());
        assert!(lire("hier").is_none());
        assert!(lire("2026-10-27").is_none(), "une date seule n'est pas une heure");
        assert_eq!(en_clair(REF), "le 27/10/2026 à 14:05");
        assert_eq!(en_clair("hier"), "hier");
    }

    #[test]
    fn sans_reference_tout_passe() {
        assert!(juger(None, "2020-01-01T00:00:00.000").is_ok());
    }

    #[test]
    fn quatre_minutes_en_arriere_passent_six_non() {
        assert!(juger(Some(REF), "2026-10-27T14:01:00.000").is_ok(), "4 min");
        assert!(juger(Some(REF), "2026-10-27T14:00:00.000").is_ok(), "5 min pile : la borne passe");
        let e = juger(Some(REF), "2026-10-27T13:59:59.000").unwrap_err();
        assert!(e.starts_with("HORLOGE_RECULE — "), "{e}");
        assert!(e.contains("dernière écriture le 27/10/2026 à 14:05"), "{e}");
        assert!(e.contains("il est maintenant le 27/10/2026 à 13:59"), "{e}");
        assert!(e.contains("corriger la date et l'heure de Windows"), "{e}");
        assert!(e.contains("Paramètres → Horloge"), "le recours est nommé : {e}");
        // Une machine repartie a hier.
        assert!(juger(Some(REF), "2026-10-26T14:05:00.000").is_err());
    }

    #[test]
    fn une_heure_en_avance_passe_toujours() {
        assert!(juger(Some(REF), "2026-10-28T09:00:00.000").is_ok());
        assert_eq!(recul_secondes(REF, "2026-10-27T14:06:00.000"), Some(-60));
    }

    #[test]
    fn une_reference_illisible_ne_bloque_pas_et_se_remplace() {
        assert!(juger(Some("n'importe quoi"), "2026-10-27T14:00:00.000").is_ok());
        assert_eq!(
            nouvelle_reference(Some("n'importe quoi"), "2026-10-27T14:00:00.000").as_deref(),
            Some("2026-10-27T14:00:00.000")
        );
    }

    #[test]
    fn une_heure_illisible_ne_s_ecrit_pas() {
        assert!(juger(None, "hier").unwrap_err().starts_with("HORLOGE_RECULE"));
        assert_eq!(nouvelle_reference(Some(REF), "hier"), None);
    }

    #[test]
    fn la_reference_ne_recule_jamais() {
        assert_eq!(nouvelle_reference(None, REF).as_deref(), Some(REF));
        // Plus tard : elle avance.
        assert_eq!(
            nouvelle_reference(Some(REF), "2026-10-27T15:00:00.000").as_deref(),
            Some("2026-10-27T15:00:00.000")
        );
        // Un peu plus tot (dans la tolerance) : elle reste ; sinon quatre
        // minutes de recul par ecriture feraient une heure en quinze.
        assert_eq!(nouvelle_reference(Some(REF), "2026-10-27T14:01:00.000"), None);
        assert_eq!(nouvelle_reference(Some(REF), REF), None);
    }

    #[test]
    fn le_patron_ne_confirme_que_ce_qui_bloque() {
        // Avancee en 2030 puis corrigee : bloque, la confirmation passe.
        assert!(confirmation_permise(Some("2030-01-01T08:00:00.000"), REF).is_ok());
        // Rien ne bloque : rien a confirmer.
        let e = confirmation_permise(Some(REF), "2026-10-27T14:03:00.000").unwrap_err();
        assert!(e.contains("rien à confirmer"), "{e}");
        assert!(confirmation_permise(None, REF).is_err());
        // L'heure de la machine illisible : refus, pas de confirmation.
        assert!(confirmation_permise(Some(REF), "hier").is_err());
    }

    #[test]
    fn seul_le_geste_du_patron_echappe_a_la_garde() {
        assert!(exemptee("confirmer_horloge"));
        assert!(!exemptee("creer_vente"));
        assert!(!exemptee("definir_mode_caisse"));
    }
}
