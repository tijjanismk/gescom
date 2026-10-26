//! Les clients de passage (D35, 25/10/2026).
//!
//! Une vente sans nom n'est plus versee au seul client « Comptant » :
//! le serveur cree, dans la transaction de la vente, une fiche « Client
//! de passage » a elle — code `PAS-000123`, `est_generique = 1`,
//! `statut = 'passage'`. Le « Comptant » reste la porte de l'ecran (rien
//! a saisir au comptoir) ; c'est ici qu'on le remplace, vente par vente.
//!
//! Pourquoi `est_generique = 1` : toutes les lectures qui ecartent le
//! « Comptant » (liste des clients, relances, creances, rapports) le
//! font par ce drapeau ; la fiche passage en sort d'office, sans qu'on
//! ait a retrouver chacune. Et toutes les gardes du « Comptant » (ni
//! credit, ni avoir, retour en especes ou en echange) s'y appliquent.
//!
//! Le passant qui revient et donne son nom : `promouvoir_sur` fait de sa
//! fiche un client ordinaire, avec ses achats. Avant, le nom ecrit sur
//! ses factures est fige (`figer_noms_sur`) : une facture emise ne
//! change pas de nom a la reimpression (OHADA).

use crate::base::{Acces, Base};
use crate::parametres;

/// Le statut d'une fiche de passage (`client.statut`).
pub const STATUT: &str = "passage";
/// Le nom d'une fiche de passage, jusqu'a sa promotion.
pub const NOM: &str = "Client de passage";

// =====================================================================
//  Sur `Base` (le serveur, les deux moteurs)
// =====================================================================

/// Ce client est-il LE « Comptant » de ce dossier (pas une fiche de
/// passage) ? C'est lui, et lui seul, que l'on remplace.
pub fn est_le_comptant_sur(acces: &mut impl Acces, client_id: &str) -> bool {
    let dossier = acces.dossier().to_string();
    // COUNT et non `SELECT 1` : PostgreSQL type un litteral en int4.
    acces
        .lire_une(
            "SELECT COUNT(*) FROM client
             WHERE id = ?1 AND dossier_id = ?2 AND est_generique = 1 AND statut <> ?3",
            &parametres![client_id.to_string(), dossier, STATUT.to_string()],
            |r| r.get::<i64>(0),
        )
        .ok()
        .flatten()
        .unwrap_or(0)
        > 0
}

/// Cree la fiche d'une vente sans nom ; rend `(id, code)`.
///
/// A appeler DANS la transaction de la vente : le numero est reserve au
/// compteur (`compteur_piece`, cle `PAS`) et le retour arriere le rend,
/// sans trou ni doublon a deux caisses.
pub fn creer_fiche_sur(acces: &mut impl Acces, maintenant: &str, auteur: &str) -> Result<(String, String), String> {
    let rang = crate::argent::suivant_sur(acces, "PAS")?;
    // `client.code` est unique sur TOUTE la base : prefixe du dossier,
    // comme les autres codes (`dossiers::prefixe_de_code_sur`).
    let prefixe = crate::dossiers::prefixe_de_code_sur(acces)?;
    let code = format!("{prefixe}PAS-{rang:06}");
    let id = uuid::Uuid::new_v4().to_string();
    let dossier = acces.dossier().to_string();
    acces
        .executer(
            "INSERT INTO client
               (id, code, nom, est_generique, actif, cree_le, modifie_le,
                cree_par, modifie_par, origine, statut, dossier_id)
             VALUES (?1, ?2, ?3, 1, 1, ?4, ?4, ?5, ?5, 'app', ?6, ?7)",
            &parametres![
                id.clone(),
                code.clone(),
                NOM.to_string(),
                maintenant.to_string(),
                auteur.to_string(),
                STATUT.to_string(),
                dossier
            ],
        )
        .map_err(|e| e.0)?;
    Ok((id, code))
}

