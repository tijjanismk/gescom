//! D37 — ce que le menu peut cacher (regle pure, testee).
//!
//! Cacher une entree n'est qu'un affichage : aucune donnee, aucune
//! regle, aucune trace ne change. Quatre entrees ne se cachent jamais :
//! sans Caisse on n'encaisse plus, sans Parametres on ne reaffiche plus
//! rien — le poste serait coince.

/// Les entrees qu'on peut cacher : (`href` de l'ecran, nom du menu),
/// dans l'ordre du menu.
pub const MASQUABLES: &[(&str, &str)] = &[
    ("achats", "Achats"),
    ("pieces", "Pièces"),
    ("stock", "Stock"),
    ("clients", "Clients"),
    ("fournisseurs", "Fournisseurs"),
    ("retours", "Retours"),
    ("transferts", "Transferts"),
    ("cheques", "Chèques"),
    ("relances", "Relances"),
    ("journal", "Journal"),
    ("historique", "Historique"),
    ("rapports", "Rapports"),
];

/// Toujours visibles, sans case.
pub const TOUJOURS_VISIBLES: &[(&str, &str)] = &[
    ("dashboard", "Tableau de bord"),
    ("ventes", "Ventes"),
    ("caisse", "Caisse"),
    ("parametres", "Paramètres"),
];

/// « Boutique simple » : ce qui RESTE visible en plus des quatre
/// toujours visibles.
pub const BOUTIQUE_SIMPLE: &[&str] = &["achats", "stock", "clients", "rapports"];

/// Les entrees cachees par « Boutique simple ».
pub fn caches_boutique_simple() -> Vec<String> {
    MASQUABLES
        .iter()
        .map(|(h, _)| *h)
        .filter(|h| !BOUTIQUE_SIMPLE.contains(h))
        .map(str::to_string)
        .collect()
}

/// Une liste d'entrees a cacher, jugee : rien de toujours visible, rien
/// d'inconnu ; sans doublon, dans l'ordre du menu.
pub fn valider_caches(caches: &[String]) -> Result<Vec<String>, String> {
    for c in caches {
        if let Some((_, nom)) = TOUJOURS_VISIBLES.iter().find(|(h, _)| h == c) {
            return Err(format!(
                "« {nom} » reste toujours dans le menu : sans lui, le poste serait coincé."
            ));
        }
        if !MASQUABLES.iter().any(|(h, _)| h == c) {
            return Err(format!("Entrée de menu inconnue : « {c} »."));
        }
    }
    Ok(MASQUABLES
        .iter()
        .map(|(h, _)| *h)
        .filter(|h| caches.iter().any(|c| c == h))
        .map(str::to_string)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(l: &[&str]) -> Vec<String> {
        l.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn on_cache_ce_qui_est_masquable_dans_l_ordre_du_menu() {
        assert_eq!(valider_caches(&v(&["relances", "cheques", "cheques"])).unwrap(), v(&["cheques", "relances"]));
        assert!(valider_caches(&[]).unwrap().is_empty());
    }

    #[test]
    fn caisse_et_parametres_ne_se_cachent_jamais() {
        for h in ["dashboard", "ventes", "caisse", "parametres"] {
            let e = valider_caches(&v(&[h])).unwrap_err();
            assert!(e.contains("toujours"), "{h} : {e}");
        }
    }

    #[test]
    fn une_entree_inconnue_est_refusee() {
        assert!(valider_caches(&v(&["pirate"])).unwrap_err().contains("inconnue"));
    }

    #[test]
    fn boutique_simple_garde_l_essentiel() {
        let c = caches_boutique_simple();
        for garde in ["achats", "stock", "clients", "rapports"] {
            assert!(!c.iter().any(|x| x == garde), "{garde}");
        }
        for cache in ["pieces", "fournisseurs", "retours", "transferts", "cheques", "relances", "journal", "historique"] {
            assert!(c.iter().any(|x| x == cache), "{cache}");
        }
        assert_eq!(valider_caches(&c).unwrap(), c);
    }
}
