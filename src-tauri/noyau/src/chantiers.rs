//! Chantiers §14 — TVA, dettes fournisseur, irrécouvrable, expiration avoirs.

use crate::utils::maintenant_iso;

// =====================================================================
//  TVA
// =====================================================================

/// Lire le taux TVA de tous les articles.
pub fn lire_taux_tva(conn: &rusqlite::Connection) -> Result<Vec<serde_json::Value>, String> {
    let mut stmt = conn.prepare(
        "SELECT a.id, a.nom, a.unite_base,
                COALESCE(a.taux_tva_defaut, 0.0) as taux_tva
         FROM article a WHERE a.actif = 1 ORDER BY a.nom"
    ).map_err(|e| e.to_string())?;
    let x = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "id":         row.get::<_,String>(0)?,
            "nom":        row.get::<_,String>(1)?,
            "unite_base": row.get::<_,String>(2)?,
            "taux_tva":   row.get::<_,f64>(3)?,
        }))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
    Ok(x)
}

/// Sauvegarder le taux TVA d'un article.
pub fn sauvegarder_tva_article(
    conn: &rusqlite::Connection,
    article_id: String,
    taux_tva: f64,
) -> Result<(), String> {
    conn.execute(
        "UPDATE article SET taux_tva_defaut = ?1, modifie_le = ?2 WHERE id = ?3",
        rusqlite::params![taux_tva, maintenant_iso(), article_id],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

/// Résumé TVA collectée sur une période.
pub fn lire_resume_tva(
    conn: &rusqlite::Connection,
    date_debut: String,
    date_fin: String,
) -> Result<serde_json::Value, String> {

    let mut stmt = conn.prepare(
        "SELECT lv.taux_tva,
                CAST(SUM(lv.montant_tva) AS INTEGER) as total_tva,
                CAST(SUM(lv.prix_pratique * lv.quantite)
                     - SUM(lv.montant_tva) AS INTEGER) as total_ht
         FROM ligne_vente lv
         JOIN vente v ON v.id = lv.vente_id
         WHERE v.date_vente BETWEEN ?1 AND ?2
           AND v.statut != 'annulee'
           AND lv.taux_tva > 0
         GROUP BY lv.taux_tva
         ORDER BY lv.taux_tva"
    ).map_err(|e| e.to_string())?;

    let par_taux: Vec<serde_json::Value> = stmt.query_map(
        rusqlite::params![date_debut, date_fin],
        |row| Ok(serde_json::json!({
            "taux":      row.get::<_,f64>(0)?,
            "total_tva": row.get::<_,i64>(1)?,
            "total_ht":  row.get::<_,i64>(2)?,
        }))
    ).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    let total_tva: i64 = par_taux.iter()
        .filter_map(|v| v["total_tva"].as_i64())
        .sum();

    Ok(serde_json::json!({
        "par_taux":   par_taux,
        "total_tva":  total_tva,
        "date_debut": date_debut,
        "date_fin":   date_fin,
    }))
}

// =====================================================================
//  DETTES FOURNISSEURS
// =====================================================================

pub fn lire_dettes_fournisseurs(
    conn: &rusqlite::Connection,
) -> Result<Vec<serde_json::Value>, String> {

    let mut stmt = conn.prepare(
        "SELECT f.id, f.nom, f.telephone, f.est_voisin,
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
                     AND pc.statut <> 'annule'), 0
                ) AS INTEGER) as total_achats,
                CAST(COALESCE(
                  (SELECT SUM(pf.montant)
                   FROM paiement_fournisseur pf
                   WHERE pf.fournisseur_id = f.id), 0
                ) AS INTEGER) as total_paye
         FROM fournisseur f
         WHERE f.actif = 1
         ORDER BY f.nom"
    ).map_err(|e| e.to_string())?;

    let x = stmt.query_map([], |row| {
        let total_achats: i64 = row.get(4)?;
        let total_paye: i64 = row.get(5)?;
        let dette = (total_achats - total_paye).max(0);
        Ok(serde_json::json!({
            "id":           row.get::<_,String>(0)?,
            "nom":          row.get::<_,String>(1)?,
            "telephone":    row.get::<_,Option<String>>(2)?,
            "est_voisin":   row.get::<_,i64>(3)? != 0,
            "total_achats": total_achats,
            "total_paye":   total_paye,
            "dette":        dette,
        }))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
    Ok(x)
}

pub fn regler_dette_fournisseur(
    conn: &rusqlite::Connection,
    fournisseur_id: String,
    montant: i64,
    mode: String,
    note: Option<String>,
    piece_id: Option<String>,
) -> Result<serde_json::Value, String> {
    crate::argent::regler_dette_fournisseur(&conn, fournisseur_id, montant, mode, note, piece_id)
}

/// Repare les reglements globaux enregistres AVANT la repartition
/// ecrite : ceux dont `piece_id` est NULL alors qu'ils couvrent des
/// factures.
///
/// Chaque ligne non imputee est remplacee par une ou plusieurs lignes
/// portant leur `piece_id`, dont la somme vaut exactement l'ancienne.
/// Aucun franc n'est cree ni perdu — seule l'attribution change.
///
/// Idempotent : une fois reparties, les lignes ont un `piece_id` et ne
/// sont plus reprises. Un surplus reel (paiement superieur a la dette)
/// reste NULL et le restera, c'est une avance legitime.
///
/// Appelee par `entretenir_base`, qui copie la base avant d'agir.
pub fn reimputer_paiements_globaux(
    conn: &rusqlite::Connection,
) -> Result<i64, String> {
    let mut st = conn.prepare(
        "SELECT DISTINCT fournisseur_id FROM paiement_fournisseur
         WHERE piece_id IS NULL"
    ).map_err(|e| e.to_string())?;
    let fournisseurs: Vec<String> = st.query_map([], |r| r.get(0))
        .map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
    drop(st);

    let now = maintenant_iso();
    let mut reparties = 0_i64;

    for f_id in fournisseurs {
        // `imputer_paiements_fournisseur` realloue puis recalcule les
        // statuts : un seul chemin, celui qu'empruntent aussi les
        // reglements courants.
        reparties += reallouer_globaux(conn, &f_id, &now)?;
        imputer_paiements_fournisseur(conn, &f_id, &now)?;
    }

    Ok(reparties)
}

/// Ecrit l'affectation des paiements non imputes d'UN fournisseur.
///
/// Chaque ligne `piece_id IS NULL` est remplacee par une ou plusieurs
/// lignes portant leur `piece_id`, dont la somme vaut exactement
/// l'ancienne. Aucun franc n'est cree ni perdu — seule l'attribution
/// change.
///
/// Idempotent : une ligne imputee n'est plus reprise, et une avance
/// reelle (rien a couvrir) reste NULL sans etre reecrite a chaque appel.
// Deplace avec `imputer_paiements_fournisseur`, son appelant.
pub(crate) use crate::argent::reallouer_globaux;

/// Recalcule le statut des factures fournisseur d'apres les paiements.
///
/// Deux sources de reglement :
///   - paiements IMPUTES (piece_id renseigne) — affectes a leur facture
///   - paiements GLOBAUX (piece_id NULL) — repartis de la facture la
///     plus ancienne a la plus recente
///
/// Une facture passe a "paye" quand le cumul couvre son total, et
/// revient a "emis" sinon (annulation d'un paiement, avoir ajoute).
/// Retourne les numeros des factures desormais soldees.
// Le corps a demenage dans `noyau::argent` avec
// `regler_dette_fournisseur`, son unique appelant.
pub(crate) use crate::argent::imputer_paiements_fournisseur;

// =====================================================================
//  IRRÉCOUVRABLE
// =====================================================================

pub fn marquer_irrecouvrable(
    conn: &rusqlite::Connection,
    vente_id: String,
    motif: String,
) -> Result<(), String> {
    let auteur = crate::argent::id_utilisateur_courant_pub(&conn);
    let now = maintenant_iso();

    let statut: String = conn.query_row(
        "SELECT statut FROM vente WHERE id = ?1",
        rusqlite::params![vente_id],
        |r| r.get(0),
    ).map_err(|_| "Vente introuvable".to_string())?;

    if statut == "payee" {
        return Err("Cette vente est déjà entièrement payée".to_string());
    }
    if statut == "irrecouvrable" {
        return Err("Cette créance est déjà marquée irrécouvrable".to_string());
    }

    conn.execute(
        "UPDATE vente SET statut = 'irrecouvrable', modifie_le = ?1 WHERE id = ?2",
        rusqlite::params![now, vente_id],
    ).map_err(|e| e.to_string())?;

    conn.execute(
        "INSERT INTO creance_irrecouvrable
         (id, vente_id, motif, auteur_id, date_marque, cree_le, origine)
         VALUES (?1,?2,?3,?4,?5,?6,'app')",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(),
            vente_id, motif, auteur, now, now
        ],
    ).map_err(|e| e.to_string())?;

    conn.execute(
        "INSERT INTO journal
         (id, type_evenement, entite_type, entite_id, auteur_id,
          nouveau_valeur, origine, date_evenement)
         VALUES (?1,'creance_irrecouvrable','vente',?2,?3,?4,'app',?5)",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(),
            vente_id, auteur,
            format!(r#"{{"motif":"{}"}}"#, motif),
            now
        ],
    ).ok();
    Ok(())
}

