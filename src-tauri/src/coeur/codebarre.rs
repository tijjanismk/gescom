//! Codes-barres EAN-13 — calcul pur, testable.
//!
//! Un EAN-13 fait 13 chiffres : 12 de données + 1 clé de contrôle.
//! La clé se calcule en pondérant les 12 premiers alternativement par
//! 1 et 3, puis en complétant à la dizaine supérieure.
//!
//! Les préfixes 20 à 29 sont réservés à l'usage INTERNE : ils ne sont
//! attribués à aucun pays et ne peuvent donc pas entrer en collision
//! avec un code du commerce. C'est ce qu'utilisent les grandes surfaces
//! pour leurs articles pesés.
//!
//! Un article qui a déjà un code fabricant garde le sien — on ne
//! génère que pour ceux qui n'en ont pas.

/// Préfixe interne. 20 = usage libre, jamais attribué à un pays.
pub const PREFIXE_INTERNE: &str = "20";

/// Clé de contrôle d'un EAN-13, à partir des 12 premiers chiffres.
pub fn cle_ean13(douze: &str) -> Option<u8> {
    if douze.len() != 12 || !douze.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let somme: u32 = douze
        .chars()
        .enumerate()
        .map(|(i, c)| {
            let d = c.to_digit(10).unwrap();
            if i % 2 == 0 { d } else { d * 3 }
        })
        .sum();
    Some(((10 - (somme % 10)) % 10) as u8)
}

/// Construit un EAN-13 interne à partir d'un compteur.
///
/// `sequence` va de 0 à 9 999 999 999 : le préfixe occupe 2 chiffres,
/// il en reste 10 avant la clé. Largement de quoi tenir.
pub fn generer_ean13_interne(sequence: u64) -> Option<String> {
    if sequence > 9_999_999_999 {
        return None;
    }
    let douze = format!("{}{:010}", PREFIXE_INTERNE, sequence);
    let cle = cle_ean13(&douze)?;
    Some(format!("{}{}", douze, cle))
}

/// Un code est-il un EAN-13 valide ?
pub fn valider_ean13(code: &str) -> bool {
    if code.len() != 13 || !code.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    match cle_ean13(&code[..12]) {
        Some(c) => code.as_bytes()[12] - b'0' == c,
        None => false,
    }
}

/// Ce code a-t-il été généré en interne ?
pub fn est_interne(code: &str) -> bool {
    code.len() == 13 && code.starts_with(PREFIXE_INTERNE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cle_connue() {
        // Exemples publics de la norme EAN-13.
        assert_eq!(cle_ean13("400638133393"), Some(1));
        assert_eq!(cle_ean13("978020137962"), Some(4));
        assert_eq!(cle_ean13("501234567890"), Some(0));
    }

    #[test]
    fn cle_refuse_entree_invalide() {
        assert_eq!(cle_ean13("123"), None);
        assert_eq!(cle_ean13("12345678901A"), None);
        assert_eq!(cle_ean13(""), None);
    }

    #[test]
    fn generation_toujours_valide() {
        // Toute sequence doit produire un code que valider_ean13 accepte.
        for seq in [0u64, 1, 42, 999, 123456, 9_999_999_999] {
            let code = generer_ean13_interne(seq).expect("generation");
            assert_eq!(code.len(), 13, "code={}", code);
            assert!(valider_ean13(&code), "code invalide : {}", code);
            assert!(est_interne(&code), "prefixe absent : {}", code);
        }
    }

    #[test]
    fn generation_bornee() {
        assert!(generer_ean13_interne(10_000_000_000).is_none());
    }

    #[test]
    fn sequences_distinctes_donnent_codes_distincts() {
        let a = generer_ean13_interne(1).unwrap();
        let b = generer_ean13_interne(2).unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn validation_rejette_cle_fausse() {
        let bon = generer_ean13_interne(7).unwrap();
        let mut mauvais: Vec<char> = bon.chars().collect();
        // Modifier la cle
        mauvais[12] = if mauvais[12] == '0' { '1' } else { '0' };
        let mauvais: String = mauvais.into_iter().collect();
        assert!(!valider_ean13(&mauvais));
    }

    #[test]
    fn validation_rejette_longueur_et_lettres() {
        assert!(!valider_ean13("12345"));
        assert!(!valider_ean13("ABCDEFGHIJKLM"));
        assert!(!valider_ean13("40063813339"));
    }

    #[test]
    fn code_fabricant_non_interne() {
        // Un code commercial reel ne doit pas passer pour interne.
        assert!(!est_interne("4006381333931"));
    }
}
