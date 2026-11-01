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

// ---------------------------------------------------------------------
//  Les colonnes d'une piece (D58)
// ---------------------------------------------------------------------
//
// Une colonne n'apparait que cochee : plus d'« auto » qui decide d'apres
// le document. Cacher une colonne ne change jamais un montant.

/// Le catalogue, dans l'ordre du tableau.
pub const COLONNES: [&str; 7] =
    ["reference", "designation", "quantite", "prix_unitaire", "remise", "tva", "total"];

/// Toujours la : sans elles une piece ne se verifie plus. L'unite suit
/// la quantite (« 3 sacs »).
pub const COLONNES_FIXES: [&str; 4] = ["designation", "quantite", "prix_unitaire", "total"];

/// Les genres qui ont un tableau de lignes. Le recu, le releve, le
/// ticket et le bulletin ont leur propre mise en page.
pub const GENRES_A_COLONNES: [&str; 4] = ["facture", "devis", "bon_commande", "bon_livraison"];

/// D'office : les colonnes fixes seulement.
pub fn colonnes_defaut() -> Vec<String> {
    COLONNES_FIXES.iter().map(|c| c.to_string()).collect()
}

/// Garde les colonnes connues, sans doublon, dans l'ordre du catalogue.
/// Refuse une colonne inconnue et une colonne fixe retiree, et le dit.
pub fn valider_colonnes(choix: &[String]) -> Result<Vec<String>, String> {
    for c in choix {
        if !COLONNES.contains(&c.as_str()) {
            return Err(format!("Colonne inconnue : « {c} »."));
        }
    }
    for f in COLONNES_FIXES {
        if !choix.iter().any(|c| c == f) {
            return Err(format!("La colonne « {} » reste toujours sur une pièce.", libelle_colonne(f)));
        }
    }
    Ok(COLONNES.iter().filter(|c| choix.iter().any(|x| x == *c)).map(|c| c.to_string()).collect())
}

pub fn libelle_colonne(c: &str) -> &'static str {
    match c {
        "reference" => "Référence",
        "designation" => "Désignation",
        "quantite" => "Quantité",
        "prix_unitaire" => "Prix unitaire",
        "remise" => "Remise",
        "tva" => "TVA %",
        "total" => "Total",
        _ => "?",
    }
}

/// Les colonnes gardees avec une piece (`piece_commerciale.colonnes`,
/// JSON). `None` : elle suit le reglage de son genre — aussi pour un
/// texte abime : un reglage ne bloque jamais une impression.
pub fn colonnes_gardees(texte: Option<&str>) -> Option<Vec<String>> {
    texte
        .and_then(|t| serde_json::from_str::<Vec<String>>(t).ok())
        .and_then(|c| valider_colonnes(&c).ok())
}

