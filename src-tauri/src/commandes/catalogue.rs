//! Import / export du catalogue en CSV.
//!
//! Format retenu : CSV point-virgule, encodage UTF-8 avec BOM. C'est ce
//! qu'Excel francophone ouvre correctement en double-cliquant — un CSV
//! virgule sans BOM lui donne une seule colonne pleine de caractères
//! abîmés, et le commerçant conclut que l'export ne marche pas.
//!
//! L'import est TOLÉRANT sur la forme et STRICT sur le fond : il
//! accepte les colonnes dans le désordre, les espaces, les prix écrits
//! « 12 500 » ou « 12500 » — mais refuse un article sans nom ou sans
//! prix, et signale chaque ligne rejetée avec son numéro.

use tauri::State;
use crate::commandes::ventes::EtatApp;
use crate::utils::maintenant_iso;

const ENTETE: &str = "Nom;Categorie;Unite;Prix;Prix achat;TVA %;Code barre;Stock";

fn echapper(v: &str) -> String {
    // Un champ commencant par =, +, -, @ (ou tab/CR) est interprete
    // comme une formule par Excel/LibreOffice a l'ouverture — un nom
    // d'article du genre "=cmd|'/c calc'!A0" deviendrait executable
    // chez le patron qui exporte puis ouvre le fichier. On neutralise
    // en prefixant d'une apostrophe : Excel l'affiche comme texte brut.
    let v = if v.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{}", v)
    } else {
        v.to_string()
    };

    // Le point-virgule et le retour a la ligne cassent la structure.
    if v.contains(';') || v.contains('\n') || v.contains('"') {
        format!("\"{}\"", v.replace('"', "\"\""))
    } else {
        v
    }
}

/// Exporte le catalogue complet.
#[tauri::command]
pub fn exporter_articles_csv(
    etat: State<EtatApp>,
) -> Result<String, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    let mut st = conn.prepare(
        "SELECT a.nom, COALESCE(c.nom, ''), COALESCE(uv.libelle, a.unite_base),
                CAST(COALESCE(uv.prix_reference, 0) AS INTEGER),
                CAST(COALESCE(a.dernier_prix_achat, 0) AS INTEGER),
                COALESCE(a.taux_tva_defaut, 0.0),
                COALESCE(a.code_barre, ''),
                COALESCE((SELECT SUM(sd.quantite) FROM stock_depot sd
                          WHERE sd.article_id = a.id), 0)
         FROM article a
         LEFT JOIN categorie c ON c.id = a.categorie_id
         LEFT JOIN unite_vente uv ON uv.article_id = a.id AND uv.actif = 1
         WHERE a.actif = 1
         GROUP BY a.id, uv.id
         ORDER BY a.nom, uv.facteur"
    ).map_err(|e| e.to_string())?;

    let lignes: Vec<String> = st.query_map([], |r| {
        let taux: f64 = r.get(5)?;
        Ok(format!(
            "{};{};{};{};{};{};{};{}",
            echapper(&r.get::<_, String>(0)?),
            echapper(&r.get::<_, String>(1)?),
            echapper(&r.get::<_, String>(2)?),
            r.get::<_, i64>(3)?,
            r.get::<_, i64>(4)?,
            (taux * 100.0).round() as i64,
            echapper(&r.get::<_, String>(6)?),
            r.get::<_, f64>(7)?,
        ))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    // BOM UTF-8 : sans lui, Excel affiche « Café » au lieu de « Café ».
    Ok(format!("\u{FEFF}{}\n{}", ENTETE, lignes.join("\n")))
}

/// Découpe une ligne CSV en respectant les guillemets.
fn decouper(ligne: &str) -> Vec<String> {
    let mut champs = Vec::new();
    let mut courant = String::new();
    let mut dans_guillemets = false;
    let mut chars = ligne.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '"' if dans_guillemets && chars.peek() == Some(&'"') => {
                courant.push('"');
                chars.next();
            }
            '"' => dans_guillemets = !dans_guillemets,
            ';' if !dans_guillemets => {
                champs.push(courant.trim().to_string());
                courant.clear();
            }
            _ => courant.push(c),
        }
    }
    champs.push(courant.trim().to_string());
    champs
}

/// « 12 500 », « 12500 », « 12.500 » -> 12500
fn parser_montant(v: &str) -> Option<i64> {
    let nettoye: String = v.chars()
        .filter(|c| c.is_ascii_digit())
        .collect();
    if nettoye.is_empty() { None } else { nettoye.parse().ok() }
}

fn parser_decimal(v: &str) -> Option<f64> {
    v.replace(',', ".").trim().parse().ok()
}

