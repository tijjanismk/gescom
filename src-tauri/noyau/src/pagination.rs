//! Commandes Tauri paginées pour ventes, clients, stock, fournisseurs.
//!
//! Toutes retournent { donnees: [...], total: N, pages: P }
//! pour permettre la pagination côté frontend.


// =====================================================================
//  Utilitaire : construire la clause WHERE et les params dynamiquement
// =====================================================================

pub fn periode_vers_dates(periode: &str) -> (Option<String>, Option<String>) {
    let auj = chrono::Local::now().format("%Y-%m-%d").to_string();
    match periode {
        "aujourd_hui" => (Some(auj.clone()), Some(auj)),
        "semaine" => {
            let lundi = chrono::Local::now()
                - chrono::Duration::days(chrono::Local::now().weekday()
                    .num_days_from_monday() as i64);
            (Some(lundi.format("%Y-%m-%d").to_string()), Some(auj))
        }
        "mois" => {
            let debut = chrono::Local::now()
                .with_day(1)
                .unwrap_or(chrono::Local::now());
            (Some(debut.format("%Y-%m-%d").to_string()), Some(auj))
        }
        _ => (None, None),
    }
}

use chrono::Datelike;

// =====================================================================
//  VENTES PAGINÉES
// =====================================================================

// =====================================================================
//  Construction des filtres
// =====================================================================
//
// Les conditions WHERE etaient assemblees par `format!`, avec au mieux
// un doublement des quotes — et RIEN du tout sur les dates, qui
// viennent pourtant du filtre « personnalise ». Une commande Tauri est
// appelable directement : la valeur n'a pas a etre de confiance.
//
// `Filtres` accumule condition + valeur liee. Le SQL ne contient plus
// que des `?N`, la valeur ne touche jamais la requete.

pub struct Filtres {
    conditions: Vec<String>,
    valeurs: Vec<Box<dyn rusqlite::ToSql>>,
}

impl Filtres {
    fn new(base: &str) -> Self {
        Filtres { conditions: vec![base.to_string()], valeurs: Vec::new() }
    }

    /// `gabarit` porte un seul `{}`, remplace par le prochain `?N`.
    fn ajouter<T: rusqlite::ToSql + 'static>(&mut self, gabarit: &str, valeur: T) {
        let n = self.valeurs.len() + 1;
        self.conditions.push(gabarit.replace("{}", &format!("?{}", n)));
        self.valeurs.push(Box::new(valeur));
    }

    /// Ignore une valeur absente ou vide, sans consommer de numero.
    fn ajouter_si<T: rusqlite::ToSql + 'static>(
        &mut self, gabarit: &str, valeur: Option<T>,
    ) where T: AsRef<str> {
        if let Some(v) = valeur {
            if !v.as_ref().is_empty() {
                self.ajouter(gabarit, v.as_ref().to_string());
            }
        }
    }

    fn clause(&self) -> String {
        self.conditions.join(" AND ")
    }

    fn params(&self) -> Vec<&dyn rusqlite::ToSql> {
        self.valeurs.iter().map(|b| b.as_ref()).collect()
    }
}

