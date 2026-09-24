//! L'affectation comptable (v3, E-2 — decision D23) : quelle operation
//! de Gescom va sur quel compte du plan.
//!
//! Les defauts sont livres ICI, pas en base : une affectation absente
//! de la table vaut son defaut. La table ne garde que ce que le patron
//! ou le comptable a change, par dossier — un defaut corrige dans une
//! version suivante profite a tous ceux qui ne l'ont pas touche.
//!
//! Chaque operation n'accepte que certains comptes (`prefixes`) : une
//! vente sur un compte de charge ou une caisse en classe 6 ferait des
//! journaux faux sans que rien ne le dise.

/// Une operation, son libelle, son groupe a l'ecran, son compte par
/// defaut et les debuts de numero qu'elle accepte.
pub struct Operation {
    pub cle: &'static str,
    pub libelle: &'static str,
    pub groupe: &'static str,
    pub defaut: &'static str,
    pub prefixes: &'static [&'static str],
}

const TRESO: &[&str] = &["5"];
const CHARGE: &[&str] = &["6"];

pub const OPERATIONS: &[Operation] = &[
    // --- Tresorerie : un compte par mode de paiement (coeur::saisie) ---
    Operation { cle: "tresorerie:especes", libelle: "Espèces", groupe: "Trésorerie", defaut: "571", prefixes: TRESO },
    Operation { cle: "tresorerie:orange_money", libelle: "Orange Money", groupe: "Trésorerie", defaut: "552", prefixes: TRESO },
    Operation { cle: "tresorerie:moov_money", libelle: "Moov Money", groupe: "Trésorerie", defaut: "552", prefixes: TRESO },
    Operation { cle: "tresorerie:cheque", libelle: "Chèques reçus", groupe: "Trésorerie", defaut: "513", prefixes: TRESO },
    Operation { cle: "tresorerie:virement", libelle: "Virements", groupe: "Trésorerie", defaut: "521", prefixes: TRESO },
    // --- Ventes ---
    Operation { cle: "ventes:marchandises", libelle: "Ventes de marchandises", groupe: "Ventes", defaut: "701", prefixes: &["70"] },
    Operation { cle: "ventes:remises", libelle: "Remises accordées", groupe: "Ventes", defaut: "7019", prefixes: &["70"] },
    Operation { cle: "ventes:tva", libelle: "TVA collectée", groupe: "Ventes", defaut: "4431", prefixes: &["443", "444"] },
    Operation { cle: "ventes:clients", libelle: "Clients (crédit, règlements)", groupe: "Ventes", defaut: "411", prefixes: &["41"] },
    Operation { cle: "ventes:retours", libelle: "Retours de marchandise", groupe: "Ventes", defaut: "701", prefixes: &["70"] },
    Operation { cle: "ventes:avoirs", libelle: "Avoirs accordés sans marchandise", groupe: "Ventes", defaut: "7019", prefixes: &["70"] },
    Operation { cle: "ventes:irrecouvrables", libelle: "Créances irrécouvrables", groupe: "Ventes", defaut: "6511", prefixes: &["65"] },
    // --- Achats ---
    Operation { cle: "achats:marchandises", libelle: "Achats de marchandises", groupe: "Achats", defaut: "601", prefixes: &["60"] },
    Operation { cle: "achats:tva", libelle: "TVA récupérable", groupe: "Achats", defaut: "4452", prefixes: &["445"] },
    Operation { cle: "achats:fournisseurs", libelle: "Fournisseurs", groupe: "Achats", defaut: "401", prefixes: &["40"] },
    Operation { cle: "achats:retours", libelle: "Retours aux fournisseurs", groupe: "Achats", defaut: "601", prefixes: &["60"] },
    // --- Caisse ---
    Operation { cle: "caisse:ecart_manquant", libelle: "Manquant à la clôture", groupe: "Caisse", defaut: "658", prefixes: CHARGE },
    Operation { cle: "caisse:ecart_excedent", libelle: "Excédent à la clôture", groupe: "Caisse", defaut: "758", prefixes: &["7"] },
    // --- Depenses de caisse, par categorie (ecran Caisse) ---
    Operation { cle: "depense:transport", libelle: "Transport", groupe: "Dépenses", defaut: "618", prefixes: CHARGE },
    Operation { cle: "depense:carburant", libelle: "Carburant", groupe: "Dépenses", defaut: "6053", prefixes: CHARGE },
    Operation { cle: "depense:loyer", libelle: "Loyer", groupe: "Dépenses", defaut: "622", prefixes: CHARGE },
    Operation { cle: "depense:salaire", libelle: "Salaire / main d'œuvre", groupe: "Dépenses", defaut: "661", prefixes: CHARGE },
    Operation { cle: "depense:electricite", libelle: "Électricité", groupe: "Dépenses", defaut: "6052", prefixes: CHARGE },
    Operation { cle: "depense:eau", libelle: "Eau", groupe: "Dépenses", defaut: "6051", prefixes: CHARGE },
    Operation { cle: "depense:fourniture", libelle: "Fournitures", groupe: "Dépenses", defaut: "6055", prefixes: CHARGE },
    Operation { cle: "depense:entretien", libelle: "Entretien", groupe: "Dépenses", defaut: "624", prefixes: CHARGE },
    Operation { cle: "depense:taxe", libelle: "Taxe / impôt", groupe: "Dépenses", defaut: "641", prefixes: CHARGE },
    Operation { cle: "depense:autre", libelle: "Autre dépense", groupe: "Dépenses", defaut: "658", prefixes: CHARGE },
    // --- Paie (Gescom Equipe, G-4) : le journal PA ---
    Operation { cle: "paie:salaires", libelle: "Salaires bruts", groupe: "Paie", defaut: "661", prefixes: &["66"] },
    Operation { cle: "paie:remunerations_dues", libelle: "Rémunérations dues au personnel", groupe: "Paie", defaut: "422", prefixes: &["42"] },
    Operation { cle: "paie:avances", libelle: "Avances au personnel", groupe: "Paie", defaut: "421", prefixes: &["42"] },
    Operation { cle: "paie:retenues", libelle: "Retenues saisies (casse, absence…)", groupe: "Paie", defaut: "758", prefixes: &["75", "66"] },
    Operation { cle: "paie:charges_sociales", libelle: "Charges sociales de l'employeur", groupe: "Paie", defaut: "664", prefixes: &["66"] },
    Operation { cle: "paie:organismes", libelle: "Organismes sociaux (cotisations sans compte)", groupe: "Paie", defaut: "431", prefixes: &["43", "44"] },
];

