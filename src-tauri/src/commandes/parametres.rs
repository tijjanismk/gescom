//! Commandes Tauri pour les paramètres (articles, catégories).

use tauri::State;
use crate::commandes::ventes::EtatApp;
use crate::utils::maintenant_iso;

#[tauri::command]
pub fn lire_categories(etat: State<EtatApp>) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(
        "SELECT id, nom FROM categorie WHERE actif = 1 ORDER BY nom"
    ).map_err(|e| e.to_string())?;
    let x = stmt.query_map([], |row| {
        Ok(serde_json::json!({"id": row.get::<_,String>(0)?, "nom": row.get::<_,String>(1)?}))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
    Ok(x)
}

#[tauri::command]
pub fn creer_categorie(etat: State<EtatApp>, nom: String) -> Result<String, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let id = uuid::Uuid::new_v4().to_string();
    let now = maintenant_iso();
    conn.execute(
        "INSERT INTO categorie (id, nom, schema_attributs, actif, cree_le, modifie_le, origine)
         VALUES (?1,?2,'[]',1,?3,?4,'app')",
        rusqlite::params![id, nom, now, now],
    ).map_err(|e| e.to_string())?;
    Ok(id)
}

#[tauri::command]
pub fn lire_articles_complets(etat: State<EtatApp>) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(
        "SELECT a.id, a.nom, a.unite_base, a.dernier_prix_achat,
                u.id, u.libelle, u.facteur, u.prix_reference, u.code_barre
         FROM article a
         JOIN unite_vente u ON u.article_id = a.id AND u.actif = 1
         WHERE a.actif = 1 ORDER BY a.nom, u.facteur"
    ).map_err(|e| e.to_string())?;

    let mut articles: Vec<serde_json::Value> = Vec::new();
    let mut courant_id = String::new();

    stmt.query_map([], |row| {
        Ok((
            row.get::<_,String>(0)?,
            row.get::<_,String>(1)?,
            row.get::<_,String>(2)?,
            row.get::<_,Option<i64>>(3)?,
            row.get::<_,String>(4)?,
            row.get::<_,String>(5)?,
            row.get::<_,f64>(6)?,
            row.get::<_,i64>(7)?,
            row.get::<_,Option<String>>(8)?,
        ))
    }).map_err(|e| e.to_string())?
    .filter_map(|r| r.ok())
    .for_each(|(art_id, art_nom, unite_base, prix_achat,
                u_id, u_libelle, facteur, prix_ref, code_barre)| {
        let unite = serde_json::json!({
            "id": u_id, "libelle": u_libelle,
            "facteur": facteur, "prix_reference": prix_ref,
            "code_barre": code_barre,
        });
        if art_id != courant_id {
            courant_id = art_id.clone();
            articles.push(serde_json::json!({
                "id": art_id, "nom": art_nom,
                "unite_base": unite_base,
                "dernier_prix_achat": prix_achat,
                "unites": [unite],
            }));
        } else if let Some(last) = articles.last_mut() {
            if let Some(unites) = last["unites"].as_array_mut() {
                unites.push(unite);
            }
        }
    });
    Ok(articles)
}

#[tauri::command]
pub fn creer_article_complet(
    etat: State<EtatApp>,
    nom: String,
    categorie_id: Option<String>,
    unite_base: String,
    prix_reference: i64,
) -> Result<String, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let now = maintenant_iso();
    let art_id = uuid::Uuid::new_v4().to_string();
    let auteur = crate::commandes::ventes::id_utilisateur_courant_pub(&conn);

    conn.execute(
        "INSERT INTO article
         (id, nom, categorie_id, unite_base, gere_en_stock, attributs,
          actif, cree_le, modifie_le, cree_par, modifie_par, origine)
         VALUES (?1,?2,?3,?4,1,'{}',1,?5,?6,?7,?8,'app')",
        rusqlite::params![art_id, nom, categorie_id, unite_base,
                          now, now, auteur, auteur],
    ).map_err(|e| e.to_string())?;

    conn.execute(
        "INSERT INTO unite_vente
         (id, article_id, libelle, facteur, prix_reference, actif,
          cree_le, modifie_le, cree_par, modifie_par, origine)
         VALUES (?1,?2,?3,1.0,?4,1,?5,?6,?7,?8,'app')",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(), art_id, unite_base,
            prix_reference, now, now, auteur, auteur
        ],
    ).map_err(|e| e.to_string())?;

    let depot_id: String = conn.query_row(
        "SELECT id FROM depot WHERE est_defaut = 1 LIMIT 1",
        [], |r| r.get(0)
    ).map_err(|e| e.to_string())?;

    conn.execute(
        "INSERT OR IGNORE INTO stock_depot (id, article_id, depot_id, quantite)
         VALUES (?1,?2,?3,0)",
        rusqlite::params![uuid::Uuid::new_v4().to_string(), art_id, depot_id],
    ).ok();

    Ok(art_id)
}

