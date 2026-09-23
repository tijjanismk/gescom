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
//! gescom-serveur --installer-service | --desinstaller-service   (Windows, administrateur)
//! ```
//!
//! En service Windows (`--service`, lance par le gestionnaire de
//! services), la configuration se lit dans `%ProgramData%\Gescom\
//! serveur.json` et la sortie va dans `serveur.log` a cote : voir
//! `service.rs`.

mod api;
mod canal;
mod console;
mod etat;
mod http;
mod journal_poste;
mod journal_technique;
mod reseau_local;
mod sauvegarde;
mod service;
mod socle;

use std::collections::HashMap;
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
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
    let mut options = Options::depuis_arguments();

    #[cfg(windows)]
    {
        if options.installer_service {
            if let Err(e) = service::installer() {
                eprintln!("{e}");
                std::process::exit(1);
            }
            return;
        }
        if options.desinstaller_service {
            if let Err(e) = service::desinstaller() {
                eprintln!("{e}");
                std::process::exit(1);
            }
            return;
        }
        if options.service {
            // Sous le gestionnaire de services : la configuration vient
            // du fichier, les arguments de la ligne de commande passent
            // devant s'ils sont donnes.
            options.completer_depuis(&service::chemin_configuration());
            if options.base.is_none() {
                options.base = Some(
                    service::dossier_programme().join("gescom.db").to_string_lossy().to_string(),
                );
            }
            if let Err(e) = service::executer(move |arret| {
                let (srv, ecouteur) = preparer(options.clone());
                boucle(srv, ecouteur, arret);
            }) {
                eprintln!("{e}");
                std::process::exit(1);
            }
            return;
        }
    }

    // D4 : restaurer une sauvegarde — HORS LIGNE, avant d'ouvrir quoi
    // que ce soit. Il faut etre devant la machine, serveur arrete.
    if let Some(fichier) = options.restaurer.as_deref() {
        let cible = options.base.clone().unwrap_or_else(chemin_base_par_defaut);
        match gescom_noyau::sauvegarde::restaurer(&cible, std::path::Path::new(fichier), None) {
            Ok(r) => {
                println!("Sauvegarde restaurée ({}) sur {}", r.moteur, sans_mot_de_passe(&cible));
                if let Some(c) = r.copie_avant {
                    println!("  la base précédente est gardée : {}", c.display());
                }
                println!("  Relancer le serveur pour servir la base restaurée.");
                return;
            }
            Err(e) => {
                eprintln!("Restauration impossible : {e}");
                std::process::exit(1);
            }
        }
    }

    let (srv, ecouteur) = preparer(options);
    boucle(srv, ecouteur, Arc::new(AtomicBool::new(false)));
}

/// Ouvre la base, l'amorce, pose le poste serveur, planifie les
/// sauvegardes et se met a l'ecoute — tout ce qui precede la premiere
/// connexion. Le serveur qui en sort est pret ; il ne sert encore
/// personne.
fn preparer(options: Options) -> (Arc<Serveur>, TcpListener) {
    let cible = options.base.unwrap_or_else(chemin_base_par_defaut);
    // Meme detection que `Base::ouvrir` : une adresse PostgreSQL n'est
    // pas un chemin de fichier, et n'a pas de dossier parent a creer.
    let est_postgres = cible.starts_with("postgres://") || cible.starts_with("postgresql://");

    // Le journal technique, avant tout le reste : ce qui echoue a
    // l'ouverture de la base doit deja s'y lire. En service, stderr
    // est deja le fichier — on n'y recopie pas.
    let journal = options
        .journal
        .clone()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| chemin_journal_par_defaut(&cible, est_postgres, options.service));
    journal_technique::ouvrir(Some(journal), !options.service);

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
    if !est_postgres {
        // Le chemin fichier ne passe pas par `amorcer` : les migrations
        // qui vivent sur `Base` s'appellent ici (v3, C-1).
        amorcage::lectures_du_comptable(&mut base);
    }
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

    // D6 : `--promouvoir` redonne le role superadmin a un compte, puis
    // on s'arrete — pas d'ecoute, pas de session. Il faut etre devant
    // la machine : c'est ce qui distingue ce chemin d'un compte de
    // secours livre, utilisable depuis n'importe quelle caisse.
    if let Some(pseudo) = options.promouvoir.as_deref() {
        match gescom_noyau::auth::promouvoir_superadmin_sur(&mut base, pseudo) {
            Ok(message) => {
                println!("{message}");
                std::process::exit(0);
            }
            Err(e) => {
                eprintln!("Promotion impossible : {e}");
                std::process::exit(1);
            }
        }
    }

    // D-2 : la connexion brute a prepare le fichier (migrations,
    // integrite, sessions) ; elle ne sert plus rien ensuite — tout passe
    // par `Base`. On la ferme ici plutot que de la garder en double.
    drop(conn);
    let srv = Arc::new(Serveur {
        est_postgres,
        base: Mutex::new(base),
        chemin_base: cible.clone(),
        canal: Canal::nouveau(),
        registre: socle::registre(),
        jetons: Mutex::new(HashMap::new()),
        port: options.port,
        demarre_le: maintenant_iso(),
        derniere_sauvegarde: Mutex::new(None),
        limiteur_postes: Mutex::new(journal_poste::Limiteur::default()),
    });

    if let Some(d) = options.sauvegardes {
        if let Ok(mut base) = srv.base.lock() {
            let _ = base.executer(
                "INSERT INTO config_app (cle, valeur) VALUES ('dossier_sauvegarde', ?1)
                 ON CONFLICT(cle) DO UPDATE SET valeur = excluded.valeur",
                &gescom_noyau::parametres![d],
            );
        }
    }
    // Sur les deux moteurs : VACUUM INTO pour SQLite, pg_dump pour
    // PostgreSQL (D4).
    sauvegarde::planifier(Arc::clone(&srv));

    let adresse = format!("{}:{}", options.hote, options.port);
    let ecouteur = match TcpListener::bind(&adresse) {
        Ok(e) => e,
        Err(e) => {
            journal_technique::erreur(format!("Impossible d'écouter sur {adresse} : {e} — un autre Gescom tourne-t-il déjà sur ce poste ?"));
            std::process::exit(1);
        }
    };

    println!("Gescom serveur — protocole v{}", gescom_noyau::VERSION_PROTOCOLE);
    println!("  base        : {} ({})", sans_mot_de_passe(&cible), if est_postgres { "PostgreSQL" } else { "SQLite" });
    println!("  écoute      : http://{adresse}");
    println!(
        "  sauvegardes : {} ({})",
        sauvegarde::dossier(&srv).display(),
        if est_postgres { "pg_dump" } else { "VACUUM INTO" }
    );
    println!("  commandes   : {}", srv.registre.len());
    println!("  journal     : {}", journal_technique::chemin().map(|c| c.display().to_string()).unwrap_or_default());
    journal_technique::info(format!(
        "Serveur démarré — protocole v{}, base {} ({}), écoute http://{adresse}, {} commandes",
        gescom_noyau::VERSION_PROTOCOLE,
        sans_mot_de_passe(&cible),
        if est_postgres { "PostgreSQL" } else { "SQLite" },
        srv.registre.len()
    ));

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
    println!("  Console : http://localhost:{}", options.port);
    println!("            a ouvrir dans un navigateur sur ce poste.");

    (srv, ecouteur)
}

