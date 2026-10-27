//! D36 — un article peut ne pas suivre son stock (`article.gere_en_stock`).
//!
//! Sachets, credit de recharge, livraison, coupe, vrac : ils se vendent
//! normalement, ne sont jamais a decouvert, sortent des ecrans de stock.
//! Leurs mouvements s'ecrivent quand meme (le stock reste la somme des
//! mouvements) : on sait ce qui est sorti, on ne le compte plus.
//!
//! Cesser de suivre est permis, trace, et dit combien d'unites sortent du
//! stock. Reprendre exige un comptage pour chaque magasin actif du
//! dossier (`coeur::stock::verifier_comptages`) : le stock repart de la
//! quantite reelle, jamais d'un chiffre faux.
//!
//! L'article est commun aux dossiers (D3 du plan multi-societe) : la case
//! vaut pour tous. Reprendre compte les magasins du dossier ouvert ; la
//! reponse nomme les autres dossiers ou l'article a encore un stock, a
//! compter a leur tour.

use crate::base::{Acces, Base};
use crate::parametres;

/// L'article suit-il son stock ? (Inconnu : oui — le comportement d'avant.)
pub fn est_suivi_sur(acces: &mut impl Acces, article_id: &str) -> bool {
    acces
        .lire_une("SELECT gere_en_stock FROM article WHERE id = ?1", &parametres![article_id.to_string()], |r| r.get::<i64>(0))
        .ok()
        .flatten()
        .map(|v| v != 0)
        .unwrap_or(true)
}

/// Voir `est_suivi_sur` (facades `Connection`).
pub fn est_suivi(conn: &rusqlite::Connection, article_id: &str) -> bool {
    conn.query_row("SELECT gere_en_stock FROM article WHERE id = ?1", rusqlite::params![article_id], |r| r.get::<_, i64>(0))
        .map(|v| v != 0)
        .unwrap_or(true)
}

/// Le refus commun des ecrans de stock (transfert, inventaire).
pub fn refus_non_suivi(nom: &str) -> String {
    format!(
        "« {nom} » ne suit pas son stock : rien à transférer ni à compter. \
         Reprendre le suivi d'abord (Paramètres → Articles)."
    )
}

/// Refuse un geste de stock (transfert, inventaire) sur un article qui
/// ne suit pas son stock — en le nommant.
pub fn exiger_suivi_sur(acces: &mut impl Acces, article_id: &str) -> Result<(), String> {
    if est_suivi_sur(acces, article_id) {
        return Ok(());
    }
    let nom: String = acces
        .lire_une("SELECT nom FROM article WHERE id = ?1", &parametres![article_id.to_string()], |r| r.get::<String>(0))
        .ok()
        .flatten()
        .unwrap_or_else(|| article_id.to_string());
    Err(refus_non_suivi(&nom))
}

/// Voir `exiger_suivi_sur`.
pub fn exiger_suivi(conn: &rusqlite::Connection, article_id: &str) -> Result<(), String> {
    if est_suivi(conn, article_id) {
        return Ok(());
    }
    let nom: String = conn
        .query_row("SELECT nom FROM article WHERE id = ?1", rusqlite::params![article_id], |r| r.get(0))
        .unwrap_or_else(|_| article_id.to_string());
    Err(refus_non_suivi(&nom))
}

