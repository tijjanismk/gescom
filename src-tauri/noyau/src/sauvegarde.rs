//! Sauvegarde automatique de la base SQLite.
//!
//! Copie la base vers un dossier configurable (clé USB, autre disque).
//! Nommage : gescom_backup_2026-08-17_14-30.db
//! Configurable dans les paramètres société.

use crate::utils::maintenant_iso;
use std::path::PathBuf;

/// Delai entre deux sauvegardes automatiques.
///
/// Une semaine : au-dela, ce qu'on perd en cas de panne depasse ce qu'un
/// commercant accepte de ressaisir de memoire.
const JOURS_ENTRE_SAUVEGARDES: i64 = 7;

pub fn sauvegarder_base(
    conn: &rusqlite::Connection,
    dossier_destination: String,
) -> Result<String, String> {
    effectuer_sauvegarde(&conn, &dossier_destination)
}

/// Copie effective. Partagee par la sauvegarde manuelle et
/// l'automatique : deux implementations auraient fini par diverger, et
/// c'est la derniere chose qu'on veut sur une sauvegarde.
pub fn effectuer_sauvegarde(
    conn: &rusqlite::Connection,
    dossier_destination: &str,
) -> Result<String, String> {
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
    let dest_dir = PathBuf::from(dossier_destination);
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
    let utilisateur_id = crate::argent::id_utilisateur_courant_pub(conn);
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
pub fn lire_config_sauvegarde(
    conn: &rusqlite::Connection,
) -> Result<serde_json::Value, String> {

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

/// Sauvegarde hebdomadaire, appelée au démarrage de l'application.
///
/// Pourquoi au démarrage et pas sur une minuterie : le poste d'un
/// commerçant n'est pas un serveur, il est éteint le soir. Une tâche
/// planifiée à heure fixe ne se déclencherait jamais. L'ouverture de
/// l'application, elle, est le seul moment garanti.
///
/// Le réglage existait déjà en base et n'était appliqué NULLE PART :
/// l'interrupteur affichait « activé » sans que rien ne sauvegarde
/// jamais. C'est cette fonction qui le rend vrai.
///
/// Ne renvoie JAMAIS `Err` sur un échec de copie : l'appelant est le
/// démarrage de l'app, qui ne doit pas être bloqué parce qu'une clé USB
/// est débranchée. L'échec revient dans `erreur`, à afficher — une
/// sauvegarde qui échoue en silence recréerait le mensonge d'origine.
pub fn sauvegarde_auto_si_necessaire(
    conn: &rusqlite::Connection,
) -> Result<serde_json::Value, String> {

    let actif: bool = conn.query_row(
        "SELECT valeur FROM config_app WHERE cle = 'sauvegarde_auto'",
        [], |r| r.get::<_, String>(0),
    ).map(|v| v == "1").unwrap_or(false);

    if !actif {
        return Ok(serde_json::json!({ "effectuee": false, "raison": "desactivee" }));
    }

    let dossier: Option<String> = conn.query_row(
        "SELECT valeur FROM config_app WHERE cle = 'dossier_sauvegarde'",
        [], |r| r.get(0),
    ).ok().filter(|d: &String| !d.is_empty());

    let Some(dossier) = dossier else {
        // Activée sans dossier : l'utilisateur croit être protégé alors
        // qu'aucune destination n'est connue. Il faut le lui dire.
        return Ok(serde_json::json!({
            "effectuee": false,
            "raison":    "sans_dossier",
            "erreur":    "Sauvegarde automatique activée, mais aucun dossier \
                          n'est choisi. Aucune sauvegarde n'a été faite.",
        }));
    };

    // Dernière sauvegarde connue, lue dans le journal (append-only).
    // Stocker la date dans `config_app` créerait une seconde vérité,
    // qui finirait par contredire la trace.
    let derniere: Option<String> = conn.query_row(
        "SELECT date_evenement FROM journal
         WHERE type_evenement = 'sauvegarde'
         ORDER BY date_evenement DESC LIMIT 1",
        [], |r| r.get(0),
    ).ok();

    // Comparaison sur la DATE seule : deux ouvertures le même jour ne
    // doivent pas produire deux copies, et l'heure d'ouverture varie.
    let jours = match derniere.as_deref() {
        None => i64::MAX, // jamais sauvegardé
        Some(d) => {
            match chrono::NaiveDateTime::parse_from_str(
                d.split('.').next().unwrap_or(d), "%Y-%m-%dT%H:%M:%S",
            ) {
                Ok(dt) => (chrono::Local::now().date_naive() - dt.date()).num_days(),
                // Date illisible : on sauvegarde plutôt que de sauter.
                Err(_) => i64::MAX,
            }
        }
    };

    if jours < JOURS_ENTRE_SAUVEGARDES {
        return Ok(serde_json::json!({
            "effectuee": false,
            "raison":    "recente",
            "jours":     jours,
        }));
    }

    match effectuer_sauvegarde(&conn, &dossier) {
        Ok(chemin) => Ok(serde_json::json!({
            "effectuee": true,
            "chemin":    chemin,
        })),
        Err(e) => Ok(serde_json::json!({
            "effectuee": false,
            "raison":    "echec",
            "erreur":    format!(
                "La sauvegarde automatique a échoué : {}. \
                 Vérifier que le dossier est accessible.", e
            ),
        })),
    }
}

/// Sauvegarder la configuration de sauvegarde.
pub fn sauvegarder_config_sauvegarde(
    conn: &rusqlite::Connection,
    dossier_sauvegarde: Option<String>,
    sauvegarde_auto: bool,
) -> Result<(), String> {

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