pub fn operation(cle: &str) -> Option<&'static Operation> {
    OPERATIONS.iter().find(|o| o.cle == cle)
}

/// L'operation d'un mode de paiement ; un mode inconnu va en especes
/// (le mode est juge a la saisie, `coeur::saisie`).
pub fn cle_tresorerie(mode: &str) -> &'static str {
    let cle = format!("tresorerie:{mode}");
    operation(&cle).map(|o| o.cle).unwrap_or("tresorerie:especes")
}

/// L'operation d'une categorie de depense ; une categorie absente ou
/// inconnue va en « autre ».
pub fn cle_depense(categorie: Option<&str>) -> &'static str {
    let cle = format!("depense:{}", categorie.unwrap_or("autre"));
    operation(&cle).map(|o| o.cle).unwrap_or("depense:autre")
}

/// Ce compte convient-il a cette operation ? Pure.
pub fn verifier(cle: &str, compte: &str) -> Result<(), String> {
    let op = operation(cle).ok_or_else(|| format!("Opération inconnue : « {cle} »."))?;
    if op.prefixes.iter().any(|p| compte.starts_with(p)) {
        Ok(())
    } else {
        Err(format!(
            "« {} » ne va pas sur le compte {compte} : il faut un compte qui commence par {}.",
            op.libelle,
            op.prefixes.join(" ou ")
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coeur::plan_comptable::SYSCOHADA;

    #[test]
    fn chaque_operation_a_un_compte_du_plan_qui_lui_convient() {
        let mut vues = std::collections::HashSet::new();
        for o in OPERATIONS {
            assert!(vues.insert(o.cle), "doublon {}", o.cle);
            assert!(SYSCOHADA.iter().any(|(n, _)| *n == o.defaut), "{} : {} absent du plan", o.cle, o.defaut);
            verifier(o.cle, o.defaut).unwrap_or_else(|e| panic!("{}: {e}", o.cle));
        }
    }

    #[test]
    fn chaque_mode_de_paiement_a_sa_tresorerie() {
        for m in crate::coeur::saisie::MODES_ENCAISSEMENT {
            assert_eq!(cle_tresorerie(m), format!("tresorerie:{m}"), "{m}");
        }
        assert_eq!(cle_tresorerie("inconnu"), "tresorerie:especes");
    }

    #[test]
    fn une_depense_sans_categorie_va_en_autre() {
        assert_eq!(cle_depense(Some("loyer")), "depense:loyer");
        assert_eq!(cle_depense(None), "depense:autre");
        assert_eq!(cle_depense(Some("bizarre")), "depense:autre");
    }

    #[test]
    fn un_compte_qui_ne_convient_pas_est_refuse_en_le_disant() {
        let e = verifier("ventes:marchandises", "601").unwrap_err();
        assert!(e.contains("commence par 70"), "{e}");
        assert!(verifier("tresorerie:especes", "5711").is_ok());
        assert!(verifier("tresorerie:especes", "411").is_err());
        assert!(verifier("inconnue", "571").is_err());
    }
}
