//! Les modèles de documents.
//!
//! ## Pourquoi ils vivent en base et pas dans le code
//!
//! Un commerçant qui veut son logo à droite, sa mention légale au pied
//! et sa colonne « référence » en plus ne doit pas attendre une
//! version de Gescom. Le modèle est donc une donnée : du JSON dans une
//! table, modifiable depuis l'écran.
//!
//! Et parce que c'est une donnée, le serveur v2 la distribue. Un
//! modèle corrigé sur le poste principal est vu par toutes les caisses
//! au rechargement suivant — c'est le vrai déploiement. L'export
//! fichier sert à passer d'une INSTALLATION à une autre (clé USB,
//! nouvelle boutique), pas d'un poste à l'autre du même magasin.
//!
//! ## Ce que ce module ne fait pas
//!
//! Il ne rend rien. Il stocke, relit et transporte du JSON opaque. Le
//! rendu vit côté écran (`src/lib/modeles/rendu.ts`), là où le HTML
//! est déjà produit et où l'aperçu doit s'afficher sans aller-retour.
//! Un moteur de rendu en Rust obligerait à le réécrire en TypeScript
//! pour l'aperçu, donc à maintenir deux rendus qui divergeraient.

use rusqlite::{params, Connection, Result as ResSql};
use serde::{Deserialize, Serialize};

use crate::utils::maintenant_iso;

/// Version du format d'échange. Incrémentée seulement si un fichier
/// exporté par une version antérieure cesse d'être lisible.
/// 2 depuis le 17/09/2026 : le lot emporte les images posées sur ses
/// modèles. Un fichier de version 1 se lit toujours — `images` vaut
/// alors une liste vide.
pub const VERSION_ECHANGE: u32 = 2;

/// Marqueur du fichier d'export. Sans lui, on ouvrirait n'importe quel
/// JSON en croyant y trouver des modèles, et l'erreur ne se verrait
/// qu'à l'impression suivante.
pub const MARQUEUR: &str = "gescom-modeles";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Modele {
    pub id: String,
    /// `facture`, `recu_paiement`, `releve_creance`, `journal_caisse`…
    pub genre: String,
    pub nom: String,
    pub format: String,
    /// Le modèle lui-même : blocs, styles, liaisons. Opaque ici.
    pub contenu: serde_json::Value,
    /// Livré avec Gescom. Reste modifiable, mais `reinitialiser` le
    /// remet dans son état d'usine — il faut toujours un chemin de
    /// retour après une mise en page ratée un soir de clôture.
    pub est_defaut: bool,
    /// Le modèle effectivement utilisé pour son genre.
    pub actif: bool,
    /// Quand ce modele a ete touche pour la derniere fois.
    ///
    /// `default` parce que ce n'est PAS au client de l'inventer : c'est
    /// l'enregistrement qui le date. L'ecran envoyait un modele d'usine
    /// qui ne le portait pas, serde refusait tout le lot avec
    /// « missing field `modifie_le` », et l'ecran des modeles s'ouvrait
    /// sur une erreur au lieu de proposer ses modeles.
    #[serde(default)]
    pub modifie_le: String,
}

/// Une image posée sur un modèle du lot, telle qu'elle voyage : son
/// identifiant (les blocs la désignent par lui), son nom, et ses octets
/// en `data:` URL.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageDuLot {
    pub id: String,
    pub nom: String,
    pub contenu: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lot {
    pub marqueur: String,
    pub version: u32,
    pub exporte_le: String,
    #[serde(default)]
    pub societe: Option<String>,
    pub modeles: Vec<Modele>,
    /// Les images posées sur ces modèles. Pas celles de la société —
    /// le logo d'une boutique ne part pas chez une autre.
    #[serde(default)]
    pub images: Vec<ImageDuLot>,
}

/// Les identifiants d'images posées par ces modèles, sans doublon.
fn images_posees(modeles: &[Modele]) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    for m in modeles {
        if let Some(blocs) = m.contenu.get("blocs").and_then(|b| b.as_array()) {
            for b in blocs {
                if let Some(id) = b.get("imageId").and_then(|v| v.as_str()) {
                    if !id.is_empty() && !ids.iter().any(|x| x == id) {
                        ids.push(id.to_string());
                    }
                }
            }
        }
    }
    ids
}

// =====================================================================
//  Lecture
// =====================================================================

