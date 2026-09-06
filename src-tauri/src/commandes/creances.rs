//! Commandes pour le règlement des créances existantes.

use tauri::State;
use crate::commandes::ventes::EtatApp;
use crate::utils::maintenant_iso;

/// Etat des creances d'UN client — le releve qu'on lui remet.
///
/// D36 : le reste du se calcule sur les paiements reellement encaisses,
/// jamais sur le statut seul. Un statut peut mentir apres un cheque
/// rejete ou un avoir consomme ; la somme des paiements, non.
///
/// D31/D41 : le total se derive des `prix_pratique` stockes et passe par
/// `reste_exigible`, sinon un reliquat d'un franc apparait sur le
/// document remis au client — le genre de detail qui fait perdre une
/// heure au comptoir.
#[tauri::command]
pub fn lire_etat_creances_client(
    etat: State<EtatApp>,
    client_id: String,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let client = conn.query_row(
        "SELECT nom, code, telephone, adresse FROM client WHERE id = ?1",
        rusqlite::params![client_id],
        |r| Ok(serde_json::json!({
            "nom":       r.get::<_, String>(0)?,
            "code":      r.get::<_, String>(1)?,
            "telephone": r.get::<_, Option<String>>(2)?,
            "adresse":   r.get::<_, Option<String>>(3)?,
        })),
    ).map_err(|_| "Client introuvable".to_string())?;

    let mut st = conn.prepare(
        "SELECT v.date_vente, COALESCE(p.numero, ''),
                CAST(COALESCE(SUM(lv.prix_pratique * lv.quantite), 0) AS INTEGER),
                CAST(COALESCE(
                  (SELECT SUM(montant) FROM paiement WHERE vente_id = v.id), 0
                ) AS INTEGER)
         FROM vente v
         LEFT JOIN ligne_vente lv ON lv.vente_id = v.id
         LEFT JOIN piece_commerciale p ON p.id = v.piece_id
         WHERE v.client_id = ?1
           AND v.statut IN ('creance_ouverte','partiellement_payee')
         GROUP BY v.id
         ORDER BY v.date_vente"
    ).map_err(|e| e.to_string())?;

    let lignes: Vec<serde_json::Value> = st.query_map(
        rusqlite::params![client_id], |r| {
            let total: i64 = r.get(2)?;
            let paye: i64 = r.get(3)?;
            Ok(serde_json::json!({
                "date":   r.get::<_, String>(0)?,
                "numero": r.get::<_, String>(1)?,
                "total":  total,
                "paye":   paye,
                "reste":  crate::coeur::calcul::reste_exigible(total, paye),
            }))
        }
    ).map_err(|e| e.to_string())?.filter_map(|r| r.ok())
    // Une vente soldee au franc pres (D41) n'a plus rien a reclamer :
    // la faire figurer avec un reste a zero ferait douter le client.
    .filter(|l| l["reste"].as_i64().unwrap_or(0) > 0)
    .collect();

    let total_du: i64 = lignes.iter()
        .filter_map(|l| l["reste"].as_i64()).sum();

    // Avoirs ouverts : le client a du credit chez nous. L'omettre du
    // releve reviendrait a lui reclamer plus qu'il ne doit reellement.
    let avoirs: i64 = conn.query_row(
        "SELECT CAST(COALESCE(SUM(montant), 0) AS INTEGER)
         FROM avoir WHERE client_id = ?1 AND statut = 'ouvert'",
        rusqlite::params![client_id], |r| r.get(0),
    ).unwrap_or(0);

    Ok(serde_json::json!({
        "tiers":     client,
        "lignes":    lignes,
        "total_du":  total_du,
        "avoirs":    avoirs,
        "net_du":    (total_du - avoirs).max(0),
        "societe":   societe(&conn),
    }))
}

