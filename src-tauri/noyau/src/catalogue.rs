//! Les lectures du comptoir : clients, dépôts, articles.
//!
//! ## Pourquoi celles-ci d'abord
//!
//! Ce sont les cinq commandes qu'un poste caisse appelle avant de
//! pouvoir afficher quoi que ce soit. Tant qu'elles ne passent pas par
//! le serveur, une caisse connectée montre un écran vide — et rien ne
//! sert de porter `creer_vente` avant de pouvoir choisir un client.
//!
//! Elles ne touchent à rien : que des `SELECT`. C'est ce qui en fait le
//! bon premier lot d'une migration qui doit se faire domaine par
//! domaine — on éprouve le chemin serveur sans risquer une écriture
//! d'argent.
//!
//! Le code vient de `commandes/ventes.rs` et n'a pas été récrit : la
//! façade Tauri l'appelle désormais ici, le serveur aussi. Deux copies
//! auraient fini par ne plus donner le même catalogue au comptoir et à
//! la caisse.

use rusqlite::Connection;
use serde_json::{json, Value};

pub fn lire_clients(conn: &Connection) -> Result<Vec<Value>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, code, nom, telephone FROM client
             WHERE actif = 1 ORDER BY nom ASC",
        )
        .map_err(|e| e.to_string())?;
    let x = stmt
        .query_map([], |row| {
            Ok(json!({
                "id": row.get::<_, String>(0)?,
                "code": row.get::<_, String>(1)?,
                "nom": row.get::<_, String>(2)?,
                "telephone": row.get::<_, Option<String>>(3)?,
            }))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    Ok(x)
}

/// Le client de passage — celui des ventes au comptant (D40).
pub fn lire_client_generique(conn: &Connection) -> Result<Value, String> {
    conn.query_row(
        "SELECT id, code, nom FROM client WHERE est_generique = 1 LIMIT 1",
        [],
        |row| {
            Ok(json!({
                "id": row.get::<_, String>(0)?,
                "code": row.get::<_, String>(1)?,
                "nom": row.get::<_, String>(2)?,
            }))
        },
    )
    .map_err(|e| e.to_string())
}

pub fn lire_depots(conn: &Connection) -> Result<Vec<Value>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, nom, est_defaut FROM depot WHERE actif = 1
             ORDER BY est_defaut DESC, nom ASC",
        )
        .map_err(|e| e.to_string())?;
    let x = stmt
        .query_map([], |row| {
            Ok(json!({
                "id": row.get::<_, String>(0)?,
                "nom": row.get::<_, String>(1)?,
                "est_defaut": row.get::<_, i64>(2)? != 0,
            }))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    Ok(x)
}

pub fn lire_depot_defaut(conn: &Connection) -> Result<Value, String> {
    conn.query_row(
        "SELECT id, nom FROM depot WHERE est_defaut = 1 LIMIT 1",
        [],
        |row| {
            Ok(json!({
                "id": row.get::<_, String>(0)?,
                "nom": row.get::<_, String>(1)?,
            }))
        },
    )
    .map_err(|e| e.to_string())
}

/// Le catalogue du point de vente, avec le stock du dépôt demandé.
pub fn lire_articles_avec_unites(
    conn: &Connection,
    role: Option<String>,
    depot_id: Option<String>,
) -> Result<Vec<Value>, String> {
    let est_patron = role.as_deref() == Some("patron");

    // Un depot inconnu ou desactive retombe sur le defaut plutot que
    // d'echouer : l'ecran doit s'ouvrir meme apres desactivation du
    // depot memorise dans la sidebar.
    let depot_id: String = match depot_id.filter(|d| !d.is_empty()) {
        Some(d) => conn
            .query_row(
                "SELECT id FROM depot WHERE id = ?1 AND actif = 1",
                rusqlite::params![d],
                |row| row.get(0),
            )
            .or_else(|_| {
                conn.query_row(
                    "SELECT id FROM depot WHERE est_defaut = 1 AND actif = 1 LIMIT 1",
                    [],
                    |row| row.get(0),
                )
            })
            .map_err(|e| e.to_string())?,
        None => conn
            .query_row(
                "SELECT id FROM depot WHERE est_defaut = 1 AND actif = 1 LIMIT 1",
                [],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?,
    };

    let mut stmt = conn
        .prepare(
            "SELECT a.id, a.nom, a.unite_base, a.dernier_prix_achat,
                    u.id, u.libelle, u.facteur, u.prix_reference,
                    COALESCE(sd.quantite, 0) as stock,
                    COALESCE(a.taux_tva_defaut, 0.0) as taux_tva_defaut
             FROM article a
             JOIN unite_vente u ON u.article_id = a.id AND u.actif = 1
             LEFT JOIN stock_depot sd ON sd.article_id = a.id AND sd.depot_id = ?1
             WHERE a.actif = 1
             ORDER BY a.nom, u.facteur ASC",
        )
        .map_err(|e| e.to_string())?;

    let mut articles: Vec<Value> = Vec::new();
    let mut courant_id = String::new();

    stmt.query_map(rusqlite::params![depot_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Option<i64>>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, String>(5)?,
            row.get::<_, f64>(6)?,
            row.get::<_, i64>(7)?,
            row.get::<_, f64>(8)?,
            row.get::<_, f64>(9)?,
        ))
    })
    .map_err(|e| e.to_string())?
    .filter_map(|r| r.ok())
    .for_each(
        |(art_id, art_nom, unite_base, prix_achat, u_id, u_libelle, facteur, prix_ref, stock, taux_tva)| {
            let unite = json!({
                "id": u_id, "libelle": u_libelle,
                "facteur": facteur, "prix_reference": prix_ref,
            });
            if art_id != courant_id {
                courant_id = art_id.clone();
                let mut art = json!({
                    "id": art_id, "nom": art_nom,
                    "unite_base": unite_base, "stock": stock,
                    // Source unique du taux : la fiche article (Parametres -> TVA).
                    "taux_tva_defaut": taux_tva,
                    "unites": [unite],
                });
                // §7 — Prix d'achat protégé côté serveur.
                //
                // Le filtre est ICI et non dans l'écran : un poste
                // caisse qui parle au serveur reçoit la réponse telle
                // quelle, et un employé n'a pas à connaître la marge du
                // patron parce qu'il sait ouvrir les outils du
                // navigateur.
                if est_patron {
                    art["dernier_prix_achat"] = prix_achat
                        .map(|p| json!(p))
                        .unwrap_or(Value::Null);
                }
                articles.push(art);
            } else if let Some(last) = articles.last_mut() {
                if let Some(unites) = last["unites"].as_array_mut() {
                    unites.push(unite);
                }
            }
        },
    );
    Ok(articles)
}

