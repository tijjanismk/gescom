//! D43 — l'assistant de creation du dossier : ce que le serveur accepte
//! (regles pures, testees).
//!
//! L'ecran guide, le serveur juge (D27) : chaque champ de l'assistant
//! repasse ici avant la moindre ecriture.

/// La marque d'une base neuve dont l'assistant reste a faire
/// (`config_app`, cle `ASSISTANT_CLE`). Absente : l'assistant est fait —
/// une base qui existait avant D43 ne le voit jamais.
pub const ASSISTANT_CLE: &str = "assistant_dossier";
pub const ASSISTANT_A_FAIRE: &str = "a_faire";

/// Le code d'erreur que rend le serveur a toute commande tant que
/// l'assistant n'est pas fait.
pub const CODE_ASSISTANT_A_FAIRE: &str = "ASSISTANT_A_FAIRE";

/// Le mot de passe d'usine du patron : il ne se garde pas.
pub const MOT_DE_PASSE_USINE: &str = "admin123";

/// Le taux normal de TVA au Mali, en pourcent. Les deux seuls choix de
/// l'assistant : 0 (la boutique ne facture pas la TVA — la plupart sont
/// a l'impot synthetique) ou 18.
pub const TVA_NORMALE_PCT: i64 = 18;
pub const TAUX_TVA_PROPOSES: &[i64] = &[0, TVA_NORMALE_PCT];

/// Le premier numero de facture le plus haut qu'on accepte : la suite
/// s'imprime sur cinq chiffres (`FAC-2026-01251`).
pub const NUMERO_DEPART_MAX: i64 = 99_999;

/// Un fond de caisse au-dela est une faute de frappe (un zero de trop).
pub const FOND_CAISSE_MAX: i64 = 10_000_000;

pub const MOT_DE_PASSE_MIN: usize = 6;

/// Un champ texte : rogne, vide = absent.
pub fn texte(v: Option<&str>) -> Option<String> {
    v.map(str::trim).filter(|s| !s.is_empty()).map(str::to_string)
}

/// Un champ obligatoire, avec sa longueur maximale.
pub fn obligatoire(v: &str, nom: &str, max: usize) -> Result<String, String> {
    let v = v.trim();
    if v.is_empty() {
        return Err(format!("{nom} : obligatoire."));
    }
    if v.chars().count() > max {
        return Err(format!("{nom} : {max} caractères au plus."));
    }
    Ok(v.to_string())
}

/// Un identifiant de connexion : 3 a 30 caracteres, lettres, chiffres,
/// point, tiret, souligne. Garde tel que tape (la connexion compare a
/// l'identique) ; les doublons, eux, se cherchent sans la casse.
pub fn verifier_pseudo(pseudo: &str) -> Result<String, String> {
    let p = pseudo.trim().to_string();
    let n = p.chars().count();
    if !(3..=30).contains(&n) {
        return Err(format!("Identifiant « {} » : 3 à 30 caractères.", pseudo.trim()));
    }
    if !p.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_')) {
        return Err(format!(
            "Identifiant « {} » : lettres sans accent, chiffres, point, tiret ou souligné.",
            pseudo.trim()
        ));
    }
    Ok(p)
}

/// Un mot de passe choisi : assez long, pas celui d'usine, pas
/// l'identifiant.
pub fn verifier_mot_de_passe(mdp: &str, pseudo: &str) -> Result<(), String> {
    if mdp.chars().count() < MOT_DE_PASSE_MIN {
        return Err(format!("Le mot de passe : {MOT_DE_PASSE_MIN} caractères au moins."));
    }
    if mdp == MOT_DE_PASSE_USINE {
        return Err("Le mot de passe d'usine ne se garde pas : il est écrit dans le manuel.".to_string());
    }
    if mdp.eq_ignore_ascii_case(pseudo) {
        return Err("Le mot de passe ne peut pas être l'identifiant.".to_string());
    }
    Ok(())
}

pub fn verifier_tva(pct: i64) -> Result<(), String> {
    if TAUX_TVA_PROPOSES.contains(&pct) {
        Ok(())
    } else {
        Err(format!("TVA : 0 ou {TVA_NORMALE_PCT} %, rien d'autre."))
    }
}

pub fn verifier_numero_depart(n: i64) -> Result<(), String> {
    if (1..=NUMERO_DEPART_MAX).contains(&n) {
        Ok(())
    } else {
        Err(format!("Premier numéro de facture : de 1 à {NUMERO_DEPART_MAX}."))
    }
}