/// Etat GLOBAL des creances — un client par ligne, tous confondus.
///
/// Le reste se calcule PAR VENTE puis s'additionne, jamais
/// `SUM(total) - SUM(paye)` : le seuil de solde (D41) s'applique a
/// chaque vente. Une vente soldee a 3 F pres et une autre a 3 F pres
/// feraient sinon reapparaitre une creance de 6 F qui n'existe pas.
#[tauri::command]
pub fn lire_etat_creances_global(
    etat: State<EtatApp>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let mut st = conn.prepare(
        "SELECT c.id, c.nom, c.code, c.telephone, v.id,
                CAST(COALESCE(SUM(lv.prix_pratique * lv.quantite), 0) AS INTEGER),
                CAST(COALESCE(
                  (SELECT SUM(montant) FROM paiement WHERE vente_id = v.id), 0
                ) AS INTEGER)
         FROM vente v
         JOIN client c ON c.id = v.client_id
         LEFT JOIN ligne_vente lv ON lv.vente_id = v.id
         WHERE v.statut IN ('creance_ouverte','partiellement_payee')
         GROUP BY v.id
         ORDER BY c.nom, v.date_vente"
    ).map_err(|e| e.to_string())?;

    // (client_id) -> (nom, code, tel, nb_factures, total_du)
    let mut par_client: Vec<(String, String, String, Option<String>, i64, i64)> = Vec::new();

    let ventes = st.query_map([], |r| {
        let total: i64 = r.get(5)?;
        let paye: i64 = r.get(6)?;
        Ok((
            r.get::<_, String>(0)?, r.get::<_, String>(1)?,
            r.get::<_, String>(2)?, r.get::<_, Option<String>>(3)?,
            crate::coeur::calcul::reste_exigible(total, paye),
        ))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok());

    for (id, nom, code, tel, reste) in ventes {
        if reste <= 0 {
            continue;
        }
        match par_client.iter_mut().find(|l| l.0 == id) {
            Some(l) => { l.4 += 1; l.5 += reste; }
            None => par_client.push((id, nom, code, tel, 1, reste)),
        }
    }

    // Le plus gros debiteur en premier : c'est celui qu'on appelle.
    par_client.sort_by(|a, b| b.5.cmp(&a.5));

    let total_general: i64 = par_client.iter().map(|l| l.5).sum();
    let lignes: Vec<serde_json::Value> = par_client.into_iter()
        .map(|(_, nom, code, tel, nb, du)| serde_json::json!({
            "nom": nom, "code": code, "telephone": tel,
            "nb": nb, "total_du": du,
        })).collect();

    Ok(serde_json::json!({
        "lignes":        lignes,
        "total_general": total_general,
        "societe":       societe(&conn),
    }))
}

/// En-tete societe, commun aux documents imprimes.
pub(crate) fn societe(conn: &rusqlite::Connection) -> serde_json::Value {
    conn.query_row(
        "SELECT nom, adresse, telephone FROM parametres_societe WHERE id = 1",
        [], |r| Ok(serde_json::json!({
            "nom":       r.get::<_, String>(0)?,
            "adresse":   r.get::<_, Option<String>>(1)?,
            "telephone": r.get::<_, Option<String>>(2)?,
        })),
    ).unwrap_or(serde_json::json!({
        "nom": "", "adresse": null, "telephone": null
    }))
}

/// Lire toutes les ventes avec créance ouverte ou partielle.
#[tauri::command]
pub fn lire_creances_ouvertes(
    etat: State<EtatApp>,
    recherche: Option<String>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let filtre = recherche.as_deref().unwrap_or("").to_lowercase();

    let mut stmt = conn.prepare(
        "SELECT v.id, v.date_vente, v.statut,
                c.id as client_id, c.nom, c.code, c.telephone,
                p.numero as numero_facture,
                CAST(COALESCE(SUM(lv.prix_pratique * lv.quantite), 0) AS INTEGER) as total,
                CAST(COALESCE(
                    (SELECT SUM(montant) FROM paiement WHERE vente_id = v.id), 0
                ) AS INTEGER) as total_paye
         FROM vente v
         JOIN client c ON c.id = v.client_id
         LEFT JOIN ligne_vente lv ON lv.vente_id = v.id
         LEFT JOIN piece_commerciale p ON p.id = v.piece_id
         WHERE v.statut IN ('creance_ouverte', 'partiellement_payee')
         GROUP BY v.id
         ORDER BY v.date_vente DESC"
    ).map_err(|e| e.to_string())?;

    let x = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,   // vente id
            row.get::<_, String>(1)?,   // date_vente
            row.get::<_, String>(2)?,   // statut
            row.get::<_, String>(3)?,   // client_id
            row.get::<_, String>(4)?,   // client nom
            row.get::<_, String>(5)?,   // client code
            row.get::<_, Option<String>>(6)?,  // telephone
            row.get::<_, Option<String>>(7)?,  // numero_facture
            row.get::<_, i64>(8)?,      // total
            row.get::<_, i64>(9)?,      // total_paye
        ))
    })
    .map_err(|e| e.to_string())?
    .filter_map(|r| r.ok())
    .filter(|(_, _, _, _, nom, _, _, _, _, _)| {
        filtre.is_empty() || nom.to_lowercase().contains(&filtre)
    })
    .map(|(id, date, statut, client_id, client_nom, client_code,
           telephone, numero_facture, total, total_paye)| {
        let reste = crate::coeur::calcul::reste_exigible(total, total_paye);
        serde_json::json!({
            "id":             id.clone(),
            "vente_id":       id,
            "date_vente":     date,
            "statut":         statut,
            "client_id":      client_id,
            "client_nom":     client_nom,
            "client_code":    client_code,
            "telephone":      telephone,
            "numero_facture": numero_facture,
            "total":          total,
            "total_paye":     total_paye,
            "reste":          reste,
        })
    })
    // Un residu d'arrondi n'est plus une creance : l'afficher a 0 dans
    // la liste des impayes n'aurait aucun sens. `solder_residus_creances`
    // remet leur statut a jour.
    .filter(|v| v["reste"].as_i64().unwrap_or(0) > 0)
    .collect();

    Ok(x)
}

