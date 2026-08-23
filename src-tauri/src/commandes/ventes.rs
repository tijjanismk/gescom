//! Commandes Tauri pour les ventes et paiements.

use tauri::State;
use std::sync::Mutex;
use rusqlite::Connection;
use crate::utils::maintenant_iso;

// =====================================================================
//  État partagé
// =====================================================================

pub struct EtatApp {
    pub conn: Mutex<Connection>,
}

// =====================================================================
//  Utilitaire utilisateur
// =====================================================================

pub fn id_utilisateur_courant_pub(conn: &Connection) -> String {
    conn.query_row(
        "SELECT id FROM utilisateur WHERE actif = 1 LIMIT 1",
        [], |row| row.get(0),
    ).unwrap_or_else(|_| "system".to_string())
}

/// Récupère l'id utilisateur selon son rôle — pour le multi-utilisateur.
pub fn id_utilisateur_par_role(conn: &Connection, role: &str) -> String {
    conn.query_row(
        "SELECT u.id FROM utilisateur u
         JOIN role r ON r.id = u.role_id
         WHERE r.nom = ?1 AND u.actif = 1 LIMIT 1",
        rusqlite::params![role],
        |row| row.get(0),
    ).unwrap_or_else(|_| id_utilisateur_courant_pub(conn))
}

// =====================================================================
//  CLIENTS
// =====================================================================

#[tauri::command]
pub fn lire_clients(etat: State<EtatApp>) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(
        "SELECT id, code, nom, telephone FROM client
         WHERE actif = 1 ORDER BY nom ASC"
    ).map_err(|e| e.to_string())?;
    let x = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_,String>(0)?,
            "code": row.get::<_,String>(1)?,
            "nom": row.get::<_,String>(2)?,
            "telephone": row.get::<_,Option<String>>(3)?,
        }))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
    Ok(x)
}

#[tauri::command]
pub fn lire_client_generique(etat: State<EtatApp>) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let result = conn.query_row(
        "SELECT id, code, nom FROM client WHERE est_generique = 1 LIMIT 1",
        [], |row| Ok(serde_json::json!({
            "id": row.get::<_,String>(0)?,
            "code": row.get::<_,String>(1)?,
            "nom": row.get::<_,String>(2)?,
        }))
    ).map_err(|e| e.to_string())?;
    Ok(result)
}

#[tauri::command]
pub fn creer_client_rapide(
    etat: State<EtatApp>,
    nom: String,
    telephone: Option<String>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
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

// =====================================================================
//  ARTICLES
// =====================================================================

#[tauri::command]
pub fn lire_articles_avec_unites(
    etat: State<EtatApp>,
    role: Option<String>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let est_patron = role.as_deref() == Some("patron");

    let depot_id: String = conn.query_row(
        "SELECT id FROM depot WHERE est_defaut = 1 AND actif = 1 LIMIT 1",
        [], |row| row.get(0),
    ).map_err(|e| e.to_string())?;

    let mut stmt = conn.prepare(
        "SELECT a.id, a.nom, a.unite_base, a.dernier_prix_achat,
                u.id, u.libelle, u.facteur, u.prix_reference,
                COALESCE(sd.quantite, 0) as stock,
                COALESCE(a.taux_tva_defaut, 0.0) as taux_tva_defaut
         FROM article a
         JOIN unite_vente u ON u.article_id = a.id AND u.actif = 1
         LEFT JOIN stock_depot sd ON sd.article_id = a.id AND sd.depot_id = ?1
         WHERE a.actif = 1
         ORDER BY a.nom, u.facteur ASC"
    ).map_err(|e| e.to_string())?;

    let mut articles: Vec<serde_json::Value> = Vec::new();
    let mut courant_id = String::new();

    stmt.query_map(rusqlite::params![depot_id], |row| {
        Ok((
            row.get::<_,String>(0)?,
            row.get::<_,String>(1)?,
            row.get::<_,String>(2)?,
            row.get::<_,Option<i64>>(3)?,
            row.get::<_,String>(4)?,
            row.get::<_,String>(5)?,
            row.get::<_,f64>(6)?,
            row.get::<_,i64>(7)?,
            row.get::<_,f64>(8)?,
            row.get::<_,f64>(9)?,
        ))
    }).map_err(|e| e.to_string())?
    .filter_map(|r| r.ok())
    .for_each(|(art_id, art_nom, unite_base, prix_achat,
                u_id, u_libelle, facteur, prix_ref, stock, taux_tva)| {
        let unite = serde_json::json!({
            "id": u_id, "libelle": u_libelle,
            "facteur": facteur, "prix_reference": prix_ref,
        });
        if art_id != courant_id {
            courant_id = art_id.clone();
            let mut art = serde_json::json!({
                "id": art_id, "nom": art_nom,
                "unite_base": unite_base, "stock": stock,
                // Source unique du taux : la fiche article (Parametres -> TVA).
                "taux_tva_defaut": taux_tva,
                "unites": [unite],
            });
            // §7 — Prix d'achat protégé côté serveur
            if est_patron {
                art["dernier_prix_achat"] = prix_achat
                    .map(|p| serde_json::json!(p))
                    .unwrap_or(serde_json::Value::Null);
            }
            articles.push(art);
        } else if let Some(last) = articles.last_mut() {
            if let Some(unites) = last["unites"].as_array_mut() {
                unites.push(unite);
            }
        }
    });
    Ok(articles)
}

#[tauri::command]
pub fn lire_depots(etat: State<EtatApp>) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(
        "SELECT id, nom, est_defaut FROM depot WHERE actif = 1
         ORDER BY est_defaut DESC, nom ASC"
    ).map_err(|e| e.to_string())?;
    let x = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_,String>(0)?,
            "nom": row.get::<_,String>(1)?,
            "est_defaut": row.get::<_,i64>(2)? != 0,
        }))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
    Ok(x)
}

