//! `gescom-serveur` — le poste qui detient la base.
//!
//! ## Le modele
//!
//! Un seul executable detient le fichier SQLite : celui-ci. Les postes
//! caisse ne touchent jamais au fichier, ils appellent des commandes.
//! C'est ce qui rend le multiposte sur : deux machines qui ouvriraient
//! la meme base par un partage Windows produiraient une corruption
//! silencieuse — le verrouillage SQLite ne traverse pas SMB de facon
//! fiable, et personne ne s'en apercoit avant la premiere restauration.
//!
//! En monoposte, ce meme service tourne dans le processus du client
//! (voir `reseau.rs` cote application) : un seul chemin de code, pas un
//! mode degrade qu'on testerait moins.
//!
//! ## Usage
//!
//! ```text
//! gescom-serveur [--port 7300] [--base CHEMIN] [--sauvegardes DOSSIER]
//! ```

mod api;
mod canal;
mod console;
mod etat;
mod http;
mod reseau_local;
mod sauvegarde;
mod socle;

use std::collections::HashMap;
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use gescom_noyau::protocole::PORT_DEFAUT;
use gescom_noyau::utils::maintenant_iso;
use gescom_noyau::{amorcage, persistance, postes, sessions};
use rusqlite::Connection;

use canal::Canal;
use etat::Serveur;

/// Delai de lecture d'une connexion.
///
/// Soixante secondes : au-dessus des trente de la longue attente du
/// canal, pour ne pas couper une attente legitime. Sans delai du tout,
/// un poste debranche en pleine requete laisserait un fil bloque pour
/// toujours.
const DELAI_LECTURE: Duration = Duration::from_secs(60);

