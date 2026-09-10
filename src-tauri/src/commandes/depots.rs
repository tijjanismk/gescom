//! Gestion des dépôts — scénario « un poste, plusieurs magasins ».
//!
//! Le patron gère tous ses points de vente depuis un seul ordinateur.
//! Chaque dépôt a son stock ; les ventes, achats et pièces portent le
//! dépôt sur lequel ils ont eu lieu.
//!
//! Ce n'est PAS du multi-poste : une seule installation, une seule base.
//! Le multi-poste (chaque magasin avec son ordinateur) demande l'API
//! réseau de la v2.

use tauri::State;
use crate::commandes::ventes::EtatApp;
use crate::utils::maintenant_iso;

/// Tous les dépôts, avec leur nombre d'articles et la valeur du stock.
#[tauri::command]
pub fn lire_depots_detail(
    etat: State<EtatApp>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let mut st = conn.prepare(
        "SELECT d.id, d.nom, d.est_defaut, d.actif,
                (SELECT COUNT(*) FROM stock_depot sd
                  JOIN article a ON a.id = sd.article_id
                  WHERE sd.depot_id = d.id AND sd.quantite > 0 AND a.actif = 1),
                CAST(COALESCE((SELECT SUM(sd.quantite * COALESCE(a.dernier_prix_achat,0))
                  FROM stock_depot sd JOIN article a ON a.id = sd.article_id
                  WHERE sd.depot_id = d.id AND sd.quantite > 0), 0) AS INTEGER),
                (SELECT COUNT(*) FROM vente v WHERE v.depot_id = d.id),
                -- Unites encore presentes, et unites manquantes : l'ecran
                -- doit pouvoir prevenir avant une desactivation forcee.
                COALESCE((SELECT SUM(sd.quantite) FROM stock_depot sd
                  WHERE sd.depot_id = d.id AND sd.quantite > 0), 0),
                COALESCE((SELECT SUM(-sd.quantite) FROM stock_depot sd
                  WHERE sd.depot_id = d.id AND sd.quantite < 0), 0)
         FROM depot d
         ORDER BY d.est_defaut DESC, d.nom"
    ).map_err(|e| e.to_string())?;

    let x = st.query_map([], |r| {
        Ok(serde_json::json!({
            "id":            r.get::<_, String>(0)?,
            "nom":           r.get::<_, String>(1)?,
            "est_defaut":    r.get::<_, i64>(2)? != 0,
            "actif":         r.get::<_, i64>(3)? != 0,
            "nb_articles":   r.get::<_, i64>(4)?,
            "valeur_stock":  r.get::<_, i64>(5)?,
            "nb_ventes":     r.get::<_, i64>(6)?,
            "unites_stock":  r.get::<_, f64>(7)?,
            "unites_manque": r.get::<_, f64>(8)?,
        }))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    Ok(x)
}

#[tauri::command]
pub fn creer_depot(
    etat: State<EtatApp>,
    nom: String,
    est_defaut: Option<bool>,
) -> Result<serde_json::Value, String> {
    if nom.trim().is_empty() {
        return Err("Le nom du dépôt est obligatoire".to_string());
    }

    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let now = maintenant_iso();
    let id = uuid::Uuid::new_v4().to_string();
    let defaut = est_defaut.unwrap_or(false);

    // Un seul dépôt par défaut : l'ancien perd son statut.
    if defaut {
        conn.execute("UPDATE depot SET est_defaut = 0", [])
            .map_err(|e| e.to_string())?;
    }

    conn.execute(
        "INSERT INTO depot (id, nom, est_defaut, actif, cree_le, modifie_le, origine)
         VALUES (?1, ?2, ?3, 1, ?4, ?5, 'app')",
        rusqlite::params![id, nom.trim(), defaut as i64, now, now],
    ).map_err(|e| e.to_string())?;

    // Initialiser le stock à zéro pour tous les articles actifs, sinon
    // le nouveau dépôt n'apparaît dans aucun état de stock tant qu'il
    // n'a rien reçu.
    conn.execute(
        "INSERT OR IGNORE INTO stock_depot (id, article_id, depot_id, quantite)
         SELECT lower(hex(randomblob(16))), a.id, ?1, 0
         FROM article a WHERE a.actif = 1",
        rusqlite::params![id],
    ).ok();

    Ok(serde_json::json!({ "id": id, "nom": nom.trim() }))
}

#[tauri::command]
pub fn renommer_depot(
    etat: State<EtatApp>,
    depot_id: String,
    nom: String,
) -> Result<(), String> {
    if nom.trim().is_empty() {
        return Err("Le nom du dépôt est obligatoire".to_string());
    }
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE depot SET nom = ?1, modifie_le = ?2 WHERE id = ?3",
        rusqlite::params![nom.trim(), maintenant_iso(), depot_id],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn definir_depot_defaut(
    etat: State<EtatApp>,
    depot_id: String,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    conn.execute("UPDATE depot SET est_defaut = 0", [])
        .map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE depot SET est_defaut = 1, modifie_le = ?1 WHERE id = ?2",
        rusqlite::params![maintenant_iso(), depot_id],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

/// Désactive un dépôt.
///
/// Par défaut, refusé s'il reste du stock — positif ou négatif : la
/// marchandise disparaîtrait des états sans avoir été ni transférée ni
/// vendue.
///
/// `force` lève ce refus. Le stock n'est alors ni déplacé ni soldé : il
/// est GELÉ tel quel dans `stock_depot`. Les écrans, qui filtrent sur
/// les dépôts actifs, cessent de l'afficher ; il réapparaît intact à la
/// réactivation. C'est le cas du magasin qu'on ferme sans avoir le temps
/// de faire le tour de ses rayons — mais le journal en garde la trace,
/// pour que personne ne découvre le trou six mois plus tard sans
/// explication.
#[tauri::command]
pub fn desactiver_depot(
    etat: State<EtatApp>,
    depot_id: String,
    force: Option<bool>,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    desactiver_depot_sur(&conn, depot_id, force)
}

/// Logique de `desactiver_depot`, sur une connexion quelconque.
///
/// Separee de la commande pour etre jouable sur une base de test :
/// les scenarios de `tests_multi_depot` verifient ce que le SQL fait
/// reellement a la base, ce qu'aucun test de formule ne montre.
pub(crate) fn desactiver_depot_sur(
    conn: &rusqlite::Connection,
    depot_id: String,
    force: Option<bool>,
) -> Result<(), String> {

    let est_defaut: i64 = conn.query_row(
        "SELECT est_defaut FROM depot WHERE id = ?1",
        rusqlite::params![depot_id], |r| r.get(0),
    ).map_err(|_| "Dépôt introuvable".to_string())?;

    if est_defaut != 0 {
        return Err(
            "Impossible de désactiver le dépôt par défaut. \
             Désigner un autre dépôt par défaut d'abord.".to_string()
        );
    }

    // Le stock NEGATIF compte autant que le positif : un dépôt laissé à
    // -12 après une vente à découvert emportait sa dette hors des écrans
    // en se désactivant.
    let (reste, manque): (f64, f64) = conn.query_row(
        "SELECT COALESCE(SUM(CASE WHEN quantite > 0 THEN quantite END), 0),
                COALESCE(SUM(CASE WHEN quantite < 0 THEN -quantite END), 0)
         FROM stock_depot WHERE depot_id = ?1",
        rusqlite::params![depot_id], |r| Ok((r.get(0)?, r.get(1)?)),
    ).unwrap_or((0.0, 0.0));

    let forcer = force.unwrap_or(false);

    if !forcer && reste > 0.0 {
        return Err(format!(
            "Ce dépôt contient encore {} unité(s) en stock. \
             Transférer la marchandise avant de le désactiver.",
            reste
        ));
    }

    if !forcer && manque > 0.0 {
        return Err(format!(
            "Ce dépôt est à découvert de {} unité(s). Régulariser par \
             une entrée ou un ajustement avant de le désactiver.",
            manque
        ));
    }

    let now = maintenant_iso();
    conn.execute(
        "UPDATE depot SET actif = 0, est_defaut = 0, modifie_le = ?1 WHERE id = ?2",
        rusqlite::params![now, depot_id],
    ).map_err(|e| e.to_string())?;

    // Fermeture sur stock non vide : la trace vaut l'inventaire. Sans
    // elle, la marchandise gelee n'est plus mentionnee nulle part.
    if reste > 0.0 || manque > 0.0 {
        let auteur = crate::commandes::ventes::id_utilisateur_courant_pub(&conn);
        conn.execute(
            "INSERT INTO journal
             (id, type_evenement, entite_type, entite_id, auteur_id,
              nouveau_valeur, origine, date_evenement)
             VALUES (?1,'depot_desactive_avec_stock','depot',?2,?3,?4,'app',?5)",
            rusqlite::params![
                uuid::Uuid::new_v4().to_string(), depot_id, auteur,
                format!(r#"{{"stock_gele":{},"decouvert_gele":{}}}"#,
                        reste, manque),
                now
            ],
        ).ok();
    }

    Ok(())
}

/// Remet un dépôt en service.
///
/// Indispensable depuis que la désactivation peut geler du stock : sans
/// elle, la marchandise restée dans un dépôt fermé serait inatteignable.
/// Le stock gelé réapparaît tel qu'il était, aucun mouvement n'est créé
/// — il n'a jamais bougé.
#[tauri::command]
pub fn reactiver_depot(
    etat: State<EtatApp>,
    depot_id: String,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    reactiver_depot_sur(&conn, depot_id)
}

/// Logique de `reactiver_depot`, sur une connexion quelconque.
///
/// Separee de la commande pour etre jouable sur une base de test :
/// les scenarios de `tests_multi_depot` verifient ce que le SQL fait
/// reellement a la base, ce qu'aucun test de formule ne montre.
pub(crate) fn reactiver_depot_sur(
    conn: &rusqlite::Connection,
    depot_id: String,
) -> Result<(), String> {
    let now = maintenant_iso();

    let modifiees = conn.execute(
        // `est_defaut` reste a 0 : le depot par defaut se choisit
        // explicitement, il ne se recupere pas par accident.
        "UPDATE depot SET actif = 1, modifie_le = ?1 WHERE id = ?2 AND actif = 0",
        rusqlite::params![now, depot_id],
    ).map_err(|e| e.to_string())?;

    if modifiees == 0 {
        return Err("Dépôt introuvable ou déjà actif".to_string());
    }

    let auteur = crate::commandes::ventes::id_utilisateur_courant_pub(&conn);
    conn.execute(
        "INSERT INTO journal
         (id, type_evenement, entite_type, entite_id, auteur_id,
          origine, date_evenement)
         VALUES (?1,'depot_reactive','depot',?2,?3,'app',?4)",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(), depot_id, auteur, now
        ],
    ).ok();

    Ok(())
}

/// Stock d'un dépôt précis — l'écran Transferts en a besoin pour
/// afficher le stock de la SOURCE et non celui du dépôt par défaut.
#[tauri::command]
pub fn lire_stock_depot(
    etat: State<EtatApp>,
    depot_id: String,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let mut st = conn.prepare(
        "SELECT a.id, a.nom, a.unite_base, COALESCE(sd.quantite, 0)
         FROM article a
         LEFT JOIN stock_depot sd ON sd.article_id = a.id AND sd.depot_id = ?1
         WHERE a.actif = 1
         ORDER BY a.nom"
    ).map_err(|e| e.to_string())?;

    let x = st.query_map(rusqlite::params![depot_id], |r| {
        Ok(serde_json::json!({
            "article_id": r.get::<_, String>(0)?,
            "nom":        r.get::<_, String>(1)?,
            "unite_base": r.get::<_, String>(2)?,
            "quantite":   r.get::<_, f64>(3)?,
        }))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    Ok(x)
}

/// Comparatif entre magasins — la vue que le cahier Excel appelait
/// « créance magasin » et « acompte magasin », mais fondée sur les
/// ventes réelles plutôt que sur des transferts facturés.
#[tauri::command]
pub fn lire_resume_par_depot(
    etat: State<EtatApp>,
    date_debut: Option<String>,
    date_fin: Option<String>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let d1 = date_debut.unwrap_or_else(||
        chrono::Local::now().format("%Y-%m-01").to_string());
    let d2 = date_fin.unwrap_or_else(||
        chrono::Local::now().format("%Y-%m-%d").to_string());

    let mut st = conn.prepare(
        "SELECT d.id, d.nom,
                CAST(COALESCE(SUM(
                  (SELECT SUM(lv.prix_pratique * lv.quantite)
                   FROM ligne_vente lv WHERE lv.vente_id = v.id)), 0) AS INTEGER),
                COUNT(DISTINCT v.id),
                CAST(COALESCE(SUM(
                  (SELECT COALESCE(SUM(p.montant), 0)
                   FROM paiement p WHERE p.vente_id = v.id)), 0) AS INTEGER)
         FROM depot d
         LEFT JOIN vente v ON v.depot_id = d.id
              AND DATE(v.date_vente) BETWEEN ?1 AND ?2
              AND v.statut <> 'annulee'
         WHERE d.actif = 1
         GROUP BY d.id
         ORDER BY d.est_defaut DESC, d.nom"
    ).map_err(|e| e.to_string())?;

    let x = st.query_map(rusqlite::params![d1, d2], |r| {
        let ca: i64 = r.get(2)?;
        let paye: i64 = r.get(4)?;
        Ok(serde_json::json!({
            "depot_id":  r.get::<_, String>(0)?,
            "nom":       r.get::<_, String>(1)?,
            "ca":        ca,
            "nb_ventes": r.get::<_, i64>(3)?,
            "encaisse":  paye,
            "impaye":    (ca - paye).max(0),
        }))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    Ok(x)
}

/// Stock d'un article dans TOUS les dépôts.
///
/// Le vendeur doit voir ce qu'il a sous la main et ce qui dort à côté.
/// Sans cette vue, il vend à l'aveugle : soit il refuse une vente qu'il
/// pouvait honorer, soit il promet une quantité qu'il n'a pas.
#[tauri::command]
pub fn lire_stock_article_depots(
    etat: State<EtatApp>,
    article_id: String,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let mut st = conn.prepare(
        "SELECT d.id, d.nom, d.est_defaut, COALESCE(sd.quantite, 0)
         FROM depot d
         LEFT JOIN stock_depot sd
                ON sd.depot_id = d.id AND sd.article_id = ?1
         WHERE d.actif = 1
         ORDER BY d.est_defaut DESC, d.nom"
    ).map_err(|e| e.to_string())?;

    let x = st.query_map(rusqlite::params![article_id], |r| {
        Ok(serde_json::json!({
            "depot_id":   r.get::<_, String>(0)?,
            "nom":        r.get::<_, String>(1)?,
            "est_defaut": r.get::<_, i64>(2)? != 0,
            "quantite":   r.get::<_, f64>(3)?,
        }))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    Ok(x)
}

/// Stock de plusieurs articles dans tous les dépôts, en un appel.
///
/// Le POS charge ses articles une fois au démarrage ; interroger dépôt
/// par dépôt à chaque frappe serait intenable.
#[tauri::command]
pub fn lire_stock_multi_depots(
    etat: State<EtatApp>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    lire_stock_multi_depots_sur(&conn)
}

/// Logique de `lire_stock_multi_depots`, sur une connexion quelconque.
///
/// Separee de la commande pour etre jouable sur une base de test :
/// les scenarios de `tests_multi_depot` verifient ce que le SQL fait
/// reellement a la base, ce qu'aucun test de formule ne montre.
pub(crate) use gescom_noyau::comptoir::lire_stock_multi_depots_sur;

/// Historique GLOBAL des mouvements de stock.
///
/// Le journal ne montre que la journee : des le lendemain, un
/// ajustement ou un transfert devient invisible. Or c'est justement
/// l'historique qui permet de comprendre un stock faux — « qui a sorti
/// ces 12 sacs, et quand ».
///
/// Filtres tous optionnels. `limite` borne le retour : la table grossit
/// d'une ligne par vente et par article.
#[tauri::command]
pub fn lire_mouvements_stock(
    etat: State<EtatApp>,
    article_id: Option<String>,
    depot_id: Option<String>,
    type_mouvement: Option<String>,
    date_debut: Option<String>,
    date_fin: Option<String>,
    limite: Option<i64>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let lim = limite.unwrap_or(300).clamp(1, 2000);

    // Parametres lies, jamais de `format!` sur des valeurs : chaque
    // filtre est neutralise par un NULL cote SQL plutot que par une
    // concatenation (cf. lire_fournisseurs_pagines, encore a corriger).
    let vide = |o: Option<String>| o.filter(|s| !s.is_empty());

    let mut st = conn.prepare(
        // Le numero de facture ne se lit jamais directement sur
        // mouvement_stock : operation_id pointe vers une table differente
        // selon le type (vente, retour/echange via la vente d'origine,
        // achat/retour_fournisseur directement vers la piece). D'ou les
        // quatre jointures conditionnelles, ramenees a une seule colonne.
        "SELECT ms.date_mouvement, ms.type_mouvement, a.nom, a.unite_base,
                ms.quantite_delta, d.nom, COALESCE(ms.motif, ''),
                COALESCE(u.nom, '—'),
                COALESCE(f.nom, ''),
                CAST(COALESCE(ms.prix_achat_unitaire, 0) AS INTEGER),
                COALESCE(pc_vente.numero, pc_ret.numero, pc_op.numero, '')
         FROM mouvement_stock ms
         JOIN article a ON a.id = ms.article_id
         JOIN depot d ON d.id = ms.depot_id
         LEFT JOIN utilisateur u ON u.id = ms.auteur_id
         LEFT JOIN fournisseur f ON f.id = ms.fournisseur_id
         LEFT JOIN vente v_op ON ms.type_mouvement = 'vente'
           AND v_op.id = ms.operation_id
         LEFT JOIN retour ret_op ON ms.type_mouvement IN ('retour','echange')
           AND ret_op.id = ms.operation_id
         LEFT JOIN vente v_ret ON v_ret.id = ret_op.vente_id
         LEFT JOIN piece_commerciale pc_vente
           ON pc_vente.id = COALESCE(v_op.piece_id, v_ret.piece_id)
         LEFT JOIN piece_commerciale pc_ret ON ms.type_mouvement = 'retour_fournisseur'
           AND pc_ret.id = ms.operation_id
         LEFT JOIN piece_commerciale pc_op ON ms.type_mouvement = 'achat'
           AND pc_op.id = ms.operation_id
         WHERE (?1 IS NULL OR ms.article_id = ?1)
           AND (?2 IS NULL OR ms.depot_id = ?2)
           AND (?3 IS NULL OR ms.type_mouvement = ?3)
           AND (?4 IS NULL OR DATE(ms.date_mouvement) >= ?4)
           AND (?5 IS NULL OR DATE(ms.date_mouvement) <= ?5)
         ORDER BY ms.date_mouvement DESC
         LIMIT ?6"
    ).map_err(|e| e.to_string())?;

    let x = st.query_map(
        rusqlite::params![
            vide(article_id), vide(depot_id), vide(type_mouvement),
            vide(date_debut), vide(date_fin), lim
        ],
        |r| {
            let t: String = r.get(1)?;
            let delta: f64 = r.get(4)?;
            Ok(serde_json::json!({
                "date":        r.get::<_, String>(0)?,
                "type":        t,
                "libelle":     crate::coeur::stock::libelle(&t),
                "article":     r.get::<_, String>(2)?,
                "unite_base":  r.get::<_, String>(3)?,
                "quantite":    delta.abs(),
                // Le signe tranche : ajustement et transfert n'ont pas
                // de sens fixe (coeur::stock::est_entrant).
                "entrant":     delta > 0.0,
                "depot":       r.get::<_, String>(5)?,
                "motif":       r.get::<_, String>(6)?,
                "auteur":      r.get::<_, String>(7)?,
                "fournisseur": r.get::<_, String>(8)?,
                "prix_achat":  r.get::<_, i64>(9)?,
                "numero_facture": r.get::<_, String>(10)?,
            }))
        }
    ).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    Ok(x)
}

/// Ventes a decouvert sur une periode — marchandise sortie au-dela du
/// stock connu. Chacune signale soit un stock faux, soit une entree non
/// saisie : c'est le compteur qui dit quand aller regulariser.
#[tauri::command]
pub fn lire_ventes_a_decouvert(
    etat: State<EtatApp>,
    date_debut: Option<String>,
    date_fin: Option<String>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
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