#[tauri::command]
pub fn lire_depot_defaut(etat: State<EtatApp>) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let r = conn.query_row(
        "SELECT id, nom FROM depot WHERE est_defaut = 1 LIMIT 1",
        [], |row| Ok(serde_json::json!({
            "id": row.get::<_,String>(0)?, "nom": row.get::<_,String>(1)?
        }))
    ).map_err(|e| e.to_string())?;
    Ok(r)
}

#[tauri::command]
pub fn creer_article_rapide(
    etat: State<EtatApp>,
    nom: String,
    unite_base: String,
    prix_reference: i64,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let now = maintenant_iso();
    let art_id = uuid::Uuid::new_v4().to_string();
    let auteur = id_utilisateur_courant_pub(&conn);

    conn.execute(
        "INSERT INTO article
         (id, nom, unite_base, gere_en_stock, attributs, actif,
          cree_le, modifie_le, cree_par, modifie_par, origine)
         VALUES (?1,?2,?3,1,'{}',1,?4,?5,?6,?7,'app')",
        rusqlite::params![art_id, nom, unite_base, now, now, auteur, auteur],
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
    ).map_err(|e| e.to_string())?;

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

// =====================================================================
//  VENTES
// =====================================================================

#[derive(serde::Deserialize)]
pub struct ParamsLigneInput {
    pub article_id: String,
    pub unite_vente_id: String,
    pub depot_source_id: String,
    pub source_approvisionnement: String,
    pub quantite: f64,
    pub facteur: f64,
    pub prix_reference: i64,
    pub prix_pratique: i64,
    pub taux_tva: Option<f64>,
}

#[tauri::command]
pub fn creer_vente(
    etat: State<EtatApp>,
    client_id: String,
    depot_id: String,
    mode_reglement: String,
    lignes: Vec<ParamsLigneInput>,
    utilisateur_role: Option<String>,
    // montant_paye  : encaisse immediatement (comptant integral ou acompte)
    // mode_paiement : especes | orange_money | moov_money | cheque
    // avoir_montant : montant d'avoir client a consommer sur cette vente
    montant_paye: Option<i64>,
    mode_paiement: Option<String>,
    avoir_montant: Option<i64>,
) -> Result<serde_json::Value, String> {
    let mut conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let role = utilisateur_role.as_deref().unwrap_or("employe");
    let auteur_id = id_utilisateur_par_role(&conn, role);
    let now = maintenant_iso();
    let vente_id = uuid::Uuid::new_v4().to_string();

    let tx = conn.transaction().map_err(|e| e.to_string())?;

    tx.execute(
        "INSERT INTO vente
         (id, client_id, depot_id, mode_reglement, auteur_id, statut,
          date_vente, cree_le, modifie_le, cree_par, modifie_par, origine)
         VALUES (?1,?2,?3,?4,?5,'creance_ouverte',?6,?7,?8,?9,?10,'app')",
        rusqlite::params![vente_id, client_id, depot_id, mode_reglement,
                          auteur_id, now, now, now, auteur_id, auteur_id],
    ).map_err(|e| e.to_string())?;

    let mut total: i64 = 0;

    for ligne in &lignes {
        let ligne_id = uuid::Uuid::new_v4().to_string();
        let montant = (ligne.prix_pratique as f64 * ligne.quantite).round() as i64;
        let taux_tva = ligne.taux_tva.unwrap_or(0.0);
        // TVA incluse dans le prix TTC — extraction : montant × taux / (1 + taux)
        let montant_tva = if taux_tva > 0.0 {
            (montant as f64 * taux_tva / (1.0 + taux_tva)).round() as i64
        } else { 0 };
        total += montant;

        tx.execute(
            "INSERT INTO ligne_vente
             (id, vente_id, article_id, unite_vente_id, depot_source_id,
              source_approvisionnement, quantite, prix_reference, prix_pratique,
              taux_tva, montant_tva, cree_le, origine)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,'app')",
            rusqlite::params![
                ligne_id, vente_id, ligne.article_id, ligne.unite_vente_id,
                ligne.depot_source_id, ligne.source_approvisionnement,
                ligne.quantite, ligne.prix_reference, ligne.prix_pratique,
                taux_tva, montant_tva, now
            ],
        ).map_err(|e| e.to_string())?;

        // Décrément stock
        let qte_base = ligne.quantite * ligne.facteur;
        tx.execute(
            "INSERT INTO stock_depot (id, article_id, depot_id, quantite)
             VALUES (?1,?2,?3,0 - ?4)
             ON CONFLICT(article_id, depot_id)
             DO UPDATE SET quantite = quantite - ?4",
            rusqlite::params![
                uuid::Uuid::new_v4().to_string(),
                ligne.article_id, ligne.depot_source_id, qte_base
            ],
        ).map_err(|e| e.to_string())?;

        tx.execute(
            "INSERT INTO mouvement_stock
             (id, article_id, depot_id, type_mouvement, quantite_delta,
              operation_id, auteur_id, date_mouvement, cree_le, cree_par, origine)
             VALUES (?1,?2,?3,'vente',?4,?5,?6,?7,?8,?9,'app')",
            rusqlite::params![
                uuid::Uuid::new_v4().to_string(),
                ligne.article_id, ligne.depot_source_id, -qte_base,
                vente_id, auteur_id, now, now, auteur_id
            ],
        ).map_err(|e| e.to_string())?;

        // §7 — Tracer la remise
        if ligne.prix_pratique < ligne.prix_reference {
            tx.execute(
                "INSERT INTO journal
                 (id, type_evenement, entite_type, entite_id, auteur_id,
                  ancien_valeur, nouveau_valeur, origine, date_evenement)
                 VALUES (?1,'remise_accordee','ligne_vente',?2,?3,?4,?5,'app',?6)",
                rusqlite::params![
                    uuid::Uuid::new_v4().to_string(), ligne_id, auteur_id,
                    ligne.prix_reference.to_string(),
                    ligne.prix_pratique.to_string(), now
                ],
            ).ok();
        }
    }

    // =================================================================
    //  Reglement — DANS la transaction.
    //  Auparavant le front enchainait creer_vente / appliquer_avoir_vente /
    //  enregistrer_paiement en appels separes : une coupure au milieu
    //  laissait le stock sorti et la vente en creance ouverte alors que
    //  le client avait paye.
    // =================================================================
    let mut total_regle: i64 = 0;

    // ---- Avoirs (du plus ancien au plus recent) ----
    let avoir_demande = avoir_montant.unwrap_or(0).min(total);
    if avoir_demande > 0 {
        let avoirs: Vec<(String, i64)> = {
            let mut st = tx.prepare(
                "SELECT id, montant FROM avoir
                 WHERE client_id = ?1 AND statut = 'ouvert'
                 ORDER BY cree_le ASC"
            ).map_err(|e| e.to_string())?;
            let v = st.query_map(rusqlite::params![client_id], |r| {
                Ok((r.get::<_,String>(0)?, r.get::<_,i64>(1)?))
            }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
            v
        };

        let mut reste = avoir_demande;
        for (avoir_id, montant_avoir) in avoirs {
            if reste <= 0 { break; }
            tx.execute(
                "UPDATE avoir SET statut = 'consomme', vente_utilisation_id = ?1
                 WHERE id = ?2",
                rusqlite::params![vente_id, avoir_id],
            ).map_err(|e| e.to_string())?;

            if montant_avoir > reste {
                // Solde non consomme : nouvel avoir ouvert.
                tx.execute(
                    "INSERT INTO avoir (id, client_id, montant, statut, cree_le, origine)
                     VALUES (?1,?2,?3,'ouvert',?4,'avoir_solde')",
                    rusqlite::params![
                        uuid::Uuid::new_v4().to_string(),
                        client_id, montant_avoir - reste, now
                    ],
                ).map_err(|e| e.to_string())?;
                total_regle += reste;
                reste = 0;
            } else {
                total_regle += montant_avoir;
                reste -= montant_avoir;
            }
        }

        if total_regle > 0 {
            tx.execute(
                "INSERT INTO paiement
                 (id, vente_id, montant, mode, date_paiement, auteur_id,
                  cree_le, cree_par, origine)
                 VALUES (?1,?2,?3,'avoir',?4,?5,?6,?7,'app')",
                rusqlite::params![
                    uuid::Uuid::new_v4().to_string(),
                    vente_id, total_regle, now, auteur_id, now, auteur_id
                ],
            ).map_err(|e| e.to_string())?;
        }
    }

    // ---- Encaissement ----
    let mode_p = mode_paiement.as_deref().unwrap_or("especes");
    let a_encaisser = montant_paye.unwrap_or(0).min(total - total_regle).max(0);
    if a_encaisser > 0 {
        tx.execute(
            "INSERT INTO paiement
             (id, vente_id, montant, mode, date_paiement, auteur_id,
              cree_le, cree_par, origine)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,'app')",
            rusqlite::params![
                uuid::Uuid::new_v4().to_string(),
                vente_id, a_encaisser, mode_p, now, auteur_id, now, auteur_id
            ],
        ).map_err(|e| e.to_string())?;
        total_regle += a_encaisser;

        // Caisse — uniquement les reglements reels, jamais les avoirs.
        let session: Option<String> = tx.query_row(
            "SELECT id FROM session_caisse WHERE statut = 'ouverte' LIMIT 1",
            [], |r| r.get(0),
        ).ok();
        if let Some(sid) = session {
            tx.execute(
                "INSERT INTO mouvement_caisse
                 (id, session_id, sens, moyen, montant, motif,
                  operation_id, date_mouvement, cree_le, cree_par, origine)
                 VALUES (?1,?2,'entree',?3,?4,'vente',?5,?6,?7,?8,'app')",
                rusqlite::params![
                    uuid::Uuid::new_v4().to_string(),
                    sid, mode_p, a_encaisser, vente_id, now, now, auteur_id
                ],
            ).map_err(|e| e.to_string())?;
        }
    }

    // ---- Statut final ----
    let statut = if total_regle >= total { "payee" }
                 else if total_regle > 0  { "partiellement_payee" }
                 else                     { "creance_ouverte" };
    tx.execute(
        "UPDATE vente SET statut = ?1, modifie_le = ?2 WHERE id = ?3",
        rusqlite::params![statut, now, vente_id],
    ).map_err(|e| e.to_string())?;

    // La table `facture` legacy n'est PLUS alimentee : piece_commerciale
    // est le referentiel unique des documents. La facture POS est creee
    // juste apres par creer_facture_depuis_vente (numero FAC-).
    // Avant, chaque vente portait deux numeros : GESCOM- et FAC-.

    // Journal vente
    tx.execute(
        "INSERT INTO journal
         (id, type_evenement, entite_type, entite_id, auteur_id,
          nouveau_valeur, origine, date_evenement)
         VALUES (?1,'vente_creee','vente',?2,?3,?4,'app',?5)",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(), vente_id, auteur_id,
            format!(r#"{{"total":{},"regle":{}}}"#, total, total_regle), now
        ],
    ).ok();

    tx.commit().map_err(|e| e.to_string())?;

    Ok(serde_json::json!({
        "vente_id":       vente_id,
        "total":          total,
        "total_regle":    total_regle,
        "reste":          total - total_regle,
        "statut":         statut,
    }))
}