/// Enregistrer un paiement sur une créance existante.
/// Met à jour le statut de la vente automatiquement.
#[tauri::command]
pub fn regler_creance(
    etat: State<EtatApp>,
    vente_id: String,
    montant: i64,
    mode: String,
    utilisateur_role: Option<String>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let role = utilisateur_role.as_deref().unwrap_or("patron");
    let auteur_id = crate::commandes::ventes::id_utilisateur_par_role(&conn, role);
    let maintenant = maintenant_iso();

    if montant <= 0 {
        return Err("Le montant doit être positif".to_string());
    }

    // Vérifier que la vente existe et a un reste dû
    let (total, total_paye_avant): (i64, i64) = conn.query_row(
        "SELECT
            CAST(COALESCE(SUM(lv.prix_pratique * lv.quantite), 0) AS INTEGER),
            CAST(COALESCE((SELECT SUM(montant) FROM paiement WHERE vente_id = v.id), 0) AS INTEGER)
         FROM vente v
         LEFT JOIN ligne_vente lv ON lv.vente_id = v.id
         WHERE v.id = ?1
         GROUP BY v.id",
        rusqlite::params![vente_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    ).map_err(|_| "Vente introuvable".to_string())?;

    // Encaissement reel : la caisse doit etre ouverte. Un avoir ne
    // touche pas au tiroir et reste autorise.
    if mode != "avoir" {
        crate::utils::exiger_session_caisse(&conn)?;
    }

    let reste_avant = crate::coeur::calcul::reste_exigible(total, total_paye_avant);
    if reste_avant <= 0 {
        return Err("Cette vente est déjà entièrement payée".to_string());
    }

    // Limiter le paiement au reste dû (pas de surpayement)
    let montant_effectif = montant.min(reste_avant);

    // Insérer le paiement
    conn.execute(
        "INSERT INTO paiement
         (id, vente_id, montant, mode, date_paiement, auteur_id, cree_le, cree_par, origine)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'app')",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(),
            vente_id, montant_effectif, mode,
            maintenant, auteur_id, maintenant, auteur_id
        ],
    ).map_err(|e| e.to_string())?;

    // Calculer le nouveau total payé
    let total_paye_apres = total_paye_avant + montant_effectif;
    // Regle unique, via le coeur teste — une comparaison locale
    // `>= total` ignorerait le seuil et rouvrirait la creance.
    let nouveau_statut = match crate::coeur::calcul::statut_vente(total, total_paye_apres) {
        crate::coeur::calcul::StatutVente::Payee => "payee",
        _ => "partiellement_payee",
    };
    let residu = total - total_paye_apres;

    // Mettre à jour le statut de la vente
    conn.execute(
        "UPDATE vente SET statut = ?1, modifie_le = ?2 WHERE id = ?3",
        rusqlite::params![nouveau_statut, maintenant, vente_id],
    ).map_err(|e| e.to_string())?;

    // Répercuter sur la pièce commerciale liée.
    // Sans cela une facture émise restait "emis" indéfiniment, même
    // réglée — impossible de distinguer un impayé d'une facture soldée.
    // Symétrique de l'imputation côté fournisseur.
    if nouveau_statut == "payee" {
        conn.execute(
            "UPDATE piece_commerciale
             SET statut = 'paye', modifie_le = ?1
             WHERE id = (SELECT piece_id FROM vente WHERE id = ?2)
               AND statut IN ('emis','accepte','brouillon')",
            rusqlite::params![maintenant, vente_id],
        ).ok();
    }

    // Alimenter la caisse si session ouverte
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
                 VALUES (?1, ?2, 'entree', ?3, ?4, 'vente', ?5, ?6, ?7, ?8, 'app')",
                rusqlite::params![
                    uuid::Uuid::new_v4().to_string(),
                    sid, mode, montant_effectif, vente_id,
                    maintenant, maintenant, auteur_id
                ],
            ).ok();
        }
    }

    // Trace obligatoire : sans elle, l'ecart entre CA et encaisse
    // devient inexplicable au rapprochement (D41).
    if nouveau_statut == "payee" && residu > 0 {
        conn.execute(
            "INSERT INTO journal
             (id, type_evenement, entite_type, entite_id, auteur_id,
              nouveau_valeur, origine, date_evenement)
             VALUES (?1, 'residu_absorbe', 'vente', ?2, ?3, ?4, 'app', ?5)",
            rusqlite::params![
                uuid::Uuid::new_v4().to_string(),
                vente_id, auteur_id,
                format!(r#"{{"residu":{}}}"#, residu),
                maintenant
            ],
        ).ok();
    }

    // Journal
    conn.execute(
        "INSERT INTO journal
         (id, type_evenement, entite_type, entite_id, auteur_id,
          nouveau_valeur, origine, date_evenement)
         VALUES (?1, 'creance_reglee', 'vente', ?2, ?3, ?4, 'app', ?5)",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(),
            vente_id, auteur_id,
            format!(r#"{{"montant":{},"mode":"{}","statut":"{}"}}"#,
                montant_effectif, mode, nouveau_statut),
            maintenant
        ],
    ).ok();

    Ok(serde_json::json!({
        "montant_encaisse":  montant_effectif,
        "reste_apres":       crate::coeur::calcul::reste_exigible(total, total_paye_apres),
        "residu_absorbe":    if nouveau_statut == "payee" { residu.max(0) } else { 0 },
        "statut":            nouveau_statut,
        "soldee":            nouveau_statut == "payee",
    }))
}

