//! Point d'entrée Tauri — enregistrement des commandes.

mod utils;
mod coeur;
mod persistance;
mod commandes;
mod seed;

use std::sync::Mutex;
use tauri::Manager;
use commandes::ventes::EtatApp;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // Chemin de la base de données
            let data_dir = app.path().app_data_dir()
                .expect("Impossible de trouver le répertoire de données");
            std::fs::create_dir_all(&data_dir).expect("Création du répertoire de données");
            let db_path = data_dir.join("gescom.db");

            // Ouvrir et initialiser la base
            let conn = persistance::ouvrir_base(db_path.to_str().unwrap())
                .expect("Impossible d'ouvrir la base de données");
            persistance::initialiser_tables(&conn)
                .expect("Impossible d'initialiser les tables");

            // Seeder si la base est vide
            if seed::base_est_vide(&conn) {
                seed::seeder(&conn).expect("Erreur lors du seeding");
            }

            // Partager la connexion
            app.manage(EtatApp { conn: Mutex::new(conn) });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Ventes & clients
            commandes::ventes::lire_clients,
            commandes::ventes::lire_client_generique,
            commandes::ventes::creer_client_rapide,
            commandes::ventes::lire_articles_avec_unites,
            commandes::ventes::lire_depots,
            commandes::ventes::lire_depot_defaut,
            commandes::ventes::creer_article_rapide,
            commandes::ventes::creer_vente,
            commandes::ventes::enregistrer_paiement,
            commandes::ventes::lire_clients_avec_creances,
            // Fournisseurs & stock
            commandes::fournisseurs::lire_fournisseurs,
            commandes::fournisseurs::lire_fournisseurs_avec_dettes,
            commandes::fournisseurs::creer_fournisseur,
            commandes::fournisseurs::enregistrer_entree_stock,
            commandes::fournisseurs::enregistrer_ajustement_inventaire,
            // Paramètres articles
            commandes::parametres::lire_categories,
            commandes::parametres::creer_categorie,
            commandes::parametres::lire_articles_complets,
            commandes::parametres::creer_article_complet,
            commandes::parametres::lire_stocks,
            // Retours & avoirs
            commandes::retours::lire_ventes_recentes,
            commandes::retours::enregistrer_retour,
            commandes::retours::lire_avoirs_ouverts_tous,
            // Pagination & filtres
            commandes::pagination::lire_ventes_paginees,
            commandes::pagination::lire_clients_pagines,
            commandes::pagination::lire_stocks_pagines,
            commandes::pagination::lire_fournisseurs_pagines,
            commandes::pagination::lire_ventes_recentes_paginee,
            // Auth
            commandes::auth::connexion,
            commandes::auth::changer_mot_de_passe,
            commandes::auth::creer_utilisateur,
            commandes::auth::lire_utilisateurs,
            // Société & factures
            commandes::societe::lire_parametres_societe,
            commandes::societe::sauvegarder_parametres_societe,
            commandes::societe::lire_donnees_facture,
            // Caisse
            commandes::caisse::lire_resume_caisse,
            commandes::caisse::lire_mouvements_caisse_du_jour,
            commandes::caisse::ouvrir_session_caisse,
            commandes::caisse::fermer_session_caisse,
            // Dashboard
            commandes::dashboard::lire_resume_dashboard,
            commandes::dashboard::lire_ventes_du_jour,
            // Sauvegarde
            commandes::sauvegarde::sauvegarder_base,
            commandes::sauvegarde::lire_config_sauvegarde,
            commandes::sauvegarde::sauvegarder_config_sauvegarde,
            // facture
            commandes::impression::imprimer_facture,
            //logo
            commandes::logo::sauvegarder_logo,
            commandes::logo::lire_logo_base64,
            commandes::logo::supprimer_logo,
        ])
        .run(tauri::generate_context!())
        .expect("Erreur lors du démarrage de l'application");
}
