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

// =====================================================================
//  SUR L'UN OU L'AUTRE MOTEUR
// =====================================================================
//
// Le reglage et l'historique se lisent partout. La COPIE, elle, est
// l'affaire du moteur : `VACUUM INTO` copie un fichier SQLite ; une
// base PostgreSQL n'est pas un fichier, elle se sauvegarde avec
// `pg_dump` (D4), lance d'ici avec l'URL que le serveur tient deja.

use crate::base::Base;
use crate::parametres;

/// Une sauvegarde, sur l'un ou l'autre moteur.
///
/// SQLite : `VACUUM INTO`, un fichier `.db` complet, WAL compris.
/// PostgreSQL : `pg_dump --format=custom`, un fichier `.dump` que
/// `pg_restore` sait rejouer. Dans les deux cas, une ligne au journal.
pub fn sauvegarder_base_sur_base(base: &mut Base, dossier_destination: String) -> Result<String, String> {
    if let Some(conn) = base.sqlite() {
        return effectuer_sauvegarde(conn, &dossier_destination);
    }
    let executable = config(base, "pg_dump_chemin").filter(|c| !c.trim().is_empty());
    let dest = pg_dump(base.cible(), std::path::Path::new(&dossier_destination), executable.as_deref())?;
    let chemin = dest.to_string_lossy().to_string();
    let dossier = base.dossier().to_string();
    let auteur = crate::argent::id_utilisateur_courant_sur(base);
    let _ = base.executer(
        "INSERT INTO journal
         (id, type_evenement, entite_type, entite_id, auteur_id,
          nouveau_valeur, origine, date_evenement, dossier_id)
         VALUES (?1, 'sauvegarde', 'base', 'db', ?2, ?3, 'app', ?4, ?5)",
        &parametres![uuid::Uuid::new_v4().to_string(), auteur, chemin.clone(), maintenant_iso(), dossier],
    );
    Ok(chemin)
}

// ---------------------------------------------------------------------
//  pg_dump
// ---------------------------------------------------------------------
//
// Le mot de passe ne va NI dans le depot (D10) NI sur la ligne de
// commande : la liste des processus est lisible par tout le monde sur
// le poste. Il passe par `PGPASSWORD`, que pg_dump lit et que personne
// d'autre ne voit. L'URL d'ou il vient est celle que le serveur tient
// deja — la meme information, au meme endroit.

/// Les morceaux d'une URL `postgresql://user:pass@hote:port/base?...`.
struct Connexion {
    utilisateur: Option<String>,
    mot_de_passe: Option<String>,
    hote: String,
    port: String,
    base: String,
}

/// Decoupe l'URL a la main : c'est une forme fixe, et une dependance
/// pour six champs serait un poids de plus dans l'installeur.
fn decouper_url(url: &str) -> Result<Connexion, String> {
    let reste = url
        .strip_prefix("postgresql://")
        .or_else(|| url.strip_prefix("postgres://"))
        .ok_or_else(|| "Ce n'est pas une URL PostgreSQL.".to_string())?;
    let reste = reste.split('?').next().unwrap_or(reste);
    let (acces, suite) = match reste.rsplit_once('@') {
        Some((a, s)) => (Some(a), s),
        None => (None, reste),
    };
    let (hote_port, base) = suite.split_once('/').unwrap_or((suite, ""));
    let (hote, port) = match hote_port.rsplit_once(':') {
        Some((h, p)) if !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()) => (h, p),
        _ => (hote_port, "5432"),
    };
    let (utilisateur, mot_de_passe) = match acces {
        Some(a) => match a.split_once(':') {
            Some((u, p)) => (Some(decoder(u)), Some(decoder(p))),
            None => (Some(decoder(a)), None),
        },
        None => (None, None),
    };
    if base.is_empty() {
        return Err("L'URL ne nomme pas de base.".to_string());
    }
    Ok(Connexion {
        utilisateur,
        mot_de_passe,
        hote: if hote.is_empty() { "localhost".into() } else { hote.to_string() },
        port: port.to_string(),
        base: decoder(base),
    })
}