/// Cesser ou reprendre le suivi du stock d'un article.
///
/// `comptages` : (magasin, quantite comptee, en unite de base) — exiges
/// pour reprendre, ignores pour cesser.
pub fn changer_suivi_stock_sur(
    base: &mut Base,
    article_id: String,
    suivre: bool,
    comptages: Vec<(String, f64)>,
) -> Result<serde_json::Value, String> {
    let dossier = base.dossier().to_string();
    let (nom, actuel): (String, i64) = base
        .lire_une(
            "SELECT nom, gere_en_stock FROM article WHERE id = ?1",
            &parametres![article_id.clone()],
            |r| Ok((r.get::<String>(0)?, r.get::<i64>(1)?)),
        )
        .map_err(|e| e.0)?
        .ok_or_else(|| "Article introuvable".to_string())?;
    if (actuel != 0) == suivre {
        return Err(if suivre {
            format!("« {nom} » suit déjà son stock.")
        } else {
            format!("« {nom} » ne suit déjà plus son stock.")
        });
    }

    // Le stock de l'article, magasin par magasin, dans ce dossier.
    let stocks: Vec<(String, f64)> = base
        .lire_plusieurs(
            "SELECT depot_id, COALESCE(quantite, 0) FROM stock_depot
             WHERE article_id = ?1 AND dossier_id = ?2",
            &parametres![article_id.clone(), dossier.clone()],
            |r| Ok((r.get::<String>(0)?, r.get::<f64>(1)?)),
        )
        .map_err(|e| e.0)?;
    let stock_de = |depot: &str| stocks.iter().find(|(d, _)| d == depot).map(|(_, q)| *q).unwrap_or(0.0);

    // A reprendre : les comptages, juges avant toute ecriture.
    let comptes = if suivre {
        let magasins: Vec<(String, String)> = base
            .lire_plusieurs(
                "SELECT id, nom FROM depot WHERE actif = 1 AND dossier_id = ?1 ORDER BY nom, id",
                &parametres![dossier.clone()],
                |r| Ok((r.get::<String>(0)?, r.get::<String>(1)?)),
            )
            .map_err(|e| e.0)?;
        crate::coeur::stock::verifier_comptages(&magasins, &comptages)?
    } else {
        Vec::new()
    };
    // Les autres dossiers ou l'article a du stock : la case vaut pour eux
    // aussi, on les nomme.
    let autres: Vec<String> = base
        .lire_plusieurs(
            "SELECT DISTINCT d.societe FROM stock_depot sd
             JOIN dossier d ON d.id = sd.dossier_id
             WHERE sd.article_id = ?1 AND sd.dossier_id <> ?2 AND sd.quantite <> 0",
            &parametres![article_id.clone(), dossier.clone()],
            |r| r.get::<String>(0),
        )
        .unwrap_or_default();

    let maintenant = crate::utils::maintenant_iso();
    let auteur = crate::argent::id_utilisateur_courant_sur(base);
    let sortis: f64 = stocks.iter().map(|(_, q)| *q).sum();

    let mut tx = base.transaction().map_err(|e| e.0)?;
    for (depot, compte) in &comptes {
        let delta = compte - stock_de(depot);
        if delta != 0.0 {
            tx.executer(
                "INSERT INTO mouvement_stock
                 (id, article_id, depot_id, type_mouvement, quantite_delta,
                  motif, operation_id, auteur_id, date_mouvement, cree_le, cree_par, origine, dossier_id)
                 VALUES (?1,?2,?3,'ajustement',?4,'Reprise du suivi du stock',?5,?6,?7,?7,?6,'app',?8)",
                &parametres![
                    uuid::Uuid::new_v4().to_string(),
                    article_id.clone(),
                    depot.clone(),
                    delta,
                    uuid::Uuid::new_v4().to_string(),
                    auteur.clone(),
                    maintenant.clone(),
                    dossier.clone()
                ],
            )
            .map_err(|e| e.0)?;
        }
    }
    tx.executer(
        "UPDATE article SET gere_en_stock = ?1, modifie_le = ?2, modifie_par = ?3 WHERE id = ?4",
        &parametres![if suivre { 1i64 } else { 0i64 }, maintenant.clone(), auteur.clone(), article_id.clone()],
    )
    .map_err(|e| e.0)?;
    let detail = if suivre {
        serde_json::json!({ "suivi": true, "comptages": comptes.iter().map(|(d, q)| serde_json::json!({ "depot_id": d, "quantite": q })).collect::<Vec<_>>() })
    } else {
        serde_json::json!({ "suivi": false, "sortis_du_stock": sortis })
    };
    tx.executer(
        "INSERT INTO journal
           (id, type_evenement, entite_type, entite_id, auteur_id,
            ancien_valeur, nouveau_valeur, origine, date_evenement, dossier_id)
         VALUES (?1,'suivi_stock_change','article',?2,?3,?4,?5,'app',?6,?7)",
        &parametres![
            uuid::Uuid::new_v4().to_string(),
            article_id.clone(),
            auteur,
            serde_json::json!({ "nom": nom, "suivi": !suivre }).to_string(),
            detail.to_string(),
            maintenant,
            dossier
        ],
    )
    .map_err(|e| e.0)?;
    tx.valider().map_err(|e| e.0)?;

    Ok(serde_json::json!({
        "article_id": article_id,
        "nom": nom,
        "suivi": suivre,
        // Cesser : ce qui sort du stock et de sa valeur (unite de base).
        "sortis_du_stock": if suivre { 0.0 } else { sortis },
        "autres_dossiers": autres,
    }))
}
