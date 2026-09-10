//! Commandes pour le règlement des créances existantes.

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
pub fn lire_etat_creances_client(
    conn: &rusqlite::Connection,
    client_id: String,
) -> Result<serde_json::Value, String> {

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
pub fn lire_etat_creances_global(
    conn: &rusqlite::Connection,
) -> Result<serde_json::Value, String> {

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

/// Historique des reglements d'un client — ce qu'on lui montre quand il
/// conteste.
///
/// Les annulations y figurent comme des lignes a part entiere, montant
/// negatif : le client voit ce qui avait ete enregistre ET la
/// correction. C'est tout l'interet de ne jamais supprimer.
pub fn lire_reglements_client(
    conn: &rusqlite::Connection,
    client_id: String,
) -> Result<Vec<serde_json::Value>, String> {

    let mut st = conn.prepare(
        "SELECT p.id, p.montant, p.mode, p.date_paiement,
                COALESCE(u.nom, '—'),
                v.id, COALESCE(pc.numero, ''), v.date_vente,
                p.annule_paiement_id,
                -- Un reglement deja annule porte une contre-passation
                -- qui le designe : on ne l'annule pas deux fois.
                EXISTS (SELECT 1 FROM paiement a
                        WHERE a.annule_paiement_id = p.id) AS deja_annule,
                -- Total de la facture, et cumul verse JUSQU'A CE
                -- REGLEMENT INCLUS. Leur difference donne le solde tel
                -- qu'il etait juste apres ce versement — la dette qui
                -- descend ligne par ligne, ce que le client vient
                -- verifier.
                CAST(COALESCE((SELECT SUM(lv.prix_pratique * lv.quantite)
                               FROM ligne_vente lv WHERE lv.vente_id = v.id), 0)
                     AS INTEGER),
                CAST(COALESCE((SELECT SUM(p2.montant) FROM paiement p2
                               WHERE p2.vente_id = v.id
                                 AND p2.date_paiement <= p.date_paiement), 0)
                     AS INTEGER)
         FROM paiement p
         JOIN vente v ON v.id = p.vente_id
         LEFT JOIN piece_commerciale pc ON pc.id = v.piece_id
         LEFT JOIN utilisateur u ON u.id = p.auteur_id
         WHERE v.client_id = ?1
         ORDER BY p.date_paiement DESC"
    ).map_err(|e| e.to_string())?;

    let x = st.query_map(rusqlite::params![client_id], |r| {
        let montant: i64 = r.get(1)?;
        let total_facture: i64 = r.get(10)?;
        let paye_cumule: i64 = r.get(11)?;
        Ok(serde_json::json!({
            "id":             r.get::<_, String>(0)?,
            "montant":        montant,
            "mode":           r.get::<_, String>(2)?,
            "date_paiement":  r.get::<_, String>(3)?,
            "auteur_nom":     r.get::<_, String>(4)?,
            "vente_id":       r.get::<_, String>(5)?,
            "numero_facture": r.get::<_, String>(6)?,
            "date_vente":     r.get::<_, String>(7)?,
            // Cette ligne EST une annulation.
            "est_annulation": r.get::<_, Option<String>>(8)?.is_some(),
            // Cette ligne A ETE annulee par une autre.
            "deja_annule":    r.get::<_, i64>(9)? != 0,
            // Passe par le coeur (invariant 12) : un residu d'arrondi
            // sous le seuil ne doit pas s'afficher comme une dette.
            "reste_apres":    crate::coeur::calcul::reste_exigible(
                                  total_facture, paye_cumule),
        }))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    Ok(x)
}

/// Annule un reglement par contre-passation — jamais par suppression.
///
/// `remboursement` distingue les deux seuls cas reels :
///   - `false` : erreur de saisie, l'argent n'est jamais entre.
///   - `true`  : le client est rembourse, l'argent ressort du tiroir.
///
/// C'est `coeur::calcul::effet_caisse_annulation` qui tranche ce que la
/// caisse doit encaisser de tout ca — regle pure, testee.
pub fn annuler_reglement(
    conn: &rusqlite::Connection,
    paiement_id: String,
    motif: String,
    remboursement: bool,
    utilisateur_role: Option<String>,
) -> Result<serde_json::Value, String> {
    if motif.trim().is_empty() {
        return Err("Le motif est obligatoire : c'est lui qui explique \
                    la correction au client et au controle.".to_string());
    }
    let role = utilisateur_role.as_deref().unwrap_or("patron");
    let auteur_id = crate::argent::id_utilisateur_par_role(&conn, role);
    let maintenant = maintenant_iso();

    let (montant, mode, vente_id, date_paiement, est_annulation):
        (i64, String, String, String, bool) = conn.query_row(
        "SELECT montant, mode, vente_id, date_paiement,
                annule_paiement_id IS NOT NULL
         FROM paiement WHERE id = ?1",
        rusqlite::params![paiement_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get::<_, i64>(4)? != 0)),
    ).map_err(|_| "Règlement introuvable".to_string())?;

    if est_annulation {
        return Err("Cette ligne est déjà une annulation — on n'annule \
                    pas une annulation.".to_string());
    }

    let deja: i64 = conn.query_row(
        "SELECT COUNT(*) FROM paiement WHERE annule_paiement_id = ?1",
        rusqlite::params![paiement_id], |r| r.get(0),
    ).unwrap_or(0);
    if deja > 0 {
        return Err("Ce règlement a déjà été annulé.".to_string());
    }

    // Le paiement a-t-il ete saisi pendant la session encore ouverte ?
    // C'est ce qui decide si sa ligne de caisse est encore corrigeable
    // ou si la cloture l'a deja absorbee.
    let dans_session_ouverte: bool = conn.query_row(
        "SELECT COUNT(*) FROM session_caisse
         WHERE statut = 'ouverte' AND ?1 >= cree_le",
        rusqlite::params![date_paiement], |r| r.get::<_, i64>(0),
    ).map(|n| n > 0).unwrap_or(false);

    let effet = crate::coeur::calcul::effet_caisse_annulation(
        remboursement, dans_session_ouverte, mode != "avoir");
    let sort_de_caisse = effet == crate::coeur::calcul::EffetCaisse::ContrePassation;

    // Caisse ouverte exigee UNIQUEMENT si de l'argent bouge (D46) :
    // corriger une erreur de saisie sur une session close ne touche pas
    // au tiroir, et ne doit donc pas etre bloque par une caisse fermee.
    let session_id = if sort_de_caisse {
        Some(crate::utils::exiger_session_caisse(&conn)?)
    } else {
        None
    };

    // ---- La contre-passation ----
    conn.execute(
        "INSERT INTO paiement
         (id, vente_id, montant, mode, date_paiement, auteur_id,
          cree_le, cree_par, origine, annule_paiement_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'annulation', ?9)",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(), vente_id, -montant, mode,
            maintenant, auteur_id, maintenant, auteur_id, paiement_id
        ],
    ).map_err(|e| e.to_string())?;

    // ---- Statuts recalcules sur les paiements REELS (D36) ----
    let (total, total_paye): (i64, i64) = conn.query_row(
        "SELECT
            CAST(COALESCE(SUM(lv.prix_pratique * lv.quantite), 0) AS INTEGER),
            CAST(COALESCE((SELECT SUM(montant) FROM paiement
                           WHERE vente_id = v.id), 0) AS INTEGER)
         FROM vente v
         LEFT JOIN ligne_vente lv ON lv.vente_id = v.id
         WHERE v.id = ?1 GROUP BY v.id",
        rusqlite::params![vente_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    ).map_err(|e| e.to_string())?;

    let statut = match crate::coeur::calcul::statut_vente(total, total_paye) {
        crate::coeur::calcul::StatutVente::Payee => "payee",
        crate::coeur::calcul::StatutVente::PartiellementPayee => "partiellement_payee",
        crate::coeur::calcul::StatutVente::CreanceOuverte => "creance_ouverte",
    };
    conn.execute(
        "UPDATE vente SET statut = ?1, modifie_le = ?2 WHERE id = ?3",
        rusqlite::params![statut, maintenant, vente_id],
    ).map_err(|e| e.to_string())?;

    // La facture soldee redevient due. Sans ca elle resterait 'paye'
    // avec un reste au tableau : les deux ecrans se contrediraient.
    if statut != "payee" {
        conn.execute(
            "UPDATE piece_commerciale SET statut = 'emis', modifie_le = ?1
             WHERE id = (SELECT piece_id FROM vente WHERE id = ?2)
               AND statut = 'paye'",
            rusqlite::params![maintenant, vente_id],
        ).ok();
    }

    // ---- Caisse ----
    if let Some(sid) = session_id {
        conn.execute(
            "INSERT INTO mouvement_caisse
             (id, session_id, sens, moyen, montant, motif, libelle,
              operation_id, date_mouvement, cree_le, cree_par, origine)
             VALUES (?1,?2,'sortie',?3,?4,'remboursement',?5,?6,?7,?8,?9,'annulation')",
            rusqlite::params![
                uuid::Uuid::new_v4().to_string(), sid, mode, montant,
                format!("Annulation règlement — {}", motif.trim()),
                vente_id, maintenant, maintenant, auteur_id
            ],
        ).map_err(|e| e.to_string())?;
    }

    conn.execute(
        "INSERT INTO journal
         (id, type_evenement, entite_type, entite_id, auteur_id,
          ancien_valeur, nouveau_valeur, origine, date_evenement)
         VALUES (?1,'reglement_annule','paiement',?2,?3,?4,?5,'app',?6)",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(), paiement_id, auteur_id,
            format!(r#"{{"montant":{}}}"#, montant),
            format!(
                r#"{{"motif":"{}","remboursement":{},"caisse":"{}"}}"#,
                motif.trim().replace('"', "'"), remboursement,
                if sort_de_caisse { "sortie" } else { "aucune" }),
            maintenant
        ],
    ).ok();

    Ok(serde_json::json!({
        "montant_annule":  montant,
        "sortie_de_caisse": sort_de_caisse,
        "nouveau_statut":  statut,
        "reste_du":        crate::coeur::calcul::reste_exigible(total, total_paye),
    }))
}

/// Donnees d'un recu de reglement — client ou fournisseur.
///
/// Le recu n'est PAS une piece commerciale de plus : ni numero de serie,
/// ni ligne en base. C'est une VUE d'un paiement qui existe deja, comme
/// le bon de sortie est une vue d'une facture. Le numeroter en ferait un
/// document a serie continue (D28), avec tout ce que ca impose — pour
/// un papier qui ne fait que constater ce que `paiement` enregistre.
///
/// Une seule commande pour les deux cotes : la difference tient au sens
/// de l'argent, pas au calcul. Le `reste_du` est celui de la facture
/// APRES ce reglement — c'est le chiffre que le tiers vient verifier.
pub fn lire_donnees_recu(
    conn: &rusqlite::Connection,
    paiement_id: String,
    // "client" | "fournisseur"
    cote: String,
) -> Result<serde_json::Value, String> {

    if cote == "fournisseur" {
        let (montant, mode, date_p, note, auteur, f_nom, f_tel, f_adr, numero):
            (i64, String, String, Option<String>, Option<String>,
             String, Option<String>, Option<String>, Option<String>) =
            conn.query_row(
                "SELECT pf.montant, pf.mode, pf.date_paiement, pf.note,
                        u.nom, f.nom, f.telephone, f.adresse, pc.numero
                 FROM paiement_fournisseur pf
                 JOIN fournisseur f ON f.id = pf.fournisseur_id
                 LEFT JOIN utilisateur u ON u.id = pf.auteur_id
                 LEFT JOIN piece_commerciale pc ON pc.id = pf.piece_id
                 WHERE pf.id = ?1",
                rusqlite::params![paiement_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?,
                        r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?)),
            ).map_err(|_| "Règlement introuvable".to_string())?;

        // Reste du sur LA FACTURE imputee, s'il y en a une. Un reglement
        // non impute (avance) n'a pas de facture a solder.
        let reste: Option<i64> = conn.query_row(
            "SELECT CAST(COALESCE(SUM(lp.montant_ht + lp.montant_tva), 0)
                    - COALESCE((SELECT SUM(montant) FROM paiement_fournisseur
                                WHERE piece_id = pc.id), 0) AS INTEGER)
             FROM piece_commerciale pc
             JOIN ligne_piece lp ON lp.piece_id = pc.id
             WHERE pc.id = (SELECT piece_id FROM paiement_fournisseur WHERE id = ?1)
             GROUP BY pc.id",
            rusqlite::params![paiement_id], |r| r.get(0),
        ).ok();

        return Ok(serde_json::json!({
            "cote":      "fournisseur",
            "montant":   montant,
            "mode":      mode,
            "date":      date_p,
            "note":      note,
            "auteur":    auteur.unwrap_or_else(|| "—".into()),
            "reference": numero.unwrap_or_default(),
            "reste_du":  reste,
            "tiers": {
                "nom": f_nom, "code": "",
                "telephone": f_tel, "adresse": f_adr,
            },
            "societe":   societe(&conn),
        }));
    }

    // ---- Cote client ----
    let (montant, mode, date_p, auteur, vente_id, numero, c_nom, c_code,
         c_tel, c_adr, est_annulation):
        (i64, String, String, Option<String>, String, Option<String>,
         String, String, Option<String>, Option<String>, bool) =
        conn.query_row(
            "SELECT p.montant, p.mode, p.date_paiement, u.nom,
                    v.id, pc.numero, c.nom, c.code, c.telephone, c.adresse,
                    p.annule_paiement_id IS NOT NULL
             FROM paiement p
             JOIN vente v ON v.id = p.vente_id
             JOIN client c ON c.id = v.client_id
             LEFT JOIN piece_commerciale pc ON pc.id = v.piece_id
             LEFT JOIN utilisateur u ON u.id = p.auteur_id
             WHERE p.id = ?1",
            rusqlite::params![paiement_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?,
                    r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?, r.get(9)?,
                    r.get::<_, i64>(10)? != 0)),
        ).map_err(|_| "Règlement introuvable".to_string())?;

    // Reste du sur la vente, tous paiements confondus — donc APRES
    // celui-ci, et net de toute annulation (D36).
    let (total, paye): (i64, i64) = conn.query_row(
        "SELECT CAST(COALESCE(SUM(lv.prix_pratique * lv.quantite), 0) AS INTEGER),
                CAST(COALESCE((SELECT SUM(montant) FROM paiement
                               WHERE vente_id = v.id), 0) AS INTEGER)
         FROM vente v
         LEFT JOIN ligne_vente lv ON lv.vente_id = v.id
         WHERE v.id = ?1 GROUP BY v.id",
        rusqlite::params![vente_id], |r| Ok((r.get(0)?, r.get(1)?)),
    ).unwrap_or((0, 0));

    Ok(serde_json::json!({
        "cote":      "client",
        "montant":   montant,
        "mode":      mode,
        "date":      date_p,
        "note":      serde_json::Value::Null,
        "auteur":    auteur.unwrap_or_else(|| "—".into()),
        "reference": numero.unwrap_or_default(),
        "reste_du":  crate::coeur::calcul::reste_exigible(total, paye),
        // Un recu tire d'une annulation doit le dire : c'est un
        // justificatif de correction, pas un encaissement.
        "est_annulation": est_annulation,
        "tiers": {
            "nom": c_nom, "code": c_code,
            "telephone": c_tel, "adresse": c_adr,
        },
        "societe":   societe(&conn),
    }))
}

/// En-tete societe, commun aux documents imprimes.
pub fn societe(conn: &rusqlite::Connection) -> serde_json::Value {
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
pub fn lire_creances_ouvertes(
    conn: &rusqlite::Connection,
    recherche: Option<String>,
) -> Result<Vec<serde_json::Value>, String> {

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
pub fn regler_creance(
    conn: &rusqlite::Connection,
    vente_id: String,
    montant: i64,
    mode: String,
    utilisateur_role: Option<String>,
) -> Result<serde_json::Value, String> {
    let role = utilisateur_role.as_deref().unwrap_or("patron");
    let auteur_id = crate::argent::id_utilisateur_par_role(&conn, role);
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
pub fn solder_residus_creances(
    conn: &mut rusqlite::Connection,
    utilisateur_role: Option<String>,
    // true = simulation, rien n'est écrit. À passer d'abord.
    simulation: Option<bool>,
) -> Result<serde_json::Value, String> {
    if utilisateur_role.as_deref() != Some("patron") {
        return Err("Réservé au patron".to_string());
    }
    let simule = simulation.unwrap_or(true);
    let maintenant = maintenant_iso();
    let auteur_id = crate::argent::id_utilisateur_courant_pub(&conn);

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