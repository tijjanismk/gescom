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
        ).map_err(|_| "Aucun magasin par défaut configuré".to_string())?,
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


    let tx = conn.transaction().map_err(|e| e.to_string())?;

    // Reserve DANS la transaction : si l'enregistrement echoue plus
    // bas, le retour arriere annule aussi l'increment, et la serie
    // n'a pas de trou.
    let numero = if fournisseur_id.is_some() {
        Some(crate::argent::reserver_numero(&tx, "facture_fournisseur")?)
    } else {
        None
    };
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

    let tx = conn.transaction().map_err(|e| e.to_string())?;

    // Reserve DANS la transaction : si l'enregistrement echoue plus
    // bas, le retour arriere annule aussi l'increment, et la serie
    // n'a pas de trou.
    let numero = crate::argent::reserver_numero(&tx, "avoir_fournisseur")?;
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
// =====================================================================
//  VALIDER UNE FACTURE FOURNISSEUR — le miroir de `valider_facture`
// =====================================================================

/// Valide une facture fournisseur en brouillon : dette, reglement,
/// caisse — et le stock seulement si aucun bon ne s'en est charge.
///
/// ## Pourquoi cette fonction n'existait pas
///
/// Cote client, une facture se cree en brouillon puis se VALIDE : c'est
/// la validation qui sort le stock et encaisse. Cote fournisseur, il n'y
/// avait qu'`enregistrer_achat`, qui fait tout d'un seul geste. La
/// chaine s'arretait donc au bon de reception : la conversion
/// BRF -> FAF etait refusee, parce qu'une conversion generique aurait
/// cree une facture sans dette ni decaissement — une dette fantome, et
/// un risque de payer deux fois au reglement.
///
/// Cette fonction ferme le trou : elle est a la FAF ce que
/// `valider_facture` est a la facture client.
///
/// ## Le stock
///
/// Depuis que la reception deplace le stock, une FAF issue d'un bon de
/// reception ne fait plus QUE l'argent : la marchandise est deja
/// entree. Une FAF sans bon en amont fait entrer le stock elle-meme,
/// comme `enregistrer_achat`.
pub fn valider_facture_fournisseur(
    conn: &mut rusqlite::Connection,
    piece_id: String,
    // "comptant" -> soldee tout de suite | "credit" -> dette
    mode_reglement: String,
    // especes | orange_money | moov_money | cheque
    mode_paiement: Option<String>,
    // Acompte verse a la signature, si credit.
    acompte: Option<i64>,
    utilisateur_role: Option<String>,
) -> Result<serde_json::Value, String> {
    let (fournisseur_id, type_p, statut, depot_opt, numero):
        (String, String, String, Option<String>, String) =
        conn.query_row(
            "SELECT tiers_id, type_piece, statut, depot_id, numero
             FROM piece_commerciale WHERE id = ?1",
            rusqlite::params![piece_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        ).map_err(|_| "Pièce introuvable".to_string())?;

    if type_p != "facture_fournisseur" {
        return Err("Seule une facture fournisseur se valide ici".to_string());
    }
    if statut != "brouillon" {
        return Err(format!("Facture déjà en statut « {statut} »"));
    }

    let lignes = crate::argent::lire_lignes_raw(conn, &piece_id)?;
    if lignes.is_empty() {
        return Err("La facture ne contient aucune ligne".to_string());
    }

    // Le total se recalcule ICI, depuis les lignes, jamais depuis un
    // montant transmis par l'ecran : un ecran perime ferait naitre une
    // dette qui ne correspond a rien de ce qui est facture.
    let total: i64 = lignes
        .iter()
        .map(|(_, _, qte, prix, remise_pct, _)| {
            let brut = (*prix as f64 * qte).round() as i64;
            let remise = (brut as f64 * remise_pct / 100.0).round() as i64;
            brut - remise
        })
        .sum();

    let comptant = mode_reglement == "comptant";
    let montant_regle = if comptant { total } else { acompte.unwrap_or(0).max(0) };

    // Caisse fermee : on REFUSE au lieu d'ecrire la dette sans sa sortie
    // de caisse. Meme regle qu'`enregistrer_achat` (D46) — sinon le
    // comptage du soir tombe faux d'exactement ce montant.
    if montant_regle > 0 {
        crate::utils::exiger_session_caisse(conn)?;
    }

    let role = utilisateur_role.as_deref().unwrap_or("employe");
    let auteur = crate::argent::id_utilisateur_par_role(conn, role);
    let now = maintenant_iso();
    let stock_ailleurs = crate::pieces::stock_confie_a_un_bon(conn, &piece_id);
    let depot = crate::pieces::depot_de_piece(conn, depot_opt);

    // Le depot se verifie AVANT d'ouvrir la transaction : sortir par un
    // `?` une fois `tx` en vie laisserait un retour arriere implicite,
    // correct mais illisible.
    if !stock_ailleurs && depot.is_none() {
        return Err(
            "Aucun magasin actif : impossible de faire entrer cette marchandise."
                .to_string(),
        );
    }

    let tx = conn.transaction().map_err(|e| e.to_string())?;

    // ---- 1. Stock, si aucun bon ne l'a deja fait entrer ----
    if !stock_ailleurs {
        let depot = depot.as_deref().unwrap_or_default();
        for (article_id, unite_id, qte, prix, _, _) in &lignes {
            let facteur: f64 = tx
                .query_row(
                    "SELECT facteur FROM unite_vente WHERE id = ?1",
                    rusqlite::params![unite_id],
                    |r| r.get(0),
                )
                .unwrap_or(1.0);

            tx.execute(
                "INSERT INTO mouvement_stock
                 (id, article_id, depot_id, type_mouvement, quantite_delta,
                  operation_id, auteur_id, date_mouvement, cree_le, cree_par,
                  origine, fournisseur_id, prix_achat_unitaire)
                 VALUES (?1,?2,?3,'achat',?4,?5,?6,?7,?7,?6,'app',?8,?9)",
                rusqlite::params![
                    uuid::Uuid::new_v4().to_string(),
                    article_id, depot, qte * facteur,
                    piece_id, auteur, now, fournisseur_id, prix
                ],
            ).map_err(|e| e.to_string())?;

            // Le dernier prix d'achat sert a la marge : il suit la
            // facture, pas la reception, parce que c'est la facture qui
            // porte le prix reellement consenti.
            tx.execute(
                "UPDATE article SET dernier_prix_achat = ?1, modifie_le = ?2
                 WHERE id = ?3",
                rusqlite::params![prix, now, article_id],
            ).ok();
        }
    }

    // ---- 2. Statut, selon ce qui est REELLEMENT regle ----
    let statut_piece = if montant_regle >= total && total > 0 { "paye" } else { "emis" };
    tx.execute(
        "UPDATE piece_commerciale SET statut = ?1, modifie_le = ?2 WHERE id = ?3",
        rusqlite::params![statut_piece, now, piece_id],
    ).map_err(|e| e.to_string())?;

    // ---- 3. Le reglement ----
    // C'est CE paiement_fournisseur, et lui seul, que lit le calcul de
    // dette. S'il manque, la FAF parait payee mais la dette affichee ne
    // bouge pas : reglement double possible au prochain « Encaisser ».
    if montant_regle > 0 {
        tx.execute(
            "INSERT INTO paiement_fournisseur
             (id, fournisseur_id, piece_id, montant, mode, note,
              auteur_id, date_paiement, cree_le, origine)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,'app')",
            rusqlite::params![
                uuid::Uuid::new_v4().to_string(),
                fournisseur_id, piece_id, montant_regle,
                mode_paiement.as_deref().unwrap_or("especes"),
                format!("Facture {numero}"),
                auteur, now, now
            ],
        ).map_err(|e| e.to_string())?;

        // ---- 4. La sortie de caisse ----
        let sid = crate::utils::exiger_session_caisse(&tx)?;
        tx.execute(
            "INSERT INTO mouvement_caisse
             (id, session_id, sens, moyen, montant, motif,
              operation_id, date_mouvement, cree_le, cree_par, origine)
             VALUES (?1,?2,'sortie',?3,?4,'achat',?5,?6,?6,?7,'app')",
            rusqlite::params![
                uuid::Uuid::new_v4().to_string(),
                sid, mode_paiement.as_deref().unwrap_or("especes"),
                montant_regle, piece_id, now, auteur
            ],
        ).map_err(|e| e.to_string())?;
    }

    tx.execute(
        "INSERT INTO journal
         (id, type_evenement, entite_type, entite_id, auteur_id,
          nouveau_valeur, origine, date_evenement)
         VALUES (?1,'facture_fournisseur_validee','piece_commerciale',?2,?3,?4,'app',?5)",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(), piece_id, auteur,
            format!(
                "{{\"total\":{total},\"regle\":{montant_regle},\"statut\":\"{statut_piece}\"}}"
            ),
            now
        ],
    ).ok();

    tx.commit().map_err(|e| e.to_string())?;

    Ok(serde_json::json!({
        "piece_id":      piece_id,
        "numero":        numero,
        "statut":        statut_piece,
        "total":         total,
        "montant_regle": montant_regle,
        "stock_entre":   !stock_ailleurs,
    }))
}

