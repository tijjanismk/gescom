//! Le suivi client en base (PLAN-EQUIPE, H-1/H-2 — D34). Version
//! `Base` seule (D29) : Gescom Equipe ne parle qu'au serveur.
//!
//! Un prospect EST un client (`client.statut`), pas une seconde fiche :
//! il redevient « client » tout seul a sa premiere vente
//! (`argent::creer_vente_datee_sur[_base]`). Les relances de creance
//! existantes sont un echange comme un autre — `lister_echanges_sur`
//! les fond dans le meme fil, plutot que d'en faire une liste de plus.

use crate::base::Base;
use crate::coeur::crm as regles;
use crate::parametres;

fn texte(v: &Option<String>) -> Option<String> {
    v.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string)
}

fn jour(v: &str, quoi: &str) -> Result<String, String> {
    let d = v.trim();
    if d.is_empty() {
        return Err(format!("{quoi} est vide."));
    }
    crate::coeur::dates::jour(d)
        .map(|j| j.format("%Y-%m-%d").to_string())
        .ok_or_else(|| format!("{quoi} illisible : « {d} »"))
}

/// Le nom du client, s'il est bien dans ce dossier.
fn client_du_dossier(base: &mut Base, client_id: &str) -> Result<String, String> {
    let dossier = base.dossier().to_string();
    base.lire_une(
        "SELECT nom FROM client WHERE id = ?1 AND dossier_id = ?2",
        &parametres![client_id, dossier],
        |r| r.get::<String>(0),
    )
    .map_err(|e| e.0)?
    .ok_or_else(|| "Client introuvable.".to_string())
}

fn noter(
    acces: &mut impl crate::base::Acces,
    type_evenement: &str,
    entite_type: &str,
    id: &str,
    nouveau: serde_json::Value,
) -> Result<(), String> {
    let auteur = crate::argent::id_utilisateur_courant_sur(acces);
    let dossier = acces.dossier().to_string();
    acces
        .executer(
            "INSERT INTO journal
               (id, type_evenement, entite_type, entite_id, auteur_id, nouveau_valeur, origine, date_evenement, dossier_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'app', ?7, ?8)",
            &parametres![
                uuid::Uuid::new_v4().to_string(),
                type_evenement,
                entite_type,
                id,
                auteur,
                nouveau.to_string(),
                crate::utils::maintenant_iso(),
                dossier
            ],
        )
        .map(|_| ())
        .map_err(|e| e.0)
}

// =====================================================================
//  Echanges (H-1)
// =====================================================================

pub fn creer_echange_sur(
    base: &mut Base,
    client_id: String,
    genre: String,
    quoi: String,
    suite_prevue: Option<String>,
) -> Result<serde_json::Value, String> {
    let nom = client_du_dossier(base, &client_id)?;
    let (genre, quoi) = regles::valider_echange(&genre, &quoi)?;
    let suite_prevue = texte(&suite_prevue);
    let auteur = crate::argent::id_utilisateur_courant_sur(base);
    let dossier = base.dossier().to_string();
    let now = crate::utils::maintenant_iso();
    let id = uuid::Uuid::new_v4().to_string();
    let mut tx = base.transaction().map_err(|e| e.0)?;
    tx.executer(
        "INSERT INTO echange
           (id, dossier_id, client_id, genre, quoi, suite_prevue, auteur_id, date_echange, cree_le)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?8)",
        &parametres![
            id.clone(), dossier, client_id.clone(), genre.clone(), quoi.clone(),
            suite_prevue.clone(), auteur.clone(), now.clone()
        ],
    )
    .map_err(|e| e.0)?;
    noter(&mut tx, "echange_cree", "echange", &id, serde_json::json!({ "client": nom, "genre": genre }))?;
    tx.valider().map_err(|e| e.0)?;
    Ok(serde_json::json!({
        "id": id, "client_id": client_id, "genre": genre, "quoi": quoi,
        "suite_prevue": suite_prevue, "date": now, "source": "echange",
    }))
}

