//! Les reglages des documents imprimes (v3, A-1) — le stockage.
//!
//! Les regles (genres, defauts, validation) sont dans `coeur::documents`.
//! Ici : lire et ecrire, sur l'un ou l'autre moteur, par `Acces`.
//!
//! ## Ou, et pourquoi
//!
//! Dans `config_app`, pas dans une table : une dizaine de lignes par
//! boutique, lues d'un coup a chaque impression. `config_app` n'est pas
//! cloisonnee, comme `parametres_societe` : l'en-tete, le pied et les
//! signatures sont ceux de la societe (D18).
//!
//! - `documents_reglages` : un objet JSON `{ genre: reglage }` ;
//! - `documents_signature_<genre>_<rang>` : l'image d'une signature
//!   (cachet, signature scannee), en `data:` URL. Le CONTENU, pas un
//!   chemin (D8) : il voyage vers les caisses tel quel, et la sauvegarde
//!   de la base l'emporte — y compris `pg_dump`.
//!
//! Les nouvelles commandes n'ont qu'une version, sur `Base` (D22 : le
//! serveur sert tout par `Base`, sur les deux moteurs).

use crate::base::Acces;
use crate::coeur::documents::{self as regles, ReglageGenre};
use crate::parametres;

const CLE_REGLAGES: &str = "documents_reglages";
const CLE_COORDONNEES: &str = "documents_coordonnees";

/// Un cachet ou une signature scannee : quelques dizaines de ko. Au-dela
/// c'est une photo de telephone qu'il faut recadrer.
pub const IMAGE_SIGNATURE_MAX: usize = 512 * 1024;

const TYPES_IMAGE: [&str; 3] = ["image/png", "image/jpeg", "image/webp"];

fn cle_image(genre: &str, rang: usize) -> String {
    format!("documents_signature_{genre}_{rang}")
}

fn lire_cle(acces: &mut impl Acces, cle: &str) -> Result<Option<String>, String> {
    acces
        .lire_une("SELECT valeur FROM config_app WHERE cle = ?1", &parametres![cle], |r| r.get::<String>(0))
        .map_err(|e| e.0)
}

fn ecrire_cle(acces: &mut impl Acces, cle: &str, valeur: &str) -> Result<(), String> {
    acces
        .executer(
            "INSERT INTO config_app (cle, valeur) VALUES (?1, ?2)
             ON CONFLICT (cle) DO UPDATE SET valeur = excluded.valeur",
            &parametres![cle, valeur],
        )
        .map(|_| ())
        .map_err(|e| e.0)
}

fn effacer_cle(acces: &mut impl Acces, cle: &str) -> Result<(), String> {
    acces
        .executer("DELETE FROM config_app WHERE cle = ?1", &parametres![cle])
        .map(|_| ())
        .map_err(|e| e.0)
}

/// Les libelles des paires v2 (`signature_facture_gauche`…), pour que
/// le premier passage en v3 garde ce que la boutique avait regle.
fn anciennes(acces: &mut impl Acces) -> Result<std::collections::HashMap<String, String>, String> {
    let lignes = acces
        .lire_plusieurs(
            "SELECT cle, valeur FROM config_app WHERE cle LIKE 'signature_%'",
            &[],
            |r| Ok((r.get::<String>(0)?, r.get::<String>(1)?)),
        )
        .map_err(|e| e.0)?;
    Ok(lignes.into_iter().collect())
}

fn tous(acces: &mut impl Acces) -> Result<Vec<(String, ReglageGenre)>, String> {
    let enregistre: serde_json::Value = lire_cle(acces, CLE_REGLAGES)?
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or(serde_json::Value::Null);
    let v2 = anciennes(acces)?;
    let ancienne = |cle: &str| v2.get(cle).cloned();
    Ok(regles::GENRES
        .iter()
        .map(|g| (g.to_string(), regles::fusionner(g, enregistre.get(*g), &ancienne)))
        .collect())
}

fn ecrire_tous(acces: &mut impl Acces, tous: &[(String, ReglageGenre)]) -> Result<(), String> {
    let mut m = serde_json::Map::new();
    for (g, r) in tous {
        m.insert(g.clone(), serde_json::to_value(r).map_err(|e| e.to_string())?);
    }
    ecrire_cle(acces, CLE_REGLAGES, &serde_json::Value::Object(m).to_string())
}

/// Tous les reglages, images comprises : ce que l'ecran lit une fois
/// avant d'imprimer. `{ ordre: [...], genres: { genre: reglage } }`,
/// chaque signature portant `image` (data URL) ou `null`.
pub fn lire_reglages_sur(acces: &mut impl Acces) -> Result<serde_json::Value, String> {
    let mut genres = serde_json::Map::new();
    for (g, r) in tous(acces)? {
        let mut v = serde_json::to_value(&r).map_err(|e| e.to_string())?;
        if let Some(sigs) = v.get_mut("signatures").and_then(|s| s.as_array_mut()) {
            for (rang, s) in sigs.iter_mut().enumerate() {
                s["image"] = serde_json::json!(lire_cle(acces, &cle_image(&g, rang))?);
            }
        }
        genres.insert(g, v);
    }
    Ok(serde_json::json!({
        "ordre": regles::GENRES,
        "genres": genres,
        "coordonnees": lire_coordonnees_sur(acces)?,
    }))
}

