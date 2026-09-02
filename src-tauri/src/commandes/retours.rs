//! Commandes Tauri pour les retours et avoirs.
//!
//! Trois modes (§6 module-facturation-stock.md) :
//!   - remboursement  : sortie de caisse, stock remonte
//!   - echange        : stock retourné remonte, stock remplacement descend,
//!                      reliquat → remboursement ou avoir si positif,
//!                      complément client si négatif
//!   - avoir_conserve : avoir créé, stock remonte, pas de caisse

use tauri::State;
use crate::commandes::ventes::EtatApp;
use crate::utils::maintenant_iso;

// =====================================================================
//  LIRE LES VENTES RÉCENTES
// =====================================================================

#[tauri::command]
pub fn lire_ventes_recentes(
    etat: State<EtatApp>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let mut stmt = conn.prepare(
        "SELECT v.id, v.date_vente, v.statut, v.client_id,
                c.nom, c.code,
                f.numero
         FROM vente v
         JOIN client c ON c.id = v.client_id
         LEFT JOIN piece_commerciale f ON f.id = v.piece_id
         ORDER BY v.date_vente DESC
         LIMIT 50"
    ).map_err(|e| e.to_string())?;

    let ventes_base: Vec<(String, String, String, String, String, String, Option<String>)> = {
        let x = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, Option<String>>(6)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
        x
    };

    let mut ventes = Vec::new();

    for (id, date_vente, statut, client_id, client_nom, client_code, numero_facture) in ventes_base {
        let mut stmt_l = conn.prepare(
            "SELECT lv.id, lv.article_id, a.nom, lv.unite_vente_id,
                    u.libelle, lv.depot_source_id,
                    lv.quantite, lv.prix_pratique,
                    CAST(lv.prix_pratique * lv.quantite AS INTEGER) as montant
             FROM ligne_vente lv
             JOIN article a ON a.id = lv.article_id
             JOIN unite_vente u ON u.id = lv.unite_vente_id
             WHERE lv.vente_id = ?1"
        ).map_err(|e| e.to_string())?;

        let lignes: Vec<serde_json::Value> = {
            let x = stmt_l.query_map(rusqlite::params![id], |row| {
                Ok(serde_json::json!({
                    "id":              row.get::<_, String>(0)?,
                    "article_id":      row.get::<_, String>(1)?,
                    "article_nom":     row.get::<_, String>(2)?,
                    "unite_vente_id":  row.get::<_, String>(3)?,
                    "unite_libelle":   row.get::<_, String>(4)?,
                    "depot_source_id": row.get::<_, String>(5)?,
                    "quantite":        row.get::<_, f64>(6)?,
                    "prix_pratique":   row.get::<_, i64>(7)?,
                    "montant":         row.get::<_, i64>(8)?,
                }))
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();
            x
        };

        let total: i64 = lignes.iter().filter_map(|l| l["montant"].as_i64()).sum();

        ventes.push(serde_json::json!({
            "id":             id,
            "date_vente":     date_vente,
            "statut":         statut,
            "client_id":      client_id,
            "client_nom":     client_nom,
            "client_code":    client_code,
            "numero_facture": numero_facture,
            "total":          total,
            "lignes":         lignes,
        }));
    }

    Ok(ventes)
}

// =====================================================================
//  ENREGISTRER UN RETOUR
// =====================================================================

#[tauri::command]
pub fn enregistrer_retour(
    etat: State<EtatApp>,
    vente_id: String,
    ligne_vente_id: String,
    quantite: f64,
    mode_resolution: String,
    // Remboursement / complément
    mode_encaissement: Option<String>,
    // Échange
    article_remplacement_id: Option<String>,
    unite_remplacement_id: Option<String>,
    quantite_remplacement: Option<f64>,
    // Reliquat positif d'échange
    mode_reliquat_positif: Option<String>,    // "remboursement" / "avoir"
    mode_encaissement_reliquat: Option<String>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let utilisateur_id = crate::commandes::ventes::id_utilisateur_courant_pub(&conn);
    let maintenant = maintenant_iso();
    let retour_id = uuid::Uuid::new_v4().to_string();

    // Récupérer les infos de la ligne de vente.
    let (article_id, unite_vente_id, depot_source_id, facteur, prix_pratique, client_id):
        (String, String, String, f64, i64, String) = conn.query_row(
        "SELECT lv.article_id, lv.unite_vente_id, lv.depot_source_id,
                u.facteur, lv.prix_pratique, v.client_id
         FROM ligne_vente lv
         JOIN unite_vente u ON u.id = lv.unite_vente_id
         JOIN vente v ON v.id = lv.vente_id
         WHERE lv.id = ?1",
        rusqlite::params![ligne_vente_id],
        |row| Ok((
            row.get(0)?, row.get(1)?, row.get(2)?,
            row.get(3)?, row.get(4)?, row.get(5)?,
        )),
    ).map_err(|e| e.to_string())?;

    // =================================================================
    //  GARDE 1 — on ne retourne pas plus qu'on n'a vendu
    // =================================================================
    let qte_vendue: f64 = conn.query_row(
        "SELECT quantite FROM ligne_vente WHERE id = ?1",
        rusqlite::params![ligne_vente_id], |r| r.get(0),
    ).map_err(|e| e.to_string())?;

    // `retour` ne porte pas ligne_vente_id : on agrege par vente+article.
    let deja_retourne: f64 = conn.query_row(
        "SELECT COALESCE(SUM(quantite), 0) FROM retour
         WHERE vente_id = ?1 AND article_id = ?2",
        rusqlite::params![vente_id, article_id], |r| r.get(0),
    ).unwrap_or(0.0);

    let retournable = qte_vendue - deja_retourne;
    if quantite <= 0.0 {
        return Err("La quantité retournée doit être positive".to_string());
    }
    if quantite > retournable + 1e-9 {
        return Err(format!(
            "Quantité trop élevée : {} vendue(s), {} déjà retournée(s), \
             il reste {} retournable(s).",
            qte_vendue, deja_retourne, retournable.max(0.0)
        ));
    }

    let montant_credit: i64 = (prix_pratique as f64 * quantite).round() as i64;
    let quantite_base = quantite * facteur;

    // =================================================================
    //  GARDE 2 — on ne rend que ce que le client a réellement versé
    // =================================================================
    //
    //  Ordre comptable : le retour éteint d'abord la DETTE, puis le
    //  solde éventuel revient au client.
    //
    //  Sans cela, une vente à crédit de 10 000 F jamais réglée dont la
    //  marchandise revient sortait 10 000 F de la caisse ET laissait la
    //  créance ouverte — l'argent partait deux fois.
    let total_vente: i64 = conn.query_row(
        "SELECT CAST(COALESCE(SUM(prix_pratique * quantite), 0) AS INTEGER)
         FROM ligne_vente WHERE vente_id = ?1",
        rusqlite::params![vente_id], |r| r.get(0),
    ).unwrap_or(0);

    let deja_paye: i64 = conn.query_row(
        "SELECT CAST(COALESCE(SUM(montant), 0) AS INTEGER)
         FROM paiement WHERE vente_id = ?1",
        rusqlite::params![vente_id], |r| r.get(0),
    ).unwrap_or(0);

    // Ce qui reste dû AVANT ce retour.
    let reste_du = (total_vente - deja_paye).max(0);

    // Le retour s'impute d'abord sur la dette (sauf echange).
    let part_creance = if mode_resolution == "echange" {
        0
    } else {
        montant_credit.min(reste_du)
    };
    // Ce qui depasse la dette a ete paye : il revient au client.
    //
    // SECONDE BORNE — defense en profondeur.
    // La garde sur la quantite empeche normalement montant_credit de
    // depasser le total de la vente. Mais elle est le seul rempart : si
    // un prix a change apres la vente, ou si des retours successifs se
    // cumulent au-dela du vendu, on rendrait plus que ce que le client
    // a verse. On plafonne donc explicitement au versement net.
    let rembourse_deja: i64 = conn.query_row(
        "SELECT CAST(COALESCE(SUM(mc.montant), 0) AS INTEGER)
         FROM mouvement_caisse mc
         WHERE mc.motif IN ('remboursement','remboursement_reliquat')
           AND mc.operation_id IN (SELECT id FROM retour WHERE vente_id = ?1)",
        rusqlite::params![vente_id], |r| r.get(0),
    ).unwrap_or(0);

    // Versements REELS du client : on exclut les paiements de mode
    // 'retour' (ecritures internes) et 'avoir' (pas d'argent recu).
    let verse_reel: i64 = conn.query_row(
        "SELECT CAST(COALESCE(SUM(montant), 0) AS INTEGER)
         FROM paiement
         WHERE vente_id = ?1 AND mode NOT IN ('retour', 'avoir')",
        rusqlite::params![vente_id], |r| r.get(0),
    ).unwrap_or(0);

    let plafond_client = (verse_reel - rembourse_deja).max(0);
    let part_client_brute = montant_credit - part_creance;
    let part_client = part_client_brute.min(plafond_client);

    // Ce qui ne peut etre ni impute ni rendu : le signaler plutot que
    // de le faire disparaitre en silence.
    let non_attribue = part_client_brute - part_client;
    if non_attribue > 0 {
        eprintln!(
            "[gescom] retour {} : {} F non attribuables \
             (verse {}, deja rembourse {})",
            retour_id, non_attribue, verse_reel, rembourse_deja
        );
    }

    // 1. Remonter le stock de l'article retourné — TOUJOURS.
    conn.execute(
        // ON CONFLICT et non UPDATE nu : si le couple article/depot
        // n'existe pas encore, un UPDATE ne fait rien et le retour est
        // perdu en silence.
        "INSERT INTO stock_depot (id, article_id, depot_id, quantite)
         VALUES (?4, ?2, ?3, ?1)
         ON CONFLICT(article_id, depot_id)
         DO UPDATE SET quantite = quantite + ?1",
        rusqlite::params![quantite_base, article_id, depot_source_id,
                          uuid::Uuid::new_v4().to_string()],
    ).map_err(|e| e.to_string())?;

    // Mouvement de stock — entrée.
    conn.execute(
        "INSERT INTO mouvement_stock
         (id, article_id, depot_id, type_mouvement, quantite_delta,
          operation_id, auteur_id, date_mouvement, cree_le, cree_par, origine)
         VALUES (?1, ?2, ?3, 'retour', ?4, ?5, ?6, ?7, ?8, ?9, 'app')",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(),
            article_id, depot_source_id, quantite_base,
            retour_id, utilisateur_id,
            maintenant, maintenant, utilisateur_id
        ],
    ).map_err(|e| e.to_string())?;

    // 2. Insérer le retour.
    conn.execute(
        "INSERT INTO retour
         (id, vente_id, article_id, unite_vente_id, quantite,
          depot_reintegration_id, mode_resolution, montant_credit,
          reliquat, reliquat_resolution, auteur_id, date_retour,
          cree_le, cree_par, origine)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,0,?9,?10,?11,?12,?13,'app')",
        rusqlite::params![
            retour_id, vente_id, article_id, unite_vente_id, quantite,
            depot_source_id, mode_resolution, montant_credit,
            mode_encaissement.as_deref().unwrap_or("aucun"),
            utilisateur_id, maintenant, maintenant, utilisateur_id
        ],
    ).map_err(|e| e.to_string())?;

    // 2 bis. Imputer la part qui éteint la dette.
    //
    // Enregistré comme un paiement de mode 'retour' : le reste dû
    // diminue mécaniquement, puisqu'il vaut total − SUM(paiements).
    // Aucun argent ne bouge, la caisse n'est pas touchée.
    // Pas pour l'echange : la marchandise part vers un autre article,
    // le credit finance le remplacement et non la dette.
    if part_creance > 0 && mode_resolution != "echange" {
        conn.execute(
            "INSERT INTO paiement
             (id, vente_id, montant, mode, date_paiement,
              auteur_id, cree_le, cree_par, origine)
             VALUES (?1,?2,?3,'retour',?4,?5,?6,?7,'retour')",
            rusqlite::params![
                uuid::Uuid::new_v4().to_string(),
                vente_id, part_creance, maintenant,
                utilisateur_id, maintenant, utilisateur_id
            ],
        ).map_err(|e| e.to_string())?;

        // Recalculer le statut via le coeur testé.
        let paye_apres = deja_paye + part_creance;
        let statut = match crate::coeur::calcul::statut_vente(total_vente, paye_apres) {
            crate::coeur::calcul::StatutVente::Payee => "payee",
            crate::coeur::calcul::StatutVente::PartiellementPayee => "partiellement_payee",
            crate::coeur::calcul::StatutVente::CreanceOuverte => "creance_ouverte",
        };
        conn.execute(
            "UPDATE vente SET statut = ?1, modifie_le = ?2 WHERE id = ?3",
            rusqlite::params![statut, maintenant, vente_id],
        ).ok();
    }

    // D40 — un avoir au comptant atterrit dans le pot commun
    // `client0000`, que le POS refuse de lire. On rend l'argent ou on
    // remplace, tout de suite.
    let client_generique = crate::utils::est_client_generique(&conn, &client_id);
    if client_generique && mode_resolution == "avoir_conserve" {
        return Err("Pas d'avoir pour un client comptant. \
                    Choisir remboursement ou échange.".to_string());
    }

    // 3. Le solde éventuel — uniquement ce que le client a versé.
    let mut numero_avoir: Option<String> = None;
    match mode_resolution.as_str() {

        "remboursement" => {
            if part_client > 0 {
                let mode = mode_encaissement.as_deref().unwrap_or("especes");
                enregistrer_sortie_caisse(&conn, part_client, mode, &retour_id,
                    "remboursement", &utilisateur_id, &maintenant)?;
            }
            // part_client == 0 : le retour a servi a eteindre la dette,
            // rien a rendre. Ce n'est pas une erreur.
        }

        "avoir_conserve" if part_client > 0 => {
            numero_avoir = Some(creer_avoir_client(
                &conn, &client_id, &retour_id, part_client, &maintenant,
                &article_id, &unite_vente_id, quantite, &utilisateur_id,
            )?);
        }

        "echange" => {
            // Décrémenter le stock de l'article de remplacement.
            if let (Some(art_remp_id), Some(unite_remp_id), Some(qte_remp)) = (
                &article_remplacement_id,
                &unite_remplacement_id,
                quantite_remplacement,
            ) {
                // Deux lectures distinctes : un `JOIN depot ON
                // est_defaut = 1` sans condition de jointure renvoyait
                // zero ligne quand aucun depot n'est marque par defaut
                // -> QueryReturnedNoRows en pleine transaction.
                let (facteur_remp, prix_remp): (f64, i64) = conn.query_row(
                    "SELECT u.facteur, u.prix_reference
                     FROM unite_vente u WHERE u.id = ?1",
                    rusqlite::params![unite_remp_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                ).map_err(|_| "Unité de remplacement introuvable".to_string())?;

                // Le remplacement sort du depot d'origine de la vente,
                // pas d'un depot par defaut arbitraire : la marchandise
                // rendue y est reintegree juste au-dessus.
                let depot_defaut: String = conn.query_row(
                    "SELECT id FROM depot WHERE est_defaut = 1 AND actif = 1 LIMIT 1",
                    [], |r| r.get(0),
                ).or_else(|_| conn.query_row(
                    "SELECT id FROM depot WHERE id = ?1 AND actif = 1",
                    rusqlite::params![depot_source_id], |r| r.get(0),
                )).map_err(|_| {
                    "Aucun dépôt par défaut actif. Désigner un dépôt \
                     par défaut dans Paramètres → Dépôts.".to_string()
                })?;

                let qte_base_remp = qte_remp * facteur_remp;
                let montant_remplacement: i64 =
                    (prix_remp as f64 * qte_remp).round() as i64;
                let reliquat = montant_credit - montant_remplacement;

                // Décrémenter le stock de l'article de remplacement.
                conn.execute(
                    "INSERT INTO stock_depot (id, article_id, depot_id, quantite)
                     VALUES (?4, ?2, ?3, 0 - ?1)
                     ON CONFLICT(article_id, depot_id)
                     DO UPDATE SET quantite = quantite - ?1",
                    rusqlite::params![qte_base_remp, art_remp_id, depot_defaut,
                                      uuid::Uuid::new_v4().to_string()],
                ).map_err(|e| e.to_string())?;

                // Mouvement de stock — sortie remplacement.
                conn.execute(
                    "INSERT INTO mouvement_stock
                     (id, article_id, depot_id, type_mouvement, quantite_delta,
                      operation_id, auteur_id, date_mouvement, cree_le, cree_par, origine)
                     VALUES (?1, ?2, ?3, 'echange', ?4, ?5, ?6, ?7, ?8, ?9, 'app')",
                    rusqlite::params![
                        uuid::Uuid::new_v4().to_string(),
                        art_remp_id, depot_defaut, -qte_base_remp,
                        retour_id, utilisateur_id,
                        maintenant, maintenant, utilisateur_id
                    ],
                ).map_err(|e| e.to_string())?;

                // Gérer le reliquat.
                if reliquat > 0 {
                    // Client reçoit de l'argent ou un avoir.
                    match mode_reliquat_positif.as_deref() {
                        // `client_generique` D'ABORD : la branche par
                        // defaut ci-dessous est l'avoir, donc un
                        // mode_reliquat_positif absent creerait un avoir
                        // au comptant sans que personne l'ait demande.
                        Some("remboursement") | None if client_generique => {
                            let mode = mode_encaissement_reliquat
                                .as_deref().unwrap_or("especes");
                            enregistrer_sortie_caisse(&conn, reliquat, mode,
                                &retour_id, "remboursement_reliquat",
                                &utilisateur_id, &maintenant)?;
                        }
                        _ if client_generique => {
                            return Err("Pas d'avoir pour un client comptant. \
                                        Le reliquat doit être remboursé.".to_string());
                        }
                        Some("remboursement") => {
                            let mode = mode_encaissement_reliquat
                                .as_deref().unwrap_or("especes");
                            enregistrer_sortie_caisse(&conn, reliquat, mode,
                                &retour_id, "remboursement_reliquat",
                                &utilisateur_id, &maintenant)?;
                        }
                        _ => {
                            // Avoir par défaut.
                            numero_avoir = Some(creer_avoir_client(
                                &conn, &client_id, &retour_id, reliquat,
                                &maintenant, &article_id, &unite_vente_id,
                                quantite, &utilisateur_id,
                            )?);
                        }
                    }
                } else if reliquat < 0 {
                    // Client paie la différence — entrée de caisse.
                    let mode = mode_encaissement.as_deref().unwrap_or("especes");
                    let session_id: Option<String> = conn.query_row(
                        "SELECT id FROM session_caisse WHERE statut = 'ouverte' LIMIT 1",
                        [], |row| row.get(0),
                    ).ok();

                    if let Some(sid) = session_id {
                        conn.execute(
                            "INSERT INTO mouvement_caisse
                             (id, session_id, sens, moyen, montant, motif,
                              operation_id, date_mouvement, cree_le, cree_par, origine)
                             VALUES (?1, ?2, 'entree', ?3, ?4, 'complement_echange',
                                     ?5, ?6, ?7, ?8, 'app')",
                            rusqlite::params![
                                uuid::Uuid::new_v4().to_string(),
                                sid, mode, (-reliquat), retour_id,
                                maintenant, maintenant, utilisateur_id
                            ],
                        ).map_err(|e| e.to_string())?;
                    }
                }
                // reliquat == 0 → soldé exactement, rien à faire.
            }
        }

        _ => {}
    }

    // 4. Journal.
    conn.execute(
        "INSERT INTO journal
         (id, type_evenement, entite_type, entite_id, auteur_id,
          nouveau_valeur, origine, date_evenement)
         VALUES (?1, 'retour_enregistre', 'retour', ?2, ?3, ?4, 'app', ?5)",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(),
            retour_id, utilisateur_id,
            format!(r#"{{"mode":"{}","credit":{}}}"#, mode_resolution, montant_credit),
            maintenant
        ],
    ).map_err(|e| e.to_string())?;

    Ok(serde_json::json!({
        "retour_id":      retour_id,
        "montant_credit": montant_credit,
        // Repartition : ce qui a eteint la dette, ce qui revient au
        // client. Permet au front d'expliquer le resultat.
        "part_creance":   part_creance,
        "part_client":    part_client,
        // Numero de la piece AVC quand un avoir a ete cree — permet
        // d'ouvrir directement l'impression cote front.
        "numero_avoir":  numero_avoir,
    }))
}

// =====================================================================
//  Fonctions utilitaires internes
// =====================================================================

fn enregistrer_sortie_caisse(
    conn: &rusqlite::Connection,
    montant: i64,
    mode: &str,
    operation_id: &str,
    motif: &str,
    utilisateur_id: &str,
    maintenant: &str,
) -> Result<(), String> {
    let session_id: Option<String> = conn.query_row(
        "SELECT id FROM session_caisse WHERE statut = 'ouverte' LIMIT 1",
        [], |row| row.get(0),
    ).ok();

    if let Some(sid) = session_id {
        conn.execute(
            "INSERT INTO mouvement_caisse
             (id, session_id, sens, moyen, montant, motif,
              operation_id, date_mouvement, cree_le, cree_par, origine)
             VALUES (?1, ?2, 'sortie', ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'app')",
            rusqlite::params![
                uuid::Uuid::new_v4().to_string(),
                sid, mode, montant, motif, operation_id,
                maintenant, maintenant, utilisateur_id
            ],
        ).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Cree l'avoir ET sa piece commerciale AVC-AAAA-NNNNN.
///
/// Un avoir est un engagement financier envers le client : il doit
/// exister un document numerote et imprimable, sinon le client repart
/// avec une promesse non opposable.
fn creer_avoir_client(
    conn: &rusqlite::Connection,
    client_id: &str,
    retour_id: &str,
    montant: i64,
    maintenant: &str,
    // Ligne d'origine, pour que la piece porte l'article concerne.
    article_id: &str,
    unite_vente_id: &str,
    quantite: f64,
    auteur_id: &str,
) -> Result<String, String> {
    let avoir_id = uuid::Uuid::new_v4().to_string();

    conn.execute(
        "INSERT INTO avoir
         (id, client_id, retour_id, montant, statut, cree_le, origine)
         VALUES (?1, ?2, ?3, ?4, 'ouvert', ?5, 'app')",
        rusqlite::params![avoir_id, client_id, retour_id, montant, maintenant],
    ).map_err(|e| e.to_string())?;

    // ---- Piece commerciale AVC ----
    let numero = crate::commandes::pieces::prochain_numero(conn, "avoir_client");
    let piece_id = uuid::Uuid::new_v4().to_string();

    conn.execute(
        "INSERT INTO piece_commerciale
         (id, type_piece, numero, statut, tiers_type, tiers_id,
          auteur_id, date_piece, remise_globale, note,
          cree_le, modifie_le, origine)
         VALUES (?1,'avoir_client',?2,'emis','client',?3,
                 ?4,?5,0.0,?6,?7,?8,'retour')",
        rusqlite::params![
            piece_id, numero, client_id, auteur_id, maintenant,
            format!("Avoir sur retour — {} F", montant),
            maintenant, maintenant
        ],
    ).map_err(|e| e.to_string())?;

    let prix_u = if quantite > 0.0 {
        (montant as f64 / quantite).round() as i64
    } else { montant };

    conn.execute(
        "INSERT INTO ligne_piece
         (id, piece_id, article_id, unite_vente_id, quantite,
          prix_unitaire, remise_pct, remise_montant,
          taux_tva, montant_tva, montant_ht, cree_le)
         VALUES (?1,?2,?3,?4,?5,?6,0.0,0,0.0,0,?7,?8)",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(),
            piece_id, article_id, unite_vente_id,
            quantite, prix_u, montant, maintenant
        ],
    ).map_err(|e| e.to_string())?;

    Ok(numero)
}

// =====================================================================
//  AVOIRS OUVERTS
// =====================================================================

#[tauri::command]
pub fn lire_avoirs_ouverts_tous(
    etat: State<EtatApp>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let mut stmt = conn.prepare(
        "SELECT av.id, c.nom, c.code, av.montant, av.cree_le
         FROM avoir av
         JOIN client c ON c.id = av.client_id
         WHERE av.statut = 'ouvert'
         ORDER BY av.cree_le DESC"
    ).map_err(|e| e.to_string())?;

    let x = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "id":          row.get::<_, String>(0)?,
            "client_nom":  row.get::<_, String>(1)?,
            "client_code": row.get::<_, String>(2)?,
            "montant":     row.get::<_, i64>(3)?,
            "cree_le":     row.get::<_, String>(4)?,
        }))
    })
    .map_err(|e| e.to_string())?
    .filter_map(|r| r.ok())
    .collect();
    Ok(x)
}
