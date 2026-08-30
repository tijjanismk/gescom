//! Codes-barres — attribution et impression d'étiquettes.
//!
//! Le calcul EAN-13 vit dans `coeur::codebarre`, testé.
//! Ce module ne fait que l'attribution et la lecture.

use tauri::State;
use crate::commandes::ventes::EtatApp;
use crate::utils::maintenant_iso;
use crate::coeur::codebarre::{generer_ean13_interne, valider_ean13, est_interne};

/// Prochaine séquence libre pour un code interne.
///
/// MAX et non COUNT : `article.code_barre` est UNIQUE, et un article
/// supprimé rejouerait un code déjà attribué. Même règle que les pièces.
fn prochaine_sequence(conn: &rusqlite::Connection) -> u64 {
    let dernier: i64 = conn.query_row(
        "SELECT COALESCE(MAX(CAST(substr(code_barre, 3, 10) AS INTEGER)), 0)
         FROM article
         WHERE code_barre LIKE '20%' AND length(code_barre) = 13",
        [], |r| r.get(0),
    ).unwrap_or(0);
    (dernier as u64) + 1
}

/// Attribue un code interne à un article qui n'en a pas.
#[tauri::command]
pub fn generer_code_barre(
    etat: State<EtatApp>,
    article_id: String,
) -> Result<String, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let existant: Option<String> = conn.query_row(
        "SELECT code_barre FROM article WHERE id = ?1",
        rusqlite::params![article_id], |r| r.get(0),
    ).map_err(|_| "Article introuvable".to_string())?;

    // Un code fabricant se conserve : il est imprimé sur l'emballage.
    if let Some(c) = existant {
        if !c.trim().is_empty() {
            return Ok(c);
        }
    }

    let seq = prochaine_sequence(&conn);
    let code = generer_ean13_interne(seq)
        .ok_or_else(|| "Séquence de codes épuisée".to_string())?;

    conn.execute(
        "UPDATE article SET code_barre = ?1, modifie_le = ?2 WHERE id = ?3",
        rusqlite::params![code, maintenant_iso(), article_id],
    ).map_err(|e| e.to_string())?;

    Ok(code)
}

/// Attribue un code à tous les articles actifs qui n'en ont pas.
#[tauri::command]
pub fn generer_codes_barres_manquants(
    etat: State<EtatApp>,
) -> Result<serde_json::Value, String> {
    let mut conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let sans_code: Vec<String> = {
        let mut st = conn.prepare(
            "SELECT id FROM article
             WHERE actif = 1
               AND (code_barre IS NULL OR trim(code_barre) = '')
             ORDER BY nom"
        ).map_err(|e| e.to_string())?;
        let v = st.query_map([], |r| r.get::<_, String>(0))
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok()).collect();
        v
    };

    let mut seq = prochaine_sequence(&conn);
    let now = maintenant_iso();
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let mut nb = 0;

    for id in &sans_code {
        let code = match generer_ean13_interne(seq) {
            Some(c) => c,
            None => break,
        };
        tx.execute(
            "UPDATE article SET code_barre = ?1, modifie_le = ?2 WHERE id = ?3",
            rusqlite::params![code, now, id],
        ).map_err(|e| e.to_string())?;
        seq += 1;
        nb += 1;
    }

    tx.commit().map_err(|e| e.to_string())?;
    Ok(serde_json::json!({ "generes": nb, "restants": sans_code.len() - nb }))
}

/// Saisir un code fabricant à la main.
#[tauri::command]
pub fn definir_code_barre(
    etat: State<EtatApp>,
    article_id: String,
    code: String,
) -> Result<(), String> {
    let c = code.trim().to_string();

    if !c.is_empty() && !valider_ean13(&c) {
        return Err(format!(
            "« {} » n'est pas un EAN-13 valide. \
             Attendu : 13 chiffres avec une clé de contrôle correcte. \
             Vérifier la saisie, ou laisser vide pour générer un code interne.",
            c
        ));
    }

    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    // code_barre est UNIQUE : sans ce controle, l'utilisateur recevrait
    // une erreur SQLite brute au lieu du nom de l'article en conflit.
    if !c.is_empty() {
        let occupe: Option<String> = conn.query_row(
            "SELECT nom FROM article WHERE code_barre = ?1 AND id <> ?2",
            rusqlite::params![c, article_id], |r| r.get(0),
        ).ok();
        if let Some(nom) = occupe {
            return Err(format!("Ce code est déjà utilisé par « {} ».", nom));
        }
    }

    conn.execute(
        "UPDATE article SET code_barre = ?1, modifie_le = ?2 WHERE id = ?3",
        rusqlite::params![
            if c.is_empty() { None } else { Some(c) },
            maintenant_iso(), article_id
        ],
    ).map_err(|e| e.to_string())?;

    Ok(())
}

/// Articles avec leur code, pour l'écran et les étiquettes.
#[tauri::command]
pub fn lire_articles_codes_barres(
    etat: State<EtatApp>,
    sans_code_seulement: Option<bool>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let filtre = sans_code_seulement.unwrap_or(false);

    let mut st = conn.prepare(
        "SELECT a.id, a.nom, COALESCE(a.code_barre, ''), a.unite_base,
                CAST(COALESCE((SELECT uv.prix_reference FROM unite_vente uv
                   WHERE uv.article_id = a.id AND uv.actif = 1
                   ORDER BY uv.facteur LIMIT 1), 0) AS INTEGER)
         FROM article a
         WHERE a.actif = 1
           AND (?1 = 0 OR a.code_barre IS NULL OR trim(a.code_barre) = '')
         ORDER BY a.nom"
    ).map_err(|e| e.to_string())?;

    let x = st.query_map(rusqlite::params![filtre as i64], |r| {
        let code: String = r.get(2)?;
        Ok(serde_json::json!({
            "id":          r.get::<_, String>(0)?,
            "nom":         r.get::<_, String>(1)?,
            "code_barre":  code.clone(),
            "unite_base":  r.get::<_, String>(3)?,
            "prix":        r.get::<_, i64>(4)?,
            "interne":     est_interne(&code),
            "valide":      code.is_empty() || valider_ean13(&code),
        }))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    Ok(x)
}