/// Les coordonnees affichees sous le nom, quand l'en-tete n'est pas une
/// image. Absent ou illisible : celles du generateur historique.
pub fn lire_coordonnees_sur(acces: &mut impl Acces) -> Result<Vec<String>, String> {
    let enregistre: Option<Vec<String>> = lire_cle(acces, CLE_COORDONNEES)?
        .and_then(|t| serde_json::from_str(&t).ok());
    Ok(enregistre
        .and_then(|c| regles::valider_coordonnees(&c).ok())
        .unwrap_or_else(|| regles::COORDONNEES_DEFAUT.iter().map(|c| c.to_string()).collect()))
}

pub fn enregistrer_coordonnees_sur(acces: &mut impl Acces, choix: Vec<String>) -> Result<Vec<String>, String> {
    let c = regles::valider_coordonnees(&choix)?;
    ecrire_cle(acces, CLE_COORDONNEES, &serde_json::json!(c).to_string())?;
    Ok(c)
}

/// Le reglage d'un genre, avec ses images.
pub fn lire_reglage_sur(acces: &mut impl Acces, genre: &str) -> Result<serde_json::Value, String> {
    if !regles::GENRES.contains(&genre) {
        return Err(format!("Genre de document inconnu : « {genre} »."));
    }
    let tout = lire_reglages_sur(acces)?;
    Ok(tout["genres"][genre].clone())
}

/// Enregistre le reglage d'un genre. Les images des emplacements qui
/// disparaissent partent avec eux : une signature retiree ne doit pas
/// laisser son cachet revenir le jour ou on en rajoute une.
pub fn enregistrer_reglage_sur(
    acces: &mut impl Acces,
    genre: &str,
    reglage: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let r: ReglageGenre = serde_json::from_value(reglage)
        .map_err(|e| format!("Réglage illisible : {e}"))?;
    regles::valider(genre, &r)?;
    let r = regles::normaliser(r);
    let garde = r.signatures.len();

    let mut tout = tous(acces)?;
    for (g, x) in tout.iter_mut() {
        if g == genre {
            *x = r.clone();
        }
    }
    ecrire_tous(acces, &tout)?;
    for rang in garde..regles::SIGNATURES_MAX {
        effacer_cle(acces, &cle_image(genre, rang))?;
    }
    lire_reglage_sur(acces, genre)
}

/// Remet un genre aux reglages d'usine, images comprises.
pub fn retablir_defaut_sur(acces: &mut impl Acces, genre: &str) -> Result<serde_json::Value, String> {
    if !regles::GENRES.contains(&genre) {
        return Err(format!("Genre de document inconnu : « {genre} »."));
    }
    let v2 = anciennes(acces)?;
    let d = regles::defaut(genre, &|cle: &str| v2.get(cle).cloned());
    let mut tout = tous(acces)?;
    for (g, x) in tout.iter_mut() {
        if g == genre {
            *x = d.clone();
        }
    }
    ecrire_tous(acces, &tout)?;
    for rang in 0..regles::SIGNATURES_MAX {
        effacer_cle(acces, &cle_image(genre, rang))?;
    }
    lire_reglage_sur(acces, genre)
}

/// Verifie une image de signature et la rend telle quelle.
pub fn valider_image_signature(data_url: &str) -> Result<(), String> {
    let (entete, b64) = data_url
        .split_once(";base64,")
        .ok_or_else(|| "Image illisible : data URL en base64 attendue.".to_string())?;
    let type_mime = entete.strip_prefix("data:").unwrap_or("");
    if !TYPES_IMAGE.contains(&type_mime) {
        return Err(format!(
            "Format refusé pour une signature : « {type_mime} ». PNG, JPEG ou WebP."
        ));
    }
    let octets = crate::images::decoder_base64(b64)?;
    if octets.len() > IMAGE_SIGNATURE_MAX {
        return Err(format!(
            "Image trop lourde pour une signature : {} ko, {} ko au plus. La recadrer.",
            octets.len() / 1024,
            IMAGE_SIGNATURE_MAX / 1024
        ));
    }
    Ok(())
}

/// Pose (ou retire, avec `None`) l'image d'un emplacement de signature.
/// L'emplacement doit exister : on ne pose pas un cachet sous rien.
pub fn poser_image_signature_sur(
    acces: &mut impl Acces,
    genre: &str,
    rang: usize,
    data_url: Option<String>,
) -> Result<serde_json::Value, String> {
    let (_, r) = tous(acces)?
        .into_iter()
        .find(|(g, _)| g == genre)
        .ok_or_else(|| format!("Genre de document inconnu : « {genre} »."))?;
    if rang >= r.signatures.len() {
        return Err(format!(
            "Pas de signature n° {} sur ce document : l'ajouter d'abord.",
            rang + 1
        ));
    }
    match data_url.filter(|d| !d.trim().is_empty()) {
        Some(d) => {
            valider_image_signature(&d)?;
            ecrire_cle(acces, &cle_image(genre, rang), &d)?;
        }
        None => effacer_cle(acces, &cle_image(genre, rang))?,
    }
    lire_reglage_sur(acces, genre)
}
