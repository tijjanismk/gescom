//! Commandes Tauri pour les ventes et paiements.

use tauri::State;
use std::sync::Mutex;
use rusqlite::Connection;

// =====================================================================
//  État partagé
// =====================================================================

pub struct EtatApp {
    pub conn: Mutex<Connection>,
}

// =====================================================================
//  Utilitaire utilisateur
// =====================================================================

// Ces trois-la vivent desormais dans `noyau::argent` : le serveur v2
// execute le meme code que le comptoir. On les re-exporte pour ne pas
// toucher aux 27 fichiers de commandes qui les appellent.
#[allow(unused_imports)]
pub use gescom_noyau::argent::{id_utilisateur_courant_pub, id_utilisateur_par_role};

/// Récupère l'id utilisateur selon son rôle — pour le multi-utilisateur.

// =====================================================================
//  CLIENTS
// =====================================================================

// Les lectures du comptoir vivent dans `noyau::catalogue` : le serveur
// v2 les expose aux postes caisse, et deux copies auraient fini par ne
// plus donner le meme catalogue au comptoir et a la caisse.
#[tauri::command]
pub fn lire_clients(etat: State<EtatApp>) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    crate::coeur_catalogue::lire_clients(&conn)
}

#[tauri::command]
pub fn lire_client_generique(etat: State<EtatApp>) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    crate::coeur_catalogue::lire_client_generique(&conn)
}

#[tauri::command]
pub fn creer_client_rapide(
    etat: State<EtatApp>,
    nom: String,
    telephone: Option<String>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::comptoir::creer_client_rapide(&conn, nom, telephone)
}

/// Modifie les coordonnees d'un client.
///
/// Le `code` n'est PAS modifiable : il est imprime sur les pieces deja
/// remises et sert de reference au comptoir. Le changer ferait mentir
/// tous les documents anterieurs.
///
/// Le client generique non plus : il est le pot commun des ventes au
/// comptant (D40), le renommer rendrait le journal illisible.
#[tauri::command]
pub fn modifier_client(
    etat: State<EtatApp>,
    client_id: String,
    nom: String,
    telephone: Option<String>,
    adresse: Option<String>,
    email: Option<String>,
    nif: Option<String>,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::comptoir::modifier_client(
        &conn, client_id, nom, telephone, adresse, email, nif,
    )
}

// =====================================================================
//  ARTICLES
// =====================================================================

#[tauri::command]
pub fn lire_articles_avec_unites(
    etat: State<EtatApp>,
    role: Option<String>,
    // Depot dont on veut le stock. Absent -> depot par defaut.
    //
    // Sans ce parametre, `stock` etait TOUJOURS celui du depot par
    // defaut : sur un autre magasin, le POS affichait « Rupture » sur
    // un article present, et posait le drapeau `vente_a_decouvert` au
    // hasard — l'ecran « ventes a decouvert » devenait faux.
    depot_id: Option<String>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    lire_articles_avec_unites_sur(&conn, role, depot_id)
}

/// Logique de `lire_articles_avec_unites`, sur une connexion quelconque.
///
/// Separee de la commande pour etre jouable sur une base de test :
/// les scenarios de `tests_multi_depot` verifient ce que le SQL fait
/// reellement a la base, ce qu'aucun test de formule ne montre.
/// Conserve pour `tests_multi_depot`, qui joue sur une connexion nue.
/// Le corps a demenage dans `noyau::catalogue`, d'ou le serveur le sert
/// aussi aux postes caisse.
pub(crate) fn lire_articles_avec_unites_sur(
    conn: &rusqlite::Connection,
    role: Option<String>,
    depot_id: Option<String>,
) -> Result<Vec<serde_json::Value>, String> {
    crate::coeur_catalogue::lire_articles_avec_unites(conn, role, depot_id)
}

#[tauri::command]
pub fn lire_depots(etat: State<EtatApp>) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    crate::coeur_catalogue::lire_depots(&conn)
}

#[tauri::command]
pub fn lire_depot_defaut(etat: State<EtatApp>) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    crate::coeur_catalogue::lire_depot_defaut(&conn)
}

#[tauri::command]
pub fn creer_article_rapide(
    etat: State<EtatApp>,
    nom: String,
    unite_base: String,
    prix_reference: i64,
    prix_achat: Option<i64>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::comptoir::creer_article_rapide(
        &conn, nom, unite_base, prix_reference, prix_achat,
    )
}

// =====================================================================
//  VENTES
// =====================================================================

pub use gescom_noyau::argent::ParamsLigneInput;

#[tauri::command]
pub fn creer_vente(
    etat: State<EtatApp>,
    client_id: String,
    depot_id: String,
    mode_reglement: String,
    lignes: Vec<ParamsLigneInput>,
    utilisateur_role: Option<String>,
    // montant_paye  : encaisse immediatement (comptant integral ou acompte)
    // mode_paiement : especes | orange_money | moov_money | cheque
    // avoir_montant : montant d'avoir client a consommer sur cette vente
    montant_paye: Option<i64>,
    mode_paiement: Option<String>,
    avoir_montant: Option<i64>,
) -> Result<serde_json::Value, String> {
    let mut conn = etat.conn.lock().map_err(|e| e.to_string())?;
    creer_vente_sur(
        &mut conn, client_id, depot_id, mode_reglement, lignes, utilisateur_role, montant_paye, mode_paiement, avoir_montant,
    )
}

/// Logique de `creer_vente`, sur une connexion quelconque.
///
/// Separee de la commande pour etre jouable sur une base de test :
/// les scenarios de `tests_multi_depot` verifient ce que le SQL fait
/// reellement a la base, ce qu'aucun test de formule ne montre.
/// Conserve pour `tests_multi_depot`. Le corps a demenage dans
/// `noyau::argent`, d'ou le serveur le sert aussi aux postes caisse.
pub(crate) use gescom_noyau::argent::creer_vente_sur;

// =====================================================================
//  PAIEMENTS
// =====================================================================

#[tauri::command]
pub fn enregistrer_paiement(
    etat: State<EtatApp>,
    vente_id: String,
    montant: i64,
    mode: String,
    utilisateur_role: Option<String>,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::argent::enregistrer_paiement(&conn, vente_id, montant, mode, utilisateur_role)
}

// =====================================================================
//  CRÉANCES CLIENTS
// =====================================================================

#[tauri::command]
pub fn lire_clients_avec_creances(
    etat: State<EtatApp>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::comptoir::lire_clients_avec_creances(&conn)
}