// =====================================================================
//  ANNULER UNE FACTURE FOURNISSEUR PAR UN AVOIR
// =====================================================================

/// Convertit une facture fournisseur entiere en avoir, en un geste.
///
/// ## Pourquoi ce n'est pas une nouvelle mecanique
///
/// Rendre une partie d'une facture existait deja
/// (`enregistrer_retour_fournisseur`) : il sait sortir le stock du bon
/// depot, plafonner au reliquat de la facture, refuser le decouvert,
/// et choisir entre deduire la dette ou encaisser un remboursement.
///
/// Annuler la facture ENTIERE, c'est le meme geste sur toutes les
/// lignes. Reecrire ces regles ici en aurait fait une seconde copie
/// qui aurait derive — c'est exactement ce qui etait arrive au calcul
/// de dette. On lit donc les lignes et on delegue.
///
/// ## Ce qui differe du cote client
///
/// `annuler_facture_par_avoir` (pieces.rs) s'appuie sur la `vente`
/// derriere la facture : elle l'annule, rend le stock, rembourse
/// l'acompte. Une facture fournisseur n'a pas de vente. Le pendant
/// naturel est donc le retour integral, qui produit la meme chose :
/// un AVF, la marchandise qui repart, et la dette reduite d'autant.
pub fn annuler_facture_fournisseur_par_avoir(
    conn: &mut rusqlite::Connection,
    piece_id: String,
    // "avoir" (defaut) -> vient en deduction de la dette
    // "remboursement"  -> le fournisseur rend l'argent, la caisse entre
    mode_resolution: Option<String>,
    mode_encaissement: Option<String>,
    motif: Option<String>,
    utilisateur_role: Option<String>,
) -> Result<serde_json::Value, String> {
    let (type_piece, statut, fournisseur_id, numero): (String, String, String, String) =
        conn.query_row(
            "SELECT type_piece, statut, tiers_id, numero
             FROM piece_commerciale WHERE id = ?1",
            rusqlite::params![piece_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .map_err(|_| "Pièce introuvable".to_string())?;

    if type_piece != "facture_fournisseur" {
        return Err("Seule une facture fournisseur s'annule ainsi".to_string());
    }
    if statut == "annule" {
        return Err("Facture déjà annulée".to_string());
    }
    // Un brouillon n'a produit NI dette NI stock : lui fabriquer un
    // avoir inventerait un credit chez un fournisseur a qui l'on ne doit
    // rien. Il se supprime ou se modifie, il ne s'annule pas par avoir.
    if statut == "brouillon" {
        return Err(
            "Cette facture est encore en brouillon : la modifier ou la \
             supprimer, un avoir n'a rien à annuler."
                .to_string(),
        );
    }

    // Les lignes, avec le facteur de leur unite : le retour compte en
    // unite de base, comme tout mouvement de stock.
    let lignes: Vec<LigneRetourFournisseur> = {
        let mut st = conn
            .prepare(
                "SELECT lp.article_id, lp.unite_vente_id, lp.quantite,
                        COALESCE(uv.facteur, 1.0), lp.prix_unitaire
                 FROM ligne_piece lp
                 LEFT JOIN unite_vente uv ON uv.id = lp.unite_vente_id
                 WHERE lp.piece_id = ?1",
            )
            .map_err(|e| e.to_string())?;
        let v = st
            .query_map(rusqlite::params![piece_id], |r| {
                Ok(LigneRetourFournisseur {
                    article_id: r.get(0)?,
                    unite_vente_id: r.get(1)?,
                    quantite: r.get(2)?,
                    facteur: r.get(3)?,
                    prix_achat: r.get(4)?,
                })
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect::<Vec<_>>();
        v
    };

    if lignes.is_empty() {
        return Err("Cette facture n'a aucune ligne à rendre".to_string());
    }

    // Le depot n'est pas transmis : `enregistrer_retour_fournisseur`
    // prend celui de la facture d'origine, ce qui est precisement ce
    // qu'on veut — la marchandise repart d'ou elle etait entree.
    let resultat = enregistrer_retour_fournisseur_sur(
        conn,
        fournisseur_id,
        None,
        lignes,
        Some(piece_id.clone()),
        mode_resolution,
        mode_encaissement,
        Some(motif.unwrap_or_else(|| format!("Annulation de la facture {numero}"))),
        utilisateur_role,
    )?;

    // La facture porte la trace de son annulation. Le statut 'annule'
    // la sort des totaux sans la faire disparaitre : c'est la trace, et
    // la cacher serait pire.
    let now = maintenant_iso();
    conn.execute(
        "UPDATE piece_commerciale
         SET statut = 'annule', note = ?1, modifie_le = ?2
         WHERE id = ?3",
        rusqlite::params![
            "Annulée par avoir fournisseur",
            now,
            piece_id
        ],
    )
    .map_err(|e| e.to_string())?;

    Ok(resultat)
}

// =====================================================================
//  LES ACHATS, SUR L'UN OU L'AUTRE MOTEUR
// =====================================================================
//
// Les six commandes du module, portees sur `Base`, dans le meme esprit
// que `pieces` : meme ordre d'ecriture, memes refus, `dossier_id` sur
// chaque table cloisonnee, transactions la ou l'argent et le stock
// bougent ensemble.

use crate::argent::{id_utilisateur_par_role_sur, lire_lignes_raw_sur, reserver_numero_sur};
use crate::base::{Acces, Base};
use crate::parametres;

/// Le magasin par defaut du dossier, ou un refus qui le dit.
fn depot_par_defaut_sur(acces: &mut impl Acces) -> Result<String, String> {
    let dossier = acces.dossier().to_string();
    acces
        .lire_une(
            "SELECT id FROM depot WHERE est_defaut = 1 AND dossier_id = ?1 LIMIT 1",
            &parametres![dossier],
            |r| r.get::<String>(0),
        )
        .map_err(|e| e.0)?
        .ok_or_else(|| "Aucun magasin par défaut configuré".to_string())
}

fn journaliser(
    acces: &mut impl Acces,
    evenement: &str,
    entite_type: &str,
    entite_id: &str,
    auteur: &str,
    valeur: String,
    now: &str,
) {
    let dossier = acces.dossier().to_string();
    let _ = acces.executer(
        "INSERT INTO journal
         (id, type_evenement, entite_type, entite_id, auteur_id,
          nouveau_valeur, origine, date_evenement, dossier_id)
         VALUES (?1,?2,?3,?4,?5,?6,'app',?7,?8)",
        &parametres![
            uuid::Uuid::new_v4().to_string(),
            evenement,
            entite_type,
            entite_id,
            auteur,
            valeur,
            now,
            dossier
        ],
    );
}

/// Une ligne de facture fournisseur, sans remise ni TVA.
fn inserer_ligne_achat(
    acces: &mut impl Acces,
    piece_id: &str,
    article_id: &str,
    unite_vente_id: &str,
    quantite: f64,
    prix: i64,
    montant: i64,
    now: &str,
) -> Result<(), String> {
    let dossier = acces.dossier().to_string();
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
                prix,
                montant,
                now,
                dossier
            ],
        )
        .map_err(|e| e.0)
        .map(|_| ())
}

/// Sortie de caisse pour un reglement fournisseur.
fn sortie_de_caisse(
    acces: &mut impl Acces,
    session_id: &str,
    moyen: &str,
    montant: i64,
    motif: &str,
    operation_id: &str,
    now: &str,
    auteur: &str,
) -> Result<(), String> {
    let dossier = acces.dossier().to_string();
    acces
        .executer(
            "INSERT INTO mouvement_caisse
             (id, session_id, sens, moyen, montant, motif,
              operation_id, date_mouvement, cree_le, cree_par, origine, dossier_id)
             VALUES (?1,?2,'sortie',?3,?4,?5,?6,?7,?7,?8,'app',?9)",
            &parametres![
                uuid::Uuid::new_v4().to_string(),
                session_id,
                moyen,
                montant,
                motif,
                operation_id,
                now,
                auteur,
                dossier
            ],
        )
        .map_err(|e| e.0)
        .map(|_| ())
}

#[allow(clippy::too_many_arguments)]
pub fn enregistrer_achat_sur_base(
    base: &mut Base,
    fournisseur_id: Option<String>,
    depot_id: Option<String>,
    lignes: Vec<LigneAchat>,
    mode_reglement: Option<String>,
    mode_paiement: Option<String>,
    acompte: Option<i64>,
    note: Option<String>,
    utilisateur_role: Option<String>,
    piece_origine_id: Option<String>,
) -> Result<serde_json::Value, String> {
    if lignes.is_empty() {
        return Err("Aucune ligne à enregistrer".to_string());
    }
    let dossier = base.dossier().to_string();
    let now = maintenant_iso();
    let role = utilisateur_role.as_deref().unwrap_or("employe");
    let auteur = id_utilisateur_par_role_sur(base, role);

    let depot_id = match depot_id {
        Some(d) if !d.is_empty() => d,
        _ => depot_par_defaut_sur(base)?,
    };

    // Un bon de reception ne se facture qu'une fois — meme garde-fou
    // que cote client.
    if let Some(ref src) = piece_origine_id {
        let statut_src: String = base
            .lire_une(
                "SELECT statut FROM piece_commerciale WHERE id = ?1 AND dossier_id = ?2",
                &parametres![src.clone(), dossier.clone()],
                |r| r.get::<String>(0),
            )
            .map_err(|e| e.0)?
            .ok_or_else(|| "Pièce d'origine introuvable".to_string())?;
        let deja = crate::pieces::descendant_actif_sur(base, src);
        crate::coeur::pieces::peut_transferer(&statut_src, deja.as_deref())?;
    }

    // Caisse fermee : refus AVANT d'ecrire (D46).
    let comptant = mode_reglement.as_deref() == Some("comptant");
    let session = if comptant || acompte.unwrap_or(0) > 0 {
        Some(crate::caisses::exiger_sur(base, None)?)
    } else {
        None
    };

    let mut tx = base.transaction().map_err(|e| e.0)?;

    let numero = if fournisseur_id.is_some() {
        Some(reserver_numero_sur(&mut tx, "facture_fournisseur")?)
    } else {
        None
    };
    let op_id = uuid::Uuid::new_v4().to_string();
    // La piece sert d'`operation_id` aux mouvements (D51) ; sans
    // fournisseur, un simple identifiant de lot.
    let piece_id = fournisseur_id.as_ref().map(|_| uuid::Uuid::new_v4().to_string());
    let operation_id = piece_id.clone().unwrap_or_else(|| op_id.clone());
    let mut total: i64 = 0;

    // ---- 1. Stock et mouvements (le declencheur tient le compteur).
    for l in &lignes {
        let quantite_base = l.quantite * l.facteur;
        total += (l.prix_achat as f64 * l.quantite).round() as i64;
        let prix_base = if l.facteur > 0.0 {
            (l.prix_achat as f64 / l.facteur).round() as i64
        } else {
            l.prix_achat
        };

        tx.executer(
            "INSERT INTO mouvement_stock
             (id, article_id, depot_id, type_mouvement, quantite_delta,
              operation_id, auteur_id, date_mouvement, cree_le, cree_par, origine,
              fournisseur_id, prix_achat_unitaire, dossier_id)
             VALUES (?1,?2,?3,'achat',?4,?5,?6,?7,?7,?6,'app',?8,?9,?10)",
            &parametres![
                uuid::Uuid::new_v4().to_string(),
                l.article_id.clone(),
                depot_id.clone(),
                quantite_base,
                operation_id.clone(),
                auteur.clone(),
                now.clone(),
                fournisseur_id.clone(),
                prix_base,
                dossier.clone()
            ],
        )
        .map_err(|e| e.0)?;

        // `article` n'est pas cloisonne.
        let _ = tx.executer(
            "UPDATE article SET dernier_prix_achat = ?1, modifie_le = ?2 WHERE id = ?3",
            &parametres![prix_base, now.clone(), l.article_id.clone()],
        );
    }

    // Le statut se DEDUIT de ce qui est regle, jamais du mode declare.
    let montant_regle: i64 = if comptant { total } else { acompte.unwrap_or(0).clamp(0, total) };
    let statut_piece = if montant_regle >= total && total > 0 { "paye" } else { "emis" };

    // ---- 2. Facture fournisseur.
    let mut piece_id_retour = serde_json::Value::Null;
    let mut numero_retour = serde_json::Value::Null;

    if let (Some(f_id), Some(num), Some(pid)) =
        (fournisseur_id.as_ref(), numero.as_ref(), piece_id.as_ref())
    {
        tx.executer(
            "INSERT INTO piece_commerciale
             (id, type_piece, numero, statut, tiers_type, tiers_id, depot_id,
              piece_origine_id, auteur_id, date_piece, remise_globale, note,
              cree_le, modifie_le, origine, dossier_id)
             VALUES (?1,'facture_fournisseur',?2,?3,'fournisseur',?4,?5,
                     ?6,?7,?8,0.0,?9,?8,?8,'achat',?10)",
            &parametres![
                pid.clone(),
                num.clone(),
                statut_piece,
                f_id.clone(),
                depot_id.clone(),
                piece_origine_id.clone(),
                auteur.clone(),
                now.clone(),
                note.clone(),
                dossier.clone()
            ],
        )
        .map_err(|e| e.0)?;

        for l in &lignes {
            let montant_ht = (l.prix_achat as f64 * l.quantite).round() as i64;
            inserer_ligne_achat(
                &mut tx, pid, &l.article_id, &l.unite_vente_id, l.quantite, l.prix_achat,
                montant_ht, &now,
            )?;
        }

        // ---- 3. Le reglement — c'est LUI que lit le calcul de dette.
        if montant_regle > 0 {
            tx.executer(
                "INSERT INTO paiement_fournisseur
                 (id, fournisseur_id, piece_id, montant, mode, note,
                  auteur_id, date_paiement, cree_le, origine, dossier_id)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?8,'achat',?9)",
                &parametres![
                    uuid::Uuid::new_v4().to_string(),
                    f_id.clone(),
                    pid.clone(),
                    montant_regle,
                    mode_paiement.as_deref().unwrap_or("especes"),
                    format!("Achat {}", num),
                    auteur.clone(),
                    now.clone(),
                    dossier.clone()
                ],
            )
            .map_err(|e| e.0)?;
        }

        // Le bon de reception a donne sa facture : consomme.
        if let Some(ref src) = piece_origine_id {
            tx.executer(
                "UPDATE piece_commerciale SET statut = 'transfere', modifie_le = ?1
                 WHERE id = ?2 AND type_piece = 'bon_reception' AND dossier_id = ?3",
                &parametres![now.clone(), src.clone(), dossier.clone()],
            )
            .map_err(|e| e.0)?;
        }

        piece_id_retour = serde_json::json!(pid);
        numero_retour = serde_json::json!(num);
    }

    // ---- 4. Sortie de caisse, meme sans tiers identifie.
    if montant_regle > 0 {
        let sid = session.ok_or_else(|| "CAISSE_FERMEE".to_string())?;
        sortie_de_caisse(
            &mut tx,
            &sid,
            mode_paiement.as_deref().unwrap_or("especes"),
            montant_regle,
            "achat",
            &op_id,
            &now,
            &auteur,
        )?;
    }

    journaliser(
        &mut tx,
        "achat_enregistre",
        "operation",
        &op_id,
        &auteur,
        format!(r#"{{"total":{},"nb_lignes":{}}}"#, total, lignes.len()),
        &now,
    );

    tx.valider().map_err(|e| e.0)?;

    Ok(serde_json::json!({
        "piece_id": piece_id_retour,
        "numero":   numero_retour,
        "total":    total,
        "regle":    montant_regle,
        "statut":   statut_piece,
    }))
}

/// Refuse un retour qui mettrait le magasin a decouvert (D32), les
/// quantites CUMULEES par article.
pub fn verifier_stock_disponible_sur(
    acces: &mut impl Acces,
    depot_id: &str,
    lignes: &[LigneRetourFournisseur],
) -> Result<(), String> {
    let dossier = acces.dossier().to_string();
    let mut demande: std::collections::HashMap<&str, f64> = std::collections::HashMap::new();
    for l in lignes {
        *demande.entry(l.article_id.as_str()).or_insert(0.0) += l.quantite * l.facteur;
    }

    for (article_id, voulu) in demande {
        if voulu <= 0.0 {
            continue;
        }
        let dispo: f64 = acces
            .lire_une(
                "SELECT COALESCE(quantite, 0) FROM stock_depot
                 WHERE article_id = ?1 AND depot_id = ?2 AND dossier_id = ?3",
                &parametres![article_id, depot_id, dossier.clone()],
                |r| r.get::<f64>(0),
            )
            .ok()
            .flatten()
            .unwrap_or(0.0);

        if voulu > dispo {
            let nom: String = acces
                .lire_une(
                    "SELECT nom FROM article WHERE id = ?1",
                    &parametres![article_id],
                    |r| r.get::<String>(0),
                )
                .ok()
                .flatten()
                .unwrap_or_else(|| "cet article".to_string());
            let depot: String = acces
                .lire_une(
                    "SELECT nom FROM depot WHERE id = ?1 AND dossier_id = ?2",
                    &parametres![depot_id, dossier.clone()],
                    |r| r.get::<String>(0),
                )
                .ok()
                .flatten()
                .unwrap_or_else(|| "ce depot".to_string());
            return Err(format!(
                "Stock insuffisant pour « {nom} » dans « {depot} » : \
                 {dispo} disponible(s), {voulu} demandé(s). La marchandise doit \
                 être en stock pour repartir chez le fournisseur."
            ));
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn enregistrer_retour_fournisseur_sur_base(
    base: &mut Base,
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
    let dossier = base.dossier().to_string();
    let now = maintenant_iso();
    let role = utilisateur_role.as_deref().unwrap_or("employe");
    let auteur = id_utilisateur_par_role_sur(base, role);

    // La marchandise repart d'ou elle est entree : le depot de la
    // facture d'achat, pas le defaut (meme regle que D43).
    let depot_id = match depot_id {
        Some(d) if !d.is_empty() => d,
        _ => {
            let du_lot: Option<String> = match piece_origine_id.as_ref() {
                Some(p) => base
                    .lire_une(
                        "SELECT depot_id FROM piece_commerciale WHERE id = ?1 AND dossier_id = ?2",
                        &parametres![p.clone(), dossier.clone()],
                        |r| r.get::<Option<String>>(0),
                    )
                    .ok()
                    .flatten()
                    .flatten()
                    .filter(|d| !d.is_empty()),
                None => None,
            };
            match du_lot {
                Some(d) => d,
                None => depot_par_defaut_sur(base)?,
            }
        }
    };

    verifier_stock_disponible_sur(base, &depot_id, &lignes)?;

    let rembourse = mode_resolution.as_deref() == Some("remboursement");
    let session = if rembourse {
        Some(crate::caisses::exiger_sur(base, None)?)
    } else {
        None
    };

    let mut tx = base.transaction().map_err(|e| e.0)?;

    let numero = reserver_numero_sur(&mut tx, "avoir_fournisseur")?;
    let op_id = uuid::Uuid::new_v4().to_string();
    let piece_id = uuid::Uuid::new_v4().to_string();
    let mut total: i64 = 0;

    // Un AVF rembourse est deja solde : 'paye', il ne reduit pas la
    // dette en plus de l'entree de caisse.
    let statut_piece = if rembourse { "paye" } else { "emis" };

    tx.executer(
        "INSERT INTO piece_commerciale
         (id, type_piece, numero, statut, tiers_type, tiers_id, depot_id,
          piece_origine_id, auteur_id, date_piece, remise_globale, note,
          cree_le, modifie_le, origine, dossier_id)
         VALUES (?1,'avoir_fournisseur',?2,?3,'fournisseur',?4,?5,?6,
                 ?7,?8,0.0,?9,?8,?8,'retour',?10)",
        &parametres![
            piece_id.clone(),
            numero.clone(),
            statut_piece,
            fournisseur_id.clone(),
            depot_id.clone(),
            piece_origine_id,
            auteur.clone(),
            now.clone(),
            motif,
            dossier.clone()
        ],
    )
    .map_err(|e| e.0)?;

    for l in &lignes {
        let quantite_base = l.quantite * l.facteur;
        let montant = (l.prix_achat as f64 * l.quantite).round() as i64;
        total += montant;
        let prix_base = if l.facteur > 0.0 {
            (l.prix_achat as f64 / l.facteur).round() as i64
        } else {
            l.prix_achat
        };

        tx.executer(
            "INSERT INTO mouvement_stock
             (id, article_id, depot_id, type_mouvement, quantite_delta,
              operation_id, auteur_id, date_mouvement, cree_le, cree_par, origine,
              fournisseur_id, prix_achat_unitaire, dossier_id)
             VALUES (?1,?2,?3,'retour_fournisseur',?4,?5,?6,?7,?7,?6,'app',?8,?9,?10)",
            &parametres![
                uuid::Uuid::new_v4().to_string(),
                l.article_id.clone(),
                depot_id.clone(),
                -quantite_base,
                // L'avoir, pas op_id : c'est lui qui rend le numero
                // depuis un mouvement (D51).
                piece_id.clone(),
                auteur.clone(),
                now.clone(),
                fournisseur_id.clone(),
                prix_base,
                dossier.clone()
            ],
        )
        .map_err(|e| e.0)?;

        inserer_ligne_achat(
            &mut tx, &piece_id, &l.article_id, &l.unite_vente_id, l.quantite, l.prix_achat,
            montant, &now,
        )?;
    }

    // Remboursement : l'argent revient dans le tiroir.
    if rembourse && total > 0 {
        let sid = session.ok_or_else(|| "CAISSE_FERMEE".to_string())?;
        tx.executer(
            "INSERT INTO mouvement_caisse
             (id, session_id, sens, moyen, montant, motif,
              operation_id, date_mouvement, cree_le, cree_par, origine, dossier_id)
             VALUES (?1,?2,'entree',?3,?4,'retour_fournisseur',?5,?6,?6,?7,'app',?8)",
            &parametres![
                uuid::Uuid::new_v4().to_string(),
                sid,
                mode_encaissement.as_deref().unwrap_or("especes"),
                total,
                op_id,
                now.clone(),
                auteur.clone(),
                dossier.clone()
            ],
        )
        .map_err(|e| e.0)?;
    }

    journaliser(
        &mut tx,
        "retour_fournisseur",
        "piece",
        &piece_id,
        &auteur,
        format!(
            r#"{{"total":{},"mode":"{}"}}"#,
            total,
            if rembourse { "remboursement" } else { "avoir" }
        ),
        &now,
    );

    tx.valider().map_err(|e| e.0)?;

    Ok(serde_json::json!({
        "piece_id": piece_id,
        "numero":   numero,
        "total":    total,
        "statut":   statut_piece,
    }))
}

pub fn lire_factures_fournisseur_retournables_sur_base(
    base: &mut Base,
    fournisseur_id: String,
) -> Result<Vec<serde_json::Value>, String> {
    let dossier = base.dossier().to_string();

    let factures: Vec<(String, String, String, String, Option<String>, String)> = base
        .lire_plusieurs(
            "SELECT pc.id, pc.numero, pc.date_piece, pc.statut,
                    pc.depot_id, COALESCE(d.nom, '')
             FROM piece_commerciale pc
             LEFT JOIN depot d ON d.id = pc.depot_id
             WHERE pc.tiers_type = 'fournisseur' AND pc.tiers_id = ?1
               AND pc.type_piece = 'facture_fournisseur'
               AND pc.statut <> 'annule'
               AND pc.dossier_id = ?2
             ORDER BY pc.date_piece DESC LIMIT 50",
            &parametres![fournisseur_id, dossier.clone()],
            |r| {
                Ok((
                    r.get::<String>(0)?,
                    r.get::<String>(1)?,
                    r.get::<String>(2)?,
                    r.get::<String>(3)?,
                    r.get::<Option<String>>(4)?,
                    r.get::<String>(5)?,
                ))
            },
        )
        .map_err(|e| e.0)?;

    let mut resultat = Vec::new();

    for (piece_id, numero, date_piece, statut, depot_id, depot_nom) in factures {
        let lignes: Vec<serde_json::Value> = base
            .lire_plusieurs(
                "SELECT lp.id, lp.article_id, a.nom, lp.unite_vente_id,
                        u.libelle, u.facteur, lp.quantite, lp.prix_unitaire
                 FROM ligne_piece lp
                 JOIN article a ON a.id = lp.article_id
                 JOIN unite_vente u ON u.id = lp.unite_vente_id
                 WHERE lp.piece_id = ?1 AND lp.dossier_id = ?2
                 ORDER BY a.nom",
                &parametres![piece_id.clone(), dossier.clone()],
                |r| {
                    Ok(serde_json::json!({
                        "ligne_id":       r.get::<String>(0)?,
                        "article_id":     r.get::<String>(1)?,
                        "article_nom":    r.get::<String>(2)?,
                        "unite_vente_id": r.get::<String>(3)?,
                        "unite_libelle":  r.get::<String>(4)?,
                        "facteur":        r.get::<f64>(5)?,
                        "quantite":       r.get::<f64>(6)?,
                        "prix_achat":     r.get::<i64>(7)?,
                    }))
                },
            )
            .map_err(|e| e.0)?;

        // Deja retourne SUR CETTE FACTURE (piece_origine_id).
        let retournes: std::collections::HashMap<String, f64> = base
            .lire_plusieurs(
                "SELECT lp.article_id,
                        CAST(COALESCE(SUM(lp.quantite), 0) AS DOUBLE PRECISION)
                 FROM piece_commerciale pc
                 JOIN ligne_piece lp ON lp.piece_id = pc.id
                 WHERE pc.type_piece = 'avoir_fournisseur'
                   AND pc.statut <> 'annule'
                   AND pc.piece_origine_id = ?1
                   AND pc.dossier_id = ?2
                 GROUP BY lp.article_id",
                &parametres![piece_id.clone(), dossier.clone()],
                |r| Ok((r.get::<String>(0)?, r.get::<f64>(1)?)),
            )
            .map_err(|e| e.0)?
            .into_iter()
            .collect();

        let mut lignes_completes = Vec::with_capacity(lignes.len());
        for mut l in lignes {
            let art = l["article_id"].as_str().unwrap_or("").to_string();
            let qte = l["quantite"].as_f64().unwrap_or(0.0);
            let deja = *retournes.get(&art).unwrap_or(&0.0);
            let restant = (qte - deja).max(0.0);

            // Ce qui reste sur la facture ne dit pas ce qu'on peut
            // rendre : la marchandise a pu etre vendue depuis.
            let facteur = l["facteur"].as_f64().unwrap_or(1.0).max(0.000_001);
            let en_stock_base: f64 = match &depot_id {
                Some(d) => base
                    .lire_une(
                        "SELECT COALESCE(quantite, 0) FROM stock_depot
                         WHERE article_id = ?1 AND depot_id = ?2 AND dossier_id = ?3",
                        &parametres![art.clone(), d.clone(), dossier.clone()],
                        |r| r.get::<f64>(0),
                    )
                    .ok()
                    .flatten()
                    .unwrap_or(0.0),
                None => f64::INFINITY,
            };
            let en_stock = en_stock_base / facteur;

            l["deja_retourne"] = serde_json::json!(deja);
            l["quantite_restante"] = serde_json::json!(restant);
            l["stock_disponible"] =
                serde_json::json!(if en_stock.is_finite() { en_stock } else { -1.0 });
            l["retournable"] = serde_json::json!(restant.min(en_stock.max(0.0)));
            lignes_completes.push(l);
        }

        let total: i64 = lignes_completes
            .iter()
            .map(|l| {
                let q = l["quantite"].as_f64().unwrap_or(0.0);
                let p = l["prix_achat"].as_i64().unwrap_or(0);
                (p as f64 * q).round() as i64
            })
            .sum();

        resultat.push(serde_json::json!({
            "piece_id":   piece_id,
            "numero":     numero,
            "date_piece": date_piece,
            "statut":     statut,
            "depot_nom":  depot_nom,
            "total":      total,
            "lignes":     lignes_completes,
        }));
    }

    Ok(resultat)
}

pub fn valider_facture_fournisseur_sur_base(
    base: &mut Base,
    piece_id: String,
    mode_reglement: String,
    mode_paiement: Option<String>,
    acompte: Option<i64>,
    utilisateur_role: Option<String>,
) -> Result<serde_json::Value, String> {
    let dossier = base.dossier().to_string();
    let (fournisseur_id, type_p, statut, depot_opt, numero): (String, String, String, Option<String>, String) =
        base.lire_une(
            "SELECT tiers_id, type_piece, statut, depot_id, numero
             FROM piece_commerciale WHERE id = ?1 AND dossier_id = ?2",
            &parametres![piece_id.clone(), dossier.clone()],
            |r| {
                Ok((
                    r.get::<String>(0)?,
                    r.get::<String>(1)?,
                    r.get::<String>(2)?,
                    r.get::<Option<String>>(3)?,
                    r.get::<String>(4)?,
                ))
            },
        )
        .map_err(|e| e.0)?
        .ok_or_else(|| "Pièce introuvable".to_string())?;

    if type_p != "facture_fournisseur" {
        return Err("Seule une facture fournisseur se valide ici".to_string());
    }
    if statut != "brouillon" {
        return Err(format!("Facture déjà en statut « {statut} »"));
    }

    let lignes = lire_lignes_raw_sur(base, &piece_id)?;
    if lignes.is_empty() {
        return Err("La facture ne contient aucune ligne".to_string());
    }

    // Le total se recalcule depuis les lignes, jamais depuis l'ecran.
    let total: i64 = lignes
        .iter()
        .map(|(_, _, qte, prix, remise_pct, _)| {
            let brut = (*prix as f64 * qte).round() as i64;
            brut - (brut as f64 * remise_pct / 100.0).round() as i64
        })
        .sum();

    let comptant = mode_reglement == "comptant";
    let montant_regle = if comptant { total } else { acompte.unwrap_or(0).max(0) };

    let session = if montant_regle > 0 {
        Some(crate::caisses::exiger_sur(base, None)?)
    } else {
        None
    };

    let role = utilisateur_role.as_deref().unwrap_or("employe");
    let auteur = id_utilisateur_par_role_sur(base, role);
    let now = maintenant_iso();
    let stock_ailleurs = crate::pieces::stock_confie_a_un_bon_sur(base, &dossier, &piece_id);
    let depot = crate::pieces::depot_de_piece_sur(base, depot_opt);

    if !stock_ailleurs && depot.is_none() {
        return Err(
            "Aucun magasin actif : impossible de faire entrer cette marchandise."
                .to_string(),
        );
    }

    let mut tx = base.transaction().map_err(|e| e.0)?;

    // ---- 1. Stock, si aucun bon ne l'a deja fait entrer.
    if !stock_ailleurs {
        let depot = depot.clone().unwrap_or_default();
        for (article_id, unite_id, qte, prix, _, _) in &lignes {
            let facteur: f64 = tx
                .lire_une(
                    "SELECT facteur FROM unite_vente WHERE id = ?1",
                    &parametres![unite_id.clone()],
                    |r| r.get::<f64>(0),
                )
                .ok()
                .flatten()
                .unwrap_or(1.0);

            tx.executer(
                "INSERT INTO mouvement_stock
                 (id, article_id, depot_id, type_mouvement, quantite_delta,
                  operation_id, auteur_id, date_mouvement, cree_le, cree_par,
                  origine, fournisseur_id, prix_achat_unitaire, dossier_id)
                 VALUES (?1,?2,?3,'achat',?4,?5,?6,?7,?7,?6,'app',?8,?9,?10)",
                &parametres![
                    uuid::Uuid::new_v4().to_string(),
                    article_id.clone(),
                    depot.clone(),
                    qte * facteur,
                    piece_id.clone(),
                    auteur.clone(),
                    now.clone(),
                    fournisseur_id.clone(),
                    *prix,
                    dossier.clone()
                ],
            )
            .map_err(|e| e.0)?;

            // Le dernier prix d'achat suit la facture, qui porte le
            // prix reellement consenti.
            let _ = tx.executer(
                "UPDATE article SET dernier_prix_achat = ?1, modifie_le = ?2 WHERE id = ?3",
                &parametres![*prix, now.clone(), article_id.clone()],
            );
        }
    }

    // ---- 2. Statut, selon ce qui est REELLEMENT regle.
    let statut_piece = if montant_regle >= total && total > 0 { "paye" } else { "emis" };
    tx.executer(
        "UPDATE piece_commerciale SET statut = ?1, modifie_le = ?2
         WHERE id = ?3 AND dossier_id = ?4",
        &parametres![statut_piece, now.clone(), piece_id.clone(), dossier.clone()],
    )
    .map_err(|e| e.0)?;

    // ---- 3. Le reglement, et sa sortie de caisse.
    if montant_regle > 0 {
        let moyen = mode_paiement.as_deref().unwrap_or("especes");
        tx.executer(
            "INSERT INTO paiement_fournisseur
             (id, fournisseur_id, piece_id, montant, mode, note,
              auteur_id, date_paiement, cree_le, origine, dossier_id)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?8,'app',?9)",
            &parametres![
                uuid::Uuid::new_v4().to_string(),
                fournisseur_id.clone(),
                piece_id.clone(),
                montant_regle,
                moyen,
                format!("Facture {numero}"),
                auteur.clone(),
                now.clone(),
                dossier.clone()
            ],
        )
        .map_err(|e| e.0)?;

        let sid = session.ok_or_else(|| "CAISSE_FERMEE".to_string())?;
        sortie_de_caisse(&mut tx, &sid, moyen, montant_regle, "achat", &piece_id, &now, &auteur)?;
    }

    journaliser(
        &mut tx,
        "facture_fournisseur_validee",
        "piece_commerciale",
        &piece_id,
        &auteur,
        format!("{{\"total\":{total},\"regle\":{montant_regle},\"statut\":\"{statut_piece}\"}}"),
        &now,
    );

    tx.valider().map_err(|e| e.0)?;

    Ok(serde_json::json!({
        "piece_id":      piece_id,
        "numero":        numero,
        "statut":        statut_piece,
        "total":         total,
        "montant_regle": montant_regle,
        "stock_entre":   !stock_ailleurs,
    }))
}

pub fn annuler_facture_fournisseur_par_avoir_sur_base(
    base: &mut Base,
    piece_id: String,
    mode_resolution: Option<String>,
    mode_encaissement: Option<String>,
    motif: Option<String>,
    utilisateur_role: Option<String>,
) -> Result<serde_json::Value, String> {
    let dossier = base.dossier().to_string();
    let (type_piece, statut, fournisseur_id, numero): (String, String, String, String) = base
        .lire_une(
            "SELECT type_piece, statut, tiers_id, numero
             FROM piece_commerciale WHERE id = ?1 AND dossier_id = ?2",
            &parametres![piece_id.clone(), dossier.clone()],
            |r| {
                Ok((
                    r.get::<String>(0)?,
                    r.get::<String>(1)?,
                    r.get::<String>(2)?,
                    r.get::<String>(3)?,
                ))
            },
        )
        .map_err(|e| e.0)?
        .ok_or_else(|| "Pièce introuvable".to_string())?;

    if type_piece != "facture_fournisseur" {
        return Err("Seule une facture fournisseur s'annule ainsi".to_string());
    }
    if statut == "annule" {
        return Err("Facture déjà annulée".to_string());
    }
    // Un brouillon n'a produit ni dette ni stock : rien a annuler.
    if statut == "brouillon" {
        return Err(
            "Cette facture est encore en brouillon : la modifier ou la \
             supprimer, un avoir n'a rien à annuler."
                .to_string(),
        );
    }

    let lignes: Vec<LigneRetourFournisseur> = base
        .lire_plusieurs(
            "SELECT lp.article_id, lp.unite_vente_id, lp.quantite,
                    COALESCE(uv.facteur, 1.0), lp.prix_unitaire
             FROM ligne_piece lp
             LEFT JOIN unite_vente uv ON uv.id = lp.unite_vente_id
             WHERE lp.piece_id = ?1 AND lp.dossier_id = ?2",
            &parametres![piece_id.clone(), dossier.clone()],
            |r| {
                Ok(LigneRetourFournisseur {
                    article_id: r.get::<String>(0)?,
                    unite_vente_id: r.get::<String>(1)?,
                    quantite: r.get::<f64>(2)?,
                    facteur: r.get::<f64>(3)?,
                    prix_achat: r.get::<i64>(4)?,
                })
            },
        )
        .map_err(|e| e.0)?;

    if lignes.is_empty() {
        return Err("Cette facture n'a aucune ligne à rendre".to_string());
    }

    // Le retour integral fait tout : stock, AVF, dette ou caisse. Le
    // depot n'est pas transmis, il prend celui de la facture.
    let resultat = enregistrer_retour_fournisseur_sur_base(
        base,
        fournisseur_id,
        None,
        lignes,
        Some(piece_id.clone()),
        mode_resolution,
        mode_encaissement,
        Some(motif.unwrap_or_else(|| format!("Annulation de la facture {numero}"))),
        utilisateur_role,
    )?;

    let now = maintenant_iso();
    base.executer(
        "UPDATE piece_commerciale
         SET statut = 'annule', note = ?1, modifie_le = ?2
         WHERE id = ?3 AND dossier_id = ?4",
        &parametres!["Annulée par avoir fournisseur", now, piece_id, dossier],
    )
    .map_err(|e| e.0)?;

    Ok(resultat)
}
