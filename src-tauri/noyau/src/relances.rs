//! Relances créances — historique, envoi WhatsApp, marquage.

use crate::utils::maintenant_iso;

// =====================================================================
//  Lire créances avec détail pour relance
// =====================================================================

pub fn lire_creances_relances(
    conn: &rusqlite::Connection,
    en_retard_seulement: Option<bool>,
) -> Result<Vec<serde_json::Value>, String> {

    let filtre_retard = if en_retard_seulement == Some(true) {
        "AND (julianday('now') - julianday(v.date_vente)) > 30"
    } else { "" };

    let sql = format!(
        "SELECT
            v.id, v.date_vente, v.statut,
            c.id as client_id, c.nom as client_nom,
            c.code as client_code, c.telephone,
            p.numero as facture_num,
            CAST(COALESCE(
              (SELECT SUM(prix_pratique * quantite) FROM ligne_vente WHERE vente_id = v.id)
            , 0) AS INTEGER) as total,
            CAST(COALESCE(
              (SELECT SUM(montant) FROM paiement WHERE vente_id = v.id)
            , 0) AS INTEGER) as total_paye,
            CAST(julianday('now') - julianday(v.date_vente) AS INTEGER) as jours_retard,
            COALESCE(
              (SELECT COUNT(*) FROM relance_creance rc WHERE rc.vente_id = v.id)
            , 0) as nb_relances,
            COALESCE(
              (SELECT MAX(rc.date_relance) FROM relance_creance rc WHERE rc.vente_id = v.id)
            , NULL) as derniere_relance
         FROM vente v
         JOIN client c ON c.id = v.client_id
         LEFT JOIN piece_commerciale p ON p.id = v.piece_id
         WHERE v.statut IN ('creance_ouverte', 'partiellement_payee')
           AND c.est_generique = 0
           {}
         ORDER BY jours_retard DESC, total DESC",
        filtre_retard
    );

    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;

    let x = stmt.query_map([], |row| {
        let total: i64 = row.get(8)?;
        let paye: i64 = row.get(9)?;
        Ok(serde_json::json!({
            "vente_id":       row.get::<_,String>(0)?,
            "date_vente":     row.get::<_,String>(1)?,
            "statut":         row.get::<_,String>(2)?,
            "client_id":      row.get::<_,String>(3)?,
            "client_nom":     row.get::<_,String>(4)?,
            "client_code":    row.get::<_,String>(5)?,
            "telephone":      row.get::<_,Option<String>>(6)?,
            "facture_num":    row.get::<_,Option<String>>(7)?,
            "total":          total,
            "total_paye":     paye,
            "reste":          total - paye,
            "jours_retard":   row.get::<_,i64>(10)?,
            "nb_relances":    row.get::<_,i64>(11)?,
            "derniere_relance": row.get::<_,Option<String>>(12)?,
        }))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
    Ok(x)
}

// =====================================================================
//  Enregistrer une relance
// =====================================================================

pub fn enregistrer_relance(
    conn: &rusqlite::Connection,
    vente_id: String,
    canal: String,      // whatsapp | sms | appel | email | visite
    note: Option<String>,
) -> Result<(), String> {

    let auteur = crate::argent::id_utilisateur_courant_pub(&conn);
    let now = maintenant_iso();

    conn.execute(
        "INSERT INTO relance_creance
         (id, vente_id, canal, note, auteur_id, date_relance, cree_le, origine)
         VALUES (?1,?2,?3,?4,?5,?6,?7,'app')",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(),
            vente_id, canal, note, auteur, now, now
        ],
    ).map_err(|e| e.to_string())?;

    Ok(())
}

// =====================================================================
//  Lire historique relances d'une créance
// =====================================================================

pub fn lire_historique_relances(
    conn: &rusqlite::Connection,
    vente_id: String,
) -> Result<Vec<serde_json::Value>, String> {

    let mut stmt = conn.prepare(
        "SELECT rc.id, rc.canal, rc.note, rc.date_relance, u.nom as auteur_nom
         FROM relance_creance rc
         LEFT JOIN utilisateur u ON u.id = rc.auteur_id
         WHERE rc.vente_id = ?1
         ORDER BY rc.date_relance DESC"
    ).map_err(|e| e.to_string())?;

    let x = stmt.query_map(rusqlite::params![vente_id], |row| {
        Ok(serde_json::json!({
            "id":           row.get::<_,String>(0)?,
            "canal":        row.get::<_,String>(1)?,
            "note":         row.get::<_,Option<String>>(2)?,
            "date_relance": row.get::<_,String>(3)?,
            "auteur_nom":   row.get::<_,Option<String>>(4)?,
        }))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
    Ok(x)
}

// =====================================================================
//  Stats relances globales
// =====================================================================

pub fn lire_stats_relances(
    conn: &rusqlite::Connection,
) -> Result<serde_json::Value, String> {

    let total_creances: i64 = conn.query_row(
        "SELECT COUNT(*) FROM vente
         WHERE statut IN ('creance_ouverte','partiellement_payee')", [],
        |r| r.get(0)
    ).unwrap_or(0);

    let sans_relance: i64 = conn.query_row(
        "SELECT COUNT(*) FROM vente v
         WHERE v.statut IN ('creance_ouverte','partiellement_payee')
           AND NOT EXISTS (SELECT 1 FROM relance_creance rc WHERE rc.vente_id = v.id)", [],
        |r| r.get(0)
    ).unwrap_or(0);

    let relances_semaine: i64 = conn.query_row(
        "SELECT COUNT(*) FROM relance_creance
         WHERE date_relance >= date('now', '-7 days')", [],
        |r| r.get(0)
    ).unwrap_or(0);

    let montant_en_jeu: i64 = conn.query_row(
        "SELECT CAST(COALESCE(SUM(
            (SELECT COALESCE(SUM(prix_pratique * quantite), 0) FROM ligne_vente WHERE vente_id = v.id) -
            (SELECT COALESCE(SUM(montant), 0) FROM paiement WHERE vente_id = v.id)
         ), 0) AS INTEGER)
         FROM vente v
         WHERE v.statut IN ('creance_ouverte','partiellement_payee')", [],
        |r| r.get(0)
    ).unwrap_or(0);

    Ok(serde_json::json!({
        "total_creances":   total_creances,
        "sans_relance":     sans_relance,
        "relances_semaine": relances_semaine,
        "montant_en_jeu":   montant_en_jeu,
    }))
}

