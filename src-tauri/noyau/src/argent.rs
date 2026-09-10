//! Le domaine argent : la vente, la facture, la numérotation.
//!
//! ## Pourquoi ce code a déménagé sans être récrit
//!
//! `creer_vente` et `valider_facture` sont les deux endroits où le
//! stock sort et où l'argent entre. Les récrire pour les porter vers le
//! serveur, même fidèlement en apparence, c'est exactement ce qui coûte
//! ses livres à un commerçant. Le texte a donc été coupé et collé tel
//! quel ; seuls les préfixes de chemin ont changé.
//!
//! Le filet : les 26 scénarios de `tests_multi_depot` jouent ces
//! fonctions sur une base en mémoire et vérifient ce que le SQL fait
//! réellement — vente répartie, découvert constaté, dépôt de sortie,
//! annulation par avoir. Ils continuent de les appeler à travers les
//! façades restées dans `commandes/`.
//!
//! ## Ce qui reste dehors
//!
//! Les commandes Tauri elles-mêmes. Une `#[tauri::command]` ne peut pas
//! vivre ici — le noyau ne connaît pas Tauri, et c'est ce qui permet au
//! serveur de le lier sans embarquer une fenêtre.

use rusqlite::Connection;

use crate::utils::maintenant_iso;

pub fn id_utilisateur_courant_pub(conn: &Connection) -> String {
    conn.query_row(
        "SELECT id FROM utilisateur WHERE actif = 1 LIMIT 1",
        [], |row| row.get(0),
    ).unwrap_or_else(|_| "system".to_string())
}

pub fn id_utilisateur_par_role(conn: &Connection, role: &str) -> String {
    conn.query_row(
        "SELECT u.id FROM utilisateur u
         JOIN role r ON r.id = u.role_id
         WHERE r.nom = ?1 AND u.actif = 1 LIMIT 1",
        rusqlite::params![role],
        |row| row.get(0),
    ).unwrap_or_else(|_| id_utilisateur_courant_pub(conn))
}

pub fn prochain_numero(conn: &rusqlite::Connection, type_piece: &str) -> String {
    let annee = chrono::Local::now().format("%Y").to_string();
    let prefix = match type_piece {
        "devis"            => "DEV",
        "proforma"         => "PRO",
        "commande_client"  => "CMD",
        "bon_livraison"    => "BL",
        "facture"          => "FAC",
        "facture_acompte"  => "ACP",
        "avoir_client"     => "AVC",
        // Cote fournisseur — absents jusqu'ici : les 4 types tombaient sur
        // "PIE" avec un compteur par type, donc collision sur numero UNIQUE.
        "bon_commande_fournisseur" => "BCF",
        "bon_reception"            => "BRF",
        "facture_fournisseur"      => "FAF",
        "avoir_fournisseur"        => "AVF",
        _                  => "PIE",
    };
    // MAX et non COUNT : une piece supprimee ou deux creations simultanees
    // rejouaient un numero deja pris (numero est UNIQUE -> erreur bloquante).
    // On filtre sur le prefixe seul : un prefixe = un type = un compteur.
    let motif = format!("{}-{}-%", prefix, annee);
    let dernier: i64 = conn.query_row(
        "SELECT COALESCE(MAX(CAST(substr(numero, -5) AS INTEGER)), 0)
         FROM piece_commerciale WHERE numero LIKE ?1",
        rusqlite::params![motif],
        |r| r.get(0),
    ).unwrap_or(0);
    let _ = type_piece;
    format!("{}-{}-{:05}", prefix, annee, dernier + 1)
}

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
    /// Vente au-dela du stock disponible. Le POS calcule le drapeau et
    /// l'affiche en orange, mais il n'etait pas transmis : la colonne
    /// `ligne_vente.vente_a_decouvert` existait depuis l'origine et
    /// restait a 0 partout. Sans elle, impossible de compter les ventes
    /// a decouvert apres coup.
    pub a_decouvert: Option<bool>,
}

