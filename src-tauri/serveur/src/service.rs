//! Le serveur en SERVICE Windows.
//!
//! Un serveur qu'il faut penser a lancer chaque matin est un serveur
//! qui ne tournera pas. En service, il demarre avec la machine, avant
//! qu'une session soit ouverte, et se relance tout seul s'il tombe. On
//! l'arrete et on le redemarre proprement :
//!
//! ```text
//! gescom-serveur --installer-service      (droits administrateur)
//! gescom-serveur --desinstaller-service
//! sc stop GescomServeur   /   sc start GescomServeur
//! ```
//!
//! Sous le gestionnaire de services il n'y a ni console ni session : la
//! sortie est envoyee dans un fichier journal (`SetStdHandle`, que
//! `println!` suit a chaque ecriture), et la configuration (base, port,
//! sauvegardes) se lit dans `%ProgramData%\Gescom\serveur.json`, ecrit
//! par l'installeur. Le mot de passe PostgreSQL vit dans ce fichier,
//! sur la machine du serveur, protege par les droits du dossier — pas
//! dans le depot (D10).
//!
//! Parle directement a l'API Win32 (`windows-sys`) : pas de crate
//! d'enveloppe, le contrat tient en quatre appels — s'inscrire aupres
//! du repartiteur, enregistrer le gestionnaire de controle, annoncer
//! l'etat, et rendre la main quand STOP arrive.

#![cfg(windows)]

use std::os::windows::io::AsRawHandle;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::System::Console::{SetStdHandle, STD_ERROR_HANDLE, STD_OUTPUT_HANDLE};
use windows_sys::Win32::System::Services::{
    RegisterServiceCtrlHandlerExW, SetServiceStatus, StartServiceCtrlDispatcherW,
    SERVICE_ACCEPT_SHUTDOWN, SERVICE_ACCEPT_STOP, SERVICE_CONTROL_INTERROGATE,
    SERVICE_CONTROL_SHUTDOWN, SERVICE_CONTROL_STOP, SERVICE_RUNNING, SERVICE_START_PENDING,
    SERVICE_STATUS, SERVICE_STATUS_HANDLE, SERVICE_STOPPED, SERVICE_STOP_PENDING,
    SERVICE_TABLE_ENTRYW, SERVICE_WIN32_OWN_PROCESS,
};

/// Le nom interne du service — celui de `sc stop` / `sc start`.
pub const NOM_SERVICE: &str = "GescomServeur";
const NOM_AFFICHE: &str = "Gescom Serveur";
const DESCRIPTION: &str =
    "Détient la base Gescom et sert les caisses du magasin (port 7300 par défaut).";

/// Le dossier de la machine, pas d'un utilisateur : le service tourne
/// sous LocalSystem, dont le profil n'est pas un endroit ou ranger la
/// base d'une boutique.
pub fn dossier_programme() -> PathBuf {
    std::env::var_os("ProgramData")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"))
        .join("Gescom")
}

pub fn chemin_configuration() -> PathBuf {
    dossier_programme().join("serveur.json")
}

pub fn chemin_journal() -> PathBuf {
    dossier_programme().join("serveur.log")
}

// ---------------------------------------------------------------------
//  Installer / desinstaller — par `sc.exe`, l'outil que l'administrateur
//  connait deja, et dont les messages d'erreur sont les siens.
// ---------------------------------------------------------------------

pub fn installer() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let bin = format!("\"{}\" --service", exe.display());
    std::fs::create_dir_all(dossier_programme()).map_err(|e| e.to_string())?;

    sc(&["create", NOM_SERVICE, "binPath=", &bin, "start=", "auto", "DisplayName=", NOM_AFFICHE])?;
    sc(&["description", NOM_SERVICE, DESCRIPTION])?;
    // Tombe ? Relance apres 5 s, puis 30 s, puis 60 s — et le compteur
    // se remet a zero apres un jour sans incident.
    sc(&["failure", NOM_SERVICE, "reset=", "86400", "actions=", "restart/5000/restart/30000/restart/60000"])?;
    sc(&["start", NOM_SERVICE])?;
    println!("Service « {NOM_AFFICHE} » installé et démarré.");
    println!("  configuration : {}", chemin_configuration().display());
    println!("  journal       : {}", chemin_journal().display());
    println!("  arrêter       : sc stop {NOM_SERVICE}");
    println!("  redémarrer    : sc start {NOM_SERVICE}");
    Ok(())
}

pub fn desinstaller() -> Result<(), String> {
    // L'arret peut echouer s'il est deja arrete : ce n'est pas une erreur.
    let _ = sc(&["stop", NOM_SERVICE]);
    std::thread::sleep(std::time::Duration::from_secs(2));
    sc(&["delete", NOM_SERVICE])?;
    println!("Service « {NOM_AFFICHE} » retiré.");
    Ok(())
}

fn sc(args: &[&str]) -> Result<(), String> {
    let sortie = Command::new("sc.exe").args(args).output().map_err(|e| format!("sc.exe : {e}"))?;
    if sortie.status.success() {
        return Ok(());
    }
    let texte = String::from_utf8_lossy(&sortie.stdout).trim().to_string();
    Err(format!(
        "sc {} : {}{}",
        args[0],
        texte,
        if texte.contains("1073") { "" } else { "\n(ouvrir PowerShell EN ADMINISTRATEUR)" }
    ))
}