// =====================================================================
//  PAIEMENTS
// =====================================================================

#[tauri::command]
pub fn enregistrer_paiement(
    etat: State<EtatApp>,
    vente_id: String,
    montant: i64,
    mode: String,
    utilisateur_role: Option<String>,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let role = utilisateur_role.as_deref().unwrap_or("employe");
    let auteur_id = id_utilisateur_par_role(&conn, role);
    let now = maintenant_iso();

    // Insérer le paiement
    conn.execute(
        "INSERT INTO paiement
         (id, vente_id, montant, mode, date_paiement, auteur_id, cree_le, cree_par, origine)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,'app')",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(), vente_id, montant,
            mode, now, auteur_id, now, auteur_id
        ],
    ).map_err(|e| e.to_string())?;

    // Mettre à jour le statut de la vente
    let total: i64 = conn.query_row(
        "SELECT CAST(COALESCE(SUM(prix_pratique * quantite), 0) AS INTEGER)
         FROM ligne_vente WHERE vente_id = ?1",
        rusqlite::params![vente_id], |r| r.get(0),
    ).unwrap_or(0);

    let total_paye: i64 = conn.query_row(
        "SELECT CAST(COALESCE(SUM(montant), 0) AS INTEGER)
         FROM paiement WHERE vente_id = ?1",
        rusqlite::params![vente_id], |r| r.get(0),
    ).unwrap_or(0);

    let statut = if total_paye >= total { "payee" }
        else if total_paye > 0 { "partiellement_payee" }
        else { "creance_ouverte" };

    conn.execute(
        "UPDATE vente SET statut = ?1, modifie_le = ?2 WHERE id = ?3",
        rusqlite::params![statut, now, vente_id],
    ).map_err(|e| e.to_string())?;

    // Alimenter la caisse si session ouverte et mode espèces/mobile
    if mode != "avoir" {
        let session_id: Option<String> = conn.query_row(
            "SELECT id FROM session_caisse WHERE statut = 'ouverte' LIMIT 1",
            [], |r| r.get(0),
        ).ok();

        if let Some(sid) = session_id {
            conn.execute(
                "INSERT INTO mouvement_caisse
                 (id, session_id, sens, moyen, montant, motif,
                  operation_id, date_mouvement, cree_le, cree_par, origine)
                 VALUES (?1,?2,'entree',?3,?4,'vente',?5,?6,?7,?8,'app')",
                rusqlite::params![
                    uuid::Uuid::new_v4().to_string(), sid, mode, montant,
                    vente_id, now, now, auteur_id
                ],
            ).ok();
        }
    }

    Ok(())
}

// =====================================================================
//  CRÉANCES CLIENTS
// =====================================================================

#[tauri::command]
pub fn lire_clients_avec_creances(
    etat: State<EtatApp>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(
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
         ORDER BY total_creances DESC, c.nom ASC"
    ).map_err(|e| e.to_string())?;

    let x = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "id": row.get::<_,String>(0)?,
            "code": row.get::<_,String>(1)?,
            "nom": row.get::<_,String>(2)?,
            "telephone": row.get::<_,Option<String>>(3)?,
            "total_creances": row.get::<_,i64>(4)?,
            "nb_ventes": row.get::<_,i64>(5)?,
        }))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
    Ok(x)
}