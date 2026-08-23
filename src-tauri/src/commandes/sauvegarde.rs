//! Sauvegarde automatique de la base SQLite.
//!
//! Copie la base vers un dossier configurable (clé USB, autre disque).
//! Nommage : gescom_backup_2026-08-17_14-30.db
//! Configurable dans les paramètres société.

use tauri::State;
use crate::commandes::ventes::EtatApp;
use crate::utils::maintenant_iso;
use std::path::PathBuf;

#[tauri::command]
pub fn sauvegarder_base(
    etat: State<EtatApp>,
    dossier_destination: String,
) -> Result<String, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    // Lire le chemin de la base depuis la connexion.
    let chemin_base: String = conn.query_row(
        "PRAGMA database_list", [],
        |row| row.get(2),
    ).map_err(|e| e.to_string())?;

    if chemin_base.is_empty() {
        return Err("Base de données en mémoire — sauvegarde impossible".to_string());
    }

    let source = PathBuf::from(&chemin_base);
    if !source.exists() {
        return Err(format!("Fichier source introuvable : {}", chemin_base));
    }

    // Créer le dossier destination si nécessaire.
    let dest_dir = PathBuf::from(&dossier_destination);
    if !dest_dir.exists() {
        std::fs::create_dir_all(&dest_dir)
            .map_err(|e| format!("Impossible de créer le dossier : {}", e))?;
    }

    // Nom du fichier de backup avec horodatage.
    let horodatage = chrono::Local::now().format("%Y-%m-%d_%H-%M").to_string();
    let nom_fichier = format!("gescom_backup_{}.db", horodatage);
    let dest = dest_dir.join(&nom_fichier);

    // Copie via SQLite VACUUM INTO — copie propre et coherente, y
    // compris le WAL (une copie de gescom.db seul donnerait une base
    // vide, les ecritures recentes vivant dans gescom.db-wal).
    //
    // Le chemin est passe en PARAMETRE : l'interpoler cassait sur une
    // apostrophe, frequente dans les noms d'utilisateur Windows.
    conn.execute(
        "VACUUM INTO ?1",
        rusqlite::params![dest.to_string_lossy().to_string()],
    ).map_err(|e| format!("Erreur VACUUM INTO : {}", e))?;

    // Journaliser la sauvegarde.
    let utilisateur_id = crate::commandes::ventes::id_utilisateur_courant_pub(&conn);
    conn.execute(
        "INSERT INTO journal
         (id, type_evenement, entite_type, entite_id, auteur_id,
          nouveau_valeur, origine, date_evenement)
         VALUES (?1, 'sauvegarde', 'base', 'db', ?2, ?3, 'app', ?4)",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(),
            utilisateur_id,
            dest.to_string_lossy().to_string(),
            maintenant_iso()
        ],
    ).ok();

    Ok(dest.to_string_lossy().to_string())
}

/// Lire la configuration de sauvegarde.
#[tauri::command]
pub fn lire_config_sauvegarde(
    etat: State<EtatApp>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    // Config stockée dans une table simple key-value.
    let dossier: Option<String> = conn.query_row(
        "SELECT valeur FROM config_app WHERE cle = 'dossier_sauvegarde'",
        [], |r| r.get(0),
    ).ok();

    let auto: Option<String> = conn.query_row(
        "SELECT valeur FROM config_app WHERE cle = 'sauvegarde_auto'",
        [], |r| r.get(0),
    ).ok();

    let derniere: Option<String> = conn.query_row(
        "SELECT nouveau_valeur FROM journal
         WHERE type_evenement = 'sauvegarde'
         ORDER BY date_evenement DESC LIMIT 1",
        [], |r| r.get(0),
    ).ok();

    Ok(serde_json::json!({
        "dossier_sauvegarde": dossier,
        "sauvegarde_auto":    auto.as_deref() == Some("1"),
        "derniere_sauvegarde": derniere,
    }))
}

/// Sauvegarder la configuration de sauvegarde.
#[tauri::command]
pub fn sauvegarder_config_sauvegarde(
    etat: State<EtatApp>,
    dossier_sauvegarde: Option<String>,
    sauvegarde_auto: bool,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    if let Some(ref dossier) = dossier_sauvegarde {
        conn.execute(
            "INSERT INTO config_app (cle, valeur) VALUES ('dossier_sauvegarde', ?1)
             ON CONFLICT(cle) DO UPDATE SET valeur = ?1",
            rusqlite::params![dossier],
        ).map_err(|e| e.to_string())?;
    }

    conn.execute(
        "INSERT INTO config_app (cle, valeur)
         VALUES ('sauvegarde_auto', ?1)
         ON CONFLICT(cle) DO UPDATE SET valeur = ?1",
        rusqlite::params![if sauvegarde_auto { "1" } else { "0" }],
    ).map_err(|e| e.to_string())?;

    Ok(())
}
