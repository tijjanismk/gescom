//! Qui fait le geste en cours.
//!
//! ## Pourquoi ce module existe
//!
//! Le noyau recevait le ROLE de l'appelant, pas son identite, et
//! retrouvait l'auteur par `SELECT ... WHERE role = ? LIMIT 1` : le
//! premier caissier actif signait les ventes de tous les caissiers.
//! Journal, remises, ouverture de caisse, antidatage — tout etait
//! attribue a la mauvaise personne des que deux comptes partageaient
//! un role (revue du 23/09/2026, D26).
//!
//! Passer l'identifiant en argument aurait touche une centaine de
//! signatures, dans les deux versions de chaque commande. Le serveur
//! sert chaque requete sur son propre fil (`main.rs::boucle`) : il pose
//! ici l'utilisateur de la SESSION avant d'appeler la commande, et les
//! aides d'auteur (`argent::id_utilisateur_*`) le lisent en premier.
//!
//! Hors serveur (fenetre monoposte v1, tests), rien n'est pose : les
//! aides retombent sur leur ancien comportement.

use std::cell::RefCell;

thread_local! {
    static AUTEUR: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Retire l'auteur a la fin de la portee. Un fil reutilise ne doit
/// jamais signer au nom de la requete precedente.
#[must_use = "l'auteur est retire des que la garde tombe"]
pub struct Garde {
    precedent: Option<String>,
}

impl Drop for Garde {
    fn drop(&mut self) {
        let precedent = self.precedent.take();
        AUTEUR.with(|a| *a.borrow_mut() = precedent);
    }
}

/// Pose l'auteur des ecritures faites sur ce fil, jusqu'a la chute de
/// la garde. Un identifiant vide ne pose rien.
pub fn poser(utilisateur_id: &str) -> Garde {
    let nouveau = Some(utilisateur_id.to_string()).filter(|s| !s.trim().is_empty());
    let precedent = AUTEUR.with(|a| std::mem::replace(&mut *a.borrow_mut(), nouveau));
    Garde { precedent }
}

/// L'auteur pose sur ce fil, s'il y en a un.
pub fn courant() -> Option<String> {
    AUTEUR.with(|a| a.borrow().clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_garde_retire_l_auteur() {
        assert_eq!(courant(), None);
        {
            let _g = poser("u-1");
            assert_eq!(courant().as_deref(), Some("u-1"));
        }
        assert_eq!(courant(), None);
    }

    #[test]
    fn les_gardes_s_emboitent() {
        let _a = poser("u-1");
        {
            let _b = poser("u-2");
            assert_eq!(courant().as_deref(), Some("u-2"));
        }
        assert_eq!(courant().as_deref(), Some("u-1"));
    }

    #[test]
    fn un_identifiant_vide_ne_pose_rien() {
        let _g = poser("  ");
        assert_eq!(courant(), None);
    }

    #[test]
    fn chaque_fil_a_le_sien() {
        let _g = poser("u-1");
        let autre = std::thread::spawn(courant).join().unwrap();
        assert_eq!(autre, None);
    }
}
