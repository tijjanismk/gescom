//! Les portes : qui a le droit de faire quoi.
//!
//! Ce fichier existait en v1 mais n'etait declare dans aucun `mod` —
//! il ne compilait pas, et referencait un type `ErreurPermission`
//! inexistant. Le controle reel se faisait au coup par coup dans les
//! commandes, quand il se faisait. En monoposte, c'etait tolerable :
//! une seule personne devant l'ecran. Des qu'un poste caisse parle au
//! serveur par le reseau, ca ne l'est plus.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErreurPermission {
    /// Le role existe mais ne couvre pas cette action.
    Refuse { role: String, permission: String },
}

impl fmt::Display for ErreurPermission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErreurPermission::Refuse { role, permission } => write!(
                f,
                "PERMISSION_REFUSEE — le rôle « {role} » ne permet pas « {permission} »."
            ),
        }
    }
}

pub struct ContexteUtilisateur {
    pub id: String,
    pub role: String,
}

/// Le patron peut tout. L'employe vend, encaisse et lit le stock. Le
/// role lecture ne fait que lire.
///
/// Une liste blanche, jamais une liste noire : une commande ajoutee
/// demain est refusee par defaut aux roles restreints. L'inverse
/// ouvrirait chaque nouveaute a tout le monde sans que personne ne le
/// remarque.
pub fn verifier_permission(
    contexte: &ContexteUtilisateur,
    permission: &str,
) -> Result<(), ErreurPermission> {
    let autorise = match contexte.role.as_str() {
        "patron" => true,
        "employe" => matches!(
            permission,
            "ventes:creer"
                | "ventes:lire"
                | "paiements:creer"
                | "clients:creer"
                | "articles:creer"
                | "clients:lire"
                | "stock:lire"
                | "caisse:ouvrir"
                | "caisse:mouvementer"
                | "pieces:creer"
                | "pieces:lire"
                | "retours:creer"
        ),
        "lecture" => permission.ends_with(":lire"),
        _ => false,
    };

    if autorise {
        Ok(())
    } else {
        Err(ErreurPermission::Refuse {
            role: contexte.role.clone(),
            permission: permission.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(role: &str) -> ContexteUtilisateur {
        ContexteUtilisateur { id: "u".into(), role: role.into() }
    }

    #[test]
    fn le_patron_peut_tout() {
        assert!(verifier_permission(&ctx("patron"), "caisse:cloturer").is_ok());
    }

    #[test]
    fn l_employe_ne_cloture_pas() {
        assert!(verifier_permission(&ctx("employe"), "caisse:cloturer").is_err());
    }

    #[test]
    fn la_lecture_ne_vend_pas() {
        assert!(verifier_permission(&ctx("lecture"), "ventes:creer").is_err());
        assert!(verifier_permission(&ctx("lecture"), "ventes:lire").is_ok());
    }

    /// Une permission inventee doit etre refusee, pas acceptee par
    /// defaut. C'est ce test qui garde la liste blanche blanche.
    #[test]
    fn une_permission_inconnue_est_refusee() {
        assert!(verifier_permission(&ctx("employe"), "compta:cloturer").is_err());
    }
}
