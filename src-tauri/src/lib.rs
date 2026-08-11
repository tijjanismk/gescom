pub mod coeur;
pub mod persistance;
pub mod porte;
pub mod utils;
pub mod commandes;
pub mod seed;  // ← ajoute cette ligne

use commandes::ventes::EtatApp;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let chemin = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("gescom")
        .join("gescom.db");

    std::fs::create_dir_all(chemin.parent().unwrap()).unwrap();

    let conn = rusqlite::Connection::open(&chemin).unwrap();
    conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
    persistance::initialiser_tables(&conn).unwrap();

    // Seed au premier démarrage si la base est vide.
    if seed::base_est_vide(&conn) {
        seed::seeder(&conn).expect("Erreur lors du seed");
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(EtatApp {
            conn: std::sync::Mutex::new(conn),
        })
        .invoke_handler(tauri::generate_handler![
            commandes::ventes::creer_vente,
            commandes::ventes::enregistrer_paiement,
            commandes::ventes::lire_clients,
            commandes::ventes::lire_client_generique,
            commandes::ventes::lire_depot_defaut,
            commandes::ventes::lire_articles_avec_unites,
            commandes::ventes::creer_client_rapide,
            commandes::ventes::creer_article_rapide,
            commandes::dashboard::lire_resume_dashboard,
            commandes::dashboard::lire_stocks,
            commandes::dashboard::lire_clients_avec_creances,
            commandes::dashboard::lire_resume_caisse,
            commandes::dashboard::ouvrir_session_caisse,
            commandes::dashboard::fermer_session_caisse,
            commandes::parametres::lire_categories,
            commandes::parametres::creer_categorie,
            commandes::parametres::lire_articles_complets,
            commandes::parametres::creer_article_complet,
            commandes::parametres::creer_unite_vente,
            commandes::fournisseurs::lire_fournisseurs,
            commandes::fournisseurs::lire_fournisseurs_avec_dettes,
            commandes::fournisseurs::creer_fournisseur,
            commandes::fournisseurs::enregistrer_entree_stock,
            commandes::fournisseurs::enregistrer_ajustement_inventaire,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}