// =====================================================================
//  SUR L'UN OU L'AUTRE MOTEUR
// =====================================================================
//
// Le retard se calcule en Rust (`utils::jours_depuis`) et le filtre
// « en retard » s'applique apres lecture : `julianday` n'existe pas
// sur PostgreSQL, et une creance de plus de trente jours se compte
// aussi bien ici qu'en SQL.

use crate::base::Base;
use crate::parametres;

pub fn lire_creances_relances_sur_base(
    base: &mut Base,
    en_retard_seulement: Option<bool>,
) -> Result<Vec<serde_json::Value>, String> {
    let dossier = base.dossier().to_string();
    let mut lignes: Vec<serde_json::Value> = base
        .lire_plusieurs(
            "SELECT
                v.id, v.date_vente, v.statut,
                c.id, c.nom, c.code, c.telephone,
                p.numero,
                CAST(COALESCE(
                  (SELECT SUM(prix_pratique * quantite) FROM ligne_vente WHERE vente_id = v.id)
                , 0) AS BIGINT),
                CAST(COALESCE(
                  (SELECT SUM(montant) FROM paiement WHERE vente_id = v.id)
                , 0) AS BIGINT),
                (SELECT COUNT(*) FROM relance_creance rc WHERE rc.vente_id = v.id),
                (SELECT MAX(rc.date_relance) FROM relance_creance rc WHERE rc.vente_id = v.id)
             FROM vente v
             JOIN client c ON c.id = v.client_id
             LEFT JOIN piece_commerciale p ON p.id = v.piece_id
             WHERE v.statut IN ('creance_ouverte', 'partiellement_payee')
               AND c.est_generique = 0
               AND v.dossier_id = ?1
             ORDER BY v.date_vente ASC",
            &parametres![dossier],
            |r| {
                let total: i64 = r.get::<i64>(8)?;
                let paye: i64 = r.get::<i64>(9)?;
                let date_vente: String = r.get::<String>(1)?;
                Ok(serde_json::json!({
                    "vente_id":       r.get::<String>(0)?,
                    "jours_retard":   crate::utils::jours_depuis(&date_vente),
                    "date_vente":     date_vente,
                    "statut":         r.get::<String>(2)?,
                    "client_id":      r.get::<String>(3)?,
                    "client_nom":     r.get::<String>(4)?,
                    "client_code":    r.get::<String>(5)?,
                    "telephone":      r.get::<Option<String>>(6)?,
                    "facture_num":    r.get::<Option<String>>(7)?,
                    "total":          total,
                    "total_paye":     paye,
                    "reste":          total - paye,
                    "nb_relances":    r.get::<i64>(10)?,
                    "derniere_relance": r.get::<Option<String>>(11)?,
                }))
            },
        )
        .map_err(|e| e.0)?;

    if en_retard_seulement == Some(true) {
        lignes.retain(|l| l["jours_retard"].as_i64().unwrap_or(0) > 30);
    }
    // Meme ordre que la version SQLite : les plus en retard d'abord,
    // puis les plus gros montants.
    lignes.sort_by(|a, b| {
        b["jours_retard"].as_i64().cmp(&a["jours_retard"].as_i64())
            .then(b["total"].as_i64().cmp(&a["total"].as_i64()))
    });
    Ok(lignes)
}