fn main() {
    let options = Options::depuis_arguments();

    let cible = options.base.unwrap_or_else(chemin_base_par_defaut);
    // Meme detection que `Base::ouvrir` : une adresse PostgreSQL n'est
    // pas un chemin de fichier, et n'a pas de dossier parent a creer.
    let est_postgres = cible.starts_with("postgres://") || cible.starts_with("postgresql://");

    let mut amorcage_fait = false;

    // ---- La connexion SQLite brute, pour les commandes pas encore
    // portees (D11). `None` sur une cible PostgreSQL : voir etat.rs. ----
    let conn: Option<Connection> = if est_postgres {
        None
    } else {
        if let Some(parent) = std::path::Path::new(&cible).parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let conn = match persistance::ouvrir_base(&cible) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Impossible d'ouvrir la base « {cible} » : {e}");
                std::process::exit(1);
            }
        };
        if let Err(e) = persistance::initialiser_tables(&conn) {
            eprintln!("Impossible d'initialiser la base : {e}");
            std::process::exit(1);
        }

        // Une base neuve n'a ni role ni compte. Sans cet amorcage, le
        // serveur demarrait, ecoutait, et refusait toutes les connexions
        // avec « Identifiant ou mot de passe incorrect » — sans qu'aucun
        // ecran ne permette de creer le premier compte. Le seul remede
        // etait de lancer la fenetre une fois sur le meme fichier.
        match gescom_noyau::seed::amorcer_si_vide(&conn) {
            Ok(true) => amorcage_fait = true,
            Ok(false) => {}
            Err(e) => {
                eprintln!("Impossible d'amorcer la base : {e}");
                std::process::exit(1);
            }
        }

        // Une base abimee doit etre RESTAUREE, pas servie a cinq caisses
        // qui saisiront la journee par-dessus — ce qui rendrait ensuite
        // la restauration inutile.
        match persistance::verifier_integrite(&conn) {
            Ok(Some(probleme)) => {
                eprintln!("BASE ABIMEE : {probleme}");
                eprintln!("Restaurer une sauvegarde avant de redémarrer le serveur.");
                std::process::exit(2);
            }
            Err(e) => eprintln!("[avertissement] contrôle d'intégrité impossible : {e}"),
            Ok(None) => {}
        }

        // Les jetons d'avant le redemarrage ne sont plus verifiables :
        // la correspondance jeton -> session vit en memoire. Les
        // laisser « actives » en base ferait mentir la liste des postes
        // connectes.
        sessions::revoquer_toutes(&conn, "redemarrage_serveur").ok();

        // Le serveur est lui-meme un poste : ses propres operations
        // (une sauvegarde, une revocation) ont ainsi une origine
        // identifiable dans le journal, au lieu d'un champ vide.
        let empreinte = format!("serveur:{cible}");
        if let Err(e) = postes::inscrire_ou_retrouver(&conn, "Serveur", &empreinte, "serveur", None)
        {
            eprintln!("[avertissement] inscription du poste serveur : {e}");
        }

        Some(conn)
    };

    // ---- La Base, sur l'un ou l'autre moteur (D11) ----
    //
    // Sur une cible fichier, c'est une SECONDE connexion vers le MEME
    // fichier que `conn` — SQLite en WAL le permet, et c'est exactement
    // la transition que D11 decrit : une `Base` pour ce qui est porte,
    // une `Connection` pour le reste. Le schema est deja pret (la
    // premiere connexion vient de le poser) : pas la peine de rejouer
    // `amorcage::amorcer` par-dessus.
    //
    // Sur une adresse PostgreSQL, c'est le SEUL chemin : `amorcer` doit
    // tourner ici, et lui seul prepare la base.
    let mut base = match gescom_noyau::base::Base::ouvrir(&cible) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Impossible d'ouvrir la base « {cible} » : {e}");
            std::process::exit(1);
        }
    };
    if est_postgres {
        match amorcage::amorcer(&mut base) {
            Ok(true) => amorcage_fait = true,
            Ok(false) => {}
            Err(e) => {
                eprintln!("Impossible d'amorcer la base : {e}");
                std::process::exit(1);
            }
        }

        // L'authentification est portee (D11) : les memes precautions
        // qu'au demarrage du chemin fichier s'appliquent ici.
        gescom_noyau::sessions::revoquer_toutes_sur(&mut base, "redemarrage_serveur").ok();
        let empreinte = format!("serveur:{cible}");
        if let Err(e) =
            postes::inscrire_ou_retrouver_sur(&mut base, "Serveur", &empreinte, "serveur", None)
        {
            eprintln!("[avertissement] inscription du poste serveur : {e}");
        }
    }

    let srv = Arc::new(Serveur {
        conn: conn.map(Mutex::new),
        base: Mutex::new(base),
        chemin_base: cible.clone(),
        canal: Canal::nouveau(),
        registre: socle::registre(),
        jetons: Mutex::new(HashMap::new()),
        port: options.port,
        demarre_le: maintenant_iso(),
        derniere_sauvegarde: Mutex::new(None),
    });

    if let Some(d) = options.sauvegardes {
        if let Some(conn) = srv.conn.as_ref().and_then(|m| m.lock().ok()) {
            conn.execute(
                "INSERT INTO config_app (cle, valeur) VALUES ('dossier_sauvegarde', ?1)
                 ON CONFLICT(cle) DO UPDATE SET valeur = excluded.valeur",
                rusqlite::params![d],
            )
            .ok();
        }
    }
    // Sur les deux moteurs : VACUUM INTO pour SQLite, pg_dump pour
    // PostgreSQL (D4).
    sauvegarde::planifier(Arc::clone(&srv));

    let adresse = format!("{}:{}", options.hote, options.port);
    let ecouteur = match TcpListener::bind(&adresse) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Impossible d'écouter sur {adresse} : {e}");
            eprintln!("Un autre Gescom tourne-t-il déjà sur ce poste ?");
            std::process::exit(1);
        }
    };

    println!("Gescom serveur — protocole v{}", gescom_noyau::VERSION_PROTOCOLE);
    println!("  base        : {cible} ({})", if est_postgres { "PostgreSQL" } else { "SQLite" });
    println!("  écoute      : http://{adresse}");
    println!(
        "  sauvegardes : {} ({})",
        sauvegarde::dossier(&srv).display(),
        if est_postgres { "pg_dump" } else { "VACUUM INTO" }
    );
    println!("  commandes   : {}", srv.registre.len());

    // L'adresse a saisir sur les caisses, et l'etat du pare-feu.
    // Sans ces deux lignes, la premiere installation multiposte se
    // solde par un « Serveur injoignable » que rien n'explique.
    for ligne in reseau_local::conseils(options.port) {
        println!("{ligne}");
    }

    println!();
    if amorcage_fait {
        println!();
        println!("  BASE NEUVE — comptes créés :");
        println!("    admin   / admin123     (patron)");
        println!("    employe / employe123   (employé)");
        println!("    Les deux exigent un changement de mot de passe à la");
        println!("    première connexion.");
    }
    // D11 : le serveur tient une Base sur les deux moteurs, et
    // l'authentification (sessions, postes, permissions) est portee —
    // une caisse PEUT se connecter. Mais les 186 commandes du registre
    // restent sur `Connection` : dire ici ce qui marche et ce qui ne
    // marche pas encore vaut mieux qu'un silence qu'on decouvre commande
    // par commande.
    if est_postgres {
        println!();
        println!("  ⚠ PostgreSQL : une caisse peut se connecter (identifiants,");
        println!("    permissions, sessions — portés). Aucune des 186 commandes");
        println!("    de vente, stock, pièces ne répond encore : chacune refusera");
        println!("    avec « pas encore disponible sur PostgreSQL » (D11).");
    }
    println!("  Console : http://localhost:{}", options.port);
    println!("            a ouvrir dans un navigateur sur ce poste.");

    for flux in ecouteur.incoming() {
        match flux {
            Ok(flux) => {
                let srv = Arc::clone(&srv);
                // Un fil par connexion. SQLite serialise de toute facon
                // les ecritures derriere le verrou de `srv.conn` ; le
                // parallelisme sert ici a ne pas faire attendre une
                // caisse pendant qu'une autre imprime.
                std::thread::spawn(move || servir(&srv, flux));
            }
            Err(e) => eprintln!("[connexion refusée] {e}"),
        }
    }
}

