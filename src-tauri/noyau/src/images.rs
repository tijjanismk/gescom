//! Logo, en-tête et pied de page — la lecture, partagée.
//!
//! ## Pourquoi ces trois images vivent ici
//!
//! Elles sont stockées comme des FICHIERS sur le disque ; la base ne
//! garde que leur chemin. Une caisse en réseau lisait donc son propre
//! disque, où il n'y a rien : elle imprimait des factures sans logo,
//! sans en-tête et sans pied, alors que le poste serveur les imprimait
//! complètes. Deux factures différentes pour la même boutique.
//!
//! La lecture est donc servie par celui qui détient les fichiers. Le
//! poste caisse demande, le serveur répond en base64, et l'impression
//! reste locale — c'est la machine devant le client qui a l'imprimante.
//!
//! ## L'écriture (D8)
//!
//! La caisse lit le fichier et en envoie le CONTENU, en base64 : un
//! chemin de caisse ne désigne rien chez le serveur. Celui-ci le range
//! dans son dossier d'images et enregistre le chemin en base ; les
//! caisses le relisent ensuite par `lire_base64`.

use std::io::Read;

/// Les trois images, et rien d'autre.
///
/// Le nom arrive de l'écran : le restreindre ici évite qu'un appel
/// forgé fasse lire un chemin arbitraire du disque du serveur.
fn colonne_et_base(genre: &str) -> Option<(&'static str, &'static str)> {
    match genre {
        "logo" => Some(("logo_chemin", "logo")),
        "entete" => Some(("entete_chemin", "entete")),
        "pied" => Some(("pied_chemin", "pied")),
        _ => None,
    }
}

/// L'image demandée, en `data:` URL prête à poser dans le HTML.
///
/// `dossier_donnees` est le repli : une image déposée à côté de la base
/// sans que son chemin ait été enregistré. C'est ce que faisait la
/// version Tauri avec `app_data_dir`, sauf qu'ici le dossier est passé
/// explicitement — le noyau ne connaît pas Tauri.
///
/// `None` quand il n'y a pas d'image : ce n'est pas une erreur, une
/// boutique sans logo imprime très bien.
pub fn lire_base64(
    conn: &rusqlite::Connection,
    genre: &str,
    dossier_donnees: Option<&std::path::Path>,
) -> Result<Option<String>, String> {
    let Some((colonne, base)) = colonne_et_base(genre) else {
        return Err(format!("Image inconnue : « {genre} »"));
    };

    // `format!` sur un nom de COLONNE, pas sur une valeur : les trois
    // seules chaines possibles sont ecrites ci-dessus, jamais recues.
    let chemin_bd: Option<String> = conn
        .query_row(
            &format!("SELECT {colonne} FROM parametres_societe WHERE id = 1"),
            [],
            |r| r.get(0),
        )
        .ok()
        .flatten();

    let chemin = match chemin_bd {
        Some(c) if !c.is_empty() => c,
        _ => {
            let Some(dossier) = dossier_donnees else {
                return Ok(None);
            };
            let trouve = ["png", "jpg", "jpeg", "svg", "webp"]
                .iter()
                .map(|e| dossier.join(format!("{base}.{e}")))
                .find(|p| p.exists());
            match trouve {
                Some(p) => p.to_string_lossy().to_string(),
                None => return Ok(None),
            }
        }
    };

    let path = std::path::Path::new(&chemin);
    if !path.exists() {
        return Ok(None);
    }

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png")
        .to_lowercase();

    let mime = match ext.as_str() {
        "svg" => "image/svg+xml",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        _ => "image/png",
    };

    let mut fichier = std::fs::File::open(path)
        .map_err(|e| format!("Impossible de lire l'image : {e}"))?;
    let mut buffer = Vec::new();
    fichier
        .read_to_end(&mut buffer)
        .map_err(|e| format!("Erreur lecture : {e}"))?;

    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&buffer);
    Ok(Some(format!("data:{mime};base64,{b64}")))
}

