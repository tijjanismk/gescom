//! Point d'entrée Tauri — enregistrement de toutes les commandes.

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
            std::fs::create_dir_all(&data_dir)
                .expect("Création du répertoire de données");
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
            // ---- Ventes & clients ----
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
            // ---- Fournisseurs & stock ----
            commandes::fournisseurs::lire_fournisseurs,
            commandes::fournisseurs::lire_fournisseurs_avec_dettes,
            commandes::fournisseurs::creer_fournisseur,
            commandes::fournisseurs::enregistrer_entree_stock,
            commandes::fournisseurs::enregistrer_ajustement_inventaire,
            // ---- Paramètres articles ----
            commandes::parametres::lire_categories,
            commandes::parametres::creer_categorie,
            commandes::parametres::lire_articles_complets,
            commandes::parametres::creer_article_complet,
            commandes::parametres::lire_stocks,
            // ---- Retours & avoirs ----
            commandes::retours::lire_ventes_recentes,
            commandes::retours::enregistrer_retour,
            commandes::retours::lire_avoirs_ouverts_tous,
            // ---- Avoirs à la vente & scanner ----
            commandes::avoirs::lire_avoirs_client,
            commandes::avoirs::total_avoirs_client,
            commandes::avoirs::appliquer_avoir_vente,
            commandes::avoirs::chercher_article_par_code_barre,
            commandes::avoirs::lire_config_scanner,
            commandes::avoirs::sauvegarder_config_scanner,
            commandes::avoirs::sauvegarder_code_barre_article,
            commandes::avoirs::lire_articles_avec_codes_barres,
            // ---- Pagination & filtres ----
            commandes::pagination::lire_ventes_paginees,
            commandes::pagination::lire_clients_pagines,
            commandes::pagination::lire_stocks_pagines,
            commandes::pagination::lire_fournisseurs_pagines,
            commandes::pagination::lire_ventes_recentes_paginee,
            // ---- Auth ----
            commandes::auth::connexion,
            commandes::auth::changer_mot_de_passe,
            commandes::auth::creer_utilisateur,
            commandes::auth::lire_utilisateurs,
            // ---- Société & factures ----
            commandes::societe::lire_parametres_societe,
            commandes::societe::sauvegarder_parametres_societe,
            commandes::societe::lire_donnees_facture,
            // ---- Logo ----
            commandes::logo::sauvegarder_logo,
            commandes::logo::lire_logo_base64,
            commandes::logo::supprimer_logo,
            // ---- Impression ----
            commandes::impression::imprimer_facture,
            // ---- Caisse ----
            commandes::caisse::lire_resume_caisse,
            commandes::caisse::lire_mouvements_caisse_du_jour,
            commandes::caisse::ouvrir_session_caisse,
            commandes::caisse::fermer_session_caisse,
            // ---- Dashboard ----
            commandes::dashboard::lire_resume_dashboard,
            commandes::dashboard::lire_ventes_du_jour,
            // ---- Sauvegarde ----
            commandes::sauvegarde::sauvegarder_base,
            commandes::sauvegarde::lire_config_sauvegarde,
            commandes::sauvegarde::sauvegarder_config_sauvegarde,
            // Creances
            commandes::creances::lire_creances_ouvertes,
            commandes::creances::regler_creance,
            // ---- Chantiers §14 ----
            commandes::chantiers::lire_taux_tva,
            commandes::chantiers::sauvegarder_tva_article,
            commandes::chantiers::lire_resume_tva,
            commandes::chantiers::lire_dettes_fournisseurs,
            commandes::chantiers::regler_dette_fournisseur,
            commandes::chantiers::marquer_irrecouvrable,
            commandes::chantiers::lire_irrecouvrable,
            commandes::chantiers::lire_config_avoirs,
            commandes::chantiers::sauvegarder_config_avoirs,
            commandes::chantiers::expirer_avoirs,
            // ---- Pièces commerciales ----
            commandes::pieces::lire_toutes_pieces_client,
            commandes::pieces::lire_pieces_client,
            commandes::pieces::lire_lignes_piece,
            commandes::pieces::creer_piece,
            commandes::pieces::convertir_piece,
            commandes::pieces::changer_statut_piece,
            commandes::pieces::lire_donnees_piece,
            commandes::pieces::lire_fiche_client,
            commandes::pieces::imprimer_piece,
        ])
        .run(tauri::generate_context!())
        .expect("Erreur lors du démarrage de l'application");
}