pub fn verifier_fond_caisse(fond: i64) -> Result<(), String> {
    if fond < 0 {
        return Err("Le fond de caisse ne peut pas être négatif.".to_string());
    }
    if fond > FOND_CAISSE_MAX {
        return Err(format!(
            "Un fond de caisse de plus de {} F : vérifier le nombre de zéros.",
            FOND_CAISSE_MAX
        ));
    }
    Ok(())
}

/// Les dates du premier exercice doivent couvrir aujourd'hui : sinon la
/// premiere vente du jour serait refusee par le garde-fou des dates.
pub fn verifier_dates_couvrent(debut: &str, fin: &str, aujourd_hui: &str) -> Result<(), String> {
    let jour = &aujourd_hui[..10.min(aujourd_hui.len())];
    if jour < debut {
        return Err(format!(
            "Les dates de travail commencent après aujourd'hui ({}) : aucune vente ne passerait d'ici là.",
            crate::coeur::dates::en_lettres(debut)
        ));
    }
    if jour > fin {
        return Err(format!(
            "Les dates de travail finissent avant aujourd'hui ({}) : aucune vente ne passerait.",
            crate::coeur::dates::en_lettres(fin)
        ));
    }
    Ok(())
}

/// Les identifiants d'une meme saisie (le patron puis ses vendeurs) :
/// deux fois le meme, c'est deux comptes qui ne peuvent pas exister.
pub fn verifier_pseudos_distincts(pseudos: &[String]) -> Result<(), String> {
    for (i, p) in pseudos.iter().enumerate() {
        if pseudos[..i].iter().any(|q| q.eq_ignore_ascii_case(p)) {
            return Err(format!("L'identifiant « {p} » est donné deux fois."));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_identifiant_se_normalise_et_se_refuse() {
        assert_eq!(verifier_pseudo(" Moussa.T ").unwrap(), "Moussa.T");
        assert!(verifier_pseudo("ab").is_err());
        assert!(verifier_pseudo("aminata touré").is_err());
        assert!(verifier_pseudo(&"x".repeat(31)).is_err());
    }

    #[test]
    fn le_mot_de_passe_d_usine_ne_se_garde_pas() {
        assert!(verifier_mot_de_passe("admin123", "admin").unwrap_err().contains("usine"));
        assert!(verifier_mot_de_passe("12345", "admin").is_err());
        assert!(verifier_mot_de_passe("Moussa", "moussa").is_err());
        assert!(verifier_mot_de_passe("Bamako-2026", "admin").is_ok());
    }

    #[test]
    fn la_tva_est_zero_ou_dix_huit() {
        assert!(verifier_tva(0).is_ok());
        assert!(verifier_tva(18).is_ok());
        assert!(verifier_tva(20).is_err());
    }

    #[test]
    fn le_numero_de_depart_et_le_fond_ont_leurs_bornes() {
        assert!(verifier_numero_depart(1251).is_ok());
        assert!(verifier_numero_depart(0).is_err());
        assert!(verifier_numero_depart(100_000).is_err());
        assert!(verifier_fond_caisse(0).is_ok());
        assert!(verifier_fond_caisse(25_000).is_ok());
        assert!(verifier_fond_caisse(-1).is_err());
        assert!(verifier_fond_caisse(250_000_000).is_err());
    }

    #[test]
    fn les_dates_doivent_couvrir_aujourd_hui() {
        assert!(verifier_dates_couvrent("2026-01-01", "2026-12-31", "2026-10-28T09:00:00").is_ok());
        assert!(verifier_dates_couvrent("2026-10-28", "2026-10-28", "2026-10-28").is_ok());
        assert!(verifier_dates_couvrent("2027-01-01", "2027-12-31", "2026-10-28").unwrap_err().contains("après"));
        assert!(verifier_dates_couvrent("2025-01-01", "2025-12-31", "2026-10-28").unwrap_err().contains("avant"));
    }

    #[test]
    fn deux_fois_le_meme_identifiant_se_refuse() {
        let l = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(verifier_pseudos_distincts(&l(&["admin", "awa"])).is_ok());
        assert!(verifier_pseudos_distincts(&l(&["admin", "awa", "Awa"])).is_err());
    }

    #[test]
    fn un_champ_obligatoire_se_rogne_et_se_borne() {
        assert_eq!(obligatoire("  Quincaillerie Traoré ", "Raison sociale", 120).unwrap(), "Quincaillerie Traoré");
        assert!(obligatoire("   ", "Raison sociale", 120).is_err());
        assert!(obligatoire(&"x".repeat(121), "Raison sociale", 120).is_err());
        assert_eq!(texte(Some("  ")), None);
    }
}