/// Meme lecture, sur `Base`. Le chemin vient de la base ; le repli
/// « a cote du fichier » n'existe que pour SQLite, qui a un fichier.
pub fn lire_base64_sur_base(
    base: &mut crate::base::Base,
    genre: &str,
    dossier_donnees: Option<&std::path::Path>,
) -> Result<Option<String>, String> {
    let Some((colonne, base_nom)) = colonne_et_base(genre) else {
        return Err(format!("Image inconnue : « {genre} »"));
    };
    let chemin_bd: Option<String> = base
        .lire_une(
            &format!("SELECT {colonne} FROM parametres_societe WHERE id = 1"),
            &[],
            |r| r.get::<Option<String>>(0),
        )
        .ok()
        .flatten()
        .flatten();

    let chemin = match chemin_bd {
        Some(c) if !c.is_empty() => c,
        _ => {
            let Some(dossier) = dossier_donnees else {
                return Ok(None);
            };
            let trouve = ["png", "jpg", "jpeg", "svg", "webp"]
                .iter()
                .map(|e| dossier.join(format!("{base_nom}.{e}")))
                .find(|p| p.exists());
            match trouve {
                Some(p) => p.to_string_lossy().to_string(),
                None => return Ok(None),
            }
        }
    };
    lire_fichier_base64(std::path::Path::new(&chemin))
}

/// L'encodage d'un fichier image, commun aux deux chemins.
fn lire_fichier_base64(path: &std::path::Path) -> Result<Option<String>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("png").to_lowercase();
    let mime = match ext.as_str() {
        "svg" => "image/svg+xml",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        _ => "image/png",
    };
    let mut fichier = std::fs::File::open(path).map_err(|e| format!("Impossible de lire l'image : {e}"))?;
    let mut buffer = Vec::new();
    fichier.read_to_end(&mut buffer).map_err(|e| format!("Erreur lecture : {e}"))?;
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&buffer);
    Ok(Some(format!("data:{mime};base64,{b64}")))
}

// ---------------------------------------------------------------------------
// L'écriture (D8)
// ---------------------------------------------------------------------------

/// Poids maximal d'une image reçue par le réseau : 10 Mo. Au-delà,
/// l'envoi traîne sur le Wi-Fi d'une boutique, et une image de facture
/// n'a aucune raison d'approcher cette taille.
pub const TAILLE_MAX_IMAGE: usize = 10 * 1024 * 1024;

/// Les formats acceptés à l'écriture — les mêmes qu'à la lecture.
const EXTENSIONS_IMAGE: [&str; 5] = ["png", "jpg", "jpeg", "webp", "svg"];

/// Décode le contenu base64 envoyé par la caisse.
pub fn decoder_base64(texte: &str) -> Result<Vec<u8>, String> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(texte)
        .map_err(|_| "Contenu illisible : base64 attendu.".to_string())
}

/// Ce qu'on accepte, et rien d'autre : une extension connue, un poids
/// raisonnable. Rend l'extension en minuscules.
fn valider_image(nom: &str, contenu: &[u8]) -> Result<String, String> {
    let ext = std::path::Path::new(nom)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if !EXTENSIONS_IMAGE.contains(&ext.as_str()) {
        return Err(format!(
            "Format refusé : « {ext} ». Acceptés : {}.",
            EXTENSIONS_IMAGE.join(", ")
        ));
    }
    if contenu.len() > TAILLE_MAX_IMAGE {
        return Err(format!(
            "Image trop lourde : {} octets, maximum {}.",
            contenu.len(),
            TAILLE_MAX_IMAGE
        ));
    }
    Ok(ext)
}

