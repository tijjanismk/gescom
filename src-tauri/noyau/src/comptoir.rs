//! Le comptoir : ce que l'ecran de vente appelle en plus du catalogue.
//!
//! Creation rapide d'un client ou d'un article au moment de vendre,
//! stock consolide de tous les depots, reglage du scanner. Rien de
//! spectaculaire — mais sans ces quatre-la, un poste caisse ouvre
//! l'ecran de vente et bute des le premier client inconnu.
//!
//! Le code vient de `commandes/`, coupe et non recrit :
//! `creer_client_rapide` fabrique un code client, et deux formats de
//! code entre le comptoir et la caisse seraient une confusion durable
//! sur les pieces deja imprimees.

use crate::argent::id_utilisateur_courant_pub;
use crate::utils::maintenant_iso;

pub fn creer_client_rapide(
    conn: &rusqlite::Connection,
    nom: String,
    telephone: Option<String>,
) -> Result<serde_json::Value, String> {
    let now = maintenant_iso();
    let id = uuid::Uuid::new_v4().to_string();
    let auteur = id_utilisateur_courant_pub(&conn);

    // Générer un code unique
    let nb: i64 = conn.query_row(
        "SELECT COUNT(*) FROM client WHERE est_generique = 0", [], |r| r.get(0)
    ).unwrap_or(0);
    let code = format!("CLIENT{:05}", nb + 1);

    conn.execute(
        "INSERT INTO client
         (id, code, nom, telephone, est_generique, actif,
          cree_le, modifie_le, cree_par, modifie_par, origine)
         VALUES (?1,?2,?3,?4,0,1,?5,?6,?7,?8,'app')",
        rusqlite::params![id, code, nom, telephone, now, now, auteur, auteur],
    ).map_err(|e| e.to_string())?;

    Ok(serde_json::json!({"id": id, "code": code, "nom": nom, "telephone": telephone}))
}

pub fn creer_article_rapide(
    conn: &rusqlite::Connection,
    nom: String,
    unite_base: String,
    prix_reference: i64,
    // Renseigne depuis l'ecran Achats : on connait le prix d'ACHAT bien
    // avant le prix de vente. Le prix de vente peut alors rester a 0 et
    // se fixer plus tard, dans Parametres.
    prix_achat: Option<i64>,
) -> Result<serde_json::Value, String> {
    let nom = nom.trim().to_string();
    if nom.is_empty() {
        return Err("Le nom de l'article est obligatoire".to_string());
    }


    // Un doublon de nom casse l'import CSV, qui rapproche les articles
    // par `lower(nom)` et en mettrait deux a jour a la fois.
    let existant: Option<(String, String)> = conn.query_row(
        "SELECT id, nom FROM article WHERE lower(nom) = lower(?1) AND actif = 1",
        rusqlite::params![nom], |r| Ok((r.get(0)?, r.get(1)?)),
    ).ok();
    if let Some((_, nom_existant)) = existant {
        return Err(format!("L'article « {} » existe déjà.", nom_existant));
    }
    let now = maintenant_iso();
    let art_id = uuid::Uuid::new_v4().to_string();
    let auteur = id_utilisateur_courant_pub(&conn);

    conn.execute(
        "INSERT INTO article
         (id, nom, unite_base, gere_en_stock, attributs, actif,
          dernier_prix_achat,
          cree_le, modifie_le, cree_par, modifie_par, origine)
         VALUES (?1,?2,?3,1,'{}',1,?4,?5,?6,?7,?8,'app')",
        rusqlite::params![art_id, nom, unite_base, prix_achat,
                          now, now, auteur, auteur],
    ).map_err(|e| e.to_string())?;

    let unite_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO unite_vente
         (id, article_id, libelle, facteur, prix_reference, actif,
          cree_le, modifie_le, cree_par, modifie_par, origine)
         VALUES (?1,?2,?3,1.0,?4,1,?5,?6,?7,?8,'app')",
        rusqlite::params![unite_id, art_id, unite_base, prix_reference,
                          now, now, auteur, auteur],
    ).map_err(|e| e.to_string())?;

    // Initialiser le stock à 0 dans le dépôt par défaut
    let depot_id: String = conn.query_row(
        "SELECT id FROM depot WHERE est_defaut = 1 LIMIT 1",
        [], |r| r.get(0)
    ).or_else(|_| conn.query_row(
        "SELECT id FROM depot WHERE actif = 1 ORDER BY nom LIMIT 1",
        [], |r| r.get(0)
    )).map_err(|_| "Aucun magasin actif".to_string())?;

    conn.execute(
        "INSERT OR IGNORE INTO stock_depot (id, article_id, depot_id, quantite)
         VALUES (?1,?2,?3,0)",
        rusqlite::params![uuid::Uuid::new_v4().to_string(), art_id, depot_id],
    ).ok();

    Ok(serde_json::json!({
        "id": art_id, "nom": nom, "unite_base": unite_base, "stock": 0.0,
        "unites": [{"id": unite_id, "libelle": unite_base,
                    "facteur": 1.0, "prix_reference": prix_reference}]
    }))
}

