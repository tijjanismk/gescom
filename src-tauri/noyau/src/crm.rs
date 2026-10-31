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
            id.clone(), dossier.clone(), client_id.clone(), genre.clone(), quoi.clone(),
            suite_prevue.clone(), auteur.clone(), now.clone()
        ],
    )
    .map_err(|e| e.0)?;
    // D50, point 10 : un echange sur un prospect « nouveau » le passe
    // « contacte », sans rien saisir. Les autres etapes ne bougent pas.
    tx.executer(
        "UPDATE client SET etape_prospect = ?1, modifie_le = ?2
         WHERE id = ?3 AND dossier_id = ?4 AND statut = 'prospect'
           AND (etape_prospect IS NULL OR etape_prospect = ?5)",
        &parametres![regles::ETAPE_CONTACTE, now.clone(), client_id.clone(), dossier, regles::ETAPE_NOUVEAU],
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
    // C-2 : les droits valent par dossier. Un rappel attribue a qui
    // n'entre pas ici ne lui parviendrait jamais (sa liste est filtree
    // sur son dossier de session) — refuse plutot qu'orphelin.
    if crate::acces_dossiers::role_dans_sur(base, &pour_utilisateur_id, &dossier)?.is_none() {
        return Err(format!(
            "{pour_nom} n'entre pas dans ce dossier : le rappel ne lui parviendrait jamais."
        ));
    }
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
    let code = crate::comptoir::prochain_code_client_sur(base)?;

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
/// recents d'abord, abandonnes compris : l'ecran les cache d'office
/// (D50, point 12).
pub fn lister_prospects_sur(base: &mut Base) -> Result<Vec<serde_json::Value>, String> {
    let dossier = base.dossier().to_string();
    base.lire_plusieurs(
        "SELECT id, code, nom, telephone, origine_prospect, cree_le, etape_prospect, raison_sortie
         FROM client WHERE dossier_id = ?1 AND statut = 'prospect'
         ORDER BY cree_le DESC",
        &parametres![dossier],
        |r| {
            let etape = r.get::<Option<String>>(6)?;
            Ok(serde_json::json!({
                "id": r.get::<String>(0)?,
                "code": r.get::<String>(1)?,
                "nom": r.get::<String>(2)?,
                "telephone": r.get::<Option<String>>(3)?,
                "origine_prospect": r.get::<Option<String>>(4)?,
                "cree_le": r.get::<String>(5)?,
                "etape": regles::etape_lue(etape.as_deref()),
                "raison_abandon": r.get::<Option<String>>(7)?,
            }))
        },
    )
    .map_err(|e| e.0)
}

/// D50, points 10-11 : change l'etape d'un prospect. `abandonne` exige
/// une raison ; un client (deja achete) n'a plus d'etape.
pub fn changer_etape_prospect_sur(
    base: &mut Base,
    client_id: String,
    etape: String,
    raison: Option<String>,
) -> Result<serde_json::Value, String> {
    let (etape, raison) = regles::valider_etape(&etape, raison.as_deref())?;
    let dossier = base.dossier().to_string();
    let (nom, statut): (String, String) = base
        .lire_une(
            "SELECT nom, statut FROM client WHERE id = ?1 AND dossier_id = ?2",
            &parametres![client_id.clone(), dossier.clone()],
            |r| Ok((r.get::<String>(0)?, r.get::<String>(1)?)),
        )
        .map_err(|e| e.0)?
        .ok_or_else(|| "Client introuvable.".to_string())?;
    if statut != "prospect" {
        return Err(format!("{nom} a déjà acheté : ce n'est plus un prospect."));
    }
    let now = crate::utils::maintenant_iso();
    let mut tx = base.transaction().map_err(|e| e.0)?;
    tx.executer(
        "UPDATE client SET etape_prospect = ?1, raison_sortie = ?2, modifie_le = ?3
         WHERE id = ?4 AND dossier_id = ?5 AND statut = 'prospect'",
        &parametres![etape.clone(), raison.clone(), now, client_id.clone(), dossier],
    )
    .map_err(|e| e.0)?;
    noter(&mut tx, "prospect_etape", "client", &client_id, serde_json::json!({ "nom": nom, "etape": etape, "raison": raison }))?;
    tx.valider().map_err(|e| e.0)?;
    Ok(serde_json::json!({ "id": client_id, "etape": etape, "raison_abandon": raison }))
}

/// D50, point 13 : marque un client « parti ». Ne bloque aucune vente,
/// n'efface rien (point 15) : sa prochaine vente le fait revenir tout
/// seul (`argent::prospect_devient_client[_sur]`).
pub fn marquer_client_parti_sur(base: &mut Base, client_id: String, raison: String) -> Result<serde_json::Value, String> {
    let raison = regles::valider_raison_depart(&raison)?;
    let dossier = base.dossier().to_string();
    let (nom, statut, generique, parti): (String, String, i64, Option<String>) = base
        .lire_une(
            "SELECT nom, statut, est_generique, parti_le FROM client WHERE id = ?1 AND dossier_id = ?2",
            &parametres![client_id.clone(), dossier.clone()],
            |r| Ok((r.get::<String>(0)?, r.get::<String>(1)?, r.get::<i64>(2)?, r.get::<Option<String>>(3)?)),
        )
        .map_err(|e| e.0)?
        .ok_or_else(|| "Client introuvable.".to_string())?;
    if generique != 0 {
        return Err("Le client divers ne part pas : il n'est personne.".to_string());
    }
    if statut != "client" {
        return Err(format!("{nom} n'a jamais acheté : abandonner le prospect plutôt."));
    }
    if parti.is_some() {
        return Err(format!("{nom} est déjà marqué parti."));
    }
    let now = crate::utils::maintenant_iso();
    let mut tx = base.transaction().map_err(|e| e.0)?;
    // `revenu_le` vide : un reconquis qui repart n'est plus un reconquis.
    tx.executer(
        "UPDATE client SET parti_le = ?1, raison_sortie = ?2, revenu_le = NULL, modifie_le = ?1
         WHERE id = ?3 AND dossier_id = ?4",
        &parametres![now.clone(), raison.clone(), client_id.clone(), dossier],
    )
    .map_err(|e| e.0)?;
    noter(&mut tx, "client_parti", "client", &client_id, serde_json::json!({ "nom": nom, "raison": raison }))?;
    tx.valider().map_err(|e| e.0)?;
    Ok(serde_json::json!({ "id": client_id, "parti_le": now, "raison": raison }))
}

/// D50, point 13 : « Il est revenu » — annule un depart marque par
/// erreur. Ce n'est pas un reconquis (`revenu_le` ne bouge pas).
pub fn annuler_client_parti_sur(base: &mut Base, client_id: String) -> Result<serde_json::Value, String> {
    let dossier = base.dossier().to_string();
    let (nom, parti): (String, Option<String>) = base
        .lire_une(
            "SELECT nom, parti_le FROM client WHERE id = ?1 AND dossier_id = ?2",
            &parametres![client_id.clone(), dossier.clone()],
            |r| Ok((r.get::<String>(0)?, r.get::<Option<String>>(1)?)),
        )
        .map_err(|e| e.0)?
        .ok_or_else(|| "Client introuvable.".to_string())?;
    if parti.is_none() {
        return Err(format!("{nom} n'est pas marqué parti."));
    }
    let now = crate::utils::maintenant_iso();
    let mut tx = base.transaction().map_err(|e| e.0)?;
    tx.executer(
        "UPDATE client SET parti_le = NULL, raison_sortie = NULL, modifie_le = ?1
         WHERE id = ?2 AND dossier_id = ?3",
        &parametres![now, client_id.clone(), dossier],
    )
    .map_err(|e| e.0)?;
    noter(&mut tx, "client_parti_annule", "client", &client_id, serde_json::json!({ "nom": nom }))?;
    tx.valider().map_err(|e| e.0)?;
    Ok(serde_json::json!({ "id": client_id, "parti_le": serde_json::Value::Null }))
}

/// D50, point 12 : le compte des prospects d'un mois (« AAAA-MM », le
/// mois en cours d'office). Un prospect devenu client se reconnait a
/// son `devenu_client_le`. Aucun montant.
pub fn lire_compte_prospects_sur(base: &mut Base, mois: Option<String>) -> Result<serde_json::Value, String> {
    let mois = match mois.as_deref().map(str::trim).filter(|m| !m.is_empty()) {
        Some(m) => regles::valider_mois(m)?,
        None => crate::utils::maintenant_iso()[..7].to_string(),
    };
    let dossier = base.dossier().to_string();
    // (origine, cree ce mois, devenu client ce mois) — une ligne par
    // fiche, comptee en Rust : pas de GROUP BY sur deux conditions.
    let fiches: Vec<(Option<String>, String, Option<String>)> = base
        .lire_plusieurs(
            "SELECT origine_prospect, cree_le, devenu_client_le FROM client
             WHERE dossier_id = ?1 AND (statut = 'prospect' OR devenu_client_le IS NOT NULL)
               AND (SUBSTR(cree_le, 1, 7) = ?2 OR SUBSTR(devenu_client_le, 1, 7) = ?2)",
            &parametres![dossier.clone(), mois.clone()],
            |r| Ok((r.get::<Option<String>>(0)?, r.get::<String>(1)?, r.get::<Option<String>>(2)?)),
        )
        .map_err(|e| e.0)?;
    let mut crees = 0i64;
    let mut devenus = 0i64;
    let mut par_origine: std::collections::BTreeMap<String, (i64, i64)> = std::collections::BTreeMap::new();
    for (origine, cree_le, devenu) in fiches {
        let origine = origine.map(|o| o.trim().to_string()).filter(|o| !o.is_empty()).unwrap_or_default();
        let e = par_origine.entry(origine).or_default();
        if cree_le.starts_with(&mois) {
            crees += 1;
            e.0 += 1;
        }
        if devenu.as_deref().is_some_and(|d| d.starts_with(&mois)) {
            devenus += 1;
            e.1 += 1;
        }
    }
    let abandons: Vec<(Option<String>, i64)> = base
        .lire_plusieurs(
            "SELECT raison_sortie, COUNT(*) FROM client
             WHERE dossier_id = ?1 AND statut = 'prospect' AND etape_prospect = ?2
             GROUP BY raison_sortie",
            &parametres![dossier, regles::ETAPE_ABANDONNE],
            |r| Ok((r.get::<Option<String>>(0)?, r.get::<i64>(1)?)),
        )
        .map_err(|e| e.0)?;
    Ok(serde_json::json!({
        "mois": mois,
        "crees": crees,
        "devenus_clients": devenus,
        "par_origine": par_origine.into_iter()
            .map(|(o, (c, d))| serde_json::json!({ "origine": if o.is_empty() { serde_json::Value::Null } else { o.into() }, "crees": c, "devenus_clients": d }))
            .collect::<Vec<_>>(),
        "abandons": abandons.into_iter()
            .map(|(r, n)| serde_json::json!({ "raison": r, "nombre": n }))
            .collect::<Vec<_>>(),
    }))
}
