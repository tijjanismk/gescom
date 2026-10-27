//! Gescom Serveur — la fenetre du serveur (D25, 27/10/2026).
//!
//! « Une interface graphique dediee a mon serveur, comme sur Ciel. » Une
//! coque Tauri fine, sans base ni code metier : elle affiche la console
//! que le serveur de CE poste sert deja (`/console`) — la meme page que
//! dans le navigateur, rien en double. Elle lit le port dans
//! `%ProgramData%\Gescom\serveur.json` (sinon 40000), verifie `/sante`,
//! puis s'y rend.
//!
//! Si le serveur ne repond pas, sa propre page (`page/index.html`) dit
//! « Le serveur ne tourne pas » et propose « Demarrer le service »
//! (`sc start GescomServeur`, avec l'accord administrateur que Windows
//! demande) et « Reessayer ». Un fil la ramene a cette page quand la
//! console ne repond plus.
//!
//! La console est une page DISTANTE : elle n'a aucun acces aux commandes
//! de la fenetre. Tauri refuse deja les commandes aux origines distantes
//! (pas de capacite `remote`) ; chaque commande ici le reverifie.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::time::Duration;

use serde::Serialize;
use tauri::{Manager, Url};

/// La plage du serveur Gescom (D39), recopiee de `gescom_noyau::protocole`
/// — la fenetre ne depend pas du noyau. `noyau/tests/plage_de_ports.rs`
/// verifie que la copie suit.
const PORT_DEFAUT: u16 = 40000;
const PORT_MAX: u16 = 40100;

/// Le nom du service Windows (`serveur/src/service.rs`, D12).
const NOM_SERVICE: &str = "GescomServeur";

/// En debogage, le meme remplacement que le serveur
/// (`serveur/src/configuration.rs`) : un essai ne lit pas le vrai
/// `ProgramData`.
fn chemin_configuration() -> PathBuf {
    #[cfg(debug_assertions)]
    if let Some(c) = std::env::var_os("GESCOM_SERVEUR_JSON").filter(|c| !c.is_empty()) {
        return PathBuf::from(c);
    }
    std::env::var_os("ProgramData")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"))
        .join("Gescom")
        .join("serveur.json")
}

/// Ou joindre le serveur de ce poste : le port de `serveur.json`, et
/// l'hote — `127.0.0.1` quand il ecoute toutes les cartes (ou ce poste
/// seulement), l'adresse elle-meme quand elle a ete reglee a la main.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Reglage {
    hote: String,
    port: u16,
}

fn lire_reglage(texte: Option<&str>) -> Reglage {
    let v: serde_json::Value = texte
        .and_then(|t| serde_json::from_str(t.trim_start_matches('\u{feff}')).ok())
        .unwrap_or(serde_json::Value::Null);
    let port = v
        .get("port")
        .and_then(|p| p.as_u64())
        .and_then(|p| u16::try_from(p).ok())
        .filter(|p| *p != 0)
        .unwrap_or(PORT_DEFAUT);
    let hote = match v.get("hote").and_then(|h| h.as_str()).map(str::trim) {
        None | Some("") | Some("0.0.0.0") | Some("::") | Some("127.0.0.1") | Some("localhost") => "127.0.0.1",
        Some(autre) => autre,
    }
    .to_string();
    Reglage { hote, port }
}

/// Les adresses a essayer, dans l'ordre : celle du reglage, puis le port
/// par defaut sur ce poste (un serveur lance a la main, sans service).
fn candidats(r: &Reglage) -> Vec<Reglage> {
    let mut c = vec![r.clone()];
    let defaut = Reglage { hote: "127.0.0.1".to_string(), port: PORT_DEFAUT };
    if !c.contains(&defaut) {
        c.push(defaut);
    }
    c
}

/// « hote:port » pour une URL (IPv6 entre crochets).
fn hote_port(hote: &str, port: u16) -> String {
    if hote.contains(':') {
        format!("[{hote}]:{port}")
    } else {
        format!("{hote}:{port}")
    }
}

/// `/sante` repond-il, et est-ce un serveur Gescom ?
fn sante(hote: &str, port: u16) -> bool {
    let adresse = hote_port(hote, port);
    let Some(cible) = adresse.to_socket_addrs().ok().and_then(|mut a| a.next()) else { return false };
    let Ok(mut flux) = TcpStream::connect_timeout(&cible, Duration::from_millis(1500)) else { return false };
    flux.set_read_timeout(Some(Duration::from_secs(4))).ok();
    let requete = format!("GET /sante HTTP/1.1\r\nHost: {adresse}\r\nConnection: close\r\n\r\n");
    if flux.write_all(requete.as_bytes()).is_err() {
        return false;
    }
    let mut reponse = String::new();
    if flux.read_to_string(&mut reponse).is_err() {
        return false;
    }
    reponse
        .split_once("\r\n\r\n")
        .and_then(|(_, corps)| serde_json::from_str::<serde_json::Value>(corps).ok())
        .is_some_and(|v| v.get("version_protocole").is_some())
}

