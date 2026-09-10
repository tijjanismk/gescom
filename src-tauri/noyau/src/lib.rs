//! Noyau Gescom — tout ce qui ne depend pas de l'interface.
//!
//! Ce crate existe parce que le v2 a deux executables : le client
//! (fenetre Tauri) et le serveur (sans fenetre). Les deux ont besoin
//! des memes calculs, du meme schema et du meme protocole. Les avoir
//! en double, c'est se garantir qu'ils divergeront — et une divergence
//! entre le poste caisse et le serveur se solde par un compte faux.
//!
//! Regle : rien ici ne connait `tauri`. Si un module en a besoin, il
//! n'appartient pas au noyau.

pub mod utils;
pub mod coeur;
pub mod persistance;
pub mod portes;

pub mod protocole;
pub mod postes;
pub mod sessions;
pub mod caisses;
pub mod registre;

/// Version du protocole client/serveur.
///
/// Un poste caisse qui ne parle pas la meme version que le serveur est
/// refuse a la connexion, avec un message clair. Le contraire — laisser
/// passer et esperer — produit des champs manquants lus comme des
/// zeros, donc des montants faux.
pub const VERSION_PROTOCOLE: u32 = 1;
