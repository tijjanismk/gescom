//! Un serveur HTTP/1.1 minimal, sans dependance.
//!
//! ## Pourquoi pas axum
//!
//! Le besoin tient en trois routes et une dizaine de postes sur un
//! reseau local de boutique. Un runtime asynchrone apporterait ici sa
//! complexite sans son benefice : SQLite serialise de toute facon les
//! ecritures derriere un verrou, et un fil par connexion suffit
//! largement a dix caisses.
//!
//! Le gain reel est ailleurs : zero crate a telecharger. Le poste de
//! developpement est a Bamako, la connexion n'est pas garantie, et un
//! `cargo build` qui exige le reseau est un `cargo build` qui echoue
//! le jour ou on en a besoin.
//!
//! Ce que ce module ne fait PAS, et qu'il ne faut pas lui demander :
//! ni TLS, ni HTTP/2, ni transfert par morceaux, ni keep-alive. Il
//! parle a un reseau local ferme, derriere le routeur de la boutique.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;

pub struct Requete {
    pub methode: String,
    pub chemin: String,
    pub parametres: HashMap<String, String>,
    pub entetes: HashMap<String, String>,
    pub corps: Vec<u8>,
    pub ip: String,
}

impl Requete {
    /// Le jeton porte par `Authorization: Bearer ...`.
    pub fn jeton(&self) -> Option<&str> {
        self.entetes
            .get("authorization")
            .and_then(|v| v.strip_prefix("Bearer "))
            .map(str::trim)
    }

    pub fn json(&self) -> Result<serde_json::Value, String> {
        if self.corps.is_empty() {
            return Ok(serde_json::Value::Null);
        }
        serde_json::from_slice(&self.corps).map_err(|e| format!("Corps JSON illisible : {e}"))
    }
}

/// Taille maximale d'un corps de requete : 8 Mio.
///
/// Un logo ou un en-tete de facture en base64 passe largement. Au-dela,
/// c'est une erreur ou une attaque, et lire sans borne offrirait a
/// n'importe qui sur le reseau de faire tomber le serveur par la
/// memoire.
const CORPS_MAX: usize = 8 * 1024 * 1024;

pub fn lire_requete(flux: &TcpStream) -> Result<Requete, String> {
    let ip = flux
        .peer_addr()
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|_| "?".to_string());
    let mut lecteur = BufReader::new(flux);

    let mut ligne = String::new();
    lecteur
        .read_line(&mut ligne)
        .map_err(|e| format!("Lecture : {e}"))?;
    let mut mots = ligne.split_whitespace();
    let methode = mots.next().unwrap_or("").to_string();
    let cible = mots.next().unwrap_or("/").to_string();
    if methode.is_empty() {
        return Err("Requete vide".to_string());
    }

    let (chemin, parametres) = decouper_cible(&cible);

    let mut entetes = HashMap::new();
    loop {
        let mut l = String::new();
        let n = lecteur.read_line(&mut l).map_err(|e| e.to_string())?;
        if n == 0 || l.trim().is_empty() {
            break;
        }
        if let Some((cle, valeur)) = l.split_once(':') {
            // Les noms d'entete sont insensibles a la casse ; on
            // normalise a la lecture pour ne pas avoir a y penser
            // ensuite.
            entetes.insert(cle.trim().to_lowercase(), valeur.trim().to_string());
        }
    }

    let taille: usize = entetes
        .get("content-length")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    if taille > CORPS_MAX {
        return Err(format!("Corps trop volumineux ({taille} octets)"));
    }
    let mut corps = vec![0u8; taille];
    if taille > 0 {
        lecteur
            .read_exact(&mut corps)
            .map_err(|e| format!("Corps tronque : {e}"))?;
    }

    Ok(Requete { methode, chemin, parametres, entetes, corps, ip })
}

fn decouper_cible(cible: &str) -> (String, HashMap<String, String>) {
    let mut parametres = HashMap::new();
    let (chemin, requete) = match cible.split_once('?') {
        Some((c, q)) => (c, Some(q)),
        None => (cible, None),
    };
    if let Some(q) = requete {
        for paire in q.split('&').filter(|p| !p.is_empty()) {
            let (cle, valeur) = paire.split_once('=').unwrap_or((paire, ""));
            parametres.insert(decoder(cle), decoder(valeur));
        }
    }
    (chemin.to_string(), parametres)
}

fn decoder(s: &str) -> String {
    let octets = s.as_bytes();
    let mut sortie = Vec::with_capacity(octets.len());
    let mut i = 0;
    while i < octets.len() {
        match octets[i] {
            b'+' => {
                sortie.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < octets.len() => {
                let h = std::str::from_utf8(&octets[i + 1..i + 3]).unwrap_or("");
                match u8::from_str_radix(h, 16) {
                    Ok(o) => {
                        sortie.push(o);
                        i += 3;
                    }
                    Err(_) => {
                        sortie.push(octets[i]);
                        i += 1;
                    }
                }
            }
            o => {
                sortie.push(o);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&sortie).into_owned()
}

pub fn repondre_json(
    flux: &mut TcpStream,
    code: u16,
    corps: &serde_json::Value,
) -> std::io::Result<()> {
    let texte = serde_json::to_vec(corps).unwrap_or_else(|_| b"{}".to_vec());
    ecrire(flux, code, "application/json; charset=utf-8", &texte)
}

pub fn repondre_texte(flux: &mut TcpStream, code: u16, texte: &str) -> std::io::Result<()> {
    ecrire(flux, code, "text/plain; charset=utf-8", texte.as_bytes())
}

fn ecrire(
    flux: &mut TcpStream,
    code: u16,
    type_contenu: &str,
    corps: &[u8],
) -> std::io::Result<()> {
    let raison = match code {
        200 => "OK",
        204 => "No Content",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        409 => "Conflict",
        413 => "Payload Too Large",
        426 => "Upgrade Required",
        _ => "Internal Server Error",
    };
    // La fenetre Tauri a pour origine `tauri://localhost` : sans
    // en-tetes CORS, le navigateur embarque refuse la reponse sans que
    // le serveur voie quoi que ce soit. Le reseau vise etant un LAN
    // ferme, l'origine est ouverte ; c'est le jeton qui protege, pas
    // l'origine.
    let entete = format!(
        "HTTP/1.1 {code} {raison}\r\n\
         Content-Type: {type_contenu}\r\n\
         Content-Length: {}\r\n\
         Access-Control-Allow-Origin: *\r\n\
         Access-Control-Allow-Headers: Content-Type, Authorization\r\n\
         Access-Control-Allow-Methods: GET, POST, OPTIONS\r\n\
         Cache-Control: no-store\r\n\
         Connection: close\r\n\r\n",
        corps.len()
    );
    flux.write_all(entete.as_bytes())?;
    flux.write_all(corps)?;
    flux.flush()
}