/// Sert les connexions jusqu'a ce que `arret` soit leve — par le
/// gestionnaire de services, ou jamais quand on tourne a la main.
///
/// L'ecouteur est en mode non bloquant : `accept` rend la main toutes
/// les 200 ms pour regarder le drapeau. Un `accept` bloquant ne se
/// reveillerait qu'a la prochaine connexion, et « sc stop » attendrait
/// qu'une caisse veuille bien appeler.
fn boucle(srv: Arc<Serveur>, ecouteur: TcpListener, arret: Arc<AtomicBool>) {
    ecouteur.set_nonblocking(true).ok();
    while !arret.load(Ordering::SeqCst) {
        match ecouteur.accept() {
            Ok((flux, _)) => {
                flux.set_nonblocking(false).ok();
                let srv = Arc::clone(&srv);
                // Un fil par connexion. SQLite serialise de toute facon
                // les ecritures derriere le verrou de `srv.base` ; le
                // parallelisme sert ici a ne pas faire attendre une
                // caisse pendant qu'une autre imprime.
                std::thread::spawn(move || servir(&srv, flux));
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(200));
            }
            Err(e) => journal_technique::erreur(format!("Connexion refusée par le système : {e}")),
        }
    }
    journal_technique::info("Arrêt demandé : le serveur ferme.");
    // Les jetons meurent avec le processus ; le dire en base evite une
    // liste de postes « connectes » qui ment au prochain demarrage.
    if let Ok(mut b) = srv.base.lock() {
        gescom_noyau::sessions::revoquer_toutes_sur(&mut b, "arret_serveur").ok();
    }
}