/// `%40` -> `@`, pour un mot de passe qui en contient un.
fn decoder(v: &str) -> String {
    let octets = v.as_bytes();
    let mut out = Vec::with_capacity(octets.len());
    let mut i = 0;
    while i < octets.len() {
        if octets[i] == b'%' && i + 2 < octets.len() {
            if let Ok(n) = u8::from_str_radix(&v[i + 1..i + 3], 16) {
                out.push(n);
                i += 3;
                continue;
            }
        }
        out.push(octets[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

/// Ou est `pg_dump` ? Le reglage d'abord, puis le PATH, puis le dossier
/// d'installation habituel sous Windows — la version la plus haute.
fn trouver_pg_dump(explicite: Option<&str>) -> Result<std::path::PathBuf, String> {
    if let Some(c) = explicite {
        let p = std::path::PathBuf::from(c);
        if p.exists() {
            return Ok(p);
        }
        return Err(format!("pg_dump introuvable au chemin réglé : {c}"));
    }
    let nom = if cfg!(windows) { "pg_dump.exe" } else { "pg_dump" };
    if let Some(chemins) = std::env::var_os("PATH") {
        for d in std::env::split_paths(&chemins) {
            let p = d.join(nom);
            if p.exists() {
                return Ok(p);
            }
        }
    }
    let mut candidats: Vec<std::path::PathBuf> = ["C:\\Program Files\\PostgreSQL", "C:\\Program Files (x86)\\PostgreSQL"]
        .iter()
        .filter_map(|racine| std::fs::read_dir(racine).ok())
        .flatten()
        .filter_map(|e| e.ok())
        .map(|e| e.path().join("bin").join(nom))
        .filter(|p| p.exists())
        .collect();
    candidats.sort();
    candidats.pop().ok_or_else(|| {
        "pg_dump introuvable. Installer les outils client PostgreSQL, ou régler \
         « pg_dump_chemin » dans les paramètres."
            .to_string()
    })
}

/// Lance `pg_dump` et rend le fichier ecrit.
pub fn pg_dump(url: &str, dossier_destination: &std::path::Path, executable: Option<&str>) -> Result<std::path::PathBuf, String> {
    let c = decouper_url(url)?;
    let exe = trouver_pg_dump(executable)?;
    std::fs::create_dir_all(dossier_destination)
        .map_err(|e| format!("Impossible de créer le dossier : {e}"))?;
    let horodatage = chrono::Local::now().format("%Y-%m-%d_%H-%M").to_string();
    let dest = dossier_destination.join(format!("gescom_backup_{horodatage}.dump"));

    let mut cmd = std::process::Command::new(&exe);
    cmd.arg("--format=custom")
        .arg("--no-password")
        .arg("--host").arg(&c.hote)
        .arg("--port").arg(&c.port)
        .arg("--dbname").arg(&c.base)
        .arg("--file").arg(&dest);
    if let Some(u) = &c.utilisateur {
        cmd.arg("--username").arg(u);
    }
    if let Some(p) = &c.mot_de_passe {
        cmd.env("PGPASSWORD", p);
    }
    let sortie = cmd.output().map_err(|e| format!("Impossible de lancer {} : {e}", exe.display()))?;
    if !sortie.status.success() {
        let _ = std::fs::remove_file(&dest);
        let detail = String::from_utf8_lossy(&sortie.stderr).trim().to_string();
        return Err(format!("pg_dump a échoué : {detail}"));
    }
    Ok(dest)
}

#[cfg(test)]
mod tests_url {
    use super::decouper_url;

    #[test]
    fn une_url_complete() {
        let c = decouper_url("postgresql://postgres:s%40cret@10.0.0.5:5433/gescom?sslmode=disable").unwrap();
        assert_eq!(c.utilisateur.as_deref(), Some("postgres"));
        assert_eq!(c.mot_de_passe.as_deref(), Some("s@cret"));
        assert_eq!(c.hote, "10.0.0.5");
        assert_eq!(c.port, "5433");
        assert_eq!(c.base, "gescom");
    }

    #[test]
    fn une_url_minimale() {
        let c = decouper_url("postgres://localhost/gescom").unwrap();
        assert!(c.utilisateur.is_none());
        assert_eq!(c.port, "5432");
        assert_eq!(c.base, "gescom");
    }

    #[test]
    fn sans_base_ou_hors_postgres() {
        assert!(decouper_url("postgresql://localhost").is_err());
        assert!(decouper_url("C:/gescom.db").is_err());
    }
}

fn config(base: &mut Base, cle: &str) -> Option<String> {
    base.lire_une("SELECT valeur FROM config_app WHERE cle = ?1", &parametres![cle], |r| r.get::<String>(0))
        .ok()
        .flatten()
}

fn derniere_sauvegarde(base: &mut Base, colonne: &str) -> Option<String> {
    let dossier = base.dossier().to_string();
    base.lire_une(
        &format!(
            "SELECT {colonne} FROM journal
             WHERE type_evenement = 'sauvegarde' AND dossier_id = ?1
             ORDER BY date_evenement DESC LIMIT 1"
        ),
        &parametres![dossier],
        |r| r.get::<Option<String>>(0),
    )
    .ok()
    .flatten()
    .flatten()
}

pub fn lire_config_sauvegarde_sur_base(base: &mut Base) -> Result<serde_json::Value, String> {
    let dossier = config(base, "dossier_sauvegarde");
    let auto = config(base, "sauvegarde_auto");
    let derniere = derniere_sauvegarde(base, "nouveau_valeur");
    Ok(serde_json::json!({
        "dossier_sauvegarde":  dossier,
        "sauvegarde_auto":     auto.as_deref() == Some("1"),
        "derniere_sauvegarde": derniere,
        "moteur":              if base.est_postgres() { "postgresql" } else { "sqlite" },
    }))
}

pub fn sauvegarde_auto_si_necessaire_sur_base(base: &mut Base) -> Result<serde_json::Value, String> {
    if config(base, "sauvegarde_auto").as_deref() != Some("1") {
        return Ok(serde_json::json!({ "effectuee": false, "raison": "desactivee" }));
    }
    let Some(dossier) = config(base, "dossier_sauvegarde").filter(|d| !d.is_empty()) else {
        return Ok(serde_json::json!({
            "effectuee": false,
            "raison":    "sans_dossier",
            "erreur":    "Sauvegarde automatique activée, mais aucun dossier \
                          n'est choisi. Aucune sauvegarde n'a été faite.",
        }));
    };
    let jours = match derniere_sauvegarde(base, "date_evenement").as_deref() {
        None => i64::MAX,
        Some(d) => match chrono::NaiveDateTime::parse_from_str(d.split('.').next().unwrap_or(d), "%Y-%m-%dT%H:%M:%S") {
            Ok(dt) => (chrono::Local::now().date_naive() - dt.date()).num_days(),
            Err(_) => i64::MAX,
        },
    };
    if jours < JOURS_ENTRE_SAUVEGARDES {
        return Ok(serde_json::json!({ "effectuee": false, "raison": "recente", "jours": jours }));
    }
    match sauvegarder_base_sur_base(base, dossier) {
        Ok(chemin) => Ok(serde_json::json!({ "effectuee": true, "chemin": chemin })),
        Err(e) => Ok(serde_json::json!({
            "effectuee": false,
            "raison":    "echec",
            "erreur":    format!("La sauvegarde automatique a échoué : {}. Vérifier que le dossier est accessible.", e),
        })),
    }
}

pub fn sauvegarder_config_sauvegarde_sur_base(
    base: &mut Base,
    dossier_sauvegarde: Option<String>,
    sauvegarde_auto: bool,
) -> Result<(), String> {
    let mut reglages: Vec<(&str, String)> = Vec::new();
    if let Some(d) = dossier_sauvegarde {
        reglages.push(("dossier_sauvegarde", d));
    }
    reglages.push(("sauvegarde_auto", if sauvegarde_auto { "1".into() } else { "0".into() }));
    for (cle, valeur) in reglages {
        base.executer(
            "INSERT INTO config_app (cle, valeur) VALUES (?1, ?2)
             ON CONFLICT (cle) DO UPDATE SET valeur = ?2",
            &parametres![cle, valeur],
        )
        .map_err(|e| e.0)?;
    }
    Ok(())
}