// =====================================================================
//  LES MEMES, SUR L'UN OU L'AUTRE MOTEUR
// =====================================================================
//
// Deuxieme lot du portage, apres « se connecter ». Ce sont les lectures
// qu'un poste caisse fait AVANT d'afficher quoi que ce soit : sans
// elles, une caisse branchee sur PostgreSQL montre un ecran vide.
//
// Que des SELECT. C'est ce qui en fait le bon lot suivant — on eprouve
// le chemin sur des requetes a jointures, des agregats et des colonnes
// nulles, sans risquer une ecriture d'argent.

use crate::base::{Base, Resultat};
use crate::parametres;

pub fn lire_clients_sur(base: &mut Base) -> Result<Vec<Value>, String> {
    let dossier = base.dossier().to_string();
    base.lire_plusieurs(
        "SELECT id, code, nom, telephone FROM client
         WHERE actif = 1 AND dossier_id = ?1 ORDER BY nom ASC",
        &parametres![dossier],
        |r| {
            Ok(json!({
                "id": r.get::<String>(0)?,
                "code": r.get::<String>(1)?,
                "nom": r.get::<String>(2)?,
                "telephone": r.get::<Option<String>>(3)?,
            }))
        },
    )
    .map_err(|e| e.0)
}

pub fn lire_client_generique_sur(base: &mut Base) -> Result<Value, String> {
    let dossier = base.dossier().to_string();
    base.lire_une(
        "SELECT id, code, nom FROM client
         WHERE est_generique = 1 AND dossier_id = ?1 LIMIT 1",
        &parametres![dossier],
        |r| {
            Ok(json!({
                "id": r.get::<String>(0)?,
                "code": r.get::<String>(1)?,
                "nom": r.get::<String>(2)?,
            }))
        },
    )
    .map_err(|e| e.0)?
    .ok_or_else(|| "Aucun client générique".to_string())
}

pub fn lire_depots_sur(base: &mut Base) -> Result<Vec<Value>, String> {
    let dossier = base.dossier().to_string();
    base.lire_plusieurs(
        "SELECT id, nom, est_defaut FROM depot
         WHERE actif = 1 AND dossier_id = ?1
         ORDER BY est_defaut DESC, nom ASC",
        &parametres![dossier],
        |r| {
            Ok(json!({
                "id": r.get::<String>(0)?,
                "nom": r.get::<String>(1)?,
                "est_defaut": r.get::<i64>(2)? != 0,
            }))
        },
    )
    .map_err(|e| e.0)
}

pub fn lire_depot_defaut_sur(base: &mut Base) -> Result<Value, String> {
    let dossier = base.dossier().to_string();
    base.lire_une(
        "SELECT id, nom FROM depot
         WHERE est_defaut = 1 AND dossier_id = ?1 LIMIT 1",
        &parametres![dossier],
        |r| {
            Ok(json!({
                "id": r.get::<String>(0)?,
                "nom": r.get::<String>(1)?,
            }))
        },
    )
    .map_err(|e| e.0)?
    .ok_or_else(|| "Aucun magasin par défaut".to_string())
}

