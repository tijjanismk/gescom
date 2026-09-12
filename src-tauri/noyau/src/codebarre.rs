//! Codes-barres — attribution et impression d'étiquettes.
//!
//! Le calcul EAN-13 vit dans `coeur::codebarre`, testé.
//! Ce module ne fait que l'attribution et la lecture.

use crate::utils::maintenant_iso;
use crate::coeur::codebarre::{generer_ean13_interne, valider_ean13, est_interne};

/// Reserve la prochaine sequence d'un code interne.
///
/// Meme compteur transactionnel que les pieces
/// ([`crate::argent::suivant`]) : `article.code_barre` est UNIQUE, et
/// lire le plus grand code deja attribue laissait deux etiquetages
/// simultanes tomber sur le meme.
///
/// Une seule suite, sans annee : un code-barre colle sur un sac ne se
/// reinitialise pas au 1er janvier.
///
/// **Consomme une sequence a chaque appel.**
pub fn reserver_sequence(conn: &rusqlite::Connection) -> Result<u64, String> {
    Ok(crate::argent::suivant(conn, "codebarre")? as u64)
}

/// Attribue un code interne à un article qui n'en a pas.
pub fn generer_code_barre(
    conn: &rusqlite::Connection,
    article_id: String,
) -> Result<String, String> {

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

    let seq = reserver_sequence(&conn)?;
    let code = generer_ean13_interne(seq)
        .ok_or_else(|| "Séquence de codes épuisée".to_string())?;

    conn.execute(
        "UPDATE article SET code_barre = ?1, modifie_le = ?2 WHERE id = ?3",
        rusqlite::params![code, maintenant_iso(), article_id],
    ).map_err(|e| e.to_string())?;

    Ok(code)
}

/// Attribue un code à tous les articles actifs qui n'en ont pas.
pub fn generer_codes_barres_manquants(
    conn: &mut rusqlite::Connection,
) -> Result<serde_json::Value, String> {

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

    let mut seq = reserver_sequence(&conn)?;
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
pub fn definir_code_barre(
    conn: &rusqlite::Connection,
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
pub fn lire_articles_codes_barres(
    conn: &rusqlite::Connection,
    sans_code_seulement: Option<bool>,
) -> Result<Vec<serde_json::Value>, String> {
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

// =====================================================================
//  SUR L'UN OU L'AUTRE MOTEUR
// =====================================================================
//
// `article` et `unite_vente` ne sont pas cloisonnes : un code-barre
// est commun a tous les dossiers, comme l'article qu'il designe.

use crate::base::Base;
use crate::parametres;

/// Meme compteur transactionnel que les pieces, sur `Base`.
pub fn reserver_sequence_sur(base: &mut impl crate::base::Acces) -> Result<u64, String> {
    Ok(crate::argent::suivant_sur(base, "codebarre")? as u64)
}

fn poser_code(
    acces: &mut impl crate::base::Acces,
    article_id: &str,
    code: Option<String>,
    now: &str,
) -> Result<(), String> {
    acces
        .executer(
            "UPDATE article SET code_barre = CAST(?1 AS TEXT), modifie_le = ?2 WHERE id = ?3",
            &parametres![code, now, article_id],
        )
        .map_err(|e| e.0)
        .map(|_| ())
}

pub fn generer_code_barre_sur_base(base: &mut Base, article_id: String) -> Result<String, String> {
    let existant: Option<String> = base
        .lire_une(
            "SELECT code_barre FROM article WHERE id = ?1",
            &parametres![article_id.clone()],
            |r| r.get::<Option<String>>(0),
        )
        .map_err(|e| e.0)?
        .ok_or_else(|| "Article introuvable".to_string())?;

    // Un code fabricant se conserve : il est imprime sur l'emballage.
    if let Some(c) = existant {
        if !c.trim().is_empty() {
            return Ok(c);
        }
    }

    let seq = reserver_sequence_sur(base)?;
    let code = generer_ean13_interne(seq).ok_or_else(|| "Séquence de codes épuisée".to_string())?;
    poser_code(base, &article_id, Some(code.clone()), &maintenant_iso())?;
    Ok(code)
}

pub fn generer_codes_barres_manquants_sur_base(base: &mut Base) -> Result<serde_json::Value, String> {
    let sans_code: Vec<String> = base
        .lire_plusieurs(
            "SELECT id FROM article
             WHERE actif = 1 AND (code_barre IS NULL OR TRIM(code_barre) = '')
             ORDER BY nom",
            &[],
            |r| r.get::<String>(0),
        )
        .map_err(|e| e.0)?;

    let now = maintenant_iso();
    let mut tx = base.transaction().map_err(|e| e.0)?;
    let mut nb = 0;
    for id in &sans_code {
        // Une sequence par article, reservee DANS la transaction : un
        // echec rend les numeros, et deux postes qui etiquettent en
        // meme temps ne se marchent pas dessus.
        let seq = reserver_sequence_sur(&mut tx)?;
        let Some(code) = generer_ean13_interne(seq) else { break };
        poser_code(&mut tx, id, Some(code), &now)?;
        nb += 1;
    }
    tx.valider().map_err(|e| e.0)?;
    Ok(serde_json::json!({ "generes": nb, "restants": sans_code.len() - nb }))
}

pub fn definir_code_barre_sur_base(
    base: &mut Base,
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
    if !c.is_empty() {
        let occupe: Option<String> = base
            .lire_une(
                "SELECT nom FROM article WHERE code_barre = ?1 AND id <> ?2",
                &parametres![c.clone(), article_id.clone()],
                |r| r.get::<String>(0),
            )
            .map_err(|e| e.0)?;
        if let Some(nom) = occupe {
            return Err(format!("Ce code est déjà utilisé par « {} ».", nom));
        }
    }
    poser_code(base, &article_id, if c.is_empty() { None } else { Some(c) }, &maintenant_iso())
}

pub fn lire_articles_codes_barres_sur_base(
    base: &mut Base,
    sans_code_seulement: Option<bool>,
) -> Result<Vec<serde_json::Value>, String> {
    let filtre = sans_code_seulement.unwrap_or(false);
    base.lire_plusieurs(
        "SELECT a.id, a.nom, COALESCE(a.code_barre, ''), a.unite_base,
                CAST(COALESCE((SELECT uv.prix_reference FROM unite_vente uv
                   WHERE uv.article_id = a.id AND uv.actif = 1
                   ORDER BY uv.facteur LIMIT 1), 0) AS BIGINT)
         FROM article a
         WHERE a.actif = 1
           AND (CAST(?1 AS BIGINT) = 0 OR a.code_barre IS NULL OR TRIM(a.code_barre) = '')
         ORDER BY a.nom",
        &parametres![filtre as i64],
        |r| {
            let code: String = r.get::<String>(2)?;
            Ok(serde_json::json!({
                "id":         r.get::<String>(0)?,
                "nom":        r.get::<String>(1)?,
                "code_barre": code.clone(),
                "unite_base": r.get::<String>(3)?,
                "prix":       r.get::<i64>(4)?,
                "interne":    est_interne(&code),
                "valide":     code.is_empty() || valider_ean13(&code),
            }))
        },
    )
    .map_err(|e| e.0)
}
