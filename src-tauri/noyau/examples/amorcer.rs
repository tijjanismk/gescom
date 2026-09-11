//! Prepare une base : schema, roles, comptes, et la demo si on la demande.
//!
//!   cargo run -p gescom-noyau --example amorcer -- <cible> [--demo]
//!
//! `cible` est un chemin de fichier (SQLite) ou une URL postgresql://.
//! C'est l'outil de mise en route d'un poste principal : le serveur le
//! fait aussi au demarrage, mais on veut parfois preparer la base AVANT
//! de lancer quoi que ce soit.

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(cible) = args.next() else {
        eprintln!("usage : amorcer <chemin|postgresql://...> [--demo]");
        std::process::exit(2);
    };
    let demo = args.any(|a| a == "--demo");

    let mut base = match gescom_noyau::base::Base::ouvrir(&cible) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Impossible d'ouvrir « {cible} » : {e}");
            std::process::exit(1);
        }
    };
    println!("moteur : {}", base.moteur());

    match gescom_noyau::amorcage::amorcer(&mut base) {
        Ok(true) => {
            println!("base amorcée — comptes créés :");
            for (pseudo, mdp, role) in gescom_noyau::amorcage::COMPTES_USINE {
                println!("  {pseudo:8} / {mdp:12} ({role})");
            }
            println!("  Les deux exigent un changement à la première connexion.");
        }
        Ok(false) => println!("base déjà amorcée — rien à faire."),
        Err(e) => {
            eprintln!("Amorçage impossible : {e}");
            std::process::exit(1);
        }
    }

    if demo {
        match gescom_noyau::amorcage::donnees_demo(&mut base) {
            Ok((a, c)) => println!("démo : {a} articles, {c} clients."),
            Err(e) => eprintln!("Démo impossible : {e}"),
        }
    }
}
