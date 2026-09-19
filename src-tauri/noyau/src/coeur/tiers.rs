//! Les tiers — ce qui se calcule sans base.

/// Le prochain code client : le plus grand deja pris, plus un. Jamais le
/// nombre de clients (D28) — un client supprime ou un import avec ses
/// propres numeros suffisent a faire retomber COUNT + 1 sur un code qui
/// existe. `prefixe` : vide pour le dossier d'origine, « CODE- » pour un
/// autre dossier (le code est unique sur toute la base).
pub fn code_client_suivant(prefixe: &str, dernier: Option<i64>) -> String {
    format!("{prefixe}CLIENT{:05}", dernier.unwrap_or(0) + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_code_suit_le_plus_grand_pas_le_nombre() {
        assert_eq!(code_client_suivant("", None), "CLIENT00001");
        assert_eq!(code_client_suivant("", Some(4)), "CLIENT00005");
        // Trois clients dont le 12 : le suivant est 13, pas 4.
        assert_eq!(code_client_suivant("", Some(12)), "CLIENT00013");
        assert_eq!(code_client_suivant("QUINC-", Some(2)), "QUINC-CLIENT00003");
    }
}
