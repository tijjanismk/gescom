//! Transferts inter-dépôts.
//!
//! Un transfert déplace de la marchandise entre deux dépôts du MÊME
//! propriétaire. Ce n'est ni une vente ni un achat : aucun chiffre
//! d'affaires, aucune créance, aucun mouvement de caisse.
//!
//! Pourquoi pas traiter les magasins comme des clients et fournisseurs,
//! comme le fait le cahier Excel : ça créerait un CA fictif, le stock
//! serait compté deux fois — sorti d'un magasin, entré dans l'autre —
//! et une marge inventée apparaîtrait entre les deux.
//!
//! Le document produit est un BON DE TRANSFERT numéroté (BTR-AAAA-NNNNN),
//! imprimable et signable par le gérant qui reçoit.

use tauri::State;
use serde::Deserialize;
use crate::commandes::ventes::EtatApp;
use crate::utils::maintenant_iso;

#[derive(Deserialize)]
pub struct LigneTransfert {
    pub article_id: String,
    pub unite_vente_id: String,
    /// Quantité dans l'unité choisie (sac, carton…).
    pub quantite: f64,
    /// Combien d'unités de base vaut cette unité.
    pub facteur: f64,
}

/// Numéro de bon de transfert — même règle que `prochain_numero` :
/// MAX et non COUNT, sinon une suppression rejoue un numéro.
fn prochain_bon(conn: &rusqlite::Connection) -> String {
    let annee = chrono::Local::now().format("%Y").to_string();
    let motif = format!("BTR-{}-%", annee);
    let dernier: i64 = conn.query_row(
        "SELECT COALESCE(MAX(CAST(substr(bon, -5) AS INTEGER)), 0)
         FROM transfert WHERE bon LIKE ?1",
        rusqlite::params![motif],
        |r| r.get(0),
    ).unwrap_or(0);
    format!("BTR-{}-{:05}", annee, dernier + 1)
}

/// Enregistre un transfert complet, dans une transaction unique.
///
/// Le stock sort du dépôt source et entre dans le dépôt destination.
/// Deux mouvements de stock par ligne, comme une vente et un achat
/// symétriques — mais sans argent.
#[tauri::command]
pub fn enregistrer_transfert(
    etat: State<EtatApp>,
    depot_source: String,
    depot_dest: String,
    lignes: Vec<LigneTransfert>,
    motif: Option<String>,
    utilisateur_role: Option<String>,
) -> Result<serde_json::Value, String> {
    if lignes.is_empty() {
        return Err("Aucune ligne à transférer".to_string());
    }
    if depot_source == depot_dest {
        return Err("Les dépôts source et destination sont identiques".to_string());
    }

    let mut conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let now = maintenant_iso();
    let role = utilisateur_role.as_deref().unwrap_or("employe");
    let auteur = crate::commandes::ventes::id_utilisateur_par_role(&conn, role);

    // Vérifier le stock disponible AVANT d'ouvrir la transaction : un
    // transfert ne doit pas mettre le dépôt source à découvert. Une
    // vente le peut (article commandé au voisin), pas un transfert.
    for l in &lignes {
        let quantite_base = l.quantite * l.facteur;
        let dispo: f64 = conn.query_row(
            "SELECT COALESCE(quantite, 0) FROM stock_depot
             WHERE article_id = ?1 AND depot_id = ?2",
            rusqlite::params![l.article_id, depot_source],
            |r| r.get(0),
        ).unwrap_or(0.0);

        if dispo < quantite_base - 1e-9 {
            let nom: String = conn.query_row(
                "SELECT nom FROM article WHERE id = ?1",
                rusqlite::params![l.article_id], |r| r.get(0),
            ).unwrap_or_else(|_| "?".to_string());
            return Err(format!(
                "Stock insuffisant pour « {} » : {} disponible(s), \
                 {} demandé(s).",
                nom, dispo, quantite_base
            ));
        }
    }

    let bon = prochain_bon(&conn);
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let op_id = uuid::Uuid::new_v4().to_string();
    let mut nb = 0;

    for l in &lignes {
        let quantite_base = l.quantite * l.facteur;

        // Sortie du dépôt source.
        tx.execute(
            "INSERT INTO stock_depot (id, article_id, depot_id, quantite)
             VALUES (?1,?2,?3, 0 - ?4)
             ON CONFLICT(article_id, depot_id)
             DO UPDATE SET quantite = quantite - ?4",
            rusqlite::params![
                uuid::Uuid::new_v4().to_string(),
                l.article_id, depot_source, quantite_base
            ],
        ).map_err(|e| e.to_string())?;

        // Entrée dans le dépôt destination.
        tx.execute(
            "INSERT INTO stock_depot (id, article_id, depot_id, quantite)
             VALUES (?1,?2,?3,?4)
             ON CONFLICT(article_id, depot_id)
             DO UPDATE SET quantite = quantite + ?4",
            rusqlite::params![
                uuid::Uuid::new_v4().to_string(),
                l.article_id, depot_dest, quantite_base
            ],
        ).map_err(|e| e.to_string())?;

        // Deux mouvements, pour que l'historique de chaque dépôt soit
        // complet quand on le consulte séparément.
        for (dep, delta) in [
            (&depot_source, -quantite_base),
            (&depot_dest,    quantite_base),
        ] {
            tx.execute(
                "INSERT INTO mouvement_stock
                 (id, article_id, depot_id, type_mouvement, quantite_delta,
                  motif, operation_id, auteur_id, date_mouvement,
                  cree_le, cree_par, origine)
                 VALUES (?1,?2,?3,'transfert',?4,?5,?6,?7,?8,?9,?10,'app')",
                rusqlite::params![
                    uuid::Uuid::new_v4().to_string(),
                    l.article_id, dep, delta,
                    bon, op_id, auteur, now, now, auteur
                ],
            ).map_err(|e| e.to_string())?;
        }

        tx.execute(
            "INSERT INTO transfert
             (id, bon, article_id, depot_source, depot_dest, quantite,
              unite_vente_id, motif, auteur_id, date_transfert, cree_le, origine)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,'app')",
            rusqlite::params![
                uuid::Uuid::new_v4().to_string(), bon,
                l.article_id, depot_source, depot_dest, l.quantite,
                l.unite_vente_id, motif, auteur, now, now
            ],
        ).map_err(|e| e.to_string())?;

        nb += 1;
    }

    tx.execute(
        "INSERT INTO journal
         (id, type_evenement, entite_type, entite_id, auteur_id,
          nouveau_valeur, origine, date_evenement)
         VALUES (?1,'transfert','transfert',?2,?3,?4,'app',?5)",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(), bon, auteur,
            format!(r#"{{"lignes":{},"de":"{}","vers":"{}"}}"#,
                    nb, depot_source, depot_dest),
            now
        ],
    ).ok();

    tx.commit().map_err(|e| e.to_string())?;

    Ok(serde_json::json!({ "bon": bon, "nb_lignes": nb }))
}

