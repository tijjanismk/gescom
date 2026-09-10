//! Le contrôle d'installation, joué là où il compte : depuis un
//! exécutable qui n'est pas celui de l'installation.
//!
//! Le binaire de test vit dans `target/…/deps`. Que Gescom soit
//! installé sur la machine (le contrôle répond « déplacée ») ou non
//! (« non installée »), la réponse doit être la même : refus. C'est
//! exactement la situation d'un `Gescom.exe` posé sur une clé USB.

use gescom_noyau::installation::{verifier, Etat};

#[test]
fn un_executable_hors_installation_est_refuse() {
    let etat = verifier();
    assert!(
        !etat.autorise(),
        "un binaire lancé hors du dossier d'installation doit être refusé, \
         obtenu : {etat:?}"
    );
    assert!(!etat.message().is_empty(), "le refus doit dire quoi faire");
}

/// Ne vérifie rien : sert à LIRE le verdict sur un poste donné.
///
/// `cargo test -p gescom-noyau --test installation -- --nocapture`
#[test]
fn verdict_de_ce_poste() {
    let etat = verifier();
    println!("--- contrôle d'installation ---");
    println!("exécutable : {:?}", std::env::current_exe().ok());
    println!("verdict    : {etat:?}");
    println!("autorise   : {}", etat.autorise());
    if let Etat::Deplacee { attendu, reel } = &etat {
        println!("attendu    : {attendu}");
        println!("réel       : {reel}");
    }
}