/// Fige le nom en cours de ce client sur ses pieces emises qui n'en
/// portent pas encore. A appeler AVANT de changer son nom : figer au
/// renommage revient a figer a l'emission, puisque jusque-la les deux
/// noms sont les memes. Les brouillons suivent la fiche (ils ne sont
/// pas emis).
pub fn figer_noms_sur(acces: &mut impl Acces, client_id: &str) -> Result<(), String> {
    let dossier = acces.dossier().to_string();
    acces
        .executer(
            "UPDATE piece_commerciale
             SET tiers_nom = (SELECT c.nom FROM client c WHERE c.id = ?1 AND c.dossier_id = ?2)
             WHERE tiers_type = 'client' AND tiers_id = ?1 AND dossier_id = ?2
               AND tiers_nom IS NULL AND statut <> 'brouillon'",
            &parametres![client_id.to_string(), dossier],
        )
        .map(|_| ())
        .map_err(|e| e.0)
}

/// Les fiches de passage du dossier, les plus recentes d'abord.
/// `recherche` : un morceau du code (`PAS-000123`) ou du numero d'une
/// de ses factures — le passant revient avec son ticket.
pub fn lire_passages_sur(base: &mut Base, recherche: Option<String>) -> Result<Vec<serde_json::Value>, String> {
    let dossier = base.dossier().to_string();
    let motif = recherche
        .map(|r| r.trim().to_string())
        .filter(|r| !r.is_empty())
        .map(|r| format!("%{r}%"));
    base.lire_plusieurs(
        "SELECT c.id, c.code, c.nom, c.cree_le,
                (SELECT COUNT(*) FROM vente v WHERE v.client_id = c.id AND v.dossier_id = ?1),
                CAST(COALESCE((SELECT SUM(pa.montant) FROM paiement pa
                               JOIN vente v ON v.id = pa.vente_id AND v.dossier_id = ?1
                               WHERE v.client_id = c.id AND pa.dossier_id = ?1), 0) AS BIGINT),
                (SELECT MAX(p.numero) FROM piece_commerciale p
                  WHERE p.tiers_type = 'client' AND p.tiers_id = c.id AND p.dossier_id = ?1)
         FROM client c
         WHERE c.dossier_id = ?1 AND c.statut = ?3
           AND (CAST(?2 AS TEXT) IS NULL
                OR LOWER(c.code) LIKE LOWER(?2)
                OR EXISTS (SELECT 1 FROM piece_commerciale p
                           WHERE p.tiers_type = 'client' AND p.tiers_id = c.id
                             AND p.dossier_id = ?1 AND LOWER(p.numero) LIKE LOWER(?2)))
         ORDER BY c.cree_le DESC, c.id
         LIMIT 100",
        &parametres![dossier, motif, STATUT.to_string()],
        |r| {
            Ok(serde_json::json!({
                "id":        r.get::<String>(0)?,
                "code":      r.get::<String>(1)?,
                "nom":       r.get::<String>(2)?,
                "cree_le":   r.get::<String>(3)?,
                "nb_ventes": r.get::<i64>(4)?,
                "paye":      r.get::<i64>(5)?,
                "numero":    r.get::<Option<String>>(6)?,
            }))
        },
    )
    .map_err(|e| e.0)
}