/// Le depot a servir : celui demande s'il existe et vit, sinon le
/// defaut.
///
/// Un depot inconnu ou desactive ne doit pas faire echouer l'ecran —
/// il se retrouve memorise dans la barre laterale bien apres avoir ete
/// ferme, et le commercant ne comprendrait pas pourquoi sa caisse
/// refuse de s'ouvrir.
fn depot_a_servir(base: &mut Base, demande: Option<String>) -> Resultat<String> {
    let dossier = base.dossier().to_string();
    if let Some(d) = demande.filter(|d| !d.is_empty()) {
        // Le filtre de dossier compte ici AUTANT que le `actif = 1` :
        // un identifiant memorise dans la barre laterale pourrait
        // designer le magasin d'une autre societe, et la caisse
        // servirait son stock sans rien signaler.
        if let Some(id) = base.lire_une(
            "SELECT id FROM depot WHERE id = ?1 AND actif = 1 AND dossier_id = ?2",
            &parametres![d, dossier.clone()],
            |r| r.get::<String>(0),
        )? {
            return Ok(id);
        }
    }
    base.lire_une(
        "SELECT id FROM depot
         WHERE est_defaut = 1 AND actif = 1 AND dossier_id = ?1 LIMIT 1",
        &parametres![dossier],
        |r| r.get::<String>(0),
    )?
    .ok_or_else(|| crate::base::Erreur("Aucun magasin par défaut".into()))
}

pub fn lire_articles_avec_unites_sur(
    base: &mut Base,
    role: Option<String>,
    depot_id: Option<String>,
) -> Result<Vec<Value>, String> {
    let est_patron = role.as_deref() == Some("patron");
    let depot_id = depot_a_servir(base, depot_id).map_err(|e| e.0)?;
    let dossier = base.dossier().to_string();

    // `article` et `unite_vente` sont communs a tous les dossiers (D3 du
    // plan multi-societe) : pas de filtre dessus. Le stock, lui, reste
    // celui d'UN magasin d'UN dossier — le filtre porte sur `stock_depot`,
    // dans la jointure et non le WHERE, pour ne pas transformer le LEFT
    // JOIN en INNER quand un article n'a encore aucun stock ici.
    let lignes = base
        .lire_plusieurs(
            "SELECT a.id, a.nom, a.unite_base, a.dernier_prix_achat,
                    u.id, u.libelle, u.facteur, u.prix_reference,
                    COALESCE(sd.quantite, 0) as stock,
                    COALESCE(a.taux_tva_defaut, 0.0) as taux_tva_defaut
             FROM article a
             JOIN unite_vente u ON u.article_id = a.id AND u.actif = 1
             LEFT JOIN stock_depot sd
                 ON sd.article_id = a.id AND sd.depot_id = ?1 AND sd.dossier_id = ?2
             WHERE a.actif = 1
             ORDER BY a.nom, u.facteur ASC",
            &parametres![depot_id, dossier],
            |r| {
                Ok((
                    r.get::<String>(0)?,
                    r.get::<String>(1)?,
                    r.get::<String>(2)?,
                    r.get::<Option<i64>>(3)?,
                    r.get::<String>(4)?,
                    r.get::<String>(5)?,
                    r.get::<f64>(6)?,
                    r.get::<i64>(7)?,
                    r.get::<f64>(8)?,
                    r.get::<f64>(9)?,
                ))
            },
        )
        .map_err(|e| e.0)?;

    let mut articles: Vec<Value> = Vec::new();
    let mut courant_id = String::new();

    for (art_id, art_nom, unite_base, prix_achat, u_id, u_libelle, facteur, prix_ref, stock, taux_tva)
        in lignes
    {
        let unite = json!({
            "id": u_id, "libelle": u_libelle,
            "facteur": facteur, "prix_reference": prix_ref,
        });
        if art_id != courant_id {
            courant_id = art_id.clone();
            let mut art = json!({
                "id": art_id, "nom": art_nom,
                "unite_base": unite_base, "stock": stock,
                "taux_tva_defaut": taux_tva,
                "unites": [unite],
            });
            // Le prix d'achat est filtre ICI et non dans l'ecran : un
            // poste caisse recoit la reponse telle quelle, et un employe
            // n'a pas a connaitre la marge du patron parce qu'il sait
            // ouvrir les outils du navigateur.
            if est_patron {
                art["dernier_prix_achat"] =
                    prix_achat.map(|p| json!(p)).unwrap_or(Value::Null);
            }
            articles.push(art);
        } else if let Some(last) = articles.last_mut() {
            if let Some(unites) = last["unites"].as_array_mut() {
                unites.push(unite);
            }
        }
    }
    Ok(articles)
}