/// L'adresse de la console du serveur de ce poste, s'il repond.
fn trouver(r: &Reglage) -> Option<(Reglage, String)> {
    candidats(r)
        .into_iter()
        .find(|c| sante(&c.hote, c.port))
        .map(|c| {
            let url = format!("http://{}/console", hote_port(&c.hote, c.port));
            (c, url)
        })
}

/// L'etat du service Windows, lu par `sc query` : les codes d'etat sont
/// en anglais quelle que soit la langue de Windows (« RUNNING »).
fn etat_service() -> &'static str {
    let mut commande = std::process::Command::new("sc.exe");
    commande.args(["query", NOM_SERVICE]);
    sans_fenetre(&mut commande);
    let Ok(sortie) = commande.output() else { return "inconnu" };
    // 1060 : le service n'existe pas.
    if sortie.status.code() == Some(1060) {
        return "absent";
    }
    let texte = String::from_utf8_lossy(&sortie.stdout);
    if texte.contains("RUNNING") {
        "en_marche"
    } else if texte.contains("PENDING") {
        "en_cours"
    } else if texte.contains("STOPPED") {
        "arrete"
    } else if texte.contains("1060") {
        "absent"
    } else {
        "inconnu"
    }
}

/// Pas de fenetre noire pour `sc` ou PowerShell.
fn sans_fenetre(commande: &mut std::process::Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        commande.creation_flags(0x0800_0000);
    }
    #[cfg(not(windows))]
    let _ = commande;
}

/// `sc start GescomServeur`, eleve : Windows demande l'accord d'un
/// administrateur (UAC). Par PowerShell (`Start-Process -Verb RunAs`), qui
/// rend le code de `sc`, ou 1223 si l'accord est refuse.
fn demarrer_service_eleve() -> Result<String, String> {
    let script = format!(
        "try {{ $p = Start-Process -FilePath 'sc.exe' -ArgumentList 'start','{NOM_SERVICE}' -Verb RunAs \
         -WindowStyle Hidden -PassThru -Wait; exit $p.ExitCode }} catch {{ exit 1223 }}"
    );
    let mut commande = std::process::Command::new("powershell.exe");
    commande.args(["-NoProfile", "-NonInteractive", "-Command", &script]);
    sans_fenetre(&mut commande);
    let statut = commande.status().map_err(|e| format!("PowerShell introuvable : {e}"))?;
    match statut.code() {
        Some(0) => Ok("Le service démarre.".to_string()),
        Some(1056) => Ok("Le service tournait déjà.".to_string()),
        Some(1223) => Err("Accord administrateur refusé : le service n'a pas été démarré.".to_string()),
        Some(1060) => Err("Le service Gescom Serveur n'est pas installé sur ce poste.".to_string()),
        Some(1058) => Err("Le service Gescom Serveur est désactivé (services.msc).".to_string()),
        autre => Err(format!(
            "Le service n'a pas démarré (code {}). Son journal : %ProgramData%\\Gescom\\serveur.log",
            autre.map(|c| c.to_string()).unwrap_or_else(|| "?".to_string())
        )),
    }
}

/// La page qui appelle est-elle la page locale de la fenetre ? La console
/// du serveur, page distante, ne commande rien ici.
fn depuis_la_page_locale(webview: &tauri::Webview) -> Result<(), String> {
    let url = webview.url().map_err(|e| e.to_string())?;
    if url.scheme() == "tauri" || url.host_str() == Some("tauri.localhost") {
        Ok(())
    } else {
        Err("Commande réservée à la page de la fenêtre Gescom Serveur.".to_string())
    }
}

/// Ce que la page locale affiche.
#[derive(Serialize)]
struct Etat {
    trouve: bool,
    port: u16,
    adresse: String,
    /// `absent`, `arrete`, `en_cours`, `en_marche`, `inconnu`.
    service: String,
    configuration: String,
    /// Le port n'est pas dans la plage que les caisses joignent (D39).
    hors_plage: bool,
}

fn hors_plage(port: u16) -> bool {
    !(PORT_DEFAUT..=PORT_MAX).contains(&port)
}

