//! Fournisseurs — liste, création, stock, fiche détail.

use crate::utils::maintenant_iso;

// =====================================================================
//  Liste fournisseurs
// =====================================================================

pub fn lire_fournisseurs(
    conn: &rusqlite::Connection,
) -> Result<Vec<serde_json::Value>, String> {
    let mut stmt = conn.prepare(
        "SELECT id, nom, telephone, adresse, est_voisin
         FROM fournisseur ORDER BY nom"
    ).map_err(|e| e.to_string())?;

    let x = stmt.query_map([], |r| {
        Ok(serde_json::json!({
            "id":        r.get::<_,String>(0)?,
            "nom":       r.get::<_,String>(1)?,
            "telephone": r.get::<_,Option<String>>(2)?,
            "adresse":   r.get::<_,Option<String>>(3)?,
            "est_voisin":r.get::<_,i64>(4)? != 0,
        }))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
    Ok(x)
}

// =====================================================================
//  Fournisseurs avec dettes (pour Chantiers)
// =====================================================================

pub fn lire_fournisseurs_avec_dettes(
    conn: &rusqlite::Connection,
) -> Result<Vec<serde_json::Value>, String> {

    let mut stmt = conn.prepare(
        "SELECT f.id, f.nom, f.telephone,
                CAST(COALESCE(
                  (SELECT COALESCE(SUM(
                     CASE pc.type_piece
                       WHEN 'facture_fournisseur' THEN lp.montant_ht + lp.montant_tva
                       -- Un AVF deja rembourse en especes ('paye') ne
                       -- reduit pas la dette : la caisse l'a deja fait.
                       WHEN 'avoir_fournisseur'   THEN
                            CASE WHEN pc.statut = 'paye' THEN 0
                                 ELSE -(lp.montant_ht + lp.montant_tva) END
                       ELSE 0 END), 0)
                   FROM piece_commerciale pc
                   JOIN ligne_piece lp ON lp.piece_id = pc.id
                   WHERE pc.tiers_type = 'fournisseur'
                     AND pc.tiers_id = f.id
                     AND pc.type_piece IN ('facture_fournisseur','avoir_fournisseur')
                     AND pc.statut <> 'annule')
                , 0) AS INTEGER) as total_achats,
                CAST(COALESCE(
                  (SELECT SUM(pf.montant) FROM paiement_fournisseur pf
                   WHERE pf.fournisseur_id = f.id)
                , 0) AS INTEGER) as total_paye
         FROM fournisseur f
         ORDER BY f.nom"
    ).map_err(|e| e.to_string())?;

    let x = stmt.query_map([], |r| {
        let total_achats: i64 = r.get(3)?;
        let total_paye: i64 = r.get(4)?;
        let dette = (total_achats - total_paye).max(0);
        Ok(serde_json::json!({
            "id":          r.get::<_,String>(0)?,
            "nom":         r.get::<_,String>(1)?,
            "telephone":   r.get::<_,Option<String>>(2)?,
            "total_achats":total_achats,
            "total_paye":  total_paye,
            "dette":       dette,
        }))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
    Ok(x)
}

// =====================================================================
//  Créer fournisseur
// =====================================================================

pub fn creer_fournisseur(
    conn: &rusqlite::Connection,
    nom: String,
    telephone: Option<String>,
    adresse: Option<String>,
    nif: Option<String>,
    email: Option<String>,
    est_voisin: Option<bool>,
) -> Result<serde_json::Value, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = maintenant_iso();
    let voisin = est_voisin.unwrap_or(false) as i64;

    conn.execute(
        "INSERT INTO fournisseur
         (id, nom, telephone, adresse, nif, email, est_voisin, cree_le, modifie_le)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        rusqlite::params![id, nom, telephone, adresse, nif, email, voisin, now, now],
    ).map_err(|e| e.to_string())?;

    Ok(serde_json::json!({"id": id, "nom": nom}))
}

/// Modifie les coordonnees d'un fournisseur.
///
/// `est_voisin` en fait partie : c'est un fait qui change (un
/// fournisseur de depannage devient un fournisseur regulier), pas une
/// donnee figee a la creation.
pub fn modifier_fournisseur(
    conn: &rusqlite::Connection,
    fournisseur_id: String,
    nom: String,
    telephone: Option<String>,
    adresse: Option<String>,
    nif: Option<String>,
    email: Option<String>,
    est_voisin: Option<bool>,
) -> Result<(), String> {
    if nom.trim().is_empty() {
        return Err("Le nom est obligatoire".to_string());
    }

    let existe: i64 = conn.query_row(
        "SELECT COUNT(*) FROM fournisseur WHERE id = ?1",
        rusqlite::params![fournisseur_id], |r| r.get(0),
    ).unwrap_or(0);
    if existe == 0 {
        return Err("Fournisseur introuvable".to_string());
    }

    // Un champ efface redevient NULL, pas une chaine vide : sinon les
    // ecrans affichent une ligne vide au lieu de ne rien afficher.
    let vide = |o: Option<String>| o.filter(|s| !s.trim().is_empty());
    let now = maintenant_iso();

    conn.execute(
        "UPDATE fournisseur
         SET nom = ?1, telephone = ?2, adresse = ?3, nif = ?4, email = ?5,
             est_voisin = ?6, modifie_le = ?7
         WHERE id = ?8",
        rusqlite::params![
            nom.trim(), vide(telephone), vide(adresse), vide(nif), vide(email),
            est_voisin.unwrap_or(false) as i64, now, fournisseur_id
        ],
    ).map_err(|e| e.to_string())?;

    Ok(())
}

/// Etat de la dette envers UN fournisseur — le releve qu'on lui oppose.
///
/// D9 : dette = SUM(FAF) − SUM(AVF non payes) − paiements. D36 : elle se
/// lit dans `paiement_fournisseur`, jamais dans le statut de la piece.
/// Un statut 'paye' pose sans ecriture de paiement laisserait le releve
/// annoncer une dette soldee qui ne l'est pas.
pub fn lire_etat_dette_fournisseur(
    conn: &rusqlite::Connection,
    fournisseur_id: String,
) -> Result<serde_json::Value, String> {

    let tiers = conn.query_row(
        "SELECT nom, telephone, adresse FROM fournisseur WHERE id = ?1",
        rusqlite::params![fournisseur_id],
        |r| Ok(serde_json::json!({
            "nom":       r.get::<_, String>(0)?,
            "code":      "",
            "telephone": r.get::<_, Option<String>>(1)?,
            "adresse":   r.get::<_, Option<String>>(2)?,
        })),
    ).map_err(|_| "Fournisseur introuvable".to_string())?;

    // Pieces engageantes du fournisseur. Une AVF REDUIT la dette, d'ou
    // le signe negatif — et seulement si elle n'est pas deja payee
    // (remboursee en especes), sinon elle compterait deux fois.
    let mut st = conn.prepare(
        "SELECT pc.date_piece, pc.numero, pc.type_piece, pc.statut,
                CAST(COALESCE(SUM(lp.montant_ht + lp.montant_tva), 0) AS INTEGER),
                CAST(COALESCE(
                  (SELECT SUM(pf.montant) FROM paiement_fournisseur pf
                   WHERE pf.piece_id = pc.id), 0) AS INTEGER)
         FROM piece_commerciale pc
         JOIN ligne_piece lp ON lp.piece_id = pc.id
         WHERE pc.tiers_type = 'fournisseur'
           AND pc.tiers_id = ?1
           AND pc.type_piece IN ('facture_fournisseur','avoir_fournisseur')
           AND pc.statut <> 'annule'
         GROUP BY pc.id
         ORDER BY pc.date_piece"
    ).map_err(|e| e.to_string())?;

    let toutes: Vec<serde_json::Value> = st.query_map(
        rusqlite::params![fournisseur_id], |r| {
            let type_piece: String = r.get(2)?;
            let statut: String = r.get(3)?;
            let montant: i64 = r.get(4)?;
            let paye: i64 = r.get(5)?;

            let avoir = type_piece == "avoir_fournisseur";
            // Une AVF remboursee en especes est close : elle ne reduit
            // plus la dette, l'argent est deja revenu.
            let du = if avoir {
                if statut == "paye" { 0 } else { -montant }
            } else {
                crate::coeur::calcul::reste_exigible(montant, paye)
            };

            Ok(serde_json::json!({
                "date":    r.get::<_, String>(0)?,
                "numero":  r.get::<_, String>(1)?,
                "type":    if avoir { "Avoir" } else { "Facture" },
                "total":   montant,
                "paye":    paye,
                "reste":   du,
            }))
        }
    ).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    // Les pieces soldees ne figurent pas : le releve dit ce qui reste
    // du, pas l'historique complet — celui-ci vit dans la fiche.
    let lignes: Vec<serde_json::Value> = toutes.into_iter()
        .filter(|l| l["reste"].as_i64().unwrap_or(0) != 0)
        .collect();

    let total_du: i64 = lignes.iter()
        .filter_map(|l| l["reste"].as_i64()).sum();

    Ok(serde_json::json!({
        "tiers":    tiers,
        "lignes":   lignes,
        "total_du": total_du.max(0),
        "avoirs":   0,
        "net_du":   total_du.max(0),
        "societe":  crate::creances::societe(&conn),
    }))
}

/// Etat GLOBAL des dettes — un fournisseur par ligne.
///
/// D9/D36 : la dette se lit dans `paiement_fournisseur`, jamais dans le
/// statut. Le reste se calcule PAR FACTURE puis s'additionne, pour la
/// meme raison que cote client (seuil de solde, D41).
pub fn lire_etat_dettes_global(
    conn: &rusqlite::Connection,
) -> Result<serde_json::Value, String> {

    let mut st = conn.prepare(
        "SELECT f.id, f.nom, f.telephone, pc.type_piece, pc.statut,
                CAST(COALESCE(SUM(lp.montant_ht + lp.montant_tva), 0) AS INTEGER),
                CAST(COALESCE(
                  (SELECT SUM(pf.montant) FROM paiement_fournisseur pf
                   WHERE pf.piece_id = pc.id), 0) AS INTEGER)
         FROM piece_commerciale pc
         JOIN fournisseur f ON f.id = pc.tiers_id
         JOIN ligne_piece lp ON lp.piece_id = pc.id
         WHERE pc.tiers_type = 'fournisseur'
           AND pc.type_piece IN ('facture_fournisseur','avoir_fournisseur')
           AND pc.statut <> 'annule'
         GROUP BY pc.id
         ORDER BY f.nom, pc.date_piece"
    ).map_err(|e| e.to_string())?;

    // (id, nom, tel, nb, total_du)
    let mut par_f: Vec<(String, String, Option<String>, i64, i64)> = Vec::new();

    let pieces = st.query_map([], |r| {
        let type_piece: String = r.get(3)?;
        let statut: String = r.get(4)?;
        let montant: i64 = r.get(5)?;
        let paye: i64 = r.get(6)?;
        // Une AVF deja remboursee en especes est close : elle ne reduit
        // plus la dette, l'argent est revenu.
        let du = if type_piece == "avoir_fournisseur" {
            if statut == "paye" { 0 } else { -montant }
        } else {
            crate::coeur::calcul::reste_exigible(montant, paye)
        };
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?,
            r.get::<_, Option<String>>(2)?, du))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok());

    for (id, nom, tel, du) in pieces {
        if du == 0 {
            continue;
        }
        match par_f.iter_mut().find(|l| l.0 == id) {
            // Une facture due compte, un avoir ne compte pas comme
            // « facture » — il ne fait que reduire le total.
            Some(l) => { if du > 0 { l.3 += 1; } l.4 += du; }
            None => par_f.push((id, nom, tel, if du > 0 { 1 } else { 0 }, du)),
        }
    }

    // Un fournisseur dont les avoirs depassent les factures n'est pas un
    // creancier : il ne figure pas sur un etat de dette.
    par_f.retain(|l| l.4 > 0);
    par_f.sort_by(|a, b| b.4.cmp(&a.4));

    let total_general: i64 = par_f.iter().map(|l| l.4).sum();
    let lignes: Vec<serde_json::Value> = par_f.into_iter()
        .map(|(_, nom, tel, nb, du)| serde_json::json!({
            "nom": nom, "code": "", "telephone": tel,
            "nb": nb, "total_du": du,
        })).collect();

    Ok(serde_json::json!({
        "lignes":        lignes,
        "total_general": total_general,
        "societe":       crate::creances::societe(&conn),
    }))
}

// =====================================================================
//  Enregistrer entrée stock (achat)
// =====================================================================

pub fn enregistrer_entree_stock(
    conn: &rusqlite::Connection,
    article_id: String,
    depot_id: Option<String>,  // null → dépôt par défaut
    quantite: f64,
    prix_achat: Option<i64>,
    fournisseur_id: Option<String>,
    utilisateur_role: Option<String>,
) -> Result<(), String> {
    let now = maintenant_iso();
    let role = utilisateur_role.as_deref().unwrap_or("employe");
    let auteur = crate::argent::id_utilisateur_par_role(&conn, role);

    // Résoudre le dépôt — utiliser le défaut si null
    let depot_id = match depot_id {
        Some(d) if !d.is_empty() => d,
        _ => conn.query_row(
            "SELECT id FROM depot WHERE est_defaut = 1 LIMIT 1",
            [], |r| r.get(0)
        ).map_err(|_| "Aucun magasin par défaut configuré".to_string())?,
    };

    let op_id = uuid::Uuid::new_v4().to_string();

    // Mettre à jour le stock
    // Le stock suit desormais son mouvement : le declencheur
    // `stock_suit_les_mouvements` met le compteur a jour dans la meme
    // transaction. L'ecrire ici le compterait deux fois.

    // 'entree' et non 'achat' (D42) : cette commande ne cree ni facture
    // fournisseur, ni dette, ni mouvement de caisse. Les confondre
    // gonflait les achats du jour d'un montant que personne ne doit.
    conn.execute(
        "INSERT INTO mouvement_stock
         (id, article_id, depot_id, type_mouvement, quantite_delta,
          operation_id, auteur_id, date_mouvement, cree_le, cree_par, origine,
          fournisseur_id, prix_achat_unitaire)
         VALUES (?1,?2,?3,'entree',?4,?5,?6,?7,?8,?9,'app',?10,?11)",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(),
            article_id, depot_id, quantite,
            op_id, auteur, now, now, auteur,
            fournisseur_id, prix_achat
        ],
    ).map_err(|e| e.to_string())?;

    // Mettre à jour le dernier prix d'achat
    if let Some(px) = prix_achat {
        conn.execute(
            "UPDATE article SET dernier_prix_achat = ?1 WHERE id = ?2",
            rusqlite::params![px, article_id],
        ).ok();
    }

    Ok(())
}

// =====================================================================
//  RETOUR SANS FACTURE — le miroir de l'entrée sans facture
// =====================================================================

/// Rend au fournisseur une marchandise entrée SANS facture.
///
/// ## Pourquoi ce n'est pas un avoir
///
/// `enregistrer_retour_fournisseur` crée une pièce AVF qui vient en
/// déduction de la dette. C'est juste quand la marchandise a été
/// facturée : on doit moins puisqu'on rend une partie.
///
/// Une marchandise entrée par `enregistrer_entree_stock` n'a jamais été
/// facturée — D42, l'entrée n'est pas un achat, elle ne crée ni dette
/// ni mouvement de caisse. Lui fabriquer un avoir créerait un CRÉDIT
/// chez un fournisseur à qui l'on ne doit rien, et `lire_etat_dettes_global`
/// le déduirait d'autres factures. On aurait inventé de l'argent.
///
/// Le retour d'une entrée sans facture est donc, symétriquement, une
/// simple sortie de stock : pas de pièce, pas de dette, pas de caisse.
/// C'est le cas du dépannage entre voisins — on emprunte dix sacs au
/// magasin d'à côté un vendredi, on les rend le lundi.
///
/// ## Pas de reliquat
///
/// Contrairement au retour sur facture, rien ne limite à « ce qui reste
/// de cette entrée-là » : une entrée sans facture n'a aucun document
/// que le commerçant puisse rouvrir pour vérifier. Le seul plafond qui
/// ait un sens est le stock réellement présent.
pub fn enregistrer_retour_sans_facture(
    conn: &rusqlite::Connection,
    article_id: String,
    depot_id: Option<String>,
    quantite: f64,
    fournisseur_id: Option<String>,
    motif: Option<String>,
    utilisateur_role: Option<String>,
) -> Result<(), String> {
    enregistrer_retour_sans_facture_sur(
        &conn, article_id, depot_id, quantite, fournisseur_id, motif,
        utilisateur_role,
    )
}

/// Logique du retour sans facture, sur une connexion quelconque.
///
/// Separee de la commande pour etre jouable sur une base de test.
pub fn enregistrer_retour_sans_facture_sur(
    conn: &rusqlite::Connection,
    article_id: String,
    depot_id: Option<String>,
    quantite: f64,
    fournisseur_id: Option<String>,
    motif: Option<String>,
    utilisateur_role: Option<String>,
) -> Result<(), String> {
    if quantite <= 0.0 {
        return Err("La quantité à rendre doit être positive.".to_string());
    }

    let now = maintenant_iso();
    let role = utilisateur_role.as_deref().unwrap_or("employe");
    let auteur = crate::argent::id_utilisateur_par_role(conn, role);

    let depot_id = match depot_id {
        Some(d) if !d.is_empty() => d,
        _ => conn
            .query_row(
                "SELECT id FROM depot WHERE est_defaut = 1 LIMIT 1",
                [],
                |r| r.get(0),
            )
            .map_err(|_| "Aucun magasin par défaut configuré".to_string())?,
    };

    // Même refus que le retour sur facture : la marchandise doit
    // physiquement quitter la boutique pour repartir. En rendre plus
    // qu'on en détient est une erreur de saisie, pas un événement.
    let dispo: f64 = conn
        .query_row(
            "SELECT COALESCE(quantite, 0) FROM stock_depot
             WHERE article_id = ?1 AND depot_id = ?2",
            rusqlite::params![article_id, depot_id],
            |r| r.get(0),
        )
        .unwrap_or(0.0);

    if quantite > dispo {
        let nom: String = conn
            .query_row(
                "SELECT nom FROM article WHERE id = ?1",
                rusqlite::params![article_id],
                |r| r.get(0),
            )
            .unwrap_or_else(|_| "cet article".to_string());
        return Err(format!(
            "Stock insuffisant pour « {nom} » : {dispo} disponible(s), \
             {quantite} demandé(s)."
        ));
    }

    // Le stock suit desormais son mouvement : le declencheur
    // `stock_suit_les_mouvements` met le compteur a jour dans la meme
    // transaction. L'ecrire ici le compterait deux fois.

    let op_id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO mouvement_stock
         (id, article_id, depot_id, type_mouvement, quantite_delta,
          operation_id, auteur_id, date_mouvement, cree_le, cree_par, origine,
          fournisseur_id)
         VALUES (?1,?2,?3,'retour_fournisseur',?4,?5,?6,?7,?8,?9,'app',?10)",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(),
            article_id,
            depot_id,
            -quantite,
            op_id,
            auteur,
            now,
            now,
            auteur,
            fournisseur_id
        ],
    )
    .map_err(|e| e.to_string())?;

    // Le journal est la seule trace de cette opération : elle ne
    // produit aucune pièce que le commerçant puisse rouvrir. Le motif y
    // est donc conservé tel qu'il a été saisi.
    conn.execute(
        "INSERT INTO journal
         (id, type_evenement, entite_type, entite_id, auteur_id,
          nouveau_valeur, origine, date_evenement)
         VALUES (?1,'retour_sans_facture','article',?2,?3,?4,'app',?5)",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(),
            article_id,
            auteur,
            serde_json::json!({
                "quantite": quantite,
                "depot_id": depot_id,
                "fournisseur_id": fournisseur_id,
                "motif": motif,
            })
            .to_string(),
            now
        ],
    )
    .ok();

    Ok(())
}

