//! Authentification avec bcrypt — plus simple qu'argon2, pas de dépendance rand_core.

use tauri::State;
use crate::commandes::ventes::EtatApp;
use crate::utils::maintenant_iso;

// =====================================================================
//  CONNEXION
// =====================================================================

#[tauri::command]
pub fn connexion(
    etat: State<EtatApp>,
    identifiant: String,
    mot_de_passe: String,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let maintenant = maintenant_iso();

    let result = conn.query_row(
        "SELECT ua.utilisateur_id, ua.mot_de_passe, ua.doit_changer_mdp,
                u.nom, r.nom as role_nom
         FROM utilisateur_auth ua
         JOIN utilisateur u ON u.id = ua.utilisateur_id
         JOIN role r ON r.id = u.role_id
         WHERE (ua.pseudo = ?1 OR ua.email = ?1) AND u.actif = 1",
        rusqlite::params![identifiant],
        |row| Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
        )),
    );

    let (utilisateur_id, hash_stocke, doit_changer, nom, role) = match result {
        Ok(r) => r,
        Err(_) => return Err("Identifiant ou mot de passe incorrect".to_string()),
    };

    let valide = bcrypt::verify(&mot_de_passe, &hash_stocke)
        .map_err(|e| format!("Erreur vérification : {}", e))?;

    if !valide {
        return Err("Identifiant ou mot de passe incorrect".to_string());
    }

    conn.execute(
        "UPDATE utilisateur_auth SET derniere_connexion = ?1 WHERE utilisateur_id = ?2",
        rusqlite::params![maintenant, utilisateur_id],
    ).ok();

    Ok(serde_json::json!({
        "id":               utilisateur_id,
        "nom":              nom,
        "role":             role,
        "doit_changer_mdp": doit_changer != 0,
    }))
}

// =====================================================================
//  CHANGER MOT DE PASSE
// =====================================================================

#[tauri::command]
pub fn changer_mot_de_passe(
    etat: State<EtatApp>,
    utilisateur_id: String,
    ancien_mdp: String,
    nouveau_mdp: String,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let hash_stocke: String = conn.query_row(
        "SELECT mot_de_passe FROM utilisateur_auth WHERE utilisateur_id = ?1",
        rusqlite::params![utilisateur_id],
        |row| row.get(0),
    ).map_err(|_| "Utilisateur introuvable".to_string())?;

    let valide = bcrypt::verify(&ancien_mdp, &hash_stocke)
        .map_err(|e| e.to_string())?;

    if !valide {
        return Err("Ancien mot de passe incorrect".to_string());
    }

    if nouveau_mdp.len() < 6 {
        return Err("Le mot de passe doit contenir au moins 6 caractères".to_string());
    }

    let nouveau_hash = hasher_mot_de_passe_pub(&nouveau_mdp)?;

    conn.execute(
        "UPDATE utilisateur_auth SET mot_de_passe = ?1, doit_changer_mdp = 0
         WHERE utilisateur_id = ?2",
        rusqlite::params![nouveau_hash, utilisateur_id],
    ).map_err(|e| e.to_string())?;

    Ok(())
}

// =====================================================================
//  CRÉER UTILISATEUR
// =====================================================================

#[tauri::command]
pub fn creer_utilisateur(
    etat: State<EtatApp>,
    nom: String,
    pseudo: String,
    email: Option<String>,
    mot_de_passe: String,
    role_nom: String,
    auteur_id: String,
) -> Result<String, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let maintenant = maintenant_iso();

    if mot_de_passe.len() < 6 {
        return Err("Le mot de passe doit contenir au moins 6 caractères".to_string());
    }

    let role_id: String = conn.query_row(
        "SELECT id FROM role WHERE nom = ?1",
        rusqlite::params![role_nom],
        |row| row.get(0),
    ).map_err(|_| format!("Rôle '{}' introuvable", role_nom))?;

    let utilisateur_id = uuid::Uuid::new_v4().to_string();

    conn.execute(
        "INSERT INTO utilisateur
         (id, nom, role_id, actif, cree_le, modifie_le, origine)
         VALUES (?1, ?2, ?3, 1, ?4, ?5, 'app')",
        rusqlite::params![utilisateur_id, nom, role_id, maintenant, maintenant],
    ).map_err(|e| e.to_string())?;

    let hash = hasher_mot_de_passe_pub(&mot_de_passe)?;

    conn.execute(
        "INSERT INTO utilisateur_auth
         (utilisateur_id, pseudo, email, mot_de_passe, doit_changer_mdp)
         VALUES (?1, ?2, ?3, ?4, 0)",
        rusqlite::params![utilisateur_id, pseudo, email, hash],
    ).map_err(|e| e.to_string())?;

    conn.execute(
        "INSERT INTO journal
         (id, type_evenement, entite_type, entite_id, auteur_id,
          nouveau_valeur, origine, date_evenement)
         VALUES (?1, 'utilisateur_cree', 'utilisateur', ?2, ?3, ?4, 'app', ?5)",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(),
            utilisateur_id, auteur_id,
            format!(r#"{{"nom":"{}","role":"{}"}}"#, nom, role_nom),
            maintenant
        ],
    ).ok();

    Ok(utilisateur_id)
}

// =====================================================================
//  LIRE UTILISATEURS
// =====================================================================

#[tauri::command]
pub fn lire_utilisateurs(
    etat: State<EtatApp>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let mut stmt = conn.prepare(
        "SELECT u.id, u.nom, r.nom as role, ua.pseudo, ua.email,
                ua.derniere_connexion, u.actif
         FROM utilisateur u
         JOIN role r ON r.id = u.role_id
         LEFT JOIN utilisateur_auth ua ON ua.utilisateur_id = u.id
         ORDER BY r.nom ASC, u.nom ASC"
    ).map_err(|e| e.to_string())?;

    let x = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "id":                 row.get::<_, String>(0)?,
            "nom":                row.get::<_, String>(1)?,
            "role":               row.get::<_, String>(2)?,
            "pseudo":             row.get::<_, Option<String>>(3)?,
            "email":              row.get::<_, Option<String>>(4)?,
            "derniere_connexion": row.get::<_, Option<String>>(5)?,
            "actif":              row.get::<_, i64>(6)? != 0,
        }))
    })
    .map_err(|e| e.to_string())?
    .filter_map(|r| r.ok())
    .collect();

    Ok(x)
}

// =====================================================================
//  Hashage bcrypt
// =====================================================================

pub fn hasher_mot_de_passe_pub(mdp: &str) -> Result<String, String> {
    bcrypt::hash(mdp, bcrypt::DEFAULT_COST)
        .map_err(|e| format!("Erreur hashage : {}", e))
}