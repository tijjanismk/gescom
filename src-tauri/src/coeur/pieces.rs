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
        assert!(peut_modifier("bon_livraison", "emis").is_ok());
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
    fn message_utilisateur_mentionne_l_avoir() {
        // Refuser sans dire quoi faire est inutilisable au comptoir.
        let e = peut_modifier("facture", "emis").unwrap_err();
        assert!(e.to_lowercase().contains("avoir"));
        let e = peut_annuler("facture", "emis", true).unwrap_err();
        assert!(e.to_lowercase().contains("avoir"));
    }
}