// =====================================================================
//  Ajustement inventaire
// =====================================================================

pub fn enregistrer_ajustement_inventaire(
    conn: &rusqlite::Connection,
    article_id: String,
    depot_id: String,
    // `quantite_reelle` et non `nouvelle_quantite` : c'est ce qui a ete
    // COMPTE physiquement dans le depot. Le front envoyait deja ce nom,
    // d'ou l'erreur « invalid args ».
    quantite_reelle: f64,
    motif: Option<String>,
    utilisateur_role: Option<String>,
) -> Result<(), String> {

    let quantite_actuelle: f64 = conn.query_row(
        "SELECT COALESCE(quantite, 0) FROM stock_depot
         WHERE article_id = ?1 AND depot_id = ?2",
        rusqlite::params![article_id, depot_id],
        |r| r.get(0),
    ).unwrap_or(0.0);

    let delta = quantite_reelle - quantite_actuelle;
    let now = maintenant_iso();
    let role = utilisateur_role.as_deref().unwrap_or("employe");
    let auteur = crate::argent::id_utilisateur_par_role(&conn, role);
    let op_id = uuid::Uuid::new_v4().to_string();

    // Le stock suit desormais son mouvement : le declencheur
    // `stock_suit_les_mouvements` met le compteur a jour dans la meme
    // transaction. L'ecrire ici le compterait deux fois.

    conn.execute(
        "INSERT INTO mouvement_stock
         (id, article_id, depot_id, type_mouvement, quantite_delta,
          motif, operation_id, auteur_id, date_mouvement, cree_le, cree_par, origine)
         VALUES (?1,?2,?3,'ajustement',?4,?5,?6,?7,?8,?9,?10,'app')",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(),
            article_id, depot_id, delta,
            // Le motif etait saisi par le patron puis jete. La colonne
            // existe depuis l'origine ; un ecart sans raison n'est pas
            // exploitable a l'inventaire suivant.
            motif,
            op_id, auteur, now, now, auteur
        ],
    ).map_err(|e| e.to_string())?;

    Ok(())
}