/// Solde les créances devenues non recouvrables (résidus d'arrondi, D41).
///
/// Rattrapage unique : le seuil ne vaut que pour les statuts écrits
/// après lui. Aucun paiement créé, aucun mouvement de caisse — statut
/// seul, plus une trace au journal.
#[tauri::command]
pub fn solder_residus_creances(
    etat: State<EtatApp>,
    utilisateur_role: Option<String>,
    // true = simulation, rien n'est écrit. À passer d'abord.
    simulation: Option<bool>,
) -> Result<serde_json::Value, String> {
    if utilisateur_role.as_deref() != Some("patron") {
        return Err("Réservé au patron".to_string());
    }
    let simule = simulation.unwrap_or(true);
    let mut conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let maintenant = maintenant_iso();
    let auteur_id = crate::commandes::ventes::id_utilisateur_courant_pub(&conn);

    let candidates: Vec<(String, i64, i64)> = {
        let mut st = conn.prepare(
            "SELECT v.id,
                    CAST(COALESCE((SELECT SUM(lv.prix_pratique * lv.quantite)
                      FROM ligne_vente lv WHERE lv.vente_id = v.id), 0) AS INTEGER),
                    CAST(COALESCE((SELECT SUM(p.montant)
                      FROM paiement p WHERE p.vente_id = v.id), 0) AS INTEGER)
             FROM vente v
             WHERE v.statut IN ('creance_ouverte','partiellement_payee')"
        ).map_err(|e| e.to_string())?;

        // `let v = …; v` et non la chaine en fin de bloc : sinon le
        // temporaire de query_map survit a `st` -> E0597.
        let v = st.query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        // `paye > 0` : sans encaissement il n'y a pas de residu, juste
        // une petite vente impayee. `reste > 0` ecarte le trop-percu,
        // qui est un autre probleme et ne doit pas se fermer en silence.
        .filter(|(_, total, paye)| {
            let reste = total - paye;
            *paye > 0 && reste > 0 && reste <= crate::coeur::calcul::SEUIL_SOLDE
        })
        .map(|(id, total, paye)| (id, total - paye, paye))
        .collect();
        v
    };

    let total_absorbe: i64 = candidates.iter().map(|(_, r, _)| r).sum();

    if !simule {
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        for (vente_id, residu, _) in &candidates {
            tx.execute(
                "UPDATE vente SET statut = 'payee', modifie_le = ?1 WHERE id = ?2",
                rusqlite::params![maintenant, vente_id],
            ).map_err(|e| e.to_string())?;

            tx.execute(
                "UPDATE piece_commerciale
                 SET statut = 'paye', modifie_le = ?1
                 WHERE id = (SELECT piece_id FROM vente WHERE id = ?2)
                   AND statut IN ('emis','accepte','brouillon')",
                rusqlite::params![maintenant, vente_id],
            ).ok();

            tx.execute(
                "INSERT INTO journal
                 (id, type_evenement, entite_type, entite_id, auteur_id,
                  nouveau_valeur, origine, date_evenement)
                 VALUES (?1,'residu_absorbe','vente',?2,?3,?4,'maintenance',?5)",
                rusqlite::params![
                    uuid::Uuid::new_v4().to_string(), vente_id, auteur_id,
                    format!(r#"{{"residu":{}}}"#, residu), maintenant
                ],
            ).ok();
        }
        tx.commit().map_err(|e| e.to_string())?;
    }

    Ok(serde_json::json!({
        "simulation":    simule,
        "concernees":    candidates.len(),
        "total_absorbe": total_absorbe,
        "seuil":         crate::coeur::calcul::SEUIL_SOLDE,
    }))
}