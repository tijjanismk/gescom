//! Le tableau de bord : ce que le patron regarde le matin.
//!
//! Cinq lectures, toutes en `SELECT`. Elles sont portees en bloc parce
//! que c'est l'ecran d'accueil : une caisse qui se connecte y arrive
//! avant tout le reste, et cinq erreurs `commande_inconnue` en guise de
//! bienvenue donneraient l'impression d'un logiciel casse.
//!
//! Code coupe de `commandes/dashboard.rs` et `commandes/depots.rs`,
//! non recrit.

use chrono::Datelike;

const JOURS_COURTS: [&str; 7] = ["Lun", "Mar", "Mer", "Jeu", "Ven", "Sam", "Dim"];
const MOIS_COURTS: [&str; 12] = [
    "Jan", "Fév", "Mar", "Avr", "Mai", "Juin",
    "Juil", "Août", "Sep", "Oct", "Nov", "Déc",
];

pub fn lire_resume_dashboard(
    conn: &rusqlite::Connection,
    // Depot actif. None ou vide = tous les depots (vue consolidee).
    depot_id: Option<String>,
) -> Result<serde_json::Value, String> {

    // Le depot vient d'une liste fermee cote Rust (lire_depots), jamais
    // d'une saisie libre. On le passe malgre tout en PARAMETRE lie et
    // non par interpolation : une clause construite par format! est la
    // porte ouverte a l'injection le jour ou la source change.
    //
    // Astuce : `(?N IS NULL OR colonne = ?N)` laisse passer tout quand
    // le parametre est NULL. Une seule requete couvre les deux cas.
    let dep: Option<String> = match depot_id {
        Some(d) if !d.is_empty() => Some(d),
        _ => None,
    };

    let debut_jour = chrono::Local::now().format("%Y-%m-%dT00:00:00").to_string();
    let debut_semaine = {
        let now = chrono::Local::now();
        let lundi = now - chrono::Duration::days(now.weekday().num_days_from_monday() as i64);
        lundi.format("%Y-%m-%dT00:00:00").to_string()
    };
    let debut_mois = chrono::Local::now().format("%Y-%m-01T00:00:00").to_string();
    let debut_mois_prec = {
        let now = chrono::Local::now();
        let mois = now.month();
        let annee = now.year();
        if mois == 1 {
            format!("{}-12-01T00:00:00", annee - 1)
        } else {
            format!("{}-{:02}-01T00:00:00", annee, mois - 1)
        }
    };
    let fin_mois_prec = chrono::Local::now().format("%Y-%m-01T00:00:00").to_string();

    // CA jour
    let ca_jour: i64 = conn.query_row(
        "SELECT CAST(COALESCE(SUM(
            (SELECT COALESCE(SUM(prix_pratique * quantite), 0)
             FROM ligne_vente WHERE vente_id = v.id)
         ), 0) AS INTEGER)
         FROM vente v
         WHERE v.date_vente >= ?1 AND v.statut != 'annulee'
           AND (?2 IS NULL OR v.depot_id = ?2)",
        rusqlite::params![debut_jour, dep],
        |r| r.get(0),
    ).unwrap_or(0);

    let nb_ventes_jour: i64 = conn.query_row(
        "SELECT COUNT(*) FROM vente WHERE date_vente >= ?1 AND statut != 'annulee'
           AND (?2 IS NULL OR depot_id = ?2)",
        rusqlite::params![debut_jour, dep], |r| r.get(0),
    ).unwrap_or(0);

    // CA semaine
    let ca_semaine: i64 = conn.query_row(
        "SELECT CAST(COALESCE(SUM(
            (SELECT COALESCE(SUM(prix_pratique * quantite), 0)
             FROM ligne_vente WHERE vente_id = v.id)
         ), 0) AS INTEGER)
         FROM vente v WHERE v.date_vente >= ?1 AND v.statut != 'annulee'
           AND (?2 IS NULL OR v.depot_id = ?2)",
        rusqlite::params![debut_semaine, dep], |r| r.get(0),
    ).unwrap_or(0);

    // CA mois
    let (ca_mois, nb_ventes_mois): (i64, i64) = conn.query_row(
        "SELECT
            CAST(COALESCE(SUM(
                (SELECT COALESCE(SUM(prix_pratique * quantite), 0)
                 FROM ligne_vente WHERE vente_id = v.id)
            ), 0) AS INTEGER),
            COUNT(*)
         FROM vente v WHERE v.date_vente >= ?1 AND v.statut != 'annulee'
           AND (?2 IS NULL OR v.depot_id = ?2)",
        rusqlite::params![debut_mois, dep],
        |r| Ok((r.get(0)?, r.get(1)?)),
    ).unwrap_or((0, 0));

    // CA mois précédent
    let ca_mois_precedent: i64 = conn.query_row(
        "SELECT CAST(COALESCE(SUM(
            (SELECT COALESCE(SUM(prix_pratique * quantite), 0)
             FROM ligne_vente WHERE vente_id = v.id)
         ), 0) AS INTEGER)
         FROM vente v
         WHERE v.date_vente >= ?1 AND v.date_vente < ?2
           AND v.statut != 'annulee'
           AND (?3 IS NULL OR v.depot_id = ?3)",
        rusqlite::params![debut_mois_prec, fin_mois_prec, dep],
        |r| r.get(0),
    ).unwrap_or(0);

    // Créances
    let (total_creances, nb_creances_ouvertes): (i64, i64) = conn.query_row(
        "SELECT
            CAST(COALESCE(SUM(
                (SELECT COALESCE(SUM(prix_pratique * quantite), 0)
                 FROM ligne_vente WHERE vente_id = v.id) -
                (SELECT COALESCE(SUM(montant), 0)
                 FROM paiement WHERE vente_id = v.id)
            ), 0) AS INTEGER),
            COUNT(DISTINCT v.id)
         FROM vente v
         WHERE v.statut IN ('creance_ouverte','partiellement_payee')
           AND (?1 IS NULL OR v.depot_id = ?1)",
        rusqlite::params![dep],
        |r| Ok((r.get(0)?, r.get(1)?)),
    ).unwrap_or((0, 0));

    // Créances en retard (pièces avec échéance dépassée)
    let nb_creances_en_retard: i64 = conn.query_row(
        "SELECT COUNT(*) FROM piece_commerciale
         WHERE type_piece = 'facture'
           AND statut IN ('brouillon','emis')
           AND date_echeance IS NOT NULL
           AND date_echeance < date('now')",
        [], |r| r.get(0),
    ).unwrap_or(0);

    // Avoirs ouverts
    let total_avoirs_ouverts: i64 = conn.query_row(
        "SELECT CAST(COALESCE(SUM(montant), 0) AS INTEGER)
         FROM avoir WHERE statut = 'ouvert'",
        [], |r| r.get(0),
    ).unwrap_or(0);

    // Stock
    // Compte des ARTICLES, pas des lignes de stock : creer un depot
    // insere une ligne a 0 pour chaque article, ce qui doublait le
    // compteur de ruptures le jour de l'ouverture d'un magasin.
    // En vue consolidee, un article present ailleurs n'est pas en
    // rupture — c'est le total qui tranche.
    let stock_ruptures: i64 = conn.query_row(
        "SELECT COUNT(*) FROM (
           SELECT sd.article_id
           FROM stock_depot sd
           JOIN article a ON a.id = sd.article_id
           JOIN depot d ON d.id = sd.depot_id AND d.actif = 1
           WHERE a.actif = 1 AND a.gere_en_stock = 1
             AND (?1 IS NULL OR sd.depot_id = ?1)
           GROUP BY sd.article_id
           HAVING SUM(sd.quantite) <= 0
         )",
        rusqlite::params![dep], |r| r.get(0),
    ).unwrap_or(0);

    let stock_alertes: i64 = conn.query_row(
        "SELECT COUNT(*) FROM (
           SELECT sd.article_id
           FROM stock_depot sd
           JOIN article a ON a.id = sd.article_id
           JOIN depot d ON d.id = sd.depot_id AND d.actif = 1
           WHERE a.actif = 1 AND a.gere_en_stock = 1
             AND (?1 IS NULL OR sd.depot_id = ?1)
           GROUP BY sd.article_id
           HAVING SUM(sd.quantite) > 0 AND SUM(sd.quantite) < 5
         )",
        rusqlite::params![dep], |r| r.get(0),
    ).unwrap_or(0);

    // Caisse
    let (caisse_solde, caisse_ouverte): (i64, bool) = conn.query_row(
        "SELECT
            COALESCE(fond_ouverture, 0) +
            -- `motif <> 'ouverture'` : le fond est deja dans
            -- `fond_ouverture` juste au-dessus, et il existe aussi
            -- comme mouvement pour la trace. L'oublier le comptait
            -- deux fois.
            COALESCE((SELECT SUM(CASE WHEN sens='entree' THEN montant ELSE -montant END)
                      FROM mouvement_caisse
                      WHERE session_id = sc.id AND motif <> 'ouverture'), 0),
            sc.statut = 'ouverte'
         FROM session_caisse sc
         WHERE sc.statut = 'ouverte'
         ORDER BY sc.cree_le DESC LIMIT 1",
        [], |r| Ok((r.get(0)?, r.get::<_,bool>(1)?)),
    ).unwrap_or((0, false));

    // Pièces en attente
    let factures_brouillon: i64 = conn.query_row(
        "SELECT COUNT(*) FROM piece_commerciale
         WHERE type_piece = 'facture' AND statut = 'brouillon'",
        [], |r| r.get(0),
    ).unwrap_or(0);

    let commandes_en_attente: i64 = conn.query_row(
        "SELECT COUNT(*) FROM piece_commerciale
         WHERE type_piece = 'commande_client'
           AND statut NOT IN ('transfere','annule')",
        [], |r| r.get(0),
    ).unwrap_or(0);

    Ok(serde_json::json!({
        "ca_jour":               ca_jour,
        "ca_semaine":            ca_semaine,
        "ca_mois":               ca_mois,
        "ca_mois_precedent":     ca_mois_precedent,
        "nb_ventes_jour":        nb_ventes_jour,
        "nb_ventes_mois":        nb_ventes_mois,
        "total_creances":        total_creances,
        "nb_creances_ouvertes":  nb_creances_ouvertes,
        "nb_creances_en_retard": nb_creances_en_retard,
        "total_avoirs_ouverts":  total_avoirs_ouverts,
        "stock_ruptures":        stock_ruptures,
        "stock_alertes":         stock_alertes,
        "caisse_solde":          caisse_solde,
        "caisse_session_ouverte":caisse_ouverte,
        "factures_brouillon":    factures_brouillon,
        "commandes_en_attente":  commandes_en_attente,
    }))
}

