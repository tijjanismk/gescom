//! Le suivi client (PLAN-EQUIPE, H-1/H-2 — D34) : echanges, rappels,
//! prospects. Pas de second fichier clients (D34) : un prospect EST un
//! client, distingue par `client.statut`.
//!
//! Les regles ici sont volontairement minces — un genre connu, un
//! texte non vide, un nom non vide — le gros du travail (cloisonnement,
//! generation du code client, fusion avec les relances de creance) vit
//! en base (`crm.rs`), qui n'a pas de version pure a lui.

/// Les genres d'echange reconnus. `note` couvre ce qui n'est ni un
/// appel, ni une visite, ni un message — une remarque sur la fiche.
pub const GENRES_ECHANGE: &[&str] = &["appel", "visite", "whatsapp", "note"];

/// Juge un echange : le genre est connu, il y a quelque chose a dire.
/// Rend le genre et le texte nettoyes. Pure.
pub fn valider_echange(genre: &str, quoi: &str) -> Result<(String, String), String> {
    let genre = genre.trim().to_lowercase();
    let quoi = quoi.trim().to_string();
    if !GENRES_ECHANGE.contains(&genre.as_str()) {
        return Err(format!(
            "Genre d'échange inconnu : « {genre} » (appel, visite, whatsapp ou note)."
        ));
    }
    if quoi.is_empty() {
        return Err("Dire ce qui s'est passé.".to_string());
    }
    Ok((genre, quoi))
}

/// Juge ce qu'il faut faire d'un rappel — la date se juge a part
/// (`coeur::dates`), comme le reste des dates saisies. Pure.
pub fn valider_rappel(quoi: &str) -> Result<String, String> {
    let quoi = quoi.trim().to_string();
    if quoi.is_empty() {
        return Err("Dire ce qu'il faut faire.".to_string());
    }
    Ok(quoi)
}

/// Juge le nom d'un prospect. Pure.
pub fn valider_prospect(nom: &str) -> Result<String, String> {
    let nom = nom.trim().to_string();
    if nom.is_empty() {
        return Err("Le nom est vide.".to_string());
    }
    Ok(nom)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_echange_veut_un_genre_connu_et_quelque_chose_a_dire() {
        assert_eq!(
            valider_echange(" Appel ", " rien de particulier ").unwrap(),
            ("appel".into(), "rien de particulier".into())
        );
        assert!(valider_echange("sms", "bonjour").unwrap_err().contains("inconnu"));
        assert!(valider_echange("appel", "  ").unwrap_err().contains("passé"));
    }

    #[test]
    fn un_rappel_et_un_prospect_veulent_un_texte_non_vide() {
        assert_eq!(valider_rappel(" relancer pour le prix ").unwrap(), "relancer pour le prix");
        assert!(valider_rappel("").unwrap_err().contains("faire"));
        assert_eq!(valider_prospect(" Boutique Konaté ").unwrap(), "Boutique Konaté");
        assert!(valider_prospect("   ").unwrap_err().contains("nom"));
    }
}