pub fn lire_stock_multi_depots_sur(
    conn: &rusqlite::Connection,
) -> Result<Vec<serde_json::Value>, String> {
    let mut st = conn.prepare(
        "SELECT sd.article_id, sd.depot_id, d.nom, d.est_defaut, sd.quantite
         FROM stock_depot sd
         JOIN depot d ON d.id = sd.depot_id
         WHERE d.actif = 1 AND sd.quantite <> 0"
    ).map_err(|e| e.to_string())?;

    let x = st.query_map([], |r| {
        Ok(serde_json::json!({
            "article_id": r.get::<_, String>(0)?,
            "depot_id":   r.get::<_, String>(1)?,
            "depot_nom":  r.get::<_, String>(2)?,
            "est_defaut": r.get::<_, i64>(3)? != 0,
            "quantite":   r.get::<_, f64>(4)?,
        }))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    Ok(x)
}

/// Le scanner de codes-barres est-il actif ?
pub fn lire_config_scanner(conn: &rusqlite::Connection) -> Result<bool, String> {
    let actif: String = conn
        .query_row(
            "SELECT valeur FROM config_app WHERE cle = 'scanner_actif'",
            [],
            |r| r.get(0),
        )
        .unwrap_or_else(|_| "0".to_string());
    Ok(actif == "1")
}

// =====================================================================
//  CLIENTS — ce qui manquait au portage
// =====================================================================
//
// Ces deux commandes etaient restees dans le crate applicatif, sans
// raison : elles ne touchent ni a l'ecran ni au systeme, ce sont des
// commandes metier comme les autres. Une caisse en reseau les executait
// donc sur SA base locale — vide. Modifier un client y semblait
// reussir, et la modification n'existait nulle part.

/// Modifie la fiche d'un client.
pub fn modifier_client(
    conn: &rusqlite::Connection,
    client_id: String,
    nom: String,
    telephone: Option<String>,
    adresse: Option<String>,
    email: Option<String>,
    nif: Option<String>,
) -> Result<(), String> {
    if nom.trim().is_empty() {
        return Err("Le nom est obligatoire".to_string());
    }

    let est_generique: i64 = conn
        .query_row(
            "SELECT est_generique FROM client WHERE id = ?1",
            rusqlite::params![client_id],
            |r| r.get(0),
        )
        .map_err(|_| "Client introuvable".to_string())?;

    if est_generique == 1 {
        return Err("Le client générique ne se modifie pas : il regroupe toutes les \
                    ventes au comptant."
            .to_string());
    }

    let now = crate::utils::maintenant_iso();
    let auteur = crate::argent::id_utilisateur_courant_pub(conn);

    // `vide` plutot que la chaine vide : un champ efface doit redevenir
    // NULL, sinon les ecrans affichent une ligne vide au lieu de rien.
    let vide = |o: Option<String>| o.filter(|s| !s.trim().is_empty());

    conn.execute(
        "UPDATE client
         SET nom = ?1, telephone = ?2, adresse = ?3, email = ?4, nif = ?5,
             modifie_le = ?6, modifie_par = ?7
         WHERE id = ?8",
        rusqlite::params![
            nom.trim(),
            vide(telephone),
            vide(adresse),
            vide(email),
            vide(nif),
            now,
            auteur,
            client_id
        ],
    )
    .map_err(|e| e.to_string())?;

    conn.execute(
        "INSERT INTO journal
         (id, type_evenement, entite_type, entite_id, auteur_id,
          nouveau_valeur, origine, date_evenement)
         VALUES (?1,'client_modifie','client',?2,?3,?4,'app',?5)",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(),
            client_id,
            auteur,
            format!("{{\"nom\":\"{}\"}}", nom.trim().replace('"', "'")),
            now
        ],
    )
    .ok();

    Ok(())
}

/// Les clients et ce qu'ils doivent, du plus endette au moins.
pub fn lire_clients_avec_creances(
    conn: &rusqlite::Connection,
) -> Result<Vec<serde_json::Value>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT c.id, c.code, c.nom, c.telephone,
                CAST(COALESCE(SUM(
                  CASE WHEN v.statut != 'payee' THEN
                    (SELECT COALESCE(SUM(prix_pratique * quantite), 0)
                     FROM ligne_vente WHERE vente_id = v.id) -
                    (SELECT COALESCE(SUM(montant), 0)
                     FROM paiement WHERE vente_id = v.id)
                  ELSE 0 END
                ), 0) AS INTEGER) as total_creances,
                COUNT(DISTINCT v.id) as nb_ventes
         FROM client c
         LEFT JOIN vente v ON v.client_id = c.id
         WHERE c.actif = 1 AND c.est_generique = 0
         GROUP BY c.id
         ORDER BY total_creances DESC, c.nom ASC",
        )
        .map_err(|e| e.to_string())?;

    let x = stmt
        .query_map([], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_,String>(0)?,
                "code": row.get::<_,String>(1)?,
                "nom": row.get::<_,String>(2)?,
                "telephone": row.get::<_,Option<String>>(3)?,
                "total_creances": row.get::<_,i64>(4)?,
                "nb_ventes": row.get::<_,i64>(5)?,
            }))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    Ok(x)
}

