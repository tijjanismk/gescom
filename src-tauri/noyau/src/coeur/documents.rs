//! Les reglages des documents imprimes (v3, chantier A — D17, D18).
//!
//! Le commercant ne dessine pas sa facture : il choisit un format, coche
//! quelques cases et nomme au plus trois signatures, PAR GENRE de
//! document. Tout le reste est fixe, dans le generateur de l'ecran.
//!
//! Ce module est pur : les genres, les defauts d'usine, la validation et
//! la fusion d'un reglage enregistre avec les defauts. Le stockage est
//! dans `documents.rs`.

use serde::{Deserialize, Serialize};

/// Les genres, dans l'ordre de l'ecran.
pub const GENRES: [&str; 8] = [
    "facture", "devis", "bon_commande", "bon_livraison", "recu", "releve", "ticket", "bulletin",
];

/// Formats d'impression acceptes.
pub const FORMATS: [&str; 4] = ["a4", "a5", "thermique_80", "thermique_58"];

/// Trois emplacements au plus (D18).
pub const SIGNATURES_MAX: usize = 3;

/// Un libelle de signature tient sous un trait de 6 cm.
pub const LIBELLE_MAX: usize = 40;

/// La mention de bas de page : quelques lignes, pas un contrat.
pub const MENTION_MAX: usize = 600;

/// Les coordonnees de la societe qu'un en-tete SANS image peut afficher
/// sous le nom (le nom, lui, est toujours la : c'est l'ancre du bloc).
/// Un en-tete image les porte deja et les remplace toutes (A2).
pub const COORDONNEES: [&str; 7] =
    ["adresse", "telephone", "telephone2", "email", "site_web", "nif", "rccm"];

/// Celles du generateur historique.
pub const COORDONNEES_DEFAUT: [&str; 5] = ["adresse", "telephone", "telephone2", "nif", "rccm"];

/// Garde les coordonnees connues, sans doublon, dans l'ordre du
/// catalogue ; refuse un nom inconnu plutot que de l'ignorer en silence.
pub fn valider_coordonnees(choix: &[String]) -> Result<Vec<String>, String> {
    for c in choix {
        if !COORDONNEES.contains(&c.as_str()) {
            return Err(format!("Coordonnée inconnue : « {c} »."));
        }
    }
    Ok(COORDONNEES
        .iter()
        .filter(|c| choix.iter().any(|x| x == *c))
        .map(|c| c.to_string())
        .collect())
}

/// Le genre d'un type de piece commerciale. Tout type inconnu retombe
/// sur la facture : c'est le document le plus complet.
pub fn genre_de_piece(type_piece: &str) -> &'static str {
    match type_piece {
        "devis" | "proforma" => "devis",
        "commande_client" | "bon_commande_fournisseur" => "bon_commande",
        "bon_livraison" | "bon_reception" => "bon_livraison",
        _ => "facture",
    }
}

