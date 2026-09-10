//! Achats fournisseur — entrée de stock + facture fournisseur (FAF).
//!
//! Un achat produit UN document : une pièce `facture_fournisseur`.
//! C'est elle qui porte la dette — plus aucun calcul dérivé de
//! `mouvement_stock`. Bons de commande et bons de réception restent du
//! ressort du fournisseur, ils ne sont pas générés ici.
//!
//! Tout se fait dans une transaction unique : si une ligne échoue,
//! ni le stock ni la facture ne bougent.

use serde::Deserialize;
use crate::utils::maintenant_iso;

#[derive(Deserialize)]
pub struct LigneAchat {
    pub article_id: String,
    pub unite_vente_id: String,
    /// Quantité dans l'unité de vente choisie (carton, sac...).
    pub quantite: f64,
    /// Combien d'unités de base vaut cette unité.
    pub facteur: f64,
    /// Prix d'achat unitaire, pour l'unité choisie.
    pub prix_achat: i64,
}

/// Enregistre un achat complet : stock, mouvements, facture fournisseur.
///
/// - `mode_reglement` = "comptant" → le total est réglé immédiatement,
///   la dette retombe à zéro.
/// - `mode_reglement` = "credit"   → la dette reste ouverte, sauf si un
///   `acompte` est donné : montant réglé tout de suite, le reste reste dû.
///   Même logique que `valider_facture` côté vente (D30 étendu).
///
/// Sans `fournisseur_id`, le stock est mis à jour mais aucune facture
/// n'est créée (régularisation interne).
pub fn enregistrer_achat(
    conn: &mut rusqlite::Connection,
    fournisseur_id: Option<String>,
    depot_id: Option<String>,
    lignes: Vec<LigneAchat>,
    mode_reglement: Option<String>,
    // especes | orange_money | moov_money | cheque — pour le comptant/acompte
    mode_paiement: Option<String>,
    // N'a de sens que si mode_reglement = "credit". Ignoré sinon.
    acompte: Option<i64>,
    note: Option<String>,
    utilisateur_role: Option<String>,
    // Trace la piece d'origine (bon_reception) quand cet achat en decoule.
    // Optionnel : un achat direct (sans BCF/BRF prealable) n'en a pas.
    piece_origine_id: Option<String>,
) -> Result<serde_json::Value, String> {
    if lignes.is_empty() {
        return Err("Aucune ligne à enregistrer".to_string());
    }
    let now = maintenant_iso();
    let role = utilisateur_role.as_deref().unwrap_or("employe");
    let auteur = crate::argent::id_utilisateur_par_role(&conn, role);

    // Résoudre le dépôt avant d'ouvrir la transaction.
    let depot_id = match depot_id {
        Some(d) if !d.is_empty() => d,
        _ => conn.query_row(
            "SELECT id FROM depot WHERE est_defaut = 1 LIMIT 1",
            [], |r| r.get(0),
        ).map_err(|_| "Aucun dépôt par défaut configuré".to_string())?,
    };

    // Un bon de reception ne se facture qu'UNE fois — meme garde-fou que
    // cote client (coeur::pieces::peut_transferer). Sans lui, deux clics
    // sur « facturer » depuis le meme BRF creaient deux FAF : dette
    // doublee envers le fournisseur, et un second decaissement au
    // reglement. Le cote client etait verrouille, pas celui-ci.
    if let Some(ref src) = piece_origine_id {
        let statut_src: String = conn.query_row(
            "SELECT statut FROM piece_commerciale WHERE id = ?1",
            rusqlite::params![src], |r| r.get(0),
        ).map_err(|_| "Pièce d'origine introuvable".to_string())?;

        let deja: Option<String> = conn.query_row(
            "SELECT numero FROM piece_commerciale
             WHERE piece_origine_id = ?1
               AND statut <> 'annule'
               AND type_piece NOT IN ('avoir_client','avoir_fournisseur')
             ORDER BY cree_le LIMIT 1",
            rusqlite::params![src], |r| r.get(0),
        ).ok();

        crate::coeur::pieces::peut_transferer(&statut_src, deja.as_deref())?;
    }

    // Caisse fermee : on REFUSE au lieu d'ecrire l'achat sans sa sortie
    // de caisse. C'etait le trou — la session etait lue plus bas, et si
    // elle etait fermee le mouvement etait simplement omis, en silence.
    // L'argent quittait le tiroir, la facture le disait, la caisse non :
    // le comptage du soir tombait faux d'exactement ce montant, sans rien
    // pour l'expliquer. D46, deja applique a chaque encaissement cote
    // vente, vaut evidemment aussi pour un decaissement.
    let comptant = mode_reglement.as_deref() == Some("comptant");
    if comptant || acompte.unwrap_or(0) > 0 {
        crate::utils::exiger_session_caisse(&conn)?;
    }

    // Numéro réservé avant la transaction (lecture seule).
    let numero = if fournisseur_id.is_some() {
        Some(crate::argent::prochain_numero(&conn, "facture_fournisseur"))
    } else {
        None
    };

    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let op_id = uuid::Uuid::new_v4().to_string();
    // Généré tôt pour pouvoir servir d'`operation_id` sur les mouvements de
    // stock ci-dessous : c'est ce qui permet de retrouver le numéro de
    // facture d'un mouvement (jointure piece_commerciale) sans avoir à
    // rejouer tout l'achat. Sans fournisseur, pas de facture : on retombe
    // sur op_id, un simple identifiant de lot sans numéro associé.
    let piece_id = fournisseur_id.as_ref().map(|_| uuid::Uuid::new_v4().to_string());
    let operation_id = piece_id.clone().unwrap_or_else(|| op_id.clone());
    let mut total: i64 = 0;

    // Calcule apres coup (total connu seulement une fois les lignes lues) —
    // voir plus bas, juste avant la creation de la facture.

    // ---- 1. Stock et mouvements ----
    for l in &lignes {
        let quantite_base = l.quantite * l.facteur;
        let montant = (l.prix_achat as f64 * l.quantite).round() as i64;
        total += montant;

        // Le stock suit desormais son mouvement : le declencheur
        // `stock_suit_les_mouvements` met le compteur a jour dans la meme
        // transaction. L'ecrire ici le compterait deux fois.

        // Prix ramené à l'unité de base, pour rester homogène avec
        // quantite_delta qui est lui aussi en unité de base.
        let prix_base = if l.facteur > 0.0 {
            (l.prix_achat as f64 / l.facteur).round() as i64
        } else { l.prix_achat };

        tx.execute(
            "INSERT INTO mouvement_stock
             (id, article_id, depot_id, type_mouvement, quantite_delta,
              operation_id, auteur_id, date_mouvement, cree_le, cree_par, origine,
              fournisseur_id, prix_achat_unitaire)
             VALUES (?1,?2,?3,'achat',?4,?5,?6,?7,?8,?9,'app',?10,?11)",
            rusqlite::params![
                uuid::Uuid::new_v4().to_string(),
                l.article_id, depot_id, quantite_base,
                operation_id, auteur, now, now, auteur,
                fournisseur_id, prix_base
            ],
        ).map_err(|e| e.to_string())?;

        tx.execute(
            "UPDATE article SET dernier_prix_achat = ?1, modifie_le = ?2 WHERE id = ?3",
            rusqlite::params![prix_base, now, l.article_id],
        ).ok();
    }

    // Montant reellement regle a cet instant — meme principe que
    // valider_facture cote vente (pieces.rs) : le statut se DEDUIT de ce
    // qui est encaisse, jamais du mode declare seul.
    //   comptant -> tout le total.
    //   credit   -> l'acompte donne, plafonne au total (0 si absent).
    let montant_regle: i64 = if mode_reglement.as_deref() == Some("comptant") {
        total
    } else {
        acompte.unwrap_or(0).clamp(0, total)
    };

    // ---- 2. Facture fournisseur ----
    let mut piece_id_retour = serde_json::Value::Null;
    let mut numero_retour = serde_json::Value::Null;

    if let (Some(f_id), Some(num), Some(piece_id)) =
        (fournisseur_id.as_ref(), numero.as_ref(), piece_id.as_ref())
    {
        // Une facture fournisseur arrive du fournisseur : elle est ferme
        // des sa reception. Son statut reflete donc le REGLEMENT, pas un
        // cycle de validation interne.
        //   soldee (comptant, ou acompte >= total) -> "paye"
        //   sinon (credit sec ou acompte partiel)   -> "emis"
        // "validee" n'a de sens que cote client (brouillon -> vente creee).
        let statut_piece = if montant_regle >= total && total > 0 {
            "paye"
        } else {
            "emis"
        };

        tx.execute(
            "INSERT INTO piece_commerciale
             (id, type_piece, numero, statut, tiers_type, tiers_id, depot_id,
              piece_origine_id, auteur_id, date_piece, remise_globale, note,
              cree_le, modifie_le, origine)
             VALUES (?1,'facture_fournisseur',?2,?3,'fournisseur',?4,?5,
                     ?6,?7,?8,0.0,?9,?10,?11,'achat')",
            rusqlite::params![
                piece_id, num, statut_piece, f_id, depot_id,
                piece_origine_id, auteur, now, note, now, now
            ],
        ).map_err(|e| e.to_string())?;

        for l in &lignes {
            let montant_ht = (l.prix_achat as f64 * l.quantite).round() as i64;
            tx.execute(
                "INSERT INTO ligne_piece
                 (id, piece_id, article_id, unite_vente_id, quantite,
                  prix_unitaire, remise_pct, remise_montant,
                  taux_tva, montant_tva, montant_ht, cree_le)
                 VALUES (?1,?2,?3,?4,?5,?6,0.0,0,0.0,0,?7,?8)",
                rusqlite::params![
                    uuid::Uuid::new_v4().to_string(),
                    piece_id, l.article_id, l.unite_vente_id,
                    l.quantite, l.prix_achat, montant_ht, now
                ],
            ).map_err(|e| e.to_string())?;
        }

        // ---- 3. Solde de la dette (comptant OU acompte credit) ----
        // Uniquement ici : sans fournisseur, il n'y a pas de dette a solder.
        // C'est CE paiement_fournisseur, et lui seul, que lit le calcul de
        // dette (fournisseurs.rs) — s'il manque, la FAF reste "paye" en
        // apparence mais la dette affichee ne bouge pas : reglement double
        // possible au prochain "Encaisser". D'ou l'ecriture ici, toujours
        // couplee au statut, jamais separee.
        if montant_regle > 0 {
            tx.execute(
                "INSERT INTO paiement_fournisseur
                 (id, fournisseur_id, piece_id, montant, mode, note,
                  auteur_id, date_paiement, cree_le, origine)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,'achat')",
                rusqlite::params![
                    uuid::Uuid::new_v4().to_string(),
                    f_id, piece_id, montant_regle,
                    mode_paiement.as_deref().unwrap_or("especes"),
                    format!("Achat {}", num),
                    auteur, now, now
                ],
            ).map_err(|e| e.to_string())?;
        }

        // Le bon de reception a donne sa facture : il est consomme. Sans
        // ce marquage, le garde-fou ci-dessus ne tenait que sur le
        // descendant ; avec, les deux verrous jouent ici comme cote
        // client, et l'ecran cache le bouton « facturer » de lui-meme.
        if let Some(ref src) = piece_origine_id {
            tx.execute(
                "UPDATE piece_commerciale SET statut = 'transfere', modifie_le = ?1
                 WHERE id = ?2 AND type_piece = 'bon_reception'",
                rusqlite::params![now, src],
            ).map_err(|e| e.to_string())?;
        }

        piece_id_retour = serde_json::json!(piece_id);
        numero_retour = serde_json::json!(num);
    }

    // ---- 4. Sortie de caisse ----
    // HORS du bloc fournisseur : l'argent quitte le tiroir meme sans
    // tiers identifie (regularisation, achat de depannage). Sans ce
    // mouvement, la cloture affiche un excedent egal aux achats comptant.
    // Montant = montant_regle (comptant OU acompte), jamais `total` seul —
    // sinon un acompte partiel ferait sortir plus de caisse que ce qui est
    // reellement paye.
    if montant_regle > 0 {
        let mode_p = mode_paiement.as_deref().unwrap_or("especes");
        // `exiger_session_caisse` a deja tranche plus haut : si on est ici,
        // la session existe. Le `?` remplace l'ancien `if let Some(...)`
        // qui laissait passer l'achat sans sa contrepartie de caisse.
        let sid = crate::utils::exiger_session_caisse(&tx)?;
        {
            tx.execute(
                "INSERT INTO mouvement_caisse
                 (id, session_id, sens, moyen, montant, motif,
                  operation_id, date_mouvement, cree_le, cree_par, origine)
                 VALUES (?1,?2,'sortie',?3,?4,'achat',?5,?6,?7,?8,'app')",
                rusqlite::params![
                    uuid::Uuid::new_v4().to_string(),
                    sid, mode_p, montant_regle, op_id, now, now, auteur
                ],
            ).map_err(|e| e.to_string())?;
        }
    }

    tx.execute(
        "INSERT INTO journal
         (id, type_evenement, entite_type, entite_id, auteur_id,
          nouveau_valeur, origine, date_evenement)
         VALUES (?1,'achat_enregistre','operation',?2,?3,?4,'app',?5)",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(), op_id, auteur,
            format!(r#"{{"total":{},"nb_lignes":{}}}"#, total, lignes.len()),
            now
        ],
    ).ok();

    tx.commit().map_err(|e| e.to_string())?;

    Ok(serde_json::json!({
        "piece_id": piece_id_retour,
        "numero":   numero_retour,
        "total":    total,
        "regle":    montant_regle,
        "statut":   if montant_regle >= total && total > 0 { "paye" } else { "emis" },
    }))
}