pub fn creer_vente_sur(
    conn: &mut rusqlite::Connection,
    client_id: String,
    depot_id: String,
    mode_reglement: String,
    lignes: Vec<ParamsLigneInput>,
    utilisateur_role: Option<String>,
    montant_paye: Option<i64>,
    mode_paiement: Option<String>,
    avoir_montant: Option<i64>,
) -> Result<serde_json::Value, String> {
    let role = utilisateur_role.as_deref().unwrap_or("employe");
    let auteur_id = id_utilisateur_par_role(&conn, role);
    let now = maintenant_iso();
    let vente_id = uuid::Uuid::new_v4().to_string();

    let client_generique = crate::utils::est_client_generique(&conn, &client_id);

    // Un encaissement reel exige une caisse ouverte : sans session,
    // l'argent entre dans le tiroir sans `mouvement_caisse`, et la
    // cloture du soir affiche un excedent inexplicable. Verifie AVANT
    // la transaction, comme les autres refus.
    if montant_paye.unwrap_or(0) > 0 {
        crate::utils::exiger_session_caisse(&conn)?;
    }

    // D40 — refus AVANT d'ouvrir la transaction : rien a defaire.
    if mode_reglement == "credit" && client_generique {
        return Err("Pas de crédit pour un client comptant. \
                    Créer ou sélectionner le client d'abord.".to_string());
    }

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

        // Decouvert : recalcule ICI, dans la transaction, jamais pris
        // du POS.
        //
        // Le drapeau arrivait de l'ecran, ou il est calcule sur le
        // stock lu au chargement (`lire_articles_avec_unites`). Entre
        // ce chargement et la vente, quelqu'un d'autre a pu vendre le
        // dernier sac — au comptoir a cote, ou depuis une autre
        // caisse. L'ecran affiche encore 1, le caissier vend, et la
        // ligne s'ecrit `vente_a_decouvert = 0` pendant que le stock
        // passe a -1 : le decouvert devient INVISIBLE, donc jamais
        // regularise. C'est le seul defaut qui compte ici — pas la
        // vente elle-meme, qu'on accepte.
        //
        // `valider_facture_sur` procede deja ainsi (pieces.rs). Le
        // POS etait la derniere voie a faire confiance au client.
        let qte_base = ligne.quantite * ligne.facteur;
        let dispo: f64 = tx.query_row(
            "SELECT COALESCE(quantite, 0) FROM stock_depot
             WHERE article_id = ?1 AND depot_id = ?2",
            rusqlite::params![ligne.article_id, ligne.depot_source_id],
            |r| r.get(0),
        ).unwrap_or(0.0);
        // Le drapeau du POS reste un OU : il porte le cas de la vente
        // repartie, ou le manque est impute a un depot dont la ligne
        // n'est pas celle qu'on lit ici.
        let a_decouvert = crate::coeur::stock::est_a_decouvert(dispo, qte_base)
            || ligne.a_decouvert.unwrap_or(false);

        tx.execute(
            "INSERT INTO ligne_vente
             (id, vente_id, article_id, unite_vente_id, depot_source_id,
              source_approvisionnement, quantite, prix_reference, prix_pratique,
              taux_tva, montant_tva, vente_a_decouvert, cree_le, origine)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,'app')",
            rusqlite::params![
                ligne_id, vente_id, ligne.article_id, ligne.unite_vente_id,
                ligne.depot_source_id, ligne.source_approvisionnement,
                ligne.quantite, ligne.prix_reference, ligne.prix_pratique,
                taux_tva, montant_tva,
                a_decouvert as i64,
                now
            ],
        ).map_err(|e| e.to_string())?;

        // Décrément stock
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
    // Pot commun `client0000` : le premier passant venu raflerait le
    // credit d'un autre. Le POS le bloque deja (Ventes.tsx), mais la
    // commande est appelable directement.
    let avoir_demande = if client_generique {
        0
    } else {
        avoir_montant.unwrap_or(0).min(total)
    };
    if avoir_demande > 0 {
        let avoirs: Vec<(String, i64, Option<String>)> = {
            let mut st = tx.prepare(
                "SELECT id, montant, piece_id FROM avoir
                 WHERE client_id = ?1 AND statut = 'ouvert'
                 ORDER BY cree_le ASC"
            ).map_err(|e| e.to_string())?;
            let v = st.query_map(rusqlite::params![client_id], |r| {
                Ok((r.get::<_,String>(0)?, r.get::<_,i64>(1)?,
                    r.get::<_,Option<String>>(2)?))
            }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
            v
        };

        let mut reste = avoir_demande;
        for (avoir_id, montant_avoir, piece_avoir) in avoirs {
            if reste <= 0 { break; }
            tx.execute(
                "UPDATE avoir SET statut = 'consomme', vente_utilisation_id = ?1
                 WHERE id = ?2",
                rusqlite::params![vente_id, avoir_id],
            ).map_err(|e| e.to_string())?;

            if montant_avoir > reste {
                // Solde non consomme : nouvel avoir ouvert.
                tx.execute(
                    // `piece_id` herite : l'AVC d'origine continue
                    // d'afficher le credit restant (bug #8).
                    "INSERT INTO avoir
                     (id, client_id, piece_id, montant, statut, cree_le, origine)
                     VALUES (?1,?2,?3,?4,'ouvert',?5,'avoir_solde')",
                    rusqlite::params![
                        uuid::Uuid::new_v4().to_string(),
                        client_id, piece_avoir, montant_avoir - reste, now
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

pub fn lire_lignes_raw(
    conn: &rusqlite::Connection,
    piece_id: &str,
) -> Result<Vec<(String, String, f64, i64, f64, f64)>, String> {
    let mut stmt = conn.prepare(
        "SELECT article_id, unite_vente_id, quantite, prix_unitaire,
                remise_pct, taux_tva
         FROM ligne_piece WHERE piece_id = ?1"
    ).map_err(|e| e.to_string())?;

    let x = stmt.query_map(rusqlite::params![piece_id], |row| {
        Ok((
            row.get::<_,String>(0)?, row.get::<_,String>(1)?,
            row.get::<_,f64>(2)?,   row.get::<_,i64>(3)?,
            row.get::<_,f64>(4)?,   row.get::<_,f64>(5)?,
        ))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
    Ok(x)
}

pub fn valider_facture_sur(
    conn: &mut rusqlite::Connection,
    piece_id: String,
    mode_reglement: String,
    mode_paiement: Option<String>,
    acompte: Option<i64>,
    utilisateur_role: Option<String>,
) -> Result<serde_json::Value, String> {
    // Vérifier que c'est bien une facture brouillon
    let (client_id, type_p, statut, remise_g, depot_id_opt, numero):
        (String, String, String, f64, Option<String>, String) =
        conn.query_row(
            "SELECT tiers_id, type_piece, statut, remise_globale, depot_id, numero
             FROM piece_commerciale WHERE id = ?1",
            rusqlite::params![piece_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
        ).map_err(|_| "Pièce introuvable".to_string())?;

    if type_p != "facture" {
        return Err("Seules les factures peuvent être validées".to_string());
    }
    if statut != "brouillon" {
        return Err(format!("Facture déjà en statut '{}'", statut));
    }

    // Lire les lignes
    let lignes_raw = lire_lignes_raw(&conn, &piece_id)?;
    if lignes_raw.is_empty() {
        return Err("La facture ne contient aucune ligne".to_string());
    }

    // Dépôt par défaut si non spécifié
    let depot_id = match depot_id_opt {
        Some(d) if !d.is_empty() => d,
        _ => conn.query_row(
            "SELECT id FROM depot WHERE est_defaut = 1 LIMIT 1",
            [], |r| r.get(0)
        ).map_err(|_| "Aucun dépôt par défaut".to_string())?,
    };

    let role = utilisateur_role.as_deref().unwrap_or("employe");
    let auteur_id = id_utilisateur_par_role(&conn, role);
    let now = maintenant_iso();

    // Calculer les montants ligne par ligne.
    // La remise globale est REPARTIE au prorata sur chaque ligne, sinon
    // SUM(prix_pratique * quantite) ne correspond pas au montant du et la
    // creance reapparait apres reglement.
    // La TVA est AJOUTEE au HT : le client doit le TTC.
    struct MontantsLigne {
        montant_tva: i64,
        prix_pratique: i64,
    }
    let montants: Vec<MontantsLigne> = lignes_raw.iter()
        .map(|(_, _, qte, prix, remise_pct, taux_tva)| {
            let brut = (*prix as f64 * qte).round() as i64;
            let remise = (brut as f64 * remise_pct / 100.0).round() as i64;
            let ht_ligne = brut - remise;
            // Quote-part de remise globale (meme pourcentage sur chaque ligne).
            let remise_g_ligne = (ht_ligne as f64 * remise_g / 100.0).round() as i64;
            let montant_ht = ht_ligne - remise_g_ligne;
            let montant_tva = (montant_ht as f64 * taux_tva).round() as i64;
            let montant_ttc = montant_ht + montant_tva;
            // Prix unitaire TTC arrondi au franc : c'est LUI qui est stocke.
            let prix_pratique = if *qte > 0.0 {
                (montant_ttc as f64 / qte).round() as i64
            } else { *prix };
            MontantsLigne { montant_tva, prix_pratique }
        }).collect();

    // Le montant du doit etre derive des prix_pratique REELLEMENT stockes,
    // pas de la somme arithmetique des TTC de lignes : toutes les requetes
    // recalculent CAST(SUM(prix_pratique * quantite) AS INTEGER), et
    // l'arrondi du prix unitaire fait diverger les deux. Une divergence
    // d'un seul franc laisse une creance residuelle apres reglement.
    let total_net: i64 = lignes_raw.iter().enumerate()
        .map(|(i, (_, _, qte, _, _, _))| montants[i].prix_pratique as f64 * qte)
        .sum::<f64>() as i64;   // `as i64` tronque, comme le CAST de SQLite

    let tx = conn.transaction().map_err(|e| e.to_string())?;

    // 1. Créer la vente
    let vente_id = uuid::Uuid::new_v4().to_string();
    tx.execute(
        // piece_id : SANS ce lien, regler_creance ne retrouve pas la
        // piece a passer en 'paye'. Une facture soldee restait 'emis',
        // donc encore modifiable — c'etait le trou signale.
        "INSERT INTO vente
         (id, client_id, depot_id, mode_reglement, auteur_id, statut,
          date_vente, cree_le, modifie_le, cree_par, modifie_par, origine,
          piece_id)
         VALUES (?1,?2,?3,?4,?5,'creance_ouverte',?6,?7,?8,?9,?10,'app',?11)",
        rusqlite::params![
            vente_id, client_id, depot_id, mode_reglement,
            auteur_id, now, now, now, auteur_id, auteur_id, piece_id
        ],
    ).map_err(|e| e.to_string())?;

    // 2. Insérer les lignes de vente + décrémenter le stock
    for (i, (art_id, uv_id, qte, prix_u, _remise_pct, taux_tva))
        in lignes_raw.iter().enumerate()
    {
        let montant_tva   = montants[i].montant_tva;
        let prix_pratique = montants[i].prix_pratique;

        // Facteur de l'unité de vente
        let facteur: f64 = tx.query_row(
            "SELECT facteur FROM unite_vente WHERE id = ?1",
            rusqlite::params![uv_id], |r| r.get(0),
        ).unwrap_or(1.0);

        // Découvert : marchandise sortie au-dela du stock connu. La
        // colonne n'etait jamais renseignee par cette voie — une
        // facture validee depuis l'ecran Pieces pouvait mettre un stock
        // a -12 sans apparaitre dans « ventes a decouvert », donc sans
        // jamais etre regularisee.
        let qte_base = qte * facteur;
        let dispo: f64 = tx.query_row(
            "SELECT COALESCE(quantite, 0) FROM stock_depot
             WHERE article_id = ?1 AND depot_id = ?2",
            rusqlite::params![art_id, depot_id], |r| r.get(0),
        ).unwrap_or(0.0);
        let a_decouvert = crate::coeur::stock::est_a_decouvert(dispo, qte_base);

        tx.execute(
            "INSERT INTO ligne_vente
             (id, vente_id, article_id, unite_vente_id, depot_source_id,
              source_approvisionnement, quantite, prix_reference,
              prix_pratique, taux_tva, montant_tva, vente_a_decouvert,
              cree_le, origine)
             VALUES (?1,?2,?3,?4,?5,'stock',?6,?7,?8,?9,?10,?12,?11,'app')",
            rusqlite::params![
                uuid::Uuid::new_v4().to_string(),
                vente_id, art_id, uv_id, depot_id,
                qte, prix_u, prix_pratique,
                taux_tva, montant_tva, now,
                a_decouvert as i64
            ],
        ).map_err(|e| e.to_string())?;

        // Décrémenter stock
        tx.execute(
            "INSERT INTO stock_depot (id, article_id, depot_id, quantite)
             VALUES (?1,?2,?3,0 - ?4)
             ON CONFLICT(article_id, depot_id)
             DO UPDATE SET quantite = quantite - ?4",
            rusqlite::params![
                uuid::Uuid::new_v4().to_string(), art_id, depot_id, qte_base
            ],
        ).map_err(|e| e.to_string())?;

        tx.execute(
            "INSERT INTO mouvement_stock
             (id, article_id, depot_id, type_mouvement, quantite_delta,
              operation_id, auteur_id, date_mouvement, cree_le, cree_par, origine)
             VALUES (?1,?2,?3,'vente',?4,?5,?6,?7,?8,?9,'app')",
            rusqlite::params![
                uuid::Uuid::new_v4().to_string(),
                art_id, depot_id, -qte_base,
                vente_id, auteur_id, now, now, auteur_id
            ],
        ).map_err(|e| e.to_string())?;
    }

    // 3. Numéro de facture lié à la vente
    // La table `facture` legacy n'est plus alimentee : piece_commerciale
    // est le referentiel unique. Elle l'etait encore ici, donc chaque
    // facture validee depuis l'ecran Pieces portait DEUX numeros —
    // GESCOM-… et FAC-…. Le client en voyait un, l'ecran l'autre.

    // 4. Paiement si comptant
    if mode_reglement == "comptant" {
        let mode_p = mode_paiement.as_deref().unwrap_or("especes");
        tx.execute(
            "INSERT INTO paiement
             (id, vente_id, montant, mode, date_paiement,
              auteur_id, cree_le, cree_par, origine)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,'app')",
            rusqlite::params![
                uuid::Uuid::new_v4().to_string(),
                vente_id, total_net, mode_p, now,
                auteur_id, now, auteur_id
            ],
        ).map_err(|e| e.to_string())?;

        tx.execute(
            "UPDATE vente SET statut = 'payee', modifie_le = ?1 WHERE id = ?2",
            rusqlite::params![now, vente_id],
        ).map_err(|e| e.to_string())?;

        // Caisse — encaissement reel, session exigee.
        let session_id = crate::utils::exiger_session_caisse(&tx)?;
        {
            tx.execute(
                "INSERT INTO mouvement_caisse
                 (id, session_id, sens, moyen, montant, motif,
                  operation_id, date_mouvement, cree_le, cree_par, origine)
                 VALUES (?1,?2,'entree',?3,?4,'vente',?5,?6,?7,?8,'app')",
                rusqlite::params![
                    uuid::Uuid::new_v4().to_string(), session_id,
                    mode_p, total_net, vente_id, now, now, auteur_id
                ],
            ).ok();
        }
    } else if let Some(ac) = acompte {
        if ac > 0 {
            let mode_p = mode_paiement.as_deref().unwrap_or("especes");
            tx.execute(
                "INSERT INTO paiement
                 (id, vente_id, montant, mode, date_paiement,
                  auteur_id, cree_le, cree_par, origine)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,'app')",
                rusqlite::params![
                    uuid::Uuid::new_v4().to_string(),
                    vente_id, ac, mode_p, now,
                    auteur_id, now, auteur_id
                ],
            ).map_err(|e| e.to_string())?;

            // ENTREE de caisse : l'acompte est de l'argent recu. La
            // branche comptant le faisait, pas celle-ci — l'acompte
            // n'apparaissait donc jamais dans le tiroir et la cloture
            // affichait un manque de ce montant.
            // Acompte reellement encaisse : caisse ouverte exigee.
            let sid = crate::utils::exiger_session_caisse(&tx)?;
            {
                tx.execute(
                    "INSERT INTO mouvement_caisse
                     (id, session_id, sens, moyen, montant, motif,
                      operation_id, date_mouvement, cree_le, cree_par, origine)
                     VALUES (?1,?2,'entree',?3,?4,'vente',?5,?6,?7,?8,'app')",
                    rusqlite::params![
                        uuid::Uuid::new_v4().to_string(),
                        sid, mode_p, ac, vente_id, now, now, auteur_id
                    ],
                ).map_err(|e| e.to_string())?;
            }

            let statut_v = if crate::coeur::calcul::reste_exigible(total_net, ac) == 0
                { "payee" } else { "partiellement_payee" };
            tx.execute(
                "UPDATE vente SET statut = ?1, modifie_le = ?2 WHERE id = ?3",
                rusqlite::params![statut_v, now, vente_id],
            ).map_err(|e| e.to_string())?;
        }
    }

    // 5. Statut de la piece selon ce qui est REELLEMENT encaisse.
    //    Soldee -> "validee" (effets produits et rien du).
    //    Acompte ou credit sec -> "emis", passera a "paye" au solde
    //    de la creance (creances.rs).
    let encaisse: i64 = if mode_reglement == "comptant" {
        total_net
    } else {
        acompte.unwrap_or(0)
    };
    // Statuts factures : 'paye' (soldee) ou 'emis' (reste du).
    // 'validee' voulait dire "effets produits", mais toute facture qui
    // existe a produit ses effets — le mot n'apportait rien et faisait
    // doublon avec 'paye'.
    let statut_piece = if crate::coeur::calcul::reste_exigible(total_net, encaisse) == 0
        { "paye" } else { "emis" };

    tx.execute(
        "UPDATE piece_commerciale
         SET statut = ?1, modifie_le = ?2 WHERE id = ?3",
        rusqlite::params![statut_piece, now, piece_id],
    ).map_err(|e| e.to_string())?;

    tx.commit().map_err(|e| e.to_string())?;

    Ok(serde_json::json!({
        "vente_id":      vente_id,
        // Le numero est celui de la piece elle-meme (FAC-…), plus un
        // GESCOM- distinct comme avant.
        "numero_facture": numero,
        "total_net":     total_net,
        "statut_piece":  statut_piece,
        "statut_vente":  if crate::coeur::calcul::reste_exigible(total_net, encaisse) == 0
                         { "payee" }
                         else if encaisse > 0 { "partiellement_payee" }
                         else { "creance_ouverte" },
    }))
}

pub fn creer_facture_depuis_vente_sur(
    conn: &rusqlite::Connection,
    vente_id: String,
    client_id: String,
    mode_reglement: String,
    utilisateur_role: Option<String>,
) -> Result<serde_json::Value, String> {

    let role = utilisateur_role.as_deref().unwrap_or("employe");
    let auteur_id = id_utilisateur_par_role(&conn, role);
    let now = maintenant_iso();

    // Numérotation — meme fonction que les pieces saisies manuellement.
    // Avant : COUNT(*) local, en parallele de prochain_numero. Deux
    // compteurs pour la meme sequence FAC- = collision sur numero UNIQUE
    // des qu'une facture est creee des deux cotes.
    let numero = prochain_numero(&conn, "facture");
    let piece_id = uuid::Uuid::new_v4().to_string();

    // Statut selon ce qui est REELLEMENT encaisse, pas selon le mode
    // annonce. Une vente a credit avec acompte n'est ni "emis" tout
    // court (le client a paye une partie) ni soldee.
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

    let statut = if total_vente > 0
        && crate::coeur::calcul::reste_exigible(total_vente, total_paye) == 0 {
        // Soldee des l'emission — comptant, ou credit paye d'avance.
        // 'paye' et non 'validee' : toute facture existante a produit
        // ses effets, seul le reglement distingue les etats.
        "paye"
    } else {
        // Emise, avec ou sans acompte. Passera a "paye" au solde de la
        // creance (voir creances.rs).
        "emis"
    };
    let _ = &mode_reglement;

    // Créer la pièce
    conn.execute(
        "INSERT INTO piece_commerciale
         (id, type_piece, numero, statut, tiers_type, tiers_id,
          auteur_id, date_piece, remise_globale, note, cree_le, modifie_le, origine)
         VALUES (?1,'facture',?2,?3,'client',?4,?5,?6,0.0,?7,?8,?9,'pos')",
        rusqlite::params![
            piece_id, numero, statut,
            client_id, auteur_id, now,
            if total_paye > 0 && total_paye < total_vente {
                format!("Vente POS — acompte reçu {} F", total_paye)
            } else {
                "Vente POS".to_string()
            },
            now, now
        ],
    ).map_err(|e| e.to_string())?;

    // Copier les lignes de vente → lignes de pièce
    let mut stmt = conn.prepare(
        "SELECT lv.article_id, lv.unite_vente_id, lv.quantite,
                lv.prix_pratique,
                CAST(COALESCE(lv.montant_tva, 0) AS INTEGER),
                CAST(COALESCE(lv.taux_tva, 0.0) AS REAL)
         FROM ligne_vente lv
         WHERE lv.vente_id = ?1"
    ).map_err(|e| e.to_string())?;

    let lignes: Vec<(String, String, f64, i64, i64, f64)> = stmt
        .query_map(rusqlite::params![vente_id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?,
                r.get(3)?, r.get(4)?, r.get(5)?))
        }).map_err(|e| e.to_string())?
        .filter_map(|r| r.ok()).collect();

    for (art_id, uv_id, qte, prix, tva, taux_tva) in &lignes {
        // prix_pratique est du TTC (D8). Une ligne_piece stocke du HT,
        // comme les pieces saisies manuellement -> on convertit.
        let montant_ttc = (*prix as f64 * qte).round() as i64;
        let montant_ht  = montant_ttc - *tva;
        let prix_ht = if *qte > 0.0 {
            (montant_ht as f64 / qte).round() as i64
        } else {
            *prix
        };
        conn.execute(
            "INSERT INTO ligne_piece
             (id, piece_id, article_id, unite_vente_id, quantite,
              prix_unitaire, remise_pct, remise_montant,
              taux_tva, montant_tva, montant_ht, cree_le)
             VALUES (?1,?2,?3,?4,?5,?6,0.0,0,?7,?8,?9,?10)",
            rusqlite::params![
                uuid::Uuid::new_v4().to_string(),
                piece_id, art_id, uv_id, qte,
                prix_ht, taux_tva, tva, montant_ht, now
            ],
        ).map_err(|e| e.to_string())?;
    }

    // Lier la vente à la pièce via colonne optionnelle
    if let Err(e) = conn.execute(
        "UPDATE vente SET piece_id = ?1 WHERE id = ?2",
        rusqlite::params![piece_id, vente_id],
    ) {
        // Non bloquant : la piece est creee, seul le lien manque.
        // Si ce message apparait, la migration vente.piece_id n'est pas passee.
        eprintln!("[gescom] lien vente->piece non etabli : {}", e);
    }

    Ok(serde_json::json!({
        "piece_id": piece_id,
        "numero":   numero,
        "statut":   statut,
    }))
}