// =====================================================================
//  Fiche fournisseur — détail
// =====================================================================

pub fn lire_fournisseur_detail(
    conn: &rusqlite::Connection,
    fournisseur_id: String,
) -> Result<serde_json::Value, String> {
    let f = conn.query_row(
        "SELECT id, nom, telephone, adresse, nif, email, est_voisin, cree_le
         FROM fournisseur WHERE id = ?1",
        rusqlite::params![fournisseur_id],
        |r| Ok(serde_json::json!({
            "id":        r.get::<_,String>(0)?,
            "nom":       r.get::<_,String>(1)?,
            "telephone": r.get::<_,Option<String>>(2)?,
            "adresse":   r.get::<_,Option<String>>(3)?,
            "nif":       r.get::<_,Option<String>>(4)?,
            "email":     r.get::<_,Option<String>>(5)?,
            "est_voisin":r.get::<_,i64>(6)? != 0,
            "cree_le":   r.get::<_,String>(7)?,
        }))
    ).map_err(|e| e.to_string())?;
    Ok(f)
}

// =====================================================================
//  Fiche fournisseur — stats, paiements, achats
// =====================================================================

pub fn lire_fiche_fournisseur(
    conn: &rusqlite::Connection,
    fournisseur_id: String,
) -> Result<serde_json::Value, String> {

    let (_qte, nb_achats, derniere_cmd): (f64, i64, Option<String>) =
        conn.query_row(
            // 'entree' et 'reception' incluses : marchandise recue de
             // ce fournisseur, facturee ou non. Le montant du, lui,
             // vient des pieces.
             "SELECT COALESCE(SUM(quantite_delta), 0), COUNT(*), MAX(date_mouvement)
             FROM mouvement_stock
             WHERE type_mouvement IN ('achat','entree','reception') AND quantite_delta > 0
               AND fournisseur_id = ?1",
            rusqlite::params![fournisseur_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        ).unwrap_or((0.0, 0, None));

    let total_achats: i64 = conn.query_row(
        "SELECT CAST(COALESCE(SUM(
             CASE pc.type_piece
               WHEN 'facture_fournisseur' THEN lp.montant_ht + lp.montant_tva
               WHEN 'avoir_fournisseur'   THEN
                    CASE WHEN pc.statut = 'paye' THEN 0
                         ELSE -(lp.montant_ht + lp.montant_tva) END
               ELSE 0 END), 0) AS INTEGER)
         FROM piece_commerciale pc
         JOIN ligne_piece lp ON lp.piece_id = pc.id
         WHERE pc.tiers_type = 'fournisseur'
           AND pc.tiers_id = ?1
           AND pc.type_piece IN ('facture_fournisseur','avoir_fournisseur')
           AND pc.statut <> 'annule'",
        rusqlite::params![fournisseur_id], |r| r.get(0),
    ).unwrap_or(0);

    let total_paye: i64 = conn.query_row(
        "SELECT CAST(COALESCE(SUM(montant), 0) AS INTEGER)
         FROM paiement_fournisseur WHERE fournisseur_id = ?1",
        rusqlite::params![fournisseur_id], |r| r.get(0),
    ).unwrap_or(0);

    let dette = (total_achats - total_paye).max(0);

    // Meme jeu de colonnes que l'onglet Reglements du client
    // (creances.rs::lire_reglements_client) : sans le numero de facture
    // et le reste apres versement, l'ecran fournisseur ne pouvait ni
    // filtrer ni s'imprimer comme son symetrique.
    let mut stmt_p = conn.prepare(
        "SELECT pf.id, pf.montant, pf.mode, pf.note, pf.date_paiement, u.nom,
                COALESCE(pc.numero, ''),
                pf.annule_paiement_id IS NOT NULL,
                EXISTS(SELECT 1 FROM paiement_fournisseur x
                        WHERE x.annule_paiement_id = pf.id),
                CAST(COALESCE((SELECT SUM(lp.montant_ht + lp.montant_tva)
                               FROM ligne_piece lp
                               WHERE lp.piece_id = pf.piece_id), 0) AS INTEGER),
                CAST(COALESCE((SELECT SUM(y.montant)
                               FROM paiement_fournisseur y
                               WHERE y.piece_id = pf.piece_id
                                 AND y.date_paiement <= pf.date_paiement), 0) AS INTEGER)
         FROM paiement_fournisseur pf
         LEFT JOIN utilisateur u ON u.id = pf.auteur_id
         LEFT JOIN piece_commerciale pc ON pc.id = pf.piece_id
         WHERE pf.fournisseur_id = ?1 ORDER BY pf.date_paiement DESC"
    ).map_err(|e| e.to_string())?;

    let paiements: Vec<serde_json::Value> = stmt_p.query_map(
        rusqlite::params![fournisseur_id], |r| {
            let total_faf: i64 = r.get(9)?;
            let verse_cumule: i64 = r.get(10)?;
            Ok(serde_json::json!({
                "id":            r.get::<_,String>(0)?,
                "montant":       r.get::<_,i64>(1)?,
                "mode":          r.get::<_,String>(2)?,
                "note":          r.get::<_,Option<String>>(3)?,
                "date_paiement": r.get::<_,String>(4)?,
                "auteur_nom":    r.get::<_,Option<String>>(5)?,
                "numero_facture": r.get::<_,String>(6)?,
                // Une contre-passation : montant negatif, barree a l'ecran.
                "est_annulation": r.get::<_,i64>(7)? != 0,
                // Deja annule : le bouton « contester » disparait.
                "annule":         r.get::<_,i64>(8)? != 0,
                // Dette restante sur CETTE facture juste apres CE versement.
                // Via `reste_exigible` (invariant 12) pour qu'un residu
                // d'arrondi ne s'affiche pas comme une dette.
                "reste_apres": crate::coeur::calcul::reste_exigible(
                    total_faf, verse_cumule),
            }))
        }
    ).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    // La FAF d'origine, quand elle existe : `operation_id` porte l'id de
    // la piece depuis que enregistrer_achat l'y ecrit (achats.rs). Vide
    // pour une 'entree' sans facture, et pour les achats anterieurs a ce
    // changement — l'ecran n'affiche l'apercu que si le numero est la.
    let mut stmt_a = conn.prepare(
        "SELECT ms.id, a.nom, ms.quantite_delta,
                COALESCE(ms.prix_achat_unitaire, a.dernier_prix_achat, 0),
                ms.date_mouvement,
                COALESCE(pc.id, ''), COALESCE(pc.numero, '')
         FROM mouvement_stock ms
         JOIN article a ON a.id = ms.article_id
         LEFT JOIN piece_commerciale pc ON ms.type_mouvement = 'achat'
           AND pc.id = ms.operation_id
         WHERE ms.type_mouvement IN ('achat','entree','reception') AND ms.quantite_delta > 0
           AND ms.fournisseur_id = ?1
         ORDER BY ms.date_mouvement DESC LIMIT 50"
    ).map_err(|e| e.to_string())?;

    let achats: Vec<serde_json::Value> = stmt_a.query_map(
        rusqlite::params![fournisseur_id], |r| {
        Ok(serde_json::json!({
            "id":            r.get::<_,String>(0)?,
            "article_nom":   r.get::<_,String>(1)?,
            "quantite":      r.get::<_,f64>(2)?,
            "prix_achat":    r.get::<_,i64>(3)?,
            "date_mouvement":r.get::<_,String>(4)?,
            "piece_id":      r.get::<_,String>(5)?,
            "piece_numero":  r.get::<_,String>(6)?,
        }))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    Ok(serde_json::json!({
        "stats": {
            "total_achats":      total_achats,
            "nb_achats":         nb_achats,
            "dette":             dette,
            "total_paye":        total_paye,
            "derniere_commande": derniere_cmd,
        },
        "paiements": paiements,
        "achats":    achats,
    }))
}
// =====================================================================
//  ANNULER UN PAIEMENT FOURNISSEUR
// =====================================================================
//
//  Symetrique de `creances::annuler_reglement`, et pour la meme raison :
//  on se trompe aussi en payant un fournisseur — mauvais montant, mauvais
//  fournisseur, versement saisi deux fois. Sans cette commande, la seule
//  issue etait de saisir un paiement negatif a la main, qui ne laisse
//  aucune trace de la correction.
//
//  Le sens de caisse est INVERSE du cote client : annuler un versement au
//  fournisseur fait RENTRER l'argent dans le tiroir. La decision, elle,
//  est la meme et vient du meme endroit (coeur::calcul).
// =====================================================================

pub fn annuler_paiement_fournisseur(
    conn: &rusqlite::Connection,
    paiement_id: String,
    motif: String,
    // true  : le fournisseur nous a REND l'argent aujourd'hui
    // false : erreur de saisie, l'argent n'a jamais bouge
    remboursement: bool,
    utilisateur_role: Option<String>,
) -> Result<serde_json::Value, String> {
    if motif.trim().is_empty() {
        return Err("Le motif est obligatoire : c'est lui qui explique \
                    la correction au fournisseur et au controle.".to_string());
    }
    let role = utilisateur_role.as_deref().unwrap_or("patron");
    let auteur_id = crate::argent::id_utilisateur_par_role(&conn, role);
    let maintenant = crate::utils::maintenant_iso();

    let (montant, mode, fournisseur_id, piece_id, date_paiement, est_annulation):
        (i64, String, String, Option<String>, String, bool) = conn.query_row(
        "SELECT montant, mode, fournisseur_id, piece_id, date_paiement,
                annule_paiement_id IS NOT NULL
         FROM paiement_fournisseur WHERE id = ?1",
        rusqlite::params![paiement_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?,
                r.get::<_, i64>(5)? != 0)),
    ).map_err(|_| "Paiement introuvable".to_string())?;

    if est_annulation {
        return Err("Cette ligne est déjà une annulation — on n'annule \
                    pas une annulation.".to_string());
    }

    let deja: i64 = conn.query_row(
        "SELECT COUNT(*) FROM paiement_fournisseur WHERE annule_paiement_id = ?1",
        rusqlite::params![paiement_id], |r| r.get(0),
    ).unwrap_or(0);
    if deja > 0 {
        return Err("Ce paiement a déjà été annulé.".to_string());
    }

    let dans_session_ouverte: bool = conn.query_row(
        "SELECT COUNT(*) FROM session_caisse
         WHERE statut = 'ouverte' AND ?1 >= cree_le",
        rusqlite::params![date_paiement], |r| r.get::<_, i64>(0),
    ).map(|n| n > 0).unwrap_or(false);

    // `touche_la_caisse` : un virement ou un cheque ne passe pas par le
    // tiroir, exactement comme l'avoir cote client (D29).
    let touche_la_caisse = mode != "virement" && mode != "cheque";
    let effet = crate::coeur::calcul::effet_caisse_annulation(
        remboursement, dans_session_ouverte, touche_la_caisse);
    let contre_passer = effet == crate::coeur::calcul::EffetCaisse::ContrePassation;

    let session_id = if contre_passer {
        Some(crate::utils::exiger_session_caisse(&conn)?)
    } else {
        None
    };

    // ---- La contre-passation ----
    // Montant negatif sur la MEME facture : le calcul de dette somme les
    // paiements (D36), il n'y a donc rien d'autre a defaire.
    conn.execute(
        "INSERT INTO paiement_fournisseur
         (id, fournisseur_id, piece_id, montant, mode, note,
          auteur_id, date_paiement, cree_le, origine, annule_paiement_id)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,'annulation',?10)",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(), fournisseur_id, piece_id,
            -montant, mode, format!("Annulation — {}", motif.trim()),
            auteur_id, maintenant, maintenant, paiement_id
        ],
    ).map_err(|e| e.to_string())?;

    // ---- La FAF soldee redevient due ----
    // Sans ca elle resterait 'paye' avec un reste au tableau : la fiche
    // et l'ecran Pieces se contrediraient.
    if let Some(ref pid) = piece_id {
        let (total, verse): (i64, i64) = conn.query_row(
            "SELECT CAST(COALESCE((SELECT SUM(lp.montant_ht + lp.montant_tva)
                                   FROM ligne_piece lp WHERE lp.piece_id = ?1), 0) AS INTEGER),
                    CAST(COALESCE((SELECT SUM(montant) FROM paiement_fournisseur
                                   WHERE piece_id = ?1), 0) AS INTEGER)",
            rusqlite::params![pid], |r| Ok((r.get(0)?, r.get(1)?)),
        ).unwrap_or((0, 0));

        if crate::coeur::calcul::reste_exigible(total, verse) > 0 {
            conn.execute(
                "UPDATE piece_commerciale SET statut = 'emis', modifie_le = ?1
                 WHERE id = ?2 AND statut = 'paye'",
                rusqlite::params![maintenant, pid],
            ).ok();
        }
    }

    // ---- Caisse : ENTREE, l'argent revient ----
    if let Some(sid) = session_id {
        conn.execute(
            "INSERT INTO mouvement_caisse
             (id, session_id, sens, moyen, montant, motif, libelle,
              operation_id, date_mouvement, cree_le, cree_par, origine)
             VALUES (?1,?2,'entree',?3,?4,'remboursement',?5,?6,?7,?8,?9,'annulation')",
            rusqlite::params![
                uuid::Uuid::new_v4().to_string(), sid, mode, montant,
                format!("Annulation paiement fournisseur — {}", motif.trim()),
                paiement_id, maintenant, maintenant, auteur_id
            ],
        ).map_err(|e| e.to_string())?;
    }

    conn.execute(
        "INSERT INTO journal
         (id, type_evenement, entite_type, entite_id, auteur_id,
          ancien_valeur, nouveau_valeur, origine, date_evenement)
         VALUES (?1,'paiement_fournisseur_annule','paiement_fournisseur',?2,?3,?4,?5,'app',?6)",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(), paiement_id, auteur_id,
            format!(r#"{{"montant":{}}}"#, montant),
            format!(
                r#"{{"motif":"{}","remboursement":{},"caisse":"{}"}}"#,
                motif.trim().replace('"', "'"), remboursement,
                if contre_passer { "entree" } else { "aucune" }),
            maintenant
        ],
    ).ok();

    Ok(serde_json::json!({
        "montant_annule":    montant,
        "entree_de_caisse":  contre_passer,
    }))
}