/// « Passer en client ordinaire » : le passant a donne son nom. La fiche
/// garde son code et ses achats ; elle quitte `est_generique` (donc les
/// gardes du « Comptant ») et devient un client comme un autre. Le nom
/// « Client de passage » reste ecrit sur ses factures deja emises.
pub fn promouvoir_sur(
    base: &mut Base,
    client_id: String,
    nom: String,
    telephone: Option<String>,
) -> Result<serde_json::Value, String> {
    let nom = nom.trim().to_string();
    if nom.is_empty() {
        return Err("Le nom est obligatoire pour en faire un client ordinaire.".to_string());
    }
    let telephone = telephone.map(|t| t.trim().to_string()).filter(|t| !t.is_empty());
    let dossier = base.dossier().to_string();
    let code: String = base
        .lire_une(
            "SELECT code FROM client WHERE id = ?1 AND dossier_id = ?2 AND statut = ?3",
            &parametres![client_id.clone(), dossier.clone(), STATUT.to_string()],
            |r| r.get::<String>(0),
        )
        .map_err(|e| e.0)?
        .ok_or_else(|| "Seule une fiche de client de passage se passe en client ordinaire.".to_string())?;

    let maintenant = crate::utils::maintenant_iso();
    let auteur = crate::argent::id_utilisateur_courant_sur(base);
    let mut tx = base.transaction().map_err(|e| e.0)?;
    figer_noms_sur(&mut tx, &client_id)?;
    tx.executer(
        "UPDATE client
         SET nom = ?1, telephone = ?2, est_generique = 0, statut = 'client',
             modifie_le = ?3, modifie_par = ?4
         WHERE id = ?5 AND dossier_id = ?6",
        &parametres![nom.clone(), telephone.clone(), maintenant.clone(), auteur.clone(), client_id.clone(), dossier.clone()],
    )
    .map_err(|e| e.0)?;
    tx.executer(
        "INSERT INTO journal
           (id, type_evenement, entite_type, entite_id, auteur_id,
            ancien_valeur, nouveau_valeur, origine, date_evenement, dossier_id)
         VALUES (?1,'passage_promu','client',?2,?3,?4,?5,'app',?6,?7)",
        &parametres![
            uuid::Uuid::new_v4().to_string(),
            client_id.clone(),
            auteur,
            serde_json::json!({ "nom": NOM, "code": code }).to_string(),
            serde_json::json!({ "nom": nom, "telephone": telephone, "code": code }).to_string(),
            maintenant,
            dossier
        ],
    )
    .map_err(|e| e.0)?;
    tx.valider().map_err(|e| e.0)?;
    Ok(serde_json::json!({ "id": client_id, "code": code, "nom": nom }))
}

// =====================================================================
//  Sur `rusqlite` (les facades `Connection`, un seul dossier)
// =====================================================================

/// Voir `est_le_comptant_sur`.
pub fn est_le_comptant(conn: &rusqlite::Connection, client_id: &str) -> bool {
    conn.query_row(
        "SELECT 1 FROM client WHERE id = ?1 AND est_generique = 1 AND statut <> ?2",
        rusqlite::params![client_id, STATUT],
        |r| r.get::<_, i64>(0),
    )
    .is_ok()
}

/// Voir `creer_fiche_sur` — dans la transaction de la vente.
pub fn creer_fiche(conn: &rusqlite::Connection, maintenant: &str, auteur: &str) -> Result<(String, String), String> {
    let rang = crate::argent::suivant(conn, "PAS")?;
    let code = format!("PAS-{rang:06}");
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO client
           (id, code, nom, est_generique, actif, cree_le, modifie_le,
            cree_par, modifie_par, origine, statut)
         VALUES (?1, ?2, ?3, 1, 1, ?4, ?4, ?5, ?5, 'app', ?6)",
        rusqlite::params![id, code, NOM, maintenant, auteur, STATUT],
    )
    .map_err(|e| e.to_string())?;
    Ok((id, code))
}

/// Voir `figer_noms_sur`.
pub fn figer_noms(conn: &rusqlite::Connection, client_id: &str) -> Result<(), String> {
    conn.execute(
        "UPDATE piece_commerciale
         SET tiers_nom = (SELECT c.nom FROM client c WHERE c.id = ?1)
         WHERE tiers_type = 'client' AND tiers_id = ?1
           AND tiers_nom IS NULL AND statut <> 'brouillon'",
        rusqlite::params![client_id],
    )
    .map(|_| ())
    .map_err(|e| e.to_string())
}
