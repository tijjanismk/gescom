//! Livraison et reception — le document qui constate le mouvement reel.
//!
//! ## Ce que ce module faisait, et ce qu'il fait maintenant
//!
//! Il n'etait qu'un axe d'information : il notait ce qui etait parti,
//! sans toucher au stock. Le stock sortait a `valider_facture`.
//!
//! C'etait faux dans le seul cas ou la livraison sert a quelque chose :
//! une commande facturee lundi et livree jeudi faisait sortir la
//! marchandise lundi, alors qu'elle etait encore dans le magasin. Le
//! stock informatique et les sacs empiles ne disaient pas la meme
//! chose.
//!
//! Desormais **le stock bouge une fois, au premier document qui
//! constate le mouvement physique** :
//!
//! - un bon de livraison fait SORTIR ce qu'on y declare livre ;
//! - un bon de reception fait ENTRER ce qu'on y declare recu ;
//! - une facture qui descend d'un bon ne bouge plus rien — voir
//!   [`crate::pieces::stock_confie_a_un_bon`] ;
//! - une facture sans bon en amont sort le stock elle-meme, comme
//!   avant. C'est le cas de toutes les pieces deja en base, et du
//!   commercant qui remet la marchandise au comptoir : rien ne change
//!   pour lui.
//!
//! Le mouvement porte sur l'ECART, jamais sur le total : corriger
//! « 6 livres » en « 7 livres » sort une unite, pas sept.
//!
//! Ce module ne touche toujours pas a la caisse. Paiement et livraison
//! restent deux axes independants, ce qui rend representable le cas
//! courant « paye, pas encore livre » (et son symetrique, « livre, pas
//! encore paye »).
//!
//! Reglage desactive par defaut : la plupart des commercants vises
//! remettent la marchandise au comptoir, la livraison n'existe pas chez
//! eux et l'ecran ne doit pas s'encombrer. Meme principe que le bon de
//! sortie (parametres.rs).

use serde::Deserialize;
use crate::utils::maintenant_iso;

/// Tolerance de comparaison sur des quantites en REAL.
///
/// `SUM(quantite_livree) >= SUM(quantite)` sur des flottants ne tombe
/// jamais juste apres quelques additions : sans marge, une commande
/// entierement livree resterait affichee « partiel » pour un
/// milliardieme d'unite.
const EPSILON: f64 = 0.0001;

/// Etat de livraison derive des quantites, jamais stocke.
///
/// Un statut stocke se desynchronise des lignes des qu'une quantite
/// change. Ici il se recalcule, donc il ne peut pas mentir.
pub fn etat(livree: f64, commandee: f64) -> &'static str {
    if commandee <= EPSILON {
        "sans_objet"
    } else if livree <= EPSILON {
        "non_livre"
    } else if livree >= commandee - EPSILON {
        "livre"
    } else {
        "partiel"
    }
}

// Le reglage `suivi_livraison_actif` vit dans `parametres.rs`, avec le
// scanner et le bon de sortie : les bascules d'ecran sont regroupees la,
// pas eparpillees dans chaque module metier.

// =====================================================================
//  LE MOUVEMENT DE STOCK
// =====================================================================

/// Le sens du mouvement, selon ce que la piece constate.
///
/// Cote client la marchandise part, cote fournisseur elle arrive : meme
/// geste, sens opposes. Une commande ou une facture ne constatent aucun
/// mouvement physique — elles ne bougent rien.
fn sens(type_piece: &str) -> Option<(f64, &'static str)> {
    use crate::coeur::stock;
    match type_piece {
        "bon_livraison" => Some((-1.0, stock::LIVRAISON)),
        "bon_reception" => Some((1.0, stock::RECEPTION)),
        _ => None,
    }
}