/// Range les octets dans `dossier`, sous le nom fixe du genre, et rend
/// la colonne cible avec le chemin écrit.
///
/// Seule l'extension du nom reçu compte ; le nom sur disque vient du
/// genre — poser un logo écrase le logo précédent, comme sur la
/// version à un poste.
fn poser_fichier(
    genre: &str,
    nom: &str,
    contenu: &[u8],
    dossier: &std::path::Path,
) -> Result<(&'static str, std::path::PathBuf), String> {
    let Some((colonne, base)) = colonne_et_base(genre) else {
        return Err(format!("Image inconnue : « {genre} »"));
    };
    let ext = valider_image(nom, contenu)?;
    std::fs::create_dir_all(dossier)
        .map_err(|e| format!("Impossible de créer le dossier d'images : {e}"))?;
    let chemin = dossier.join(format!("{base}.{ext}"));
    std::fs::write(&chemin, contenu)
        .map_err(|e| format!("Impossible d'écrire l'image : {e}"))?;
    // Le nouveau fichier écrit, les AUTRES extensions du même genre
    // s'effacent : poser un `logo.jpg` par-dessus un `logo.png` laissait
    // le png sur le disque, et le repli de lecture balaie les extensions
    // dans l'ordre — une colonne vidée faisait revenir l'ancien logo.
    // Dans cet ordre : si l'écriture échoue, l'image précédente est
    // encore là.
    for autre in EXTENSIONS_IMAGE {
        if autre != ext {
            let _ = std::fs::remove_file(dossier.join(format!("{base}.{autre}")));
        }
    }
    Ok((colonne, chemin))
}

/// Écrit l'image sur le disque puis enregistre son chemin en base.
///
/// Le fichier d'abord : si l'enregistrement échoue, il ne reste qu'un
/// fichier orphelin que la prochaine écriture écrasera — l'inverse
/// laisserait la base montrer une image absente.
pub fn ecrire(
    conn: &rusqlite::Connection,
    genre: &str,
    nom: &str,
    contenu: &[u8],
    dossier: &std::path::Path,
) -> Result<(), String> {
    let (colonne, chemin) = poser_fichier(genre, nom, contenu, dossier)?;
    let chemin_texte = chemin.to_string_lossy().to_string();
    conn.execute(
        &format!("UPDATE parametres_societe SET {colonne} = ?1 WHERE id = 1"),
        rusqlite::params![chemin_texte],
    )
    .map_err(|e| format!("Impossible d'enregistrer l'image : {e}"))?;
    Ok(())
}

/// Même écriture, sur `Base`.
///
/// `dossier` vient de l'appelant : à côté du fichier SQLite, ou un
/// dossier fixe côté PostgreSQL. Sans dossier, refuse plutôt que de
/// faire semblant.
pub fn ecrire_sur_base(
    base: &mut crate::base::Base,
    genre: &str,
    nom: &str,
    contenu: &[u8],
    dossier: Option<&std::path::Path>,
) -> Result<(), String> {
    let Some(dossier) = dossier else {
        return Err(
            "Impossible d'écrire l'image : aucun dossier d'images sur ce moteur.".to_string(),
        );
    };
    let (colonne, chemin) = poser_fichier(genre, nom, contenu, dossier)?;
    let chemin_texte = chemin.to_string_lossy().to_string();
    base.executer(
        &format!("UPDATE parametres_societe SET {colonne} = ?1 WHERE id = 1"),
        &[crate::base::Valeur::Texte(chemin_texte)],
    )
    .map_err(|e| format!("Impossible d'enregistrer l'image : {e}"))?;
    Ok(())
}

/// Efface l'image : la colonne redevient vide ET le fichier disparait.
///
/// Effacer aussi le fichier compte : la lecture a un repli sur le
/// dossier, et un fichier laisse en place ferait revenir l'image
/// qu'on vient de supprimer.
pub fn supprimer(
    conn: &rusqlite::Connection,
    genre: &str,
    dossier: Option<&std::path::Path>,
) -> Result<(), String> {
    let Some((colonne, base)) = colonne_et_base(genre) else {
        return Err(format!("Image inconnue : « {genre} »"));
    };
    conn.execute(
        &format!("UPDATE parametres_societe SET {colonne} = NULL WHERE id = 1"),
        [],
    )
    .map_err(|e| format!("Impossible de supprimer l'image : {e}"))?;
    if let Some(dossier) = dossier {
        for ext in EXTENSIONS_IMAGE {
            let _ = std::fs::remove_file(dossier.join(format!("{base}.{ext}")));
        }
    }
    Ok(())
}