/// Cherche le serveur de ce poste ; s'il repond, la fenetre affiche sa
/// console (et la page locale disparait).
#[tauri::command]
async fn chercher_serveur(webview: tauri::Webview) -> Result<Etat, String> {
    depuis_la_page_locale(&webview)?;
    let chemin = chemin_configuration();
    let (reglage, trouve) = tauri::async_runtime::spawn_blocking(move || {
        let r = lire_reglage(std::fs::read_to_string(&chemin).ok().as_deref());
        let t = trouver(&r);
        (r, t)
    })
    .await
    .map_err(|e| e.to_string())?;
    if let Some((c, url)) = trouve {
        let url = Url::parse(&url).map_err(|e| e.to_string())?;
        webview.navigate(url).map_err(|e| e.to_string())?;
        return Ok(Etat {
            trouve: true,
            port: c.port,
            adresse: hote_port(&c.hote, c.port),
            service: String::new(),
            configuration: chemin_configuration().display().to_string(),
            hors_plage: hors_plage(c.port),
        });
    }
    let service = tauri::async_runtime::spawn_blocking(etat_service).await.map_err(|e| e.to_string())?;
    Ok(Etat {
        trouve: false,
        port: reglage.port,
        adresse: hote_port(&reglage.hote, reglage.port),
        service: service.to_string(),
        configuration: chemin_configuration().display().to_string(),
        hors_plage: hors_plage(reglage.port),
    })
}

/// « Demarrer le service » : l'accord administrateur, puis `sc start`.
#[tauri::command]
async fn demarrer_service(webview: tauri::Webview) -> Result<String, String> {
    depuis_la_page_locale(&webview)?;
    tauri::async_runtime::spawn_blocking(demarrer_service_eleve).await.map_err(|e| e.to_string())?
}

/// Ramene la fenetre a sa page locale quand la console ne repond plus
/// (service arrete, poste en veille) : trois controles manques de suite,
/// une quinzaine de secondes. La console suit elle-meme un changement de
/// port ; ce fil relit l'adresse affichee a chaque tour.
fn surveiller(fenetre: tauri::WebviewWindow, locale: Url) {
    let mut manques = 0u32;
    loop {
        std::thread::sleep(Duration::from_secs(5));
        let Ok(url) = fenetre.url() else { return };
        let distante = url.scheme() == "http" && url.host_str() != Some("tauri.localhost");
        if !distante {
            manques = 0;
            continue;
        }
        let hote = url.host_str().unwrap_or("127.0.0.1").trim_start_matches('[').trim_end_matches(']').to_string();
        let port = url.port_or_known_default().unwrap_or(PORT_DEFAUT);
        if sante(&hote, port) {
            manques = 0;
            continue;
        }
        manques += 1;
        if manques >= 3 {
            manques = 0;
            let mut retour = locale.clone();
            retour.set_query(Some("perdu=1"));
            let _ = fenetre.navigate(retour);
        }
    }
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![chercher_serveur, demarrer_service])
        .setup(|app| {
            if let Some(fenetre) = app.get_webview_window("main") {
                if let Ok(locale) = fenetre.url() {
                    std::thread::spawn(move || surveiller(fenetre, locale));
                }
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Gescom Serveur n'a pas pu ouvrir sa fenêtre");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_reglage_se_lit_dans_serveur_json_ou_retombe_sur_40000() {
        assert_eq!(lire_reglage(None), Reglage { hote: "127.0.0.1".into(), port: PORT_DEFAUT });
        assert_eq!(lire_reglage(Some("pas du json")).port, PORT_DEFAUT);
        let r = lire_reglage(Some("\u{feff}{\"base\":\"C:\\\\x.db\",\"port\":40007,\"hote\":\"0.0.0.0\"}"));
        assert_eq!(r, Reglage { hote: "127.0.0.1".into(), port: 40007 });
        assert_eq!(lire_reglage(Some("{\"port\":40003,\"hote\":\"127.0.0.1\"}")).hote, "127.0.0.1");
        assert_eq!(lire_reglage(Some("{\"port\":40003,\"hote\":\"192.168.1.20\"}")).hote, "192.168.1.20");
        assert_eq!(lire_reglage(Some("{\"port\":0}")).port, PORT_DEFAUT);
        assert_eq!(lire_reglage(Some("{\"port\":70000}")).port, PORT_DEFAUT);
    }

    #[test]
    fn on_essaie_le_reglage_puis_le_port_par_defaut() {
        let r = Reglage { hote: "127.0.0.1".into(), port: 40005 };
        assert_eq!(candidats(&r).iter().map(|c| c.port).collect::<Vec<_>>(), vec![40005, PORT_DEFAUT]);
        let d = Reglage { hote: "127.0.0.1".into(), port: PORT_DEFAUT };
        assert_eq!(candidats(&d).len(), 1);
        assert_eq!(hote_port("::1", 40000), "[::1]:40000");
        assert!(!hors_plage(40000) && !hors_plage(40100) && hors_plage(7300) && hors_plage(40101));
    }
}
