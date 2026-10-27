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

/// Le « Fournisseur divers » ne sert qu'aux achats comptant sans
/// fournisseur : pas de piece saisie a la main, pas de reglement de
/// dette, pas de modification de sa fiche. Sinon la dette envers
/// personne revient par une autre porte. `est_divers` vient de la base ;
/// `geste` nomme ce qu'on refuse, pour le message. Pure.
pub fn refuser_si_fournisseur_divers(est_divers: bool, geste: &str) -> Result<(), String> {
    if est_divers {
        Err(format!(
            "Le « {FOURNISSEUR_DIVERS} » ne sert qu'aux achats comptant sans fournisseur : \
             {geste} impossible. Pour cela, choisir ou créer le vrai fournisseur."
        ))
    } else {
        Ok(())
    }
}

/// Un retour au « Fournisseur divers » se rembourse : un avoir sur lui
/// ne se consommerait jamais, puisqu'on ne lui achete pas a credit. Pure.
pub fn verifier_retour_fournisseur_divers(est_divers: bool, mode_resolution: Option<&str>) -> Result<(), String> {
    if est_divers && mode_resolution != Some("remboursement") {
        Err(format!(
            "Un retour au « {FOURNISSEUR_DIVERS} » se rembourse : un avoir sur lui ne \
             servirait jamais (on ne lui achète pas à crédit)."
        ))
    } else {
        Ok(())
    }
}

/// Ce qu'il faut savoir d'une fiche client pour juger un rattachement
/// (lot D.10). `passage` : `statut = 'passage'` (D35).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FicheARattacher {
    pub id: String,
    pub est_generique: bool,
    pub passage: bool,
    pub actif: bool,
}

/// Lot D.10 — « Rattacher a un client » : le patron choisit la bonne
/// fiche cliente pour un passant revenu, et toute l'histoire de la fiche
/// de passage y passe. Jamais automatique : un passant n'a ni nom ni
/// telephone sur lesquels s'appuyer. Pure ; les fiches sont lues DANS le
/// dossier ouvert (`None` : pas dans ce dossier).
///
/// - la source est une fiche de passage active (pas le « Comptant », qui
///   porte toutes les ventes sans nom d'avant D35, ni un client
///   ordinaire, ni une fiche deja rattachee) ;
/// - la cible est un client ordinaire actif : ni le « Comptant », ni une
///   fiche de passage, ni la fiche elle-meme.
pub fn juger_rattachement(
    passage: Option<&FicheARattacher>,
    cible: Option<&FicheARattacher>,
) -> Result<(), String> {
    let passage = passage.ok_or_else(|| "Fiche de passage introuvable dans ce dossier.".to_string())?;
    if !passage.passage || !passage.est_generique {
        return Err("Seule une fiche de client de passage (PAS-…) se rattache à un client.".to_string());
    }
    if !passage.actif {
        return Err("Cette fiche de passage est déjà rattachée à un client.".to_string());
    }
    let cible = cible.ok_or_else(|| "Client introuvable dans ce dossier.".to_string())?;
    if cible.id == passage.id {
        return Err("Une fiche ne se rattache pas à elle-même.".to_string());
    }
    if cible.est_generique || cible.passage {
        return Err("Rattacher à un client ordinaire : ni le « Comptant », ni une autre fiche de passage.".to_string());
    }
    if !cible.actif {
        return Err("Ce client est désactivé : le réactiver d'abord.".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fiche(id: &str, est_generique: bool, passage: bool, actif: bool) -> FicheARattacher {
        FicheARattacher { id: id.into(), est_generique, passage, actif }
    }

    #[test]
    fn un_passant_se_rattache_a_un_client_ordinaire() {
        let pas = fiche("p", true, true, true);
        let awa = fiche("a", false, false, true);
        assert!(juger_rattachement(Some(&pas), Some(&awa)).is_ok());
    }

    #[test]
    fn la_source_est_une_fiche_de_passage_active() {
        let awa = fiche("a", false, false, true);
        assert!(juger_rattachement(None, Some(&awa)).unwrap_err().contains("introuvable"));
        // Le « Comptant » : generique, pas une fiche de passage.
        let comptant = fiche("c", true, false, true);
        assert!(juger_rattachement(Some(&comptant), Some(&awa)).unwrap_err().contains("passage"));
        // Un client ordinaire ne se rattache pas (ce serait une fusion).
        let moussa = fiche("m", false, false, true);
        assert!(juger_rattachement(Some(&moussa), Some(&awa)).is_err());
        // Deja rattachee : desactivee.
        let deja = fiche("p", true, true, false);
        assert!(juger_rattachement(Some(&deja), Some(&awa)).unwrap_err().contains("déjà"));
    }

    #[test]
    fn la_cible_est_un_client_ordinaire_actif_et_pas_elle_meme() {
        let pas = fiche("p", true, true, true);
        assert!(juger_rattachement(Some(&pas), None).unwrap_err().contains("dossier"));
        assert!(juger_rattachement(Some(&pas), Some(&pas.clone())).unwrap_err().contains("elle-même"));
        let comptant = fiche("c", true, false, true);
        assert!(juger_rattachement(Some(&pas), Some(&comptant)).unwrap_err().contains("ordinaire"));
        let autre_passage = fiche("q", true, true, true);
        assert!(juger_rattachement(Some(&pas), Some(&autre_passage)).unwrap_err().contains("ordinaire"));
        let inactif = fiche("a", false, false, false);
        assert!(juger_rattachement(Some(&pas), Some(&inactif)).unwrap_err().contains("désactivé"));
    }

    #[test]
    fn le_fournisseur_divers_ne_recoit_que_des_achats_comptant() {
        assert!(refuser_si_fournisseur_divers(false, "une pièce").is_ok());
        let refus = refuser_si_fournisseur_divers(true, "une pièce saisie à la main").unwrap_err();
        assert!(refus.contains("Fournisseur divers") && refus.contains("pièce saisie"), "{refus}");
        assert!(verifier_retour_fournisseur_divers(true, Some("remboursement")).is_ok());
        assert!(verifier_retour_fournisseur_divers(false, Some("avoir")).is_ok());
        assert!(verifier_retour_fournisseur_divers(true, Some("avoir")).unwrap_err().contains("rembourse"));
        assert!(verifier_retour_fournisseur_divers(true, None).is_err(), "le mode par défaut est l'avoir");
    }

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