/// Même suppression, sur `Base`.
pub fn supprimer_sur_base(
    base: &mut crate::base::Base,
    genre: &str,
    dossier: Option<&std::path::Path>,
) -> Result<(), String> {
    let Some((colonne, base_nom)) = colonne_et_base(genre) else {
        return Err(format!("Image inconnue : « {genre} »"));
    };
    base.executer(
        &format!("UPDATE parametres_societe SET {colonne} = NULL WHERE id = 1"),
        &[],
    )
    .map_err(|e| format!("Impossible de supprimer l'image : {e}"))?;
    if let Some(dossier) = dossier {
        for ext in EXTENSIONS_IMAGE {
            let _ = std::fs::remove_file(dossier.join(format!("{base_nom}.{ext}")));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Les images POSÉES sur un document (I1)
// ---------------------------------------------------------------------------
//
// Un cachet, une signature scannée, un QR : ce ne sont pas le logo de la
// société. Les trois emplacements de la société (logo, en-tête, pied)
// sont partagés par tout le monde — vingt blocs Image qui les
// désignaient pointaient sur trois fichiers, et en remplacer un les
// changeait tous. Ici chaque image a son identité : un bloc la
// référence par `id`, la renommer ne casse rien, la remplacer ne touche
// qu'elle.
//
// Le fichier vit chez le serveur, comme les images de la société (D8) :
// la table ne garde que de quoi le retrouver. Une image sert à
// PLUSIEURS modèles : elle n'appartient à aucun, elle se pose dessus.

/// Le nom du fichier sur le disque : l'identifiant, jamais le nom saisi
/// — un nom saisi contient ce qu'on veut, y compris `..`.
fn fichier_libre(dossier: &std::path::Path, id: &str, ext: &str) -> std::path::PathBuf {
    dossier.join(format!("img_{id}.{ext}"))
}

/// Le motif que porte un modèle qui pose cette image. `serde_json`
/// sérialise sans espace : c'est ce qui rend la recherche fiable.
fn empreinte_usage(id: &str) -> String {
    format!("\"imageId\":\"{id}\"")
}

fn ligne_image(id: String, nom: String, taille: i64, cree_le: String) -> serde_json::Value {
    serde_json::json!({ "id": id, "nom": nom, "taille": taille, "cree_le": cree_le })
}

/// Pose une nouvelle image et rend son identifiant.
///
/// Le fichier d'abord, la ligne ensuite : si la ligne échoue il reste
/// un fichier orphelin qu'un import suivant n'écrasera pas (le nom
/// vient de l'id), mais rien en base ne désigne un fichier absent.
pub fn importer_libre(
    conn: &rusqlite::Connection,
    nom: &str,
    contenu: &[u8],
    dossier: &std::path::Path,
) -> Result<String, String> {
    let id = uuid::Uuid::new_v4().to_string();
    poser_libre(conn, &id, nom, contenu, dossier)?;
    Ok(id)
}

/// Pose une image sous un identifiant DONNE — c'est ce qu'exige l'import
/// d'un lot de modèles : les blocs du modèle importé désignent l'image
/// par cet identifiant, il doit survivre au voyage.
pub fn poser_libre(
    conn: &rusqlite::Connection,
    id: &str,
    nom: &str,
    contenu: &[u8],
    dossier: &std::path::Path,
) -> Result<(), String> {
    let ext = valider_image(nom, contenu)?;
    let id = id.to_string();
    std::fs::create_dir_all(dossier)
        .map_err(|e| format!("Impossible de créer le dossier d'images : {e}"))?;
    let chemin = fichier_libre(dossier, &id, &ext);
    std::fs::write(&chemin, contenu).map_err(|e| format!("Impossible d'écrire l'image : {e}"))?;
    conn.execute(
        "INSERT INTO image_document (id, nom, chemin, taille, cree_le) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![
            id,
            nom_affiche(nom),
            chemin.to_string_lossy().to_string(),
            contenu.len() as i64,
            crate::utils::maintenant_iso()
        ],
    )
    .map_err(|e| format!("Impossible d'enregistrer l'image : {e}"))?;
    Ok(())
}

/// Même import, sur `Base`.
pub fn importer_libre_sur_base(
    base: &mut crate::base::Base,
    nom: &str,
    contenu: &[u8],
    dossier: Option<&std::path::Path>,
) -> Result<String, String> {
    let id = uuid::Uuid::new_v4().to_string();
    poser_libre_sur_base(base, &id, nom, contenu, dossier)?;
    Ok(id)
}

pub fn poser_libre_sur_base(
    base: &mut crate::base::Base,
    id: &str,
    nom: &str,
    contenu: &[u8],
    dossier: Option<&std::path::Path>,
) -> Result<(), String> {
    let Some(dossier) = dossier else {
        return Err("Impossible d'écrire l'image : aucun dossier d'images sur ce moteur.".to_string());
    };
    let ext = valider_image(nom, contenu)?;
    let id = id.to_string();
    std::fs::create_dir_all(dossier)
        .map_err(|e| format!("Impossible de créer le dossier d'images : {e}"))?;
    let chemin = fichier_libre(dossier, &id, &ext);
    std::fs::write(&chemin, contenu).map_err(|e| format!("Impossible d'écrire l'image : {e}"))?;
    base.executer(
        "INSERT INTO image_document (id, nom, chemin, taille, cree_le) VALUES (?1, ?2, ?3, ?4, ?5)",
        &crate::parametres![
            id.clone(),
            nom_affiche(nom),
            chemin.to_string_lossy().to_string(),
            contenu.len() as i64,
            crate::utils::maintenant_iso()
        ],
    )
    .map_err(|e| format!("Impossible d'enregistrer l'image : {e}"))?;
    Ok(())
}

/// Le nom qu'on montre : celui du fichier, sans son chemin.
fn nom_affiche(nom: &str) -> String {
    std::path::Path::new(nom)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(nom)
        .to_string()
}

pub fn lister_libres(conn: &rusqlite::Connection) -> Result<Vec<serde_json::Value>, String> {
    let mut st = conn
        .prepare("SELECT id, nom, taille, cree_le FROM image_document ORDER BY cree_le DESC, id DESC")
        .map_err(|e| e.to_string())?;
    let lignes = st
        .query_map([], |r| Ok(ligne_image(r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    Ok(lignes)
}

pub fn lister_libres_sur_base(base: &mut crate::base::Base) -> Result<Vec<serde_json::Value>, String> {
    base.lire_plusieurs(
        "SELECT id, nom, taille, cree_le FROM image_document ORDER BY cree_le DESC, id DESC",
        &[],
        |r| Ok(ligne_image(r.get::<String>(0)?, r.get::<String>(1)?, r.get::<i64>(2)?, r.get::<String>(3)?)),
    )
    .map_err(|e| e.0)
}

/// L'image en `data:` URL, ou `None` si elle n'existe pas (en base ou
/// sur le disque) — un modèle qui la pose imprime alors un blanc, pas
/// une erreur.
pub fn lire_libre_base64(conn: &rusqlite::Connection, id: &str) -> Result<Option<String>, String> {
    let chemin: Option<String> = conn
        .query_row("SELECT chemin FROM image_document WHERE id = ?1", rusqlite::params![id], |r| r.get(0))
        .ok();
    match chemin {
        Some(c) => lire_fichier_base64(std::path::Path::new(&c)),
        None => Ok(None),
    }
}

pub fn lire_libre_base64_sur_base(base: &mut crate::base::Base, id: &str) -> Result<Option<String>, String> {
    let chemin: Option<String> = base
        .lire_une("SELECT chemin FROM image_document WHERE id = ?1", &crate::parametres![id], |r| r.get::<String>(0))
        .ok()
        .flatten();
    match chemin {
        Some(c) => lire_fichier_base64(std::path::Path::new(&c)),
        None => Ok(None),
    }
}

/// Toutes les images libres, en `data:` URL, par identifiant : ce que
/// le rendu d'un document a sous la main.
pub fn lire_libres_base64(conn: &rusqlite::Connection) -> Result<serde_json::Value, String> {
    let mut out = serde_json::Map::new();
    for img in lister_libres(conn)? {
        let id = img["id"].as_str().unwrap_or("").to_string();
        if let Some(b64) = lire_libre_base64(conn, &id)? {
            out.insert(id, serde_json::Value::String(b64));
        }
    }
    Ok(serde_json::Value::Object(out))
}

pub fn lire_libres_base64_sur_base(base: &mut crate::base::Base) -> Result<serde_json::Value, String> {
    let mut out = serde_json::Map::new();
    for img in lister_libres_sur_base(base)? {
        let id = img["id"].as_str().unwrap_or("").to_string();
        if let Some(b64) = lire_libre_base64_sur_base(base, &id)? {
            out.insert(id, serde_json::Value::String(b64));
        }
    }
    Ok(serde_json::Value::Object(out))
}

/// Les modèles qui posent cette image, par leur nom.
fn modeles_qui_posent(conn: &rusqlite::Connection, id: &str) -> Result<Vec<String>, String> {
    let mut st = conn
        .prepare("SELECT nom FROM modele_document WHERE contenu LIKE '%' || ?1 || '%' ORDER BY nom")
        .map_err(|e| e.to_string())?;
    let noms = st
        .query_map(rusqlite::params![empreinte_usage(id)], |r| r.get::<_, String>(0))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    Ok(noms)
}

fn modeles_qui_posent_sur_base(base: &mut crate::base::Base, id: &str) -> Result<Vec<String>, String> {
    base.lire_plusieurs(
        "SELECT nom FROM modele_document WHERE contenu LIKE '%' || ?1 || '%' ORDER BY nom",
        &crate::parametres![empreinte_usage(id)],
        |r| r.get::<String>(0),
    )
    .map_err(|e| e.0)
}

fn refus_si_posee(noms: &[String]) -> Result<(), String> {
    if noms.is_empty() {
        return Ok(());
    }
    // On REFUSE plutôt que de retirer l'image des blocs : une facture
    // qui perd son cachet sans qu'on l'ait demandé, c'est le genre de
    // surprise qu'on découvre devant le client. Le message dit où.
    Err(format!(
        "Cette image est posée sur {} modèle(s) : {}. La retirer de ces modèles d'abord.",
        noms.len(),
        noms.join(", ")
    ))
}

/// Efface l'image — la ligne ET le fichier — sauf si un modèle la pose.
pub fn supprimer_libre(conn: &rusqlite::Connection, id: &str) -> Result<(), String> {
    refus_si_posee(&modeles_qui_posent(conn, id)?)?;
    let chemin: Option<String> = conn
        .query_row("SELECT chemin FROM image_document WHERE id = ?1", rusqlite::params![id], |r| r.get(0))
        .ok();
    let Some(chemin) = chemin else {
        return Err("Image introuvable.".to_string());
    };
    conn.execute("DELETE FROM image_document WHERE id = ?1", rusqlite::params![id])
        .map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(chemin);
    Ok(())
}

pub fn supprimer_libre_sur_base(base: &mut crate::base::Base, id: &str) -> Result<(), String> {
    refus_si_posee(&modeles_qui_posent_sur_base(base, id)?)?;
    let chemin: Option<String> = base
        .lire_une("SELECT chemin FROM image_document WHERE id = ?1", &crate::parametres![id], |r| r.get::<String>(0))
        .ok()
        .flatten();
    let Some(chemin) = chemin else {
        return Err("Image introuvable.".to_string());
    };
    base.executer("DELETE FROM image_document WHERE id = ?1", &crate::parametres![id])
        .map_err(|e| e.0)?;
    let _ = std::fs::remove_file(chemin);
    Ok(())
}

/// Les octets d'une `data:` URL telle que `lire_libre_base64` la rend.
/// C'est la forme que l'export transporte : elle dit son type toute
/// seule, et le nom garde l'extension.
pub fn octets_de_data_url(data_url: &str) -> Result<Vec<u8>, String> {
    let b64 = data_url
        .split_once("base64,")
        .map(|(_, c)| c)
        .ok_or_else(|| "Image illisible dans le lot : data URL attendue.".to_string())?;
    decoder_base64(b64)
}