/// « auto » laisse le generateur decider d'apres le document (une
/// colonne remise s'il y a une remise), « oui » et « non » forcent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Choix {
    #[default]
    Auto,
    Oui,
    Non,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Signature {
    pub libelle: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReglageGenre {
    pub format: String,
    #[serde(default)]
    pub colonne_remise: Choix,
    #[serde(default)]
    pub colonne_tva: Choix,
    #[serde(default)]
    pub recap_tva: Choix,
    #[serde(default)]
    pub montant_lettres: bool,
    #[serde(default)]
    pub reference_article: bool,
    /// `None` : le texte `pied_facture` de la societe.
    #[serde(default)]
    pub mention: Option<String>,
    #[serde(default)]
    pub signatures: Vec<Signature>,
}

fn sig(l: &[&str]) -> Vec<Signature> {
    l.iter().map(|s| Signature { libelle: s.to_string() }).collect()
}

/// Les libelles des anciennes paires (v2) : ce sont eux que la boutique
/// a regles. `ancienne(cle)` rend la valeur enregistree, s'il y en a
/// une. Une chaine vide voulait dire « pas de signature » : on la garde.
pub fn defaut(genre: &str, ancienne: &dyn Fn(&str) -> Option<String>) -> ReglageGenre {
    let paire = |famille: &str, g: &str, d: &str| -> Vec<Signature> {
        let gauche = ancienne(&format!("signature_{famille}_gauche")).unwrap_or_else(|| g.to_string());
        let droite = ancienne(&format!("signature_{famille}_droite")).unwrap_or_else(|| d.to_string());
        [gauche, droite]
            .into_iter()
            .filter(|s| !s.trim().is_empty())
            .map(|libelle| Signature { libelle })
            .collect()
    };
    let base = |format: &str| ReglageGenre {
        format: format.to_string(),
        colonne_remise: Choix::Auto,
        colonne_tva: Choix::Auto,
        recap_tva: Choix::Auto,
        montant_lettres: false,
        reference_article: false,
        mention: None,
        signatures: vec![],
    };
    match genre {
        "facture" => ReglageGenre {
            montant_lettres: true,
            signatures: paire("facture", "Pour acquit", "Le fournisseur"),
            ..base("a4")
        },
        "devis" | "bon_commande" => ReglageGenre {
            signatures: paire("defaut", "Le vendeur", "Le client"),
            ..base("a4")
        },
        "bon_livraison" => ReglageGenre {
            signatures: paire("livraison", "Le chauffeur", "Le réceptionnaire"),
            ..base("a4")
        },
        // Ceux du generateur historique, cote client. Le cote
        // fournisseur adapte le libelle a l'impression (« Le caissier »
        // devient « Le bénéficiaire ») : c'est lui qui signe le recu.
        "recu" => ReglageGenre { montant_lettres: true, signatures: sig(&["Le caissier"]), ..base("a5") },
        "releve" => ReglageGenre { signatures: sig(&["Le client", "Pour l'entreprise"]), ..base("a4") },
        "ticket" => ReglageGenre { signatures: sig(&[]), ..base("thermique_80") },
        // Gescom Equipe (G-3, D31) : le bulletin de paie, qu'on remet.
        "bulletin" => ReglageGenre { montant_lettres: true, signatures: sig(&["L'employé", "Pour la société"]), ..base("a4") },
        _ => base("a4"),
    }
}

/// Refuse ce qui ne s'imprimerait pas, et le dit.
pub fn valider(genre: &str, r: &ReglageGenre) -> Result<(), String> {
    if !GENRES.contains(&genre) {
        return Err(format!("Genre de document inconnu : « {genre} »."));
    }
    if !FORMATS.contains(&r.format.as_str()) {
        return Err(format!("Format inconnu : « {} ».", r.format));
    }
    if genre == "ticket" && !r.format.starts_with("thermique") {
        return Err("Un ticket de caisse s'imprime sur rouleau (58 ou 80 mm).".to_string());
    }
    if genre == "bulletin" && r.format.starts_with("thermique") {
        return Err("Un bulletin de paie se remet sur une page (A4 ou A5), pas sur un rouleau.".to_string());
    }
    if genre == "ticket" && !r.signatures.is_empty() {
        return Err("On ne signe pas un ticket de caisse.".to_string());
    }
    if r.signatures.len() > SIGNATURES_MAX {
        return Err(format!(
            "Trois signatures au plus par document (demandé : {}).",
            r.signatures.len()
        ));
    }
    for s in &r.signatures {
        let l = s.libelle.trim();
        if l.is_empty() {
            return Err("Une signature sans libellé ne se signe pas : la retirer.".to_string());
        }
        if l.chars().count() > LIBELLE_MAX {
            return Err(format!("Libellé trop long : « {l} » ({LIBELLE_MAX} caractères au plus)."));
        }
    }
    if let Some(m) = &r.mention {
        if m.chars().count() > MENTION_MAX {
            return Err(format!("Mention trop longue ({MENTION_MAX} caractères au plus)."));
        }
    }
    Ok(())
}

/// Nettoie un reglage valide : libelles sans espaces autour, mention
/// vide = celle de la societe.
pub fn normaliser(mut r: ReglageGenre) -> ReglageGenre {
    for s in &mut r.signatures {
        s.libelle = s.libelle.trim().to_string();
    }
    r.mention = r.mention.map(|m| m.trim().to_string()).filter(|m| !m.is_empty());
    r
}

/// Le reglage d'un genre : l'enregistre s'il est lisible et valide,
/// sinon le defaut. Un reglage casse en base ne doit jamais empecher
/// d'imprimer une facture.
pub fn fusionner(
    genre: &str,
    enregistre: Option<&serde_json::Value>,
    ancienne: &dyn Fn(&str) -> Option<String>,
) -> ReglageGenre {
    enregistre
        .and_then(|v| serde_json::from_value::<ReglageGenre>(v.clone()).ok())
        .filter(|r| valider(genre, r).is_ok())
        .map(normaliser)
        .unwrap_or_else(|| defaut(genre, ancienne))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rien(_: &str) -> Option<String> {
        None
    }

    #[test]
    fn chaque_genre_a_un_defaut_valide() {
        for g in GENRES {
            let r = defaut(g, &rien);
            assert!(valider(g, &r).is_ok(), "{g} : {:?}", valider(g, &r));
        }
    }

    #[test]
    fn les_defauts_reprennent_les_libelles_de_la_v2() {
        let ancienne = |cle: &str| match cle {
            "signature_facture_gauche" => Some("Le client".to_string()),
            "signature_facture_droite" => Some("".to_string()),
            _ => None,
        };
        let f = defaut("facture", &ancienne);
        assert_eq!(f.signatures, sig(&["Le client"]), "vide = pas de signature");
        let bl = defaut("bon_livraison", &ancienne);
        assert_eq!(bl.signatures, sig(&["Le chauffeur", "Le réceptionnaire"]));
    }

    #[test]
    fn le_type_de_piece_donne_son_genre() {
        assert_eq!(genre_de_piece("proforma"), "devis");
        assert_eq!(genre_de_piece("bon_reception"), "bon_livraison");
        assert_eq!(genre_de_piece("bon_commande_fournisseur"), "bon_commande");
        assert_eq!(genre_de_piece("avoir_client"), "facture");
        assert_eq!(genre_de_piece("inconnu"), "facture");
    }

    #[test]
    fn quatre_signatures_c_est_trop() {
        let mut r = defaut("bon_livraison", &rien);
        r.signatures = sig(&["A", "B", "C", "D"]);
        assert!(valider("bon_livraison", &r).unwrap_err().contains("Trois"));
    }

    #[test]
    fn une_signature_vide_ou_trop_longue_est_refusee() {
        let mut r = defaut("facture", &rien);
        r.signatures = sig(&["  "]);
        assert!(valider("facture", &r).is_err());
        r.signatures = sig(&[&"x".repeat(41)]);
        assert!(valider("facture", &r).is_err());
    }

    #[test]
    fn un_ticket_reste_un_ticket() {
        let mut r = defaut("ticket", &rien);
        r.format = "a4".into();
        assert!(valider("ticket", &r).is_err());
        let mut r = defaut("ticket", &rien);
        r.signatures = sig(&["Le client"]);
        assert!(valider("ticket", &r).is_err());
    }

    #[test]
    fn un_format_ou_un_genre_inconnu_est_refuse() {
        let mut r = defaut("facture", &rien);
        r.format = "a3".into();
        assert!(valider("facture", &r).is_err());
        assert!(valider("affiche", &defaut("facture", &rien)).is_err());
    }

    #[test]
    fn un_reglage_casse_retombe_sur_le_defaut() {
        let casse = serde_json::json!({ "format": 12 });
        assert_eq!(fusionner("facture", Some(&casse), &rien), defaut("facture", &rien));
        let invalide = serde_json::json!({ "format": "a3" });
        assert_eq!(fusionner("facture", Some(&invalide), &rien), defaut("facture", &rien));
    }

    #[test]
    fn les_coordonnees_se_trient_et_l_inconnue_est_refusee() {
        let c = valider_coordonnees(&["rccm".into(), "adresse".into(), "rccm".into()]).unwrap();
        assert_eq!(c, vec!["adresse".to_string(), "rccm".to_string()]);
        assert!(valider_coordonnees(&["iban".into()]).is_err());
        assert!(valider_coordonnees(&[]).unwrap().is_empty(), "aucune : le nom seul");
    }

    #[test]
    fn un_reglage_lisible_est_repris_et_nettoye() {
        let v = serde_json::json!({
            "format": "a5", "colonne_remise": "non", "montant_lettres": false,
            "mention": "   ", "signatures": [{ "libelle": "  Le caissier " }]
        });
        let r = fusionner("facture", Some(&v), &rien);
        assert_eq!(r.format, "a5");
        assert_eq!(r.colonne_remise, Choix::Non);
        assert_eq!(r.colonne_tva, Choix::Auto, "champ absent = auto");
        assert_eq!(r.mention, None, "mention vide = celle de la société");
        assert_eq!(r.signatures, sig(&["Le caissier"]));
    }

    #[test]
    fn le_bulletin_de_paie_se_signe_des_deux_cotes_et_pas_sur_rouleau() {
        let b = defaut("bulletin", &rien);
        assert_eq!(b.signatures, sig(&["L'employé", "Pour la société"]));
        assert!(b.montant_lettres);
        assert!(valider("bulletin", &b).is_ok());
        let rouleau = ReglageGenre { format: "thermique_80".into(), ..b };
        assert!(valider("bulletin", &rouleau).unwrap_err().contains("rouleau"));
    }
}