// =====================================================================
//  SUR L'UN OU L'AUTRE MOTEUR
// =====================================================================
//
// `fournisseur`, `piece_commerciale`, `ligne_piece`,
// `paiement_fournisseur`, `mouvement_stock`, `stock_depot`, `journal`
// sont cloisonnes. La dette se lit dans `paiement_fournisseur`, jamais
// dans un statut (D36), par facture puis additionnee (D41).

use crate::base::{Acces, Base};
use crate::parametres;

/// Le total du (FAF) moins les avoirs non rembourses (D9), tel que
/// deux ecrans le lisent.
const DETTE_PIECES: &str = "
    CAST(COALESCE(
      (SELECT COALESCE(SUM(
         CASE pc.type_piece
           WHEN 'facture_fournisseur' THEN lp.montant_ht + lp.montant_tva
           WHEN 'avoir_fournisseur'   THEN
                CASE WHEN pc.statut = 'paye' THEN 0
                     ELSE -(lp.montant_ht + lp.montant_tva) END
           ELSE 0 END), 0)
       FROM piece_commerciale pc
       JOIN ligne_piece lp ON lp.piece_id = pc.id
       WHERE pc.tiers_type = 'fournisseur'
         AND pc.tiers_id = f.id
         AND pc.type_piece IN ('facture_fournisseur','avoir_fournisseur')
         AND pc.statut <> 'annule'), 0) AS BIGINT)";

