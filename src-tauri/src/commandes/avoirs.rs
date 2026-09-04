//! Commandes Tauri pour les avoirs à la vente et le scanner de code-barres.

use tauri::State;
use crate::commandes::ventes::EtatApp;
use crate::utils::maintenant_iso;

// =====================================================================
//  AVOIRS CLIENT
// =====================================================================

/// Lire les avoirs ouverts d'un client spécifique.
#[tauri::command]
pub fn lire_avoirs_client(
    etat: State<EtatApp>,
    client_id: String,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let mut stmt = conn.prepare(
        "SELECT id, montant, cree_le
         FROM avoir
         WHERE client_id = ?1 AND statut = 'ouvert'
         ORDER BY cree_le ASC"
    ).map_err(|e| e.to_string())?;

    let x = stmt.query_map(rusqlite::params![client_id], |row| {
        Ok(serde_json::json!({
            "id":      row.get::<_, String>(0)?,
            "montant": row.get::<_, i64>(1)?,
            "cree_le": row.get::<_, String>(2)?,
        }))
    })
    .map_err(|e| e.to_string())?
    .filter_map(|r| r.ok())
    .collect();

    Ok(x)
}

/// Total des avoirs ouverts d'un client.
#[tauri::command]
pub fn total_avoirs_client(
    etat: State<EtatApp>,
    client_id: String,
) -> Result<i64, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let total: i64 = conn.query_row(
        "SELECT CAST(COALESCE(SUM(montant), 0) AS INTEGER)
         FROM avoir WHERE client_id = ?1 AND statut = 'ouvert'",
        rusqlite::params![client_id],
        |r| r.get(0),
    ).unwrap_or(0);

    Ok(total)
}