pub fn lister(conn: &Connection, genre: Option<&str>) -> ResSql<Vec<Modele>> {
    let mut stmt = conn.prepare(
        "SELECT id, genre, nom, format, contenu, est_defaut, actif, modifie_le
         FROM modele_document
         WHERE (?1 IS NULL OR genre = ?1)
         ORDER BY genre ASC, est_defaut DESC, nom ASC",
    )?;
    let v = stmt
        .query_map(params![genre], ligne)?
        .collect::<ResSql<Vec<_>>>()?;
    Ok(v)
}

pub fn lire(conn: &Connection, id: &str) -> ResSql<Modele> {
    conn.query_row(
        "SELECT id, genre, nom, format, contenu, est_defaut, actif, modifie_le
         FROM modele_document WHERE id = ?1",
        params![id],
        ligne,
    )
}

/// Le modèle à utiliser pour ce genre.
///
/// `None` n'est pas une erreur : c'est le cas d'une base qui n'a pas
/// encore reçu ses modèles d'usine. L'appelant retombe alors sur le
/// générateur historique, plutôt que de refuser d'imprimer.
pub fn lire_actif(conn: &Connection, genre: &str) -> Option<Modele> {
    conn.query_row(
        "SELECT id, genre, nom, format, contenu, est_defaut, actif, modifie_le
         FROM modele_document WHERE genre = ?1 AND actif = 1 LIMIT 1",
        params![genre],
        ligne,
    )
    .ok()
}

fn ligne(r: &rusqlite::Row) -> ResSql<Modele> {
    let contenu: String = r.get(4)?;
    Ok(Modele {
        id: r.get(0)?,
        genre: r.get(1)?,
        nom: r.get(2)?,
        format: r.get(3)?,
        // Un contenu illisible ne doit pas faire échouer la LISTE : on
        // veut pouvoir voir le modèle cassé dans l'écran pour le
        // supprimer, pas se retrouver avec un écran vide.
        contenu: serde_json::from_str(&contenu).unwrap_or(serde_json::Value::Null),
        est_defaut: r.get::<_, i64>(5)? != 0,
        actif: r.get::<_, i64>(6)? != 0,
        // Tolerante a une date absente, pour la meme raison que le
        // contenu juste au-dessus : une seule ligne mal formee ne doit
        // pas emporter TOUTE la liste. Sinon l'ecran des modeles
        // s'ouvre sur une erreur, et le modele fautif devient
        // impossible a supprimer puisqu'on ne le voit plus.
        modifie_le: r.get::<_, Option<String>>(7)?.unwrap_or_default(),
    })
}

// =====================================================================
//  Écriture
// =====================================================================

pub fn enregistrer(conn: &Connection, m: &Modele, auteur: &str) -> Result<(), String> {
    if m.nom.trim().is_empty() {
        return Err("Le modèle doit avoir un nom.".to_string());
    }
    if m.genre.trim().is_empty() {
        return Err("Le modèle doit avoir un genre.".to_string());
    }
    let contenu = serde_json::to_string(&m.contenu).map_err(|e| e.to_string())?;
    let maintenant = maintenant_iso();

    conn.execute(
        "INSERT INTO modele_document
           (id, genre, nom, format, contenu, est_defaut, actif,
            cree_le, modifie_le, modifie_par)
         VALUES (?1,?2,?3,?4,?5,?6,0,?7,?7,?8)
         ON CONFLICT(id) DO UPDATE SET
            genre = excluded.genre,
            nom = excluded.nom,
            format = excluded.format,
            contenu = excluded.contenu,
            modifie_le = excluded.modifie_le,
            modifie_par = excluded.modifie_par",
        params![
            m.id,
            m.genre.trim(),
            m.nom.trim(),
            m.format,
            contenu,
            m.est_defaut as i64,
            maintenant,
            auteur
        ],
    )
    .map_err(|e| e.to_string())?;

    // Le premier modèle d'un genre devient actif tout seul. Sans ça,
    // on crée un modèle, on l'imprime, et rien ne change — sans que
    // rien n'explique pourquoi.
    let actifs: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM modele_document WHERE genre = ?1 AND actif = 1",
            params![m.genre.trim()],
            |r| r.get(0),
        )
        .unwrap_or(0);
    if actifs == 0 {
        definir_actif(conn, &m.id)?;
    }
    Ok(())
}