pub fn enregistrer_relance_sur_base(
    base: &mut Base,
    vente_id: String,
    canal: String,
    note: Option<String>,
) -> Result<(), String> {
    let dossier = base.dossier().to_string();
    let auteur = crate::argent::id_utilisateur_courant_sur(base);
    let now = maintenant_iso();
    base.executer(
        "INSERT INTO relance_creance
         (id, vente_id, canal, note, auteur_id, date_relance, cree_le, origine, dossier_id)
         VALUES (?1,?2,?3,CAST(?4 AS TEXT),?5,?6,?6,'app',?7)",
        &parametres![uuid::Uuid::new_v4().to_string(), vente_id, canal, note, auteur, now, dossier],
    )
    .map_err(|e| e.0)?;
    Ok(())
}

pub fn lire_historique_relances_sur_base(
    base: &mut Base,
    vente_id: String,
) -> Result<Vec<serde_json::Value>, String> {
    let dossier = base.dossier().to_string();
    base.lire_plusieurs(
        "SELECT rc.id, rc.canal, rc.note, rc.date_relance, u.nom
         FROM relance_creance rc
         LEFT JOIN utilisateur u ON u.id = rc.auteur_id
         WHERE rc.vente_id = ?1 AND rc.dossier_id = ?2
         ORDER BY rc.date_relance DESC",
        &parametres![vente_id, dossier],
        |r| {
            Ok(serde_json::json!({
                "id":           r.get::<String>(0)?,
                "canal":        r.get::<String>(1)?,
                "note":         r.get::<Option<String>>(2)?,
                "date_relance": r.get::<String>(3)?,
                "auteur_nom":   r.get::<Option<String>>(4)?,
            }))
        },
    )
    .map_err(|e| e.0)
}

pub fn lire_stats_relances_sur_base(base: &mut Base) -> Result<serde_json::Value, String> {
    let dossier = base.dossier().to_string();
    let mut compte = |sql: &str, params: &[crate::base::Valeur]| -> Result<i64, String> {
        Ok(base.lire_une(sql, params, |r| r.get::<i64>(0)).map_err(|e| e.0)?.unwrap_or(0))
    };
    let total_creances = compte(
        "SELECT COUNT(*) FROM vente
         WHERE statut IN ('creance_ouverte','partiellement_payee') AND dossier_id = ?1",
        &parametres![dossier.clone()],
    )?;
    let sans_relance = compte(
        "SELECT COUNT(*) FROM vente v
         WHERE v.statut IN ('creance_ouverte','partiellement_payee') AND v.dossier_id = ?1
           AND NOT EXISTS (SELECT 1 FROM relance_creance rc WHERE rc.vente_id = v.id)",
        &parametres![dossier.clone()],
    )?;
    let il_y_a_sept_jours = (chrono::Local::now() - chrono::Duration::days(7))
        .format("%Y-%m-%d")
        .to_string();
    let relances_semaine = compte(
        "SELECT COUNT(*) FROM relance_creance
         WHERE SUBSTR(date_relance, 1, 10) >= ?1 AND dossier_id = ?2",
        &parametres![il_y_a_sept_jours, dossier.clone()],
    )?;
    let montant_en_jeu = compte(
        "SELECT CAST(COALESCE(SUM(
            (SELECT COALESCE(SUM(prix_pratique * quantite), 0) FROM ligne_vente WHERE vente_id = v.id) -
            (SELECT COALESCE(SUM(montant), 0) FROM paiement WHERE vente_id = v.id)
         ), 0) AS BIGINT)
         FROM vente v
         WHERE v.statut IN ('creance_ouverte','partiellement_payee') AND v.dossier_id = ?1",
        &parametres![dossier],
    )?;
    Ok(serde_json::json!({
        "total_creances":   total_creances,
        "sans_relance":     sans_relance,
        "relances_semaine": relances_semaine,
        "montant_en_jeu":   montant_en_jeu,
    }))
}
