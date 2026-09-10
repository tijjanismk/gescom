//! Le contrôle d'installation, au tout premier instant.
//!
//! Il tourne AVANT que Tauri ne construise quoi que ce soit : pas de
//! fenêtre, pas de base ouverte, pas de plugin chargé. Une copie posée
//! sur une clé USB doit s'arrêter là, sans avoir touché au disque.
//!
//! Le message passe par `MessageBoxW` de Windows et non par la boîte de
//! dialogue de Tauri : celle-ci a besoin de la boucle d'événements, qui
//! n'a pas encore démarré. Un refus silencieux serait pire que tout —
//! l'utilisateur double-clique, rien ne se passe, et il appelle en
//! croyant ses données perdues.

use gescom_noyau::installation;

/// Arrête le programme si l'exécutable n'a pas été installé ici.
///
/// Inactif en build de débogage : sans cela, `cargo tauri dev` ne
/// démarrerait plus, puisqu'aucun installateur n'est jamais passé.
pub fn exiger_installation() {
    #[cfg(debug_assertions)]
    {
        // En développement, on veut juste savoir ce que le contrôle
        // aurait décidé.
        let etat = installation::verifier();
        if !etat.autorise() {
            eprintln!("[garde] build de débogage — passage forcé : {etat:?}");
        }
        return;
    }

    #[cfg(not(debug_assertions))]
    {
        let etat = installation::verifier();
        if etat.autorise() {
            return;
        }
        afficher(&etat.message());
        std::process::exit(1);
    }
}

#[cfg(all(windows, not(debug_assertions)))]
fn afficher(texte: &str) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        MessageBoxW, MB_ICONWARNING, MB_OK, MB_SETFOREGROUND, MB_TOPMOST,
    };

    // UTF-16 terminé par zéro : `MessageBoxW` lit jusqu'au zéro, et
    // l'oublier ferait afficher la mémoire qui suit.
    let en_utf16 = |s: &str| -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    };
    let corps = en_utf16(texte);
    let titre = en_utf16("Gescom");

    // MB_TOPMOST : l'application n'a pas de fenêtre, la boîte se
    // retrouverait sinon derrière tout le reste et l'utilisateur
    // croirait qu'il ne s'est rien passé.
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            corps.as_ptr(),
            titre.as_ptr(),
            MB_OK | MB_ICONWARNING | MB_SETFOREGROUND | MB_TOPMOST,
        );
    }
}

#[cfg(all(not(windows), not(debug_assertions)))]
fn afficher(texte: &str) {
    eprintln!("{texte}");
}