pub fn lire_fournisseurs_sur_base(base: &mut Base) -> Result<Vec<serde_json::Value>, String> {
    let dossier = base.dossier().to_string();
    base.lire_plusieurs(
        "SELECT id, nom, telephone, adresse, est_voisin FROM fournisseur WHERE dossier_id = ?1 ORDER BY nom",
        &parametres![dossier],
        |r| {
            Ok(serde_json::json!({
                "id":         r.get::<String>(0)?,
                "nom":        r.get::<String>(1)?,
                "telephone":  r.get::<Option<String>>(2)?,
                "adresse":    r.get::<Option<String>>(3)?,
                "est_voisin": r.get::<i64>(4)? != 0,
            }))
        },
    )
    .map_err(|e| e.0)
}

pub fn lire_fournisseurs_avec_dettes_sur_base(base: &mut Base) -> Result<Vec<serde_json::Value>, String> {
    let dossier = base.dossier().to_string();
    base.lire_plusieurs(
        &format!(
            "SELECT f.id, f.nom, f.telephone, {DETTE_PIECES},
                    CAST(COALESCE((SELECT SUM(pf.montant) FROM paiement_fournisseur pf
                                   WHERE pf.fournisseur_id = f.id), 0) AS BIGINT)
             FROM fournisseur f
             WHERE f.dossier_id = ?1
             ORDER BY f.nom"
        ),
        &parametres![dossier],
        |r| {
            let total_achats: i64 = r.get::<i64>(3)?;
            let total_paye: i64 = r.get::<i64>(4)?;
            Ok(serde_json::json!({
                "id":           r.get::<String>(0)?,
                "nom":          r.get::<String>(1)?,
                "telephone":    r.get::<Option<String>>(2)?,
                "total_achats": total_achats,
                "total_paye":   total_paye,
                "dette":        (total_achats - total_paye).max(0),
            }))
        },
    )
    .map_err(|e| e.0)
}

pub fn creer_fournisseur_sur_base(
    base: &mut Base,
    nom: String,
    telephone: Option<String>,
    adresse: Option<String>,
    nif: Option<String>,
    email: Option<String>,
    est_voisin: Option<bool>,
) -> Result<serde_json::Value, String> {
    let dossier = base.dossier().to_string();
    let id = uuid::Uuid::new_v4().to_string();
    let now = maintenant_iso();
    base.executer(
        "INSERT INTO fournisseur
         (id, nom, telephone, adresse, nif, email, est_voisin, cree_le, modifie_le, dossier_id)
         VALUES (?1,?2,CAST(?3 AS TEXT),CAST(?4 AS TEXT),CAST(?5 AS TEXT),CAST(?6 AS TEXT),?7,?8,?8,?9)",
        &parametres![id.clone(), nom.clone(), telephone, adresse, nif, email, est_voisin.unwrap_or(false) as i64, now, dossier],
    )
    .map_err(|e| e.0)?;
    Ok(serde_json::json!({"id": id, "nom": nom}))
}

#[allow(clippy::too_many_arguments)]
pub fn modifier_fournisseur_sur_base(
    base: &mut Base,
    fournisseur_id: String,
    nom: String,
    telephone: Option<String>,
    adresse: Option<String>,
    nif: Option<String>,
    email: Option<String>,
    est_voisin: Option<bool>,
) -> Result<(), String> {
    if nom.trim().is_empty() {
        return Err("Le nom est obligatoire".to_string());
    }
    let dossier = base.dossier().to_string();
    let vide = |o: Option<String>| o.filter(|s| !s.trim().is_empty());
    let modifiees = base
        .executer(
            "UPDATE fournisseur
             SET nom = ?1, telephone = CAST(?2 AS TEXT), adresse = CAST(?3 AS TEXT),
                 nif = CAST(?4 AS TEXT), email = CAST(?5 AS TEXT),
                 est_voisin = ?6, modifie_le = ?7
             WHERE id = ?8 AND dossier_id = ?9",
            &parametres![
                nom.trim(), vide(telephone), vide(adresse), vide(nif), vide(email),
                est_voisin.unwrap_or(false) as i64, maintenant_iso(), fournisseur_id, dossier
            ],
        )
        .map_err(|e| e.0)?;
    if modifiees == 0 {
        return Err("Fournisseur introuvable".to_string());
    }
    Ok(())
}

/// Ce qu'une piece fournisseur doit encore : une FAF son reste, un AVF
/// non rembourse son montant en negatif, un AVF rembourse rien.
fn du_de(type_piece: &str, statut: &str, montant: i64, paye: i64) -> i64 {
    if type_piece == "avoir_fournisseur" {
        if statut == "paye" { 0 } else { -montant }
    } else {
        crate::coeur::calcul::reste_exigible(montant, paye)
    }
}