/// Ecrit le mouvement correspondant a un ECART de livraison.
///
/// L'ecart, jamais le total : corriger « 6 livres » en « 7 livres »
/// sort une unite, pas sept. Un ecart negatif — le livreur revient avec
/// deux sacs refuses — fait rentrer la marchandise.
#[allow(clippy::too_many_arguments)]
fn mouvementer_ecart(
    conn: &rusqlite::Connection,
    type_piece: &str,
    depot: Option<&str>,
    article_id: &str,
    unite_id: &str,
    ecart: f64,
    auteur: &str,
    now: &str,
    // Le document qui constate le mouvement. C'est par lui que
    // l'historique de stock retrouve le numero a afficher : sans lui,
    // le mouvement existe mais ne se rattache a rien, et personne ne
    // peut verifier d'ou il vient.
    piece_id: &str,
) -> Result<(), String> {
    let Some((signe, type_mouvement)) = sens(type_piece) else {
        return Ok(());
    };
    if ecart.abs() <= EPSILON {
        return Ok(());
    }

    // Pas de depot actif : on REFUSE au lieu d'ecrire une livraison
    // dont la marchandise ne sort de nulle part. Le stock et le bon
    // raconteraient deux histoires differentes, sans rien pour
    // l'expliquer.
    let Some(depot) = depot else {
        return Err(
            "Aucun magasin actif : impossible de constater ce mouvement.".to_string(),
        );
    };

    // Le stock se compte en unite de BASE, la ligne en unite de vente :
    // un carton de douze sort douze.
    let facteur: f64 = conn
        .query_row(
            "SELECT facteur FROM unite_vente WHERE id = ?1",
            rusqlite::params![unite_id],
            |r| r.get(0),
        )
        .unwrap_or(1.0);

    conn.execute(
        "INSERT INTO mouvement_stock
         (id, article_id, depot_id, type_mouvement, quantite_delta,
          motif, operation_id, auteur_id, date_mouvement,
          cree_le, cree_par, origine)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?9,?8,'app')",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(),
            article_id,
            depot,
            type_mouvement,
            signe * ecart * facteur,
            crate::coeur::stock::libelle(type_mouvement),
            piece_id,
            auteur,
            now
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Marque une piece entierement livree, mouvements de stock compris.
///
/// Emettre un bon, c'est constater que la marchandise bouge : le bon
/// nait donc entierement livre. Cette fonction existe pour que ce
/// raccourci passe par le MEME chemin que la saisie ligne a ligne —
/// sinon un bon cree par conversion se disait livre sans que rien ne
/// sorte du magasin.
pub fn marquer_entierement_livre(
    conn: &rusqlite::Connection,
    piece_id: &str,
) -> Result<(), String> {
    let (type_piece, depot_piece): (String, Option<String>) = conn
        .query_row(
            "SELECT type_piece, depot_id FROM piece_commerciale WHERE id = ?1",
            rusqlite::params![piece_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|_| "Piece introuvable".to_string())?;

    let now = maintenant_iso();
    let auteur = crate::argent::id_utilisateur_courant_pub(conn);
    let depot = crate::pieces::depot_de_piece(conn, depot_piece);

    let lignes: Vec<(String, f64, f64, String, String)> = {
        let mut st = conn
            .prepare(
                "SELECT id, quantite, COALESCE(quantite_livree, 0),
                        article_id, unite_vente_id
                 FROM ligne_piece WHERE piece_id = ?1",
            )
            .map_err(|e| e.to_string())?;
        let v = st
            .query_map(rusqlite::params![piece_id], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();
        v
    };

    for (ligne_id, quantite, deja, article_id, unite_id) in lignes {
        conn.execute(
            "UPDATE ligne_piece SET quantite_livree = quantite WHERE id = ?1",
            rusqlite::params![ligne_id],
        )
        .map_err(|e| e.to_string())?;

        mouvementer_ecart(
            conn,
            &type_piece,
            depot.as_deref(),
            &article_id,
            &unite_id,
            quantite - deja,
            &auteur,
            &now,
            piece_id,
        )?;
    }
    Ok(())
}

// =====================================================================
//  LECTURE
// =====================================================================

/// Lignes d'une piece avec ce qui reste a livrer.
pub fn lire_livraison_piece(
    conn: &rusqlite::Connection,
    piece_id: String,
) -> Result<serde_json::Value, String> {

    let (numero, type_piece): (String, String) = conn.query_row(
        "SELECT numero, type_piece FROM piece_commerciale WHERE id = ?1",
        rusqlite::params![piece_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    ).map_err(|_| "Piece introuvable".to_string())?;

    let mut st = conn.prepare(
        "SELECT lp.id, a.nom, COALESCE(u.libelle, a.unite_base),
                lp.quantite, COALESCE(lp.quantite_livree, 0)
         FROM ligne_piece lp
         JOIN article a ON a.id = lp.article_id
         LEFT JOIN unite_vente u ON u.id = lp.unite_vente_id
         WHERE lp.piece_id = ?1
         ORDER BY a.nom"
    ).map_err(|e| e.to_string())?;

    let lignes: Vec<serde_json::Value> = st.query_map(
        rusqlite::params![piece_id], |r| {
            let qte: f64 = r.get(3)?;
            let livree: f64 = r.get(4)?;
            Ok(serde_json::json!({
                "id":              r.get::<_, String>(0)?,
                "article_nom":     r.get::<_, String>(1)?,
                "unite":           r.get::<_, String>(2)?,
                "quantite":        qte,
                "quantite_livree": livree,
                "reste":           (qte - livree).max(0.0),
            }))
        }
    ).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    let commandee: f64 = lignes.iter()
        .filter_map(|l| l["quantite"].as_f64()).sum();
    let livree: f64 = lignes.iter()
        .filter_map(|l| l["quantite_livree"].as_f64()).sum();

    Ok(serde_json::json!({
        "piece_id":   piece_id,
        "numero":     numero,
        "type_piece": type_piece,
        "lignes":     lignes,
        "etat":       etat(livree, commandee),
    }))
}

// =====================================================================
//  ECRITURE
// =====================================================================

#[derive(Deserialize)]
pub struct LigneLivraison {
    pub ligne_id: String,
    /// Quantite TOTALE livree a ce jour, pas l'increment.
    ///
    /// Un increment obligerait l'ecran a connaitre l'etat courant pour
    /// calculer la difference, et deux enregistrements rapproches
    /// doubleraient la quantite. Ici l'ecran envoie ce qu'il affiche.
    pub quantite_livree: f64,
}

/// Enregistre l'etat de livraison d'une piece.
///
/// Aucun mouvement de stock, aucun mouvement de caisse, aucune session
/// de caisse exigee : livrer n'est pas encaisser, et refuser une
/// livraison parce que le tiroir est ferme n'aurait aucun sens.
pub fn enregistrer_livraison(
    conn: &mut rusqlite::Connection,
    piece_id: String,
    lignes: Vec<LigneLivraison>,
) -> Result<serde_json::Value, String> {
    let now = maintenant_iso();
    let auteur = crate::argent::id_utilisateur_courant_pub(&conn);

    let (type_piece, depot_piece): (String, Option<String>) = conn
        .query_row(
            "SELECT type_piece, depot_id FROM piece_commerciale WHERE id = ?1",
            rusqlite::params![piece_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|_| "Piece introuvable".to_string())?;
    let depot = crate::pieces::depot_de_piece(&conn, depot_piece);

    let tx = conn.transaction().map_err(|e| e.to_string())?;

    for l in &lignes {
        // Quantite commandee de CETTE ligne, pour plafonner. Livrer plus
        // que commande n'est pas une livraison, c'est une saisie fausse.
        //
        // On lit aussi ce qui etait DEJA livre : le stock doit bouger de
        // l'ecart, pas du total. Sans cela, corriger « 6 livres » en
        // « 7 livres » sortirait sept unites de plus au lieu d'une.
        let (commandee, deja, article_id, unite_id): (f64, f64, String, String) =
            tx.query_row(
                "SELECT quantite, COALESCE(quantite_livree, 0),
                        article_id, unite_vente_id
                 FROM ligne_piece WHERE id = ?1 AND piece_id = ?2",
                rusqlite::params![l.ligne_id, piece_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .map_err(|_| format!("Ligne {} introuvable sur cette piece", l.ligne_id))?;

        let valeur = l.quantite_livree.clamp(0.0, commandee);

        tx.execute(
            "UPDATE ligne_piece SET quantite_livree = ?1 WHERE id = ?2",
            rusqlite::params![valeur, l.ligne_id],
        ).map_err(|e| e.to_string())?;

        // Le mouvement de stock, s'il y a lieu.
        mouvementer_ecart(
            &tx, &type_piece, depot.as_deref(), &article_id, &unite_id,
            valeur - deja, &auteur, &now, &piece_id,
        )?;
    }

    // Etat recalcule DANS la transaction : c'est lui qu'on journalise.
    let (livree, commandee): (f64, f64) = tx.query_row(
        "SELECT COALESCE(SUM(quantite_livree), 0), COALESCE(SUM(quantite), 0)
         FROM ligne_piece WHERE piece_id = ?1",
        rusqlite::params![piece_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    ).unwrap_or((0.0, 0.0));

    let e = etat(livree, commandee);

    // Le journal est append-only (invariant 11) : on trace l'etat
    // atteint, pas la ligne modifiee. C'est ce qui permet de repondre
    // « quand est-ce parti ? » des semaines plus tard.
    tx.execute(
        "INSERT INTO journal
         (id, type_evenement, entite_type, entite_id, auteur_id,
          nouveau_valeur, origine, date_evenement)
         VALUES (?1,'livraison_enregistree','piece_commerciale',?2,?3,?4,'app',?5)",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(), piece_id, auteur,
            format!(r#"{{"etat":"{}","livree":{},"commandee":{}}}"#,
                e, livree, commandee),
            now
        ],
    ).ok();

    tx.commit().map_err(|e| e.to_string())?;

    Ok(serde_json::json!({
        "etat":      e,
        "livree":    livree,
        "commandee": commandee,
    }))
}

// =====================================================================
//  TESTS
// =====================================================================

#[cfg(test)]
mod tests {
    use super::etat;

    #[test]
    fn rien_livre() {
        assert_eq!(etat(0.0, 10.0), "non_livre");
    }

    #[test]
    fn partiellement_livre() {
        assert_eq!(etat(3.0, 10.0), "partiel");
    }

    #[test]
    fn entierement_livre() {
        assert_eq!(etat(10.0, 10.0), "livre");
    }

    #[test]
    fn arrondi_flottant_compte_comme_livre() {
        // 0.1 * 3 != 0.3 en binaire. Sans EPSILON, cette commande
        // resterait « partiel » pour toujours.
        assert_eq!(etat(0.1 + 0.1 + 0.1, 0.3), "livre");
    }

    #[test]
    fn piece_sans_ligne() {
        assert_eq!(etat(0.0, 0.0), "sans_objet");
    }

    #[test]
    fn surlivraison_compte_comme_livre() {
        // `enregistrer_livraison` plafonne, mais une donnee ancienne ou
        // importee peut depasser : elle ne doit pas retomber en partiel.
        assert_eq!(etat(12.0, 10.0), "livre");
    }
}

// =====================================================================
//  LE MOUVEMENT, SUR L'UN OU L'AUTRE MOTEUR
// =====================================================================
//
// Le strict necessaire pour que la conversion commande -> bon de
// livraison, portee dans `pieces`, sorte le stock par le MEME chemin
// que sur SQLite. Le reste du module (saisie ligne a ligne, lecture)
// suit dans son propre lot.

use crate::base::{Acces, Base};
use crate::parametres;

#[allow(clippy::too_many_arguments)]
fn mouvementer_ecart_sur(
    acces: &mut impl Acces,
    type_piece: &str,
    depot: Option<&str>,
    article_id: &str,
    unite_id: &str,
    ecart: f64,
    auteur: &str,
    now: &str,
    piece_id: &str,
) -> Result<(), String> {
    let Some((signe, type_mouvement)) = sens(type_piece) else {
        return Ok(());
    };
    if ecart.abs() <= EPSILON {
        return Ok(());
    }
    let Some(depot) = depot else {
        return Err(
            "Aucun magasin actif : impossible de constater ce mouvement.".to_string(),
        );
    };
    let dossier = acces.dossier().to_string();

    // `unite_vente` n'est pas cloisonnee.
    let facteur: f64 = acces
        .lire_une(
            "SELECT facteur FROM unite_vente WHERE id = ?1",
            &parametres![unite_id],
            |r| r.get::<f64>(0),
        )
        .ok()
        .flatten()
        .unwrap_or(1.0);

    acces
        .executer(
            "INSERT INTO mouvement_stock
             (id, article_id, depot_id, type_mouvement, quantite_delta,
              motif, operation_id, auteur_id, date_mouvement,
              cree_le, cree_par, origine, dossier_id)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?9,?8,'app',?10)",
            &parametres![
                uuid::Uuid::new_v4().to_string(),
                article_id,
                depot,
                type_mouvement,
                signe * ecart * facteur,
                crate::coeur::stock::libelle(type_mouvement),
                piece_id,
                auteur,
                now,
                dossier
            ],
        )
        .map_err(|e| e.0)?;
    Ok(())
}

/// Meme regle que `marquer_entierement_livre`, sur l'un ou l'autre
/// moteur, dedans ou hors transaction.
pub fn marquer_entierement_livre_sur(
    acces: &mut impl Acces,
    piece_id: &str,
) -> Result<(), String> {
    let dossier = acces.dossier().to_string();
    let (type_piece, depot_piece): (String, Option<String>) = acces
        .lire_une(
            "SELECT type_piece, depot_id FROM piece_commerciale
             WHERE id = ?1 AND dossier_id = ?2",
            &parametres![piece_id, dossier.clone()],
            |r| Ok((r.get::<String>(0)?, r.get::<Option<String>>(1)?)),
        )
        .map_err(|e| e.0)?
        .ok_or_else(|| "Piece introuvable".to_string())?;

    let now = maintenant_iso();
    let auteur = crate::argent::id_utilisateur_courant_sur(acces);
    let depot = crate::pieces::depot_de_piece_sur(acces, depot_piece);

    let lignes: Vec<(String, f64, f64, String, String)> = acces
        .lire_plusieurs(
            "SELECT id, quantite, COALESCE(quantite_livree, 0),
                    article_id, unite_vente_id
             FROM ligne_piece WHERE piece_id = ?1 AND dossier_id = ?2",
            &parametres![piece_id, dossier.clone()],
            |r| {
                Ok((
                    r.get::<String>(0)?,
                    r.get::<f64>(1)?,
                    r.get::<f64>(2)?,
                    r.get::<String>(3)?,
                    r.get::<String>(4)?,
                ))
            },
        )
        .map_err(|e| e.0)?;

    for (ligne_id, quantite, deja, article_id, unite_id) in lignes {
        acces
            .executer(
                "UPDATE ligne_piece SET quantite_livree = quantite
                 WHERE id = ?1 AND dossier_id = ?2",
                &parametres![ligne_id, dossier.clone()],
            )
            .map_err(|e| e.0)?;

        mouvementer_ecart_sur(
            acces,
            &type_piece,
            depot.as_deref(),
            &article_id,
            &unite_id,
            quantite - deja,
            &auteur,
            &now,
            piece_id,
        )?;
    }
    Ok(())
}

// ---------------------------------------------------------------------
//  Lecture et saisie ligne a ligne
// ---------------------------------------------------------------------

pub fn lire_livraison_piece_sur_base(base: &mut Base, piece_id: String) -> Result<serde_json::Value, String> {
    let dossier = base.dossier().to_string();
    let (numero, type_piece): (String, String) = base
        .lire_une(
            "SELECT numero, type_piece FROM piece_commerciale WHERE id = ?1 AND dossier_id = ?2",
            &parametres![piece_id.clone(), dossier.clone()],
            |r| Ok((r.get::<String>(0)?, r.get::<String>(1)?)),
        )
        .map_err(|e| e.0)?
        .ok_or_else(|| "Piece introuvable".to_string())?;

    let lignes: Vec<serde_json::Value> = base
        .lire_plusieurs(
            "SELECT lp.id, a.nom, COALESCE(u.libelle, a.unite_base),
                    lp.quantite, COALESCE(lp.quantite_livree, 0)
             FROM ligne_piece lp
             JOIN article a ON a.id = lp.article_id
             LEFT JOIN unite_vente u ON u.id = lp.unite_vente_id
             WHERE lp.piece_id = ?1 AND lp.dossier_id = ?2
             ORDER BY a.nom",
            &parametres![piece_id.clone(), dossier],
            |r| {
                let qte: f64 = r.get::<f64>(3)?;
                let livree: f64 = r.get::<f64>(4)?;
                Ok(serde_json::json!({
                    "id":              r.get::<String>(0)?,
                    "article_nom":     r.get::<String>(1)?,
                    "unite":           r.get::<String>(2)?,
                    "quantite":        qte,
                    "quantite_livree": livree,
                    "reste":           (qte - livree).max(0.0),
                }))
            },
        )
        .map_err(|e| e.0)?;
    let commandee: f64 = lignes.iter().filter_map(|l| l["quantite"].as_f64()).sum();
    let livree: f64 = lignes.iter().filter_map(|l| l["quantite_livree"].as_f64()).sum();
    Ok(serde_json::json!({
        "piece_id":   piece_id,
        "numero":     numero,
        "type_piece": type_piece,
        "lignes":     lignes,
        "etat":       etat(livree, commandee),
    }))
}

/// Enregistre l'etat de livraison d'une piece : le stock bouge de
/// l'ECART, pas du total. Aucune caisse exigee — livrer n'est pas
/// encaisser.
pub fn enregistrer_livraison_sur_base(
    base: &mut Base,
    piece_id: String,
    lignes: Vec<LigneLivraison>,
) -> Result<serde_json::Value, String> {
    let dossier = base.dossier().to_string();
    let now = maintenant_iso();
    let auteur = crate::argent::id_utilisateur_courant_sur(base);
    let (type_piece, depot_piece): (String, Option<String>) = base
        .lire_une(
            "SELECT type_piece, depot_id FROM piece_commerciale WHERE id = ?1 AND dossier_id = ?2",
            &parametres![piece_id.clone(), dossier.clone()],
            |r| Ok((r.get::<String>(0)?, r.get::<Option<String>>(1)?)),
        )
        .map_err(|e| e.0)?
        .ok_or_else(|| "Piece introuvable".to_string())?;
    let depot = crate::pieces::depot_de_piece_sur(base, depot_piece);

    let mut tx = base.transaction().map_err(|e| e.0)?;
    for l in &lignes {
        let (commandee, deja, article_id, unite_id): (f64, f64, String, String) = tx
            .lire_une(
                "SELECT quantite, COALESCE(quantite_livree, 0), article_id, unite_vente_id
                 FROM ligne_piece WHERE id = ?1 AND piece_id = ?2 AND dossier_id = ?3",
                &parametres![l.ligne_id.clone(), piece_id.clone(), dossier.clone()],
                |r| Ok((r.get::<f64>(0)?, r.get::<f64>(1)?, r.get::<String>(2)?, r.get::<String>(3)?)),
            )
            .map_err(|e| e.0)?
            .ok_or_else(|| format!("Ligne {} introuvable sur cette piece", l.ligne_id))?;
        // Livrer plus que commande n'est pas une livraison.
        let valeur = l.quantite_livree.clamp(0.0, commandee);
        tx.executer(
            "UPDATE ligne_piece SET quantite_livree = ?1 WHERE id = ?2 AND dossier_id = ?3",
            &parametres![valeur, l.ligne_id.clone(), dossier.clone()],
        )
        .map_err(|e| e.0)?;
        mouvementer_ecart_sur(&mut tx, &type_piece, depot.as_deref(), &article_id, &unite_id, valeur - deja, &auteur, &now, &piece_id)?;
    }

    let (livree, commandee): (f64, f64) = tx
        .lire_une(
            "SELECT CAST(COALESCE(SUM(quantite_livree), 0) AS DOUBLE PRECISION),
                    CAST(COALESCE(SUM(quantite), 0) AS DOUBLE PRECISION)
             FROM ligne_piece WHERE piece_id = ?1 AND dossier_id = ?2",
            &parametres![piece_id.clone(), dossier.clone()],
            |r| Ok((r.get::<f64>(0)?, r.get::<f64>(1)?)),
        )
        .map_err(|e| e.0)?
        .unwrap_or((0.0, 0.0));
    let e = etat(livree, commandee);
    let _ = tx.executer(
        "INSERT INTO journal
         (id, type_evenement, entite_type, entite_id, auteur_id,
          nouveau_valeur, origine, date_evenement, dossier_id)
         VALUES (?1,'livraison_enregistree','piece_commerciale',?2,?3,?4,'app',?5,?6)",
        &parametres![
            uuid::Uuid::new_v4().to_string(), piece_id, auteur,
            format!(r#"{{"etat":"{}","livree":{},"commandee":{}}}"#, e, livree, commandee),
            now, dossier
        ],
    );
    tx.valider().map_err(|e| e.0)?;
    Ok(serde_json::json!({ "etat": e, "livree": livree, "commandee": commandee }))
}