/// Appliquer un avoir à une vente.
/// Consomme les avoirs du plus ancien au plus récent.
/// Retourne le montant d'avoir effectivement appliqué.
#[tauri::command]
pub fn appliquer_avoir_vente(
    etat: State<EtatApp>,
    vente_id: String,
    client_id: String,
    montant_demande: i64,
) -> Result<i64, String> {
    let mut conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let maintenant = maintenant_iso();
    let auteur_id = crate::commandes::ventes::id_utilisateur_courant_pub(&conn);

    // D40 — pot commun `client0000` : sans cette garde, le premier
    // passant venu consommait le credit d'un autre. Troisieme et
    // dernier chemin de consommation d'avoirs.
    if crate::utils::est_client_generique(&conn, &client_id) {
        return Err("Pas d'avoir pour un client comptant.".to_string());
    }

    // Transaction : consommation d'avoirs, paiement et statut forment un
    // tout. Une coupure au milieu laissait un avoir consomme sans
    // paiement en face — le client perdait son credit.
    let conn = conn.transaction().map_err(|e| e.to_string())?;

    // Lire les avoirs ouverts du client (du plus ancien au plus récent)
    let avoirs: Vec<(String, i64, Option<String>)> = {
        let mut stmt = conn.prepare(
            "SELECT id, montant, piece_id FROM avoir
             WHERE client_id = ?1 AND statut = 'ouvert'
             ORDER BY cree_le ASC"
        ).map_err(|e| e.to_string())?;

        let x = stmt.query_map(rusqlite::params![client_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?,
                row.get::<_, Option<String>>(2)?))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
        x
    };

    let mut reste_a_consommer = montant_demande;
    let mut total_applique: i64 = 0;

    for (avoir_id, montant_avoir, piece_avoir) in avoirs {
        if reste_a_consommer <= 0 { break; }

        if montant_avoir <= reste_a_consommer {
            // Consommer l'avoir entièrement
            conn.execute(
                // 'consomme' — meme valeur que creer_vente. 'utilise'
                // etait un troisieme vocabulaire pour le meme etat.
                "UPDATE avoir SET statut = 'consomme', vente_utilisation_id = ?1
                 WHERE id = ?2",
                rusqlite::params![vente_id, avoir_id],
            ).map_err(|e| e.to_string())?;

            total_applique += montant_avoir;
            reste_a_consommer -= montant_avoir;
        } else {
            // Consommer partiellement — diviser l'avoir en deux
            let montant_utilise = reste_a_consommer;
            let montant_restant = montant_avoir - montant_utilise;

            // Marquer l'avoir original comme utilisé
            conn.execute(
                // 'consomme' — meme valeur que creer_vente. 'utilise'
                // etait un troisieme vocabulaire pour le meme etat.
                "UPDATE avoir SET statut = 'consomme', vente_utilisation_id = ?1
                 WHERE id = ?2",
                rusqlite::params![vente_id, avoir_id],
            ).map_err(|e| e.to_string())?;

            // Le solde HERITE du piece_id : l'AVC d'origine continue
            // d'afficher le credit qui lui reste, au lieu de tomber a
            // zero des la premiere consommation partielle (bug #8).
            conn.execute(
                "INSERT INTO avoir
                 (id, client_id, piece_id, montant, statut, cree_le, origine)
                 VALUES (?1, ?2, ?3, ?4, 'ouvert', ?5, 'avoir_solde')",
                rusqlite::params![
                    uuid::Uuid::new_v4().to_string(),
                    client_id, piece_avoir, montant_restant, maintenant
                ],
            ).map_err(|e| e.to_string())?;

            total_applique += montant_utilise;
            reste_a_consommer = 0;
        }
    }

    if total_applique > 0 {
        // Enregistrer le paiement par avoir
        conn.execute(
            "INSERT INTO paiement
             (id, vente_id, montant, mode, date_paiement, auteur_id, cree_le, cree_par, origine)
             VALUES (?1, ?2, ?3, 'avoir', ?4, ?5, ?6, ?7, 'app')",
            rusqlite::params![
                uuid::Uuid::new_v4().to_string(),
                vente_id, total_applique,
                maintenant, auteur_id, maintenant, auteur_id
            ],
        ).map_err(|e| e.to_string())?;

        // Mettre à jour le statut de la vente
        let total_vente: i64 = conn.query_row(
            "SELECT CAST(COALESCE(SUM(prix_pratique * quantite), 0) AS INTEGER)
             FROM ligne_vente WHERE vente_id = ?1",
            rusqlite::params![vente_id], |r| r.get(0),
        ).unwrap_or(0);

        let total_paye: i64 = conn.query_row(
            "SELECT CAST(COALESCE(SUM(montant), 0) AS INTEGER)
             FROM paiement WHERE vente_id = ?1",
            rusqlite::params![vente_id], |r| r.get(0),
        ).unwrap_or(0);

        // Meme regle que creer_vente, via le coeur teste.
        let statut = match crate::coeur::calcul::statut_vente(total_vente, total_paye) {
            crate::coeur::calcul::StatutVente::Payee => "payee",
            crate::coeur::calcul::StatutVente::PartiellementPayee => "partiellement_payee",
            crate::coeur::calcul::StatutVente::CreanceOuverte => "creance_ouverte",
        };

        conn.execute(
            "UPDATE vente SET statut = ?1, modifie_le = ?2 WHERE id = ?3",
            rusqlite::params![statut, maintenant, vente_id],
        ).map_err(|e| e.to_string())?;

        // Journal
        conn.execute(
            "INSERT INTO journal
             (id, type_evenement, entite_type, entite_id, auteur_id,
              nouveau_valeur, origine, date_evenement)
             VALUES (?1, 'avoir_applique', 'vente', ?2, ?3, ?4, 'app', ?5)",
            rusqlite::params![
                uuid::Uuid::new_v4().to_string(),
                vente_id, auteur_id,
                format!(r#"{{"montant_avoir":{}}}"#, total_applique),
                maintenant
            ],
        ).ok();
    }

    conn.commit().map_err(|e| e.to_string())?;
    Ok(total_applique)
}

// =====================================================================
//  SCANNER CODE-BARRES
// =====================================================================

/// Chercher un article par son code-barres.
#[tauri::command]
pub fn chercher_article_par_code_barre(
    etat: State<EtatApp>,
    code_barre: String,
) -> Result<Option<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    // Repli sur n'importe quel depot actif : sans depot par defaut, le
    // scan echouait au lieu de simplement afficher un stock approximatif.
    let depot_id: String = conn.query_row(
        "SELECT id FROM depot WHERE est_defaut = 1 AND actif = 1 LIMIT 1",
        [], |r| r.get(0),
    ).or_else(|_| conn.query_row(
        "SELECT id FROM depot WHERE actif = 1 ORDER BY nom LIMIT 1",
        [], |r| r.get(0),
    )).map_err(|_| "Aucun dépôt actif".to_string())?;

    // L'article d'abord, ses unites ensuite. Un `JOIN unite_vente` avec
    // `LIMIT 1` ne renvoyait QU'UNE unite — la plus petite : le carton
    // disparaissait du POS et l'article partait au detail sans que
    // personne ne le remarque.
    // Le code peut designer l'ARTICLE (unite de base) ou une UNITE
    // precise — le carton porte son propre EAN (D45). On renvoie
    // `unite_scannee_id` pour que le POS preselectionne le bon
    // conditionnement : sans ca, scanner un carton le vendrait au
    // detail, au prix de la piece.
    let base = conn.query_row(
        "SELECT a.id, a.nom, a.unite_base, COALESCE(sd.quantite, 0),
                (SELECT uv.id FROM unite_vente uv
                 WHERE uv.code_barre = ?1 AND uv.actif = 1 LIMIT 1)
         FROM article a
         LEFT JOIN stock_depot sd ON sd.article_id = a.id AND sd.depot_id = ?2
         WHERE (a.code_barre = ?1
                OR a.id = (SELECT uv.article_id FROM unite_vente uv
                           WHERE uv.code_barre = ?1 AND uv.actif = 1 LIMIT 1))
           AND a.actif = 1
         LIMIT 1",
        rusqlite::params![code_barre, depot_id],
        |r| Ok((
            r.get::<_, String>(0)?, r.get::<_, String>(1)?,
            r.get::<_, String>(2)?, r.get::<_, f64>(3)?,
            r.get::<_, Option<String>>(4)?,
        )),
    );

    let (art_id, art_nom, unite_base, stock, unite_scannee) = match base {
        Ok(v) => v,
        Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };

    let unites: Vec<serde_json::Value> = {
        let mut st = conn.prepare(
            "SELECT id, libelle, facteur, prix_reference
             FROM unite_vente
             WHERE article_id = ?1 AND actif = 1
             ORDER BY facteur ASC"
        ).map_err(|e| e.to_string())?;

        let v = st.query_map(rusqlite::params![art_id], |r| {
            Ok(serde_json::json!({
                "id":             r.get::<_, String>(0)?,
                "libelle":        r.get::<_, String>(1)?,
                "facteur":        r.get::<_, f64>(2)?,
                "prix_reference": r.get::<_, i64>(3)?,
            }))
        }).map_err(|e| e.to_string())?
          .filter_map(|r| r.ok())
          .collect();
        v
    };

    if unites.is_empty() {
        return Err(format!("L'article « {} » n'a aucune unité de vente.", art_nom));
    }

    // Le cas « code inconnu » est deja traite plus haut par un retour
    // anticipe : plus de match a faire ici.
    Ok(Some(serde_json::json!({
        "id":         art_id,
        "nom":        art_nom,
        "unite_base": unite_base,
        "stock":      stock,
        "unites":     unites,
        // null = le code designait l'article, pas un conditionnement.
        "unite_scannee_id": unite_scannee,
    })))
}