pub fn lire_irrecouvrable(
    conn: &rusqlite::Connection,
) -> Result<Vec<serde_json::Value>, String> {

    let mut stmt = conn.prepare(
        "SELECT ci.id, ci.vente_id, ci.motif, ci.date_marque,
                c.nom as client_nom,
                p.numero as facture_num,
                CAST(COALESCE(
                  (SELECT SUM(prix_pratique * quantite)
                   FROM ligne_vente WHERE vente_id = ci.vente_id), 0
                ) AS INTEGER) as total,
                CAST(COALESCE(
                  (SELECT SUM(montant) FROM paiement WHERE vente_id = ci.vente_id), 0
                ) AS INTEGER) as total_paye
         FROM creance_irrecouvrable ci
         JOIN vente v ON v.id = ci.vente_id
         JOIN client c ON c.id = v.client_id
         LEFT JOIN piece_commerciale p ON p.id = v.piece_id
         ORDER BY ci.date_marque DESC"
    ).map_err(|e| e.to_string())?;

    let x = stmt.query_map([], |row| {
        let total: i64 = row.get(6)?;
        let paye: i64 = row.get(7)?;
        Ok(serde_json::json!({
            "id":            row.get::<_,String>(0)?,
            "vente_id":      row.get::<_,String>(1)?,
            "motif":         row.get::<_,String>(2)?,
            "date_marque":   row.get::<_,String>(3)?,
            "client_nom":    row.get::<_,String>(4)?,
            "facture_num":   row.get::<_,Option<String>>(5)?,
            "montant_perdu": total - paye,
        }))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
    Ok(x)
}

// =====================================================================
//  EXPIRATION AVOIRS
// =====================================================================

pub fn lire_config_avoirs(
    conn: &rusqlite::Connection,
) -> Result<serde_json::Value, String> {

    let active: Option<String> = conn.query_row(
        "SELECT valeur FROM config_app WHERE cle = 'avoirs_expiration_active'",
        [], |r| r.get(0),
    ).ok();

    let duree: Option<String> = conn.query_row(
        "SELECT valeur FROM config_app WHERE cle = 'avoirs_expiration_jours'",
        [], |r| r.get(0),
    ).ok();

    Ok(serde_json::json!({
        "active":      active.as_deref() == Some("1"),
        "duree_jours": duree.and_then(|d| d.parse::<i64>().ok()).unwrap_or(90),
    }))
}

pub fn sauvegarder_config_avoirs(
    conn: &rusqlite::Connection,
    active: bool,
    duree_jours: i64,
) -> Result<(), String> {

    if duree_jours < 30 {
        return Err("La durée minimale est de 30 jours".to_string());
    }

    conn.execute(
        "INSERT INTO config_app (cle, valeur) VALUES ('avoirs_expiration_active', ?1)
         ON CONFLICT(cle) DO UPDATE SET valeur = ?1",
        rusqlite::params![if active { "1" } else { "0" }],
    ).map_err(|e| e.to_string())?;

    conn.execute(
        "INSERT INTO config_app (cle, valeur) VALUES ('avoirs_expiration_jours', ?1)
         ON CONFLICT(cle) DO UPDATE SET valeur = ?1",
        rusqlite::params![duree_jours.to_string()],
    ).map_err(|e| e.to_string())?;

    Ok(())
}

pub fn expirer_avoirs(conn: &rusqlite::Connection) -> Result<i64, String> {

    let active: Option<String> = conn.query_row(
        "SELECT valeur FROM config_app WHERE cle = 'avoirs_expiration_active'",
        [], |r| r.get(0),
    ).ok();

    if active.as_deref() != Some("1") {
        return Ok(0);
    }

    let duree: i64 = conn.query_row(
        "SELECT CAST(valeur AS INTEGER) FROM config_app
         WHERE cle = 'avoirs_expiration_jours'",
        [], |r| r.get(0),
    ).unwrap_or(90);

    let nb = conn.execute(
        "UPDATE avoir SET statut = 'expire'
         WHERE statut = 'ouvert'
           AND (julianday('now') - julianday(cree_le)) > ?1",
        rusqlite::params![duree],
    ).map_err(|e| e.to_string())?;

    if nb > 0 {
        let auteur = crate::argent::id_utilisateur_courant_pub(&conn);
        conn.execute(
            "INSERT INTO journal
             (id, type_evenement, entite_type, entite_id, auteur_id,
              nouveau_valeur, origine, date_evenement)
             VALUES (?1,'expiration_avoirs','avoir','batch',?2,?3,'app',?4)",
            rusqlite::params![
                uuid::Uuid::new_v4().to_string(), auteur,
                format!(r#"{{"nb_expires":{},"duree_jours":{}}}"#, nb, duree),
                maintenant_iso()
            ],
        ).ok();
    }

    Ok(nb as i64)
}

/// Factures fournisseur non soldees, avec leur reste du.
///
/// Sert a l'ecran de reglement : on impute sur une facture precise au
/// lieu d'un montant global qui ne dit pas ce qu'il paie.
pub fn lire_factures_fournisseur_ouvertes(
    conn: &rusqlite::Connection,
    fournisseur_id: String,
) -> Result<Vec<serde_json::Value>, String> {

    let mut stmt = conn.prepare(
        "SELECT pc.id, pc.numero, pc.date_piece, pc.statut,
                CAST(COALESCE((SELECT SUM(lp.montant_ht + lp.montant_tva)
                   FROM ligne_piece lp WHERE lp.piece_id = pc.id), 0) AS INTEGER) as total,
                CAST(COALESCE((SELECT SUM(pf.montant)
                   FROM paiement_fournisseur pf WHERE pf.piece_id = pc.id), 0) AS INTEGER) as paye
         FROM piece_commerciale pc
         WHERE pc.tiers_type = 'fournisseur' AND pc.tiers_id = ?1
           AND pc.type_piece = 'facture_fournisseur'
           AND pc.statut NOT IN ('annule','paye')
         ORDER BY pc.date_piece ASC"
    ).map_err(|e| e.to_string())?;

    let x = stmt.query_map(rusqlite::params![fournisseur_id], |r| {
        let total: i64 = r.get(4)?;
        let paye: i64 = r.get(5)?;
        Ok(serde_json::json!({
            "piece_id":   r.get::<_,String>(0)?,
            "numero":     r.get::<_,String>(1)?,
            "date_piece": r.get::<_,String>(2)?,
            "statut":     r.get::<_,String>(3)?,
            "total":      total,
            "paye":       paye,
            "reste":      crate::coeur::calcul::reste_exigible(total, paye),
        }))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    Ok(x)
}
/// Rouvre un avoir expire.
///
/// L'expiration est un traitement de masse declenche par un reglage :
/// activer par erreur, lancer, puis desactiver laisse les avoirs
/// perdus, alors que le client a son papier en main. Sans ce chemin de
/// retour, la seule issue etait d'editer la base a la main.
///
/// Reserve au patron : c'est une remise en circulation de credit.
pub fn reactiver_avoir(
    conn: &rusqlite::Connection,
    avoir_id: String,
    utilisateur_role: Option<String>,
) -> Result<(), String> {
    if utilisateur_role.as_deref() != Some("patron") {
        return Err("Réservé au patron".to_string());
    }

    // Seul un avoir EXPIRE se rouvre. Un avoir consomme ou rembourse a
    // deja produit ses effets — le rouvrir creerait du credit a partir
    // de rien.
    let statut: String = conn.query_row(
        "SELECT statut FROM avoir WHERE id = ?1",
        rusqlite::params![avoir_id], |r| r.get(0),
    ).map_err(|_| "Avoir introuvable".to_string())?;

    if statut != "expire" {
        return Err(format!(
            "Cet avoir est « {} », pas expiré : il ne peut pas être rouvert.",
            statut
        ));
    }

    let maintenant = maintenant_iso();
    let auteur = crate::argent::id_utilisateur_courant_pub(&conn);

    conn.execute(
        "UPDATE avoir SET statut = 'ouvert' WHERE id = ?1",
        rusqlite::params![avoir_id],
    ).map_err(|e| e.to_string())?;

    conn.execute(
        "INSERT INTO journal
         (id, type_evenement, entite_type, entite_id, auteur_id,
          nouveau_valeur, origine, date_evenement)
         VALUES (?1,'avoir_reactive','avoir',?2,?3,'{}','app',?4)",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(), avoir_id, auteur, maintenant
        ],
    ).ok();

    Ok(())
}

/// Avoirs expires, pour pouvoir en rouvrir un.
pub fn lire_avoirs_expires(
    conn: &rusqlite::Connection,
) -> Result<Vec<serde_json::Value>, String> {
    let mut st = conn.prepare(
        "SELECT a.id, COALESCE(c.nom, '—'), a.montant, a.cree_le, a.piece_id
         FROM avoir a
         LEFT JOIN client c ON c.id = a.client_id
         WHERE a.statut = 'expire'
         ORDER BY a.cree_le DESC LIMIT 200"
    ).map_err(|e| e.to_string())?;

    let x = st.query_map([], |r| {
        Ok(serde_json::json!({
            "id":       r.get::<_, String>(0)?,
            "client":   r.get::<_, String>(1)?,
            "montant":  r.get::<_, i64>(2)?,
            "cree_le":  r.get::<_, String>(3)?,
            "piece_id": r.get::<_, Option<String>>(4)?,
        }))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    Ok(x)
}
// =====================================================================
//  SUR L'UN OU L'AUTRE MOTEUR
// =====================================================================
//
// `regler_dette_fournisseur` reste dans `argent`, avec l'imputation :
// il est porte avec le module `fournisseurs`. L'expiration des avoirs
// compare `cree_le` a une date calculee ici — `julianday` n'existe pas
// sur PostgreSQL.

use crate::base::Base;
use crate::parametres;

pub fn lire_taux_tva_sur_base(base: &mut Base) -> Result<Vec<serde_json::Value>, String> {
    base.lire_plusieurs(
        "SELECT a.id, a.nom, a.unite_base, COALESCE(a.taux_tva_defaut, 0.0)
         FROM article a WHERE a.actif = 1 ORDER BY a.nom",
        &[],
        |r| {
            Ok(serde_json::json!({
                "id":         r.get::<String>(0)?,
                "nom":        r.get::<String>(1)?,
                "unite_base": r.get::<String>(2)?,
                "taux_tva":   r.get::<f64>(3)?,
            }))
        },
    )
    .map_err(|e| e.0)
}

pub fn sauvegarder_tva_article_sur_base(base: &mut Base, article_id: String, taux_tva: f64) -> Result<(), String> {
    base.executer(
        "UPDATE article SET taux_tva_defaut = ?1, modifie_le = ?2 WHERE id = ?3",
        &parametres![taux_tva, maintenant_iso(), article_id],
    )
    .map_err(|e| e.0)?;
    Ok(())
}

pub fn lire_resume_tva_sur_base(base: &mut Base, date_debut: String, date_fin: String) -> Result<serde_json::Value, String> {
    let dossier = base.dossier().to_string();
    let par_taux: Vec<serde_json::Value> = base
        .lire_plusieurs(
            "SELECT lv.taux_tva,
                    CAST(SUM(lv.montant_tva) AS BIGINT),
                    CAST(SUM(lv.prix_pratique * lv.quantite) - SUM(lv.montant_tva) AS BIGINT)
             FROM ligne_vente lv
             JOIN vente v ON v.id = lv.vente_id
             WHERE v.date_vente BETWEEN ?1 AND ?2
               AND v.statut != 'annulee'
               AND lv.taux_tva > 0
               AND v.dossier_id = ?3
             GROUP BY lv.taux_tva
             ORDER BY lv.taux_tva",
            &parametres![date_debut.clone(), date_fin.clone(), dossier],
            |r| {
                Ok(serde_json::json!({
                    "taux":      r.get::<f64>(0)?,
                    "total_tva": r.get::<i64>(1)?,
                    "total_ht":  r.get::<i64>(2)?,
                }))
            },
        )
        .map_err(|e| e.0)?;
    let total_tva: i64 = par_taux.iter().filter_map(|v| v["total_tva"].as_i64()).sum();
    Ok(serde_json::json!({
        "par_taux":   par_taux,
        "total_tva":  total_tva,
        "date_debut": date_debut,
        "date_fin":   date_fin,
    }))
}

pub fn lire_dettes_fournisseurs_sur_base(base: &mut Base) -> Result<Vec<serde_json::Value>, String> {
    let dossier = base.dossier().to_string();
    base.lire_plusieurs(
        "SELECT f.id, f.nom, f.telephone, f.est_voisin,
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
                     AND pc.statut <> 'annule'), 0
                ) AS BIGINT),
                CAST(COALESCE(
                  (SELECT SUM(pf.montant) FROM paiement_fournisseur pf
                   WHERE pf.fournisseur_id = f.id), 0
                ) AS BIGINT)
         FROM fournisseur f
         WHERE f.actif = 1 AND f.dossier_id = ?1
         ORDER BY f.nom",
        &parametres![dossier],
        |r| {
            let total_achats: i64 = r.get::<i64>(4)?;
            let total_paye: i64 = r.get::<i64>(5)?;
            Ok(serde_json::json!({
                "id":           r.get::<String>(0)?,
                "nom":          r.get::<String>(1)?,
                "telephone":    r.get::<Option<String>>(2)?,
                "est_voisin":   r.get::<i64>(3)? != 0,
                "total_achats": total_achats,
                "total_paye":   total_paye,
                "dette":        (total_achats - total_paye).max(0),
            }))
        },
    )
    .map_err(|e| e.0)
}

pub fn marquer_irrecouvrable_sur_base(base: &mut Base, vente_id: String, motif: String) -> Result<(), String> {
    let dossier = base.dossier().to_string();
    let auteur = crate::argent::id_utilisateur_courant_sur(base);
    let now = maintenant_iso();
    let statut: String = base
        .lire_une(
            "SELECT statut FROM vente WHERE id = ?1 AND dossier_id = ?2",
            &parametres![vente_id.clone(), dossier.clone()],
            |r| r.get::<String>(0),
        )
        .map_err(|e| e.0)?
        .ok_or_else(|| "Vente introuvable".to_string())?;
    if statut == "payee" {
        return Err("Cette vente est déjà entièrement payée".to_string());
    }
    if statut == "irrecouvrable" {
        return Err("Cette créance est déjà marquée irrécouvrable".to_string());
    }

    let mut tx = base.transaction().map_err(|e| e.0)?;
    tx.executer(
        "UPDATE vente SET statut = 'irrecouvrable', modifie_le = ?1 WHERE id = ?2 AND dossier_id = ?3",
        &parametres![now.clone(), vente_id.clone(), dossier.clone()],
    )
    .map_err(|e| e.0)?;
    tx.executer(
        "INSERT INTO creance_irrecouvrable
         (id, vente_id, motif, auteur_id, date_marque, cree_le, origine, dossier_id)
         VALUES (?1,?2,?3,?4,?5,?5,'app',?6)",
        &parametres![uuid::Uuid::new_v4().to_string(), vente_id.clone(), motif.clone(), auteur.clone(), now.clone(), dossier.clone()],
    )
    .map_err(|e| e.0)?;
    let _ = tx.executer(
        "INSERT INTO journal
         (id, type_evenement, entite_type, entite_id, auteur_id,
          nouveau_valeur, origine, date_evenement, dossier_id)
         VALUES (?1,'creance_irrecouvrable','vente',?2,?3,?4,'app',?5,?6)",
        &parametres![uuid::Uuid::new_v4().to_string(), vente_id, auteur, format!(r#"{{"motif":"{}"}}"#, motif), now, dossier],
    );
    tx.valider().map_err(|e| e.0)
}

pub fn lire_irrecouvrable_sur_base(base: &mut Base) -> Result<Vec<serde_json::Value>, String> {
    let dossier = base.dossier().to_string();
    base.lire_plusieurs(
        "SELECT ci.id, ci.vente_id, ci.motif, ci.date_marque, c.nom, p.numero,
                CAST(COALESCE((SELECT SUM(prix_pratique * quantite)
                   FROM ligne_vente WHERE vente_id = ci.vente_id), 0) AS BIGINT),
                CAST(COALESCE((SELECT SUM(montant) FROM paiement WHERE vente_id = ci.vente_id), 0) AS BIGINT)
         FROM creance_irrecouvrable ci
         JOIN vente v ON v.id = ci.vente_id
         JOIN client c ON c.id = v.client_id
         LEFT JOIN piece_commerciale p ON p.id = v.piece_id
         WHERE ci.dossier_id = ?1
         ORDER BY ci.date_marque DESC",
        &parametres![dossier],
        |r| {
            let total: i64 = r.get::<i64>(6)?;
            let paye: i64 = r.get::<i64>(7)?;
            Ok(serde_json::json!({
                "id":            r.get::<String>(0)?,
                "vente_id":      r.get::<String>(1)?,
                "motif":         r.get::<String>(2)?,
                "date_marque":   r.get::<String>(3)?,
                "client_nom":    r.get::<String>(4)?,
                "facture_num":   r.get::<Option<String>>(5)?,
                "montant_perdu": total - paye,
            }))
        },
    )
    .map_err(|e| e.0)
}

fn config(base: &mut Base, cle: &str) -> Option<String> {
    base.lire_une("SELECT valeur FROM config_app WHERE cle = ?1", &parametres![cle], |r| r.get::<String>(0))
        .ok()
        .flatten()
}

pub fn lire_config_avoirs_sur_base(base: &mut Base) -> Result<serde_json::Value, String> {
    let active = config(base, "avoirs_expiration_active");
    let duree = config(base, "avoirs_expiration_jours");
    Ok(serde_json::json!({
        "active":      active.as_deref() == Some("1"),
        "duree_jours": duree.and_then(|d| d.parse::<i64>().ok()).unwrap_or(90),
    }))
}

pub fn sauvegarder_config_avoirs_sur_base(base: &mut Base, active: bool, duree_jours: i64) -> Result<(), String> {
    if duree_jours < 30 {
        return Err("La durée minimale est de 30 jours".to_string());
    }
    for (cle, valeur) in [
        ("avoirs_expiration_active", if active { "1".to_string() } else { "0".to_string() }),
        ("avoirs_expiration_jours", duree_jours.to_string()),
    ] {
        base.executer(
            "INSERT INTO config_app (cle, valeur) VALUES (?1, ?2)
             ON CONFLICT (cle) DO UPDATE SET valeur = ?2",
            &parametres![cle, valeur],
        )
        .map_err(|e| e.0)?;
    }
    Ok(())
}

pub fn expirer_avoirs_sur_base(base: &mut Base) -> Result<i64, String> {
    if config(base, "avoirs_expiration_active").as_deref() != Some("1") {
        return Ok(0);
    }
    let dossier = base.dossier().to_string();
    let duree: i64 = config(base, "avoirs_expiration_jours")
        .and_then(|d| d.trim().parse::<i64>().ok())
        .unwrap_or(90);
    // Un avoir cree AVANT cette date a plus de `duree` jours.
    let limite = (chrono::Local::now() - chrono::Duration::days(duree))
        .format("%Y-%m-%dT%H:%M:%S")
        .to_string();
    let nb = base
        .executer(
            "UPDATE avoir SET statut = 'expire'
             WHERE statut = 'ouvert' AND cree_le < ?1 AND dossier_id = ?2",
            &parametres![limite, dossier.clone()],
        )
        .map_err(|e| e.0)?;
    if nb > 0 {
        let auteur = crate::argent::id_utilisateur_courant_sur(base);
        let _ = base.executer(
            "INSERT INTO journal
             (id, type_evenement, entite_type, entite_id, auteur_id,
              nouveau_valeur, origine, date_evenement, dossier_id)
             VALUES (?1,'expiration_avoirs','avoir','batch',?2,?3,'app',?4,?5)",
            &parametres![
                uuid::Uuid::new_v4().to_string(), auteur,
                format!(r#"{{"nb_expires":{},"duree_jours":{}}}"#, nb, duree),
                maintenant_iso(), dossier
            ],
        );
    }
    Ok(nb as i64)
}

pub fn lire_factures_fournisseur_ouvertes_sur_base(base: &mut Base, fournisseur_id: String) -> Result<Vec<serde_json::Value>, String> {
    let dossier = base.dossier().to_string();
    base.lire_plusieurs(
        "SELECT pc.id, pc.numero, pc.date_piece, pc.statut,
                CAST(COALESCE((SELECT SUM(lp.montant_ht + lp.montant_tva)
                   FROM ligne_piece lp WHERE lp.piece_id = pc.id), 0) AS BIGINT),
                CAST(COALESCE((SELECT SUM(pf.montant)
                   FROM paiement_fournisseur pf WHERE pf.piece_id = pc.id), 0) AS BIGINT)
         FROM piece_commerciale pc
         WHERE pc.tiers_type = 'fournisseur' AND pc.tiers_id = ?1
           AND pc.type_piece = 'facture_fournisseur'
           AND pc.statut NOT IN ('annule','paye')
           AND pc.dossier_id = ?2
         ORDER BY pc.date_piece ASC",
        &parametres![fournisseur_id, dossier],
        |r| {
            let total: i64 = r.get::<i64>(4)?;
            let paye: i64 = r.get::<i64>(5)?;
            Ok(serde_json::json!({
                "piece_id":   r.get::<String>(0)?,
                "numero":     r.get::<String>(1)?,
                "date_piece": r.get::<String>(2)?,
                "statut":     r.get::<String>(3)?,
                "total":      total,
                "paye":       paye,
                "reste":      crate::coeur::calcul::reste_exigible(total, paye),
            }))
        },
    )
    .map_err(|e| e.0)
}

pub fn reactiver_avoir_sur_base(base: &mut Base, avoir_id: String, utilisateur_role: Option<String>) -> Result<(), String> {
    if utilisateur_role.as_deref() != Some("patron") {
        return Err("Réservé au patron".to_string());
    }
    let dossier = base.dossier().to_string();
    let statut: String = base
        .lire_une(
            "SELECT statut FROM avoir WHERE id = ?1 AND dossier_id = ?2",
            &parametres![avoir_id.clone(), dossier.clone()],
            |r| r.get::<String>(0),
        )
        .map_err(|e| e.0)?
        .ok_or_else(|| "Avoir introuvable".to_string())?;
    if statut != "expire" {
        return Err(format!("Cet avoir est « {} », pas expiré : il ne peut pas être rouvert.", statut));
    }
    let maintenant = maintenant_iso();
    let auteur = crate::argent::id_utilisateur_courant_sur(base);
    base.executer(
        "UPDATE avoir SET statut = 'ouvert' WHERE id = ?1 AND dossier_id = ?2",
        &parametres![avoir_id.clone(), dossier.clone()],
    )
    .map_err(|e| e.0)?;
    let _ = base.executer(
        "INSERT INTO journal
         (id, type_evenement, entite_type, entite_id, auteur_id,
          nouveau_valeur, origine, date_evenement, dossier_id)
         VALUES (?1,'avoir_reactive','avoir',?2,?3,'{}','app',?4,?5)",
        &parametres![uuid::Uuid::new_v4().to_string(), avoir_id, auteur, maintenant, dossier],
    );
    Ok(())
}

pub fn lire_avoirs_expires_sur_base(base: &mut Base) -> Result<Vec<serde_json::Value>, String> {
    let dossier = base.dossier().to_string();
    base.lire_plusieurs(
        "SELECT a.id, COALESCE(c.nom, '—'), a.montant, a.cree_le, a.piece_id
         FROM avoir a
         LEFT JOIN client c ON c.id = a.client_id
         WHERE a.statut = 'expire' AND a.dossier_id = ?1
         ORDER BY a.cree_le DESC LIMIT 200",
        &parametres![dossier],
        |r| {
            Ok(serde_json::json!({
                "id":       r.get::<String>(0)?,
                "client":   r.get::<String>(1)?,
                "montant":  r.get::<i64>(2)?,
                "cree_le":  r.get::<String>(3)?,
                "piece_id": r.get::<Option<String>>(4)?,
            }))
        },
    )
    .map_err(|e| e.0)
}