// =====================================================================
//  UNITÉS DE VENTE (packs)
// =====================================================================
//
// D39 — `facteur` s'exprime TOUJOURS en unités de base, jamais par
// rapport à l'unité précédente. Base = pièce, carton de 12, pack de
// 6 cartons -> facteurs 1, 12 et 72. Pas 6 : `stockUV` divise et
// `enregistrer_retour` multiplie, tous deux d'un seul coup.

/// Écart toléré entre le prix d'un pack et `facteur × prix de base`
/// avant d'avertir. Les remises de gros sont réelles : on signale,
/// on ne bloque pas.
const ECART_PRIX_TOLERE: f64 = 0.30;

#[tauri::command]
pub fn ajouter_unite_vente(
    etat: State<EtatApp>,
    article_id: String,
    libelle: String,
    facteur: f64,
    prix_reference: i64,
    // EAN propre au conditionnement (D45). Le carton porte le sien.
    code_barre: Option<String>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let lib = libelle.trim().to_string();

    if lib.is_empty() {
        return Err("Le libellé de l'unité est obligatoire".to_string());
    }
    if facteur <= 0.0 {
        return Err("Le facteur doit être supérieur à zéro".to_string());
    }
    if prix_reference <= 0 {
        return Err("Le prix de vente est obligatoire".to_string());
    }

    // Deux unites de meme nom rendent la vente ambigue, et l'import CSV
    // met a jour `WHERE lower(libelle) = ?` — il en toucherait deux.
    let existe: i64 = conn.query_row(
        "SELECT COUNT(*) FROM unite_vente
         WHERE article_id = ?1 AND lower(libelle) = lower(?2) AND actif = 1",
        rusqlite::params![article_id, lib], |r| r.get(0),
    ).unwrap_or(0);
    if existe > 0 {
        return Err(format!("L'unité « {} » existe déjà pour cet article.", lib));
    }

    // Un code-barres en double fait scanner le mauvais article : la
    // caisse vend un carton au prix d'une piece sans que personne ne
    // le voie. On verifie des DEUX cotes — `article` et `unite_vente`.
    let cb = code_barre.map(|c| c.trim().to_string()).filter(|c| !c.is_empty());
    if let Some(ref c) = cb {
        let pris: i64 = conn.query_row(
            "SELECT (SELECT COUNT(*) FROM article WHERE code_barre = ?1)
                  + (SELECT COUNT(*) FROM unite_vente
                     WHERE code_barre = ?1 AND actif = 1)",
            rusqlite::params![c], |r| r.get(0),
        ).unwrap_or(0);
        if pris > 0 {
            return Err(format!("Le code-barres {} est déjà attribué.", c));
        }
    }

    let now = maintenant_iso();
    let auteur = crate::commandes::ventes::id_utilisateur_courant_pub(&conn);
    let id = uuid::Uuid::new_v4().to_string();

    conn.execute(
        "INSERT INTO unite_vente
         (id, article_id, libelle, facteur, prix_reference, code_barre, actif,
          cree_le, modifie_le, cree_par, modifie_par, origine)
         VALUES (?1,?2,?3,?4,?5,?6,1,?7,?8,?9,?10,'app')",
        rusqlite::params![id, article_id, lib, facteur, prix_reference,
                          cb, now, now, auteur, auteur],
    ).map_err(|e| e.to_string())?;

    // Avertissement, pas un refus : un pack a 1 000 F quand la piece
    // est a 100 F est presque toujours une faute de frappe, mais le
    // patron reste seul juge de sa remise de gros.
    let prix_base: Option<i64> = conn.query_row(
        "SELECT prix_reference FROM unite_vente
         WHERE article_id = ?1 AND facteur = 1.0 AND actif = 1 LIMIT 1",
        rusqlite::params![article_id], |r| r.get(0),
    ).ok();

    let alerte = prix_base.and_then(|pb| {
        let attendu = pb as f64 * facteur;
        if attendu <= 0.0 { return None; }
        let ecart = (prix_reference as f64 - attendu).abs() / attendu;
        if ecart > ECART_PRIX_TOLERE {
            Some(format!(
                "Prix inhabituel : {} F pour {} × {} F attendus ({} F). À vérifier.",
                prix_reference, facteur, pb, attendu.round() as i64
            ))
        } else {
            None
        }
    });

    Ok(serde_json::json!({ "id": id, "alerte": alerte }))
}