pub fn lire_etat_dette_fournisseur_sur_base(base: &mut Base, fournisseur_id: String) -> Result<serde_json::Value, String> {
    let dossier = base.dossier().to_string();
    let tiers = base
        .lire_une(
            "SELECT nom, telephone, adresse FROM fournisseur WHERE id = ?1 AND dossier_id = ?2",
            &parametres![fournisseur_id.clone(), dossier.clone()],
            |r| {
                Ok(serde_json::json!({
                    "nom":       r.get::<String>(0)?,
                    "code":      "",
                    "telephone": r.get::<Option<String>>(1)?,
                    "adresse":   r.get::<Option<String>>(2)?,
                }))
            },
        )
        .map_err(|e| e.0)?
        .ok_or_else(|| "Fournisseur introuvable".to_string())?;

    let lignes: Vec<serde_json::Value> = base
        .lire_plusieurs(
            "SELECT pc.date_piece, pc.numero, pc.type_piece, pc.statut,
                    CAST(COALESCE(SUM(lp.montant_ht + lp.montant_tva), 0) AS BIGINT),
                    CAST(COALESCE((SELECT SUM(pf.montant) FROM paiement_fournisseur pf
                                   WHERE pf.piece_id = pc.id), 0) AS BIGINT)
             FROM piece_commerciale pc
             JOIN ligne_piece lp ON lp.piece_id = pc.id
             WHERE pc.tiers_type = 'fournisseur' AND pc.tiers_id = ?1
               AND pc.type_piece IN ('facture_fournisseur','avoir_fournisseur')
               AND pc.statut <> 'annule'
               AND pc.dossier_id = ?2
             GROUP BY pc.id, pc.date_piece, pc.numero, pc.type_piece, pc.statut
             ORDER BY pc.date_piece",
            &parametres![fournisseur_id, dossier],
            |r| {
                let type_piece: String = r.get::<String>(2)?;
                let statut: String = r.get::<String>(3)?;
                let montant: i64 = r.get::<i64>(4)?;
                let paye: i64 = r.get::<i64>(5)?;
                let avoir = type_piece == "avoir_fournisseur";
                Ok(serde_json::json!({
                    "date":   r.get::<String>(0)?,
                    "numero": r.get::<String>(1)?,
                    "type":   if avoir { "Avoir" } else { "Facture" },
                    "total":  montant,
                    "paye":   paye,
                    "reste":  du_de(&type_piece, &statut, montant, paye),
                }))
            },
        )
        .map_err(|e| e.0)?
        .into_iter()
        // Les pieces soldees ne figurent pas : le releve dit ce qui reste.
        .filter(|l| l["reste"].as_i64().unwrap_or(0) != 0)
        .collect();
    let total_du: i64 = lignes.iter().filter_map(|l| l["reste"].as_i64()).sum();
    Ok(serde_json::json!({
        "tiers":    tiers,
        "lignes":   lignes,
        "total_du": total_du.max(0),
        "avoirs":   0,
        "net_du":   total_du.max(0),
        "societe":  crate::creances::societe_sur(base),
    }))
}

pub fn lire_etat_dettes_global_sur_base(base: &mut Base) -> Result<serde_json::Value, String> {
    let dossier = base.dossier().to_string();
    let pieces: Vec<(String, String, Option<String>, i64)> = base
        .lire_plusieurs(
            "SELECT f.id, f.nom, f.telephone, pc.type_piece, pc.statut,
                    CAST(COALESCE(SUM(lp.montant_ht + lp.montant_tva), 0) AS BIGINT),
                    CAST(COALESCE((SELECT SUM(pf.montant) FROM paiement_fournisseur pf
                                   WHERE pf.piece_id = pc.id), 0) AS BIGINT)
             FROM piece_commerciale pc
             JOIN fournisseur f ON f.id = pc.tiers_id
             JOIN ligne_piece lp ON lp.piece_id = pc.id
             WHERE pc.tiers_type = 'fournisseur'
               AND pc.type_piece IN ('facture_fournisseur','avoir_fournisseur')
               AND pc.statut <> 'annule'
               AND pc.dossier_id = ?1
             GROUP BY pc.id, f.id, f.nom, f.telephone, pc.type_piece, pc.statut, pc.date_piece
             ORDER BY f.nom, pc.date_piece",
            &parametres![dossier],
            |r| {
                let type_piece: String = r.get::<String>(3)?;
                let statut: String = r.get::<String>(4)?;
                let montant: i64 = r.get::<i64>(5)?;
                let paye: i64 = r.get::<i64>(6)?;
                Ok((r.get::<String>(0)?, r.get::<String>(1)?, r.get::<Option<String>>(2)?, du_de(&type_piece, &statut, montant, paye)))
            },
        )
        .map_err(|e| e.0)?;

    let mut par_f: Vec<(String, String, Option<String>, i64, i64)> = Vec::new();
    for (id, nom, tel, du) in pieces {
        if du == 0 {
            continue;
        }
        match par_f.iter_mut().find(|l| l.0 == id) {
            Some(l) => {
                if du > 0 {
                    l.3 += 1;
                }
                l.4 += du;
            }
            None => par_f.push((id, nom, tel, if du > 0 { 1 } else { 0 }, du)),
        }
    }
    par_f.retain(|l| l.4 > 0);
    par_f.sort_by(|a, b| b.4.cmp(&a.4));
    let total_general: i64 = par_f.iter().map(|l| l.4).sum();
    let lignes: Vec<serde_json::Value> = par_f
        .into_iter()
        .map(|(_, nom, tel, nb, du)| serde_json::json!({ "nom": nom, "code": "", "telephone": tel, "nb": nb, "total_du": du }))
        .collect();
    Ok(serde_json::json!({
        "lignes":        lignes,
        "total_general": total_general,
        "societe":       crate::creances::societe_sur(base),
    }))
}