/// Lire ou sauvegarder la config du scanner.
#[tauri::command]
pub fn lire_config_scanner(
    etat: State<EtatApp>,
) -> Result<bool, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let actif: String = conn.query_row(
        "SELECT valeur FROM config_app WHERE cle = 'scanner_actif'",
        [], |r| r.get(0),
    ).unwrap_or_else(|_| "0".to_string());

    Ok(actif == "1")
}

#[tauri::command]
pub fn sauvegarder_config_scanner(
    etat: State<EtatApp>,
    actif: bool,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    conn.execute(
        "INSERT INTO config_app (cle, valeur) VALUES ('scanner_actif', ?1)
         ON CONFLICT(cle) DO UPDATE SET valeur = ?1",
        rusqlite::params![if actif { "1" } else { "0" }],
    ).map_err(|e| e.to_string())?;

    Ok(())
}

/// Enregistrer le code-barres d'un article depuis les paramètres.
#[tauri::command]
pub fn sauvegarder_code_barre_article(
    etat: State<EtatApp>,
    article_id: String,
    code_barre: String,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let maintenant = maintenant_iso();

    conn.execute(
        "UPDATE article SET code_barre = ?1, modifie_le = ?2 WHERE id = ?3",
        rusqlite::params![code_barre, maintenant, article_id],
    ).map_err(|e| e.to_string())?;

    Ok(())
}

