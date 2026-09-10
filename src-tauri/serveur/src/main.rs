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
mod etat;
mod http;
mod sauvegarde;
mod socle;

use std::collections::HashMap;
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use gescom_noyau::protocole::PORT_DEFAUT;
use gescom_noyau::utils::maintenant_iso;
use gescom_noyau::empreinte::empreinte_poste;
use gescom_noyau::licence::{self, EtatLicence};
use gescom_noyau::{persistance, postes, sessions};

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

    let chemin_base = options.base.unwrap_or_else(chemin_base_par_defaut);
    if let Some(parent) = std::path::Path::new(&chemin_base).parent() {
        std::fs::create_dir_all(parent).ok();
    }

    let conn = match persistance::ouvrir_base(&chemin_base) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Impossible d'ouvrir la base « {chemin_base} » : {e}");
            std::process::exit(1);
        }
    };
    if let Err(e) = persistance::initialiser_tables(&conn) {
        eprintln!("Impossible d'initialiser la base : {e}");
        std::process::exit(1);
    }

    // Une base abimee doit etre RESTAUREE, pas servie a cinq caisses
    // qui saisiront la journee par-dessus — ce qui rendrait ensuite la
    // restauration inutile.
    match persistance::verifier_integrite(&conn) {
        Ok(Some(probleme)) => {
            eprintln!("BASE ABIMEE : {probleme}");
            eprintln!("Restaurer une sauvegarde avant de redémarrer le serveur.");
            std::process::exit(2);
        }
        Err(e) => eprintln!("[avertissement] contrôle d'intégrité impossible : {e}"),
        Ok(None) => {}
    }

    // Les jetons d'avant le redemarrage ne sont plus verifiables : la
    // correspondance jeton -> session vit en memoire. Les laisser
    // « actives » en base ferait mentir la liste des postes connectes.
    sessions::revoquer_toutes(&conn, "redemarrage_serveur").ok();

    // Le serveur est lui-meme un poste : ses propres operations (une
    // sauvegarde, une revocation) ont ainsi une origine identifiable
    // dans le journal, au lieu d'un champ vide.
    let empreinte = format!("serveur:{chemin_base}");
    if let Err(e) = postes::inscrire_ou_retrouver(&conn, "Serveur", &empreinte, "serveur", None) {
        eprintln!("[avertissement] inscription du poste serveur : {e}");
    }

    // ---- Licence ----
    //
    // Elle vit sur le poste serveur, et sur lui seul. Les caisses n'ont
    // pas la leur : c'est le serveur qui compte les postes. Un client
    // qui achete trois postes ne doit pas avoir a activer trois
    // machines, ni voir ses caisses s'arreter au bout de trente jours.
    let (postes_max, libelle_licence) = lire_licence(&chemin_base, &conn);
    println!("  licence     : {libelle_licence}");

    let srv = Arc::new(Serveur {
        postes_max,
        licence: libelle_licence,
        conn: Mutex::new(conn),
        chemin_base: chemin_base.clone(),
        canal: Canal::nouveau(),
        registre: socle::registre(),
        jetons: Mutex::new(HashMap::new()),
        demarre_le: maintenant_iso(),
        derniere_sauvegarde: Mutex::new(None),
    });

    if let Some(d) = options.sauvegardes {
        if let Ok(conn) = srv.conn.lock() {
            conn.execute(
                "INSERT INTO config_app (cle, valeur) VALUES ('dossier_sauvegarde', ?1)
                 ON CONFLICT(cle) DO UPDATE SET valeur = excluded.valeur",
                rusqlite::params![d],
            )
            .ok();
        }
    }
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
    println!("  base        : {chemin_base}");
    println!("  écoute      : http://{adresse}");
    println!("  sauvegardes : {}", sauvegarde::dossier(&srv).display());
    println!("  commandes   : {}", srv.registre.len());

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

/// Lit la licence posee a cote de la base.
///
/// Sans licence, le serveur tourne en essai : UN poste. Il ne refuse
/// pas de demarrer — un service qui ne demarre pas est une boutique qui
/// n'ouvre pas, et le commercant ne saurait meme pas pourquoi.
fn lire_licence(chemin_base: &str, conn: &rusqlite::Connection) -> (u32, String) {
    let jour = chrono::Local::now().format("%Y-%m-%d").to_string();
    let empreinte = empreinte_poste();
    let chemin = std::path::Path::new(chemin_base)
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join("gescom.licence");

    let etat = match std::fs::read_to_string(&chemin) {
        Ok(t) if !t.trim().is_empty() => licence::verifier(&t, &empreinte, &jour),
        _ => {
            let debut = licence::debut_essai(conn, &jour);
            licence::etat_essai(&debut, &jour, &empreinte)
        }
    };

    let libelle = match &etat {
        EtatLicence::Valide { contenu, jours_restants } => format!(
            "{} — {} poste(s){}",
            contenu.boutique,
            contenu.postes_max,
            match jours_restants {
                Some(j) => format!(", {j} jour(s) restants"),
                None => String::new(),
            }
        ),
        EtatLicence::Essai { jours_restants, .. } =>
            format!("ESSAI — {jours_restants} jour(s), 1 poste"),
        EtatLicence::EssaiTermine { .. } =>
            format!("ESSAI TERMINE — poste {empreinte}"),
        EtatLicence::AutrePoste { attendue, .. } =>
            format!("LICENCE D'UN AUTRE POSTE ({attendue}) — poste {empreinte}"),
        EtatLicence::Expiree { le, .. } => format!("EXPIREE le {le}"),
        EtatLicence::SignatureInvalide { .. } => "SIGNATURE INVALIDE".to_string(),
        EtatLicence::Illisible { raison, .. } => format!("ILLISIBLE : {raison}"),
    };

    if !etat.autorise() {
        eprintln!("  ⚠ Aucune caisse ne pourra se connecter : {libelle}");
    }
    (etat.postes_max(), libelle)
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
