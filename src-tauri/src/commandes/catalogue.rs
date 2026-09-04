//! Import / export du catalogue en CSV.
//!
//! Format retenu : CSV point-virgule, encodage UTF-8 avec BOM. C'est ce
//! qu'Excel francophone ouvre correctement en double-cliquant — un CSV
//! virgule sans BOM lui donne une seule colonne pleine de caractères
//! abîmés, et le commerçant conclut que l'export ne marche pas.
//!
//! L'import est TOLÉRANT sur la forme et STRICT sur le fond : il
//! accepte les colonnes dans le désordre (l'en-tête les indexe par
//! nom), les espaces, les prix écrits « 12 500 » ou « 12500 » — mais
//! refuse un article sans nom ou sans prix, et signale chaque ligne
//! rejetée avec son numéro.
//!
//! Un article à plusieurs unités de vente sort sur PLUSIEURS lignes de
//! même nom, une par unité, distinguées par la colonne `Facteur`. La
//! première (facteur 1) crée l'article ; les suivantes ajoutent leurs
//! unités. Sans en-tête, l'ordre v1.3 s'applique et `Facteur` vaut 1.
//!
//! ⚠️ `Prix achat` et `Stock` appartiennent à l'ARTICLE, pas à l'unité :
//! ils sont répétés sur chaque ligne à l'export et seule la première
//! est lue à l'import. Modifier le stock sur une ligne de pack dans
//! Excel n'a donc aucun effet.

use tauri::State;
use crate::commandes::ventes::EtatApp;
use crate::utils::maintenant_iso;