/// Un reglage d'avant D58 (`colonne_remise`, `colonne_tva` en
/// auto/oui/non, `reference_article`, `recap_tva` en texte) dans la
/// forme d'aujourd'hui : « oui » devient coche, « auto » et « non »
/// decoches, comme une base neuve. Un reglage qui a deja `colonnes`
/// perd seulement les anciens champs.
pub fn migrer(v: &serde_json::Value) -> serde_json::Value {
    let mut v = v.clone();
    let Some(o) = v.as_object_mut() else { return v };
    let oui = |x: Option<&serde_json::Value>| x.and_then(|x| x.as_str()) == Some("oui");
    if !o.contains_key("colonnes") {
        let mut c = colonnes_defaut();
        if o.get("reference_article").and_then(|x| x.as_bool()) == Some(true) {
            c.push("reference".into());
        }
        if oui(o.get("colonne_remise")) {
            c.push("remise".into());
        }
        if oui(o.get("colonne_tva")) {
            c.push("tva".into());
        }
        // L'ordre du catalogue ; tout est connu, rien ne peut echouer.
        let c: Vec<String> = COLONNES.iter().filter(|x| c.iter().any(|y| y == *x)).map(|x| x.to_string()).collect();
        o.insert("colonnes".into(), serde_json::json!(c));
    }
    if let Some(r) = o.get("recap_tva").filter(|r| r.is_string()).cloned() {
        o.insert("recap_tva".into(), serde_json::json!(r.as_str() == Some("oui")));
    }
    for ancien in ["colonne_remise", "colonne_tva", "reference_article"] {
        o.remove(ancien);
    }
    v
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Signature {
    pub libelle: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReglageGenre {
    pub format: String,
    /// Les colonnes affichees, a la saisie et sur le papier (D58).
    #[serde(default = "colonnes_defaut")]
    pub colonnes: Vec<String>,
    /// Le recapitulatif HT / TVA par taux, sous les totaux (D58 : une
    /// case, plus d'« auto »).
    #[serde(default)]
    pub recap_tva: bool,
    #[serde(default)]
    pub montant_lettres: bool,
    /// `None` : le texte `pied_facture` de la societe.
    #[serde(default)]
    pub mention: Option<String>,
    #[serde(default)]
    pub signatures: Vec<Signature>,
    /// Le filigrane de la societe sur ce genre (25/09). Absent d'un
    /// reglage d'avant : oui — le filigrane se pose une fois pour tous,
    /// on le retire d'un genre, on ne l'ajoute pas genre par genre.
    #[serde(default = "vrai")]
    pub filigrane: bool,
    /// Le duplicata (26/10/2026) : apres la premiere impression, la
    /// feuille dit « DUPLICATA ». Optionnel, donc NON par defaut — une
    /// mise a jour ne change pas le papier sans qu'on l'ait coche.
    #[serde(default)]
    pub duplicata: bool,
}

fn vrai() -> bool {
    true
}

/// Les genres qui peuvent porter un duplicata : les pieces numerotees,
/// le recu, le ticket. Le releve se reimprime a volonte ; le bulletin ne
/// passe pas par les impressions comptees.
pub const GENRES_DUPLICATA: [&str; 6] = ["facture", "devis", "bon_commande", "bon_livraison", "recu", "ticket"];

/// Le genre dont le reglage dit si une impression porte le duplicata.
/// Une piece imprimee sur rouleau est un ticket ; `type_piece` absent :
/// un recu.
pub fn genre_imprime(type_piece: Option<&str>, format: Option<&str>) -> &'static str {
    match type_piece {
        None => "recu",
        Some(_) if format.is_some_and(|f| f.starts_with("thermique")) => "ticket",
        Some(t) => genre_de_piece(t),
    }
}

// ---------------------------------------------------------------------
//  Le filigrane (25/09)
// ---------------------------------------------------------------------
//
// Une image (le logo, souvent) OU un mot (« COPIE », « PAYÉ »), au
// milieu de chaque page, DERRIERE le texte. Estompe et floute : c'est
// ce qui l'empeche de se melanger aux lignes des tableaux — on doit
// le deviner, pas le lire par-dessus les montants.

/// Opacite, en pour cent : au-dela de 30 le filigrane se lit avant les
/// montants ; en deca de 3 il ne se voit plus a l'impression.
pub const FILIGRANE_OPACITE: std::ops::RangeInclusive<u8> = 3..=30;
/// Flou, en pixels d'ecran.
pub const FILIGRANE_FLOU: std::ops::RangeInclusive<u8> = 0..=6;
/// Taille, en pour cent de la largeur de la page.
pub const FILIGRANE_TAILLE: std::ops::RangeInclusive<u8> = 20..=100;
/// Un mot ou deux, pas une phrase.
pub const FILIGRANE_TEXTE_MAX: usize = 30;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Filigrane {
    /// Le mot, quand il n'y a pas d'image ; l'image l'emporte.
    #[serde(default)]
    pub texte: Option<String>,
    #[serde(default = "Filigrane::opacite_defaut")]
    pub opacite: u8,
    #[serde(default = "Filigrane::flou_defaut")]
    pub flou: u8,
    #[serde(default = "Filigrane::taille_defaut")]
    pub taille: u8,
}

impl Filigrane {
    fn opacite_defaut() -> u8 {
        8
    }
    fn flou_defaut() -> u8 {
        1
    }
    fn taille_defaut() -> u8 {
        60
    }
}

impl Default for Filigrane {
    fn default() -> Self {
        Filigrane {
            texte: None,
            opacite: Self::opacite_defaut(),
            flou: Self::flou_defaut(),
            taille: Self::taille_defaut(),
        }
    }
}

/// Refuse un filigrane qui masquerait le document ou ne se verrait pas.
pub fn valider_filigrane(f: &Filigrane) -> Result<(), String> {
    if !FILIGRANE_OPACITE.contains(&f.opacite) {
        return Err(format!(
            "Intensité du filigrane : entre {} et {} % (demandé : {} %).",
            FILIGRANE_OPACITE.start(), FILIGRANE_OPACITE.end(), f.opacite
        ));
    }
    if !FILIGRANE_FLOU.contains(&f.flou) {
        return Err(format!("Flou du filigrane : de {} à {} (demandé : {}).", FILIGRANE_FLOU.start(), FILIGRANE_FLOU.end(), f.flou));
    }
    if !FILIGRANE_TAILLE.contains(&f.taille) {
        return Err(format!(
            "Taille du filigrane : entre {} et {} % de la page (demandé : {} %).",
            FILIGRANE_TAILLE.start(), FILIGRANE_TAILLE.end(), f.taille
        ));
    }
    if let Some(t) = &f.texte {
        if t.trim().chars().count() > FILIGRANE_TEXTE_MAX {
            return Err(format!("Texte du filigrane trop long ({FILIGRANE_TEXTE_MAX} caractères au plus)."));
        }
    }
    Ok(())
}

/// Texte sans espaces autour ; vide = pas de texte.
pub fn normaliser_filigrane(mut f: Filigrane) -> Filigrane {
    f.texte = f.texte.map(|t| t.trim().to_string()).filter(|t| !t.is_empty());
    f
}

/// Le filigrane enregistre s'il est lisible et valide, sinon celui
/// d'usine (sans texte : rien ne s'imprime tant qu'on n'a rien pose).
pub fn fusionner_filigrane(enregistre: Option<&serde_json::Value>) -> Filigrane {
    enregistre
        .and_then(|v| serde_json::from_value::<Filigrane>(v.clone()).ok())
        .filter(|f| valider_filigrane(f).is_ok())
        .map(normaliser_filigrane)
        .unwrap_or_default()
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
        colonnes: colonnes_defaut(),
        recap_tva: false,
        montant_lettres: false,
        mention: None,
        signatures: vec![],
        filigrane: true,
        duplicata: false,
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
        // Un rouleau n'a pas de place pour un filigrane.
        "ticket" => ReglageGenre { signatures: sig(&[]), filigrane: false, ..base("thermique_80") },
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
    if r.duplicata && !GENRES_DUPLICATA.contains(&genre) {
        return Err("Pas de duplicata sur ce document : il se réimprime à volonté.".to_string());
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
    valider_colonnes(&r.colonnes)?;
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
    if let Ok(c) = valider_colonnes(&r.colonnes) {
        r.colonnes = c;
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
        .and_then(|v| serde_json::from_value::<ReglageGenre>(migrer(v)).ok())
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
    fn le_duplicata_est_optionnel_et_borne() {
        for g in GENRES {
            assert!(!defaut(g, &rien).duplicata, "{g} : non par défaut");
        }
        // Un reglage d'avant le duplicata se lit sans lui.
        let ancien = serde_json::json!({ "format": "a4" });
        assert!(!fusionner("facture", Some(&ancien), &rien).duplicata);
        for g in GENRES {
            let r = ReglageGenre { duplicata: true, ..defaut(g, &rien) };
            assert_eq!(valider(g, &r).is_ok(), GENRES_DUPLICATA.contains(&g), "{g}");
        }
    }

    #[test]
    fn le_genre_imprime_suit_la_piece_et_le_rouleau() {
        assert_eq!(genre_imprime(Some("facture"), Some("a4")), "facture");
        assert_eq!(genre_imprime(Some("proforma"), None), "devis");
        assert_eq!(genre_imprime(Some("facture"), Some("thermique_80")), "ticket");
        assert_eq!(genre_imprime(Some("bon_livraison"), Some("a5")), "bon_livraison");
        assert_eq!(genre_imprime(None, Some("a5")), "recu");
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
        assert_eq!(r.colonnes, colonnes_defaut(), "remise « non », TVA absente : décochées");
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

    #[test]
    fn un_reglage_d_avant_le_filigrane_le_garde() {
        let v = serde_json::json!({ "format": "a4" });
        assert!(fusionner("facture", Some(&v), &rien).filigrane, "champ absent = oui");
        assert!(!defaut("ticket", &rien).filigrane, "pas sur un rouleau");
    }

    #[test]
    fn le_filigrane_reste_discret_et_lisible() {
        let f = Filigrane::default();
        assert!(valider_filigrane(&f).is_ok());
        assert!(valider_filigrane(&Filigrane { opacite: 60, ..f.clone() }).unwrap_err().contains("Intensité"));
        assert!(valider_filigrane(&Filigrane { opacite: 1, ..f.clone() }).is_err(), "invisible");
        assert!(valider_filigrane(&Filigrane { flou: 9, ..f.clone() }).is_err());
        assert!(valider_filigrane(&Filigrane { taille: 150, ..f.clone() }).is_err());
        assert!(valider_filigrane(&Filigrane { texte: Some("x".repeat(31)), ..f.clone() }).is_err());
        let n = normaliser_filigrane(Filigrane { texte: Some("  COPIE ".into()), ..f.clone() });
        assert_eq!(n.texte.as_deref(), Some("COPIE"));
        assert_eq!(normaliser_filigrane(Filigrane { texte: Some("   ".into()), ..f }).texte, None);
    }

    #[test]
    fn un_filigrane_abime_retombe_sur_l_usine() {
        let casse = serde_json::json!({ "opacite": 95 });
        assert_eq!(fusionner_filigrane(Some(&casse)), Filigrane::default());
        let lisible = serde_json::json!({ "texte": " PAYÉ ", "opacite": 12 });
        let f = fusionner_filigrane(Some(&lisible));
        assert_eq!((f.texte.as_deref(), f.opacite, f.flou, f.taille), (Some("PAYÉ"), 12, 1, 60));
    }

    // --- D58 : les colonnes -------------------------------------------

    fn v(l: &[&str]) -> Vec<String> {
        l.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn d_office_les_quatre_colonnes_fixes_sans_tva_ni_remise() {
        for g in GENRES {
            let r = defaut(g, &rien);
            assert_eq!(r.colonnes, v(&["designation", "quantite", "prix_unitaire", "total"]), "{g}");
            assert!(!r.recap_tva, "{g} : récapitulatif décoché");
        }
    }

    #[test]
    fn les_colonnes_se_rangent_et_l_inconnue_ou_la_fixe_retiree_est_refusee() {
        let c = valider_colonnes(&v(&["tva", "total", "designation", "prix_unitaire", "quantite", "reference", "tva"])).unwrap();
        assert_eq!(c, v(&["reference", "designation", "quantite", "prix_unitaire", "tva", "total"]));
        assert!(valider_colonnes(&v(&["designation", "quantite", "prix_unitaire", "total", "marge"]))
            .unwrap_err().contains("inconnue"));
        let sans_prix = valider_colonnes(&v(&["designation", "quantite", "total"])).unwrap_err();
        assert!(sans_prix.contains("Prix unitaire"), "{sans_prix}");
        let mut r = defaut("facture", &rien);
        r.colonnes = v(&["designation", "total"]);
        assert!(valider("facture", &r).is_err(), "le serveur refuse, il ne complète pas");
    }

    #[test]
    fn un_ancien_auto_devient_decoche_et_un_oui_coche() {
        let auto = serde_json::json!({ "format": "a4", "colonne_remise": "auto", "colonne_tva": "auto", "recap_tva": "auto" });
        let r = fusionner("facture", Some(&auto), &rien);
        assert_eq!(r.colonnes, colonnes_defaut(), "auto = décoché, comme une base neuve");
        assert!(!r.recap_tva);

        let oui = serde_json::json!({
            "format": "a4", "colonne_remise": "oui", "colonne_tva": "oui",
            "recap_tva": "oui", "reference_article": true
        });
        let r = fusionner("devis", Some(&oui), &rien);
        assert_eq!(r.colonnes, v(&["reference", "designation", "quantite", "prix_unitaire", "remise", "tva", "total"]));
        assert!(r.recap_tva);

        let non = serde_json::json!({ "format": "a4", "colonne_remise": "non", "colonne_tva": "non", "recap_tva": "non" });
        assert_eq!(fusionner("facture", Some(&non), &rien).colonnes, colonnes_defaut());
    }

    #[test]
    fn la_migration_ne_touche_pas_un_reglage_deja_en_colonnes() {
        let neuf = serde_json::json!({ "format": "a4", "colonnes": ["designation", "quantite", "prix_unitaire", "tva", "total"],
                                       "recap_tva": true, "colonne_tva": "non" });
        let m = migrer(&neuf);
        assert_eq!(m["colonnes"], neuf["colonnes"], "le choix d'aujourd'hui l'emporte sur un ancien champ");
        assert!(m.get("colonne_tva").is_none());
        assert_eq!(m["recap_tva"], true);
        let r = fusionner("facture", Some(&neuf), &rien);
        assert!(r.colonnes.contains(&"tva".to_string()) && r.recap_tva);
    }

    #[test]
    fn une_liste_gardee_abimee_suit_le_genre() {
        assert_eq!(colonnes_gardees(None), None);
        assert_eq!(colonnes_gardees(Some("pas du json")), None);
        assert_eq!(colonnes_gardees(Some(r#"["designation","total"]"#)), None, "fixe manquante");
        assert_eq!(
            colonnes_gardees(Some(r#"["tva","designation","quantite","prix_unitaire","total"]"#)),
            Some(v(&["designation", "quantite", "prix_unitaire", "tva", "total"]))
        );
    }

    #[test]
    fn seuls_les_genres_a_lignes_ont_des_colonnes() {
        for g in GENRES_A_COLONNES {
            assert!(GENRES.contains(&g));
        }
        for t in ["facture", "avoir_client", "devis", "proforma", "commande_client", "bon_livraison",
                  "bon_reception", "bon_commande_fournisseur", "facture_fournisseur"] {
            assert!(GENRES_A_COLONNES.contains(&genre_de_piece(t)), "{t}");
        }
        assert!(!GENRES_A_COLONNES.contains(&"ticket"));
    }
}