pub fn definir_actif(conn: &Connection, id: &str) -> Result<(), String> {
    let genre: String = conn
        .query_row(
            "SELECT genre FROM modele_document WHERE id = ?1",
            params![id],
            |r| r.get(0),
        )
        .map_err(|_| "Modèle introuvable.".to_string())?;

    // Un seul actif par genre. Les deux écritures dans la même
    // transaction : entre les deux, aucun modèle n'est actif, et une
    // impression qui tomberait là n'aurait rien à quoi se raccrocher.
    conn.execute("BEGIN IMMEDIATE", []).ok();
    let r = (|| -> ResSql<()> {
        conn.execute(
            "UPDATE modele_document SET actif = 0 WHERE genre = ?1",
            params![genre],
        )?;
        conn.execute(
            "UPDATE modele_document SET actif = 1 WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    })();
    match r {
        Ok(()) => {
            conn.execute("COMMIT", []).ok();
            Ok(())
        }
        Err(e) => {
            conn.execute("ROLLBACK", []).ok();
            Err(e.to_string())
        }
    }
}

pub fn supprimer(conn: &Connection, id: &str) -> Result<(), String> {
    let (est_defaut, actif, genre): (i64, i64, String) = conn
        .query_row(
            "SELECT est_defaut, actif, genre FROM modele_document WHERE id = ?1",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .map_err(|_| "Modèle introuvable.".to_string())?;

    if est_defaut != 0 {
        return Err(
            "Un modèle d'usine ne se supprime pas. Le modifier, ou en créer une copie."
                .to_string(),
        );
    }
    conn.execute("DELETE FROM modele_document WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;

    // Supprimer l'actif laisserait le genre sans modèle : l'impression
    // retomberait silencieusement sur le générateur historique, et le
    // commerçant croirait à une panne. On désigne le suivant.
    if actif != 0 {
        if let Ok(suivant) = conn.query_row(
            "SELECT id FROM modele_document WHERE genre = ?1
             ORDER BY est_defaut DESC, nom ASC LIMIT 1",
            params![genre],
            |r| r.get::<_, String>(0),
        ) {
            definir_actif(conn, &suivant)?;
        }
    }
    Ok(())
}

// =====================================================================
//  Transport
// =====================================================================

pub fn exporter(conn: &Connection, ids: Option<Vec<String>>) -> Result<Lot, String> {
    let tous = lister(conn, None).map_err(|e| e.to_string())?;
    let modeles = match ids {
        Some(voulus) => tous
            .into_iter()
            .filter(|m| voulus.iter().any(|v| v == &m.id))
            .collect(),
        None => tous,
    };
    let societe: Option<String> = conn
        .query_row(
            "SELECT nom FROM parametres_societe WHERE id = 1",
            [],
            |r| r.get(0),
        )
        .ok();

    // Les images que ces modèles posent voyagent avec eux : un modèle
    // qui arrive sans son cachet imprimerait un blanc à sa place.
    let mut images = Vec::new();
    for id in images_posees(&modeles) {
        let nom: Option<String> = conn
            .query_row("SELECT nom FROM image_document WHERE id = ?1", params![id], |r| r.get(0))
            .ok();
        if let (Some(nom), Ok(Some(contenu))) = (nom, crate::images::lire_libre_base64(conn, &id)) {
            images.push(ImageDuLot { id, nom, contenu });
        }
    }

    Ok(Lot {
        marqueur: MARQUEUR.to_string(),
        version: VERSION_ECHANGE,
        exporte_le: maintenant_iso(),
        societe,
        modeles,
        images,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bilan {
    pub ajoutes: usize,
    pub remplaces: usize,
    pub ignores: Vec<String>,
    /// Les images du lot posées ici — celles déjà présentes ne comptent pas.
    #[serde(default)]
    pub images_ajoutees: usize,
}

/// Pose les images d'un lot, une par une, sans que l'une qui échoue
/// n'arrête les autres — même règle que pour les modèles.
///
/// `poser(id, nom, octets, dossier)` rend `Ok(true)` si l'image a été
/// posée, `Ok(false)` si elle était déjà là.
fn poser_images_du_lot(
    lot: &Lot,
    dossier: Option<&std::path::Path>,
    bilan: &mut Bilan,
    mut poser: impl FnMut(&str, &str, &[u8], &std::path::Path) -> Result<bool, String>,
) {
    if lot.images.is_empty() {
        return;
    }
    let Some(dossier) = dossier else {
        bilan.ignores.push(format!(
            "{} image(s) laissée(s) de côté : aucun dossier d'images sur ce moteur.",
            lot.images.len()
        ));
        return;
    };
    for img in &lot.images {
        let resultat = crate::images::octets_de_data_url(&img.contenu)
            .and_then(|octets| poser(&img.id, &img.nom, &octets, dossier));
        match resultat {
            Ok(true) => bilan.images_ajoutees += 1,
            Ok(false) => {}
            Err(e) => bilan.ignores.push(format!("image {} — {e}", img.nom)),
        }
    }
}

/// Reprend un lot exporté ailleurs.
///
/// Écrase par identifiant : l'import est un geste explicite, pas une
/// synchronisation de fond. Ce qui n'est PAS écrasé, c'est le choix du
/// modèle actif — il appartient à l'installation qui reçoit, pas à
/// celle qui envoie. Sans quoi importer un lot depuis la boutique du
/// cousin changerait la facture qui sort de votre imprimante.
pub fn importer(conn: &Connection, lot: &Lot, auteur: &str) -> Result<Bilan, String> {
    importer_avec_images(conn, lot, auteur, None)
}

/// L'import, avec le dossier où poser les images du lot. Sans dossier,
/// les images sont laissées de côté et le bilan le dit — le modèle
/// arrive, il imprimera un blanc à la place du cachet.
pub fn importer_avec_images(
    conn: &Connection,
    lot: &Lot,
    auteur: &str,
    dossier_images: Option<&std::path::Path>,
) -> Result<Bilan, String> {
    if lot.marqueur != MARQUEUR {
        return Err(
            "Ce fichier n'est pas un export de modèles Gescom.".to_string(),
        );
    }
    if lot.version > VERSION_ECHANGE {
        return Err(format!(
            "Ce fichier vient d'une version plus récente de Gescom \
             (format {} contre {}). Mettre à jour ce poste d'abord.",
            lot.version, VERSION_ECHANGE
        ));
    }
    if lot.modeles.is_empty() {
        return Err("Ce fichier ne contient aucun modèle.".to_string());
    }

    let mut bilan = Bilan { ajoutes: 0, remplaces: 0, ignores: Vec::new(), images_ajoutees: 0 };
    poser_images_du_lot(lot, dossier_images, &mut bilan, |id, nom, octets, dossier| {
        let deja: i64 = conn
            .query_row("SELECT COUNT(*) FROM image_document WHERE id = ?1", params![id], |r| r.get(0))
            .unwrap_or(0);
        if deja > 0 { return Ok(false) }
        crate::images::poser_libre(conn, id, nom, octets, dossier)?;
        Ok(true)
    });
    for m in &lot.modeles {
        let existe: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM modele_document WHERE id = ?1",
                params![m.id],
                |r| r.get(0),
            )
            .unwrap_or(0);
        match enregistrer(conn, m, auteur) {
            Ok(()) => {
                if existe > 0 {
                    bilan.remplaces += 1;
                } else {
                    bilan.ajoutes += 1;
                }
            }
            // Un modèle refusé ne doit pas annuler les autres : un lot
            // de six dont un est mal formé en installe cinq, et dit
            // lequel manque.
            Err(e) => bilan.ignores.push(format!("{} — {e}", m.nom)),
        }
    }
    Ok(bilan)
}

// =====================================================================
//  SUR L'UN OU L'AUTRE MOTEUR
// =====================================================================
//
// `modele_document` n'est pas cloisonnee : un modele de facture vaut
// pour toute l'installation. `definir_actif` tient ses deux ecritures
// dans une transaction `Base`, plus de `BEGIN IMMEDIATE` a la main.

use crate::base::{Acces, Base};
use crate::parametres;

const COLONNES: &str = "id, genre, nom, format, contenu, est_defaut, actif, modifie_le";

fn ligne_sur(r: &crate::base::Ligne<'_>) -> crate::base::Resultat<Modele> {
    let contenu: String = r.get::<String>(4)?;
    Ok(Modele {
        id: r.get::<String>(0)?,
        genre: r.get::<String>(1)?,
        nom: r.get::<String>(2)?,
        format: r.get::<String>(3)?,
        contenu: serde_json::from_str(&contenu).unwrap_or(serde_json::Value::Null),
        est_defaut: r.get::<i64>(5)? != 0,
        actif: r.get::<i64>(6)? != 0,
        modifie_le: r.get::<Option<String>>(7)?.unwrap_or_default(),
    })
}

pub fn lister_sur_base(base: &mut Base, genre: Option<&str>) -> Result<Vec<Modele>, String> {
    base.lire_plusieurs(
        &format!(
            "SELECT {COLONNES} FROM modele_document
             WHERE (CAST(?1 AS TEXT) IS NULL OR genre = ?1)
             ORDER BY genre ASC, est_defaut DESC, nom ASC"
        ),
        &parametres![genre],
        ligne_sur,
    )
    .map_err(|e| e.0)
}

pub fn lire_sur_base(base: &mut Base, id: &str) -> Result<Modele, String> {
    base.lire_une(&format!("SELECT {COLONNES} FROM modele_document WHERE id = ?1"), &parametres![id], ligne_sur)
        .map_err(|e| e.0)?
        .ok_or_else(|| "Modèle introuvable.".to_string())
}

pub fn lire_actif_sur_base(base: &mut Base, genre: &str) -> Option<Modele> {
    base.lire_une(
        &format!("SELECT {COLONNES} FROM modele_document WHERE genre = ?1 AND actif = 1 LIMIT 1"),
        &parametres![genre],
        ligne_sur,
    )
    .ok()
    .flatten()
}

fn definir_actif_dans(acces: &mut impl Acces, id: &str, genre: &str) -> Result<(), String> {
    acces
        .executer("UPDATE modele_document SET actif = 0 WHERE genre = ?1", &parametres![genre])
        .map_err(|e| e.0)?;
    acces
        .executer("UPDATE modele_document SET actif = 1 WHERE id = ?1", &parametres![id])
        .map_err(|e| e.0)?;
    Ok(())
}

pub fn enregistrer_sur_base(base: &mut Base, m: &Modele, auteur: &str) -> Result<(), String> {
    if m.nom.trim().is_empty() {
        return Err("Le modèle doit avoir un nom.".to_string());
    }
    if m.genre.trim().is_empty() {
        return Err("Le modèle doit avoir un genre.".to_string());
    }
    let contenu = serde_json::to_string(&m.contenu).map_err(|e| e.to_string())?;
    let maintenant = maintenant_iso();
    let mut tx = base.transaction().map_err(|e| e.0)?;
    tx.executer(
        "INSERT INTO modele_document
           (id, genre, nom, format, contenu, est_defaut, actif, cree_le, modifie_le, modifie_par)
         VALUES (?1,?2,?3,?4,?5,?6,0,?7,?7,?8)
         ON CONFLICT (id) DO UPDATE SET
            genre = excluded.genre, nom = excluded.nom, format = excluded.format,
            contenu = excluded.contenu, modifie_le = excluded.modifie_le,
            modifie_par = excluded.modifie_par",
        &parametres![m.id.clone(), m.genre.trim(), m.nom.trim(), m.format.clone(), contenu, m.est_defaut as i64, maintenant, auteur],
    )
    .map_err(|e| e.0)?;
    // Le premier modele d'un genre devient actif tout seul.
    let actifs: i64 = tx
        .lire_une(
            "SELECT COUNT(*) FROM modele_document WHERE genre = ?1 AND actif = 1",
            &parametres![m.genre.trim()],
            |r| r.get::<i64>(0),
        )
        .map_err(|e| e.0)?
        .unwrap_or(0);
    if actifs == 0 {
        definir_actif_dans(&mut tx, &m.id, m.genre.trim())?;
    }
    tx.valider().map_err(|e| e.0)
}

pub fn definir_actif_sur_base(base: &mut Base, id: &str) -> Result<(), String> {
    let genre: String = base
        .lire_une("SELECT genre FROM modele_document WHERE id = ?1", &parametres![id], |r| r.get::<String>(0))
        .map_err(|e| e.0)?
        .ok_or_else(|| "Modèle introuvable.".to_string())?;
    let mut tx = base.transaction().map_err(|e| e.0)?;
    definir_actif_dans(&mut tx, id, &genre)?;
    tx.valider().map_err(|e| e.0)
}

pub fn supprimer_sur_base(base: &mut Base, id: &str) -> Result<(), String> {
    let (est_defaut, actif, genre): (i64, i64, String) = base
        .lire_une(
            "SELECT est_defaut, actif, genre FROM modele_document WHERE id = ?1",
            &parametres![id],
            |r| Ok((r.get::<i64>(0)?, r.get::<i64>(1)?, r.get::<String>(2)?)),
        )
        .map_err(|e| e.0)?
        .ok_or_else(|| "Modèle introuvable.".to_string())?;
    if est_defaut != 0 {
        return Err("Un modèle d'usine ne se supprime pas. Le modifier, ou en créer une copie.".to_string());
    }
    let mut tx = base.transaction().map_err(|e| e.0)?;
    tx.executer("DELETE FROM modele_document WHERE id = ?1", &parametres![id]).map_err(|e| e.0)?;
    // Supprimer l'actif laisserait le genre sans modele : on designe le suivant.
    if actif != 0 {
        let suivant: Option<String> = tx
            .lire_une(
                "SELECT id FROM modele_document WHERE genre = ?1 ORDER BY est_defaut DESC, nom ASC LIMIT 1",
                &parametres![genre.clone()],
                |r| r.get::<String>(0),
            )
            .map_err(|e| e.0)?;
        if let Some(s) = suivant {
            definir_actif_dans(&mut tx, &s, &genre)?;
        }
    }
    tx.valider().map_err(|e| e.0)
}

pub fn exporter_sur_base(base: &mut Base, ids: Option<Vec<String>>) -> Result<Lot, String> {
    let tous = lister_sur_base(base, None)?;
    let modeles = match ids {
        Some(voulus) => tous.into_iter().filter(|m| voulus.iter().any(|v| v == &m.id)).collect(),
        None => tous,
    };
    let societe: Option<String> = base
        .lire_une("SELECT nom FROM parametres_societe WHERE id = 1", &[], |r| r.get::<String>(0))
        .ok()
        .flatten();
    let mut images = Vec::new();
    for id in images_posees(&modeles) {
        let nom: Option<String> = base
            .lire_une("SELECT nom FROM image_document WHERE id = ?1", &parametres![id.clone()], |r| r.get::<String>(0))
            .ok()
            .flatten();
        if let (Some(nom), Ok(Some(contenu))) = (nom, crate::images::lire_libre_base64_sur_base(base, &id)) {
            images.push(ImageDuLot { id, nom, contenu });
        }
    }
    Ok(Lot { marqueur: MARQUEUR.to_string(), version: VERSION_ECHANGE, exporte_le: maintenant_iso(), societe, modeles, images })
}

pub fn importer_sur_base(base: &mut Base, lot: &Lot, auteur: &str) -> Result<Bilan, String> {
    importer_avec_images_sur_base(base, lot, auteur, None)
}

pub fn importer_avec_images_sur_base(
    base: &mut Base,
    lot: &Lot,
    auteur: &str,
    dossier_images: Option<&std::path::Path>,
) -> Result<Bilan, String> {
    if lot.marqueur != MARQUEUR {
        return Err("Ce fichier n'est pas un export de modèles Gescom.".to_string());
    }
    if lot.version > VERSION_ECHANGE {
        return Err(format!(
            "Ce fichier vient d'une version plus récente de Gescom \
             (format {} contre {}). Mettre à jour ce poste d'abord.",
            lot.version, VERSION_ECHANGE
        ));
    }
    if lot.modeles.is_empty() {
        return Err("Ce fichier ne contient aucun modèle.".to_string());
    }
    let mut bilan = Bilan { ajoutes: 0, remplaces: 0, ignores: Vec::new(), images_ajoutees: 0 };
    poser_images_du_lot(lot, dossier_images, &mut bilan, |id, nom, octets, dossier| {
        let deja: i64 = base
            .lire_une("SELECT COUNT(*) FROM image_document WHERE id = ?1", &parametres![id.to_string()], |r| r.get::<i64>(0))
            .ok()
            .flatten()
            .unwrap_or(0);
        if deja > 0 { return Ok(false) }
        crate::images::poser_libre_sur_base(base, id, nom, octets, Some(dossier))?;
        Ok(true)
    });
    for m in &lot.modeles {
        let existe: i64 = base
            .lire_une("SELECT COUNT(*) FROM modele_document WHERE id = ?1", &parametres![m.id.clone()], |r| r.get::<i64>(0))
            .ok()
            .flatten()
            .unwrap_or(0);
        match enregistrer_sur_base(base, m, auteur) {
            Ok(()) => {
                if existe > 0 { bilan.remplaces += 1 } else { bilan.ajoutes += 1 }
            }
            Err(e) => bilan.ignores.push(format!("{} — {e}", m.nom)),
        }
    }
    Ok(bilan)
}