fn depot_ou_defaut(acces: &mut impl Acces, depot_id: Option<String>) -> Result<String, String> {
    match depot_id {
        Some(d) if !d.is_empty() => Ok(d),
        _ => {
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
    }
}

/// Une entree de marchandise SANS facture (D42) : ni dette, ni caisse.
pub fn enregistrer_entree_stock_sur_base(
    base: &mut Base,
    article_id: String,
    depot_id: Option<String>,
    quantite: f64,
    prix_achat: Option<i64>,
    fournisseur_id: Option<String>,
    utilisateur_role: Option<String>,
) -> Result<(), String> {
    let dossier = base.dossier().to_string();
    let now = maintenant_iso();
    let role = utilisateur_role.as_deref().unwrap_or("employe");
    let auteur = crate::argent::id_utilisateur_par_role_sur(base, role);
    let depot_id = depot_ou_defaut(base, depot_id)?;
    base.executer(
        "INSERT INTO mouvement_stock
         (id, article_id, depot_id, type_mouvement, quantite_delta,
          operation_id, auteur_id, date_mouvement, cree_le, cree_par, origine,
          fournisseur_id, prix_achat_unitaire, dossier_id)
         VALUES (?1,?2,?3,'entree',?4,?5,?6,?7,?7,?6,'app',CAST(?8 AS TEXT),CAST(?9 AS BIGINT),?10)",
        &parametres![
            uuid::Uuid::new_v4().to_string(), article_id.clone(), depot_id, quantite,
            uuid::Uuid::new_v4().to_string(), auteur, now, fournisseur_id, prix_achat, dossier
        ],
    )
    .map_err(|e| e.0)?;
    if let Some(px) = prix_achat {
        let _ = base.executer(
            "UPDATE article SET dernier_prix_achat = ?1 WHERE id = ?2",
            &parametres![px, article_id],
        );
    }
    Ok(())
}

/// Le miroir de l'entree sans facture : une simple sortie de stock,
/// bornee par le stock present. Pas de piece, pas de dette.
pub fn enregistrer_retour_sans_facture_sur_base(
    base: &mut Base,
    article_id: String,
    depot_id: Option<String>,
    quantite: f64,
    fournisseur_id: Option<String>,
    motif: Option<String>,
    utilisateur_role: Option<String>,
) -> Result<(), String> {
    if quantite <= 0.0 {
        return Err("La quantité à rendre doit être positive.".to_string());
    }
    let dossier = base.dossier().to_string();
    let now = maintenant_iso();
    let role = utilisateur_role.as_deref().unwrap_or("employe");
    let auteur = crate::argent::id_utilisateur_par_role_sur(base, role);
    let depot_id = depot_ou_defaut(base, depot_id)?;

    let dispo: f64 = base
        .lire_une(
            "SELECT COALESCE(quantite, 0) FROM stock_depot
             WHERE article_id = ?1 AND depot_id = ?2 AND dossier_id = ?3",
            &parametres![article_id.clone(), depot_id.clone(), dossier.clone()],
            |r| r.get::<f64>(0),
        )
        .ok()
        .flatten()
        .unwrap_or(0.0);
    if quantite > dispo {
        let nom: String = base
            .lire_une("SELECT nom FROM article WHERE id = ?1", &parametres![article_id.clone()], |r| r.get::<String>(0))
            .ok()
            .flatten()
            .unwrap_or_else(|| "cet article".to_string());
        return Err(format!(
            "Stock insuffisant pour « {nom} » : {dispo} disponible(s), {quantite} demandé(s)."
        ));
    }

    let mut tx = base.transaction().map_err(|e| e.0)?;
    tx.executer(
        "INSERT INTO mouvement_stock
         (id, article_id, depot_id, type_mouvement, quantite_delta,
          operation_id, auteur_id, date_mouvement, cree_le, cree_par, origine,
          fournisseur_id, dossier_id)
         VALUES (?1,?2,?3,'retour_fournisseur',?4,?5,?6,?7,?7,?6,'app',CAST(?8 AS TEXT),?9)",
        &parametres![
            uuid::Uuid::new_v4().to_string(), article_id.clone(), depot_id.clone(), -quantite,
            uuid::Uuid::new_v4().to_string(), auteur.clone(), now.clone(), fournisseur_id.clone(), dossier.clone()
        ],
    )
    .map_err(|e| e.0)?;
    // Le journal est la seule trace : le motif y est conserve.
    let _ = tx.executer(
        "INSERT INTO journal
         (id, type_evenement, entite_type, entite_id, auteur_id,
          nouveau_valeur, origine, date_evenement, dossier_id)
         VALUES (?1,'retour_sans_facture','article',?2,?3,?4,'app',?5,?6)",
        &parametres![
            uuid::Uuid::new_v4().to_string(), article_id, auteur,
            serde_json::json!({
                "quantite": quantite, "depot_id": depot_id,
                "fournisseur_id": fournisseur_id, "motif": motif,
            })
            .to_string(),
            now, dossier
        ],
    );
    tx.valider().map_err(|e| e.0)
}

pub fn enregistrer_ajustement_inventaire_sur_base(
    base: &mut Base,
    article_id: String,
    depot_id: String,
    quantite_reelle: f64,
    motif: Option<String>,
    utilisateur_role: Option<String>,
) -> Result<(), String> {
    let dossier = base.dossier().to_string();
    let quantite_actuelle: f64 = base
        .lire_une(
            "SELECT COALESCE(quantite, 0) FROM stock_depot
             WHERE article_id = ?1 AND depot_id = ?2 AND dossier_id = ?3",
            &parametres![article_id.clone(), depot_id.clone(), dossier.clone()],
            |r| r.get::<f64>(0),
        )
        .ok()
        .flatten()
        .unwrap_or(0.0);
    let delta = quantite_reelle - quantite_actuelle;
    let now = maintenant_iso();
    let role = utilisateur_role.as_deref().unwrap_or("employe");
    let auteur = crate::argent::id_utilisateur_par_role_sur(base, role);
    base.executer(
        "INSERT INTO mouvement_stock
         (id, article_id, depot_id, type_mouvement, quantite_delta,
          motif, operation_id, auteur_id, date_mouvement, cree_le, cree_par, origine, dossier_id)
         VALUES (?1,?2,?3,'ajustement',?4,CAST(?5 AS TEXT),?6,?7,?8,?8,?7,'app',?9)",
        &parametres![
            uuid::Uuid::new_v4().to_string(), article_id, depot_id, delta, motif,
            uuid::Uuid::new_v4().to_string(), auteur, now, dossier
        ],
    )
    .map_err(|e| e.0)?;
    Ok(())
}

pub fn lire_fournisseur_detail_sur_base(base: &mut Base, fournisseur_id: String) -> Result<serde_json::Value, String> {
    let dossier = base.dossier().to_string();
    base.lire_une(
        "SELECT id, nom, telephone, adresse, nif, email, est_voisin, cree_le
         FROM fournisseur WHERE id = ?1 AND dossier_id = ?2",
        &parametres![fournisseur_id, dossier],
        |r| {
            Ok(serde_json::json!({
                "id":         r.get::<String>(0)?,
                "nom":        r.get::<String>(1)?,
                "telephone":  r.get::<Option<String>>(2)?,
                "adresse":    r.get::<Option<String>>(3)?,
                "nif":        r.get::<Option<String>>(4)?,
                "email":      r.get::<Option<String>>(5)?,
                "est_voisin": r.get::<i64>(6)? != 0,
                "cree_le":    r.get::<String>(7)?,
            }))
        },
    )
    .map_err(|e| e.0)?
    .ok_or_else(|| "Fournisseur introuvable".to_string())
}

pub fn lire_fiche_fournisseur_sur_base(base: &mut Base, fournisseur_id: String) -> Result<serde_json::Value, String> {
    let dossier = base.dossier().to_string();
    let (nb_achats, derniere_cmd): (i64, Option<String>) = base
        .lire_une(
            "SELECT COUNT(*), MAX(date_mouvement)
             FROM mouvement_stock
             WHERE type_mouvement IN ('achat','entree','reception') AND quantite_delta > 0
               AND fournisseur_id = ?1 AND dossier_id = ?2",
            &parametres![fournisseur_id.clone(), dossier.clone()],
            |r| Ok((r.get::<i64>(0)?, r.get::<Option<String>>(1)?)),
        )
        .map_err(|e| e.0)?
        .unwrap_or((0, None));

    let (total_achats, total_paye): (i64, i64) = base
        .lire_une(
            &format!(
                "SELECT {DETTE_PIECES},
                        CAST(COALESCE((SELECT SUM(montant) FROM paiement_fournisseur
                                       WHERE fournisseur_id = f.id), 0) AS BIGINT)
                 FROM fournisseur f WHERE f.id = ?1 AND f.dossier_id = ?2"
            ),
            &parametres![fournisseur_id.clone(), dossier.clone()],
            |r| Ok((r.get::<i64>(0)?, r.get::<i64>(1)?)),
        )
        .map_err(|e| e.0)?
        .unwrap_or((0, 0));
    let dette = (total_achats - total_paye).max(0);

    let paiements: Vec<serde_json::Value> = base
        .lire_plusieurs(
            "SELECT pf.id, pf.montant, pf.mode, pf.note, pf.date_paiement, u.nom,
                    COALESCE(pc.numero, ''),
                    pf.annule_paiement_id IS NOT NULL,
                    EXISTS(SELECT 1 FROM paiement_fournisseur x WHERE x.annule_paiement_id = pf.id),
                    CAST(COALESCE((SELECT SUM(lp.montant_ht + lp.montant_tva)
                                   FROM ligne_piece lp WHERE lp.piece_id = pf.piece_id), 0) AS BIGINT),
                    CAST(COALESCE((SELECT SUM(y.montant) FROM paiement_fournisseur y
                                   WHERE y.piece_id = pf.piece_id
                                     AND y.date_paiement <= pf.date_paiement), 0) AS BIGINT)
             FROM paiement_fournisseur pf
             LEFT JOIN utilisateur u ON u.id = pf.auteur_id
             LEFT JOIN piece_commerciale pc ON pc.id = pf.piece_id
             WHERE pf.fournisseur_id = ?1 AND pf.dossier_id = ?2
             ORDER BY pf.date_paiement DESC",
            &parametres![fournisseur_id.clone(), dossier.clone()],
            |r| {
                let total_faf: i64 = r.get::<i64>(9)?;
                let verse_cumule: i64 = r.get::<i64>(10)?;
                Ok(serde_json::json!({
                    "id":             r.get::<String>(0)?,
                    "montant":        r.get::<i64>(1)?,
                    "mode":           r.get::<String>(2)?,
                    "note":           r.get::<Option<String>>(3)?,
                    "date_paiement":  r.get::<String>(4)?,
                    "auteur_nom":     r.get::<Option<String>>(5)?,
                    "numero_facture": r.get::<String>(6)?,
                    "est_annulation": r.get::<bool>(7)?,
                    "annule":         r.get::<bool>(8)?,
                    "reste_apres":    crate::coeur::calcul::reste_exigible(total_faf, verse_cumule),
                }))
            },
        )
        .map_err(|e| e.0)?;

    let achats: Vec<serde_json::Value> = base
        .lire_plusieurs(
            "SELECT ms.id, a.nom, ms.quantite_delta,
                    COALESCE(ms.prix_achat_unitaire, a.dernier_prix_achat, 0),
                    ms.date_mouvement, COALESCE(pc.id, ''), COALESCE(pc.numero, '')
             FROM mouvement_stock ms
             JOIN article a ON a.id = ms.article_id
             LEFT JOIN piece_commerciale pc ON ms.type_mouvement = 'achat' AND pc.id = ms.operation_id
             WHERE ms.type_mouvement IN ('achat','entree','reception') AND ms.quantite_delta > 0
               AND ms.fournisseur_id = ?1 AND ms.dossier_id = ?2
             ORDER BY ms.date_mouvement DESC LIMIT 50",
            &parametres![fournisseur_id, dossier],
            |r| {
                Ok(serde_json::json!({
                    "id":             r.get::<String>(0)?,
                    "article_nom":    r.get::<String>(1)?,
                    "quantite":       r.get::<f64>(2)?,
                    "prix_achat":     r.get::<i64>(3)?,
                    "date_mouvement": r.get::<String>(4)?,
                    "piece_id":       r.get::<String>(5)?,
                    "piece_numero":   r.get::<String>(6)?,
                }))
            },
        )
        .map_err(|e| e.0)?;

    Ok(serde_json::json!({
        "stats": {
            "total_achats":      total_achats,
            "nb_achats":         nb_achats,
            "dette":             dette,
            "total_paye":        total_paye,
            "derniere_commande": derniere_cmd,
        },
        "paiements": paiements,
        "achats":    achats,
    }))
}

/// Symetrique de `creances::annuler_reglement` : contre-passation, et
/// l'argent RENTRE dans le tiroir si le fournisseur rembourse.
pub fn annuler_paiement_fournisseur_sur_base(
    base: &mut Base,
    paiement_id: String,
    motif: String,
    remboursement: bool,
    utilisateur_role: Option<String>,
) -> Result<serde_json::Value, String> {
    if motif.trim().is_empty() {
        return Err("Le motif est obligatoire : c'est lui qui explique \
                    la correction au fournisseur et au controle."
            .to_string());
    }
    let dossier = base.dossier().to_string();
    let role = utilisateur_role.as_deref().unwrap_or("patron");
    let auteur_id = crate::argent::id_utilisateur_par_role_sur(base, role);
    let maintenant = maintenant_iso();

    let (montant, mode, fournisseur_id, piece_id, date_paiement, est_annulation): (i64, String, String, Option<String>, String, bool) = base
        .lire_une(
            "SELECT montant, mode, fournisseur_id, piece_id, date_paiement, annule_paiement_id IS NOT NULL
             FROM paiement_fournisseur WHERE id = ?1 AND dossier_id = ?2",
            &parametres![paiement_id.clone(), dossier.clone()],
            |r| Ok((r.get::<i64>(0)?, r.get::<String>(1)?, r.get::<String>(2)?, r.get::<Option<String>>(3)?, r.get::<String>(4)?, r.get::<bool>(5)?)),
        )
        .map_err(|e| e.0)?
        .ok_or_else(|| "Paiement introuvable".to_string())?;
    if est_annulation {
        return Err("Cette ligne est déjà une annulation — on n'annule \
                    pas une annulation."
            .to_string());
    }
    let deja: i64 = base
        .lire_une(
            "SELECT COUNT(*) FROM paiement_fournisseur WHERE annule_paiement_id = ?1 AND dossier_id = ?2",
            &parametres![paiement_id.clone(), dossier.clone()],
            |r| r.get::<i64>(0),
        )
        .map_err(|e| e.0)?
        .unwrap_or(0);
    if deja > 0 {
        return Err("Ce paiement a déjà été annulé.".to_string());
    }
    let dans_session_ouverte: bool = base
        .lire_une(
            "SELECT COUNT(*) FROM session_caisse
             WHERE statut = 'ouverte' AND CAST(?1 AS TEXT) >= cree_le AND dossier_id = ?2",
            &parametres![date_paiement, dossier.clone()],
            |r| r.get::<i64>(0),
        )
        .map_err(|e| e.0)?
        .map(|n| n > 0)
        .unwrap_or(false);
    // Un virement ou un cheque ne passe pas par le tiroir (D29).
    let touche_la_caisse = mode != "virement" && mode != "cheque";
    let effet = crate::coeur::calcul::effet_caisse_annulation(remboursement, dans_session_ouverte, touche_la_caisse);
    let contre_passer = effet == crate::coeur::calcul::EffetCaisse::ContrePassation;
    let session_id = if contre_passer { Some(crate::caisses::exiger_sur(base, None)?) } else { None };

    let mut tx = base.transaction().map_err(|e| e.0)?;
    tx.executer(
        "INSERT INTO paiement_fournisseur
         (id, fournisseur_id, piece_id, montant, mode, note,
          auteur_id, date_paiement, cree_le, origine, annule_paiement_id, dossier_id)
         VALUES (?1,?2,CAST(?3 AS TEXT),?4,?5,?6,?7,?8,?8,'annulation',?9,?10)",
        &parametres![
            uuid::Uuid::new_v4().to_string(), fournisseur_id, piece_id.clone(), -montant, mode.clone(),
            format!("Annulation — {}", motif.trim()), auteur_id.clone(), maintenant.clone(),
            paiement_id.clone(), dossier.clone()
        ],
    )
    .map_err(|e| e.0)?;

    // La FAF soldee redevient due.
    if let Some(ref pid) = piece_id {
        let (total, verse): (i64, i64) = tx
            .lire_une(
                "SELECT CAST(COALESCE((SELECT SUM(lp.montant_ht + lp.montant_tva)
                                       FROM ligne_piece lp WHERE lp.piece_id = ?1), 0) AS BIGINT),
                        CAST(COALESCE((SELECT SUM(montant) FROM paiement_fournisseur
                                       WHERE piece_id = ?1), 0) AS BIGINT)
                 FROM piece_commerciale WHERE id = ?1 AND dossier_id = ?2",
                &parametres![pid.clone(), dossier.clone()],
                |r| Ok((r.get::<i64>(0)?, r.get::<i64>(1)?)),
            )
            .map_err(|e| e.0)?
            .unwrap_or((0, 0));
        if crate::coeur::calcul::reste_exigible(total, verse) > 0 {
            let _ = tx.executer(
                "UPDATE piece_commerciale SET statut = 'emis', modifie_le = ?1
                 WHERE id = ?2 AND statut = 'paye' AND dossier_id = ?3",
                &parametres![maintenant.clone(), pid.clone(), dossier.clone()],
            );
        }
    }

    if let Some(sid) = session_id {
        tx.executer(
            "INSERT INTO mouvement_caisse
             (id, session_id, sens, moyen, montant, motif, libelle,
              operation_id, date_mouvement, cree_le, cree_par, origine, dossier_id)
             VALUES (?1,?2,'entree',?3,?4,'remboursement',?5,?6,?7,?7,?8,'annulation',?9)",
            &parametres![
                uuid::Uuid::new_v4().to_string(), sid, mode, montant,
                format!("Annulation paiement fournisseur — {}", motif.trim()),
                paiement_id.clone(), maintenant.clone(), auteur_id.clone(), dossier.clone()
            ],
        )
        .map_err(|e| e.0)?;
    }
    let _ = tx.executer(
        "INSERT INTO journal
         (id, type_evenement, entite_type, entite_id, auteur_id,
          ancien_valeur, nouveau_valeur, origine, date_evenement, dossier_id)
         VALUES (?1,'paiement_fournisseur_annule','paiement_fournisseur',?2,?3,?4,?5,'app',?6,?7)",
        &parametres![
            uuid::Uuid::new_v4().to_string(), paiement_id, auteur_id,
            format!(r#"{{"montant":{}}}"#, montant),
            format!(
                r#"{{"motif":"{}","remboursement":{},"caisse":"{}"}}"#,
                motif.trim().replace('"', "'"), remboursement,
                if contre_passer { "entree" } else { "aucune" }
            ),
            maintenant, dossier
        ],
    );
    tx.valider().map_err(|e| e.0)?;
    Ok(serde_json::json!({ "montant_annule": montant, "entree_de_caisse": contre_passer }))
}
