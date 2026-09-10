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
pub mod codebarre;
pub mod transferts;
pub mod relances;
pub mod rapports;
pub mod retours;
pub mod sauvegarde;
pub mod cheques;
pub mod caisse;
pub mod parametres;
pub mod auth;
pub mod journal;
pub mod pagination;
pub mod creances;
pub mod achats;
pub mod avoirs;
pub mod chantiers;
pub mod depots;
pub mod fournisseurs;
pub mod pieces;
pub mod pieces_pos;
pub mod livraisons;
pub mod sessions;
pub mod societe;
pub mod tableau_bord;
pub mod caisses;
pub mod argent;
pub mod catalogue;
pub mod catalogue_csv;
pub mod comptoir;
pub mod modeles;
pub mod installation;
pub mod registre;

/// Version du protocole client/serveur.
///
/// Un poste caisse qui ne parle pas la meme version que le serveur est
/// refuse a la connexion, avec un message clair. Le contraire — laisser
/// passer et esperer — produit des champs manquants lus comme des
/// zeros, donc des montants faux.
pub const VERSION_PROTOCOLE: u32 = 1;
