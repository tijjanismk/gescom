//! Immuabilité des pièces commerciales — règles pures, sans I/O.
//!
//! Principe (OHADA / SYSCOHADA, applicable au Mali) : une pièce portant
//! un numéro dans une série séquentielle est IMMUABLE dès son émission.
//! Ce n'est pas le paiement qui la fige, c'est le fait qu'elle soit
//! sortie — remise au client, ou reçue du fournisseur.
//!
//! La numérotation sans trou et l'immuabilité vont ensemble : une série
//! dont les pièces restent modifiables ne prouve rien.
//!
//! Corriger une pièce émise se fait par un AVOIR, jamais par une
//! modification. C'est aussi la seule trace exploitable en cas de
//! contrôle ou de litige avec un client.

/// Une pièce est-elle « engageante » ?
///
/// Engageante = elle constate une créance, une dette ou une TVA. Elle
/// devient immuable dès qu'elle quitte le brouillon.
///
/// Non engageante = document de travail (devis, commande, bon de
/// livraison). Modifiable tant qu'il n'est ni transféré ni annulé.
pub fn est_engageante(type_piece: &str) -> bool {
    matches!(
        type_piece,
        "facture"
            | "facture_acompte"
            | "avoir_client"
            | "facture_fournisseur"
            | "avoir_fournisseur"
    )
}

/// Statuts qui ferment définitivement une pièce, quel que soit son type.
fn est_close(statut: &str) -> bool {
    matches!(statut, "validee" | "transfere" | "annule" | "paye")
}

/// Un BON constate un mouvement physique : le bon de livraison fait
/// sortir la marchandise, le bon de réception la fait entrer — au
/// moment où il est ÉMIS. Avant, en brouillon, on le prépare et rien
/// ne bouge ; après, il est figé, parce que le stock a bougé avec lui.
///
/// Modifier un bon émis laissait le stock tel quel : « 6 sacs » corrigé
/// en « 9 sacs » sur le papier, six sortis du magasin. C'est ce que
/// cette règle empêche.
pub fn constate_le_stock(type_piece: &str) -> bool {
    matches!(type_piece, "bon_livraison" | "bon_reception")
}

/// Un bon ne se facture qu'une fois ÉMIS : c'est l'émission qui fait
/// bouger le stock, et une facture issue d'un bon ne bouge plus rien
/// (`stock_confie_a_un_bon`). Facturer un bon en brouillon laisserait
/// la marchandise au magasin pour toujours, avec une facture qui dit
/// le contraire.
pub fn peut_transferer_un_bon(type_piece: &str, statut: &str) -> Result<(), String> {
    if constate_le_stock(type_piece) && statut == "brouillon" {
        return Err(
            "Bon en brouillon — l'émettre d'abord : c'est l'émission qui fait bouger la marchandise."
                .to_string(),
        );
    }
    Ok(())
}

/// Peut-on saisir une livraison ligne à ligne sur cette pièce ?
///
/// Un bon en brouillon se PRÉPARE : rien ne sort. C'est l'émission qui
/// livre tout d'un coup ; on corrige ensuite ligne à ligne (le livreur
/// revient avec deux sacs refusés). Livrer sur un brouillon, puis le
/// modifier — ce que le brouillon autorise — perdrait le lien entre ce
/// qui est sorti et ce que le bon dit.
pub fn peut_livrer(type_piece: &str, statut: &str) -> Result<(), String> {
    if constate_le_stock(type_piece) && statut == "brouillon" {
        return Err(
            "Bon en brouillon — l'émettre d'abord : l'émission livre tout, on corrige ensuite ligne à ligne."
                .to_string(),
        );
    }
    if est_close(statut) {
        return Err(format!("Pièce {statut} — sa livraison ne bouge plus."));
    }
    Ok(())
}

