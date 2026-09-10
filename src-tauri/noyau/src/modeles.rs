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
pub const VERSION_ECHANGE: u32 = 1;

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
    pub modifie_le: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lot {
    pub marqueur: String,
    pub version: u32,
    pub exporte_le: String,
    #[serde(default)]
    pub societe: Option<String>,
    pub modeles: Vec<Modele>,
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
        modifie_le: r.get(7)?,
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

    Ok(Lot {
        marqueur: MARQUEUR.to_string(),
        version: VERSION_ECHANGE,
        exporte_le: maintenant_iso(),
        societe,
        modeles,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bilan {
    pub ajoutes: usize,
    pub remplaces: usize,
    pub ignores: Vec<String>,
}

/// Reprend un lot exporté ailleurs.
///
/// Écrase par identifiant : l'import est un geste explicite, pas une
/// synchronisation de fond. Ce qui n'est PAS écrasé, c'est le choix du
/// modèle actif — il appartient à l'installation qui reçoit, pas à
/// celle qui envoie. Sans quoi importer un lot depuis la boutique du
/// cousin changerait la facture qui sort de votre imprimante.
pub fn importer(conn: &Connection, lot: &Lot, auteur: &str) -> Result<Bilan, String> {
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

    let mut bilan = Bilan { ajoutes: 0, remplaces: 0, ignores: Vec::new() };
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
