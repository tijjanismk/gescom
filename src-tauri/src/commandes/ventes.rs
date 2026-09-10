//! Commandes Tauri pour les ventes et paiements.

use tauri::State;
use std::sync::Mutex;
use rusqlite::Connection;
use crate::utils::maintenant_iso;

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
    if nom.trim().is_empty() {
        return Err("Le nom est obligatoire".to_string());
    }

    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let est_generique: i64 = conn.query_row(
        "SELECT est_generique FROM client WHERE id = ?1",
        rusqlite::params![client_id], |r| r.get(0),
    ).map_err(|_| "Client introuvable".to_string())?;

    if est_generique == 1 {
        return Err(
            "Le client générique ne se modifie pas : il regroupe toutes les \
             ventes au comptant.".to_string()
        );
    }

    let now = maintenant_iso();
    let auteur = id_utilisateur_courant_pub(&conn);

    // `vide` plutot que la chaine vide : un champ efface doit redevenir
    // NULL, sinon les ecrans affichent une ligne vide au lieu de rien.
    let vide = |o: Option<String>| o.filter(|s| !s.trim().is_empty());

    conn.execute(
        "UPDATE client
         SET nom = ?1, telephone = ?2, adresse = ?3, email = ?4, nif = ?5,
             modifie_le = ?6, modifie_par = ?7
         WHERE id = ?8",
        rusqlite::params![
            nom.trim(), vide(telephone), vide(adresse), vide(email), vide(nif),
            now, auteur, client_id
        ],
    ).map_err(|e| e.to_string())?;

    conn.execute(
        "INSERT INTO journal
         (id, type_evenement, entite_type, entite_id, auteur_id,
          nouveau_valeur, origine, date_evenement)
         VALUES (?1,'client_modifie','client',?2,?3,?4,'app',?5)",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(), client_id, auteur,
            format!(r#"{{"nom":"{}"}}"#, nom.trim().replace('"', "'")),
            now
        ],
    ).ok();

    Ok(())
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
    let mut stmt = conn.prepare(
        "SELECT c.id, c.code, c.nom, c.telephone,
                CAST(COALESCE(SUM(
                  CASE WHEN v.statut != 'payee' THEN
                    (SELECT COALESCE(SUM(prix_pratique * quantite), 0)
                     FROM ligne_vente WHERE vente_id = v.id) -
                    (SELECT COALESCE(SUM(montant), 0)
                     FROM paiement WHERE vente_id = v.id)
                  ELSE 0 END
                ), 0) AS INTEGER) as total_creances,
                COUNT(DISTINCT v.id) as nb_ventes
         FROM client c
         LEFT JOIN vente v ON v.client_id = c.id
         WHERE c.actif = 1 AND c.est_generique = 0
         GROUP BY c.id
         ORDER BY total_creances DESC, c.nom ASC"
    ).map_err(|e| e.to_string())?;

    let x = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_,String>(0)?,
            "code": row.get::<_,String>(1)?,
            "nom": row.get::<_,String>(2)?,
            "telephone": row.get::<_,Option<String>>(3)?,
            "total_creances": row.get::<_,i64>(4)?,
            "nb_ventes": row.get::<_,i64>(5)?,
        }))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
    Ok(x)
}