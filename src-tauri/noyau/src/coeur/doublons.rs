//! D44 — reconnaitre un doublon (regles pures, testees).
//!
//! Un doublon d'article ou de fournisseur ne se voit pas a l'oeil :
//! « Riz 25 kg », « riz  25 kg » et « RIZ 25 KG » sont le meme sac. Le
//! nom se compare donc sans la casse et sans les espaces en trop ; les
//! accents, eux, comptent (« The » n'est pas « Thé »).

/// Le nom tel qu'on le compare : rogne, espaces ramenes a un seul,
/// minuscules.
pub fn normaliser_nom(nom: &str) -> String {
    nom.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

/// Le NIF tel qu'on le compare : sans espace, point ni tiret, en
/// majuscules. Vide : pas de NIF.
pub fn normaliser_nif(nif: &str) -> Option<String> {
    let n: String = nif
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_uppercase)
        .collect();
    (!n.is_empty()).then_some(n)
}

/// Ce qu'un fournisseur saisi a de commun avec un fournisseur connu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ressemblance {
    /// Meme NIF : c'est la meme entreprise. Refuse.
    MemeNif,
    /// Meme nom : peut-etre le meme, peut-etre un homonyme (deux
    /// « Moussa Traoré »). Se confirme.
    MemeNom,
}

/// Juge un fournisseur saisi contre un fournisseur connu.
pub fn ressemblance_fournisseur(
    nom: &str,
    nif: Option<&str>,
    nom_connu: &str,
    nif_connu: Option<&str>,
) -> Option<Ressemblance> {
    let nif = nif.and_then(normaliser_nif);
    let nif_connu = nif_connu.and_then(normaliser_nif);
    if nif.is_some() && nif == nif_connu {
        return Some(Ressemblance::MemeNif);
    }
    // Deux NIF connus et differents : deux entreprises, meme homonymes.
    if nif.is_some() && nif_connu.is_some() {
        return None;
    }
    (normaliser_nom(nom) == normaliser_nom(nom_connu)).then_some(Ressemblance::MemeNom)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_nom_se_compare_sans_casse_ni_espaces_en_trop() {
        assert_eq!(normaliser_nom("  Riz   25 KG "), "riz 25 kg");
        assert_eq!(normaliser_nom("Riz 25 kg"), normaliser_nom("riz  25 kg"));
        assert_ne!(normaliser_nom("Thé vert"), normaliser_nom("The vert"), "les accents comptent");
        assert_eq!(normaliser_nom("Éponge"), normaliser_nom("éponge"));
    }

    #[test]
    fn un_nif_se_compare_sans_separateurs() {
        assert_eq!(normaliser_nif(" 084-123 456 m").as_deref(), Some("084123456M"));
        assert_eq!(normaliser_nif(" - "), None);
    }

    #[test]
    fn meme_nif_refuse_meme_nom_se_confirme() {
        use Ressemblance::*;
        assert_eq!(ressemblance_fournisseur("Sodima", Some("084 123"), "SODIMA SARL", Some("084123")), Some(MemeNif));
        assert_eq!(ressemblance_fournisseur("Moussa Traoré", None, "moussa  traoré", Some("1")), Some(MemeNom));
        assert_eq!(ressemblance_fournisseur("Moussa Traoré", Some("2"), "Moussa Traoré", Some("1")), None, "deux NIF différents");
        assert_eq!(ressemblance_fournisseur("Moussa Traoré", Some("2"), "Moussa Traoré", None), Some(MemeNom));
        assert_eq!(ressemblance_fournisseur("Awa", None, "Awa Diallo", None), None);
        assert_eq!(ressemblance_fournisseur("A", Some(""), "B", Some("")), None, "deux NIF vides ne se ressemblent pas");
    }
}