/// Historique des transferts, groupés par bon.
#[tauri::command]
pub fn lire_transferts(
    etat: State<EtatApp>,
    limite: Option<i64>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let lim = limite.unwrap_or(100);

    let mut st = conn.prepare(
        "SELECT t.bon, MIN(t.date_transfert), ds.nom, dd.nom,
                COUNT(*), COALESCE(u.nom, '—'), MAX(COALESCE(t.motif,''))
         FROM transfert t
         JOIN depot ds ON ds.id = t.depot_source
         JOIN depot dd ON dd.id = t.depot_dest
         LEFT JOIN utilisateur u ON u.id = t.auteur_id
         GROUP BY t.bon
         ORDER BY MIN(t.date_transfert) DESC
         LIMIT ?1"
    ).map_err(|e| e.to_string())?;

    let bons: Vec<serde_json::Value> = st.query_map(
        rusqlite::params![lim], |r| {
            Ok(serde_json::json!({
                "bon":           r.get::<_, Option<String>>(0)?,
                "date":          r.get::<_, String>(1)?,
                "depot_source":  r.get::<_, String>(2)?,
                "depot_dest":    r.get::<_, String>(3)?,
                "nb_lignes":     r.get::<_, i64>(4)?,
                "auteur":        r.get::<_, String>(5)?,
                "motif":         r.get::<_, String>(6)?,
            }))
        }
    ).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    Ok(bons)
}

/// Détail d'un bon — pour l'affichage et l'impression.
#[tauri::command]
pub fn lire_bon_transfert(
    etat: State<EtatApp>,
    bon: String,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let mut st = conn.prepare(
        "SELECT a.nom, COALESCE(uv.libelle, a.unite_base), t.quantite,
                ds.nom, dd.nom, t.date_transfert,
                COALESCE(u.nom, '—'), COALESCE(t.motif, '')
         FROM transfert t
         JOIN article a ON a.id = t.article_id
         LEFT JOIN unite_vente uv ON uv.id = t.unite_vente_id
         JOIN depot ds ON ds.id = t.depot_source
         JOIN depot dd ON dd.id = t.depot_dest
         LEFT JOIN utilisateur u ON u.id = t.auteur_id
         WHERE t.bon = ?1
         ORDER BY a.nom"
    ).map_err(|e| e.to_string())?;

    let lignes: Vec<serde_json::Value> = st.query_map(
        rusqlite::params![bon], |r| {
            Ok(serde_json::json!({
                "article":  r.get::<_, String>(0)?,
                "unite":    r.get::<_, String>(1)?,
                "quantite": r.get::<_, f64>(2)?,
            }))
        }
    ).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    if lignes.is_empty() {
        return Err("Bon de transfert introuvable".to_string());
    }

    let (src, dst, date, auteur, motif): (String, String, String, String, String) =
        conn.query_row(
            "SELECT ds.nom, dd.nom, t.date_transfert,
                    COALESCE(u.nom,'—'), COALESCE(t.motif,'')
             FROM transfert t
             JOIN depot ds ON ds.id = t.depot_source
             JOIN depot dd ON dd.id = t.depot_dest
             LEFT JOIN utilisateur u ON u.id = t.auteur_id
             WHERE t.bon = ?1 LIMIT 1",
            rusqlite::params![bon],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        ).map_err(|e| e.to_string())?;

    let societe = conn.query_row(
        "SELECT nom, adresse, telephone FROM parametres_societe WHERE id = 1",
        [], |r| Ok(serde_json::json!({
            "nom":       r.get::<_, String>(0)?,
            "adresse":   r.get::<_, Option<String>>(1)?,
            "telephone": r.get::<_, Option<String>>(2)?,
        })),
    ).unwrap_or(serde_json::json!({"nom":"","adresse":null,"telephone":null}));

    Ok(serde_json::json!({
        "bon": bon, "depot_source": src, "depot_dest": dst,
        "date": date, "auteur": auteur, "motif": motif,
        "lignes": lignes, "societe": societe,
    }))
}
