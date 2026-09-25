//! Les tiers — ce qui se calcule sans base.

/// Le prochain code client : le plus grand deja pris, plus un. Jamais le
/// nombre de clients (D28) — un client supprime ou un import avec ses
/// propres numeros suffisent a faire retomber COUNT + 1 sur un code qui
/// existe. `prefixe` : vide pour le dossier d'origine, « CODE- » pour un
/// autre dossier (le code est unique sur toute la base).
pub fn code_client_suivant(prefixe: &str, dernier: Option<i64>) -> String {
    format!("{prefixe}CLIENT{:05}", dernier.unwrap_or(0) + 1)
}

/// Le fournisseur des achats faits sans en nommer un (le « client de
/// passage » des achats) : un par dossier, cree a la premiere occasion.
/// Sans lui, un achat sans fournisseur n'avait ni piece ni paiement —
/// la somme n'etait dite nulle part, et le journal AC laissait une dette
/// au 401 que rien ne soldait.
pub const FOURNISSEUR_DIVERS: &str = "Fournisseur divers";

/// Un achat sans fournisseur se paie comptant : on ne doit rien a
/// personne. A credit (avec ou sans acompte), il faut nommer le
/// fournisseur. Pure.
pub fn verifier_achat_sans_fournisseur(mode_reglement: Option<&str>) -> Result<(), String> {
    if mode_reglement == Some("comptant") {
        Ok(())
    } else {
        Err("Un achat sans fournisseur se paie comptant. Pour acheter à crédit \
             (avec ou sans acompte), choisir le fournisseur."
            .to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sans_fournisseur_on_paie_comptant() {
        assert!(verifier_achat_sans_fournisseur(Some("comptant")).is_ok());
        assert!(verifier_achat_sans_fournisseur(Some("credit")).unwrap_err().contains("comptant"));
        assert!(verifier_achat_sans_fournisseur(None).is_err(), "le mode par défaut est le crédit");
    }

    #[test]
    fn le_code_suit_le_plus_grand_pas_le_nombre() {
        assert_eq!(code_client_suivant("", None), "CLIENT00001");
        assert_eq!(code_client_suivant("", Some(4)), "CLIENT00005");
        // Trois clients dont le 12 : le suivant est 13, pas 4.
        assert_eq!(code_client_suivant("", Some(12)), "CLIENT00013");
        assert_eq!(code_client_suivant("QUINC-", Some(2)), "QUINC-CLIENT00003");
    }
}