// =====================================================================
//  RETOUR FOURNISSEUR
// =====================================================================
//
//  Symetrique du retour client, mais les flux sont inverses :
//  le stock SORT et la dette DIMINUE.
//
//  Deux modes :
//    "avoir"         — le fournisseur credite le compte. Piece AVF,
//                      dette reduite, caisse inchangee. Cas normal.
//    "remboursement" — le fournisseur rend l'argent. Entree de caisse,
//                      et l'AVF est marque 'paye' pour ne pas reduire
//                      la dette une seconde fois.
// =====================================================================

#[derive(Deserialize)]
pub struct LigneRetourFournisseur {
    pub article_id: String,
    pub unite_vente_id: String,
    pub quantite: f64,
    pub facteur: f64,
    pub prix_achat: i64,
}

pub fn enregistrer_retour_fournisseur(
    conn: &mut rusqlite::Connection,
    fournisseur_id: String,
    depot_id: Option<String>,
    lignes: Vec<LigneRetourFournisseur>,
    // Facture d'achat d'origine — permet de calculer le reliquat PAR
    // FACTURE et non par article, seul moyen de rester juste quand le
    // meme article a ete achete sur plusieurs factures.
    piece_origine_id: Option<String>,
    // "avoir" (defaut) | "remboursement"
    mode_resolution: Option<String>,
    // especes | orange_money | moov_money | cheque — si remboursement
    mode_encaissement: Option<String>,
    motif: Option<String>,
    utilisateur_role: Option<String>,
) -> Result<serde_json::Value, String> {
    enregistrer_retour_fournisseur_sur(
        conn, fournisseur_id, depot_id, lignes, piece_origine_id,
        mode_resolution, mode_encaissement, motif, utilisateur_role,
    )
}