fn servir(srv: &Arc<Serveur>, mut flux: TcpStream) {
    flux.set_read_timeout(Some(DELAI_LECTURE)).ok();
    flux.set_nodelay(true).ok();

    match http::lire_requete(&flux) {
        Ok(requete) => {
            if let Err(e) = api::traiter(srv, &requete, &mut flux) {
                eprintln!("[réponse interrompue] {e}");
            }
        }
        // Requete illisible : un scan de port, un poste coupe en plein
        // envoi. On ferme sans bruit plutot que de remplir le journal.
        Err(_) => {
            http::repondre_texte(&mut flux, 400, "Requête illisible").ok();
        }
    }
}

// =====================================================================
//  Options de ligne de commande
// =====================================================================

struct Options {
    hote: String,
    port: u16,
    base: Option<String>,
    sauvegardes: Option<String>,
}

impl Options {
    fn depuis_arguments() -> Options {
        let mut o = Options {
            // 0.0.0.0 et non 127.0.0.1 : sinon les autres postes du
            // magasin ne joignent pas le serveur, et le symptome
            // (« connexion refusée ») ne dit pas pourquoi.
            hote: "0.0.0.0".to_string(),
            port: PORT_DEFAUT,
            base: None,
            sauvegardes: None,
        };
        let args: Vec<String> = std::env::args().skip(1).collect();
        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--port" => {
                    if let Some(v) = args.get(i + 1).and_then(|v| v.parse().ok()) {
                        o.port = v;
                    }
                    i += 2;
                }
                "--hote" => {
                    if let Some(v) = args.get(i + 1) {
                        o.hote = v.clone();
                    }
                    i += 2;
                }
                "--base" => {
                    o.base = args.get(i + 1).cloned();
                    i += 2;
                }
                "--sauvegardes" => {
                    o.sauvegardes = args.get(i + 1).cloned();
                    i += 2;
                }
                "--aide" | "-h" | "--help" => {
                    println!(
                        "gescom-serveur [--hote 0.0.0.0] [--port {PORT_DEFAUT}] \
                         [--base CHEMIN] [--sauvegardes DOSSIER]"
                    );
                    std::process::exit(0);
                }
                _ => i += 1,
            }
        }
        o
    }
}

/// Le meme emplacement que celui ouvert par l'application Tauri.
///
/// Voulu : sur le poste principal, installer le serveur ne deplace pas
/// les donnees et ne demande pas de migration. Le monoposte devient
/// multiposte en lancant un service, rien de plus.
fn chemin_base_par_defaut() -> String {
    let base = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("ml.gescom.app");
    base.join("gescom.db").to_string_lossy().to_string()
}