// ---------------------------------------------------------------------
//  Tourner sous le gestionnaire de services
// ---------------------------------------------------------------------

struct Etat {
    poignee: SERVICE_STATUS_HANDLE,
    arret: Arc<AtomicBool>,
}
// SERVICE_STATUS_HANDLE est un pointeur opaque que Windows nous rend
// pour qu'on le lui repasse : il ne designe rien qu'un fil possede.
unsafe impl Send for Etat {}
unsafe impl Sync for Etat {}

static ETAT: OnceLock<Etat> = OnceLock::new();
/// Ce que `main` doit lancer une fois que Windows a donne le depart.
static CORPS: OnceLock<Box<dyn Fn(Arc<AtomicBool>) + Send + Sync>> = OnceLock::new();

/// Se met sous le controle du gestionnaire de services et y reste
/// jusqu'a l'arret. `corps` recoit le drapeau d'arret et ne rend la
/// main que quand tout est ferme.
pub fn executer(corps: impl Fn(Arc<AtomicBool>) + Send + Sync + 'static) -> Result<(), String> {
    rediriger_la_sortie();
    let _ = CORPS.set(Box::new(corps));
    let mut nom: Vec<u16> = NOM_SERVICE.encode_utf16().chain(std::iter::once(0)).collect();
    let table = [
        SERVICE_TABLE_ENTRYW { lpServiceName: nom.as_mut_ptr(), lpServiceProc: Some(service_main) },
        SERVICE_TABLE_ENTRYW { lpServiceName: std::ptr::null_mut(), lpServiceProc: None },
    ];
    // Bloque jusqu'a ce que le service s'arrete.
    let ok = unsafe { StartServiceCtrlDispatcherW(table.as_ptr()) };
    if ok == 0 {
        return Err(
            "Ce chemin ne se lance que par le gestionnaire de services (sc start GescomServeur). \
             Pour un essai a la main, lancer sans --service."
                .to_string(),
        );
    }
    Ok(())
}

unsafe extern "system" fn service_main(_argc: u32, _argv: *mut *mut u16) {
    let mut nom: Vec<u16> = NOM_SERVICE.encode_utf16().chain(std::iter::once(0)).collect();
    let poignee = RegisterServiceCtrlHandlerExW(nom.as_mut_ptr(), Some(controle), std::ptr::null_mut());
    if poignee.is_null() {
        return;
    }
    let arret = Arc::new(AtomicBool::new(false));
    let _ = ETAT.set(Etat { poignee, arret: Arc::clone(&arret) });

    annoncer(SERVICE_START_PENDING, 0);
    if let Some(corps) = CORPS.get() {
        annoncer(SERVICE_RUNNING, SERVICE_ACCEPT_STOP | SERVICE_ACCEPT_SHUTDOWN);
        corps(arret);
    }
    annoncer(SERVICE_STOPPED, 0);
}

unsafe extern "system" fn controle(
    code: u32,
    _type: u32,
    _donnees: *mut core::ffi::c_void,
    _contexte: *mut core::ffi::c_void,
) -> u32 {
    match code {
        SERVICE_CONTROL_STOP | SERVICE_CONTROL_SHUTDOWN => {
            annoncer(SERVICE_STOP_PENDING, 0);
            if let Some(e) = ETAT.get() {
                e.arret.store(true, Ordering::SeqCst);
            }
            0
        }
        SERVICE_CONTROL_INTERROGATE => 0,
        // ERROR_CALL_NOT_IMPLEMENTED : on ne gere pas ce controle.
        _ => 120,
    }
}

fn annoncer(etat: u32, accepte: u32) {
    let Some(e) = ETAT.get() else { return };
    let statut = SERVICE_STATUS {
        dwServiceType: SERVICE_WIN32_OWN_PROCESS,
        dwCurrentState: etat,
        dwControlsAccepted: accepte,
        dwWin32ExitCode: 0,
        dwServiceSpecificExitCode: 0,
        dwCheckPoint: 0,
        dwWaitHint: if etat == SERVICE_STOP_PENDING || etat == SERVICE_START_PENDING { 10_000 } else { 0 },
    };
    unsafe {
        SetServiceStatus(e.poignee, &statut);
    }
}

/// Toute la sortie du serveur va dans le journal : sans console, elle
/// se perdrait. `println!` relit la poignee standard a chaque ecriture,
/// donc le changement prend effet pour tout le programme.
fn rediriger_la_sortie() {
    let _ = std::fs::create_dir_all(dossier_programme());
    let Ok(fichier) = std::fs::OpenOptions::new().create(true).append(true).open(chemin_journal()) else {
        return;
    };
    let h = fichier.as_raw_handle() as HANDLE;
    unsafe {
        SetStdHandle(STD_OUTPUT_HANDLE, h);
        SetStdHandle(STD_ERROR_HANDLE, h);
    }
    // Le fichier doit vivre aussi longtemps que le processus.
    std::mem::forget(fichier);
    println!();
    println!("===== {} — service démarré =====", gescom_noyau::utils::maintenant_iso());
}