/// Le fil d'un client : ses echanges ET ses relances de creance
/// (D34 : « des echanges comme les autres, pas une liste de plus »),
/// les plus recents d'abord.
pub fn lister_echanges_sur(base: &mut Base, client_id: String) -> Result<Vec<serde_json::Value>, String> {
    let dossier = base.dossier().to_string();
    let mut fil: Vec<serde_json::Value> = base
        .lire_plusieurs(
            "SELECT e.id, e.genre, e.quoi, e.suite_prevue, e.date_echange, u.nom
             FROM echange e LEFT JOIN utilisateur u ON u.id = e.auteur_id
             WHERE e.client_id = ?1 AND e.dossier_id = ?2",
            &parametres![client_id.clone(), dossier.clone()],
            |r| {
                Ok(serde_json::json!({
                    "id": r.get::<String>(0)?,
                    "genre": r.get::<String>(1)?,
                    "quoi": r.get::<String>(2)?,
                    "suite_prevue": r.get::<Option<String>>(3)?,
                    "date": r.get::<String>(4)?,
                    "auteur_nom": r.get::<Option<String>>(5)?,
                    "source": "echange",
                }))
            },
        )
        .map_err(|e| e.0)?;

    let relances: Vec<serde_json::Value> = base
        .lire_plusieurs(
            "SELECT rc.id, rc.canal, rc.note, rc.date_relance, u.nom
             FROM relance_creance rc
             JOIN vente v ON v.id = rc.vente_id
             LEFT JOIN utilisateur u ON u.id = rc.auteur_id
             WHERE v.client_id = ?1 AND rc.dossier_id = ?2",
            &parametres![client_id, dossier],
            |r| {
                Ok(serde_json::json!({
                    "id": r.get::<String>(0)?,
                    "genre": "relance",
                    "quoi": r.get::<Option<String>>(2)?.unwrap_or_else(|| "Relance de créance".to_string()),
                    "suite_prevue": serde_json::Value::Null,
                    "date": r.get::<String>(3)?,
                    "auteur_nom": r.get::<Option<String>>(4)?,
                    "source": "relance_creance",
                    "canal": r.get::<String>(1)?,
                }))
            },
        )
        .map_err(|e| e.0)?;

    fil.extend(relances);
    fil.sort_by(|a, b| b["date"].as_str().unwrap_or("").cmp(a["date"].as_str().unwrap_or("")));
    Ok(fil)
}

// =====================================================================
//  Rappels (H-2)
// =====================================================================

pub fn creer_rappel_sur(
    base: &mut Base,
    client_id: String,
    pour_utilisateur_id: String,
    quand: String,
    quoi: String,
) -> Result<serde_json::Value, String> {
    let nom_client = client_du_dossier(base, &client_id)?;
    let quoi = regles::valider_rappel(&quoi)?;
    let quand = jour(&quand, "La date du rappel")?;
    let pour_nom: String = base
        .lire_une(
            "SELECT nom FROM utilisateur WHERE id = ?1 AND actif = 1",
            &parametres![pour_utilisateur_id.clone()],
            |r| r.get::<String>(0),
        )
        .map_err(|e| e.0)?
        .ok_or_else(|| "Ce compte n'existe pas, ou n'est plus actif.".to_string())?;
    let dossier = base.dossier().to_string();
    let auteur = crate::argent::id_utilisateur_courant_sur(base);
    let now = crate::utils::maintenant_iso();
    let id = uuid::Uuid::new_v4().to_string();
    let mut tx = base.transaction().map_err(|e| e.0)?;
    tx.executer(
        "INSERT INTO rappel
           (id, dossier_id, client_id, pour_utilisateur_id, quand, quoi, fait, cree_par, cree_le)
         VALUES (?1,?2,?3,?4,?5,?6,0,?7,?8)",
        &parametres![
            id.clone(), dossier, client_id.clone(), pour_utilisateur_id.clone(),
            quand.clone(), quoi.clone(), auteur.clone(), now.clone()
        ],
    )
    .map_err(|e| e.0)?;
    noter(
        &mut tx, "rappel_cree", "rappel", &id,
        serde_json::json!({ "client": nom_client, "pour": pour_nom, "quand": quand }),
    )?;
    tx.valider().map_err(|e| e.0)?;
    Ok(serde_json::json!({
        "id": id, "client_id": client_id, "client_nom": nom_client,
        "pour_utilisateur_id": pour_utilisateur_id, "pour_nom": pour_nom,
        "quand": quand, "quoi": quoi, "fait": false,
    }))
}

pub fn marquer_rappel_fait_sur(base: &mut Base, rappel_id: String) -> Result<serde_json::Value, String> {
    let dossier = base.dossier().to_string();
    let fait: i64 = base
        .lire_une(
            "SELECT fait FROM rappel WHERE id = ?1 AND dossier_id = ?2",
            &parametres![rappel_id.clone(), dossier.clone()],
            |r| r.get::<i64>(0),
        )
        .map_err(|e| e.0)?
        .ok_or_else(|| "Rappel introuvable.".to_string())?;
    if fait != 0 {
        return Err("Ce rappel est déjà marqué fait.".to_string());
    }
    let now = crate::utils::maintenant_iso();
    let mut tx = base.transaction().map_err(|e| e.0)?;
    tx.executer(
        "UPDATE rappel SET fait = 1, fait_le = ?1 WHERE id = ?2 AND dossier_id = ?3",
        &parametres![now.clone(), rappel_id.clone(), dossier],
    )
    .map_err(|e| e.0)?;
    noter(&mut tx, "rappel_fait", "rappel", &rappel_id, serde_json::json!({}))?;
    tx.valider().map_err(|e| e.0)?;
    Ok(serde_json::json!({ "id": rappel_id, "fait": true, "fait_le": now }))
}