/// Logique du retour fournisseur, sur une connexion quelconque.
///
/// Separee de la commande pour etre jouable sur une base de test : les
/// scenarios de `tests_multi_depot` verifient ce que le SQL fait
/// reellement a la base, ce qu'aucun test de formule ne montre.
#[allow(clippy::too_many_arguments)]
pub fn enregistrer_retour_fournisseur_sur(
    conn: &mut rusqlite::Connection,
    fournisseur_id: String,
    depot_id: Option<String>,
    lignes: Vec<LigneRetourFournisseur>,
    piece_origine_id: Option<String>,
    mode_resolution: Option<String>,
    mode_encaissement: Option<String>,
    motif: Option<String>,
    utilisateur_role: Option<String>,
) -> Result<serde_json::Value, String> {
    if lignes.is_empty() {
        return Err("Aucune ligne a retourner".to_string());
    }

    let now = maintenant_iso();
    let role = utilisateur_role.as_deref().unwrap_or("employe");
    let auteur = crate::argent::id_utilisateur_par_role(conn, role);

    // Le depot du RETOUR est celui de la facture d'achat, pas le depot
    // par defaut.
    //
    // La marchandise repart d'ou elle est entree. Sortir du depot par
    // defaut une caisse recue au magasin annexe enlevait du stock a un
    // magasin qui n'avait rien et laissait l'autre en excedent
    // permanent — les deux faux, et aucun des deux ne le disait.
    // Meme regle que l'echange, qui sort du depot de la vente (D43).
    let depot_id = match depot_id {
        Some(d) if !d.is_empty() => d,
        _ => {
            let du_lot: Option<String> = piece_origine_id.as_ref().and_then(|p| {
                conn.query_row(
                    "SELECT depot_id FROM piece_commerciale WHERE id = ?1",
                    rusqlite::params![p],
                    |r| r.get::<_, Option<String>>(0),
                )
                .ok()
                .flatten()
                .filter(|d| !d.is_empty())
            });
            match du_lot {
                Some(d) => d,
                None => conn
                    .query_row(
                        "SELECT id FROM depot WHERE est_defaut = 1 LIMIT 1",
                        [],
                        |r| r.get(0),
                    )
                    .map_err(|_| "Aucun depot par defaut configure".to_string())?,
            }
        }
    };

    // On ne retourne pas ce qu'on n'a pas.
    //
    // Une vente a decouvert se constate : le client attend, la
    // marchandise est souvent la, c'est le stock informatique qui a du
    // retard. Un retour fournisseur, non : la marchandise doit
    // PHYSIQUEMENT quitter la boutique pour repartir chez le
    // fournisseur. En retourner cinquante quand on en detient trois
    // n'est pas un evenement du commerce, c'est une erreur de saisie —
    // et l'enregistrer creuserait un stock negatif que personne ne
    // saurait plus rattacher a sa cause.
    //
    // Meme refus que le transfert (D32), pour la meme raison.
    verifier_stock_disponible(conn, &depot_id, &lignes)?;

    let rembourse = mode_resolution.as_deref() == Some("remboursement");

    // Meme regle qu'a l'achat : si le tiroir doit s'ouvrir, il doit etre
    // ouvert. Un remboursement encaisse caisse fermee entrait dans la
    // dette mais pas dans le tiroir — excedent inexplique au comptage.
    if rembourse {
        crate::utils::exiger_session_caisse(conn)?;
    }

    let numero = crate::argent::prochain_numero(conn, "avoir_fournisseur");

    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let op_id = uuid::Uuid::new_v4().to_string();
    let piece_id = uuid::Uuid::new_v4().to_string();
    let mut total: i64 = 0;

    // Un AVF rembourse en especes est deja solde : statut 'paye', il ne
    // doit pas reduire la dette en plus de l'entree de caisse.
    let statut_piece = if rembourse { "paye" } else { "emis" };

    tx.execute(
        "INSERT INTO piece_commerciale
         (id, type_piece, numero, statut, tiers_type, tiers_id, depot_id,
          piece_origine_id, auteur_id, date_piece, remise_globale, note,
          cree_le, modifie_le, origine)
         VALUES (?1,'avoir_fournisseur',?2,?3,'fournisseur',?4,?5,?6,
                 ?7,?8,0.0,?9,?10,?11,'retour')",
        rusqlite::params![
            piece_id, numero, statut_piece, fournisseur_id, depot_id,
            piece_origine_id, auteur, now, motif, now, now
        ],
    ).map_err(|e| e.to_string())?;

    for l in &lignes {
        let quantite_base = l.quantite * l.facteur;
        let montant = (l.prix_achat as f64 * l.quantite).round() as i64;
        total += montant;

        // Sortie de stock — ON CONFLICT, jamais un UPDATE nu.
        // Le stock suit desormais son mouvement : le declencheur
        // `stock_suit_les_mouvements` met le compteur a jour dans la meme
        // transaction. L'ecrire ici le compterait deux fois.

        let prix_base = if l.facteur > 0.0 {
            (l.prix_achat as f64 / l.facteur).round() as i64
        } else { l.prix_achat };

        tx.execute(
            "INSERT INTO mouvement_stock
             (id, article_id, depot_id, type_mouvement, quantite_delta,
              operation_id, auteur_id, date_mouvement, cree_le, cree_par, origine,
              fournisseur_id, prix_achat_unitaire)
             VALUES (?1,?2,?3,'retour_fournisseur',?4,?5,?6,?7,?8,?9,'app',?10,?11)",
            rusqlite::params![
                uuid::Uuid::new_v4().to_string(),
                l.article_id, depot_id, -quantite_base,
                // piece_id (l'avoir), pas op_id : c'est lui qui permet de
                // retrouver le numéro d'avoir depuis un mouvement de stock.
                piece_id, auteur, now, now, auteur,
                fournisseur_id, prix_base
            ],
        ).map_err(|e| e.to_string())?;

        tx.execute(
            "INSERT INTO ligne_piece
             (id, piece_id, article_id, unite_vente_id, quantite,
              prix_unitaire, remise_pct, remise_montant,
              taux_tva, montant_tva, montant_ht, cree_le)
             VALUES (?1,?2,?3,?4,?5,?6,0.0,0,0.0,0,?7,?8)",
            rusqlite::params![
                uuid::Uuid::new_v4().to_string(),
                piece_id, l.article_id, l.unite_vente_id,
                l.quantite, l.prix_achat, montant, now
            ],
        ).map_err(|e| e.to_string())?;
    }

    // Remboursement : l'argent revient dans le tiroir.
    if rembourse && total > 0 {
        let mode_e = mode_encaissement.as_deref().unwrap_or("especes");
        let sid = crate::utils::exiger_session_caisse(&tx)?;
        {
            tx.execute(
                "INSERT INTO mouvement_caisse
                 (id, session_id, sens, moyen, montant, motif,
                  operation_id, date_mouvement, cree_le, cree_par, origine)
                 VALUES (?1,?2,'entree',?3,?4,'retour_fournisseur',?5,?6,?7,?8,'app')",
                rusqlite::params![
                    uuid::Uuid::new_v4().to_string(),
                    sid, mode_e, total, op_id, now, now, auteur
                ],
            ).map_err(|e| e.to_string())?;
        }
    }

    tx.execute(
        "INSERT INTO journal
         (id, type_evenement, entite_type, entite_id, auteur_id,
          nouveau_valeur, origine, date_evenement)
         VALUES (?1,'retour_fournisseur','piece',?2,?3,?4,'app',?5)",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(), piece_id, auteur,
            format!(r#"{{"total":{},"mode":"{}"}}"#, total,
                    if rembourse { "remboursement" } else { "avoir" }),
            now
        ],
    ).ok();

    tx.commit().map_err(|e| e.to_string())?;

    Ok(serde_json::json!({
        "piece_id": piece_id,
        "numero":   numero,
        "total":    total,
        "statut":   statut_piece,
    }))
}