// `Facteur` ajoutee en v1.4 : sans elle, un article a plusieurs unites
// sortait sur plusieurs lignes de meme nom et revenait avec toutes ses
// unites ecrasees a facteur 1. Colonne facultative a l'import — un
// fichier au format v1.3 reste lisible, facteur 1 par defaut.
const ENTETE: &str =
    "Nom;Categorie;Unite;Facteur;Prix;Prix achat;TVA %;Code barre;Stock";

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
                COALESCE(uv.facteur, 1.0),
                CAST(COALESCE(uv.prix_reference, 0) AS INTEGER),
                CAST(COALESCE(a.dernier_prix_achat, 0) AS INTEGER),
                COALESCE(a.taux_tva_defaut, 0.0),
                -- Code de l'UNITE de la ligne, avec repli sur celui de
                -- l'article pour l'unite de base (D45).
                COALESCE(uv.code_barre, a.code_barre, ''),
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
        let facteur: f64 = r.get(3)?;
        let taux: f64 = r.get(6)?;
        Ok(format!(
            "{};{};{};{};{};{};{};{};{}",
            echapper(&r.get::<_, String>(0)?),   // Nom
            echapper(&r.get::<_, String>(1)?),   // Categorie
            echapper(&r.get::<_, String>(2)?),   // Unite
            // Entier quand c'est un entier : « 12 » et non « 12.0 »,
            // qu'Excel francophone rendrait « 12,0 » puis relirait mal.
            if facteur.fract() == 0.0 {
                format!("{}", facteur as i64)
            } else {
                format!("{}", facteur)
            },
            r.get::<_, i64>(4)?,                 // Prix
            r.get::<_, i64>(5)?,                 // Prix achat
            (taux * 100.0).round() as i64,       // TVA %
            echapper(&r.get::<_, String>(7)?),   // Code barre
            r.get::<_, f64>(8)?,                 // Stock
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

/// Entete normalisee : sans accents, minuscules, sans espaces.
/// « Prix achat » et « prix_achat » designent la meme colonne.
fn normaliser_entete(v: &str) -> String {
    v.trim().to_lowercase()
        .replace(['é', 'è', 'ê'], "e")
        .replace('à', "a")
        .replace([' ', '_', '%'], "")
}

/// Valeur d'une colonne : par nom si l'entete existe, par position
/// sinon (ordre historique v1.3, sans `Facteur`).
fn champ<'a>(
    ch: &'a [String],
    cols: &std::collections::HashMap<String, usize>,
    nom: &str,
    position_v13: Option<usize>,
) -> Option<&'a str> {
    let i = match cols.get(nom) {
        Some(i) => *i,
        None => position_v13?,
    };
    ch.get(i).map(|s| s.trim())
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

    // Premiere ligne : entete si elle commence par "Nom". On s'en sert
    // pour INDEXER les colonnes par nom, et non par position — c'est ce
    // qui rend l'import tolerant au desordre (promesse du module) et
    // permet a `Facteur`, ajoutee en v1.4, d'etre absente d'un ancien
    // fichier sans decaler tout le reste.
    let premiere = lignes.next().unwrap_or("");
    let a_entete = premiere.to_lowercase().starts_with("nom");

    let cols: std::collections::HashMap<String, usize> = if a_entete {
        decouper(premiere).iter().enumerate()
            .map(|(i, c)| (normaliser_entete(c), i))
            .collect()
    } else {
        std::collections::HashMap::new()
    };

    let mut a_traiter: Vec<&str> = Vec::new();
    if !a_entete {
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
                (Nom;Categorie;Unite;Prix). Colonne Facteur facultative.",
                num, ch.len()));
            continue;
        }

        let nom = champ(&ch, &cols, "nom", Some(0)).unwrap_or("");
        if nom.is_empty() {
            erreurs.push(format!("Ligne {} : nom vide", num));
            continue;
        }

        let brut_prix = champ(&ch, &cols, "prix", Some(3)).unwrap_or("");
        let prix = match parser_montant(brut_prix) {
            Some(p) if p > 0 => p,
            _ => {
                erreurs.push(format!(
                    "Ligne {} « {} » : prix illisible ou nul (« {} »)",
                    num, nom, brut_prix));
                continue;
            }
        };

        let categorie = champ(&ch, &cols, "categorie", Some(1)).unwrap_or("");
        let unite = {
            let u = champ(&ch, &cols, "unite", Some(2)).unwrap_or("");
            if u.is_empty() { "unité" } else { u }
        };

        // Facteur absent (fichier v1.3) = 1.0 : l'unite est la base.
        let facteur = champ(&ch, &cols, "facteur", None)
            .and_then(parser_decimal)
            .filter(|f| *f > 0.0)
            .unwrap_or(1.0);

        let prix_achat = champ(&ch, &cols, "prixachat", Some(4))
            .and_then(parser_montant);
        let tva = champ(&ch, &cols, "tva", Some(5))
            .and_then(parser_decimal)
            .map(|t| if t > 1.0 { t / 100.0 } else { t })
            .unwrap_or(0.0);
        let code_barre = champ(&ch, &cols, "codebarre", Some(6))
            .filter(|s| !s.is_empty());
        let stock = champ(&ch, &cols, "stock", Some(7))
            .and_then(parser_decimal).unwrap_or(0.0);

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

                // Une unite absente est CREEE, pas ignoree. Sans ca, un
                // export puis reimport perdait tous les packs en
                // silence : l'article a plusieurs unites sort sur
                // plusieurs lignes de meme nom, et seule la premiere
                // trouvait sa cible.
                let touchees = tx.execute(
                    "UPDATE unite_vente
                     SET prix_reference = ?1, facteur = ?2,
                         code_barre = COALESCE(?3, code_barre), modifie_le = ?4
                     WHERE article_id = ?5 AND lower(libelle) = lower(?6)",
                    rusqlite::params![prix, facteur, code_barre, now, art_id, unite],
                ).unwrap_or(0);

                if touchees == 0 {
                    tx.execute(
                        "INSERT INTO unite_vente
                         (id, article_id, libelle, facteur, prix_reference,
                          code_barre, actif,
                          cree_le, modifie_le, cree_par, modifie_par, origine)
                         VALUES (?1,?2,?3,?4,?5,?6,1,?7,?8,'import','import','import')",
                        rusqlite::params![
                            uuid::Uuid::new_v4().to_string(), art_id,
                            unite, facteur, prix, code_barre, now, now
                        ],
                    ).map_err(|e| e.to_string())?;
                }

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
                     VALUES (?1,?2,?3,?4,?5,1,?6,?7,'import','import','import')",
                    rusqlite::params![
                        uuid::Uuid::new_v4().to_string(), art_id, unite,
                        facteur, prix, now, now
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
