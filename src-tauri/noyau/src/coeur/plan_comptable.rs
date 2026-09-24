//! Le plan comptable SYSCOHADA revise (v3, E-1 — decision D23).
//!
//! Le Mali est dans l'OHADA. Le plan est le meme pour tout le monde :
//! il est COMMUN a tous les dossiers. Ce qui suit est la centaine de
//! comptes usuels d'un commerce, classes 1 a 7 ; le patron ou le
//! comptable y ajoute des sous-comptes par dossier (`4111 Client
//! Coulibaly`). La classe est le premier chiffre ; le parent est le plus
//! long numero du plan qui prefixe le compte.
//!
//! Rien ici n'ecrit d'ecriture : Gescom lit ses operations et les
//! presente au comptable (E-3). Le plan n'est que le vocabulaire.

/// (numero, libelle). L'ordre est celui du plan.
pub const SYSCOHADA: &[(&str, &str)] = &[
    // --- Classe 1 : ressources durables ---
    ("10", "Capital"),
    ("101", "Capital social"),
    ("104", "Compte de l'exploitant"),
    ("11", "Réserves"),
    ("111", "Réserve légale"),
    ("12", "Report à nouveau"),
    ("121", "Report à nouveau créditeur"),
    ("129", "Report à nouveau débiteur"),
    ("13", "Résultat net de l'exercice"),
    ("131", "Résultat net : bénéfice"),
    ("139", "Résultat net : perte"),
    ("16", "Emprunts et dettes assimilées"),
    ("162", "Emprunts et dettes auprès des établissements de crédit"),
    // --- Classe 2 : actif immobilise ---
    ("21", "Immobilisations incorporelles"),
    ("213", "Logiciels et sites internet"),
    ("22", "Terrains"),
    ("23", "Bâtiments, installations techniques et agencements"),
    ("24", "Matériel, mobilier et actifs biologiques"),
    ("244", "Matériel et mobilier"),
    ("2441", "Matériel de bureau"),
    ("2442", "Matériel informatique"),
    ("2444", "Mobilier de bureau"),
    ("245", "Matériel de transport"),
    ("28", "Amortissements"),
    ("284", "Amortissements du matériel"),
    // --- Classe 3 : stocks ---
    ("31", "Marchandises"),
    ("311", "Marchandises A"),
    ("32", "Matières premières et fournitures liées"),
    ("38", "Stocks en cours de route, en consignation ou en dépôt"),
    ("381", "Marchandises en cours de route"),
    ("39", "Dépréciations des stocks"),
    ("391", "Dépréciations des stocks de marchandises"),
    // --- Classe 4 : tiers ---
    ("40", "Fournisseurs et comptes rattachés"),
    ("401", "Fournisseurs, dettes en compte"),
    ("408", "Fournisseurs, factures non parvenues"),
    ("409", "Fournisseurs débiteurs"),
    ("4091", "Fournisseurs, avances et acomptes versés"),
    ("4098", "Fournisseurs, rabais, remises, ristournes et autres avoirs à obtenir"),
    ("41", "Clients et comptes rattachés"),
    ("411", "Clients"),
    ("416", "Créances clients litigieuses ou douteuses"),
    ("418", "Clients, produits à recevoir"),
    ("419", "Clients créditeurs"),
    ("4191", "Clients, avances et acomptes reçus"),
    ("4198", "Clients, rabais, remises, ristournes et autres avoirs à accorder"),
    ("42", "Personnel"),
    ("421", "Personnel, avances et acomptes"),
    ("422", "Personnel, rémunérations dues"),
    ("43", "Organismes sociaux"),
    ("431", "Sécurité sociale"),
    ("44", "État et collectivités publiques"),
    ("441", "État, impôt sur les bénéfices"),
    ("443", "État, TVA facturée"),
    ("4431", "TVA facturée sur ventes"),
    ("4432", "TVA facturée sur prestations de services"),
    ("444", "État, TVA due ou crédit de TVA"),
    ("4441", "État, TVA due"),
    ("4449", "État, crédit de TVA à reporter"),
    ("445", "État, TVA récupérable"),
    ("4452", "TVA récupérable sur achats"),
    ("4453", "TVA récupérable sur transport"),
    ("4454", "TVA récupérable sur services extérieurs"),
    ("447", "État, impôts retenus à la source"),
    ("47", "Débiteurs et créditeurs divers"),
    ("471", "Débiteurs et créditeurs divers"),
    ("476", "Charges constatées d'avance"),
    ("477", "Produits constatés d'avance"),
    ("49", "Dépréciations des comptes de tiers"),
    ("491", "Dépréciations des comptes clients"),
    // --- Classe 5 : tresorerie ---
    ("51", "Valeurs à encaisser"),
    ("511", "Effets à encaisser"),
    ("513", "Chèques à encaisser"),
    ("52", "Banques"),
    ("521", "Banques locales"),
    ("55", "Instruments de monnaie électronique"),
    ("552", "Monnaie électronique — téléphone portable"),
    ("57", "Caisse"),
    ("571", "Caisse siège social"),
    ("58", "Régies d'avances, accréditifs et virements internes"),
    ("585", "Virements de fonds"),
    // --- Classe 6 : charges ---
    ("60", "Achats et variations de stocks"),
    ("601", "Achats de marchandises"),
    ("6019", "Rabais, remises et ristournes obtenus (non ventilés)"),
    ("602", "Achats de matières premières et fournitures liées"),
    ("603", "Variations des stocks de biens achetés"),
    ("6031", "Variations des stocks de marchandises"),
    ("604", "Achats stockés de matières et fournitures consommables"),
    ("605", "Autres achats"),
    ("6051", "Fournitures non stockables — eau"),
    ("6052", "Fournitures non stockables — électricité"),
    ("6053", "Fournitures non stockables — autres énergies"),
    ("6055", "Fournitures de bureau non stockables"),
    ("608", "Achats d'emballages"),
    ("61", "Transports"),
    ("612", "Transports sur ventes"),
    ("614", "Transports du personnel"),
    ("618", "Autres frais de transport"),
    ("62", "Services extérieurs"),
    ("622", "Locations et charges locatives"),
    ("624", "Entretien, réparations et maintenance"),
    ("625", "Primes d'assurance"),
    ("627", "Publicité, publications, relations publiques"),
    ("628", "Frais de télécommunications"),
    ("63", "Autres services extérieurs"),
    ("631", "Frais bancaires"),
    ("632", "Rémunérations d'intermédiaires et de conseils"),
    ("64", "Impôts et taxes"),
    ("641", "Impôts et taxes directs"),
    ("646", "Droits d'enregistrement"),
    ("65", "Autres charges"),
    ("651", "Pertes sur créances clients et autres débiteurs"),
    ("6511", "Pertes sur créances clients"),
    ("658", "Charges diverses"),
    ("66", "Charges de personnel"),
    ("661", "Rémunérations directes versées au personnel national"),
    ("664", "Charges sociales"),
    ("67", "Frais financiers et charges assimilées"),
    ("671", "Intérêts des emprunts"),
    ("673", "Escomptes accordés"),
    ("676", "Pertes de change"),
    ("68", "Dotations aux amortissements"),
    ("681", "Dotations aux amortissements d'exploitation"),
    // --- Classe 7 : produits ---
    ("70", "Ventes"),
    ("701", "Ventes de marchandises"),
    ("7019", "Rabais, remises, ristournes accordés (non ventilés)"),
    ("706", "Services vendus"),
    ("707", "Produits accessoires"),
    ("7071", "Ports, emballages perdus et autres frais facturés"),
    ("71", "Subventions d'exploitation"),
    ("75", "Autres produits"),
    ("758", "Produits divers"),
    ("77", "Revenus financiers et produits assimilés"),
    ("773", "Escomptes obtenus"),
    ("776", "Gains de change"),
];

