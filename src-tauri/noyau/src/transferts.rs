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

use serde::Deserialize;
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

/// Reserve le prochain numero de bon de transfert.
///
/// Meme compteur transactionnel que les pieces
/// ([`crate::argent::reserver_numero`]) : lire le plus grand bon deja
/// ecrit laissait deux transferts simultanes repartir avec BTR-2026-00007.
///
/// **Consomme un numero a chaque appel** — ce n'est pas un apercu.
pub fn reserver_bon(conn: &rusqlite::Connection) -> Result<String, String> {
    let annee = chrono::Local::now().format("%Y").to_string();
    let rang = crate::argent::suivant(conn, &format!("BTR-{annee}"))?;
    Ok(format!("BTR-{annee}-{rang:05}"))
}

/// Enregistre un transfert complet, dans une transaction unique.
///
/// Le stock sort du dépôt source et entre dans le dépôt destination.
/// Deux mouvements de stock par ligne, comme une vente et un achat
/// symétriques — mais sans argent.
pub fn enregistrer_transfert(
    conn: &mut rusqlite::Connection,
    depot_source: String,
    depot_dest: String,
    lignes: Vec<LigneTransfert>,
    motif: Option<String>,
    utilisateur_role: Option<String>,
) -> Result<serde_json::Value, String> {
    enregistrer_transfert_sur(conn, depot_source, depot_dest, lignes,
                              motif, utilisateur_role)
}