fn servir(srv: &Arc<Serveur>, mut flux: TcpStream) {
    flux.set_read_timeout(Some(DELAI_LECTURE)).ok();
    flux.set_nodelay(true).ok();

    match http::lire_requete(&flux) {
        Ok(requete) => {
            if let Err(e) = api::traiter(srv, &requete, &mut flux) {
                journal_technique::avertissement(format!("Réponse interrompue : {e}"));
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

#[derive(Clone)]
struct Options {
    hote: String,
    port: u16,
    base: Option<String>,
    sauvegardes: Option<String>,
    /// Le fichier du journal technique (journal_technique.rs). Par
    /// defaut : `serveur.log` a cote de la base fichier, ou dans
    /// `%ProgramData%\Gescom` en service.
    journal: Option<String>,
    promouvoir: Option<String>,
    /// Restaure ce fichier de sauvegarde sur la base, puis s'arrete.
    restaurer: Option<String>,
    /// Lance par le gestionnaire de services Windows (service.rs).
    service: bool,
    installer_service: bool,
    desinstaller_service: bool,
    /// Le port a-t-il ete donne sur la ligne de commande ? Sinon le
    /// fichier de configuration peut le fixer.
    port_donne: bool,
}

impl Options {
    /// Ce que le fichier de configuration du service peut fixer. Tout
    /// est facultatif ; ce qui manque garde sa valeur par defaut.
    ///
    /// ```json
    /// { "base": "postgresql://gescom:***@127.0.0.1:5432/gescom",
    ///   "port": 7300, "sauvegardes": "D:\\Sauvegardes" }
    /// ```
    fn completer_depuis(&mut self, chemin: &std::path::Path) {
        let Ok(texte) = std::fs::read_to_string(chemin) else { return };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&texte) else {
            eprintln!("[configuration] {} illisible — ignoree", chemin.display());
            return;
        };
        if self.base.is_none() {
            self.base = v.get("base").and_then(|b| b.as_str()).filter(|b| !b.trim().is_empty()).map(str::to_string);
        }
        if !self.port_donne {
            if let Some(p) = v.get("port").and_then(|p| p.as_u64()).and_then(|p| u16::try_from(p).ok()) {
                self.port = p;
            }
        }
        if self.sauvegardes.is_none() {
            self.sauvegardes = v.get("sauvegardes").and_then(|s| s.as_str()).filter(|s| !s.trim().is_empty()).map(str::to_string);
        }
        if self.journal.is_none() {
            self.journal = v.get("journal").and_then(|s| s.as_str()).filter(|s| !s.trim().is_empty()).map(str::to_string);
        }
        if let Some(h) = v.get("hote").and_then(|h| h.as_str()).filter(|h| !h.trim().is_empty()) {
            self.hote = h.to_string();
        }
    }

    fn depuis_arguments() -> Options {
        let mut o = Options {
            // 0.0.0.0 et non 127.0.0.1 : sinon les autres postes du
            // magasin ne joignent pas le serveur, et le symptome
            // (« connexion refusée ») ne dit pas pourquoi.
            hote: "0.0.0.0".to_string(),
            port: PORT_DEFAUT,
            base: None,
            sauvegardes: None,
            journal: None,
            promouvoir: None,
            restaurer: None,
            service: false,
            installer_service: false,
            desinstaller_service: false,
            port_donne: false,
        };
        let args: Vec<String> = std::env::args().skip(1).collect();
        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--port" => {
                    if let Some(v) = args.get(i + 1).and_then(|v| v.parse().ok()) {
                        o.port = v;
                        o.port_donne = true;
                    }
                    i += 2;
                }
                "--service" => {
                    o.service = true;
                    i += 1;
                }
                "--installer-service" => {
                    o.installer_service = true;
                    i += 1;
                }
                "--desinstaller-service" => {
                    o.desinstaller_service = true;
                    i += 1;
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
                "--journal" => {
                    o.journal = args.get(i + 1).cloned();
                    i += 2;
                }
                "--promouvoir" => {
                    o.promouvoir = args.get(i + 1).cloned();
                    i += 2;
                }
                "--restaurer" => {
                    o.restaurer = args.get(i + 1).cloned();
                    i += 2;
                }
                "--aide" | "-h" | "--help" => {
                    println!(
                        "gescom-serveur [--hote 0.0.0.0] [--port {PORT_DEFAUT}] \
                         [--base CHEMIN] [--sauvegardes DOSSIER] [--journal FICHIER] [--promouvoir IDENTIFIANT]\n\
                         gescom-serveur --restaurer FICHIER [--base CHEMIN]   (serveur arrêté)\n\
                         gescom-serveur --installer-service | --desinstaller-service  (Windows, administrateur)"
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
/// L'URL telle qu'on peut l'ecrire dans un journal : le mot de passe
/// masque. Le fichier de log du serveur se lit sans droits (D10).
fn sans_mot_de_passe(cible: &str) -> String {
    let Some((tete, reste)) = cible.split_once("://") else { return cible.to_string() };
    let Some((acces, suite)) = reste.rsplit_once('@') else { return cible.to_string() };
    match acces.split_once(':') {
        Some((u, _)) => format!("{tete}://{u}:***@{suite}"),
        None => cible.to_string(),
    }
}

/// Ou va le journal quand rien ne le dit : a cote de la base fichier
/// (`gescom.db` → `gescom.log`, ce qui suit la base dans les tests et
/// les essais), dans le dossier de l'application pour PostgreSQL, et
/// dans `%ProgramData%\Gescom` en service — la ou l'administrateur
/// sait deja regarder.
fn chemin_journal_par_defaut(cible: &str, est_postgres: bool, service: bool) -> std::path::PathBuf {
    #[cfg(windows)]
    if service {
        return service::chemin_journal();
    }
    let _ = service;
    if est_postgres {
        dirs::data_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("ml.gescom.app")
            .join("serveur.log")
    } else {
        std::path::PathBuf::from(cible).with_extension("log")
    }
}

fn chemin_base_par_defaut() -> String {
    let base = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("ml.gescom.app");
    base.join("gescom.db").to_string_lossy().to_string()
}