#[tauri::command]
pub fn modifier_unite_vente(
    etat: State<EtatApp>,
    unite_id: String,
    libelle: Option<String>,
    facteur: Option<f64>,
    prix_reference: Option<i64>,
    code_barre: Option<String>,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    if let Some(f) = facteur {
        if f <= 0.0 {
            return Err("Le facteur doit être supérieur à zéro".to_string());
        }
    }
    if let Some(p) = prix_reference {
        if p <= 0 {
            return Err("Le prix de vente doit être positif".to_string());
        }
    }

    // Le facteur de l'unite de base est structurant : `lire_etat_stock`
    // et l'export CSV cherchent `facteur = 1.0` pour trouver l'unite
    // d'affichage. Le changer ferait disparaitre l'article des etats.
    let facteur_actuel: f64 = conn.query_row(
        "SELECT facteur FROM unite_vente WHERE id = ?1",
        rusqlite::params![unite_id], |r| r.get(0),
    ).map_err(|_| "Unité introuvable".to_string())?;

    if facteur_actuel == 1.0 && facteur.map(|f| f != 1.0).unwrap_or(false) {
        return Err(
            "Le facteur de l'unité de base ne peut pas changer. \
             Créer une nouvelle unité pour le conditionnement.".to_string()
        );
    }

    conn.execute(
        "UPDATE unite_vente SET
            libelle = COALESCE(?1, libelle),
            facteur = COALESCE(?2, facteur),
            prix_reference = COALESCE(?3, prix_reference),
            code_barre = COALESCE(?4, code_barre),
            modifie_le = ?5
         WHERE id = ?6",
        rusqlite::params![
            libelle.map(|l| l.trim().to_string()).filter(|l| !l.is_empty()),
            facteur, prix_reference,
            code_barre.map(|c| c.trim().to_string()).filter(|c| !c.is_empty()),
            maintenant_iso(), unite_id
        ],
    ).map_err(|e| e.to_string())?;

    Ok(())
}

/// Désactive une unité. Jamais de suppression : `ligne_vente` et
/// `ligne_piece` référencent `unite_vente_id` sur des pièces émises.
#[tauri::command]
pub fn desactiver_unite_vente(
    etat: State<EtatApp>,
    unite_id: String,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let (article_id, facteur): (String, f64) = conn.query_row(
        "SELECT article_id, facteur FROM unite_vente WHERE id = ?1",
        rusqlite::params![unite_id], |r| Ok((r.get(0)?, r.get(1)?)),
    ).map_err(|_| "Unité introuvable".to_string())?;

    if facteur == 1.0 {
        return Err(
            "L'unité de base ne peut pas être désactivée : c'est elle qui \
             porte le stock et les états.".to_string()
        );
    }

    let restantes: i64 = conn.query_row(
        "SELECT COUNT(*) FROM unite_vente
         WHERE article_id = ?1 AND actif = 1 AND id <> ?2",
        rusqlite::params![article_id, unite_id], |r| r.get(0),
    ).unwrap_or(0);
    if restantes == 0 {
        return Err("Un article doit garder au moins une unité de vente.".to_string());
    }

    conn.execute(
        "UPDATE unite_vente SET actif = 0, modifie_le = ?1 WHERE id = ?2",
        rusqlite::params![maintenant_iso(), unite_id],
    ).map_err(|e| e.to_string())?;

    Ok(())
}

// =====================================================================
//  BON DE SORTIE
// =====================================================================
//
// Reglage d'atelier : inutile la ou le vendeur remet lui-meme la
// marchandise, indispensable quand le magasin est separe de la caisse
// (quincaillerie, depot de materiaux). Celui qui encaisse n'est alors
// pas celui qui delivre, et le bon est la seule piece qui circule
// entre les deux.

#[tauri::command]
pub fn lire_config_bon_sortie(etat: State<EtatApp>) -> Result<bool, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let v: String = conn.query_row(
        "SELECT valeur FROM config_app WHERE cle = 'bon_sortie_actif'",
        [], |r| r.get(0),
    ).unwrap_or_else(|_| "0".to_string());
    Ok(v == "1")
}

#[tauri::command]
pub fn sauvegarder_config_bon_sortie(
    etat: State<EtatApp>,
    actif: bool,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO config_app (cle, valeur) VALUES ('bon_sortie_actif', ?1)
         ON CONFLICT(cle) DO UPDATE SET valeur = ?1",
        rusqlite::params![if actif { "1" } else { "0" }],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn lire_stocks(etat: State<EtatApp>) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare(
        "SELECT a.id, a.nom, a.unite_base, d.nom, sd.quantite, sd.depot_id
         FROM stock_depot sd
         JOIN article a ON a.id = sd.article_id
         JOIN depot d ON d.id = sd.depot_id
         WHERE a.actif = 1
         ORDER BY sd.quantite ASC, a.nom ASC"
    ).map_err(|e| e.to_string())?;
    let x = stmt.query_map([], |row| {
        Ok(serde_json::json!({
            "article_id": row.get::<_,String>(0)?,
            "article_nom": row.get::<_,String>(1)?,
            "unite_base": row.get::<_,String>(2)?,
            "depot_nom": row.get::<_,String>(3)?,
            "quantite": row.get::<_,f64>(4)?,
            "depot_id": row.get::<_,String>(5)?,
        }))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
    Ok(x)
}