/// Logique de `enregistrer_transfert`, sur une connexion quelconque.
///
/// Separee de la commande pour etre jouable sur une base de test :
/// les scenarios de `tests_multi_depot` verifient ce que le SQL fait
/// reellement a la base, ce qu'aucun test de formule ne montre.
pub fn enregistrer_transfert_sur(
    conn: &mut rusqlite::Connection,
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
        return Err("Les magasins source et destination sont identiques".to_string());
    }

    let now = maintenant_iso();
    let role = utilisateur_role.as_deref().unwrap_or("employe");
    let auteur = crate::argent::id_utilisateur_par_role(&conn, role);

    // Une quantité nulle ou négative inverserait le sens du transfert
    // en passant sous le contrôle de stock : le dépôt destination
    // serait vidé au profit de la source.
    for l in &lignes {
        if l.quantite <= 0.0 || l.facteur <= 0.0 {
            return Err("Chaque ligne doit porter une quantité positive".to_string());
        }
    }

    // Vérifier le stock disponible AVANT d'ouvrir la transaction : un
    // transfert ne doit pas mettre le dépôt source à découvert. Une
    // vente le peut (article commandé au voisin), pas un transfert.
    //
    // Le contrôle porte sur le TOTAL par article, et non ligne à ligne :
    // deux lignes de 10 sur un stock de 15 passaient chacune leur test
    // séparément et laissaient la source à -5.
    let mut demande: Vec<(String, f64)> = Vec::new();
    for l in &lignes {
        let quantite_base = l.quantite * l.facteur;
        match demande.iter_mut().find(|(a, _)| *a == l.article_id) {
            Some((_, q)) => *q += quantite_base,
            None => demande.push((l.article_id.clone(), quantite_base)),
        }
    }

    for (article_id, quantite_base) in &demande {
        let dispo: f64 = conn.query_row(
            "SELECT COALESCE(quantite, 0) FROM stock_depot
             WHERE article_id = ?1 AND depot_id = ?2",
            rusqlite::params![article_id, depot_source],
            |r| r.get(0),
        ).unwrap_or(0.0);

        if dispo < *quantite_base - 1e-9 {
            let nom: String = conn.query_row(
                "SELECT nom FROM article WHERE id = ?1",
                rusqlite::params![article_id], |r| r.get(0),
            ).unwrap_or_else(|_| "?".to_string());
            return Err(format!(
                "Stock insuffisant pour « {} » : {} disponible(s), \
                 {} demandé(s).",
                nom, dispo, quantite_base
            ));
        }
    }

    let tx = conn.transaction().map_err(|e| e.to_string())?;
    // Le numero est reserve DANS la transaction : si le transfert
    // echoue plus bas, le retour arriere annule aussi l'increment et
    // la serie n'a pas de trou.
    let bon = reserver_bon(&tx)?;
    let op_id = uuid::Uuid::new_v4().to_string();
    let mut nb = 0;

    for l in &lignes {
        let quantite_base = l.quantite * l.facteur;

        // Sortie du dépôt source.
        // Le stock suit desormais son mouvement : le declencheur
        // `stock_suit_les_mouvements` met le compteur a jour dans la meme
        // transaction. L'ecrire ici le compterait deux fois.

        // Entrée dans le dépôt destination.
        // Le stock suit desormais son mouvement : le declencheur
        // `stock_suit_les_mouvements` met le compteur a jour dans la meme
        // transaction. L'ecrire ici le compterait deux fois.

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
pub fn lire_transferts(
    conn: &rusqlite::Connection,
    limite: Option<i64>,
) -> Result<Vec<serde_json::Value>, String> {
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
pub fn lire_bon_transfert(
    conn: &rusqlite::Connection,
    bon: String,
) -> Result<serde_json::Value, String> {

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

// =====================================================================
//  SUR L'UN OU L'AUTRE MOTEUR
// =====================================================================
//
// `transfert`, `mouvement_stock`, `stock_depot`, `depot`, `journal`
// sont cloisonnes. Le numero de bon passe par `suivant_sur`, qui pose
// le prefixe du dossier : deux societes ont chacune leur suite BTR.

use crate::base::{Acces, Base};
use crate::parametres;

pub fn reserver_bon_sur(acces: &mut impl Acces) -> Result<String, String> {
    let annee = chrono::Local::now().format("%Y").to_string();
    let rang = crate::argent::suivant_sur(acces, &format!("BTR-{annee}"))?;
    Ok(format!("BTR-{annee}-{rang:05}"))
}

pub fn enregistrer_transfert_sur_base(
    base: &mut Base,
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
        return Err("Les magasins source et destination sont identiques".to_string());
    }
    let dossier = base.dossier().to_string();
    let now = maintenant_iso();
    let role = utilisateur_role.as_deref().unwrap_or("employe");
    let auteur = crate::argent::id_utilisateur_par_role_sur(base, role);

    for l in &lignes {
        if l.quantite <= 0.0 || l.facteur <= 0.0 {
            return Err("Chaque ligne doit porter une quantité positive".to_string());
        }
    }

    // Le controle porte sur le TOTAL par article (D32).
    let mut demande: Vec<(String, f64)> = Vec::new();
    for l in &lignes {
        let quantite_base = l.quantite * l.facteur;
        match demande.iter_mut().find(|(a, _)| *a == l.article_id) {
            Some((_, q)) => *q += quantite_base,
            None => demande.push((l.article_id.clone(), quantite_base)),
        }
    }
    for (article_id, quantite_base) in &demande {
        let dispo: f64 = base
            .lire_une(
                "SELECT COALESCE(quantite, 0) FROM stock_depot
                 WHERE article_id = ?1 AND depot_id = ?2 AND dossier_id = ?3",
                &parametres![article_id.clone(), depot_source.clone(), dossier.clone()],
                |r| r.get::<f64>(0),
            )
            .ok()
            .flatten()
            .unwrap_or(0.0);
        if dispo < *quantite_base - 1e-9 {
            let nom: String = base
                .lire_une("SELECT nom FROM article WHERE id = ?1", &parametres![article_id.clone()], |r| r.get::<String>(0))
                .ok()
                .flatten()
                .unwrap_or_else(|| "?".to_string());
            return Err(format!(
                "Stock insuffisant pour « {} » : {} disponible(s), {} demandé(s).",
                nom, dispo, quantite_base
            ));
        }
    }

    let mut tx = base.transaction().map_err(|e| e.0)?;
    let bon = reserver_bon_sur(&mut tx)?;
    let op_id = uuid::Uuid::new_v4().to_string();
    let mut nb = 0;

    for l in &lignes {
        let quantite_base = l.quantite * l.facteur;
        // Deux mouvements, pour que l'historique de chaque magasin
        // soit complet ; le declencheur tient les compteurs.
        for (dep, delta) in [(&depot_source, -quantite_base), (&depot_dest, quantite_base)] {
            tx.executer(
                "INSERT INTO mouvement_stock
                 (id, article_id, depot_id, type_mouvement, quantite_delta,
                  motif, operation_id, auteur_id, date_mouvement,
                  cree_le, cree_par, origine, dossier_id)
                 VALUES (?1,?2,?3,'transfert',?4,?5,?6,?7,?8,?8,?7,'app',?9)",
                &parametres![
                    uuid::Uuid::new_v4().to_string(), l.article_id.clone(), dep.clone(), delta,
                    bon.clone(), op_id.clone(), auteur.clone(), now.clone(), dossier.clone()
                ],
            )
            .map_err(|e| e.0)?;
        }
        tx.executer(
            "INSERT INTO transfert
             (id, bon, article_id, depot_source, depot_dest, quantite,
              unite_vente_id, motif, auteur_id, date_transfert, cree_le, origine, dossier_id)
             VALUES (?1,?2,?3,?4,?5,?6,?7,CAST(?8 AS TEXT),?9,?10,?10,'app',?11)",
            &parametres![
                uuid::Uuid::new_v4().to_string(), bon.clone(), l.article_id.clone(),
                depot_source.clone(), depot_dest.clone(), l.quantite,
                l.unite_vente_id.clone(), motif.clone(), auteur.clone(), now.clone(), dossier.clone()
            ],
        )
        .map_err(|e| e.0)?;
        nb += 1;
    }

    let _ = tx.executer(
        "INSERT INTO journal
         (id, type_evenement, entite_type, entite_id, auteur_id,
          nouveau_valeur, origine, date_evenement, dossier_id)
         VALUES (?1,'transfert','transfert',?2,?3,?4,'app',?5,?6)",
        &parametres![
            uuid::Uuid::new_v4().to_string(), bon.clone(), auteur,
            format!(r#"{{"lignes":{},"de":"{}","vers":"{}"}}"#, nb, depot_source, depot_dest),
            now, dossier
        ],
    );
    tx.valider().map_err(|e| e.0)?;
    Ok(serde_json::json!({ "bon": bon, "nb_lignes": nb }))
}

pub fn lire_transferts_sur_base(base: &mut Base, limite: Option<i64>) -> Result<Vec<serde_json::Value>, String> {
    let dossier = base.dossier().to_string();
    base.lire_plusieurs(
        "SELECT t.bon, MIN(t.date_transfert), ds.nom, dd.nom,
                COUNT(*), COALESCE(MIN(u.nom), '—'), MAX(COALESCE(t.motif,''))
         FROM transfert t
         JOIN depot ds ON ds.id = t.depot_source
         JOIN depot dd ON dd.id = t.depot_dest
         LEFT JOIN utilisateur u ON u.id = t.auteur_id
         WHERE t.dossier_id = ?2
         GROUP BY t.bon, ds.nom, dd.nom
         ORDER BY MIN(t.date_transfert) DESC
         LIMIT ?1",
        &parametres![limite.unwrap_or(100), dossier],
        |r| {
            Ok(serde_json::json!({
                "bon":          r.get::<Option<String>>(0)?,
                "date":         r.get::<String>(1)?,
                "depot_source": r.get::<String>(2)?,
                "depot_dest":   r.get::<String>(3)?,
                "nb_lignes":    r.get::<i64>(4)?,
                "auteur":       r.get::<String>(5)?,
                "motif":        r.get::<String>(6)?,
            }))
        },
    )
    .map_err(|e| e.0)
}

pub fn lire_bon_transfert_sur_base(base: &mut Base, bon: String) -> Result<serde_json::Value, String> {
    let dossier = base.dossier().to_string();
    let lignes: Vec<serde_json::Value> = base
        .lire_plusieurs(
            "SELECT a.nom, COALESCE(uv.libelle, a.unite_base), t.quantite
             FROM transfert t
             JOIN article a ON a.id = t.article_id
             LEFT JOIN unite_vente uv ON uv.id = t.unite_vente_id
             WHERE t.bon = ?1 AND t.dossier_id = ?2
             ORDER BY a.nom",
            &parametres![bon.clone(), dossier.clone()],
            |r| {
                Ok(serde_json::json!({
                    "article":  r.get::<String>(0)?,
                    "unite":    r.get::<String>(1)?,
                    "quantite": r.get::<f64>(2)?,
                }))
            },
        )
        .map_err(|e| e.0)?;
    if lignes.is_empty() {
        return Err("Bon de transfert introuvable".to_string());
    }

    let (src, dst, date, auteur, motif): (String, String, String, String, String) = base
        .lire_une(
            "SELECT ds.nom, dd.nom, t.date_transfert, COALESCE(u.nom,'—'), COALESCE(t.motif,'')
             FROM transfert t
             JOIN depot ds ON ds.id = t.depot_source
             JOIN depot dd ON dd.id = t.depot_dest
             LEFT JOIN utilisateur u ON u.id = t.auteur_id
             WHERE t.bon = ?1 AND t.dossier_id = ?2 LIMIT 1",
            &parametres![bon.clone(), dossier],
            |r| Ok((r.get::<String>(0)?, r.get::<String>(1)?, r.get::<String>(2)?, r.get::<String>(3)?, r.get::<String>(4)?)),
        )
        .map_err(|e| e.0)?
        .ok_or_else(|| "Bon de transfert introuvable".to_string())?;

    let societe = base
        .lire_une(
            "SELECT nom, adresse, telephone FROM parametres_societe WHERE id = 1",
            &[],
            |r| {
                Ok(serde_json::json!({
                    "nom":       r.get::<String>(0)?,
                    "adresse":   r.get::<Option<String>>(1)?,
                    "telephone": r.get::<Option<String>>(2)?,
                }))
            },
        )
        .ok()
        .flatten()
        .unwrap_or_else(|| serde_json::json!({"nom":"","adresse":null,"telephone":null}));

    Ok(serde_json::json!({
        "bon": bon, "depot_source": src, "depot_dest": dst,
        "date": date, "auteur": auteur, "motif": motif,
        "lignes": lignes, "societe": societe,
    }))
}