pub fn lire_ventes_paginees(
    conn: &rusqlite::Connection,
    page: i64,
    limite: i64,
    recherche: Option<String>,
    statut: Option<String>,
    periode: Option<String>,
    date_debut: Option<String>,
    date_fin: Option<String>,
) -> Result<serde_json::Value, String> {

    // Résoudre les dates selon la période.
    let (date_debut_calc, date_fin_calc) = match periode.as_deref() {
        Some(p) if p != "personnalise" && !p.is_empty() => periode_vers_dates(p),
        _ => (date_debut, date_fin),
    };

    // Conditions WHERE : gabarits + valeurs liees, jamais de valeur
    // dans la requete elle-meme.
    let mut f = Filtres::new("v.id IS NOT NULL");
    f.ajouter_si("v.statut = {}", statut.clone());
    f.ajouter_si("DATE(v.date_vente) >= {}", date_debut_calc.clone());
    f.ajouter_si("DATE(v.date_vente) <= {}", date_fin_calc.clone());
    if let Some(ref r) = recherche {
        if !r.is_empty() {
            // Le `%` entoure la VALEUR liee, pas le gabarit : sinon il
            // faudrait le concatener dans le SQL.
            f.ajouter("c.nom LIKE {}", format!("%{}%", r));
        }
    }

    let where_clause = f.clause();

    // Compter le total.
    let total: i64 = conn.query_row(
        &format!(
            "SELECT COUNT(DISTINCT v.id)
             FROM vente v
             JOIN client c ON c.id = v.client_id
             WHERE {}", where_clause
        ),
        f.params().as_slice(),
        |row| row.get(0),
    ).unwrap_or(0);

    // Lire la page.
    let offset = page * limite;
    let sql = format!(
        "SELECT v.id, v.date_vente, v.statut, v.mode_reglement,
                c.nom as client_nom, c.code as client_code,
                CAST(COALESCE(SUM(lv.prix_pratique * lv.quantite), 0) AS INTEGER) as total,
                f.numero as numero_facture
         FROM vente v
         JOIN client c ON c.id = v.client_id
         LEFT JOIN ligne_vente lv ON lv.vente_id = v.id
         LEFT JOIN facture f ON f.vente_id = v.id AND f.statut = 'validee'
         WHERE {}
         GROUP BY v.id
         ORDER BY v.date_vente DESC
         LIMIT {} OFFSET {}",
        where_clause, limite, offset
    );

    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;

    let donnees: Vec<serde_json::Value> = {
        let x = stmt.query_map(f.params().as_slice(), |row| {
            Ok(serde_json::json!({
                "id":             row.get::<_, String>(0)?,
                "date_vente":     row.get::<_, String>(1)?,
                "statut":         row.get::<_, String>(2)?,
                "mode_reglement": row.get::<_, String>(3)?,
                "client_nom":     row.get::<_, String>(4)?,
                "client_code":    row.get::<_, String>(5)?,
                "total":          row.get::<_, i64>(6)?,
                "numero_facture": row.get::<_, Option<String>>(7)?,
            }))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
        x
    };

    let pages = (total as f64 / limite as f64).ceil() as i64;

    Ok(serde_json::json!({
        "donnees": donnees,
        "total":   total,
        "pages":   pages,
        "page":    page,
    }))
}

// =====================================================================
//  CLIENTS PAGINÉS
// =====================================================================

pub fn lire_clients_pagines(
    conn: &rusqlite::Connection,
    page: i64,
    limite: i64,
    recherche: Option<String>,
    avec_creances_seulement: bool,
    ventes_filtre: Option<String>,
    tri: Option<String>,
) -> Result<serde_json::Value, String> {

    let mut conditions = vec![
        "c.actif = 1".to_string(),
        "c.est_generique = 0".to_string(),
    ];

    if let Some(ref r) = recherche {
        if !r.is_empty() {
            conditions.push(format!(
                "(c.nom LIKE '%{r}%' OR c.telephone LIKE '%{r}%')",
                r = r.replace('\'', "''")
            ));
        }
    }

    // avec_creances_seulement et ventes_filtre portent sur des colonnes
    // calculées (agrégats) : elles ne peuvent pas entrer dans ce WHERE,
    // elles sont appliquées plus bas via HAVING / le WHERE externe de la
    // sous-requête.
    let where_base = conditions.join(" AND ");

    let mut having = vec!["1=1".to_string()];
    if avec_creances_seulement {
        having.push("total_creances > 0".to_string());
    }
    match ventes_filtre.as_deref() {
        Some("avec") => having.push("nb_ventes > 0".to_string()),
        Some("sans") => having.push("nb_ventes = 0".to_string()),
        _ => {}
    }
    let having_clause = having.join(" AND ");

    let order_by = match tri.as_deref() {
        Some("creance") => "total_creances DESC, c.nom ASC",
        Some("ventes") => "nb_ventes DESC, c.nom ASC",
        Some("nom") => "c.nom ASC",
        _ => "total_creances DESC, c.nom ASC",
    };

    // `statut != 'payee'` faisait entrer les ventes ANNULEES dans la
    // creance : le client restait debiteur d'une facture qui n'existe
    // plus. On enumere les deux statuts qui portent reellement une
    // creance, comme partout ailleurs (creances.rs, relances.rs,
    // dashboard.rs) — une liste fermee ne laisse pas passer un statut
    // futur par defaut.
    // Le reste EXIGIBLE, pas le reste brut — meme regle que
    // `coeur::calcul::reste_exigible`, appliquee par `lire_creances_ouvertes`
    // (creances.rs). Sans elle, un residu d'arrondi de 1 a 5 F affichait
    // un encours dans cette liste alors que l'ecran des creances, lui, ne
    // montrait rien : deux ecrans, deux verites sur le meme client.
    //
    // La regle est traduite en SQL parce qu'elle doit entrer dans un
    // agregat — impossible de rappeler la fonction Rust ligne a ligne
    // ici. Le SEUIL reste importe de `coeur` : la valeur ne s'ecrit
    // qu'a un seul endroit, meme si le test s'ecrit deux fois.
    //
    // `paye > 0` est essentiel : sans encaissement il n'y a pas de
    // residu d'arrondi, et une petite vente de 3 F reste une creance.
    let seuil = crate::coeur::calcul::SEUIL_SOLDE;
    let creance_exigible = format!(
        "CASE
           WHEN v.statut NOT IN ('creance_ouverte','partiellement_payee') THEN 0
           WHEN CAST(lv_sum.total AS INTEGER)
                - CAST(COALESCE(p_sum.paye, 0) AS INTEGER) <= 0 THEN 0
           WHEN CAST(COALESCE(p_sum.paye, 0) AS INTEGER) > 0
            AND CAST(lv_sum.total AS INTEGER)
                - CAST(COALESCE(p_sum.paye, 0) AS INTEGER) <= {seuil} THEN 0
           ELSE CAST(lv_sum.total AS INTEGER)
                - CAST(COALESCE(p_sum.paye, 0) AS INTEGER)
         END"
    );

    let sql_count = format!(
        "SELECT COUNT(*) FROM (
           SELECT c.id,
             COALESCE(SUM({}), 0) as total_creances,
             COUNT(DISTINCT CASE WHEN v.statut <> 'annulee' THEN v.id END) as nb_ventes
           FROM client c
           LEFT JOIN vente v ON v.client_id = c.id
           LEFT JOIN (SELECT vente_id, SUM(prix_pratique * quantite) as total
                      FROM ligne_vente GROUP BY vente_id) lv_sum ON lv_sum.vente_id = v.id
           LEFT JOIN (SELECT vente_id, SUM(montant) as paye
                      FROM paiement GROUP BY vente_id) p_sum ON p_sum.vente_id = v.id
           WHERE {}
           GROUP BY c.id
           HAVING {}
         ) WHERE 1=1", creance_exigible, where_base, having_clause
    );

    let total: i64 = conn.query_row(&sql_count, [], |row| row.get(0)).unwrap_or(0);

    let offset = page * limite;
    let sql = format!(
        "SELECT c.id, c.code, c.nom, c.telephone,
                COALESCE(SUM({}), 0) as total_creances,
                COUNT(DISTINCT CASE WHEN v.statut <> 'annulee' THEN v.id END) as nb_ventes
         FROM client c
         LEFT JOIN vente v ON v.client_id = c.id
         LEFT JOIN (SELECT vente_id, SUM(prix_pratique * quantite) as total
                    FROM ligne_vente GROUP BY vente_id) lv_sum ON lv_sum.vente_id = v.id
         LEFT JOIN (SELECT vente_id, SUM(montant) as paye
                    FROM paiement GROUP BY vente_id) p_sum ON p_sum.vente_id = v.id
         WHERE {}
         GROUP BY c.id
         HAVING {}
         ORDER BY {}
         LIMIT {} OFFSET {}",
        creance_exigible, where_base, having_clause, order_by, limite, offset
    );

    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;

    let donnees: Vec<serde_json::Value> = {
        let x = stmt.query_map([], |row| {
            Ok(serde_json::json!({
                "id":             row.get::<_, String>(0)?,
                "code":           row.get::<_, String>(1)?,
                "nom":            row.get::<_, String>(2)?,
                "telephone":      row.get::<_, Option<String>>(3)?,
                "total_creances": row.get::<_, i64>(4)?,
                "nb_ventes":      row.get::<_, i64>(5)?,
            }))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
        x
    };

    let pages = (total as f64 / limite as f64).ceil() as i64;

    Ok(serde_json::json!({
        "donnees": donnees,
        "total":   total,
        "pages":   pages,
        "page":    page,
    }))
}

// =====================================================================
//  STOCK PAGINÉ
// =====================================================================

pub fn lire_stocks_pagines(
    conn: &rusqlite::Connection,
    page: i64,
    limite: i64,
    recherche: Option<String>,
    a_regulariser_seulement: bool,
    categorie_id: Option<String>,
) -> Result<serde_json::Value, String> {

    // `d.actif = 1` : le stock d'un depot desactive est gele, pas
    // disparu — il n'a plus rien a faire dans la liste tant que le
    // depot n'est pas remis en service.
    let mut conditions = vec!["a.actif = 1".to_string(), "d.actif = 1".to_string()];

    if let Some(ref r) = recherche {
        if !r.is_empty() {
            conditions.push(format!(
                "a.nom LIKE '%{}%'",
                r.replace('\'', "''")
            ));
        }
    }
    if let Some(ref cid) = categorie_id {
        if !cid.is_empty() {
            conditions.push(format!("a.categorie_id = '{}'", cid.replace('\'', "''")));
        }
    }
    if a_regulariser_seulement {
        conditions.push("sd.quantite < 0".to_string());
    }

    let where_clause = conditions.join(" AND ");

    let total: i64 = conn.query_row(
        &format!(
            "SELECT COUNT(*) FROM stock_depot sd
             JOIN article a ON a.id = sd.article_id
             JOIN depot d ON d.id = sd.depot_id
             WHERE {}", where_clause
        ),
        [],
        |row| row.get(0),
    ).unwrap_or(0);

    let offset = page * limite;
    let sql = format!(
        // Les unites sont agregees en JSON par SQLite : une seule
        // requete au lieu d'une par article. Sur 400 references,
        // la difference est visible a l'ouverture de l'ecran.
        "SELECT a.id, a.nom, a.unite_base, d.nom, sd.quantite, sd.depot_id,
                (SELECT json_group_array(json_object(
                          'id', uv.id, 'libelle', uv.libelle,
                          'facteur', uv.facteur))
                 FROM unite_vente uv
                 WHERE uv.article_id = a.id AND uv.actif = 1
                 ORDER BY uv.facteur) as unites
         FROM stock_depot sd
         JOIN article a ON a.id = sd.article_id
         JOIN depot d ON d.id = sd.depot_id
         WHERE {}
         ORDER BY sd.quantite ASC, a.nom ASC
         LIMIT {} OFFSET {}",
        where_clause, limite, offset
    );

    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;

    let donnees: Vec<serde_json::Value> = {
        let x = stmt.query_map([], |row| {
            Ok(serde_json::json!({
                "article_id":  row.get::<_, String>(0)?,
                "article_nom": row.get::<_, String>(1)?,
                "unite_base":  row.get::<_, String>(2)?,
                "depot_nom":   row.get::<_, String>(3)?,
                "quantite":    row.get::<_, f64>(4)?,
                "depot_id":    row.get::<_, String>(5)?,
                // json_group_array renvoie du texte : on le reparse pour
                // que le front recoive un vrai tableau, pas une chaine.
                "unites": row.get::<_, Option<String>>(6)?
                    .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
                    .unwrap_or(serde_json::Value::Null),
            }))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
        x
    };

    let pages = (total as f64 / limite as f64).ceil() as i64;

    Ok(serde_json::json!({
        "donnees": donnees,
        "total":   total,
        "pages":   pages,
        "page":    page,
    }))
}

// =====================================================================
//  FOURNISSEURS PAGINÉS
// =====================================================================

pub fn lire_fournisseurs_pagines(
    conn: &rusqlite::Connection,
    page: i64,
    limite: i64,
    recherche: Option<String>,
) -> Result<serde_json::Value, String> {

    let mut conditions = vec![
        "f.actif = 1".to_string(),
        "f.est_voisin = 0".to_string(),
    ];

    if let Some(ref r) = recherche {
        if !r.is_empty() {
            conditions.push(format!(
                "(f.nom LIKE '%{r}%' OR f.telephone LIKE '%{r}%')",
                r = r.replace('\'', "''")
            ));
        }
    }

    let where_clause = conditions.join(" AND ");

    let total: i64 = conn.query_row(
        &format!("SELECT COUNT(*) FROM fournisseur f WHERE {}", where_clause),
        [],
        |row| row.get(0),
    ).unwrap_or(0);

    let offset = page * limite;
    let sql = format!(
        "SELECT f.id, f.nom, f.telephone, f.nif, f.adresse, f.est_voisin,
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
                  (SELECT SUM(pf.montant) FROM paiement_fournisseur pf
                   WHERE pf.fournisseur_id = f.id), 0
                ) AS INTEGER) as total_paye,
                (SELECT COUNT(*) FROM mouvement_stock ms
                 WHERE ms.type_mouvement = 'achat'
                   AND ms.fournisseur_id = f.id) as nb_achats
         FROM fournisseur f
         WHERE {}
         ORDER BY f.nom ASC
         LIMIT {} OFFSET {}",
        where_clause, limite, offset
    );

    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;

    let donnees: Vec<serde_json::Value> = {
        let x = stmt.query_map([], |row| {
            let total_achats: i64 = row.get(6)?;
            let total_paye:   i64 = row.get(7)?;
            let dette = (total_achats - total_paye).max(0);
            Ok(serde_json::json!({
                "id":        row.get::<_, String>(0)?,
                "nom":       row.get::<_, String>(1)?,
                "telephone": row.get::<_, Option<String>>(2)?,
                "nif":       row.get::<_, Option<String>>(3)?,
                "adresse":   row.get::<_, Option<String>>(4)?,
                "est_voisin":   row.get::<_, i64>(5)? != 0,
                "total_achats": total_achats,
                "total_paye":   total_paye,
                "dette":        dette,
                "nb_achats":    row.get::<_, i64>(8)?,
                // Conserve pour compatibilite avec d'eventuels appelants.
                "total_dettes": dette,
            }))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
        x
    };

    let pages = (total as f64 / limite as f64).ceil() as i64;

    Ok(serde_json::json!({
        "donnees": donnees,
        "total":   total,
        "pages":   pages,
        "page":    page,
    }))
}

// =====================================================================
//  RETOURS PAGINÉS
// =====================================================================

pub fn lire_ventes_recentes_paginee(
    conn: &rusqlite::Connection,
    page: i64,
    limite: i64,
    recherche: Option<String>,
    periode: Option<String>,
    date_debut: Option<String>,
    date_fin: Option<String>,
) -> Result<serde_json::Value, String> {

    let (date_debut_calc, date_fin_calc) = match periode.as_deref() {
        Some(p) if p != "personnalise" && !p.is_empty() => periode_vers_dates(p),
        _ => (date_debut, date_fin),
    };

    // Idem : valeurs liees, jamais concatenees (les dates n'etaient
    // meme pas echappees).
    let mut f = Filtres::new("v.id IS NOT NULL");
    f.ajouter_si("DATE(v.date_vente) >= {}", date_debut_calc.clone());
    f.ajouter_si("DATE(v.date_vente) <= {}", date_fin_calc.clone());
    if let Some(ref r) = recherche {
        if !r.is_empty() {
            f.ajouter("c.nom LIKE {}", format!("%{}%", r));
        }
    }

    let where_clause = f.clause();

    let total: i64 = conn.query_row(
        &format!(
            "SELECT COUNT(DISTINCT v.id)
             FROM vente v JOIN client c ON c.id = v.client_id
             WHERE {}", where_clause
        ),
        f.params().as_slice(),
        |row| row.get(0),
    ).unwrap_or(0);

    let offset = page * limite;
    let sql = format!(
        "SELECT v.id, v.date_vente, v.statut, v.client_id,
                c.nom, c.code, f.numero
         FROM vente v
         JOIN client c ON c.id = v.client_id
         LEFT JOIN facture f ON f.vente_id = v.id AND f.statut = 'validee'
         WHERE {}
         ORDER BY v.date_vente DESC
         LIMIT {} OFFSET {}",
        where_clause, limite, offset
    );

    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;

    let ventes_base: Vec<(String, String, String, String, String, String, Option<String>)> = {
        let x = stmt.query_map(f.params().as_slice(), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, Option<String>>(6)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
        x
    };

    let mut donnees = Vec::new();
    for (id, date_vente, statut, client_id, client_nom, client_code, numero_facture) in ventes_base {
        let mut stmt_l = conn.prepare(
            "SELECT lv.id, lv.article_id, a.nom, lv.unite_vente_id,
                    u.libelle, lv.depot_source_id,
                    lv.quantite, lv.prix_pratique,
                    CAST(lv.prix_pratique * lv.quantite AS INTEGER) as montant
             FROM ligne_vente lv
             JOIN article a ON a.id = lv.article_id
             JOIN unite_vente u ON u.id = lv.unite_vente_id
             WHERE lv.vente_id = ?1"
        ).map_err(|e| e.to_string())?;

        let lignes: Vec<serde_json::Value> = {
            let x = stmt_l.query_map(rusqlite::params![id], |row| {
                Ok(serde_json::json!({
                    "id":              row.get::<_, String>(0)?,
                    "article_id":      row.get::<_, String>(1)?,
                    "article_nom":     row.get::<_, String>(2)?,
                    "unite_vente_id":  row.get::<_, String>(3)?,
                    "unite_libelle":   row.get::<_, String>(4)?,
                    "depot_source_id": row.get::<_, String>(5)?,
                    "quantite":        row.get::<_, f64>(6)?,
                    "prix_pratique":   row.get::<_, i64>(7)?,
                    "montant":         row.get::<_, i64>(8)?,
                }))
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();
            x
        };

        let total_vente: i64 = lignes.iter()
            .filter_map(|l| l["montant"].as_i64()).sum();

        donnees.push(serde_json::json!({
            "id":             id,
            "date_vente":     date_vente,
            "statut":         statut,
            "client_id":      client_id,
            "client_nom":     client_nom,
            "client_code":    client_code,
            "numero_facture": numero_facture,
            "total":          total_vente,
            "lignes":         lignes,
        }));
    }

    let pages = (total as f64 / limite as f64).ceil() as i64;

    Ok(serde_json::json!({
        "donnees": donnees,
        "total":   total,
        "pages":   pages,
        "page":    page,
    }))
}

// =====================================================================
//  SUR L'UN OU L'AUTRE MOTEUR
// =====================================================================
//
// Toutes les listes filtrent par PARAMETRES LIES — la version SQLite
// n'en avait que deux sur cinq, les trois autres concatenaient encore
// la recherche dans le SQL. `LOWER(x) LIKE LOWER(...)` pour que la
// casse ne compte sur aucun moteur. Les unites d'une ligne de stock
// se lisent en Rust : `json_group_array` n'existe que sur SQLite.

use crate::base::{Base, Valeur};
use crate::parametres;

/// Conditions WHERE et leurs valeurs liees, sur `Valeur`.
struct FiltresBase {
    conditions: Vec<String>,
    valeurs: Vec<Valeur>,
}

impl FiltresBase {
    fn new(base: &str) -> Self {
        FiltresBase { conditions: vec![base.to_string()], valeurs: Vec::new() }
    }
    /// `gabarit` porte un seul `{}`, remplace par le prochain `?N`.
    fn ajouter(&mut self, gabarit: &str, valeur: impl Into<Valeur>) {
        let n = self.valeurs.len() + 1;
        self.conditions.push(gabarit.replace("{}", &format!("?{}", n)));
        self.valeurs.push(valeur.into());
    }
    fn ajouter_si(&mut self, gabarit: &str, valeur: Option<String>) {
        if let Some(v) = valeur.filter(|v| !v.is_empty()) {
            self.ajouter(gabarit, v);
        }
    }
    fn recherche(&mut self, gabarit: &str, valeur: &Option<String>) {
        if let Some(r) = valeur.as_deref().filter(|r| !r.is_empty()) {
            self.ajouter(gabarit, format!("%{}%", r.to_lowercase()));
        }
    }
    fn clause(&self) -> String {
        self.conditions.join(" AND ")
    }
}

fn page_json(donnees: Vec<serde_json::Value>, total: i64, limite: i64, page: i64) -> serde_json::Value {
    serde_json::json!({
        "donnees": donnees,
        "total":   total,
        "pages":   (total as f64 / limite.max(1) as f64).ceil() as i64,
        "page":    page,
    })
}

fn compter_sur(base: &mut Base, sql: &str, params: &[Valeur]) -> Result<i64, String> {
    Ok(base.lire_une(sql, params, |r| r.get::<i64>(0)).map_err(|e| e.0)?.unwrap_or(0))
}

#[allow(clippy::too_many_arguments)]
pub fn lire_ventes_paginees_sur_base(
    base: &mut Base,
    page: i64,
    limite: i64,
    recherche: Option<String>,
    statut: Option<String>,
    periode: Option<String>,
    date_debut: Option<String>,
    date_fin: Option<String>,
) -> Result<serde_json::Value, String> {
    let (d1, d2) = match periode.as_deref() {
        Some(p) if p != "personnalise" && !p.is_empty() => periode_vers_dates(p),
        _ => (date_debut, date_fin),
    };
    let mut f = FiltresBase::new("v.dossier_id = {}");
    // `new` ne lie rien : on pose le dossier comme premiere valeur.
    f.conditions[0] = "v.dossier_id = ?1".to_string();
    f.valeurs.push(Valeur::from(base.dossier()));
    f.ajouter_si("v.statut = {}", statut);
    f.ajouter_si("SUBSTR(v.date_vente, 1, 10) >= {}", d1);
    f.ajouter_si("SUBSTR(v.date_vente, 1, 10) <= {}", d2);
    f.recherche("LOWER(c.nom) LIKE {}", &recherche);
    let ou = f.clause();

    let total = compter_sur(
        base,
        &format!("SELECT COUNT(*) FROM vente v JOIN client c ON c.id = v.client_id WHERE {ou}"),
        &f.valeurs,
    )?;
    let donnees = base
        .lire_plusieurs(
            &format!(
                "SELECT v.id, v.date_vente, v.statut, v.mode_reglement, c.nom, c.code,
                        CAST(COALESCE((SELECT SUM(lv.prix_pratique * lv.quantite)
                                       FROM ligne_vente lv WHERE lv.vente_id = v.id), 0) AS BIGINT),
                        p.numero
                 FROM vente v
                 JOIN client c ON c.id = v.client_id
                 LEFT JOIN piece_commerciale p ON p.id = v.piece_id
                 WHERE {ou}
                 ORDER BY v.date_vente DESC
                 LIMIT {} OFFSET {}",
                limite.max(1), page.max(0) * limite.max(1)
            ),
            &f.valeurs,
            |r| {
                Ok(serde_json::json!({
                    "id":             r.get::<String>(0)?,
                    "date_vente":     r.get::<String>(1)?,
                    "statut":         r.get::<String>(2)?,
                    "mode_reglement": r.get::<String>(3)?,
                    "client_nom":     r.get::<String>(4)?,
                    "client_code":    r.get::<String>(5)?,
                    "total":          r.get::<i64>(6)?,
                    "numero_facture": r.get::<Option<String>>(7)?,
                }))
            },
        )
        .map_err(|e| e.0)?;
    Ok(page_json(donnees, total, limite, page))
}

#[allow(clippy::too_many_arguments)]
pub fn lire_clients_pagines_sur_base(
    base: &mut Base,
    page: i64,
    limite: i64,
    recherche: Option<String>,
    avec_creances_seulement: bool,
    ventes_filtre: Option<String>,
    tri: Option<String>,
) -> Result<serde_json::Value, String> {
    let mut f = FiltresBase::new("c.actif = 1 AND c.est_generique = 0");
    f.ajouter("c.dossier_id = {}", base.dossier());
    if let Some(r) = recherche.as_deref().filter(|r| !r.is_empty()) {
        let n = f.valeurs.len() + 1;
        f.conditions.push(format!(
            "(LOWER(c.nom) LIKE ?{n} OR LOWER(COALESCE(c.telephone, '')) LIKE ?{n})"
        ));
        f.valeurs.push(Valeur::from(format!("%{}%", r.to_lowercase())));
    }
    let ou = f.clause();

    let mut having = vec!["1=1".to_string()];
    if avec_creances_seulement {
        having.push("total_creances > 0".to_string());
    }
    match ventes_filtre.as_deref() {
        Some("avec") => having.push("nb_ventes > 0".to_string()),
        Some("sans") => having.push("nb_ventes = 0".to_string()),
        _ => {}
    }
    let having = having.join(" AND ");
    let order_by = match tri.as_deref() {
        Some("ventes") => "nb_ventes DESC, nom ASC",
        Some("nom") => "nom ASC",
        _ => "total_creances DESC, nom ASC",
    };

    // Le reste EXIGIBLE, en SQL parce qu'il entre dans un agregat ; le
    // seuil vient de `coeur` (D41).
    let seuil = crate::coeur::calcul::SEUIL_SOLDE;
    let corps = format!(
        "SELECT c.id, c.code, c.nom, c.telephone,
                CAST(COALESCE(SUM(CASE
                   WHEN v.statut NOT IN ('creance_ouverte','partiellement_payee') THEN 0
                   WHEN CAST(COALESCE(lv_sum.total, 0) AS BIGINT) - CAST(COALESCE(p_sum.paye, 0) AS BIGINT) <= 0 THEN 0
                   WHEN CAST(COALESCE(p_sum.paye, 0) AS BIGINT) > 0
                    AND CAST(COALESCE(lv_sum.total, 0) AS BIGINT) - CAST(COALESCE(p_sum.paye, 0) AS BIGINT) <= {seuil} THEN 0
                   ELSE CAST(COALESCE(lv_sum.total, 0) AS BIGINT) - CAST(COALESCE(p_sum.paye, 0) AS BIGINT)
                 END), 0) AS BIGINT) AS total_creances,
                COUNT(DISTINCT CASE WHEN v.statut <> 'annulee' THEN v.id END) AS nb_ventes
         FROM client c
         LEFT JOIN vente v ON v.client_id = c.id
         LEFT JOIN (SELECT vente_id, SUM(prix_pratique * quantite) AS total
                    FROM ligne_vente GROUP BY vente_id) lv_sum ON lv_sum.vente_id = v.id
         LEFT JOIN (SELECT vente_id, SUM(montant) AS paye
                    FROM paiement GROUP BY vente_id) p_sum ON p_sum.vente_id = v.id
         WHERE {ou}
         GROUP BY c.id, c.code, c.nom, c.telephone"
    );
    // Les filtres sur les agregats s'appliquent au RESULTAT : PostgreSQL
    // n'accepte pas un alias du SELECT dans HAVING, SQLite si.
    let filtre = format!("SELECT * FROM ({corps}) AS t WHERE {having}");

    let total = compter_sur(base, &format!("SELECT COUNT(*) FROM ({filtre}) AS n"), &f.valeurs)?;
    let donnees = base
        .lire_plusieurs(
            &format!("{filtre} ORDER BY {order_by} LIMIT {} OFFSET {}", limite.max(1), page.max(0) * limite.max(1)),
            &f.valeurs,
            |r| {
                Ok(serde_json::json!({
                    "id":             r.get::<String>(0)?,
                    "code":           r.get::<String>(1)?,
                    "nom":            r.get::<String>(2)?,
                    "telephone":      r.get::<Option<String>>(3)?,
                    "total_creances": r.get::<i64>(4)?,
                    "nb_ventes":      r.get::<i64>(5)?,
                }))
            },
        )
        .map_err(|e| e.0)?;
    Ok(page_json(donnees, total, limite, page))
}

pub fn lire_stocks_pagines_sur_base(
    base: &mut Base,
    page: i64,
    limite: i64,
    recherche: Option<String>,
    a_regulariser_seulement: bool,
    categorie_id: Option<String>,
) -> Result<serde_json::Value, String> {
    // `d.actif = 1` : le stock d'un magasin desactive est gele.
    let mut f = FiltresBase::new("a.actif = 1 AND d.actif = 1");
    f.ajouter("sd.dossier_id = {}", base.dossier());
    f.recherche("LOWER(a.nom) LIKE {}", &recherche);
    f.ajouter_si("a.categorie_id = {}", categorie_id);
    if a_regulariser_seulement {
        f.conditions.push("sd.quantite < 0".to_string());
    }
    let ou = f.clause();
    let de = "FROM stock_depot sd JOIN article a ON a.id = sd.article_id JOIN depot d ON d.id = sd.depot_id";

    let total = compter_sur(base, &format!("SELECT COUNT(*) {de} WHERE {ou}"), &f.valeurs)?;
    let lignes: Vec<(String, String, String, String, f64, String)> = base
        .lire_plusieurs(
            &format!(
                "SELECT a.id, a.nom, a.unite_base, d.nom, sd.quantite, sd.depot_id
                 {de} WHERE {ou}
                 ORDER BY sd.quantite ASC, a.nom ASC
                 LIMIT {} OFFSET {}",
                limite.max(1), page.max(0) * limite.max(1)
            ),
            &f.valeurs,
            |r| Ok((r.get::<String>(0)?, r.get::<String>(1)?, r.get::<String>(2)?, r.get::<String>(3)?, r.get::<f64>(4)?, r.get::<String>(5)?)),
        )
        .map_err(|e| e.0)?;

    // Les unites des articles de la page, en une requete.
    let ids: Vec<String> = {
        let mut v: Vec<String> = lignes.iter().map(|l| l.0.clone()).collect();
        v.sort();
        v.dedup();
        v
    };
    let mut unites: std::collections::HashMap<String, Vec<serde_json::Value>> = std::collections::HashMap::new();
    if !ids.is_empty() {
        let places: Vec<String> = (1..=ids.len()).map(|i| format!("?{i}")).collect();
        let params: Vec<Valeur> = ids.iter().map(|i| Valeur::from(i.clone())).collect();
        let toutes: Vec<(String, serde_json::Value)> = base
            .lire_plusieurs(
                &format!(
                    "SELECT article_id, id, libelle, facteur FROM unite_vente
                     WHERE actif = 1 AND article_id IN ({})
                     ORDER BY facteur",
                    places.join(",")
                ),
                &params,
                |r| Ok((r.get::<String>(0)?, serde_json::json!({ "id": r.get::<String>(1)?, "libelle": r.get::<String>(2)?, "facteur": r.get::<f64>(3)? }))),
            )
            .map_err(|e| e.0)?;
        for (art, u) in toutes {
            unites.entry(art).or_default().push(u);
        }
    }

    let donnees = lignes
        .into_iter()
        .map(|(id, nom, unite_base, depot_nom, quantite, depot_id)| {
            let u = unites.get(&id).cloned().unwrap_or_default();
            serde_json::json!({
                "article_id":  id,
                "article_nom": nom,
                "unite_base":  unite_base,
                "depot_nom":   depot_nom,
                "quantite":    quantite,
                "depot_id":    depot_id,
                "unites":      u,
            })
        })
        .collect();
    Ok(page_json(donnees, total, limite, page))
}

pub fn lire_fournisseurs_pagines_sur_base(
    base: &mut Base,
    page: i64,
    limite: i64,
    recherche: Option<String>,
) -> Result<serde_json::Value, String> {
    let mut f = FiltresBase::new("f.actif = 1 AND f.est_voisin = 0");
    f.ajouter("f.dossier_id = {}", base.dossier());
    if let Some(r) = recherche.as_deref().filter(|r| !r.is_empty()) {
        let n = f.valeurs.len() + 1;
        f.conditions.push(format!(
            "(LOWER(f.nom) LIKE ?{n} OR LOWER(COALESCE(f.telephone, '')) LIKE ?{n})"
        ));
        f.valeurs.push(Valeur::from(format!("%{}%", r.to_lowercase())));
    }
    let ou = f.clause();

    let total = compter_sur(base, &format!("SELECT COUNT(*) FROM fournisseur f WHERE {ou}"), &f.valeurs)?;
    let donnees = base
        .lire_plusieurs(
            &format!(
                "SELECT f.id, f.nom, f.telephone, f.nif, f.adresse, f.est_voisin,
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
                           WHERE pc.tiers_type = 'fournisseur' AND pc.tiers_id = f.id
                             AND pc.type_piece IN ('facture_fournisseur','avoir_fournisseur')
                             AND pc.statut <> 'annule'), 0) AS BIGINT),
                        CAST(COALESCE((SELECT SUM(pf.montant) FROM paiement_fournisseur pf
                                       WHERE pf.fournisseur_id = f.id), 0) AS BIGINT),
                        (SELECT COUNT(*) FROM mouvement_stock ms
                         WHERE ms.type_mouvement = 'achat' AND ms.fournisseur_id = f.id)
                 FROM fournisseur f
                 WHERE {ou}
                 ORDER BY f.nom ASC
                 LIMIT {} OFFSET {}",
                limite.max(1), page.max(0) * limite.max(1)
            ),
            &f.valeurs,
            |r| {
                let total_achats: i64 = r.get::<i64>(6)?;
                let total_paye: i64 = r.get::<i64>(7)?;
                let dette = (total_achats - total_paye).max(0);
                Ok(serde_json::json!({
                    "id":           r.get::<String>(0)?,
                    "nom":          r.get::<String>(1)?,
                    "telephone":    r.get::<Option<String>>(2)?,
                    "nif":          r.get::<Option<String>>(3)?,
                    "adresse":      r.get::<Option<String>>(4)?,
                    "est_voisin":   r.get::<i64>(5)? != 0,
                    "total_achats": total_achats,
                    "total_paye":   total_paye,
                    "dette":        dette,
                    "nb_achats":    r.get::<i64>(8)?,
                    "total_dettes": dette,
                }))
            },
        )
        .map_err(|e| e.0)?;
    Ok(page_json(donnees, total, limite, page))
}

#[allow(clippy::too_many_arguments)]
pub fn lire_ventes_recentes_paginee_sur_base(
    base: &mut Base,
    page: i64,
    limite: i64,
    recherche: Option<String>,
    periode: Option<String>,
    date_debut: Option<String>,
    date_fin: Option<String>,
) -> Result<serde_json::Value, String> {
    let dossier = base.dossier().to_string();
    let (d1, d2) = match periode.as_deref() {
        Some(p) if p != "personnalise" && !p.is_empty() => periode_vers_dates(p),
        _ => (date_debut, date_fin),
    };
    let mut f = FiltresBase::new("v.id IS NOT NULL");
    f.ajouter("v.dossier_id = {}", dossier.clone());
    f.ajouter_si("SUBSTR(v.date_vente, 1, 10) >= {}", d1);
    f.ajouter_si("SUBSTR(v.date_vente, 1, 10) <= {}", d2);
    f.recherche("LOWER(c.nom) LIKE {}", &recherche);
    let ou = f.clause();

    let total = compter_sur(
        base,
        &format!("SELECT COUNT(*) FROM vente v JOIN client c ON c.id = v.client_id WHERE {ou}"),
        &f.valeurs,
    )?;
    let ventes_base: Vec<(String, String, String, String, String, String, Option<String>)> = base
        .lire_plusieurs(
            &format!(
                "SELECT v.id, v.date_vente, v.statut, v.client_id, c.nom, c.code, p.numero
                 FROM vente v
                 JOIN client c ON c.id = v.client_id
                 LEFT JOIN piece_commerciale p ON p.id = v.piece_id
                 WHERE {ou}
                 ORDER BY v.date_vente DESC
                 LIMIT {} OFFSET {}",
                limite.max(1), page.max(0) * limite.max(1)
            ),
            &f.valeurs,
            |r| Ok((r.get::<String>(0)?, r.get::<String>(1)?, r.get::<String>(2)?, r.get::<String>(3)?, r.get::<String>(4)?, r.get::<String>(5)?, r.get::<Option<String>>(6)?)),
        )
        .map_err(|e| e.0)?;

    let mut donnees = Vec::with_capacity(ventes_base.len());
    for (id, date_vente, statut, client_id, client_nom, client_code, numero_facture) in ventes_base {
        let lignes: Vec<serde_json::Value> = base
            .lire_plusieurs(
                "SELECT lv.id, lv.article_id, a.nom, lv.unite_vente_id, u.libelle,
                        lv.depot_source_id, lv.quantite, lv.prix_pratique,
                        CAST(lv.prix_pratique * lv.quantite AS BIGINT)
                 FROM ligne_vente lv
                 JOIN article a ON a.id = lv.article_id
                 JOIN unite_vente u ON u.id = lv.unite_vente_id
                 WHERE lv.vente_id = ?1 AND lv.dossier_id = ?2",
                &parametres![id.clone(), dossier.clone()],
                |r| {
                    Ok(serde_json::json!({
                        "id":              r.get::<String>(0)?,
                        "article_id":      r.get::<String>(1)?,
                        "article_nom":     r.get::<String>(2)?,
                        "unite_vente_id":  r.get::<String>(3)?,
                        "unite_libelle":   r.get::<String>(4)?,
                        "depot_source_id": r.get::<String>(5)?,
                        "quantite":        r.get::<f64>(6)?,
                        "prix_pratique":   r.get::<i64>(7)?,
                        "montant":         r.get::<i64>(8)?,
                    }))
                },
            )
            .map_err(|e| e.0)?;
        let total_vente: i64 = lignes.iter().filter_map(|l| l["montant"].as_i64()).sum();
        donnees.push(serde_json::json!({
            "id": id, "date_vente": date_vente, "statut": statut, "client_id": client_id,
            "client_nom": client_nom, "client_code": client_code,
            "numero_facture": numero_facture, "total": total_vente, "lignes": lignes,
        }));
    }
    Ok(page_json(donnees, total, limite, page))
}
