//! Ce que le serveur doit dire de lui-même au démarrage.
//!
//! ## Pourquoi ce module existe
//!
//! Deux questions tombent au moment d'installer un poste caisse, et
//! aucune n'a de réponse évidente pour un commerçant :
//!
//! - **quelle adresse taper ?** « 0.0.0.0:7300 » n'est pas une adresse
//!   qu'on saisit ailleurs, c'est ce sur quoi le serveur écoute ;
//! - **pourquoi la caisse ne joint-elle rien ?** Le pare-feu de Windows
//!   bloque en silence : pas de message, pas de journal, juste un
//!   « Serveur injoignable » du côté de la caisse.
//!
//! Le serveur répond donc aux deux tout seul, à chaque démarrage.

/// L'adresse de ce poste sur le réseau local.
///
/// On ouvre une socket UDP vers une adresse extérieure sans **rien
/// envoyer** : cela suffit à faire choisir à Windows l'interface qu'il
/// utiliserait, donc l'adresse que les autres postes voient. Énumérer
/// les cartes réseau donnerait aussi les adaptateurs virtuels de
/// VMware, Hyper-V et Bluetooth — et le commerçant recopierait la
/// mauvaise.
///
/// `None` s'il n'y a pas de route : une machine sans réseau ne sert
/// pas de serveur, et le dire vaut mieux que d'inventer une adresse.
pub fn adresse_locale() -> Option<String> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    // Adresse de documentation (RFC 5737) : elle n'est jamais routée,
    // et aucun paquet ne part. On ne veut que la décision de routage.
    socket.connect("192.0.2.1:9").ok()?;
    let addr = socket.local_addr().ok()?;
    let ip = addr.ip().to_string();
    if ip == "0.0.0.0" {
        return None;
    }
    Some(ip)
}

/// Le pare-feu laisse-t-il entrer les caisses ?
///
/// Vérifié à chaque démarrage parce que la règle est le point de panne
/// le plus probable d'une installation multiposte, et le plus muet :
/// Windows refuse la connexion sans rien écrire nulle part.
///
/// `None` quand on ne peut pas savoir — on n'affiche alors rien plutôt
/// que d'inquiéter pour rien.
#[cfg(windows)]
pub fn regle_parefeu_presente(port: u16) -> Option<bool> {
    use std::process::Command;

    let sortie = Command::new("netsh")
        .args(["advfirewall", "firewall", "show", "rule", "name=Gescom serveur"])
        .output()
        .ok()?;
    let texte = String::from_utf8_lossy(&sortie.stdout);

    // `netsh` rend 1 et « Aucune règle » quand elle n'existe pas.
    if !sortie.status.success() {
        return Some(false);
    }
    // La règle peut exister sur un autre port qu'on aurait change en
    // ligne de commande : on verifie que c'est bien celui-la.
    Some(texte.contains(&port.to_string()))
}

#[cfg(not(windows))]
pub fn regle_parefeu_presente(_port: u16) -> Option<bool> {
    None
}

/// Ce qu'on affiche au démarrage, sous l'adresse d'écoute.
pub fn conseils(port: u16) -> Vec<String> {
    let mut lignes = Vec::new();

    match adresse_locale() {
        Some(ip) => lignes.push(format!(
            "  pour les caisses : {ip}:{port}   ← l'adresse à saisir dans \
             Paramètres → Réseau"
        )),
        None => lignes.push(
            "  ⚠ aucune adresse réseau : ce poste n'est joignable que par \
             lui-même."
                .to_string(),
        ),
    }

    if regle_parefeu_presente(port) == Some(false) {
        lignes.push(String::new());
        lignes.push(
            "  ⚠ PARE-FEU : aucune règle « Gescom serveur » n'autorise le port"
                .to_string(),
        );
        lignes.push(format!(
            "    {port}. Les autres postes recevront « Serveur injoignable »"
        ));
        lignes.push("    sans autre explication.".to_string());
        lignes.push(String::new());
        lignes.push(
            "    Ouvrir PowerShell EN ADMINISTRATEUR sur ce poste, puis :"
                .to_string(),
        );
        lignes.push("      .\\outils\\parefeu.ps1 -Ouvrir".to_string());
    }

    lignes
}