/// Refuse un retour qui mettrait le depot a decouvert.
///
/// Les quantites sont CUMULEES par article avant comparaison : deux
/// lignes de trois sacs sur un stock de quatre doivent etre refusees
/// ensemble, alors que chacune prise seule passerait.
pub fn verifier_stock_disponible(
    conn: &rusqlite::Connection,
    depot_id: &str,
    lignes: &[LigneRetourFournisseur],
) -> Result<(), String> {
    let mut demande: std::collections::HashMap<&str, f64> =
        std::collections::HashMap::new();
    for l in lignes {
        *demande.entry(l.article_id.as_str()).or_insert(0.0) += l.quantite * l.facteur;
    }

    for (article_id, voulu) in demande {
        if voulu <= 0.0 {
            continue;
        }
        let dispo: f64 = conn
            .query_row(
                "SELECT COALESCE(quantite, 0) FROM stock_depot
                 WHERE article_id = ?1 AND depot_id = ?2",
                rusqlite::params![article_id, depot_id],
                |r| r.get(0),
            )
            .unwrap_or(0.0);

        if voulu > dispo {
            let nom: String = conn
                .query_row(
                    "SELECT nom FROM article WHERE id = ?1",
                    rusqlite::params![article_id],
                    |r| r.get(0),
                )
                .unwrap_or_else(|_| "cet article".to_string());
            let depot: String = conn
                .query_row(
                    "SELECT nom FROM depot WHERE id = ?1",
                    rusqlite::params![depot_id],
                    |r| r.get(0),
                )
                .unwrap_or_else(|_| "ce depot".to_string());
            return Err(format!(
                "Stock insuffisant pour « {nom} » dans « {depot} » : \
                 {dispo} disponible(s), {voulu} demandé(s). La marchandise doit \
                 être en stock pour repartir chez le fournisseur."
            ));
        }
    }
    Ok(())
}