/// La classe d'un compte : son premier chiffre.
pub fn classe(numero: &str) -> Option<u8> {
    numero.chars().next().and_then(|c| c.to_digit(10)).map(|d| d as u8)
}

/// Le parent d'un compte : le plus long numero connu qui le prefixe
/// (strictement). `None` pour un compte de tete (`10`, `60`…).
pub fn parent_de<'a>(numero: &str, connus: impl IntoIterator<Item = &'a str>) -> Option<String> {
    connus
        .into_iter()
        .filter(|c| c.len() < numero.len() && numero.starts_with(c))
        .max_by_key(|c| c.len())
        .map(str::to_string)
}

/// Juge un sous-compte saisi : des chiffres (4 a 12), une classe de 1 a
/// 7, un parent dans le plan, un libelle. Rend (numero, libelle,
/// parent). Pure.
pub fn valider_sous_compte<'a>(
    numero: &str,
    libelle: &str,
    connus: impl IntoIterator<Item = &'a str> + Clone,
) -> Result<(String, String, String), String> {
    let n = numero.trim().to_string();
    let l = libelle.trim().to_string();
    if n.len() < 4 || n.len() > 12 || !n.chars().all(|c| c.is_ascii_digit()) {
        return Err(format!("Numéro de compte « {n} » : de 4 à 12 chiffres, rien d'autre."));
    }
    if !matches!(classe(&n), Some(1..=7)) {
        return Err(format!("Le compte {n} n'est dans aucune classe de 1 à 7."));
    }
    if l.is_empty() {
        return Err("Le libellé du compte est vide.".to_string());
    }
    if connus.clone().into_iter().any(|c| c == n) {
        return Err(format!("Le compte {n} existe déjà."));
    }
    let parent = parent_de(&n, connus).ok_or_else(|| {
        format!("Le compte {n} ne se rattache à aucun compte du plan : commencer par un numéro du plan (411, 401, 571…).")
    })?;
    Ok((n, l, parent))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn numeros() -> Vec<&'static str> {
        SYSCOHADA.iter().map(|(n, _)| *n).collect()
    }

    #[test]
    fn le_plan_tient_debout() {
        let n = numeros();
        assert!((100..=160).contains(&n.len()), "une centaine de comptes : {}", n.len());
        let mut vus = std::collections::HashSet::new();
        for (num, lib) in SYSCOHADA {
            assert!(vus.insert(*num), "doublon : {num}");
            assert!(num.chars().all(|c| c.is_ascii_digit()) && !lib.is_empty());
            assert!(matches!(classe(num), Some(1..=7)), "{num}");
            // Chaque compte de plus de deux chiffres a son parent dans le plan.
            if num.len() > 2 {
                assert!(parent_de(num, n.iter().copied()).is_some(), "{num} sans parent");
            }
        }
        // Ceux dont l'affectation aura besoin (E-2).
        for c in ["401", "411", "4431", "4452", "521", "571", "601", "701", "7019", "6511", "658", "758", "552", "513"] {
            assert!(n.contains(&c), "{c} manque");
        }
    }

    #[test]
    fn le_parent_est_le_plus_long_prefixe() {
        let n = numeros();
        assert_eq!(parent_de("4431", n.iter().copied()).as_deref(), Some("443"));
        assert_eq!(parent_de("41112", n.iter().copied()).as_deref(), Some("411"));
        assert_eq!(parent_de("10", n.iter().copied()), None);
    }

    #[test]
    fn un_sous_compte_se_juge() {
        let n = numeros();
        let (num, lib, parent) = valider_sous_compte(" 4111 ", " Client Coulibaly ", n.iter().copied()).unwrap();
        assert_eq!((num.as_str(), lib.as_str(), parent.as_str()), ("4111", "Client Coulibaly", "411"));
        assert!(valider_sous_compte("411", "x", n.iter().copied()).unwrap_err().contains("4 à 12"));
        assert!(valider_sous_compte("41A1", "x", n.iter().copied()).is_err());
        assert!(valider_sous_compte("8111", "x", n.iter().copied()).unwrap_err().contains("classe"));
        assert!(valider_sous_compte("4431", "x", n.iter().copied()).unwrap_err().contains("existe"));
        assert!(valider_sous_compte("4111", "  ", n.iter().copied()).unwrap_err().contains("vide"));
        assert!(valider_sous_compte("1901", "x", n.iter().copied()).unwrap_err().contains("rattache"));
    }
}
