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
pub mod auteur;
pub mod documents;
pub mod impression;
pub mod historique;
pub mod plafonds;
pub mod coeur;
pub mod persistance;
pub mod portes;

pub mod protocole;
// La decouverte du serveur en UDP (format pur, partage) et le journal
// d'un changement de port du serveur (27/10/2026).
pub mod decouverte;
pub mod reseau_serveur;
pub mod postes;
pub mod codebarre;
pub mod transferts;
pub mod relances;
pub mod rapports;
pub mod retours;
pub mod seed;
pub mod roles;
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
pub mod fournisseurs_csv;
pub mod pieces;
pub mod pieces_pos;
pub mod livraisons;
pub mod sessions;
pub mod societe;
pub mod tableau_bord;
pub mod amorcage;
pub mod base;
pub mod dossiers;
pub mod acces_dossiers;
pub mod plan_comptable;
pub mod affectations;
pub mod journaux_comptables;
pub mod personnel;
pub mod presences;
pub mod avances;
pub mod paie;
pub mod crm;
pub mod equipe_jour;
pub mod passage;
pub mod suivi_stock;
pub mod menu;
pub mod assistant;
pub mod horloge;
pub mod caisses;
pub mod argent;
pub mod catalogue;
pub mod catalogue_csv;
pub mod comptoir;
pub mod images;
pub mod installation;
pub mod registre;

/// Version du protocole client/serveur.
///
/// Un poste caisse qui ne parle pas la meme version que le serveur est
/// refuse a la connexion, avec un message clair. Le contraire — laisser
/// passer et esperer — produit des champs manquants lus comme des
/// zeros, donc des montants faux.
pub const VERSION_PROTOCOLE: u32 = 1;
