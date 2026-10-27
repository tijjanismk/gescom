//! La plage de ports du serveur (D39) et ce que la fenetre livree peut
//! joindre doivent dire la meme chose. La CSP des deux fenetres
//! (`tauri.conf.json`) enumere les ports un par un — une CSP ne connait
//! pas les plages : un port ajoute a `protocole` sans etre ajoute ici,
//! et le poste qui y bascule ne joint plus personne, sans un mot.

use gescom_noyau::protocole::{ports_de_la_plage, PORT_DEFAUT, PORT_MAX};

fn connect_src(chemin: &str) -> Vec<String> {
    let texte = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(chemin))
        .unwrap_or_else(|e| panic!("{chemin} : {e}"));
    let v: serde_json::Value = serde_json::from_str(&texte).expect("JSON");
    let csp = v["app"]["security"]["csp"].as_str().expect("csp").to_string();
    let directive = csp
        .split(';')
        .map(str::trim)
        .find(|d| d.starts_with("connect-src"))
        .expect("connect-src")
        .to_string();
    directive.split_whitespace().skip(1).map(str::to_string).collect()
}

#[test]
fn chaque_fenetre_joint_toute_la_plage_et_rien_d_autre() {
    let attendus: Vec<String> = ports_de_la_plage().map(|p| format!("http://*:{p}")).collect();
    // La fenetre de caisse, Gescom Equipe, et Gescom Serveur (D25 : sa page
    // locale ne joint rien elle-meme, mais sa CSP suit la meme regle).
    for chemin in ["../tauri.conf.json", "../equipe/tauri.conf.json", "../serveur-fenetre/tauri.conf.json"] {
        let sources = connect_src(chemin);
        for a in &attendus {
            assert!(sources.contains(a), "{chemin} : {a} manque a connect-src");
        }
        let autres: Vec<&String> = sources
            .iter()
            .filter(|s| !attendus.contains(s))
            .filter(|s| !["'self'", "ipc:", "http://ipc.localhost"].contains(&s.as_str()))
            .collect();
        assert!(autres.is_empty(), "{chemin} : connect-src ouvre plus que la plage : {autres:?}");
    }
}

/// Gescom Equipe et Gescom Serveur (la fenetre) ne dependent pas du
/// noyau : elles recopient la plage. La copie doit suivre.
#[test]
fn equipe_recopie_la_meme_plage() {
    for chemin in ["../equipe/src/main.rs", "../serveur-fenetre/src/main.rs"] {
        let texte = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(chemin))
            .unwrap_or_else(|e| panic!("{chemin} : {e}"));
        assert!(texte.contains(&format!("const PORT_DEFAUT: u16 = {PORT_DEFAUT};")), "PORT_DEFAUT de {chemin}");
        assert!(texte.contains(&format!("const PORT_MAX: u16 = {PORT_MAX};")), "PORT_MAX de {chemin}");
    }
}

/// La recherche du serveur (UDP) ecoute le premier port de la plage, quel
/// que soit le port TCP du serveur : une caisse le trouve sans le connaitre.
#[test]
fn la_recherche_ecoute_le_premier_port_de_la_plage() {
    assert_eq!(gescom_noyau::decouverte::PORT, PORT_DEFAUT);
}