// =====================================================================
//  LES MEMES, SUR L'UN OU L'AUTRE MOTEUR
// =====================================================================
//
// Troisieme lot du portage. Celui-ci ECRIT : c'est le premier a le
// faire, et c'est pour cela qu'il vient avant `argent`. On eprouve
// l'insertion, le conflit de doublon et la transaction sur des clients
// et des articles — ou une erreur se corrige — avant de toucher a une
// vente, ou elle ne se corrige pas.

use crate::base::Base;
use crate::parametres;

/// L'identifiant de l'utilisateur courant, sur l'un ou l'autre moteur.
///
/// Le repli sur « system » est deliberе : une operation ne doit jamais
/// echouer parce qu'on ne sait pas QUI la fait — elle doit s'ecrire, et
/// le journal dira qu'on ne savait pas.
fn auteur_courant(base: &mut Base) -> String {
    base.lire_une(
        "SELECT id FROM utilisateur WHERE actif = 1 ORDER BY cree_le LIMIT 1",
        &[],
        |r| r.get::<String>(0),
    )
    .ok()
    .flatten()
    .unwrap_or_else(|| "system".to_string())
}

pub fn creer_client_rapide_sur(
    base: &mut Base,
    nom: String,
    telephone: Option<String>,
) -> Result<serde_json::Value, String> {
    let now = maintenant_iso();
    let id = uuid::Uuid::new_v4().to_string();
    let auteur = auteur_courant(base);
    let dossier = base.dossier().to_string();

    // Le code suit le NOMBRE de clients reels DU DOSSIER : le client
    // generique ne compte pas, sinon la numerotation commencerait a 2 —
    // et les clients d'une autre societe non plus, sinon deux dossiers
    // auraient des codes qui se suivent sans se ressembler.
    let nb = base
        .lire_une(
            "SELECT COUNT(*) FROM client WHERE est_generique = 0 AND dossier_id = ?1",
            &parametres![dossier.clone()],
            |r| r.get::<i64>(0),
        )
        .map_err(|e| e.0)?
        .unwrap_or(0);
    let code = format!("CLIENT{:05}", nb + 1);

    base.executer(
        "INSERT INTO client
           (id, code, nom, telephone, est_generique, actif,
            cree_le, modifie_le, cree_par, modifie_par, origine, dossier_id)
         VALUES (?1,?2,?3,?4,0,1,?5,?5,?6,?6,'app',?7)",
        &parametres![
            id.clone(),
            code.clone(),
            nom.clone(),
            telephone.clone(),
            now,
            auteur,
            dossier
        ],
    )
    .map_err(|e| e.0)?;

    Ok(serde_json::json!({
        "id": id, "code": code, "nom": nom, "telephone": telephone
    }))
}

