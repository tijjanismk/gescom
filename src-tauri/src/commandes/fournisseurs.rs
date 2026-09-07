//! Fournisseurs — liste, création, stock, fiche détail.

use tauri::State;
use crate::commandes::ventes::EtatApp;
use crate::utils::maintenant_iso;

// =====================================================================
//  Liste fournisseurs
// =====================================================================

#[tauri::command]
pub fn lire_fournisseurs(
    etat: State<EtatApp>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
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

#[tauri::command]
pub fn lire_fournisseurs_avec_dettes(
    etat: State<EtatApp>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

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

#[tauri::command]
pub fn creer_fournisseur(
    etat: State<EtatApp>,
    nom: String,
    telephone: Option<String>,
    adresse: Option<String>,
    nif: Option<String>,
    email: Option<String>,
    est_voisin: Option<bool>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
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
#[tauri::command]
pub fn modifier_fournisseur(
    etat: State<EtatApp>,
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

    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

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
#[tauri::command]
pub fn lire_etat_dette_fournisseur(
    etat: State<EtatApp>,
    fournisseur_id: String,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

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
        "societe":  crate::commandes::creances::societe(&conn),
    }))
}

/// Etat GLOBAL des dettes — un fournisseur par ligne.
///
/// D9/D36 : la dette se lit dans `paiement_fournisseur`, jamais dans le
/// statut. Le reste se calcule PAR FACTURE puis s'additionne, pour la
/// meme raison que cote client (seuil de solde, D41).
#[tauri::command]
pub fn lire_etat_dettes_global(
    etat: State<EtatApp>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

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
        "societe":       crate::commandes::creances::societe(&conn),
    }))
}

// =====================================================================
//  Enregistrer entrée stock (achat)
// =====================================================================

#[tauri::command]
pub fn enregistrer_entree_stock(
    etat: State<EtatApp>,
    article_id: String,
    depot_id: Option<String>,  // null → dépôt par défaut
    quantite: f64,
    prix_achat: Option<i64>,
    fournisseur_id: Option<String>,
    utilisateur_role: Option<String>,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let now = maintenant_iso();
    let role = utilisateur_role.as_deref().unwrap_or("employe");
    let auteur = crate::commandes::ventes::id_utilisateur_par_role(&conn, role);

    // Résoudre le dépôt — utiliser le défaut si null
    let depot_id = match depot_id {
        Some(d) if !d.is_empty() => d,
        _ => conn.query_row(
            "SELECT id FROM depot WHERE est_defaut = 1 LIMIT 1",
            [], |r| r.get(0)
        ).map_err(|_| "Aucun dépôt par défaut configuré".to_string())?,
    };

    let op_id = uuid::Uuid::new_v4().to_string();

    // Mettre à jour le stock
    conn.execute(
        "INSERT INTO stock_depot (id, article_id, depot_id, quantite)
         VALUES (?1,?2,?3,?4)
         ON CONFLICT(article_id, depot_id)
         DO UPDATE SET quantite = quantite + ?4",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(), article_id, depot_id, quantite
        ],
    ).map_err(|e| e.to_string())?;

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
//  Ajustement inventaire
// =====================================================================

#[tauri::command]
pub fn enregistrer_ajustement_inventaire(
    etat: State<EtatApp>,
    article_id: String,
    depot_id: String,
    // `quantite_reelle` et non `nouvelle_quantite` : c'est ce qui a ete
    // COMPTE physiquement dans le depot. Le front envoyait deja ce nom,
    // d'ou l'erreur « invalid args ».
    quantite_reelle: f64,
    motif: Option<String>,
    utilisateur_role: Option<String>,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let quantite_actuelle: f64 = conn.query_row(
        "SELECT COALESCE(quantite, 0) FROM stock_depot
         WHERE article_id = ?1 AND depot_id = ?2",
        rusqlite::params![article_id, depot_id],
        |r| r.get(0),
    ).unwrap_or(0.0);

    let delta = quantite_reelle - quantite_actuelle;
    let now = maintenant_iso();
    let role = utilisateur_role.as_deref().unwrap_or("employe");
    let auteur = crate::commandes::ventes::id_utilisateur_par_role(&conn, role);
    let op_id = uuid::Uuid::new_v4().to_string();

    conn.execute(
        "INSERT INTO stock_depot (id, article_id, depot_id, quantite)
         VALUES (?1,?2,?3,?4)
         ON CONFLICT(article_id, depot_id)
         DO UPDATE SET quantite = ?4",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(), article_id, depot_id, quantite_reelle
        ],
    ).map_err(|e| e.to_string())?;

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

#[tauri::command]
pub fn lire_fournisseur_detail(
    etat: State<EtatApp>,
    fournisseur_id: String,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
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

#[tauri::command]
pub fn lire_fiche_fournisseur(
    etat: State<EtatApp>,
    fournisseur_id: String,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let (_qte, nb_achats, derniere_cmd): (f64, i64, Option<String>) =
        conn.query_row(
            // 'entree' incluse : marchandise recue de ce fournisseur,
             // facturee ou non. Le montant du, lui, vient des pieces.
             "SELECT COALESCE(SUM(quantite_delta), 0), COUNT(*), MAX(date_mouvement)
             FROM mouvement_stock
             WHERE type_mouvement IN ('achat','entree') AND quantite_delta > 0
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
         WHERE ms.type_mouvement IN ('achat','entree') AND ms.quantite_delta > 0
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

#[tauri::command]
pub fn annuler_paiement_fournisseur(
    etat: State<EtatApp>,
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

    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let role = utilisateur_role.as_deref().unwrap_or("patron");
    let auteur_id = crate::commandes::ventes::id_utilisateur_par_role(&conn, role);
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