pub fn lire_ventes_periode(
    conn: &rusqlite::Connection,
    // jour | semaine | mois | annee. Defaut : jour.
    periode: Option<String>,
    // Depot actif. Sans ce filtre, les barres additionnaient TOUS les
    // depots alors que le total affiche a cote vient de
    // `lire_resume_dashboard`, lui filtre : sur deux depots, les barres
    // depassaient visiblement le total annonce.
    depot_id: Option<String>,
) -> Result<serde_json::Value, String> {
    let dep: Option<String> = depot_id.filter(|d| !d.is_empty());
    let p = periode.as_deref().unwrap_or("jour");
    let maintenant = chrono::Local::now();

    // `cle` est l'expression SQL qui range une vente dans sa case, et
    // `debut`/`fin` bornent la periode. Le reste du code est commun.
    let (debut, fin, cle, nb_cases): (String, String, &str, usize) = match p {
        "semaine" => {
            let d = maintenant - chrono::Duration::days(6);
            (
                d.format("%Y-%m-%dT00:00:00").to_string(),
                maintenant.format("%Y-%m-%dT23:59:59").to_string(),
                // Jours ecoules depuis le debut de la fenetre : 0..6.
                "CAST(julianday(DATE(v.date_vente)) - julianday(DATE(?1)) AS INTEGER)",
                7,
            )
        }
        "mois" => {
            let d = maintenant.with_day(1).unwrap_or(maintenant);
            (
                d.format("%Y-%m-01T00:00:00").to_string(),
                maintenant.format("%Y-%m-%dT23:59:59").to_string(),
                // Semaine dans le mois : (jour - 1) / 7, donc 0..4.
                "(CAST(strftime('%d', v.date_vente) AS INTEGER) - 1) / 7",
                5,
            )
        }
        "annee" => (
            maintenant.format("%Y-01-01T00:00:00").to_string(),
            maintenant.format("%Y-12-31T23:59:59").to_string(),
            "CAST(strftime('%m', v.date_vente) AS INTEGER) - 1",
            12,
        ),
        // jour
        _ => (
            maintenant.format("%Y-%m-%dT00:00:00").to_string(),
            maintenant.format("%Y-%m-%dT23:59:59").to_string(),
            "CAST(strftime('%H', v.date_vente) AS INTEGER)",
            24,
        ),
    };

    let mut cases: Vec<(i64, i64)> = vec![(0, 0); nb_cases];

    let sql = format!(
        "SELECT {cle} AS case_idx,
                CAST(COALESCE(SUM(
                    (SELECT COALESCE(SUM(prix_pratique * quantite), 0)
                     FROM ligne_vente WHERE vente_id = v.id)
                ), 0) AS INTEGER) AS total,
                COUNT(*) AS nb
         FROM vente v
         WHERE v.date_vente >= ?1 AND v.date_vente <= ?2
           AND v.statut != 'annulee'
           AND (?3 IS NULL OR v.depot_id = ?3)
         GROUP BY case_idx"
    );

    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    stmt.query_map(rusqlite::params![debut, fin, dep], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)?))
    }).map_err(|e| e.to_string())?
    .filter_map(|r| r.ok())
    .for_each(|(i, montant, nb)| {
        if i >= 0 && (i as usize) < nb_cases {
            cases[i as usize] = (montant, nb);
        }
    });

    // Libelles, et bornes d'affichage.
    let mut points: Vec<serde_json::Value> = Vec::new();
    let (de, a) = match p {
        // Une boutique n'ouvre pas a minuit : on part de 6h, sauf si une
        // vente a eu lieu avant — auquel cas la masquer serait mentir.
        "jour" | _ if p == "jour" => {
            let tot = if cases[..6].iter().any(|(m, n)| *m != 0 || *n != 0) { 0 } else { 6 };
            (tot, 23)
        }
        _ => (0, nb_cases - 1),
    };

    for i in de..=a {
        let (montant, nb) = cases[i];
        let label = match p {
            "semaine" => {
                let d = maintenant - chrono::Duration::days((6 - i) as i64);
                JOURS_COURTS[d.weekday().num_days_from_monday() as usize].to_string()
            }
            "mois" => format!("S{}", i + 1),
            "annee" => MOIS_COURTS[i].to_string(),
            _ => format!("{}h", i),
        };
        points.push(serde_json::json!({
            "label": label, "montant": montant, "nb": nb,
        }));
    }

    let total: i64 = points.iter().filter_map(|p| p["montant"].as_i64()).sum();
    let nb_total: i64 = points.iter().filter_map(|p| p["nb"].as_i64()).sum();

    Ok(serde_json::json!({
        "points":   points,
        "total":    total,
        "nb":       nb_total,
        "periode":  p,
    }))
}