/// Ce que vaut un changement de statut demandé à la main.
///
/// `Ok(true)` : le changement ÉMET un bon — la marchandise bouge, le
/// bon naît entièrement livré. `Ok(false)` : un changement ordinaire.
/// `Err` : refusé, et pourquoi.
pub fn changement_de_statut(type_piece: &str, de: &str, vers: &str) -> Result<bool, String> {
    if de == vers {
        return Ok(false);
    }
    if est_close(de) {
        return Err(format!("Pièce {de} — son statut ne change plus."));
    }
    if constate_le_stock(type_piece) {
        if de == "brouillon" && vers == "emis" {
            return Ok(true);
        }
        if de == "emis" && vers == "brouillon" {
            return Err(
                "Un bon émis ne revient pas en brouillon : la marchandise a bougé avec lui. \
                 L'annuler (le stock revient), puis en refaire un."
                    .to_string(),
            );
        }
    }
    Ok(false)
}

/// La pièce peut-elle être modifiée ?
///
/// `Err(raison)` porte un message destiné à l'utilisateur.
pub fn peut_modifier(type_piece: &str, statut: &str) -> Result<(), String> {
    if est_close(statut) {
        return Err(match statut {
            "validee" => "Pièce validée — non modifiable.".into(),
            "transfere" => "Pièce transférée — non modifiable.".into(),
            "annule" => "Pièce annulée — non modifiable.".into(),
            _ => "Pièce payée — non modifiable. Émettre un avoir.".to_string(),
        });
    }

    // Une pièce engageante n'est modifiable qu'en brouillon.
    if est_engageante(type_piece) && statut != "brouillon" {
        return Err(
            "Pièce émise et numérotée — non modifiable. \
             Pour corriger, émettre un avoir."
                .to_string(),
        );
    }

    // Un bon émis a fait bouger le stock : le corriger sur le papier
    // laisserait le magasin à l'ancienne quantité.
    if constate_le_stock(type_piece) && statut != "brouillon" {
        return Err(
            "Bon émis — la marchandise a bougé avec lui, il ne se modifie plus. \
             L'annuler (le stock revient), puis en refaire un."
                .to_string(),
        );
    }

    Ok(())
}

/// La pièce peut-elle produire la pièce suivante ?
///
/// Une pièce ne se transfère qu'UNE fois : c'est ce qui empêche une même
/// commande d'être facturée deux fois, donc facturée en double au client.
///
/// Deux verrous plutôt qu'un, parce qu'ils ne tombent pas ensemble :
///   - le statut `transfere`, posé sur la source au moment du transfert ;
///   - l'existence d'une pièce déjà issue de celle-ci.
///
/// Le second rattrape ce que le premier laisse passer. Une commande
/// convertie d'un coup en BL + facture ne marquait que la commande ; le
/// bon de livraison, lui, restait « émis » alors qu'une facture en
/// découlait déjà — il pouvait donc être refacturé.
///
/// `descendant` : numéro d'une pièce déjà issue de celle-ci et non
/// annulée. Le numéro sert au message : « déjà facturée » sans dire quelle
/// facture oblige à fouiller la liste.
pub fn peut_transferer(statut_src: &str, descendant: Option<&str>) -> Result<(), String> {
    match statut_src {
        "annule" => {
            return Err(
                "Pièce annulée — elle ne peut plus rien produire.".to_string()
            )
        }
        "transfere" => {
            return Err(match descendant {
                Some(n) => format!(
                    "Pièce déjà transférée : elle a produit {}. \
                     Pour repartir de zéro, annuler d'abord {}.",
                    n, n
                ),
                None => "Cette pièce a déjà été transférée.".to_string(),
            })
        }
        _ => {}
    }

    if let Some(n) = descendant {
        return Err(format!(
            "Cette pièce a déjà produit {} — la transférer une seconde fois \
             ferait un doublon. Pour recommencer, annuler d'abord {}.",
            n, n
        ));
    }

    Ok(())
}