/// Importe un catalogue.
///
/// Les articles existants (même nom) sont MIS À JOUR, pas dupliqués.
/// Les catégories absentes sont créées.
#[tauri::command]
pub fn importer_articles_csv(
    etat: State<EtatApp>,
    contenu: String,
    mettre_a_jour: Option<bool>,
) -> Result<serde_json::Value, String> {
    let maj = mettre_a_jour.unwrap_or(true);
    let mut conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let now = maintenant_iso();

    let depot_defaut: String = conn.query_row(
        "SELECT id FROM depot WHERE est_defaut = 1 LIMIT 1",
        [], |r| r.get(0),
    ).map_err(|_| "Aucun dépôt par défaut".to_string())?;

    let contenu = contenu.trim_start_matches('\u{FEFF}');
    let mut lignes = contenu.lines();

    // Premiere ligne : entete si elle commence par "Nom".
    let premiere = lignes.next().unwrap_or("");
    let mut a_traiter: Vec<&str> = Vec::new();
    if !premiere.to_lowercase().starts_with("nom") {
        a_traiter.push(premiere);
    }
    a_traiter.extend(lignes);

    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let mut crees = 0;
    let mut majs = 0;
    let mut erreurs: Vec<String> = Vec::new();

    for (i, ligne) in a_traiter.iter().enumerate() {
        let num = i + 2; // +1 pour l'entete, +1 pour partir de 1
        if ligne.trim().is_empty() { continue; }

        let ch = decouper(ligne);
        if ch.len() < 4 {
            erreurs.push(format!("Ligne {} : {} colonne(s), 4 minimum \
                (Nom;Categorie;Unite;Prix)", num, ch.len()));
            continue;
        }

        let nom = ch[0].trim();
        if nom.is_empty() {
            erreurs.push(format!("Ligne {} : nom vide", num));
            continue;
        }

        let prix = match parser_montant(&ch[3]) {
            Some(p) if p > 0 => p,
            _ => {
                erreurs.push(format!(
                    "Ligne {} « {} » : prix illisible ou nul (« {} »)",
                    num, nom, ch[3]));
                continue;
            }
        };

        let categorie = ch.get(1).map(|s| s.trim()).unwrap_or("");
        let unite = {
            let u = ch.get(2).map(|s| s.trim()).unwrap_or("");
            if u.is_empty() { "unité" } else { u }
        };
        let prix_achat = ch.get(4).and_then(|v| parser_montant(v));
        let tva = ch.get(5).and_then(|v| parser_decimal(v))
            .map(|t| if t > 1.0 { t / 100.0 } else { t })
            .unwrap_or(0.0);
        let code_barre = ch.get(6).map(|s| s.trim()).filter(|s| !s.is_empty());
        let stock = ch.get(7).and_then(|v| parser_decimal(v)).unwrap_or(0.0);

        // Categorie : creee si absente.
        let cat_id: Option<String> = if categorie.is_empty() {
            None
        } else {
            let existante: Option<String> = tx.query_row(
                "SELECT id FROM categorie WHERE lower(nom) = lower(?1)",
                rusqlite::params![categorie], |r| r.get(0),
            ).ok();
            match existante {
                Some(id) => Some(id),
                None => {
                    let id = uuid::Uuid::new_v4().to_string();
                    tx.execute(
                        "INSERT INTO categorie
                         (id, nom, schema_attributs, actif, cree_le, modifie_le, origine)
                         VALUES (?1,?2,'[]',1,?3,?4,'import')",
                        rusqlite::params![id, categorie, now, now],
                    ).map_err(|e| e.to_string())?;
                    Some(id)
                }
            }
        };

        // Article existant ? On compare sur le nom, insensible a la casse.
        let existant: Option<String> = tx.query_row(
            "SELECT id FROM article WHERE lower(nom) = lower(?1)",
            rusqlite::params![nom], |r| r.get(0),
        ).ok();

        match existant {
            Some(art_id) if maj => {
                tx.execute(
                    "UPDATE article SET categorie_id = COALESCE(?1, categorie_id),
                        dernier_prix_achat = COALESCE(?2, dernier_prix_achat),
                        taux_tva_defaut = ?3, modifie_le = ?4
                     WHERE id = ?5",
                    rusqlite::params![cat_id, prix_achat, tva, now, art_id],
                ).map_err(|e| e.to_string())?;

                if let Some(cb) = code_barre {
                    // Ignorer si le code est deja pris par un autre article.
                    tx.execute(
                        "UPDATE article SET code_barre = ?1 WHERE id = ?2
                         AND NOT EXISTS (SELECT 1 FROM article
                                         WHERE code_barre = ?1 AND id <> ?2)",
                        rusqlite::params![cb, art_id],
                    ).ok();
                }

                tx.execute(
                    "UPDATE unite_vente SET prix_reference = ?1
                     WHERE article_id = ?2 AND lower(libelle) = lower(?3)",
                    rusqlite::params![prix, art_id, unite],
                ).ok();

                majs += 1;
            }
            Some(_) => { /* maj desactivee : on ignore */ }
            None => {
                let art_id = uuid::Uuid::new_v4().to_string();
                tx.execute(
                    "INSERT INTO article
                     (id, nom, categorie_id, unite_base, gere_en_stock, attributs,
                      dernier_prix_achat, code_barre, actif,
                      cree_le, modifie_le, cree_par, modifie_par, origine,
                      taux_tva_defaut)
                     VALUES (?1,?2,?3,?4,1,'{}',?5,?6,1,?7,?8,'import','import','import',?9)",
                    rusqlite::params![
                        art_id, nom, cat_id, unite, prix_achat, code_barre,
                        now, now, tva
                    ],
                ).map_err(|e| e.to_string())?;

                tx.execute(
                    "INSERT INTO unite_vente
                     (id, article_id, libelle, facteur, prix_reference, actif,
                      cree_le, modifie_le, cree_par, modifie_par, origine)
                     VALUES (?1,?2,?3,1.0,?4,1,?5,?6,'import','import','import')",
                    rusqlite::params![
                        uuid::Uuid::new_v4().to_string(), art_id, unite, prix, now, now
                    ],
                ).map_err(|e| e.to_string())?;

                tx.execute(
                    "INSERT INTO stock_depot (id, article_id, depot_id, quantite)
                     VALUES (?1,?2,?3,?4)",
                    rusqlite::params![
                        uuid::Uuid::new_v4().to_string(), art_id, depot_defaut, stock
                    ],
                ).ok();

                crees += 1;
            }
        }
    }

    tx.commit().map_err(|e| e.to_string())?;

    Ok(serde_json::json!({
        "crees": crees,
        "mis_a_jour": majs,
        "erreurs": erreurs,
        "nb_erreurs": erreurs.len(),
    }))
}

