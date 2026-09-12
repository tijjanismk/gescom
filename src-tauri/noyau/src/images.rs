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
//! ## Ce qui n'est pas ici
//!
//! L'ÉCRITURE. `sauvegarder_logo` reçoit un chemin de fichier sur la
//! machine qui appelle : transmis au serveur, ce chemin ne désigne rien
//! chez lui. Régler les images depuis une caisse demanderait de
//! transporter les octets, pas le chemin. Tant que ce n'est pas fait,
//! ces réglages se font sur le poste serveur — voir `portes`.

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