/// La pièce peut-elle être annulée ?
///
/// `a_produit_effets` : la pièce a généré une vente, un paiement ou un
/// mouvement de stock. Dans ce cas l'annulation laisserait des écritures
/// orphelines — il faut passer par un avoir.
pub fn peut_annuler(
    type_piece: &str,
    statut: &str,
    a_produit_effets: bool,
) -> Result<(), String> {
    match statut {
        "annule" => return Err("Pièce déjà annulée.".to_string()),
        "validee" => {
            return Err(
                "Pièce validée — impossible d'annuler. Émettre un avoir.".to_string(),
            )
        }
        "transfere" => {
            return Err(
                "Pièce transférée — annuler d'abord la pièce qui en découle."
                    .to_string(),
            )
        }
        "paye" => {
            return Err(
                "Pièce réglée — impossible d'annuler. Émettre un avoir.".to_string(),
            )
        }
        _ => {}
    }

    if a_produit_effets {
        return Err(
            "Cette pièce a déjà produit des écritures (vente, paiement \
             ou mouvement de stock). Émettre un avoir."
                .to_string(),
        );
    }

    // Une facture émise sans effet peut encore être annulée : le numéro
    // reste consommé, la série garde sa trace. C'est la pratique
    // courante, préférable à un trou dans la numérotation.
    let _ = type_piece;
    Ok(())
}

// =====================================================================
//  TESTS
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brouillon_toujours_modifiable() {
        assert!(peut_modifier("facture", "brouillon").is_ok());
        assert!(peut_modifier("devis", "brouillon").is_ok());
        assert!(peut_modifier("facture_fournisseur", "brouillon").is_ok());
    }

    #[test]
    fn facture_emise_non_modifiable() {
        // Le coeur de la regle : l'emission fige, pas le paiement.
        assert!(peut_modifier("facture", "emis").is_err());
        assert!(peut_modifier("facture_fournisseur", "emis").is_err());
        assert!(peut_modifier("avoir_client", "emis").is_err());
    }

    #[test]
    fn facture_payee_non_modifiable() {
        // C'etait le trou : 'paye' n'etait dans aucune garde.
        assert!(peut_modifier("facture_fournisseur", "paye").is_err());
        assert!(peut_modifier("facture", "paye").is_err());
    }

    #[test]
    fn devis_modifiable_meme_emis() {
        // Un devis n'engage rien tant qu'il n'est pas accepte.
        assert!(peut_modifier("devis", "emis").is_ok());
        assert!(peut_modifier("proforma", "emis").is_ok());
        assert!(peut_modifier("commande_client", "accepte").is_ok());
        // Un bon de livraison emis, lui, a fait sortir la marchandise :
        // il ne se modifie plus (voir tests_bons).
        assert!(peut_modifier("bon_livraison", "brouillon").is_ok());
        assert!(peut_modifier("bon_livraison", "emis").is_err());
    }

    #[test]
    fn statuts_clos_bloquent_tous_les_types() {
        for t in ["devis", "facture", "bon_reception"] {
            assert!(peut_modifier(t, "validee").is_err());
            assert!(peut_modifier(t, "transfere").is_err());
            assert!(peut_modifier(t, "annule").is_err());
        }
    }

    #[test]
    fn annulation_sans_effet_autorisee() {
        assert!(peut_annuler("devis", "emis", false).is_ok());
        assert!(peut_annuler("facture", "brouillon", false).is_ok());
        assert!(peut_annuler("facture_fournisseur", "emis", false).is_ok());
    }

    #[test]
    fn annulation_avec_effets_refusee() {
        assert!(peut_annuler("facture", "emis", true).is_err());
        assert!(peut_annuler("facture_fournisseur", "emis", true).is_err());
    }

    #[test]
    fn annulation_piece_payee_refusee() {
        assert!(peut_annuler("facture_fournisseur", "paye", false).is_err());
        assert!(peut_annuler("facture", "validee", false).is_err());
    }

    #[test]
    fn transfert_normal_autorise() {
        assert!(peut_transferer("emis", None).is_ok());
        assert!(peut_transferer("brouillon", None).is_ok());
        assert!(peut_transferer("accepte", None).is_ok());
    }

    #[test]
    fn deuxieme_transfert_refuse_par_le_statut() {
        assert!(peut_transferer("transfere", None).is_err());
    }

    #[test]
    fn deuxieme_transfert_refuse_par_le_descendant() {
        // Le cas reel : commande -> BL + facture d'un coup. Seule la
        // commande passait en 'transfere' ; le BL restait 'emis' avec une
        // facture deja nee de lui, donc refacturable.
        let e = peut_transferer("emis", Some("FAC-2026-00042")).unwrap_err();
        assert!(e.contains("FAC-2026-00042"));
        assert!(e.to_lowercase().contains("doublon"));
    }

    #[test]
    fn piece_annulee_ne_produit_rien() {
        assert!(peut_transferer("annule", None).is_err());
    }

    #[test]
    fn descendant_annule_libere_la_source() {
        // L'appelant ne passe QUE les descendants non annules : une
        // facture annulee ne doit pas condamner sa commande pour toujours.
        assert!(peut_transferer("emis", None).is_ok());
    }

    #[test]
    fn refus_de_transfert_nomme_la_piece_a_annuler() {
        // Refuser sans dire par ou sortir bloque l'utilisateur au comptoir.
        let e = peut_transferer("transfere", Some("BL-2026-00007")).unwrap_err();
        assert!(e.contains("BL-2026-00007"));
        assert!(e.to_lowercase().contains("annuler"));
    }

    #[test]
    fn message_utilisateur_mentionne_l_avoir() {
        // Refuser sans dire quoi faire est inutilisable au comptoir.
        let e = peut_modifier("facture", "emis").unwrap_err();
        assert!(e.to_lowercase().contains("avoir"));
        let e = peut_annuler("facture", "emis", true).unwrap_err();
        assert!(e.to_lowercase().contains("avoir"));
    }
}

