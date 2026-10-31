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
//! gescom-serveur [--port 40000] [--base CHEMIN] [--sauvegardes DOSSIER]
//! gescom-serveur --installer-service | --desinstaller-service   (Windows, administrateur)
//! ```
//!
//! En service Windows (`--service`, lance par le gestionnaire de
//! services), la configuration se lit dans `%ProgramData%\Gescom\
//! serveur.json` et la sortie va dans `serveur.log` a cote : voir
//! `service.rs`. Lance a la main sans `--port` ni `--hote`, le serveur y
//! lit son port et son ecoute (et seulement eux) : le panneau Reseau de
//! la console les y change, puis le serveur redemarre dessus — le service
//! en s'arretant en echec (la relance automatique de D12), le serveur
//! lance a la main en se relancant lui-meme avec les memes arguments.

mod api;
mod canal;
mod configuration;
mod console;
mod decouverte;
mod etat;
mod http;
mod journal_poste;
mod journal_technique;
mod reseau_local;
mod role_poste;
mod sauvegarde;
mod service;
mod socle;

use std::collections::HashMap;
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

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

/// Posee sur le serveur relance par lui-meme apres un changement de port :
/// l'ancien processus libere peut-etre encore ses ports, on reessaie
/// quelques secondes au lieu d'abandonner tout de suite.
const VARIABLE_RELANCE: &str = "GESCOM_RELANCE";

/// Le code de sortie du service qui s'arrete pour redemarrer sur un
/// nouveau port : un arret sans « SERVICE_STOPPED » est un incident pour
/// Windows, qui applique la relance posee par `--installer-service`
/// (`sc failure … restart/5000/…`, D12).
#[cfg(windows)]
const CODE_REDEMARRAGE: i32 = 75;

fn relance() -> bool {
    std::env::var(VARIABLE_RELANCE).is_ok_and(|v| v == "1")
}