pub fn lire_top_clients(
    conn: &rusqlite::Connection,
) -> Result<Vec<serde_json::Value>, String> {
    let debut_mois = chrono::Local::now().format("%Y-%m-01T00:00:00").to_string();

    let mut stmt = conn.prepare(
        "SELECT c.nom, c.code,
                CAST(COALESCE(SUM(
                    (SELECT COALESCE(SUM(prix_pratique * quantite), 0)
                     FROM ligne_vente WHERE vente_id = v.id)
                ), 0) AS INTEGER) as ca,
                COUNT(v.id) as nb_ventes
         FROM vente v
         JOIN client c ON c.id = v.client_id
         WHERE v.date_vente >= ?1 AND v.statut != 'annulee'
           AND c.est_generique = 0
         GROUP BY c.id
         ORDER BY ca DESC
         LIMIT 10"
    ).map_err(|e| e.to_string())?;

    let x = stmt.query_map(rusqlite::params![debut_mois], |row| {
        Ok(serde_json::json!({
            "nom":       row.get::<_,String>(0)?,
            "code":      row.get::<_,String>(1)?,
            "ca":        row.get::<_,i64>(2)?,
            "nb_ventes": row.get::<_,i64>(3)?,
        }))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
    Ok(x)
}

pub fn lire_top_articles(
    conn: &rusqlite::Connection,
) -> Result<Vec<serde_json::Value>, String> {
    let debut_mois = chrono::Local::now().format("%Y-%m-01T00:00:00").to_string();

    let mut stmt = conn.prepare(
        "SELECT a.nom, a.unite_base,
                CAST(COALESCE(SUM(lv.quantite * lv.prix_pratique), 0) AS INTEGER) as ca,
                COALESCE(SUM(lv.quantite), 0) as qte_vendue
         FROM ligne_vente lv
         JOIN article a ON a.id = lv.article_id
         JOIN vente v ON v.id = lv.vente_id
         WHERE v.date_vente >= ?1 AND v.statut != 'annulee'
         GROUP BY a.id
         ORDER BY ca DESC
         LIMIT 10"
    ).map_err(|e| e.to_string())?;

    let x = stmt.query_map(rusqlite::params![debut_mois], |row| {
        Ok(serde_json::json!({
            "nom":        row.get::<_,String>(0)?,
            "unite":      row.get::<_,String>(1)?,
            "ca":         row.get::<_,i64>(2)?,
            "qte_vendue": row.get::<_,f64>(3)?,
        }))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
    Ok(x)
}

pub fn lire_ventes_a_decouvert(
    conn: &rusqlite::Connection,
    date_debut: Option<String>,
    date_fin: Option<String>,
) -> Result<serde_json::Value, String> {
    let d1 = date_debut.filter(|s| !s.is_empty());
    let d2 = date_fin.filter(|s| !s.is_empty());

    let mut st = conn.prepare(
        "SELECT a.nom, a.unite_base, lv.quantite, v.date_vente,
                COALESCE(c.nom, '—'), COALESCE(d.nom, '—')
         FROM ligne_vente lv
         JOIN vente v ON v.id = lv.vente_id
         JOIN article a ON a.id = lv.article_id
         LEFT JOIN client c ON c.id = v.client_id
         LEFT JOIN depot d ON d.id = lv.depot_source_id
         WHERE lv.vente_a_decouvert = 1
           AND v.statut <> 'annulee'
           AND (?1 IS NULL OR DATE(v.date_vente) >= ?1)
           AND (?2 IS NULL OR DATE(v.date_vente) <= ?2)
         ORDER BY v.date_vente DESC
         LIMIT 200"
    ).map_err(|e| e.to_string())?;

    let lignes: Vec<serde_json::Value> = st.query_map(
        rusqlite::params![d1, d2], |r| {
            Ok(serde_json::json!({
                "article":    r.get::<_, String>(0)?,
                "unite_base": r.get::<_, String>(1)?,
                "quantite":   r.get::<_, f64>(2)?,
                "date":       r.get::<_, String>(3)?,
                "client":     r.get::<_, String>(4)?,
                "depot":      r.get::<_, String>(5)?,
            }))
        }
    ).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    Ok(serde_json::json!({
        "nb":     lignes.len(),
        "lignes": lignes,
    }))
}

// =====================================================================
//  LE TABLEAU DE BORD, SUR L'UN OU L'AUTRE MOTEUR
// =====================================================================
//
// Les cinq lectures ci-dessus, portees sur `Base`. Ce qui change, et
// pourquoi :
//
// - **Les dates.** `julianday`, `strftime`, `date('now')` n'existent pas
//   cote PostgreSQL. Les dates sont stockees en texte ISO
//   (`2026-09-11T14:05:00.000`), ce qui permet de les decouper avec
//   `SUBSTR` — que les deux moteurs ont — et de ranger les ventes dans
//   leurs cases en Rust plutot qu'en SQL. « Aujourd'hui » se calcule
//   ici et se passe en parametre.
// - **Les sommes.** PostgreSQL rend `NUMERIC` pour `SUM(bigint)`, que
//   `i64` ne sait pas lire : chaque somme est enveloppee dans
//   `CAST(... AS BIGINT)`, que SQLite accepte aussi.
// - **Le cloisonnement.** Chaque requete filtre sur `dossier_id` : un
//   tableau de bord qui additionne les ventes d'une autre societe rend
//   des chiffres plausibles, et c'est bien le probleme.

use crate::base::{Base, Valeur};
use crate::parametres;

/// Une somme ou un compte, ou 0 quand la requete ne rend rien.
///
/// L'erreur REMONTE, contrairement a la version SQLite qui retombait a
/// 0. Sur deux moteurs, un 0 silencieux cache exactement ce qu'on
/// cherche : une somme rendue dans un type illisible, une fonction
/// absente. Un tableau de bord qui dit « erreur » vaut mieux qu'un
/// tableau de bord qui dit « aucune vente ».
fn somme(base: &mut Base, sql: &str, params: &[Valeur]) -> Result<i64, String> {
    Ok(base.lire_une(sql, params, |r| r.get::<i64>(0)).map_err(|e| e.0)?.unwrap_or(0))
}

/// Le chiffre d'affaires et le nombre de ventes entre deux bornes.
///
/// Une seule requete pour les quatre periodes du resume : la borne de
/// fin est facultative, et `dep` a NULL veut dire tous les magasins.
fn ca_entre(
    base: &mut Base,
    dossier: &str,
    debut: &str,
    fin: Option<&str>,
    dep: &Option<String>,
) -> Result<(i64, i64), String> {
    Ok(base
        .lire_une(
        "SELECT
            CAST(COALESCE(SUM(
                (SELECT COALESCE(SUM(prix_pratique * quantite), 0)
                 FROM ligne_vente WHERE vente_id = v.id)
            ), 0) AS BIGINT),
            COUNT(*)
         FROM vente v
         WHERE v.date_vente >= ?1
           AND (CAST(?2 AS TEXT) IS NULL OR v.date_vente < ?2)
           AND v.statut != 'annulee'
           AND (CAST(?3 AS TEXT) IS NULL OR v.depot_id = ?3)
           AND v.dossier_id = ?4",
            &parametres![debut, fin, dep.clone(), dossier],
            |r| Ok((r.get::<i64>(0)?, r.get::<i64>(1)?)),
        )
        .map_err(|e| e.0)?
        .unwrap_or((0, 0)))
}

pub fn lire_resume_dashboard_sur(
    base: &mut Base,
    // Depot actif. None ou vide = tous les depots (vue consolidee).
    depot_id: Option<String>,
) -> Result<serde_json::Value, String> {
    let dossier = base.dossier().to_string();
    let dep: Option<String> = depot_id.filter(|d| !d.is_empty());

    let now = chrono::Local::now();
    let aujourd_hui = now.format("%Y-%m-%d").to_string();
    let debut_jour = now.format("%Y-%m-%dT00:00:00").to_string();
    let debut_semaine = {
        let lundi = now - chrono::Duration::days(now.weekday().num_days_from_monday() as i64);
        lundi.format("%Y-%m-%dT00:00:00").to_string()
    };
    let debut_mois = now.format("%Y-%m-01T00:00:00").to_string();
    let debut_mois_prec = if now.month() == 1 {
        format!("{}-12-01T00:00:00", now.year() - 1)
    } else {
        format!("{}-{:02}-01T00:00:00", now.year(), now.month() - 1)
    };

    let (ca_jour, nb_ventes_jour) = ca_entre(base, &dossier, &debut_jour, None, &dep)?;
    let (ca_semaine, _) = ca_entre(base, &dossier, &debut_semaine, None, &dep)?;
    let (ca_mois, nb_ventes_mois) = ca_entre(base, &dossier, &debut_mois, None, &dep)?;
    let (ca_mois_precedent, _) =
        ca_entre(base, &dossier, &debut_mois_prec, Some(&debut_mois), &dep)?;

    // Creances
    let (total_creances, nb_creances_ouvertes): (i64, i64) = base
        .lire_une(
            "SELECT
                CAST(COALESCE(SUM(
                    (SELECT COALESCE(SUM(prix_pratique * quantite), 0)
                     FROM ligne_vente WHERE vente_id = v.id) -
                    (SELECT COALESCE(SUM(montant), 0)
                     FROM paiement WHERE vente_id = v.id)
                ), 0) AS BIGINT),
                COUNT(DISTINCT v.id)
             FROM vente v
             WHERE v.statut IN ('creance_ouverte','partiellement_payee')
               AND (CAST(?1 AS TEXT) IS NULL OR v.depot_id = ?1)
               AND v.dossier_id = ?2",
            &parametres![dep.clone(), dossier.clone()],
            |r| Ok((r.get::<i64>(0)?, r.get::<i64>(1)?)),
        )
        .map_err(|e| e.0)?
        .unwrap_or((0, 0));

    // Creances en retard : factures dont l'echeance est depassee.
    // `date_echeance` est un jour (`AAAA-MM-JJ`) ou un instant ISO :
    // les dix premiers caracteres suffisent a le comparer a aujourd'hui.
    let nb_creances_en_retard = somme(
        base,
        "SELECT COUNT(*) FROM piece_commerciale
         WHERE type_piece = 'facture'
           AND statut IN ('brouillon','emis')
           AND date_echeance IS NOT NULL
           AND SUBSTR(date_echeance, 1, 10) < ?1
           AND dossier_id = ?2",
        &parametres![aujourd_hui, dossier.clone()],
    )?;

    let total_avoirs_ouverts = somme(
        base,
        "SELECT CAST(COALESCE(SUM(montant), 0) AS BIGINT)
         FROM avoir WHERE statut = 'ouvert' AND dossier_id = ?1",
        &parametres![dossier.clone()],
    )?;

    // Stock — meme comptage par ARTICLE que la version SQLite : creer
    // un magasin insere une ligne a 0 pour chaque article, et compter
    // les lignes doublait les ruptures le jour de l'ouverture.
    let stock_ruptures = somme(
        base,
        "SELECT COUNT(*) FROM (
           SELECT sd.article_id
           FROM stock_depot sd
           JOIN article a ON a.id = sd.article_id
           JOIN depot d ON d.id = sd.depot_id AND d.actif = 1
           WHERE a.actif = 1 AND a.gere_en_stock = 1
             AND (CAST(?1 AS TEXT) IS NULL OR sd.depot_id = ?1)
             AND sd.dossier_id = ?2
           GROUP BY sd.article_id
           HAVING SUM(sd.quantite) <= 0
         ) ruptures",
        &parametres![dep.clone(), dossier.clone()],
    )?;

    let stock_alertes = somme(
        base,
        "SELECT COUNT(*) FROM (
           SELECT sd.article_id
           FROM stock_depot sd
           JOIN article a ON a.id = sd.article_id
           JOIN depot d ON d.id = sd.depot_id AND d.actif = 1
           WHERE a.actif = 1 AND a.gere_en_stock = 1
             AND (CAST(?1 AS TEXT) IS NULL OR sd.depot_id = ?1)
             AND sd.dossier_id = ?2
           GROUP BY sd.article_id
           HAVING SUM(sd.quantite) > 0 AND SUM(sd.quantite) < 5
         ) alertes",
        &parametres![dep.clone(), dossier.clone()],
    )?;

    // Caisse. `motif <> 'ouverture'` : le fond est deja dans
    // `fond_ouverture` juste au-dessus ; l'oublier le comptait deux fois.
    let (caisse_solde, caisse_ouverte): (i64, bool) = base
        .lire_une(
            "SELECT
                CAST(COALESCE(sc.fond_ouverture, 0) +
                COALESCE((SELECT SUM(CASE WHEN sens='entree' THEN montant ELSE -montant END)
                          FROM mouvement_caisse
                          WHERE session_id = sc.id AND motif <> 'ouverture'), 0)
                AS BIGINT),
                sc.statut = 'ouverte'
             FROM session_caisse sc
             WHERE sc.statut = 'ouverte' AND sc.dossier_id = ?1
             ORDER BY sc.cree_le DESC LIMIT 1",
            &parametres![dossier.clone()],
            |r| Ok((r.get::<i64>(0)?, r.get::<bool>(1)?)),
        )
        .map_err(|e| e.0)?
        .unwrap_or((0, false));

    let factures_brouillon = somme(
        base,
        "SELECT COUNT(*) FROM piece_commerciale
         WHERE type_piece = 'facture' AND statut = 'brouillon' AND dossier_id = ?1",
        &parametres![dossier.clone()],
    )?;

    let commandes_en_attente = somme(
        base,
        "SELECT COUNT(*) FROM piece_commerciale
         WHERE type_piece = 'commande_client'
           AND statut NOT IN ('transfere','annule')
           AND dossier_id = ?1",
        &parametres![dossier],
    )?;

    Ok(serde_json::json!({
        "ca_jour":               ca_jour,
        "ca_semaine":            ca_semaine,
        "ca_mois":               ca_mois,
        "ca_mois_precedent":     ca_mois_precedent,
        "nb_ventes_jour":        nb_ventes_jour,
        "nb_ventes_mois":        nb_ventes_mois,
        "total_creances":        total_creances,
        "nb_creances_ouvertes":  nb_creances_ouvertes,
        "nb_creances_en_retard": nb_creances_en_retard,
        "total_avoirs_ouverts":  total_avoirs_ouverts,
        "stock_ruptures":        stock_ruptures,
        "stock_alertes":         stock_alertes,
        "caisse_solde":          caisse_solde,
        "caisse_session_ouverte":caisse_ouverte,
        "factures_brouillon":    factures_brouillon,
        "commandes_en_attente":  commandes_en_attente,
    }))
}

/// La periode d'une courbe : ses bornes, et le nombre de cases.
struct Periode {
    nom: &'static str,
    debut: String,
    fin: String,
    nb_cases: usize,
}

fn periode_de(p: &str, maintenant: chrono::DateTime<chrono::Local>) -> Periode {
    match p {
        "semaine" => Periode {
            nom: "semaine",
            debut: (maintenant - chrono::Duration::days(6))
                .format("%Y-%m-%dT00:00:00")
                .to_string(),
            fin: maintenant.format("%Y-%m-%dT23:59:59").to_string(),
            nb_cases: 7,
        },
        "mois" => Periode {
            nom: "mois",
            debut: maintenant.format("%Y-%m-01T00:00:00").to_string(),
            fin: maintenant.format("%Y-%m-%dT23:59:59").to_string(),
            nb_cases: 5,
        },
        "annee" => Periode {
            nom: "annee",
            debut: maintenant.format("%Y-01-01T00:00:00").to_string(),
            fin: maintenant.format("%Y-12-31T23:59:59").to_string(),
            nb_cases: 12,
        },
        _ => Periode {
            nom: "jour",
            debut: maintenant.format("%Y-%m-%dT00:00:00").to_string(),
            fin: maintenant.format("%Y-%m-%dT23:59:59").to_string(),
            nb_cases: 24,
        },
    }
}

/// La case d'une heure de vente (`AAAA-MM-JJTHH`) dans sa periode.
///
/// C'est le travail que `julianday`/`strftime` faisaient en SQL. Ici,
/// en Rust, il est le meme sur les deux moteurs — et testable sans
/// base. Rend `None` pour une cle mal formee ou hors fenetre.
pub fn case_de(periode: &str, debut: &str, cle: &str) -> Option<usize> {
    if cle.len() < 13 {
        return None;
    }
    let nombre = |de: usize, a: usize| cle.get(de..a)?.parse::<i64>().ok();
    let idx = match periode {
        "semaine" => {
            let jour = chrono::NaiveDate::parse_from_str(cle.get(..10)?, "%Y-%m-%d").ok()?;
            let origine = chrono::NaiveDate::parse_from_str(debut.get(..10)?, "%Y-%m-%d").ok()?;
            (jour - origine).num_days()
        }
        "mois" => (nombre(8, 10)? - 1) / 7,
        "annee" => nombre(5, 7)? - 1,
        _ => nombre(11, 13)?,
    };
    usize::try_from(idx).ok()
}

pub fn lire_ventes_periode_sur(
    base: &mut Base,
    periode: Option<String>,
    depot_id: Option<String>,
) -> Result<serde_json::Value, String> {
    let dossier = base.dossier().to_string();
    let dep: Option<String> = depot_id.filter(|d| !d.is_empty());
    let maintenant = chrono::Local::now();
    let per = periode_de(periode.as_deref().unwrap_or("jour"), maintenant);
    let p = per.nom;

    // Une ligne par heure de vente ; le rangement en cases se fait
    // ensuite, en Rust (voir `case_de`).
    let heures: Vec<(String, i64, i64)> = base
        .lire_plusieurs(
            "SELECT SUBSTR(v.date_vente, 1, 13) AS heure,
                    CAST(COALESCE(SUM(
                        (SELECT COALESCE(SUM(prix_pratique * quantite), 0)
                         FROM ligne_vente WHERE vente_id = v.id)
                    ), 0) AS BIGINT) AS total,
                    COUNT(*) AS nb
             FROM vente v
             WHERE v.date_vente >= ?1 AND v.date_vente <= ?2
               AND v.statut != 'annulee'
               AND (CAST(?3 AS TEXT) IS NULL OR v.depot_id = ?3)
               AND v.dossier_id = ?4
             GROUP BY SUBSTR(v.date_vente, 1, 13)",
            &parametres![per.debut.clone(), per.fin.clone(), dep, dossier],
            |r| Ok((r.get::<String>(0)?, r.get::<i64>(1)?, r.get::<i64>(2)?)),
        )
        .map_err(|e| e.0)?;

    let mut cases: Vec<(i64, i64)> = vec![(0, 0); per.nb_cases];
    for (heure, montant, nb) in heures {
        if let Some(i) = case_de(p, &per.debut, &heure) {
            if i < per.nb_cases {
                cases[i].0 += montant;
                cases[i].1 += nb;
            }
        }
    }

    // Libelles, et bornes d'affichage.
    // Une boutique n'ouvre pas a minuit : on part de 6h, sauf si une
    // vente a eu lieu avant — auquel cas la masquer serait mentir.
    let de = if p == "jour" && cases[..6].iter().all(|(m, n)| *m == 0 && *n == 0) {
        6
    } else {
        0
    };

    let points: Vec<serde_json::Value> = (de..per.nb_cases)
        .map(|i| {
            let (montant, nb) = cases[i];
            let label = match p {
                "semaine" => {
                    let d = maintenant - chrono::Duration::days((6 - i) as i64);
                    JOURS_COURTS[d.weekday().num_days_from_monday() as usize].to_string()
                }
                "mois" => format!("S{}", i + 1),
                "annee" => MOIS_COURTS[i].to_string(),
                _ => format!("{}h", i),
            };
            serde_json::json!({ "label": label, "montant": montant, "nb": nb })
        })
        .collect();

    let total: i64 = points.iter().filter_map(|p| p["montant"].as_i64()).sum();
    let nb_total: i64 = points.iter().filter_map(|p| p["nb"].as_i64()).sum();

    Ok(serde_json::json!({
        "points":   points,
        "total":    total,
        "nb":       nb_total,
        "periode":  p,
    }))
}

pub fn lire_top_clients_sur(base: &mut Base) -> Result<Vec<serde_json::Value>, String> {
    let dossier = base.dossier().to_string();
    let debut_mois = chrono::Local::now().format("%Y-%m-01T00:00:00").to_string();

    base.lire_plusieurs(
        "SELECT c.nom, c.code,
                CAST(COALESCE(SUM(
                    (SELECT COALESCE(SUM(prix_pratique * quantite), 0)
                     FROM ligne_vente WHERE vente_id = v.id)
                ), 0) AS BIGINT) AS ca,
                COUNT(v.id) AS nb_ventes
         FROM vente v
         JOIN client c ON c.id = v.client_id
         WHERE v.date_vente >= ?1 AND v.statut != 'annulee'
           AND c.est_generique = 0
           AND v.dossier_id = ?2
         GROUP BY c.id, c.nom, c.code
         ORDER BY ca DESC
         LIMIT 10",
        &parametres![debut_mois, dossier],
        |r| {
            Ok(serde_json::json!({
                "nom":       r.get::<String>(0)?,
                "code":      r.get::<String>(1)?,
                "ca":        r.get::<i64>(2)?,
                "nb_ventes": r.get::<i64>(3)?,
            }))
        },
    )
    .map_err(|e| e.0)
}

pub fn lire_top_articles_sur(base: &mut Base) -> Result<Vec<serde_json::Value>, String> {
    let dossier = base.dossier().to_string();
    let debut_mois = chrono::Local::now().format("%Y-%m-01T00:00:00").to_string();

    // `article` n'est pas cloisonne (c'est une chose, pas une
    // relation) ; ce sont les ventes qui le sont.
    base.lire_plusieurs(
        "SELECT a.nom, a.unite_base,
                CAST(COALESCE(SUM(lv.quantite * lv.prix_pratique), 0) AS BIGINT) AS ca,
                CAST(COALESCE(SUM(lv.quantite), 0) AS DOUBLE PRECISION) AS qte_vendue
         FROM ligne_vente lv
         JOIN article a ON a.id = lv.article_id
         JOIN vente v ON v.id = lv.vente_id
         WHERE v.date_vente >= ?1 AND v.statut != 'annulee'
           AND v.dossier_id = ?2
         GROUP BY a.id, a.nom, a.unite_base
         ORDER BY ca DESC
         LIMIT 10",
        &parametres![debut_mois, dossier],
        |r| {
            Ok(serde_json::json!({
                "nom":        r.get::<String>(0)?,
                "unite":      r.get::<String>(1)?,
                "ca":         r.get::<i64>(2)?,
                "qte_vendue": r.get::<f64>(3)?,
            }))
        },
    )
    .map_err(|e| e.0)
}

pub fn lire_ventes_a_decouvert_sur(
    base: &mut Base,
    date_debut: Option<String>,
    date_fin: Option<String>,
) -> Result<serde_json::Value, String> {
    let dossier = base.dossier().to_string();
    let d1 = date_debut.filter(|s| !s.is_empty());
    let d2 = date_fin.filter(|s| !s.is_empty());

    let lignes: Vec<serde_json::Value> = base
        .lire_plusieurs(
            "SELECT a.nom, a.unite_base, lv.quantite, v.date_vente,
                    COALESCE(c.nom, '—'), COALESCE(d.nom, '—')
             FROM ligne_vente lv
             JOIN vente v ON v.id = lv.vente_id
             JOIN article a ON a.id = lv.article_id
             LEFT JOIN client c ON c.id = v.client_id
             LEFT JOIN depot d ON d.id = lv.depot_source_id
             WHERE lv.vente_a_decouvert = 1
               AND v.statut <> 'annulee'
               AND (CAST(?1 AS TEXT) IS NULL OR SUBSTR(v.date_vente, 1, 10) >= ?1)
               AND (CAST(?2 AS TEXT) IS NULL OR SUBSTR(v.date_vente, 1, 10) <= ?2)
               AND v.dossier_id = ?3
             ORDER BY v.date_vente DESC
             LIMIT 200",
            &parametres![d1, d2, dossier],
            |r| {
                Ok(serde_json::json!({
                    "article":    r.get::<String>(0)?,
                    "unite_base": r.get::<String>(1)?,
                    "quantite":   r.get::<f64>(2)?,
                    "date":       r.get::<String>(3)?,
                    "client":     r.get::<String>(4)?,
                    "depot":      r.get::<String>(5)?,
                }))
            },
        )
        .map_err(|e| e.0)?;

    Ok(serde_json::json!({
        "nb":     lignes.len(),
        "lignes": lignes,
    }))
}

#[cfg(test)]
mod tests {
    use super::case_de;

    #[test]
    fn une_heure_tombe_dans_sa_case_du_jour() {
        assert_eq!(case_de("jour", "2026-09-11T00:00:00", "2026-09-11T14"), Some(14));
        assert_eq!(case_de("jour", "2026-09-11T00:00:00", "2026-09-11T00"), Some(0));
    }

    #[test]
    fn la_semaine_compte_les_jours_depuis_le_debut_de_la_fenetre() {
        let debut = "2026-09-05T00:00:00";
        assert_eq!(case_de("semaine", debut, "2026-09-05T09"), Some(0));
        assert_eq!(case_de("semaine", debut, "2026-09-11T23"), Some(6));
    }

    #[test]
    fn le_mois_se_range_par_tranche_de_sept_jours() {
        assert_eq!(case_de("mois", "2026-09-01T00:00:00", "2026-09-01T10"), Some(0));
        assert_eq!(case_de("mois", "2026-09-01T00:00:00", "2026-09-07T10"), Some(0));
        assert_eq!(case_de("mois", "2026-09-01T00:00:00", "2026-09-08T10"), Some(1));
        assert_eq!(case_de("mois", "2026-09-01T00:00:00", "2026-09-30T10"), Some(4));
    }

    #[test]
    fn l_annee_se_range_par_mois() {
        assert_eq!(case_de("annee", "2026-01-01T00:00:00", "2026-01-03T10"), Some(0));
        assert_eq!(case_de("annee", "2026-01-01T00:00:00", "2026-12-25T10"), Some(11));
    }

    #[test]
    fn une_cle_tronquee_ou_anterieure_ne_tombe_nulle_part() {
        assert_eq!(case_de("jour", "2026-09-11T00:00:00", "2026-09"), None);
        // Un jour AVANT le debut de la fenetre : index negatif, ecarte.
        assert_eq!(case_de("semaine", "2026-09-05T00:00:00", "2026-09-01T10"), None);
    }
}
