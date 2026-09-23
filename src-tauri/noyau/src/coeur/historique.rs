//! Le journal metier, dit en francais (v3, B-1).
//!
//! Le journal s'ecrit partout depuis la v1 (`vente_creee`,
//! `reglement_annule`…) ; il ne se lisait nulle part. Pour le lire, il
//! faut nommer chaque type d'evenement comme le commercant le dirait.
//! Un type inconnu reste lisible : on l'affiche tel quel, souligne.

/// (type, libelle) — l'ordre est celui du filtre a l'ecran.
pub const TYPES: &[(&str, &str)] = &[
    ("vente_creee", "Vente"),
    ("remise_accordee", "Remise accordée"),
    ("creance_reglee", "Règlement client"),
    ("reglement_annule", "Règlement annulé"),
    ("recouvrement_exceptionnel", "Recouvrement exceptionnel"),
    ("residu_absorbe", "Résidu absorbé"),
    ("creance_irrecouvrable", "Créance irrécouvrable"),
    ("avoir_applique", "Avoir appliqué"),
    ("avoir_accorde", "Avoir accordé"),
    ("avoir_rembourse", "Avoir remboursé"),
    ("avoir_reactive", "Avoir réactivé"),
    ("expiration_avoirs", "Avoirs expirés"),
    ("retour_enregistre", "Retour client"),
    ("retour_sans_facture", "Retour sans facture"),
    ("retour_fournisseur", "Retour fournisseur"),
    ("piece_convertie", "Pièce convertie"),
    ("piece_dupliquee", "Pièce dupliquée"),
    ("piece_annulee", "Pièce annulée"),
    ("facture_annulee_par_avoir", "Facture annulée par avoir"),
    ("commande_livree_facturee", "Commande livrée et facturée"),
    ("livraison_enregistree", "Livraison enregistrée"),
    ("achat_enregistre", "Achat"),
    ("facture_fournisseur_validee", "Facture fournisseur validée"),
    ("paiement_fournisseur", "Paiement fournisseur"),
    ("paiement_fournisseur_annule", "Paiement fournisseur annulé"),
    ("depense", "Dépense de caisse"),
    ("cheque_statut", "Chèque"),
    ("transfert", "Transfert entre magasins"),
    ("client_modifie", "Fiche client modifiée"),
    ("depot_desactive_avec_stock", "Magasin désactivé"),
    ("depot_reactive", "Magasin réactivé"),
    ("utilisateur_cree", "Utilisateur créé"),
    ("utilisateur_desactive", "Compte désactivé"),
    ("utilisateur_reactive", "Compte réactivé"),
    ("role_change", "Rôle changé"),
    ("plafonds_modifies", "Plafonds modifiés"),
    ("sauvegarde", "Sauvegarde"),
    ("entretien", "Entretien de la base"),
    ("anomalie", "Anomalie"),
];

pub fn libelle_type(t: &str) -> String {
    TYPES
        .iter()
        .find(|(k, _)| *k == t)
        .map(|(_, l)| l.to_string())
        .unwrap_or_else(|| t.replace('_', " "))
}

/// Une page de l'historique : au moins 1 ligne, au plus 200.
pub fn borner_page(par_page: i64) -> i64 {
    par_page.clamp(1, 200)
}

/// Un jour saisi (`AAAA-MM-JJ`), ou rien. Une date illisible ne filtre
/// pas en silence : elle est refusee.
pub fn jour_filtre(d: Option<&str>) -> Result<Option<String>, String> {
    match d.map(str::trim).filter(|d| !d.is_empty()) {
        None => Ok(None),
        Some(d) => crate::coeur::dates::jour(d)
            .map(|j| Some(j.format("%Y-%m-%d").to_string()))
            .ok_or_else(|| format!("Date illisible : « {d} »")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_type_connu_est_dit_en_francais() {
        assert_eq!(libelle_type("reglement_annule"), "Règlement annulé");
        assert_eq!(libelle_type("vente_creee"), "Vente");
    }

    #[test]
    fn un_type_inconnu_reste_lisible() {
        assert_eq!(libelle_type("chose_nouvelle"), "chose nouvelle");
    }

    #[test]
    fn les_types_sont_uniques() {
        let mut vus = std::collections::HashSet::new();
        for (t, _) in TYPES {
            assert!(vus.insert(*t), "doublon : {t}");
        }
    }

    #[test]
    fn une_page_reste_raisonnable() {
        assert_eq!(borner_page(0), 1);
        assert_eq!(borner_page(50), 50);
        assert_eq!(borner_page(10_000), 200);
    }

    #[test]
    fn un_jour_se_lit_ou_se_refuse() {
        assert_eq!(jour_filtre(None).unwrap(), None);
        assert_eq!(jour_filtre(Some(" ")).unwrap(), None);
        assert_eq!(jour_filtre(Some("2026-09-03")).unwrap().as_deref(), Some("2026-09-03"));
        assert!(jour_filtre(Some("03/09/2026")).is_err());
    }
}