/// Comment la boucle s'est terminee.
enum Fin {
    /// `sc stop`, ou rien (on ne revient jamais d'un serveur a la main).
    Arret,
    /// Le panneau Reseau a change le port ou l'ecoute : redemarrer.
    Redemarrage,
}

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
            if let Some(c) = configuration::chemin() {
                options.completer_depuis(&c);
            }
            if options.base.is_none() {
                options.base = Some(
                    service::dossier_programme().join("gescom.db").to_string_lossy().to_string(),
                );
            }
            if let Err(e) = service::executer(move |arret| {
                if let Fin::Redemarrage = tourner(options.clone(), arret) {
                    // Pas de « SERVICE_STOPPED » : Windows y voit un
                    // incident et relance le service (D12), qui relira
                    // serveur.json.
                    journal_technique::info(
                        "Le service s'arrête pour redémarrer sur le nouveau réglage : Windows le relance \
                         (5 s, puis 30 s et 60 s si cela se répète dans la journée).",
                    );
                    std::process::exit(CODE_REDEMARRAGE);
                }
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

    // D24 (revise le 25/10) : lance par le raccourci de demarrage — sans
    // `--base` — sur la caisse d'un AUTRE poste, le serveur s'efface :
    // la base de la boutique est la-bas. Les gestes ponctuels
    // (`--promouvoir`, `--compte-limite`) ne sont pas concernes.
    if options.base.is_none() && options.promouvoir.is_none() && options.compte_limite.is_none() {
        if let Some(raison) = role_poste::verifier() {
            // Le journal qu'il aurait tenu (a cote de la base par defaut),
            // et la console : c'est la qu'on cherchera pourquoi.
            journal_technique::ouvrir(Some(role_poste::dossier_donnees().join("gescom.log")), true);
            journal_technique::info(format!("Serveur non démarré : {raison}"));
            return;
        }
    }

    // Lance a la main sans `--port` ni `--hote` : le port et l'ecoute de
    // serveur.json, s'il en a — ceux que le panneau Reseau y ecrit avant
    // de relancer le serveur. Rien d'autre n'y est lu : la base d'un
    // serveur lance a la main reste celle de ce poste (D24).
    if !options.port_donne && !options.hote_donne {
        if let Some(c) = configuration::chemin() {
            options.completer_reseau_depuis(&c);
        }
    }

    if let Fin::Redemarrage = tourner(options, Arc::new(AtomicBool::new(false))) {
        relancer();
    }
}

/// Prepare, se met a l'ecoute (TCP et recherche UDP), sert jusqu'a la fin.
fn tourner(options: Options, arret: Arc<AtomicBool>) -> Fin {
    let (srv, ecouteur) = preparer(options);
    let (ecoute, etat) = decouverte::lancer(srv.port, &srv.hote, relance());
    if let Ok(mut d) = srv.decouverte.lock() {
        *d = etat;
    }
    let fin = boucle(Arc::clone(&srv), ecouteur, arret);
    // Le port UDP libere AVANT de relancer : le suivant le reprend.
    if let Some(e) = ecoute {
        e.arreter();
    }
    fin
}

/// Le serveur lance a la main se relance lui-meme, avec les memes
/// arguments : le nouveau processus relit serveur.json et prend le nouveau
/// port. Celui-ci s'en va.
fn relancer() -> ! {
    let exe = match std::env::current_exe() {
        Ok(e) => e,
        Err(e) => {
            journal_technique::erreur(format!("Redémarrage impossible ({e}) : relancer Gescom Serveur à la main."));
            std::process::exit(1);
        }
    };
    let arguments: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    match std::process::Command::new(&exe).args(&arguments).env(VARIABLE_RELANCE, "1").spawn() {
        Ok(enfant) => {
            journal_technique::info(format!(
                "Redémarrage sur le nouveau réglage : nouveau processus (pid {}), mêmes arguments.",
                enfant.id()
            ));
            std::process::exit(0);
        }
        Err(e) => {
            journal_technique::erreur(format!("Redémarrage impossible ({e}) : relancer Gescom Serveur à la main."));
            std::process::exit(1);
        }
    }
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
        // qui vivent sur `Base` s'appellent ici (v3, C-1, E-1).
        amorcage::migrations_de_donnees(&mut base);
        // v3, D-5 : la base d'avant devient un dossier a son nom et a
        // ses dates, une fois.
        match gescom_noyau::dossiers::migrer_dossier_d_origine_sur(&mut base) {
            Ok(Some(fait)) if !(fait["societe"].is_null() && fait["debut"].is_null()) => {
                journal_technique::info(format!("Dossier d'origine nommé et daté : {fait}"))
            }
            Ok(_) => {}
            Err(e) => journal_technique::erreur(format!("Migration du dossier d'origine : {e}")),
        }
    }
    // v3, D-6 : sous le compte limite, pas de migration (il n'en a pas
    // le droit) — le proprietaire les fait avec `--compte-limite`.
    let compte_limite = est_postgres && !base.peut_migrer();
    if compte_limite {
        journal_technique::info(
            "Compte PostgreSQL limité : le moteur cloisonne les dossiers ; les migrations se font \
             avec le compte propriétaire (--compte-limite).",
        );
    }
    if est_postgres && !compte_limite {
        match amorcage::amorcer(&mut base) {
            Ok(true) => amorcage_fait = true,
            Ok(false) => {}
            Err(e) => {
                eprintln!("Impossible d'amorcer la base : {e}");
                std::process::exit(1);
            }
        }
        // D43 : une base d'essai (le banc) s'ouvre sans assistant, avec
        // le compte de l'employe — comme `seed` le fait sur SQLite. Seule
        // une base qui attend encore son assistant est touchee.
        if mode_demo() && gescom_noyau::assistant::a_faire_sur(&mut base) {
            if let Err(e) = amorcage::mode_essai(&mut base) {
                eprintln!("Base d'essai : {e}");
            }
        }
    }
    if est_postgres {

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

    // v3, D-6 (D10) : `--compte-limite NOM`, lance avec l'adresse du
    // PROPRIETAIRE — les migrations viennent d'etre faites juste au-dessus
    // — cree ou remet d'aplomb le compte limite, puis on s'arrete. A
    // rejouer a chaque mise a jour. Le mot de passe : celui de
    // `GESCOM_MDP_COMPTE_LIMITE`, sinon un tire au hasard, affiche UNE
    // fois ici, et nulle part ailleurs.
    if let Some(nom) = options.compte_limite.as_deref() {
        let donne = std::env::var("GESCOM_MDP_COMPTE_LIMITE").ok().filter(|m| !m.trim().is_empty());
        let mot_de_passe = donne.clone().unwrap_or_else(|| {
            format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple())
        });
        match amorcage::poser_compte_limite(&mut base, nom, &mot_de_passe) {
            Ok(()) => {
                let hote = cible.split_once("://").map(|(_, r)| r.rsplit_once('@').map(|(_, h)| h).unwrap_or(r)).unwrap_or("");
                println!("Compte limité « {nom} » prêt : il ne lit et n'écrit que le dossier de sa session.");
                if donne.is_some() {
                    println!("Mot de passe : celui de GESCOM_MDP_COMPTE_LIMITE.");
                    println!("Adresse du serveur : postgresql://{nom}:<mot de passe>@{hote}");
                } else {
                    println!("Adresse du serveur (à mettre dans poste.json, nulle part ailleurs) :");
                    println!("  postgresql://{nom}:{mot_de_passe}@{hote}");
                }
                println!("Sauvegardes : GESCOM_PG_SAUVEGARDE = l'adresse du propriétaire, dans l'environnement du serveur.");
                std::process::exit(0);
            }
            Err(e) => {
                eprintln!("Compte limité impossible : {e}");
                std::process::exit(1);
            }
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
        canal: Canal::nouveau(),
        registre: socle::registre(),
        jetons: Mutex::new(HashMap::new()),
        port: options.port,
        hote: options.hote.clone(),
        // `--port` ou `--hote` decident (la fenetre de Gescom, D39 ; les
        // essais) : le panneau Reseau montre, mais ne change pas.
        reseau_modifiable: !options.port_donne && !options.hote_donne && configuration::chemin().is_some(),
        en_service: options.service,
        redemarrage: AtomicBool::new(false),
        decouverte: Mutex::new(Err("pas encore lancée".to_string())),
        changement_reseau: Mutex::new(()),
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
    // La derniere copie du dossier, pour `/sante` : un redemarrage ne
    // doit pas faire croire que rien n'a jamais ete sauvegarde.
    if let Ok(mut d) = srv.derniere_sauvegarde.lock() {
        *d = sauvegarde::derniere_copie(&srv);
    }
    // Sur les deux moteurs : VACUUM INTO pour SQLite, pg_dump pour
    // PostgreSQL (D4).
    sauvegarde::planifier(Arc::clone(&srv));

    let adresse = format!("{}:{}", options.hote, options.port);
    // D39 : les fenetres livrees ne joignent que 40000..=40100 (leur
    // CSP). Hors de la plage, le serveur tourne — les essais et la
    // console en ont besoin — mais aucune caisse ne le joindra : le dire.
    if !gescom_noyau::protocole::port_dans_la_plage(options.port) {
        journal_technique::avertissement(format!(
            "Port {} hors de la plage {}-{} : les caisses Gescom ne joindront pas ce serveur. \
             Choisir un port de la plage (serveur.json ou --port).",
            options.port,
            gescom_noyau::protocole::PORT_DEFAUT,
            gescom_noyau::protocole::PORT_MAX
        ));
    }
    // Relance apres un changement de port : l'ancien processus ferme
    // peut-etre encore son ecoute (meme port, autre ecoute) — quelques
    // secondes de patience, pas plus.
    let debut = Instant::now();
    let ecouteur = loop {
        match TcpListener::bind(&adresse) {
            Ok(e) => break e,
            Err(_) if relance() && debut.elapsed() < Duration::from_secs(15) => {
                std::thread::sleep(Duration::from_millis(300));
            }
            Err(e) => {
                journal_technique::erreur(format!("Impossible d'écouter sur {adresse} : {e} — un autre Gescom tourne-t-il déjà sur ce poste ?"));
                std::process::exit(1);
            }
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
        println!("  BASE NEUVE — compte du patron : admin / admin123");
        println!("    À la première connexion, l'assistant crée le dossier :");
        println!("    société, dates de travail, nouveau mot de passe, vendeurs.");
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
fn boucle(srv: Arc<Serveur>, ecouteur: TcpListener, arret: Arc<AtomicBool>) -> Fin {
    ecouteur.set_nonblocking(true).ok();
    while !arret.load(Ordering::SeqCst) && !srv.redemarrage.load(Ordering::SeqCst) {
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
    let redemarrage = srv.redemarrage.load(Ordering::SeqCst);
    journal_technique::info(if redemarrage {
        "Redémarrage demandé (port ou écoute changés) : le serveur ferme."
    } else {
        "Arrêt demandé : le serveur ferme."
    });
    // Les jetons meurent avec le processus ; le dire en base evite une
    // liste de postes « connectes » qui ment au prochain demarrage.
    if let Ok(mut b) = srv.base.lock() {
        gescom_noyau::sessions::revoquer_toutes_sur(
            &mut b,
            if redemarrage { "redemarrage_serveur" } else { "arret_serveur" },
        )
        .ok();
    }
    // `ecouteur` tombe ici : le port est libre pour le serveur relance.
    if redemarrage {
        Fin::Redemarrage
    } else {
        Fin::Arret
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
    /// v3, D-6 : cree le compte PostgreSQL limite, puis s'arrete.
    compte_limite: Option<String>,
    /// Restaure ce fichier de sauvegarde sur la base, puis s'arrete.
    restaurer: Option<String>,
    /// Lance par le gestionnaire de services Windows (service.rs).
    service: bool,
    installer_service: bool,
    desinstaller_service: bool,
    /// Le port a-t-il ete donne sur la ligne de commande ? Sinon le
    /// fichier de configuration peut le fixer.
    port_donne: bool,
    /// L'ecoute (`--hote`), meme chose.
    hote_donne: bool,
}

impl Options {
    /// Ce que le fichier de configuration du service peut fixer. Tout
    /// est facultatif ; ce qui manque garde sa valeur par defaut.
    ///
    /// ```json
    /// { "base": "postgresql://gescom:***@127.0.0.1:5432/gescom",
    ///   "port": 40000, "sauvegardes": "D:\\Sauvegardes" }
    /// ```
    fn completer_depuis(&mut self, chemin: &std::path::Path) {
        let Ok(texte) = std::fs::read_to_string(chemin) else { return };
        // Le Bloc-notes peut y laisser un BOM : on le retire avant de lire.
        let Ok(v) = serde_json::from_str::<serde_json::Value>(texte.trim_start_matches('\u{feff}')) else {
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
        if !self.hote_donne {
            if let Some(h) = v.get("hote").and_then(|h| h.as_str()).filter(|h| !h.trim().is_empty()) {
                self.hote = h.trim().to_string();
            }
        }
    }

    /// Le port et l'ecoute seulement, pour un serveur lance a la main sans
    /// `--port` ni `--hote` (voir `main`). Un fichier absent ou illisible
    /// ne change rien : les defauts restent.
    fn completer_reseau_depuis(&mut self, chemin: &std::path::Path) {
        let Ok(texte) = std::fs::read_to_string(chemin) else { return };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(texte.trim_start_matches('\u{feff}')) else {
            eprintln!("[configuration] {} illisible — ignoree", chemin.display());
            return;
        };
        if let Some(p) = v.get("port").and_then(|p| p.as_u64()).and_then(|p| u16::try_from(p).ok()) {
            self.port = p;
        }
        if let Some(h) = v.get("hote").and_then(|h| h.as_str()).filter(|h| !h.trim().is_empty()) {
            self.hote = h.trim().to_string();
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
            compte_limite: None,
            restaurer: None,
            service: false,
            installer_service: false,
            desinstaller_service: false,
            port_donne: false,
            hote_donne: false,
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
                        o.hote_donne = true;
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
                "--compte-limite" => {
                    o.compte_limite = args.get(i + 1).cloned();
                    i += 2;
                }
                "--aide" | "-h" | "--help" => {
                    println!(
                        "gescom-serveur [--hote 0.0.0.0] [--port {PORT_DEFAUT}] \
                         [--base CHEMIN] [--sauvegardes DOSSIER] [--journal FICHIER] [--promouvoir IDENTIFIANT]\n\
                         gescom-serveur --restaurer FICHIER [--base CHEMIN]   (serveur arrêté)\n\
                         gescom-serveur --base <URL PostgreSQL du propriétaire> --compte-limite NOM\n\
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
    role_poste::dossier_donnees().join("gescom.db").to_string_lossy().to_string()
}

/// `GESCOM_DEMO=1` : une base d'essai (le banc, les demonstrations),
/// comme le lit `seed` sur SQLite.
fn mode_demo() -> bool {
    std::env::var("GESCOM_DEMO")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}
