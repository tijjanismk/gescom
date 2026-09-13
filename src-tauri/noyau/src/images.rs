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
    std::fs::create_dir_all(dossier)
        .map_err(|e| format!("Impossible de créer le dossier d'images : {e}"))?;
    let chemin = dossier.join(format!("{base}.{ext}"));
    std::fs::write(&chemin, contenu)
        .map_err(|e| format!("Impossible d'écrire l'image : {e}"))?;
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