/// Factures d'achat d'un fournisseur, avec le reliquat retournable par
/// ligne (quantite achetee moins quantite deja retournee).
///
/// Sans ce calcul, rien n'empeche de retourner deux fois la meme
/// marchandise.
pub fn lire_factures_fournisseur_retournables(
    conn: &rusqlite::Connection,
    fournisseur_id: String,
) -> Result<Vec<serde_json::Value>, String> {

    // Le depot de la facture voyage avec elle : c'est de la que la
    // marchandise repartira, donc c'est son stock qu'il faut montrer.
    let mut st_f = conn.prepare(
        "SELECT pc.id, pc.numero, pc.date_piece, pc.statut,
                pc.depot_id, COALESCE(d.nom, '')
         FROM piece_commerciale pc
         LEFT JOIN depot d ON d.id = pc.depot_id
         WHERE pc.tiers_type = 'fournisseur' AND pc.tiers_id = ?1
           AND pc.type_piece = 'facture_fournisseur'
           AND pc.statut <> 'annule'
         ORDER BY pc.date_piece DESC LIMIT 50"
    ).map_err(|e| e.to_string())?;

    let factures: Vec<(String, String, String, String, Option<String>, String)> =
        st_f.query_map(
            rusqlite::params![fournisseur_id], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))
            }
        ).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    let mut resultat = Vec::new();

    for (piece_id, numero, date_piece, statut, depot_id, depot_nom) in factures {
        let mut st_l = conn.prepare(
            "SELECT lp.id, lp.article_id, a.nom, lp.unite_vente_id,
                    u.libelle, u.facteur, lp.quantite, lp.prix_unitaire
             FROM ligne_piece lp
             JOIN article a ON a.id = lp.article_id
             JOIN unite_vente u ON u.id = lp.unite_vente_id
             WHERE lp.piece_id = ?1
             ORDER BY a.nom"
        ).map_err(|e| e.to_string())?;

        let lignes: Vec<serde_json::Value> = st_l.query_map(
            rusqlite::params![piece_id], |r| {
                Ok(serde_json::json!({
                    "ligne_id":       r.get::<_,String>(0)?,
                    "article_id":     r.get::<_,String>(1)?,
                    "article_nom":    r.get::<_,String>(2)?,
                    "unite_vente_id": r.get::<_,String>(3)?,
                    "unite_libelle":  r.get::<_,String>(4)?,
                    "facteur":        r.get::<_,f64>(5)?,
                    "quantite":       r.get::<_,f64>(6)?,
                    "prix_achat":     r.get::<_,i64>(7)?,
                }))
            }
        ).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

        // Quantites deja retournees SUR CETTE FACTURE (piece_origine_id).
        // Agreger par article tous AVF confondus serait faux des que le
        // meme article apparait sur deux factures d'achat.
        //
        // Les AVF anterieurs a cette colonne ont piece_origine_id NULL :
        // ils ne sont rattaches a aucune facture et n'entrent donc dans
        // aucun reliquat. Sans objet en dev, a savoir en production.
        let mut st_r = conn.prepare(
            "SELECT lp.article_id, COALESCE(SUM(lp.quantite), 0)
             FROM piece_commerciale pc
             JOIN ligne_piece lp ON lp.piece_id = pc.id
             WHERE pc.type_piece = 'avoir_fournisseur'
               AND pc.statut <> 'annule'
               AND pc.piece_origine_id = ?1
             GROUP BY lp.article_id"
        ).map_err(|e| e.to_string())?;

        let retournes: std::collections::HashMap<String, f64> = st_r.query_map(
            rusqlite::params![piece_id], |r| {
                Ok((r.get::<_,String>(0)?, r.get::<_,f64>(1)?))
            }
        ).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

        let lignes: Vec<serde_json::Value> = lignes.into_iter().map(|mut l| {
            let art = l["article_id"].as_str().unwrap_or("").to_string();
            let qte = l["quantite"].as_f64().unwrap_or(0.0);
            let deja = *retournes.get(&art).unwrap_or(&0.0);
            let restant = (qte - deja).max(0.0);

            // Ce qui reste sur la facture ne dit pas ce qu'on peut
            // rendre : la marchandise a pu etre vendue depuis. Le
            // retour est refuse a l'enregistrement s'il depasse le
            // stock ; l'ecran doit le dire AVANT le clic, sinon le
            // commercant saisit sa ligne pour rien.
            let facteur = l["facteur"].as_f64().unwrap_or(1.0).max(0.000_001);
            let en_stock_base: f64 = match &depot_id {
                Some(d) => conn.query_row(
                    "SELECT COALESCE(quantite, 0) FROM stock_depot
                     WHERE article_id = ?1 AND depot_id = ?2",
                    rusqlite::params![art, d],
                    |r| r.get(0),
                ).unwrap_or(0.0),
                // Facture sans depot (base ancienne) : on ne sait pas
                // d'ou la marchandise repartira, donc on ne promet
                // rien. Le refus a l'enregistrement reste le filet.
                None => f64::INFINITY,
            };
            let en_stock = en_stock_base / facteur;

            l["deja_retourne"] = serde_json::json!(deja);
            l["quantite_restante"] = serde_json::json!(restant);
            l["stock_disponible"] =
                serde_json::json!(if en_stock.is_finite() { en_stock } else { -1.0 });
            l["retournable"] = serde_json::json!(restant.min(en_stock.max(0.0)));
            l
        }).collect();

        let total: i64 = lignes.iter().map(|l| {
            let q = l["quantite"].as_f64().unwrap_or(0.0);
            let p = l["prix_achat"].as_i64().unwrap_or(0);
            (p as f64 * q).round() as i64
        }).sum();

        resultat.push(serde_json::json!({
            "piece_id":   piece_id,
            "numero":     numero,
            "date_piece": date_piece,
            "statut":     statut,
            "depot_nom":  depot_nom,
            "total":      total,
            "lignes":     lignes,
        }));
    }

    Ok(resultat)
}