/// État du stock, pour l'impression.
#[tauri::command]
pub fn lire_etat_stock(
    etat: State<EtatApp>,
    depot_id: Option<String>,
    avec_zero: Option<bool>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    let dep: Option<String> = match depot_id {
        Some(d) if !d.is_empty() => Some(d),
        _ => None,
    };
    let zero = avec_zero.unwrap_or(false);

    let mut st = conn.prepare(
        "SELECT a.nom, COALESCE(c.nom, '—'), d.nom,
                COALESCE(uv.libelle, a.unite_base),
                sd.quantite,
                CAST(COALESCE(a.dernier_prix_achat, 0) AS INTEGER),
                CAST(COALESCE(uv.prix_reference, 0) AS INTEGER)
         FROM stock_depot sd
         JOIN article a ON a.id = sd.article_id
         JOIN depot d ON d.id = sd.depot_id
         LEFT JOIN categorie c ON c.id = a.categorie_id
         LEFT JOIN unite_vente uv ON uv.article_id = a.id AND uv.actif = 1
                                 AND uv.facteur = 1.0
         WHERE a.actif = 1 AND d.actif = 1
           AND (?1 IS NULL OR sd.depot_id = ?1)
           AND (?2 = 1 OR sd.quantite <> 0)
         GROUP BY a.id, d.id
         ORDER BY c.nom, a.nom"
    ).map_err(|e| e.to_string())?;

    let lignes: Vec<serde_json::Value> = st.query_map(
        rusqlite::params![dep, zero as i64], |r| {
            let qte: f64 = r.get(4)?;
            let pa: i64 = r.get(5)?;
            Ok(serde_json::json!({
                "article":    r.get::<_, String>(0)?,
                "categorie":  r.get::<_, String>(1)?,
                "depot":      r.get::<_, String>(2)?,
                "unite":      r.get::<_, String>(3)?,
                "quantite":   qte,
                "prix_achat": pa,
                "prix_vente": r.get::<_, i64>(6)?,
                "valeur":     (pa as f64 * qte).round() as i64,
            }))
        }
    ).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    let valeur_totale: i64 = lignes.iter()
        .filter_map(|l| l["valeur"].as_i64()).sum();

    let societe = conn.query_row(
        "SELECT nom, adresse, telephone FROM parametres_societe WHERE id = 1",
        [], |r| Ok(serde_json::json!({
            "nom":       r.get::<_, String>(0)?,
            "adresse":   r.get::<_, Option<String>>(1)?,
            "telephone": r.get::<_, Option<String>>(2)?,
        })),
    ).unwrap_or(serde_json::json!({"nom":"","adresse":null,"telephone":null}));

    Ok(serde_json::json!({
        "lignes": lignes,
        "valeur_totale": valeur_totale,
        "nb_articles": lignes.len(),
        "societe": societe,
    }))
}