pub fn creer_article_rapide_sur(
    base: &mut Base,
    nom: String,
    unite_base: String,
    prix_reference: i64,
    prix_achat: Option<i64>,
) -> Result<serde_json::Value, String> {
    let nom = nom.trim().to_string();
    if nom.is_empty() {
        return Err("Le nom de l'article est obligatoire".to_string());
    }

    // Un doublon de nom casse l'import CSV, qui rapproche les articles
    // par `lower(nom)` et en mettrait deux a jour a la fois. La
    // recherche porte sur TOUS les dossiers : l'article est commun,
    // « CIMAF » ne se saisit pas deux fois sous pretexte qu'un autre
    // dossier l'a deja fait.
    let dossier = base.dossier().to_string();
    if let Some(existant) = base
        .lire_une(
            "SELECT nom FROM article WHERE lower(nom) = lower(?1) AND actif = 1",
            &parametres![nom.clone()],
            |r| r.get::<String>(0),
        )
        .map_err(|e| e.0)?
    {
        return Err(format!("L'article « {existant} » existe déjà."));
    }

    let now = maintenant_iso();
    let art_id = uuid::Uuid::new_v4().to_string();
    let unite_id = uuid::Uuid::new_v4().to_string();
    let auteur = auteur_courant(base);

    let depot_id = base
        .lire_une(
            "SELECT id FROM depot
             WHERE est_defaut = 1 AND actif = 1 AND dossier_id = ?1 LIMIT 1",
            &parametres![dossier.clone()],
            |r| r.get::<String>(0),
        )
        .map_err(|e| e.0)?
        .ok_or_else(|| "Aucun magasin actif".to_string())?;

    // Une TRANSACTION : l'article, son unite et sa ligne de stock
    // forment un tout. Un article sans unite de vente ne se vend pas,
    // et il faudrait le reparer a la main dans la base.
    let mut tx = base.transaction().map_err(|e| e.0)?;

    // Ni `article` ni `unite_vente` ne portent `dossier_id` : ils sont
    // communs a tous les dossiers.
    tx.executer(
        "INSERT INTO article
           (id, nom, unite_base, gere_en_stock, attributs, actif,
            dernier_prix_achat, cree_le, modifie_le, cree_par, modifie_par,
            origine)
         VALUES (?1,?2,?3,1,'{}',1,?4,?5,?5,?6,?6,'app')",
        &parametres![
            art_id.clone(),
            nom.clone(),
            unite_base.clone(),
            prix_achat,
            now.clone(),
            auteur.clone(),
        ],
    )
    .map_err(|e| e.0)?;

    tx.executer(
        "INSERT INTO unite_vente
           (id, article_id, libelle, facteur, prix_reference, actif,
            cree_le, modifie_le, cree_par, modifie_par, origine)
         VALUES (?1,?2,?3,1.0,?4,1,?5,?5,?6,?6,'app')",
        &parametres![
            unite_id.clone(),
            art_id.clone(),
            unite_base.clone(),
            prix_reference,
            now,
            auteur,
        ],
    )
    .map_err(|e| e.0)?;

    tx.executer(
        "INSERT INTO stock_depot (id, article_id, depot_id, quantite, dossier_id)
         VALUES (?1,?2,?3,0,?4)
         ON CONFLICT (article_id, depot_id) DO NOTHING",
        &parametres![
            uuid::Uuid::new_v4().to_string(),
            art_id.clone(),
            depot_id,
            dossier
        ],
    )
    .map_err(|e| e.0)?;

    tx.valider().map_err(|e| e.0)?;

    Ok(serde_json::json!({
        "id": art_id, "nom": nom, "unite_base": unite_base, "stock": 0.0,
        "unites": [{"id": unite_id, "libelle": unite_base,
                    "facteur": 1.0, "prix_reference": prix_reference}]
    }))
}