/// Les rappels du dossier : pour une personne, pour un client, ou tous.
/// `inclure_faits` : sinon seuls les rappels actifs, les plus proches
/// d'abord.
pub fn lister_rappels_sur(
    base: &mut Base,
    pour_utilisateur_id: Option<String>,
    client_id: Option<String>,
    inclure_faits: bool,
) -> Result<Vec<serde_json::Value>, String> {
    let dossier = base.dossier().to_string();
    base.lire_plusieurs(
        "SELECT r.id, r.client_id, c.nom, r.pour_utilisateur_id, u.nom, r.quand, r.quoi, r.fait, r.fait_le
         FROM rappel r
         JOIN client c ON c.id = r.client_id AND c.dossier_id = r.dossier_id
         LEFT JOIN utilisateur u ON u.id = r.pour_utilisateur_id
         WHERE r.dossier_id = ?1
           AND (CAST(?2 AS TEXT) IS NULL OR r.pour_utilisateur_id = ?2)
           AND (CAST(?3 AS TEXT) IS NULL OR r.client_id = ?3)
           AND (CAST(?4 AS BIGINT) = 1 OR r.fait = 0)
         ORDER BY r.fait ASC, r.quand ASC",
        &parametres![dossier, pour_utilisateur_id, client_id, inclure_faits as i64],
        |r| {
            Ok(serde_json::json!({
                "id": r.get::<String>(0)?,
                "client_id": r.get::<String>(1)?,
                "client_nom": r.get::<String>(2)?,
                "pour_utilisateur_id": r.get::<String>(3)?,
                "pour_nom": r.get::<Option<String>>(4)?,
                "quand": r.get::<String>(5)?,
                "quoi": r.get::<String>(6)?,
                "fait": r.get::<i64>(7)? != 0,
                "fait_le": r.get::<Option<String>>(8)?,
            }))
        },
    )
    .map_err(|e| e.0)
}

// =====================================================================
//  Prospects (H-2)
// =====================================================================

pub fn creer_prospect_sur(
    base: &mut Base,
    nom: String,
    telephone: Option<String>,
    origine: Option<String>,
) -> Result<serde_json::Value, String> {
    let nom = regles::valider_prospect(&nom)?;
    let telephone = texte(&telephone);
    let origine_prospect = texte(&origine);
    let now = crate::utils::maintenant_iso();
    let id = uuid::Uuid::new_v4().to_string();
    let auteur = crate::argent::id_utilisateur_courant_sur(base);
    let dossier = base.dossier().to_string();

    // Meme generateur de code que `comptoir::creer_client_rapide_sur` :
    // deux formats de code client sur les memes pieces seraient une
    // confusion durable.
    let prefixe = crate::dossiers::prefixe_de_code_sur(base)?;
    let dernier: Option<i64> = base
        .lire_une(
            "SELECT MAX(CAST(SUBSTR(code, LENGTH(?2) + 7) AS BIGINT)) FROM client
             WHERE dossier_id = ?1 AND code LIKE ?3",
            &parametres![dossier.clone(), prefixe.clone(), format!("{prefixe}CLIENT_____")],
            |r| r.get::<Option<i64>>(0),
        )
        .map_err(|e| e.0)?
        .flatten();
    let code = crate::coeur::tiers::code_client_suivant(&prefixe, dernier);

    let mut tx = base.transaction().map_err(|e| e.0)?;
    tx.executer(
        "INSERT INTO client
           (id, code, nom, telephone, est_generique, actif, statut, origine_prospect,
            cree_le, modifie_le, cree_par, modifie_par, origine, dossier_id)
         VALUES (?1,?2,?3,?4,0,1,'prospect',?5,?6,?6,?7,?7,'app',?8)",
        &parametres![
            id.clone(), code.clone(), nom.clone(), telephone.clone(),
            origine_prospect.clone(), now.clone(), auteur.clone(), dossier
        ],
    )
    .map_err(|e| e.0)?;
    noter(&mut tx, "prospect_cree", "client", &id, serde_json::json!({ "nom": nom, "origine": origine_prospect }))?;
    tx.valider().map_err(|e| e.0)?;
    Ok(serde_json::json!({
        "id": id, "code": code, "nom": nom, "telephone": telephone,
        "origine_prospect": origine_prospect, "statut": "prospect",
    }))
}

/// Les prospects du dossier — des clients sans vente encore — les plus
/// recents d'abord.
pub fn lister_prospects_sur(base: &mut Base) -> Result<Vec<serde_json::Value>, String> {
    let dossier = base.dossier().to_string();
    base.lire_plusieurs(
        "SELECT id, code, nom, telephone, origine_prospect, cree_le
         FROM client WHERE dossier_id = ?1 AND statut = 'prospect'
         ORDER BY cree_le DESC",
        &parametres![dossier],
        |r| {
            Ok(serde_json::json!({
                "id": r.get::<String>(0)?,
                "code": r.get::<String>(1)?,
                "nom": r.get::<String>(2)?,
                "telephone": r.get::<Option<String>>(3)?,
                "origine_prospect": r.get::<Option<String>>(4)?,
                "cree_le": r.get::<String>(5)?,
            }))
        },
    )
    .map_err(|e| e.0)
}