/// Lire tous les articles avec leurs codes-barres pour la page Paramètres.
#[tauri::command]
pub fn lire_articles_avec_codes_barres(
    etat: State<EtatApp>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let mut stmt = conn.prepare(
        "SELECT id, nom, unite_base, code_barre
         FROM article WHERE actif = 1 ORDER BY nom ASC"
    ).map_err(|e| e.to_string())?;

    let x = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "id":         row.get::<_, String>(0)?,
            "nom":        row.get::<_, String>(1)?,
            "unite_base": row.get::<_, String>(2)?,
            "code_barre": row.get::<_, Option<String>>(3)?,
        }))
    })
    .map_err(|e| e.to_string())?
    .filter_map(|r| r.ok())
    .collect();
    Ok(x)
}

// =====================================================================
//  CONVERSION D'UN AVOIR EN REMBOURSEMENT
// =====================================================================

/// Rembourse en espèces un avoir client encore ouvert.
///
/// Le client renonce à son crédit et repart avec l'argent. C'est un
/// DÉCAISSEMENT : le garde-fou serveur est obligatoire, sur le modèle
/// de `regler_creance` et de `regler_dette_fournisseur` (bug #3) — le
/// front peut afficher un montant périmé, ou la commande être appelée
/// directement.
///
/// Le crédit restant est recalculé ici, jamais reçu du client. On
/// consomme du plus ancien au plus récent (D5), et un avoir entamé est
/// scindé pour que le solde reste disponible.
#[tauri::command]
pub fn rembourser_avoir(
    etat: State<EtatApp>,
    // Pièce AVC visée. Le crédit se lit par `avoir.piece_id` (D44).
    piece_id: String,
    montant: i64,
    mode: String,
    utilisateur_role: Option<String>,
) -> Result<serde_json::Value, String> {
    if montant <= 0 {
        return Err("Le montant doit être positif".to_string());
    }

    let mut conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let maintenant = maintenant_iso();
    let role = utilisateur_role.as_deref().unwrap_or("patron");
    let auteur_id = crate::commandes::ventes::id_utilisateur_par_role(&conn, role);

    // Une sortie de caisse sans session ouverte est invisible au
    // rapprochement du soir : on refuse AVANT d'ecrire quoi que ce
    // soit, plutot que de laisser filer l'argent hors du tiroir.
    // Meme helper que partout ailleurs : le front detecte le code
    // CAISSE_FERMEE et propose d'ouvrir la caisse sans quitter l'ecran.
    let session_id = crate::utils::exiger_session_caisse(&conn)?;

    let avoirs: Vec<(String, String, i64)> = {
        let mut st = conn.prepare(
            "SELECT a.id, a.client_id, a.montant FROM avoir a
             WHERE a.piece_id = ?1 AND a.statut = 'ouvert'
             ORDER BY a.cree_le ASC"
        ).map_err(|e| e.to_string())?;
        let v = st.query_map(rusqlite::params![piece_id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?))
        }).map_err(|e| e.to_string())?
          .filter_map(|r| r.ok())
          .collect();
        v
    };

    let credit: i64 = avoirs.iter().map(|(_, _, m)| m).sum();
    if credit <= 0 {
        return Err("Cet avoir est déjà entièrement consommé ou remboursé.".to_string());
    }

    // Plafonne au credit reel : le front peut afficher un montant
    // perime si un autre poste a consomme l'avoir entre-temps.
    let a_rembourser = montant.min(credit);

    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let mut reste = a_rembourser;
    let mut client_id = String::new();

    for (avoir_id, cli, montant_avoir) in &avoirs {
        if reste <= 0 { break; }
        client_id = cli.clone();

        if *montant_avoir <= reste {
            tx.execute(
                // 'rembourse' et non 'consomme' : le credit n'a pas ete
                // depense en marchandise, il est sorti de la caisse.
                // Confondre les deux fausserait toute analyse ulterieure.
                "UPDATE avoir SET statut = 'rembourse' WHERE id = ?1",
                rusqlite::params![avoir_id],
            ).map_err(|e| e.to_string())?;
            reste -= montant_avoir;
        } else {
            tx.execute(
                "UPDATE avoir SET statut = 'rembourse', montant = ?1 WHERE id = ?2",
                rusqlite::params![reste, avoir_id],
            ).map_err(|e| e.to_string())?;
            // Le solde reste disponible, rattache a la meme AVC (D44).
            tx.execute(
                "INSERT INTO avoir
                 (id, client_id, piece_id, montant, statut, cree_le, origine)
                 VALUES (?1, ?2, ?3, ?4, 'ouvert', ?5, 'avoir_solde')",
                rusqlite::params![
                    uuid::Uuid::new_v4().to_string(), cli, piece_id,
                    montant_avoir - reste, maintenant
                ],
            ).map_err(|e| e.to_string())?;
            reste = 0;
        }
    }

    tx.execute(
        "INSERT INTO mouvement_caisse
         (id, session_id, sens, moyen, montant, motif,
          operation_id, date_mouvement, cree_le, cree_par, origine)
         VALUES (?1,?2,'sortie',?3,?4,'remboursement',?5,?6,?7,?8,'app')",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(), session_id, mode,
            a_rembourser, piece_id, maintenant, maintenant, auteur_id
        ],
    ).map_err(|e| e.to_string())?;

    // L'AVC passe a 'paye' quand plus rien n'est ouvert dessus : c'est
    // ce que `lire_toutes_pieces_client` affiche en « reste ».
    let credit_restant: i64 = tx.query_row(
        "SELECT CAST(COALESCE(SUM(montant), 0) AS INTEGER) FROM avoir
         WHERE piece_id = ?1 AND statut = 'ouvert'",
        rusqlite::params![piece_id], |r| r.get(0),
    ).unwrap_or(0);

    if credit_restant <= 0 {
        tx.execute(
            "UPDATE piece_commerciale SET statut = 'paye', modifie_le = ?1
             WHERE id = ?2",
            rusqlite::params![maintenant, piece_id],
        ).ok();
    }

    tx.execute(
        "INSERT INTO journal
         (id, type_evenement, entite_type, entite_id, auteur_id,
          nouveau_valeur, origine, date_evenement)
         VALUES (?1,'avoir_rembourse','piece',?2,?3,?4,'app',?5)",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(), piece_id, auteur_id,
            format!(r#"{{"montant":{},"mode":"{}","client":"{}"}}"#,
                    a_rembourser, mode, client_id),
            maintenant
        ],
    ).ok();

    tx.commit().map_err(|e| e.to_string())?;

    Ok(serde_json::json!({
        "montant_rembourse": a_rembourser,
        "credit_restant":    credit_restant,
        "solde":             credit_restant <= 0,
    }))
}
