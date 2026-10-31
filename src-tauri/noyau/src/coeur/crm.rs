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

// --- D50, points 10 a 16 : l'etape d'un prospect, le client parti ---

/// Les etapes d'un prospect (D50, point 10). Vide en base vaut `nouveau`.
pub const ETAPES_PROSPECT: &[&str] = &["nouveau", "contacte", "interesse", "abandonne"];
pub const ETAPE_NOUVEAU: &str = "nouveau";
pub const ETAPE_CONTACTE: &str = "contacte";
pub const ETAPE_ABANDONNE: &str = "abandonne";

/// Pourquoi un prospect est abandonne (point 11) — « autre » : une liste
/// fermee obligerait a mentir.
pub const RAISONS_ABANDON: &[&str] = &["trop_cher", "achete_ailleurs", "plus_besoin", "injoignable", "autre"];
/// Pourquoi un client est parti (point 13).
pub const RAISONS_DEPART: &[&str] = &["prix", "concurrent", "mecontent", "demenage", "autre"];

/// L'etape d'un prospect, lue en base : vide vaut `nouveau`. Pure.
pub fn etape_lue(etape: Option<&str>) -> &str {
    match etape.map(str::trim) {
        Some(e) if !e.is_empty() => e,
        _ => ETAPE_NOUVEAU,
    }
}

/// Juge un changement d'etape : l'etape est connue, et `abandonne`
/// exige une raison de la liste (les autres n'en gardent aucune). Rend
/// l'etape et la raison a ecrire. Pure.
pub fn valider_etape(etape: &str, raison: Option<&str>) -> Result<(String, Option<String>), String> {
    let etape = etape.trim().to_lowercase();
    if !ETAPES_PROSPECT.contains(&etape.as_str()) {
        return Err(format!(
            "Étape inconnue : « {etape} » (nouveau, contacte, interesse ou abandonne)."
        ));
    }
    if etape != ETAPE_ABANDONNE {
        return Ok((etape, None));
    }
    let raison = raison.map(|r| r.trim().to_lowercase()).filter(|r| !r.is_empty());
    match raison {
        None => Err("Dire pourquoi le prospect est abandonné.".to_string()),
        Some(r) if !RAISONS_ABANDON.contains(&r.as_str()) => Err(format!(
            "Raison inconnue : « {r} » (trop_cher, achete_ailleurs, plus_besoin, injoignable ou autre)."
        )),
        Some(r) => Ok((etape, Some(r))),
    }
}

/// Juge la raison d'un depart. Pure.
pub fn valider_raison_depart(raison: &str) -> Result<String, String> {
    let r = raison.trim().to_lowercase();
    if r.is_empty() {
        return Err("Dire pourquoi le client est parti.".to_string());
    }
    if !RAISONS_DEPART.contains(&r.as_str()) {
        return Err(format!(
            "Raison inconnue : « {r} » (prix, concurrent, mecontent, demenage ou autre)."
        ));
    }
    Ok(r)
}

/// Juge un mois « AAAA-MM » (le compte des prospects). Pure.
pub fn valider_mois(mois: &str) -> Result<String, String> {
    let m = mois.trim();
    let ok = m.len() == 7
        && m.as_bytes()[4] == b'-'
        && m[..4].bytes().all(|b| b.is_ascii_digit())
        && m[5..].parse::<u32>().is_ok_and(|n| (1..=12).contains(&n));
    if !ok {
        return Err(format!("Mois illisible : « {m} » (attendu AAAA-MM)."));
    }
    Ok(m.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l_etape_d_un_prospect_et_la_raison_d_un_abandon() {
        assert_eq!(etape_lue(None), "nouveau");
        assert_eq!(etape_lue(Some(" ")), "nouveau");
        assert_eq!(etape_lue(Some("interesse")), "interesse");
        assert_eq!(valider_etape(" Contacte ", Some("prix")).unwrap(), ("contacte".into(), None), "seul l'abandon garde une raison");
        assert!(valider_etape("gagne", None).unwrap_err().contains("inconnue"));
        assert!(valider_etape("abandonne", None).unwrap_err().contains("pourquoi"));
        assert!(valider_etape("abandonne", Some("prix")).unwrap_err().contains("inconnue"), "prix est une raison de depart");
        assert_eq!(valider_etape("abandonne", Some(" Autre ")).unwrap(), ("abandonne".into(), Some("autre".into())));
    }

    #[test]
    fn la_raison_d_un_depart_et_le_mois() {
        assert_eq!(valider_raison_depart(" Concurrent ").unwrap(), "concurrent");
        assert!(valider_raison_depart("").unwrap_err().contains("pourquoi"));
        assert!(valider_raison_depart("trop_cher").unwrap_err().contains("inconnue"));
        assert_eq!(valider_mois("2026-10").unwrap(), "2026-10");
        assert!(valider_mois("2026-13").is_err());
        assert!(valider_mois("2026-1").is_err());
        assert!(valider_mois("26-10-01").is_err());
    }

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
