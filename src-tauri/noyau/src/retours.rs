//! Commandes Tauri pour les retours et avoirs.
//!
//! Trois modes (§6 module-facturation-stock.md) :
//!   - remboursement  : sortie de caisse, stock remonte
//!   - echange        : stock retourné remonte, stock remplacement descend,
//!                      reliquat → remboursement ou avoir si positif,
//!                      complément client si négatif
//!   - avoir_conserve : avoir créé, stock remonte, pas de caisse

use crate::utils::maintenant_iso;

// =====================================================================
//  LIRE LES VENTES RÉCENTES
// =====================================================================

pub fn lire_ventes_recentes(
    conn: &rusqlite::Connection,
) -> Result<Vec<serde_json::Value>, String> {

    let mut stmt = conn.prepare(
        "SELECT v.id, v.date_vente, v.statut, v.client_id,
                c.nom, c.code,
                f.numero, c.est_generique
         FROM vente v
         JOIN client c ON c.id = v.client_id
         LEFT JOIN piece_commerciale f ON f.id = v.piece_id
         ORDER BY v.date_vente DESC
         LIMIT 50"
    ).map_err(|e| e.to_string())?;

    type VenteLue = (String, String, String, String, String, String, Option<String>, i64);
    let ventes_base: Vec<VenteLue> = {
        let x = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, i64>(7)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
        x
    };

    let mut ventes = Vec::new();

    for (id, date_vente, statut, client_id, client_nom, client_code, numero_facture, generique) in ventes_base {
        let mut stmt_l = conn.prepare(
            "SELECT lv.id, lv.article_id, a.nom, lv.unite_vente_id,
                    u.libelle, lv.depot_source_id,
                    lv.quantite, lv.prix_pratique,
                    CAST(lv.prix_pratique * lv.quantite AS INTEGER) as montant,
                    CAST(COALESCE((SELECT SUM(rt.quantite) FROM retour rt
                                   WHERE rt.ligne_vente_id = lv.id), 0) AS REAL)
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
                    "deja_retourne":   row.get::<_, f64>(9)?,
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
            "client_generique": generique != 0,
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

pub fn enregistrer_retour(
    conn: &mut rusqlite::Connection,
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
    enregistrer_retour_sur(
        conn, vente_id, ligne_vente_id, quantite, mode_resolution,
        mode_encaissement, article_remplacement_id, unite_remplacement_id,
        quantite_remplacement, mode_reliquat_positif,
        mode_encaissement_reliquat,
    )
}

/// Logique de `enregistrer_retour`, sur une connexion quelconque.
///
/// Separee de la commande pour etre jouable sur une base de test :
/// les scenarios de `tests_multi_depot` verifient ce que le SQL fait
/// reellement a la base, ce qu'aucun test de formule ne montre.
pub fn enregistrer_retour_sur(
    conn: &mut rusqlite::Connection,
    vente_id: String,
    ligne_vente_id: String,
    quantite: f64,
    mode_resolution: String,
    mode_encaissement: Option<String>,
    article_remplacement_id: Option<String>,
    unite_remplacement_id: Option<String>,
    quantite_remplacement: Option<f64>,
    mode_reliquat_positif: Option<String>,
    mode_encaissement_reliquat: Option<String>,
) -> Result<serde_json::Value, String> {
    // TOUT le retour dans UNE transaction.
    //
    // Chaque etape ecrivait separement : quand le remboursement etait
    // refuse faute de caisse ouverte, le stock etait deja remonte et la
    // ligne `retour` deja posee. Le client repartait sans son argent, et
    // la garde de quantite bloquait la seconde tentative — le retour
    // etait perdu, l'argent aussi.
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let utilisateur_id = crate::argent::id_utilisateur_courant_pub(&tx);
    let maintenant = maintenant_iso();
    let retour_id = uuid::Uuid::new_v4().to_string();

    // Récupérer les infos de la ligne de vente.
    let (article_id, unite_vente_id, depot_source_id, facteur, prix_pratique, client_id):
        (String, String, String, f64, i64, String) = tx.query_row(
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
    let qte_vendue: f64 = tx.query_row(
        "SELECT quantite FROM ligne_vente WHERE id = ?1",
        rusqlite::params![ligne_vente_id], |r| r.get(0),
    ).map_err(|e| e.to_string())?;

    // Par LIGNE, plus par article : une vente repartie entre deux
    // depots porte deux lignes du meme article, et retourner la
    // premiere rendait la seconde non retournable.
    //
    // Les retours anterieurs a v1.3 n'ont pas de ligne_vente_id : ils
    // restent comptes par article, comme avant. Conservateur — ils
    // peuvent bloquer une ligne soeur, mais ne laissent jamais
    // retourner plus que le vendu.
    let deja_retourne: f64 = tx.query_row(
        "SELECT COALESCE(SUM(quantite), 0) FROM retour
         WHERE vente_id = ?1
           AND (ligne_vente_id = ?2
                OR (ligne_vente_id IS NULL AND article_id = ?3))",
        rusqlite::params![vente_id, ligne_vente_id, article_id], |r| r.get(0),
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
    let total_vente: i64 = tx.query_row(
        "SELECT CAST(COALESCE(SUM(prix_pratique * quantite), 0) AS INTEGER)
         FROM ligne_vente WHERE vente_id = ?1",
        rusqlite::params![vente_id], |r| r.get(0),
    ).unwrap_or(0);

    let deja_paye: i64 = tx.query_row(
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
    let rembourse_deja: i64 = tx.query_row(
        "SELECT CAST(COALESCE(SUM(mc.montant), 0) AS INTEGER)
         FROM mouvement_caisse mc
         WHERE mc.motif IN ('remboursement','remboursement_reliquat')
           AND mc.operation_id IN (SELECT id FROM retour WHERE vente_id = ?1)",
        rusqlite::params![vente_id], |r| r.get(0),
    ).unwrap_or(0);

    // Versements REELS du client : on exclut les paiements de mode
    // 'retour' (ecritures internes) et 'avoir' (pas d'argent recu).
    let verse_reel: i64 = tx.query_row(
        "SELECT CAST(COALESCE(SUM(montant), 0) AS INTEGER)
         FROM paiement
         WHERE vente_id = ?1 AND mode NOT IN ('retour', 'avoir')",
        rusqlite::params![vente_id], |r| r.get(0),
    ).unwrap_or(0);
    // Ce qu'il a paye avec un avoir : pas d'argent recu, mais un credit
    // consomme — il revient en avoir, jamais en especes (D15).
    let verse_avoir: i64 = tx.query_row(
        "SELECT CAST(COALESCE(SUM(montant), 0) AS INTEGER)
         FROM paiement WHERE vente_id = ?1 AND mode = 'avoir'",
        rusqlite::params![vente_id], |r| r.get(0),
    ).unwrap_or(0);
    let avoirs_deja_rendus: i64 = tx.query_row(
        "SELECT CAST(COALESCE(SUM(a.montant), 0) AS INTEGER)
         FROM avoir a
         WHERE a.retour_id IN (SELECT id FROM retour WHERE vente_id = ?1)",
        rusqlite::params![vente_id], |r| r.get(0),
    ).unwrap_or(0);

    let plafond_especes = verse_reel - rembourse_deja;
    let plafond_total = verse_reel + verse_avoir - rembourse_deja - avoirs_deja_rendus;
    let part_client_brute = montant_credit - part_creance;
    let (part_especes, part_avoir, non_attribue) =
        crate::coeur::calcul::repartir_retour(part_client_brute, plafond_especes, plafond_total);
    let part_client = part_especes + part_avoir;

    // Ce qui ne peut etre ni impute ni rendu : une anomalie dans le
    // journal, que le cahier du jour montre — pas une ligne sur stderr.
    if non_attribue > 0 {
        crate::journal::anomalie(&tx, "retour", &retour_id, Some(&utilisateur_id), serde_json::json!({
            "message": format!("Retour : {non_attribue} F ni imputables ni remboursables (versé {verse_reel}, par avoir {verse_avoir}, déjà remboursé {rembourse_deja}, avoirs rendus {avoirs_deja_rendus})"),
            "non_attribue": non_attribue, "verse": verse_reel, "verse_avoir": verse_avoir,
            "rembourse_deja": rembourse_deja, "avoirs_rendus": avoirs_deja_rendus, "vente_id": vente_id,
        }));
    }

    // 1. Remonter le stock de l'article retourné — TOUJOURS.
    // Le stock suit desormais son mouvement : le declencheur
    // `stock_suit_les_mouvements` met le compteur a jour dans la meme
    // transaction. L'ecrire ici le compterait deux fois.

    // Mouvement de stock — entrée.
    tx.execute(
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
    tx.execute(
        "INSERT INTO retour
         (id, vente_id, ligne_vente_id, article_id, unite_vente_id, quantite,
          depot_reintegration_id, mode_resolution, montant_credit,
          reliquat, reliquat_resolution, auteur_id, date_retour,
          cree_le, cree_par, origine)
         VALUES (?1,?2,?14,?3,?4,?5,?6,?7,?8,0,?9,?10,?11,?12,?13,'app')",
        rusqlite::params![
            retour_id, vente_id, article_id, unite_vente_id, quantite,
            depot_source_id, mode_resolution, montant_credit,
            mode_encaissement.as_deref().unwrap_or("aucun"),
            utilisateur_id, maintenant, maintenant, utilisateur_id,
            ligne_vente_id
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
        tx.execute(
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
        tx.execute(
            "UPDATE vente SET statut = ?1, modifie_le = ?2 WHERE id = ?3",
            rusqlite::params![statut, maintenant, vente_id],
        ).ok();
        // D41 : un retour qui laisse 5 F au plus sur la dette la solde,
        // dans cette transaction, avec sa trace.
        crate::creances::solder_residu(&tx, &vente_id, &utilisateur_id, &maintenant)?;
    }

    // D40 — un avoir au comptant atterrit dans le pot commun
    // `client0000`, que le POS refuse de lire. On rend l'argent ou on
    // remplace, tout de suite.
    let client_generique = crate::utils::est_client_generique(&tx, &client_id);
    if client_generique && mode_resolution == "avoir_conserve" {
        return Err("Pas d'avoir pour un client comptant. \
                    Choisir remboursement ou échange.".to_string());
    }

    // 3. Le solde éventuel — uniquement ce que le client a versé.
    let mut numero_avoir: Option<String> = None;
    match mode_resolution.as_str() {

        "remboursement" => {
            if part_especes > 0 {
                let mode = mode_encaissement.as_deref().unwrap_or("especes");
                enregistrer_sortie_caisse(&tx, part_especes, mode, &retour_id,
                    "remboursement", &utilisateur_id, &maintenant)?;
            }
            // La part payee avec un avoir revient en avoir, meme quand
            // le client demande un remboursement : ce credit n'a jamais
            // ete de l'argent (D15). Un client de passage n'a pas
            // d'avoir a consommer (D40) : sa part_avoir est nulle.
            if part_avoir > 0 {
                if client_generique {
                    return Err("Cette vente a été payée avec un avoir : \
                                il ne peut pas être remboursé en espèces.".to_string());
                }
                numero_avoir = Some(creer_avoir_client(
                    &tx, &client_id, &retour_id, part_avoir, &maintenant,
                    &article_id, &unite_vente_id, quantite, &utilisateur_id,
                )?);
            }
            // part_client == 0 : le retour a servi a eteindre la dette,
            // rien a rendre. Ce n'est pas une erreur.
        }

        "avoir_conserve" if part_client > 0 => {
            numero_avoir = Some(creer_avoir_client(
                &tx, &client_id, &retour_id, part_client, &maintenant,
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
                let (facteur_remp, prix_remp): (f64, i64) = tx.query_row(
                    "SELECT u.facteur, u.prix_reference
                     FROM unite_vente u WHERE u.id = ?1",
                    rusqlite::params![unite_remp_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                ).map_err(|_| "Unité de remplacement introuvable".to_string())?;

                // Le remplacement sort du depot ou se tient le client —
                // celui de la vente d'origine, ou l'article rendu vient
                // d'etre reintegre. Prendre le depot par defaut faussait
                // DEUX stocks a la fois en multi-depot : entree ici,
                // sortie ailleurs.
                let depot_remplacement: String = tx.query_row(
                    "SELECT id FROM depot WHERE id = ?1 AND actif = 1",
                    rusqlite::params![depot_source_id], |r| r.get(0),
                ).or_else(|_| tx.query_row(
                    "SELECT id FROM depot WHERE est_defaut = 1 AND actif = 1 LIMIT 1",
                    [], |r| r.get(0),
                )).map_err(|_| {
                    "Aucun magasin actif pour sortir le remplacement. \
                     Vérifier Paramètres → Magasins.".to_string()
                })?;

                let qte_base_remp = qte_remp * facteur_remp;
                let montant_remplacement: i64 =
                    (prix_remp as f64 * qte_remp).round() as i64;
                let reliquat = montant_credit - montant_remplacement;

                // Décrémenter le stock de l'article de remplacement.
                // Le stock suit desormais son mouvement : le declencheur
                // `stock_suit_les_mouvements` met le compteur a jour dans la meme
                // transaction. L'ecrire ici le compterait deux fois.

                // Mouvement de stock — sortie remplacement.
                tx.execute(
                    "INSERT INTO mouvement_stock
                     (id, article_id, depot_id, type_mouvement, quantite_delta,
                      operation_id, auteur_id, date_mouvement, cree_le, cree_par, origine)
                     VALUES (?1, ?2, ?3, 'echange', ?4, ?5, ?6, ?7, ?8, ?9, 'app')",
                    rusqlite::params![
                        uuid::Uuid::new_v4().to_string(),
                        art_remp_id, depot_remplacement, -qte_base_remp,
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
                            enregistrer_sortie_caisse(&tx, reliquat, mode,
                                &retour_id, "remboursement_reliquat",
                                &utilisateur_id, &maintenant)?;
                        }
                        _ if client_generique => {
                            return Err("Pas d'avoir pour un client comptant. \
                                        Le reliquat doit être remboursé.".to_string());
                        }
                        Some("remboursement") => {
                            // Jamais plus d'especes que d'argent recu : la
                            // part payee par avoir revient en avoir (D15).
                            let (en_especes, en_avoir, _) = crate::coeur::calcul::repartir_retour(
                                reliquat, plafond_especes, reliquat.max(plafond_especes));
                            let mode = mode_encaissement_reliquat
                                .as_deref().unwrap_or("especes");
                            if en_especes > 0 {
                                enregistrer_sortie_caisse(&tx, en_especes, mode,
                                    &retour_id, "remboursement_reliquat",
                                    &utilisateur_id, &maintenant)?;
                            }
                            if en_avoir > 0 {
                                numero_avoir = Some(creer_avoir_client(
                                    &tx, &client_id, &retour_id, en_avoir,
                                    &maintenant, &article_id, &unite_vente_id,
                                    quantite, &utilisateur_id,
                                )?);
                            }
                        }
                        _ => {
                            // Avoir par défaut.
                            numero_avoir = Some(creer_avoir_client(
                                &tx, &client_id, &retour_id, reliquat,
                                &maintenant, &article_id, &unite_vente_id,
                                quantite, &utilisateur_id,
                            )?);
                        }
                    }
                } else if reliquat < 0 {
                    // Client paie la différence — entrée de caisse.
                    let mode = mode_encaissement.as_deref().unwrap_or("especes");
                    // Le client verse la difference : entree reelle.
                    let session_id: Option<String> =
                        Some(crate::utils::exiger_session_caisse(&tx)?);

                    if let Some(sid) = session_id {
                        tx.execute(
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
    tx.execute(
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

    tx.commit().map_err(|e| e.to_string())?;

    Ok(serde_json::json!({
        "retour_id":      retour_id,
        "montant_credit": montant_credit,
        // Repartition : ce qui a eteint la dette, ce qui revient au
        // client. Permet au front d'expliquer le resultat.
        "part_creance":   part_creance,
        "part_client":    part_client,
        "part_especes":   part_especes,
        "part_avoir":     part_avoir,
        // Numero de la piece AVC quand un avoir a ete cree — permet
        // d'ouvrir directement l'impression cote front.
        "numero_avoir":  numero_avoir,
    }))
}

// =====================================================================
//  Fonctions utilitaires internes
// =====================================================================

pub fn enregistrer_sortie_caisse(
    conn: &rusqlite::Connection,
    montant: i64,
    mode: &str,
    operation_id: &str,
    motif: &str,
    utilisateur_id: &str,
    maintenant: &str,
) -> Result<(), String> {
    // v3, C-3 : l'argent qui ressort du tiroir, sous le plafond de la
    // personne qui agit. Ici, au point de sortie : le montant n'est
    // connu qu'au fond du retour.
    crate::plafonds::exiger_remboursement(conn, montant)?;
    // Refus plutot qu'ecriture manquante : rendre de l'argent sans
    // mouvement de caisse cree un manque inexplicable a la cloture.
    let sid = crate::utils::exiger_session_caisse(conn)?;
    {
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
pub fn creer_avoir_client(
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

    // ---- Piece commerciale AVC ----
    // La piece est creee AVANT l'avoir pour que celui-ci porte son
    // `piece_id`. Les deux existaient cote a cote sans aucun lien : un
    // avoir entierement consomme affichait son montant plein en
    // « reste » sur l'ecran Pieces, pour toujours (bug #8).
    let numero = crate::argent::reserver_numero(conn, "avoir_client")?;
    let piece_id = uuid::Uuid::new_v4().to_string();

    conn.execute(
        "INSERT INTO avoir
         (id, client_id, retour_id, piece_id, montant, statut, cree_le, origine)
         VALUES (?1, ?2, ?3, ?4, ?5, 'ouvert', ?6, 'app')",
        rusqlite::params![avoir_id, client_id, retour_id, piece_id,
                          montant, maintenant],
    ).map_err(|e| e.to_string())?;

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

pub fn lire_avoirs_ouverts_tous(
    conn: &rusqlite::Connection,
) -> Result<Vec<serde_json::Value>, String> {

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

// =====================================================================
//  LES RETOURS, SUR L'UN OU L'AUTRE MOTEUR
// =====================================================================
//
// `enregistrer_retour` est le morceau qui compte : trois modes, deux
// gardes, et de l'argent qui sort du tiroir. Le texte suit la version
// SQLite pas a pas ; les aides (sortie de caisse, avoir) prennent un
// `impl Acces` pour servir dans la transaction.

use crate::argent::{id_utilisateur_courant_sur, reserver_numero_sur};
use crate::base::{Acces, Base};
use crate::parametres;

pub fn lire_ventes_recentes_sur_base(base: &mut Base) -> Result<Vec<serde_json::Value>, String> {
    let dossier = base.dossier().to_string();

    type VenteLue = (String, String, String, String, String, String, Option<String>, i64);
    let ventes_base: Vec<VenteLue> = base
        .lire_plusieurs(
            "SELECT v.id, v.date_vente, v.statut, v.client_id,
                    c.nom, c.code, f.numero, c.est_generique
             FROM vente v
             JOIN client c ON c.id = v.client_id
             LEFT JOIN piece_commerciale f ON f.id = v.piece_id
             WHERE v.dossier_id = ?1
             ORDER BY v.date_vente DESC
             LIMIT 50",
            &parametres![dossier.clone()],
            |r| {
                Ok((
                    r.get::<String>(0)?,
                    r.get::<String>(1)?,
                    r.get::<String>(2)?,
                    r.get::<String>(3)?,
                    r.get::<String>(4)?,
                    r.get::<String>(5)?,
                    r.get::<Option<String>>(6)?,
                    r.get::<i64>(7)?,
                ))
            },
        )
        .map_err(|e| e.0)?;

    let mut ventes = Vec::with_capacity(ventes_base.len());
    for (id, date_vente, statut, client_id, client_nom, client_code, numero_facture, generique) in ventes_base {
        // `deja_retourne` : la fenetre de retour propose ce qui reste, et
        // grise un article tout revenu (pas deux fois).
        let lignes: Vec<serde_json::Value> = base
            .lire_plusieurs(
                "SELECT lv.id, lv.article_id, a.nom, lv.unite_vente_id,
                        u.libelle, lv.depot_source_id,
                        lv.quantite, lv.prix_pratique,
                        CAST(lv.prix_pratique * lv.quantite AS BIGINT),
                        CAST(COALESCE((SELECT SUM(rt.quantite) FROM retour rt
                                       WHERE rt.ligne_vente_id = lv.id), 0) AS DOUBLE PRECISION)
                 FROM ligne_vente lv
                 JOIN article a ON a.id = lv.article_id
                 JOIN unite_vente u ON u.id = lv.unite_vente_id
                 WHERE lv.vente_id = ?1 AND lv.dossier_id = ?2",
                &parametres![id.clone(), dossier.clone()],
                |r| {
                    Ok(serde_json::json!({
                        "id":              r.get::<String>(0)?,
                        "article_id":      r.get::<String>(1)?,
                        "article_nom":     r.get::<String>(2)?,
                        "unite_vente_id":  r.get::<String>(3)?,
                        "unite_libelle":   r.get::<String>(4)?,
                        "depot_source_id": r.get::<String>(5)?,
                        "quantite":        r.get::<f64>(6)?,
                        "prix_pratique":   r.get::<i64>(7)?,
                        "montant":         r.get::<i64>(8)?,
                        "deja_retourne":   r.get::<f64>(9)?,
                    }))
                },
            )
            .map_err(|e| e.0)?;

        let total: i64 = lignes.iter().filter_map(|l| l["montant"].as_i64()).sum();
        ventes.push(serde_json::json!({
            "id":             id,
            "date_vente":     date_vente,
            "statut":         statut,
            "client_id":      client_id,
            "client_nom":     client_nom,
            "client_code":    client_code,
            // Le « Comptant » et les fiches de passage (D35, D40) : pas d'avoir.
            "client_generique": generique != 0,
            "numero_facture": numero_facture,
            "total":          total,
            "lignes":         lignes,
        }));
    }
    Ok(ventes)
}

/// Une somme d'argent lue dans la transaction, 0 si rien.
fn montant(acces: &mut impl Acces, sql: &str, params: &[crate::base::Valeur]) -> Result<i64, String> {
    Ok(acces.lire_une(sql, params, |r| r.get::<i64>(0)).map_err(|e| e.0)?.unwrap_or(0))
}

/// Le client est-il le client de passage ? `client` est cloisonnee.
fn est_client_generique_sur(acces: &mut impl Acces, client_id: &str) -> bool {
    let dossier = acces.dossier().to_string();
    acces
        .lire_une(
            "SELECT est_generique FROM client WHERE id = ?1 AND dossier_id = ?2",
            &parametres![client_id, dossier],
            |r| r.get::<i64>(0),
        )
        .ok()
        .flatten()
        .map(|v| v != 0)
        .unwrap_or(false)
}

/// Sortie de caisse d'un remboursement. Refus plutot qu'ecriture
/// manquante : rendre de l'argent sans mouvement cree un manque
/// inexplicable a la cloture.
pub fn enregistrer_sortie_caisse_sur(
    acces: &mut impl Acces,
    montant: i64,
    mode: &str,
    operation_id: &str,
    motif: &str,
    utilisateur_id: &str,
    maintenant: &str,
) -> Result<(), String> {
    let dossier = acces.dossier().to_string();
    crate::plafonds::exiger_remboursement_sur(acces, montant)?;
    let sid = crate::caisses::exiger_sur(acces, None)?;
    acces
        .executer(
            "INSERT INTO mouvement_caisse
             (id, session_id, sens, moyen, montant, motif,
              operation_id, date_mouvement, cree_le, cree_par, origine, dossier_id)
             VALUES (?1, ?2, 'sortie', ?3, ?4, ?5, ?6, ?7, ?7, ?8, 'app', ?9)",
            &parametres![
                uuid::Uuid::new_v4().to_string(),
                sid,
                mode,
                montant,
                motif,
                operation_id,
                maintenant,
                utilisateur_id,
                dossier
            ],
        )
        .map_err(|e| e.0)
        .map(|_| ())
}

/// L'avoir d'un geste de retour : UNE piece AVC, qu'on rende un article
/// ou plusieurs. Le premier article la cree (numero reserve dans la
/// transaction), les suivants y ajoutent leur ligne ; chacun garde sa
/// ligne `avoir` (son `retour_id`), toutes sur la meme `piece_id` —
/// le credit ouvert et le remboursement d'un avoir se lisent par piece.
#[derive(Default)]
pub struct AvoirDuRetour {
    piece: Option<(String, String)>,
    total: i64,
}

impl AvoirDuRetour {
    /// Le numero de la piece AVC, si le geste en a cree une.
    pub fn numero(&self) -> Option<String> {
        self.piece.as_ref().map(|(_, n)| n.clone())
    }
}

/// Cree l'avoir — et sa piece AVC au premier article du geste : un
/// engagement envers le client doit exister sur un document numerote.
#[allow(clippy::too_many_arguments)]
pub fn creer_avoir_client_sur(
    acces: &mut impl Acces,
    avoir: &mut AvoirDuRetour,
    client_id: &str,
    retour_id: &str,
    montant: i64,
    maintenant: &str,
    article_id: &str,
    unite_vente_id: &str,
    quantite: f64,
    auteur_id: &str,
) -> Result<String, String> {
    let dossier = acces.dossier().to_string();
    if avoir.piece.is_none() {
        let numero = reserver_numero_sur(acces, "avoir_client")?;
        let piece_id = uuid::Uuid::new_v4().to_string();
        acces
            .executer(
                "INSERT INTO piece_commerciale
                 (id, type_piece, numero, statut, tiers_type, tiers_id,
                  auteur_id, date_piece, remise_globale, note,
                  cree_le, modifie_le, origine, dossier_id)
                 VALUES (?1,'avoir_client',?2,'emis','client',?3,?4,?5,0.0,?6,?5,?5,'retour',?7)",
                &parametres![
                    piece_id.clone(),
                    numero.clone(),
                    client_id,
                    auteur_id,
                    maintenant,
                    format!("Avoir sur retour — {} F", montant),
                    dossier.clone()
                ],
            )
            .map_err(|e| e.0)?;
        avoir.piece = Some((piece_id, numero));
    }
    let (piece_id, numero) = avoir.piece.clone().unwrap_or_default();
    avoir.total += montant;

    acces
        .executer(
            "INSERT INTO avoir
             (id, client_id, retour_id, piece_id, montant, statut, cree_le, origine, dossier_id)
             VALUES (?1, ?2, ?3, ?4, ?5, 'ouvert', ?6, 'app', ?7)",
            &parametres![
                uuid::Uuid::new_v4().to_string(),
                client_id,
                retour_id,
                piece_id.clone(),
                montant,
                maintenant,
                dossier.clone()
            ],
        )
        .map_err(|e| e.0)?;
    // La note dit le total du geste, pas celui du premier article.
    acces
        .executer(
            "UPDATE piece_commerciale SET note = ?1 WHERE id = ?2 AND dossier_id = ?3",
            &parametres![format!("Avoir sur retour — {} F", avoir.total), piece_id.clone(), dossier.clone()],
        )
        .map_err(|e| e.0)?;

    let prix_u = if quantite > 0.0 { (montant as f64 / quantite).round() as i64 } else { montant };
    acces
        .executer(
            "INSERT INTO ligne_piece
             (id, piece_id, article_id, unite_vente_id, quantite,
              prix_unitaire, remise_pct, remise_montant,
              taux_tva, montant_tva, montant_ht, cree_le, dossier_id)
             VALUES (?1,?2,?3,?4,?5,?6,0.0,0,0.0,0,?7,?8,?9)",
            &parametres![
                uuid::Uuid::new_v4().to_string(),
                piece_id,
                article_id,
                unite_vente_id,
                quantite,
                prix_u,
                montant,
                maintenant,
                dossier
            ],
        )
        .map_err(|e| e.0)?;

    Ok(numero)
}

/// Un article coche dans la fenetre de retour : la ligne de la vente,
/// et combien en revient.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ArticleRendu {
    #[serde(alias = "ligneVenteId")]
    pub ligne_vente_id: String,
    pub quantite: f64,
}

/// Un article que le client prend en echange.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ArticlePris {
    #[serde(alias = "articleId")]
    pub article_id: String,
    #[serde(alias = "uniteVenteId")]
    pub unite_vente_id: String,
    pub quantite: f64,
}

/// Ce qu'un article rendu a fait, pour le bilan du geste.
struct LigneRendue {
    retour_id: String,
    montant_credit: i64,
    part_creance: i64,
    part_client: i64,
    part_especes: i64,
    part_avoir: i64,
    article_id: String,
    unite_vente_id: String,
    quantite: f64,
    depot_source_id: String,
    client_id: String,
    client_generique: bool,
    /// L'argent recu sur la vente moins deja rembourse, AVANT ce retour.
    plafond_especes: i64,
}

/// Un article qui revient, DANS la transaction de l'appelant : le stock
/// remonte, le retour s'ecrit, la dette s'eteint d'abord (D33), puis le
/// client recoit sa part — en argent ou en avoir selon ce qu'il a verse
/// (D15). En echange, rien ici ne regle la difference : c'est
/// `remplacer_et_regler`, une fois pour tout le geste.
///
/// Chaque lecture voit les ecritures des articles precedents du meme
/// geste : les gardes (« pas plus que vendu », « jamais plus que
/// verse ») s'additionnent d'un article a l'autre.
#[allow(clippy::too_many_arguments)]
fn retourner_une_ligne(
    tx: &mut impl Acces,
    avoir: &mut AvoirDuRetour,
    vente_id: &str,
    ligne_vente_id: &str,
    quantite: f64,
    mode_resolution: &str,
    mode_encaissement: Option<&str>,
    utilisateur_id: &str,
    maintenant: &str,
) -> Result<LigneRendue, String> {
    let dossier = tx.dossier().to_string();
    let retour_id = uuid::Uuid::new_v4().to_string();

    // La ligne doit etre DE cette vente : un retour groupe ne melange
    // pas deux factures.
    let (article_id, unite_vente_id, depot_source_id, facteur, prix_pratique, client_id, qte_vendue):
        (String, String, String, f64, i64, String, f64) = tx
        .lire_une(
            "SELECT lv.article_id, lv.unite_vente_id, lv.depot_source_id,
                    u.facteur, lv.prix_pratique, v.client_id, lv.quantite
             FROM ligne_vente lv
             JOIN unite_vente u ON u.id = lv.unite_vente_id
             JOIN vente v ON v.id = lv.vente_id
             WHERE lv.id = ?1 AND lv.vente_id = ?3 AND lv.dossier_id = ?2",
            &parametres![ligne_vente_id, dossier.clone(), vente_id],
            |r| {
                Ok((
                    r.get::<String>(0)?,
                    r.get::<String>(1)?,
                    r.get::<String>(2)?,
                    r.get::<f64>(3)?,
                    r.get::<i64>(4)?,
                    r.get::<String>(5)?,
                    r.get::<f64>(6)?,
                ))
            },
        )
        .map_err(|e| e.0)?
        .ok_or_else(|| "Ligne de vente introuvable".to_string())?;

    // ---- GARDE 1 : pas plus que vendu, par LIGNE.
    let deja_retourne: f64 = tx
        .lire_une(
            "SELECT CAST(COALESCE(SUM(quantite), 0) AS DOUBLE PRECISION) FROM retour
             WHERE vente_id = ?1 AND dossier_id = ?4
               AND (ligne_vente_id = ?2
                    OR (ligne_vente_id IS NULL AND article_id = ?3))",
            &parametres![vente_id, ligne_vente_id, article_id.clone(), dossier.clone()],
            |r| r.get::<f64>(0),
        )
        .map_err(|e| e.0)?
        .unwrap_or(0.0);

    let retournable = qte_vendue - deja_retourne;
    if quantite <= 0.0 {
        return Err("La quantité retournée doit être positive".to_string());
    }
    if quantite > retournable + 1e-9 {
        return Err(format!(
            "Quantité trop élevée : {} vendue(s), {} déjà retournée(s), \
             il reste {} retournable(s).",
            qte_vendue,
            deja_retourne,
            retournable.max(0.0)
        ));
    }

    let montant_credit: i64 = (prix_pratique as f64 * quantite).round() as i64;
    let quantite_base = quantite * facteur;

    // ---- GARDE 2 : le retour eteint d'abord la dette (D33).
    let total_vente = montant(
        tx,
        "SELECT CAST(COALESCE(SUM(prix_pratique * quantite), 0) AS BIGINT)
         FROM ligne_vente WHERE vente_id = ?1 AND dossier_id = ?2",
        &parametres![vente_id, dossier.clone()],
    )?;
    let deja_paye = montant(
        tx,
        "SELECT CAST(COALESCE(SUM(montant), 0) AS BIGINT)
         FROM paiement WHERE vente_id = ?1 AND dossier_id = ?2",
        &parametres![vente_id, dossier.clone()],
    )?;
    let reste_du = (total_vente - deja_paye).max(0);
    let part_creance = if mode_resolution == "echange" { 0 } else { montant_credit.min(reste_du) };

    // Seconde borne : jamais plus que ce que le client a verse.
    let rembourse_deja = montant(
        tx,
        "SELECT CAST(COALESCE(SUM(mc.montant), 0) AS BIGINT)
         FROM mouvement_caisse mc
         WHERE mc.motif IN ('remboursement','remboursement_reliquat')
           AND mc.dossier_id = ?2
           AND mc.operation_id IN (SELECT id FROM retour WHERE vente_id = ?1)",
        &parametres![vente_id, dossier.clone()],
    )?;
    let verse_reel = montant(
        tx,
        "SELECT CAST(COALESCE(SUM(montant), 0) AS BIGINT)
         FROM paiement
         WHERE vente_id = ?1 AND mode NOT IN ('retour', 'avoir') AND dossier_id = ?2",
        &parametres![vente_id, dossier.clone()],
    )?;
    // Ce qu'il a paye avec un avoir : pas d'argent recu, mais un credit
    // consomme — il revient en avoir, jamais en especes (D15).
    let verse_avoir = montant(
        tx,
        "SELECT CAST(COALESCE(SUM(montant), 0) AS BIGINT)
         FROM paiement WHERE vente_id = ?1 AND mode = 'avoir' AND dossier_id = ?2",
        &parametres![vente_id, dossier.clone()],
    )?;
    let avoirs_deja_rendus = montant(
        tx,
        "SELECT CAST(COALESCE(SUM(a.montant), 0) AS BIGINT)
         FROM avoir a
         WHERE a.dossier_id = ?2
           AND a.retour_id IN (SELECT id FROM retour WHERE vente_id = ?1)",
        &parametres![vente_id, dossier.clone()],
    )?;
    let plafond_especes = verse_reel - rembourse_deja;
    let plafond_total = verse_reel + verse_avoir - rembourse_deja - avoirs_deja_rendus;
    let part_client_brute = montant_credit - part_creance;
    let (part_especes, part_avoir, non_attribue) =
        crate::coeur::calcul::repartir_retour(part_client_brute, plafond_especes, plafond_total);
    let part_client = part_especes + part_avoir;
    if non_attribue > 0 {
        crate::journal::anomalie_sur(tx, "retour", &retour_id, Some(utilisateur_id), serde_json::json!({
            "message": format!("Retour : {non_attribue} F ni imputables ni remboursables (versé {verse_reel}, par avoir {verse_avoir}, déjà remboursé {rembourse_deja}, avoirs rendus {avoirs_deja_rendus})"),
            "non_attribue": non_attribue, "verse": verse_reel, "verse_avoir": verse_avoir,
            "rembourse_deja": rembourse_deja, "avoirs_rendus": avoirs_deja_rendus, "vente_id": vente_id,
        }));
    }

    // ---- 1. Le stock remonte — TOUJOURS.
    tx.executer(
        "INSERT INTO mouvement_stock
         (id, article_id, depot_id, type_mouvement, quantite_delta,
          operation_id, auteur_id, date_mouvement, cree_le, cree_par, origine, dossier_id)
         VALUES (?1, ?2, ?3, 'retour', ?4, ?5, ?6, ?7, ?7, ?6, 'app', ?8)",
        &parametres![
            uuid::Uuid::new_v4().to_string(),
            article_id.clone(),
            depot_source_id.clone(),
            quantite_base,
            retour_id.clone(),
            utilisateur_id,
            maintenant,
            dossier.clone()
        ],
    )
    .map_err(|e| e.0)?;

    // ---- 2. Le retour.
    tx.executer(
        "INSERT INTO retour
         (id, vente_id, ligne_vente_id, article_id, unite_vente_id, quantite,
          depot_reintegration_id, mode_resolution, montant_credit,
          reliquat, reliquat_resolution, auteur_id, date_retour,
          cree_le, cree_par, origine, dossier_id)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,0,?10,?11,?12,?12,?11,'app',?13)",
        &parametres![
            retour_id.clone(),
            vente_id,
            ligne_vente_id,
            article_id.clone(),
            unite_vente_id.clone(),
            quantite,
            depot_source_id.clone(),
            mode_resolution,
            montant_credit,
            mode_encaissement.unwrap_or("aucun"),
            utilisateur_id,
            maintenant,
            dossier.clone()
        ],
    )
    .map_err(|e| e.0)?;

    // ---- 2 bis. La part qui eteint la dette : un paiement 'retour'.
    if part_creance > 0 && mode_resolution != "echange" {
        tx.executer(
            "INSERT INTO paiement
             (id, vente_id, montant, mode, date_paiement,
              auteur_id, cree_le, cree_par, origine, dossier_id)
             VALUES (?1,?2,?3,'retour',?4,?5,?4,?5,'retour',?6)",
            &parametres![
                uuid::Uuid::new_v4().to_string(),
                vente_id,
                part_creance,
                maintenant,
                utilisateur_id,
                dossier.clone()
            ],
        )
        .map_err(|e| e.0)?;

        let paye_apres = deja_paye + part_creance;
        let statut = match crate::coeur::calcul::statut_vente(total_vente, paye_apres) {
            crate::coeur::calcul::StatutVente::Payee => "payee",
            crate::coeur::calcul::StatutVente::PartiellementPayee => "partiellement_payee",
            crate::coeur::calcul::StatutVente::CreanceOuverte => "creance_ouverte",
        };
        tx.executer(
            "UPDATE vente SET statut = ?1, modifie_le = ?2 WHERE id = ?3 AND dossier_id = ?4",
            &parametres![statut, maintenant, vente_id, dossier.clone()],
        )
        .map_err(|e| e.0)?;
    }

    // D40 — pas d'avoir au client de passage.
    let client_generique = est_client_generique_sur(tx, &client_id);
    if client_generique && mode_resolution == "avoir_conserve" {
        return Err("Pas d'avoir pour un client comptant. \
                    Choisir remboursement ou échange."
            .to_string());
    }

    // ---- 3. Le solde eventuel — uniquement ce que le client a verse.
    match mode_resolution {
        "remboursement" => {
            if part_especes > 0 {
                enregistrer_sortie_caisse_sur(
                    tx, part_especes, mode_encaissement.unwrap_or("especes"), &retour_id,
                    "remboursement", utilisateur_id, maintenant,
                )?;
            }
            // La part payee avec un avoir revient en avoir, meme quand
            // le client demande un remboursement (D15). Un client de
            // passage n'a pas d'avoir a consommer (D40) : part nulle.
            if part_avoir > 0 {
                if client_generique {
                    return Err("Cette vente a été payée avec un avoir : \
                                il ne peut pas être remboursé en espèces."
                        .to_string());
                }
                creer_avoir_client_sur(
                    tx, avoir, &client_id, &retour_id, part_avoir, maintenant,
                    &article_id, &unite_vente_id, quantite, utilisateur_id,
                )?;
            }
        }
        "avoir_conserve" if part_client > 0 => {
            creer_avoir_client_sur(
                tx, avoir, &client_id, &retour_id, part_client, maintenant,
                &article_id, &unite_vente_id, quantite, utilisateur_id,
            )?;
        }
        _ => {}
    }

    // ---- 4. Journal.
    tx.executer(
        "INSERT INTO journal
         (id, type_evenement, entite_type, entite_id, auteur_id,
          nouveau_valeur, origine, date_evenement, dossier_id)
         VALUES (?1, 'retour_enregistre', 'retour', ?2, ?3, ?4, 'app', ?5, ?6)",
        &parametres![
            uuid::Uuid::new_v4().to_string(),
            retour_id.clone(),
            utilisateur_id,
            serde_json::json!({ "mode": mode_resolution, "credit": montant_credit }).to_string(),
            maintenant,
            dossier
        ],
    )
    .map_err(|e| e.0)?;

    Ok(LigneRendue {
        retour_id,
        montant_credit,
        part_creance,
        part_client,
        part_especes,
        part_avoir,
        article_id,
        unite_vente_id,
        quantite,
        depot_source_id,
        client_id,
        client_generique,
        plafond_especes,
    })
}

/// L'echange, une fois pour tout le geste : les articles pris sortent du
/// magasin de la vente (D43, sinon le magasin par defaut), et la
/// difference entre ce qui est rendu et ce qui est pris se regle en UNE
/// fois — le client paie le complement, ou recoit le reliquat (argent
/// dans la limite de ce qu'il a verse, sinon avoir). Rend le reliquat
/// (positif : au client ; negatif : paye par lui).
#[allow(clippy::too_many_arguments)]
fn remplacer_et_regler(
    tx: &mut impl Acces,
    avoir: &mut AvoirDuRetour,
    rendues: &[LigneRendue],
    pris: &[ArticlePris],
    mode_encaissement: Option<&str>,
    mode_reliquat_positif: Option<&str>,
    mode_encaissement_reliquat: Option<&str>,
    utilisateur_id: &str,
    maintenant: &str,
) -> Result<i64, String> {
    let dossier = tx.dossier().to_string();
    let premier = rendues.first().ok_or_else(|| "Aucun article rendu.".to_string())?;
    let credit: i64 = rendues.iter().map(|l| l.montant_credit).sum();

    let depot_remplacement: String = match tx
        .lire_une(
            "SELECT id FROM depot WHERE id = ?1 AND actif = 1 AND dossier_id = ?2",
            &parametres![premier.depot_source_id.clone(), dossier.clone()],
            |r| r.get::<String>(0),
        )
        .ok()
        .flatten()
    {
        Some(id) => id,
        None => tx
            .lire_une(
                "SELECT id FROM depot WHERE est_defaut = 1 AND actif = 1
                 AND dossier_id = ?1 LIMIT 1",
                &parametres![dossier.clone()],
                |r| r.get::<String>(0),
            )
            .ok()
            .flatten()
            .ok_or_else(|| {
                "Aucun magasin actif pour sortir le remplacement. \
                 Vérifier Paramètres → Magasins."
                    .to_string()
            })?,
    };

    let mut montant_pris: i64 = 0;
    for p in pris {
        if p.quantite <= 0.0 {
            return Err("La quantité prise en échange doit être positive".to_string());
        }
        let (facteur_remp, prix_remp): (f64, i64) = tx
            .lire_une(
                "SELECT u.facteur, u.prix_reference FROM unite_vente u WHERE u.id = ?1",
                &parametres![p.unite_vente_id.clone()],
                |r| Ok((r.get::<f64>(0)?, r.get::<i64>(1)?)),
            )
            .map_err(|e| e.0)?
            .ok_or_else(|| "Unité de remplacement introuvable".to_string())?;
        tx.executer(
            "INSERT INTO mouvement_stock
             (id, article_id, depot_id, type_mouvement, quantite_delta,
              operation_id, auteur_id, date_mouvement, cree_le, cree_par, origine, dossier_id)
             VALUES (?1, ?2, ?3, 'echange', ?4, ?5, ?6, ?7, ?7, ?6, 'app', ?8)",
            &parametres![
                uuid::Uuid::new_v4().to_string(),
                p.article_id.clone(),
                depot_remplacement.clone(),
                -(p.quantite * facteur_remp),
                premier.retour_id.clone(),
                utilisateur_id,
                maintenant,
                dossier.clone()
            ],
        )
        .map_err(|e| e.0)?;
        montant_pris += (prix_remp as f64 * p.quantite).round() as i64;
    }

    let reliquat = credit - montant_pris;
    // Le reliquat en avoir se porte sur la piece AVC article par article,
    // au prorata de ce que chacun a rendu.
    let en_avoir = |tx: &mut _, avoir: &mut AvoirDuRetour, total: i64| -> Result<(), String> {
        let poids: Vec<i64> = rendues.iter().map(|l| l.montant_credit).collect();
        for (l, part) in rendues.iter().zip(crate::coeur::calcul::repartir_au_prorata(total, &poids)) {
            if part > 0 {
                creer_avoir_client_sur(
                    tx, avoir, &l.client_id, &l.retour_id, part, maintenant,
                    &l.article_id, &l.unite_vente_id, l.quantite, utilisateur_id,
                )?;
            }
        }
        Ok(())
    };

    if reliquat > 0 {
        match mode_reliquat_positif {
            // Le client de passage D'ABORD : la branche par defaut est
            // l'avoir, qu'il ne peut pas recevoir.
            Some("remboursement") | None if premier.client_generique => {
                enregistrer_sortie_caisse_sur(
                    tx, reliquat, mode_encaissement_reliquat.unwrap_or("especes"),
                    &premier.retour_id, "remboursement_reliquat", utilisateur_id, maintenant,
                )?;
            }
            _ if premier.client_generique => {
                return Err("Pas d'avoir pour un client comptant. \
                            Le reliquat doit être remboursé."
                    .to_string());
            }
            Some("remboursement") => {
                // Jamais plus d'especes que d'argent recu : la part payee
                // par avoir revient en avoir (D15).
                let (en_especes, part_en_avoir, _) = crate::coeur::calcul::repartir_retour(
                    reliquat, premier.plafond_especes, reliquat.max(premier.plafond_especes));
                if en_especes > 0 {
                    enregistrer_sortie_caisse_sur(
                        tx, en_especes, mode_encaissement_reliquat.unwrap_or("especes"),
                        &premier.retour_id, "remboursement_reliquat", utilisateur_id, maintenant,
                    )?;
                }
                if part_en_avoir > 0 {
                    en_avoir(tx, avoir, part_en_avoir)?;
                }
            }
            _ => en_avoir(tx, avoir, reliquat)?,
        }
    } else if reliquat < 0 {
        // Le client paie la difference : entree reelle.
        let sid = crate::caisses::exiger_sur(tx, None)?;
        tx.executer(
            "INSERT INTO mouvement_caisse
             (id, session_id, sens, moyen, montant, motif,
              operation_id, date_mouvement, cree_le, cree_par, origine, dossier_id)
             VALUES (?1, ?2, 'entree', ?3, ?4, 'complement_echange',
                     ?5, ?6, ?6, ?7, 'app', ?8)",
            &parametres![
                uuid::Uuid::new_v4().to_string(),
                sid,
                mode_encaissement.unwrap_or("especes"),
                -reliquat,
                premier.retour_id.clone(),
                maintenant,
                utilisateur_id,
                dossier
            ],
        )
        .map_err(|e| e.0)?;
    }
    Ok(reliquat)
}

#[allow(clippy::too_many_arguments)]
pub fn enregistrer_retour_sur_base(
    base: &mut Base,
    vente_id: String,
    ligne_vente_id: String,
    quantite: f64,
    mode_resolution: String,
    mode_encaissement: Option<String>,
    article_remplacement_id: Option<String>,
    unite_remplacement_id: Option<String>,
    quantite_remplacement: Option<f64>,
    mode_reliquat_positif: Option<String>,
    mode_encaissement_reliquat: Option<String>,
) -> Result<serde_json::Value, String> {
    // TOUT le retour dans UNE transaction : un remboursement refuse
    // faute de caisse ne doit laisser ni stock remonte ni ligne posee.
    let mut tx = base.transaction().map_err(|e| e.0)?;
    let utilisateur_id = id_utilisateur_courant_sur(&mut tx);
    let maintenant = maintenant_iso();
    let mut avoir = AvoirDuRetour::default();

    let l = retourner_une_ligne(
        &mut tx, &mut avoir, &vente_id, &ligne_vente_id, quantite, &mode_resolution,
        mode_encaissement.as_deref(), &utilisateur_id, &maintenant,
    )?;
    if mode_resolution == "echange" {
        if let (Some(article_id), Some(unite_vente_id), Some(quantite)) =
            (article_remplacement_id, unite_remplacement_id, quantite_remplacement)
        {
            remplacer_et_regler(
                &mut tx, &mut avoir, std::slice::from_ref(&l),
                &[ArticlePris { article_id, unite_vente_id, quantite }],
                mode_encaissement.as_deref(), mode_reliquat_positif.as_deref(),
                mode_encaissement_reliquat.as_deref(), &utilisateur_id, &maintenant,
            )?;
        }
    }
    // D41 : le retour qui eteint la dette a 5 F pres la solde ici.
    if l.part_creance > 0 {
        crate::creances::solder_residu_sur(&mut tx, &vente_id, &utilisateur_id, &maintenant)?;
    }

    tx.valider().map_err(|e| e.0)?;

    Ok(serde_json::json!({
        "retour_id":      l.retour_id,
        "montant_credit": l.montant_credit,
        "part_creance":   l.part_creance,
        "part_client":    l.part_client,
        "part_especes":   l.part_especes,
        "part_avoir":     l.part_avoir,
        "numero_avoir":   avoir.numero(),
    }))
}

/// Le retour de PLUSIEURS articles d'une vente, en un geste : la fenetre
/// de retour coche les articles (et, en echange, ce que le client prend).
/// Tout ou rien — un article refuse (trop rendu, caisse fermee, plafond)
/// n'en laisse aucun derriere lui. Une seule piece AVC pour le geste ;
/// la difference d'un echange se regle une fois. Nee sur `Base` (D22) :
/// la fenetre ne parle plus qu'au serveur.
#[allow(clippy::too_many_arguments)]
pub fn enregistrer_retour_groupe_sur_base(
    base: &mut Base,
    vente_id: String,
    articles: Vec<ArticleRendu>,
    mode_resolution: String,
    mode_encaissement: Option<String>,
    pris: Vec<ArticlePris>,
    mode_reliquat_positif: Option<String>,
    mode_encaissement_reliquat: Option<String>,
) -> Result<serde_json::Value, String> {
    let lignes: Vec<&str> = articles.iter().map(|a| a.ligne_vente_id.as_str()).collect();
    crate::coeur::pieces::verifier_retour_groupe(&lignes, &mode_resolution, pris.len())?;

    let mut tx = base.transaction().map_err(|e| e.0)?;
    let utilisateur_id = id_utilisateur_courant_sur(&mut tx);
    let maintenant = maintenant_iso();
    let mut avoir = AvoirDuRetour::default();

    let mut rendues = Vec::with_capacity(articles.len());
    for a in &articles {
        rendues.push(retourner_une_ligne(
            &mut tx, &mut avoir, &vente_id, &a.ligne_vente_id, a.quantite, &mode_resolution,
            mode_encaissement.as_deref(), &utilisateur_id, &maintenant,
        )?);
    }
    let reliquat = if mode_resolution == "echange" {
        remplacer_et_regler(
            &mut tx, &mut avoir, &rendues, &pris,
            mode_encaissement.as_deref(), mode_reliquat_positif.as_deref(),
            mode_encaissement_reliquat.as_deref(), &utilisateur_id, &maintenant,
        )?
    } else {
        0
    };
    let somme = |f: fn(&LigneRendue) -> i64| rendues.iter().map(f).sum::<i64>();
    let part_especes = somme(|l| l.part_especes);
    // Le plafond de remboursement (C-3) se juge sur le GESTE : chaque
    // article sous le plafond ne fait pas un remboursement permis.
    if part_especes > 0 {
        crate::plafonds::exiger_remboursement_sur(&mut tx, part_especes)?;
    }
    // D41 : une fois TOUT le geste ecrit — pas article par article, ou
    // un residu solde au premier serait eteint par le suivant.
    if somme(|l| l.part_creance) > 0 {
        crate::creances::solder_residu_sur(&mut tx, &vente_id, &utilisateur_id, &maintenant)?;
    }

    tx.valider().map_err(|e| e.0)?;

    Ok(serde_json::json!({
        "retours":        rendues.iter().map(|l| l.retour_id.clone()).collect::<Vec<_>>(),
        "montant_credit": somme(|l| l.montant_credit),
        "part_creance":   somme(|l| l.part_creance),
        "part_client":    somme(|l| l.part_client),
        "part_especes":   part_especes,
        "part_avoir":     somme(|l| l.part_avoir),
        "reliquat":       reliquat,
        "numero_avoir":   avoir.numero(),
    }))
}

pub fn lire_avoirs_ouverts_tous_sur_base(base: &mut Base) -> Result<Vec<serde_json::Value>, String> {
    let dossier = base.dossier().to_string();
    base.lire_plusieurs(
        "SELECT av.id, c.nom, c.code, av.montant, av.cree_le
         FROM avoir av
         JOIN client c ON c.id = av.client_id
         WHERE av.statut = 'ouvert' AND av.dossier_id = ?1
         ORDER BY av.cree_le DESC",
        &parametres![dossier],
        |r| {
            Ok(serde_json::json!({
                "id":          r.get::<String>(0)?,
                "client_nom":  r.get::<String>(1)?,
                "client_code": r.get::<String>(2)?,
                "montant":     r.get::<i64>(3)?,
                "cree_le":     r.get::<String>(4)?,
            }))
        },
    )
    .map_err(|e| e.0)
}