#[cfg(test)]
mod tests_bons {
    use super::*;

    #[test]
    fn un_bon_se_modifie_en_brouillon_plus_une_fois_emis() {
        assert!(peut_modifier("bon_livraison", "brouillon").is_ok());
        assert!(peut_modifier("bon_reception", "brouillon").is_ok());
        let e = peut_modifier("bon_livraison", "emis").unwrap_err();
        assert!(e.contains("bougé"), "{e}");
        assert!(peut_modifier("bon_reception", "emis").is_err());
        // Un devis émis reste modifiable : il ne constate rien.
        assert!(peut_modifier("devis", "emis").is_ok());
    }

    #[test]
    fn emettre_un_bon_constate_le_stock_et_le_retour_en_brouillon_est_refuse() {
        assert_eq!(changement_de_statut("bon_livraison", "brouillon", "emis"), Ok(true));
        assert_eq!(changement_de_statut("bon_reception", "brouillon", "emis"), Ok(true));
        assert!(changement_de_statut("bon_livraison", "emis", "brouillon").is_err());
        // Les autres pièces changent de statut sans rien constater.
        assert_eq!(changement_de_statut("devis", "brouillon", "emis"), Ok(false));
        assert_eq!(changement_de_statut("devis", "emis", "accepte"), Ok(false));
        assert!(changement_de_statut("devis", "annule", "emis").is_err());
        assert_eq!(changement_de_statut("bon_livraison", "emis", "emis"), Ok(false));
    }

    #[test]
    fn on_ne_livre_pas_sur_un_bon_en_brouillon_ni_sur_une_piece_close() {
        assert!(peut_livrer("bon_livraison", "brouillon").is_err());
        assert!(peut_livrer("bon_livraison", "emis").is_ok());
        assert!(peut_livrer("bon_reception", "emis").is_ok());
        assert!(peut_livrer("bon_livraison", "annule").is_err());
        // Une commande n'est pas un bon : sa livraison n'est qu'un suivi.
        assert!(peut_livrer("commande_client", "brouillon").is_ok());
    }

    #[test]
    fn un_bon_en_brouillon_ne_se_facture_pas_avant_d_etre_emis() {
        assert!(peut_transferer_un_bon("bon_livraison", "brouillon").is_err());
        assert!(peut_transferer_un_bon("bon_reception", "brouillon").is_err());
        assert!(peut_transferer_un_bon("bon_livraison", "emis").is_ok());
        assert!(peut_transferer_un_bon("commande_client", "brouillon").is_ok());
    }
}