pub fn modifier_client_sur(
    base: &mut Base,
    client_id: String,
    nom: String,
    telephone: Option<String>,
    adresse: Option<String>,
    email: Option<String>,
    nif: Option<String>,
) -> Result<(), String> {
    if nom.trim().is_empty() {
        return Err("Le nom est obligatoire".to_string());
    }

    let dossier = base.dossier().to_string();
    let est_generique = base
        .lire_une(
            "SELECT est_generique FROM client WHERE id = ?1 AND dossier_id = ?2",
            &parametres![client_id.clone(), dossier.clone()],
            |r| r.get::<i64>(0),
        )
        .map_err(|e| e.0)?
        .ok_or_else(|| "Client introuvable".to_string())?;

    if est_generique == 1 {
        return Err("Le client générique ne se modifie pas : il regroupe toutes les \
                    ventes au comptant."
            .to_string());
    }

    let now = maintenant_iso();
    let auteur = auteur_courant(base);
    // `vide` plutot que la chaine vide : un champ efface doit redevenir
    // NULL, sinon les ecrans affichent une ligne vide au lieu de rien.
    let vide = |o: Option<String>| o.filter(|s| !s.trim().is_empty());

    base.executer(
        "UPDATE client
         SET nom = ?1, telephone = ?2, adresse = ?3, email = ?4, nif = ?5,
             modifie_le = ?6, modifie_par = ?7
         WHERE id = ?8 AND dossier_id = ?9",
        &parametres![
            nom.trim(),
            vide(telephone),
            vide(adresse),
            vide(email),
            vide(nif),
            now.clone(),
            auteur.clone(),
            client_id.clone(),
            dossier.clone()
        ],
    )
    .map_err(|e| e.0)?;

    let _ = base.executer(
        "INSERT INTO journal
           (id, type_evenement, entite_type, entite_id, auteur_id,
            nouveau_valeur, origine, date_evenement, dossier_id)
         VALUES (?1,'client_modifie','client',?2,?3,?4,'app',?5,?6)",
        &parametres![
            uuid::Uuid::new_v4().to_string(),
            client_id,
            auteur,
            format!("{{\"nom\":\"{}\"}}", nom.trim().replace('"', "'")),
            now,
            dossier
        ],
    );

    Ok(())
}

pub fn lire_clients_avec_creances_sur(
    base: &mut Base,
) -> Result<Vec<serde_json::Value>, String> {
    let dossier = base.dossier().to_string();
    base.lire_plusieurs(
        "SELECT c.id, c.code, c.nom, c.telephone,
                CAST(COALESCE(SUM(
                  CASE WHEN v.statut <> 'payee' THEN
                    (SELECT COALESCE(SUM(prix_pratique * quantite), 0)
                     FROM ligne_vente WHERE vente_id = v.id) -
                    (SELECT COALESCE(SUM(montant), 0)
                     FROM paiement WHERE vente_id = v.id)
                  ELSE 0 END
                ), 0) AS BIGINT) as total_creances,
                COUNT(DISTINCT v.id) as nb_ventes
         FROM client c
         LEFT JOIN vente v ON v.client_id = c.id
         WHERE c.actif = 1 AND c.est_generique = 0 AND c.dossier_id = ?1
         GROUP BY c.id, c.code, c.nom, c.telephone
         ORDER BY total_creances DESC, c.nom ASC",
        &parametres![dossier],
        |r| {
            Ok(serde_json::json!({
                "id": r.get::<String>(0)?,
                "code": r.get::<String>(1)?,
                "nom": r.get::<String>(2)?,
                "telephone": r.get::<Option<String>>(3)?,
                "total_creances": r.get::<i64>(4)?,
                "nb_ventes": r.get::<i64>(5)?,
            }))
        },
    )
    .map_err(|e| e.0)
}

pub fn lire_config_scanner_sur(base: &mut Base) -> Result<bool, String> {
    Ok(base
        .lire_une(
            "SELECT valeur FROM config_app WHERE cle = 'scanner_actif'",
            &[],
            |r| r.get::<String>(0),
        )
        .map_err(|e| e.0)?
        .map(|v| v == "1")
        .unwrap_or(false))
}
