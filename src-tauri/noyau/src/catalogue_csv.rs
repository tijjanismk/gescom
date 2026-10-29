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
//! Attention : `Prix achat` et `Stock` appartiennent à l'ARTICLE, pas à l'unité :
//! ils sont répétés sur chaque ligne à l'export et seule la première
//! est lue à l'import. Modifier le stock sur une ligne de pack dans
//! Excel n'a donc aucun effet.

use crate::utils::maintenant_iso;

// `Facteur` ajoutee en v1.4 : sans elle, un article a plusieurs unites
// sortait sur plusieurs lignes de meme nom et revenait avec toutes ses
// unites ecrasees a facteur 1. Colonne facultative a l'import — un
// fichier au format v1.3 reste lisible, facteur 1 par defaut.
//
// `Suivre le stock` ajoutee au lot D.11 (D36), EN DERNIER : un fichier
// sans en-tete garde l'ordre v1.3. Facultative a l'import : absente ou
// vide, un article cree suit son stock et un article existant garde son
// reglage.
//
// `Seuil` ajoutee par D45, EN DERNIER, lue par son nom seulement : vide,
// l'article prend (ou garde) le seuil du dossier.
const ENTETE: &str =
    "Nom;Categorie;Unite;Facteur;Prix;Prix achat;TVA %;Code barre;Stock;Suivre le stock;Seuil";

/// Ce que la colonne « Seuil » ecrit : vide quand l'article suit le
/// seuil du dossier, entier quand c'est un entier.
fn seuil_ecrit(seuil: Option<f64>) -> String {
    match seuil {
        None => String::new(),
        Some(s) if s.fract() == 0.0 => format!("{}", s as i64),
        Some(s) => format!("{s}"),
    }
}

/// Ce que la colonne « Suivre le stock » ecrit a l'export.
fn suivi_ecrit(gere_en_stock: i64) -> &'static str {
    if gere_en_stock != 0 { "oui" } else { "non" }
}

/// La colonne « Suivre le stock », par son nom seulement (elle n'existe
/// pas dans l'ordre v1.3). « Suivre le stock », « Suivre stock » et
/// « Gere en stock » designent la meme.
fn champ_suivi<'a>(ch: &'a [String], cols: &std::collections::HashMap<String, usize>) -> Option<&'a str> {
    ["suivrelestock", "suivrestock", "gereenstock"]
        .iter()
        .find_map(|nom| cols.get(*nom))
        .and_then(|i| ch.get(*i))
        .map(|s| s.trim())
}

/// La valeur de la colonne lue pour une ligne, ou le refus de la ligne
/// (D36 : une valeur illisible n'est pas devinee).
fn suivi_de_la_ligne(
    ch: &[String],
    cols: &std::collections::HashMap<String, usize>,
    num: usize,
    nom: &str,
) -> Result<Option<bool>, String> {
    match champ_suivi(ch, cols) {
        None => Ok(None),
        Some(v) => crate::coeur::stock::lire_suivi_csv(v).map_err(|e| format!("Ligne {} « {} » : {}", num, nom, e)),
    }
}

/// Le refus d'une ligne qui changerait le suivi d'un article existant :
/// cesser ou reprendre passe par Parametres → Articles (D36 : la
/// permission reservee, le comptage de chaque magasin pour reprendre, la
/// trace). L'import ne fait pas semblant de le faire.
fn refus_changement_suivi(num: usize, nom: &str, suivi_actuel: bool) -> String {
    format!(
        "Ligne {} « {} » : l'article existe et {} son stock ; l'import ne change pas ce réglage \
         (Paramètres → Articles, « Suivre le stock »). Ligne ignorée.",
        num,
        nom,
        if suivi_actuel { "suit" } else { "ne suit pas" }
    )
}

/// Le refus de creer un article non suivi sans le droit de regler le
/// suivi (D36 : `parametres:modifier`).
fn refus_creation_non_suivie(num: usize, nom: &str) -> String {
    format!(
        "Ligne {} « {} » : créer un article qui ne suit pas son stock demande le droit de \
         modifier les paramètres. Ligne ignorée.",
        num, nom
    )
}

pub fn echapper(v: &str) -> String {
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
pub fn exporter_articles_csv(
    conn: &rusqlite::Connection,
) -> Result<String, String> {

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
                          WHERE sd.article_id = a.id), 0),
                a.gere_en_stock,
                a.seuil_alerte
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
            "{};{};{};{};{};{};{};{};{};{};{}",
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
            suivi_ecrit(r.get::<_, i64>(9)?),    // Suivre le stock (D36)
            seuil_ecrit(r.get::<_, Option<f64>>(10)?), // Seuil (D45)
        ))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    // BOM UTF-8 : sans lui, Excel affiche « Café » au lieu de « Café ».
    Ok(format!("\u{FEFF}{}\n{}", ENTETE, lignes.join("\n")))
}

/// Découpe une ligne CSV en respectant les guillemets.
pub fn decouper(ligne: &str) -> Vec<String> {
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
pub fn normaliser_entete(v: &str) -> String {
    v.trim().to_lowercase()
        .replace(['é', 'è', 'ê'], "e")
        .replace('à', "a")
        .replace([' ', '_', '%'], "")
}

/// Valeur d'une colonne : par nom si l'entete existe, par position
/// sinon (ordre historique v1.3, sans `Facteur`).
pub(crate) fn champ<'a>(
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
pub fn parser_montant(v: &str) -> Option<i64> {
    let nettoye: String = v.chars()
        .filter(|c| c.is_ascii_digit())
        .collect();
    if nettoye.is_empty() { None } else { nettoye.parse().ok() }
}

pub fn parser_decimal(v: &str) -> Option<f64> {
    v.replace(',', ".").trim().parse().ok()
}

/// Importe un catalogue.
///
/// Les articles existants (même nom) sont MIS À JOUR, pas dupliqués.
/// Les catégories absentes sont créées.
pub fn importer_articles_csv(
    conn: &mut rusqlite::Connection,
    contenu: String,
    mettre_a_jour: Option<bool>,
) -> Result<serde_json::Value, String> {
    let maj = mettre_a_jour.unwrap_or(true);
    let now = maintenant_iso();

    let depot_defaut: String = conn.query_row(
        "SELECT id FROM depot WHERE est_defaut = 1 LIMIT 1",
        [], |r| r.get(0),
    ).map_err(|_| "Aucun magasin par défaut".to_string())?;

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

        // D36, lot D.11 : « Suivre le stock », jugee AVANT toute ecriture
        // de la ligne (categorie comprise).
        let suivi = match suivi_de_la_ligne(&ch, &cols, num, nom) {
            Ok(s) => s,
            Err(e) => { erreurs.push(e); continue; }
        };

        // Article existant ? On compare sur le nom, insensible a la casse.
        let existant: Option<(String, i64)> = tx.query_row(
            "SELECT id, gere_en_stock FROM article WHERE lower(nom) = lower(?1)",
            rusqlite::params![nom], |r| Ok((r.get(0)?, r.get(1)?)),
        ).ok();
        match (&existant, suivi) {
            (Some((_, actuel)), Some(voulu)) if maj && (*actuel != 0) != voulu => {
                erreurs.push(refus_changement_suivi(num, nom, *actuel != 0));
                continue;
            }
            // La fenetre monoposte n'a pas de droits par commande : le
            // patron y est seul (le serveur, lui, exige
            // `parametres:modifier`, voir la version `Base`).
            _ => {}
        }
        let existant = existant.map(|(id, _)| id);

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
                // D36 : suivi par defaut ; « non » le cree hors des
                // ecrans de stock, et l'Historique le dit.
                let suivre = suivi.unwrap_or(true);
                tx.execute(
                    "INSERT INTO article
                     (id, nom, categorie_id, unite_base, gere_en_stock, attributs,
                      dernier_prix_achat, code_barre, actif,
                      cree_le, modifie_le, cree_par, modifie_par, origine,
                      taux_tva_defaut)
                     VALUES (?1,?2,?3,?4,?10,'{}',?5,?6,1,?7,?8,'import','import','import',?9)",
                    rusqlite::params![
                        art_id, nom, cat_id, unite, prix_achat, code_barre,
                        now, now, tva, suivre as i64
                    ],
                ).map_err(|e| e.to_string())?;
                if !suivre {
                    tx.execute(
                        "INSERT INTO journal
                         (id, type_evenement, entite_type, entite_id, auteur_id,
                          ancien_valeur, nouveau_valeur, origine, date_evenement)
                         VALUES (?1,'suivi_stock_change','article',?2,?3,?4,?5,'import',?6)",
                        rusqlite::params![
                            uuid::Uuid::new_v4().to_string(), art_id,
                            crate::argent::id_utilisateur_courant_pub(&tx),
                            serde_json::json!({ "nom": nom }).to_string(),
                            serde_json::json!({ "suivi": false, "cree_par_import": true }).to_string(),
                            now
                        ],
                    ).map_err(|e| e.to_string())?;
                }

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

                // Le stock d'un article importe est une ENTREE, pas un
                // chiffre pose. Avant, l'import ecrivait le compteur sans
                // mouvement : la marchandise apparaissait dans le stock
                // sans figurer dans aucun historique, et la somme des
                // mouvements ne valait plus le compteur des le premier
                // import. Le declencheur met le compteur a jour.
                if stock != 0.0 {
                    tx.execute(
                        "INSERT INTO mouvement_stock
                         (id, article_id, depot_id, type_mouvement, quantite_delta,
                          motif, auteur_id, date_mouvement, cree_le, cree_par, origine)
                         VALUES (?1,?2,?3,'entree',?4,?5,'import',?6,?6,'import','import')",
                        rusqlite::params![
                            uuid::Uuid::new_v4().to_string(), art_id, depot_defaut,
                            stock, "Import CSV du catalogue", now
                        ],
                    ).ok();
                }

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
pub fn lire_etat_stock(
    conn: &rusqlite::Connection,
    depot_id: Option<String>,
    avec_zero: Option<bool>,
) -> Result<serde_json::Value, String> {
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

// =====================================================================
//  SUR L'UN OU L'AUTRE MOTEUR
// =====================================================================
//
// `article`, `unite_vente`, `categorie` ne sont pas cloisonnes ;
// `stock_depot`, `depot`, `mouvement_stock` le sont. Les `GROUP BY`
// sans agregat de la version SQLite sont retires : PostgreSQL exige
// que chaque colonne lue soit groupee, et ils ne servaient a rien.

use crate::base::Base;
use crate::parametres;

pub fn exporter_articles_csv_sur_base(base: &mut Base) -> Result<String, String> {
    let dossier = base.dossier().to_string();
    let lignes: Vec<String> = base
        .lire_plusieurs(
            "SELECT a.nom, COALESCE(c.nom, ''), COALESCE(uv.libelle, a.unite_base),
                    COALESCE(uv.facteur, 1.0),
                    CAST(COALESCE(uv.prix_reference, 0) AS BIGINT),
                    CAST(COALESCE(a.dernier_prix_achat, 0) AS BIGINT),
                    COALESCE(a.taux_tva_defaut, 0.0),
                    COALESCE(uv.code_barre, a.code_barre, ''),
                    CAST(COALESCE((SELECT SUM(sd.quantite) FROM stock_depot sd
                                   WHERE sd.article_id = a.id AND sd.dossier_id = ?1), 0) AS DOUBLE PRECISION),
                    a.gere_en_stock,
                    a.seuil_alerte
             FROM article a
             LEFT JOIN categorie c ON c.id = a.categorie_id
             LEFT JOIN unite_vente uv ON uv.article_id = a.id AND uv.actif = 1
             WHERE a.actif = 1
             ORDER BY a.nom, uv.facteur",
            &parametres![dossier],
            |r| {
                let facteur: f64 = r.get::<f64>(3)?;
                let taux: f64 = r.get::<f64>(6)?;
                Ok(format!(
                    "{};{};{};{};{};{};{};{};{};{};{}",
                    echapper(&r.get::<String>(0)?),
                    echapper(&r.get::<String>(1)?),
                    echapper(&r.get::<String>(2)?),
                    if facteur.fract() == 0.0 { format!("{}", facteur as i64) } else { format!("{}", facteur) },
                    r.get::<i64>(4)?,
                    r.get::<i64>(5)?,
                    (taux * 100.0).round() as i64,
                    echapper(&r.get::<String>(7)?),
                    r.get::<f64>(8)?,
                    // D36 : « Suivre le stock », relue a l'import.
                    suivi_ecrit(r.get::<i64>(9)?),
                    // D45 : vide = le seuil du dossier.
                    seuil_ecrit(r.get::<Option<f64>>(10)?),
                ))
            },
        )
        .map_err(|e| e.0)?;
    Ok(format!("\u{FEFF}{}\n{}", ENTETE, lignes.join("\n")))
}

/// Un article deja en base, tel que l'import le rapproche (D44).
struct Connu {
    id: String,
    nom: String,
    gere_en_stock: bool,
    actif: bool,
}

/// L'article du meme nom (casse et espaces en trop ignores, D44), actif
/// ou non. Les articles sont communs a tous les dossiers : la recherche
/// porte sur toute la base. Charge les noms et compare en Rust — `LOWER`
/// ne baisse pas les accents sur SQLite, et ne ramene pas les espaces.
pub fn article_du_meme_nom_sur(
    acces: &mut impl crate::base::Acces,
    nom: &str,
) -> Result<Option<(String, String, bool)>, String> {
    let cible = crate::coeur::doublons::normaliser_nom(nom);
    let tous: Vec<(String, String, i64)> = acces
        .lire_plusieurs("SELECT id, nom, actif FROM article ORDER BY actif DESC, cree_le", &[], |r| {
            Ok((r.get::<String>(0)?, r.get::<String>(1)?, r.get::<i64>(2)?))
        })
        .map_err(|e| e.0)?;
    Ok(tous
        .into_iter()
        .find(|(_, n, _)| crate::coeur::doublons::normaliser_nom(n) == cible)
        .map(|(id, n, actif)| (id, n, actif != 0)))
}

/// Le refus d'un article en double, a la creation (comptoir, Achats,
/// Parametres) : le meme texte partout.
pub fn refuser_doublon_article_sur(acces: &mut impl crate::base::Acces, nom: &str) -> Result<(), String> {
    match article_du_meme_nom_sur(acces, nom)? {
        Some((_, existant, true)) => Err(format!("L'article « {existant} » existe déjà.")),
        Some((_, existant, false)) => Err(format!(
            "L'article « {existant} » existe déjà, désactivé : pas de second article du même nom."
        )),
        None => Ok(()),
    }
}

/// `peut_ne_pas_suivre` : l'appelant a-t-il le droit de regler le suivi
/// du stock (`parametres:modifier`, D36) ? Sans lui, une ligne « Suivre
/// le stock : non » qui creerait un article est refusee — le serveur le
/// juge, l'ecran ne protege rien (D27).
///
/// `simuler` (D44) : l'apercu. Tout se joue dans la transaction, puis
/// elle retombe sans etre validee — l'apercu dit exactement ce que
/// l'import fera, parce que c'est le meme code.
///
/// Les doublons (D44) : un article se reconnait a son nom, sans la casse
/// ni les espaces en trop. Deux lignes du fichier pour le meme article et
/// la meme unite : la seconde est refusee. Un code-barre deja porte par
/// un autre article : la ligne qui creerait l'article est refusee, une
/// mise a jour garde l'ancien code et le dit. Avant D44, ce code-barre
/// faisait echouer tout l'import sur une erreur SQL.
pub fn importer_articles_csv_sur_base(
    base: &mut Base,
    contenu: String,
    mettre_a_jour: Option<bool>,
    peut_ne_pas_suivre: bool,
    simuler: bool,
) -> Result<serde_json::Value, String> {
    use crate::coeur::doublons::normaliser_nom;
    use std::collections::{HashMap, HashSet};

    let dossier = base.dossier().to_string();
    let maj = mettre_a_jour.unwrap_or(true);
    let now = maintenant_iso();
    let auteur = crate::argent::id_utilisateur_courant_sur(base);
    let tva_du_dossier = crate::assistant::taux_nouvel_article_sur(base);
    let depot_defaut: String = base
        .lire_une(
            "SELECT id FROM depot WHERE est_defaut = 1 AND dossier_id = ?1 LIMIT 1",
            &parametres![dossier.clone()],
            |r| r.get::<String>(0),
        )
        .map_err(|e| e.0)?
        .ok_or_else(|| "Aucun magasin par défaut".to_string())?;

    // Ce qui existe : par nom mis en forme, et chaque code-barre a qui
    // il est (article ou unite de vente).
    let mut connus: HashMap<String, Connu> = HashMap::new();
    for (id, nom, gere, actif) in base
        .lire_plusieurs("SELECT id, nom, gere_en_stock, actif FROM article ORDER BY actif ASC, cree_le DESC", &[], |r| {
            Ok((r.get::<String>(0)?, r.get::<String>(1)?, r.get::<i64>(2)?, r.get::<i64>(3)?))
        })
        .map_err(|e| e.0)?
    {
        // Le plus ancien actif l'emporte (ordre : les inactifs d'abord,
        // ecrases par les actifs ; les recents d'abord, ecrases par les
        // anciens).
        connus.insert(normaliser_nom(&nom), Connu { id, nom, gere_en_stock: gere != 0, actif: actif != 0 });
    }
    let mut codes: HashMap<String, String> = HashMap::new();
    for (code, article_id) in base
        .lire_plusieurs(
            "SELECT code_barre, id FROM article WHERE code_barre IS NOT NULL
             UNION ALL
             SELECT code_barre, article_id FROM unite_vente WHERE code_barre IS NOT NULL",
            &[],
            |r| Ok((r.get::<String>(0)?, r.get::<String>(1)?)),
        )
        .map_err(|e| e.0)?
    {
        codes.entry(code).or_insert(article_id);
    }

    let contenu = contenu.trim_start_matches('\u{FEFF}');
    let mut lignes = contenu.lines();
    let premiere = lignes.next().unwrap_or("");
    let a_entete = premiere.to_lowercase().starts_with("nom");
    let cols: HashMap<String, usize> = if a_entete {
        decouper(premiere).iter().enumerate().map(|(i, c)| (normaliser_entete(c), i)).collect()
    } else {
        HashMap::new()
    };
    let mut a_traiter: Vec<&str> = Vec::new();
    if !a_entete {
        a_traiter.push(premiere);
    }
    a_traiter.extend(lignes);

    let mut tx = base.transaction().map_err(|e| e.0)?;
    let mut crees = 0;
    let mut majs = 0;
    let mut ignores: Vec<String> = Vec::new();
    let mut erreurs: Vec<String> = Vec::new();
    let mut avertissements: Vec<String> = Vec::new();
    // (nom mis en forme, unite en minuscules) -> la ligne qui l'a donne.
    let mut vus: HashMap<(String, String), usize> = HashMap::new();
    // Les articles crees par CE fichier : leurs lignes suivantes ajoutent
    // leurs unites, meme sans « mettre a jour ».
    let mut crees_ici: HashSet<String> = HashSet::new();

    for (i, ligne) in a_traiter.iter().enumerate() {
        let num = if a_entete { i + 2 } else { i + 1 };
        if ligne.trim().is_empty() {
            continue;
        }
        let ch = decouper(ligne);
        if ch.len() < 4 {
            erreurs.push(format!(
                "Ligne {} : {} colonne(s), 4 minimum (Nom;Categorie;Unite;Prix). Colonne Facteur facultative.",
                num, ch.len()
            ));
            continue;
        }
        let nom = champ(&ch, &cols, "nom", Some(0)).unwrap_or("");
        if nom.is_empty() {
            erreurs.push(format!("Ligne {} : nom vide", num));
            continue;
        }
        let nom = nom.split_whitespace().collect::<Vec<_>>().join(" ");
        let nom = nom.as_str();
        let brut_prix = champ(&ch, &cols, "prix", Some(3)).unwrap_or("");
        let prix = match parser_montant(brut_prix) {
            Some(p) if p > 0 => p,
            _ => {
                erreurs.push(format!("Ligne {} « {} » : prix illisible ou nul (« {} »)", num, nom, brut_prix));
                continue;
            }
        };
        let categorie = champ(&ch, &cols, "categorie", Some(1)).unwrap_or("");
        let unite = {
            let u = champ(&ch, &cols, "unite", Some(2)).unwrap_or("");
            if u.is_empty() { "unité" } else { u }
        };
        let facteur = champ(&ch, &cols, "facteur", None).and_then(parser_decimal).filter(|f| *f > 0.0).unwrap_or(1.0);
        let prix_achat = champ(&ch, &cols, "prixachat", Some(4)).and_then(parser_montant);
        // Colonne vide ou absente : `None` — un article cree prend le taux
        // du dossier (D43), un article existant garde le sien.
        let tva = champ(&ch, &cols, "tva", Some(5))
            .filter(|s| !s.is_empty())
            .and_then(parser_decimal)
            .map(|t| if t > 1.0 { t / 100.0 } else { t });
        let code_barre = champ(&ch, &cols, "codebarre", Some(6)).filter(|s| !s.is_empty()).map(|s| s.to_string());
        let stock = champ(&ch, &cols, "stock", Some(7)).and_then(parser_decimal).unwrap_or(0.0);
        // D45 : « Seuil », par son nom seulement ; vide = celui du
        // dossier (ou celui que l'article a deja). Illisible : refusee.
        let seuil = match champ(&ch, &cols, "seuil", None).filter(|s| !s.is_empty()) {
            None => None,
            Some(brut) => match parser_decimal(brut).ok_or_else(|| "illisible".to_string())
                .and_then(|s| crate::coeur::stock::verifier_seuil(s).map(|_| s))
            {
                Ok(s) => Some(s),
                Err(e) => {
                    erreurs.push(format!("Ligne {} « {} » : seuil « {} » refusé ({}).", num, nom, brut, e));
                    continue;
                }
            },
        };

        // D36, lot D.11 : « Suivre le stock », jugee AVANT toute ecriture
        // de la ligne (categorie comprise).
        let suivi = match suivi_de_la_ligne(&ch, &cols, num, nom) {
            Ok(s) => s,
            Err(e) => {
                erreurs.push(e);
                continue;
            }
        };

        // D44 : le meme article et la meme unite deux fois dans le fichier.
        let cle_ligne = (normaliser_nom(nom), unite.to_lowercase());
        if let Some(premiere) = vus.get(&cle_ligne) {
            erreurs.push(format!(
                "Ligne {} « {} » : même article et même unité que la ligne {} — doublon dans le fichier, ligne ignorée.",
                num, nom, premiere
            ));
            continue;
        }
        vus.insert(cle_ligne, num);

        let existant = connus.get(&normaliser_nom(nom)).map(|c| (c.id.clone(), c.nom.clone(), c.gere_en_stock, c.actif));
        if let Some((_, nom_connu, _, false)) = &existant {
            erreurs.push(format!(
                "Ligne {} « {} » : l'article « {} » existe, désactivé — l'import ne le reprend pas. Ligne ignorée.",
                num, nom, nom_connu
            ));
            continue;
        }
        let cree_ici = existant.as_ref().is_some_and(|(id, ..)| crees_ici.contains(id));
        match (&existant, suivi) {
            // Un article existant ne change pas de suivi par l'import.
            (Some((_, _, actuel, _)), Some(voulu)) if maj && !cree_ici && *actuel != voulu => {
                erreurs.push(refus_changement_suivi(num, nom, *actuel));
                continue;
            }
            // Le creer non suivi : le droit de regler le suivi.
            (None, Some(false)) if !peut_ne_pas_suivre => {
                erreurs.push(refus_creation_non_suivie(num, nom));
                continue;
            }
            _ => {}
        }

        // D44 : un code-barre deja porte par un AUTRE article.
        let code_pris = code_barre.as_ref().and_then(|cb| {
            codes.get(cb).filter(|proprio| existant.as_ref().is_none_or(|(id, ..)| *proprio != id)).cloned()
        });
        if existant.is_none() {
            if let (Some(cb), Some(_)) = (&code_barre, &code_pris) {
                erreurs.push(format!(
                    "Ligne {} « {} » : le code-barre {} est déjà celui d'un autre article. Ligne ignorée.",
                    num, nom, cb
                ));
                continue;
            }
        }
        let code_barre = if code_pris.is_some() {
            avertissements.push(format!(
                "Ligne {} « {} » : le code-barre {} est déjà celui d'un autre article ; l'article garde le sien.",
                num, nom, code_barre.as_deref().unwrap_or("")
            ));
            None
        } else {
            // Deja le sien (une autre de ses unites) : rien a poser.
            code_barre.filter(|cb| !codes.contains_key(cb))
        };

        if let Some((_, nom_connu, ..)) = &existant {
            if !maj && !cree_ici {
                ignores.push(format!("Ligne {} « {} » : existe déjà (« {} »), laissé tel quel.", num, nom, nom_connu));
                continue;
            }
        }
        // Categorie : creee si absente.
        let cat_id: Option<String> = if categorie.is_empty() {
            None
        } else {
            let existante: Option<String> = tx
                .lire_une(
                    "SELECT id FROM categorie WHERE LOWER(nom) = LOWER(?1)",
                    &parametres![categorie],
                    |r| r.get::<String>(0),
                )
                .map_err(|e| e.0)?;
            match existante {
                Some(id) => Some(id),
                None => {
                    let id = uuid::Uuid::new_v4().to_string();
                    tx.executer(
                        "INSERT INTO categorie (id, nom, schema_attributs, actif, cree_le, modifie_le, origine)
                         VALUES (?1,?2,'[]',1,?3,?3,'import')",
                        &parametres![id.clone(), categorie, now.clone()],
                    )
                    .map_err(|e| e.0)?;
                    Some(id)
                }
            }
        };

        match existant {
            Some((art_id, ..)) => {
                tx.executer(
                    "UPDATE article SET categorie_id = COALESCE(CAST(?1 AS TEXT), categorie_id),
                        dernier_prix_achat = COALESCE(CAST(?2 AS BIGINT), dernier_prix_achat),
                        taux_tva_defaut = COALESCE(CAST(?3 AS DOUBLE PRECISION), taux_tva_defaut),
                        seuil_alerte = COALESCE(CAST(?6 AS DOUBLE PRECISION), seuil_alerte),
                        modifie_le = ?4
                     WHERE id = ?5",
                    &parametres![cat_id, prix_achat, tva, now.clone(), art_id.clone(), seuil],
                )
                .map_err(|e| e.0)?;
                if let Some(ref cb) = code_barre {
                    tx.executer(
                        "UPDATE article SET code_barre = ?1 WHERE id = ?2 AND code_barre IS NULL",
                        &parametres![cb.clone(), art_id.clone()],
                    )
                    .map_err(|e| e.0)?;
                    codes.insert(cb.clone(), art_id.clone());
                }
                // Une unite absente est CREEE, pas ignoree.
                let touchees = tx
                    .executer(
                        "UPDATE unite_vente
                         SET prix_reference = ?1, facteur = ?2,
                             code_barre = COALESCE(CAST(?3 AS TEXT), code_barre), modifie_le = ?4
                         WHERE article_id = ?5 AND LOWER(libelle) = LOWER(?6)",
                        &parametres![prix, facteur, code_barre.clone(), now.clone(), art_id.clone(), unite],
                    )
                    .map_err(|e| e.0)?;
                if touchees == 0 {
                    tx.executer(
                        "INSERT INTO unite_vente
                         (id, article_id, libelle, facteur, prix_reference, code_barre, actif,
                          cree_le, modifie_le, cree_par, modifie_par, origine)
                         VALUES (?1,?2,?3,?4,?5,CAST(?6 AS TEXT),1,?7,?7,'import','import','import')",
                        &parametres![uuid::Uuid::new_v4().to_string(), art_id, unite, facteur, prix, code_barre, now.clone()],
                    )
                    .map_err(|e| e.0)?;
                }
                if !cree_ici {
                    majs += 1;
                }
            }
            None => {
                let art_id = uuid::Uuid::new_v4().to_string();
                // D36 : suivi par defaut ; « non » le cree hors des
                // ecrans de stock, et l'Historique le dit.
                let suivre = suivi.unwrap_or(true);
                tx.executer(
                    "INSERT INTO article
                     (id, nom, categorie_id, unite_base, gere_en_stock, attributs,
                      dernier_prix_achat, code_barre, actif,
                      cree_le, modifie_le, cree_par, modifie_par, origine, taux_tva_defaut, seuil_alerte)
                     VALUES (?1,?2,CAST(?3 AS TEXT),?4,?9,'{}',CAST(?5 AS BIGINT),CAST(?6 AS TEXT),1,?7,?7,'import','import','import',?8,CAST(?10 AS DOUBLE PRECISION))",
                    &parametres![
                        art_id.clone(), nom, cat_id, unite, prix_achat, code_barre.clone(), now.clone(),
                        tva.unwrap_or(tva_du_dossier), suivre as i64, seuil
                    ],
                )
                .map_err(|e| e.0)?;
                if let Some(cb) = &code_barre {
                    codes.insert(cb.clone(), art_id.clone());
                }
                if !suivre {
                    tx.executer(
                        "INSERT INTO journal
                           (id, type_evenement, entite_type, entite_id, auteur_id,
                            ancien_valeur, nouveau_valeur, origine, date_evenement, dossier_id)
                         VALUES (?1,'suivi_stock_change','article',?2,?3,?4,?5,'import',?6,?7)",
                        &parametres![
                            uuid::Uuid::new_v4().to_string(),
                            art_id.clone(),
                            auteur.clone(),
                            serde_json::json!({ "nom": nom }).to_string(),
                            serde_json::json!({ "suivi": false, "cree_par_import": true }).to_string(),
                            now.clone(),
                            dossier.clone()
                        ],
                    )
                    .map_err(|e| e.0)?;
                }
                tx.executer(
                    "INSERT INTO unite_vente
                     (id, article_id, libelle, facteur, prix_reference, actif,
                      cree_le, modifie_le, cree_par, modifie_par, origine)
                     VALUES (?1,?2,?3,?4,?5,1,?6,?6,'import','import','import')",
                    &parametres![uuid::Uuid::new_v4().to_string(), art_id.clone(), unite, facteur, prix, now.clone()],
                )
                .map_err(|e| e.0)?;
                // Le stock importe est une ENTREE, tracee ; le
                // declencheur tient le compteur.
                if stock != 0.0 {
                    tx.executer(
                        "INSERT INTO mouvement_stock
                         (id, article_id, depot_id, type_mouvement, quantite_delta,
                          motif, auteur_id, date_mouvement, cree_le, cree_par, origine, dossier_id)
                         VALUES (?1,?2,?3,'entree',?4,?5,'import',?6,?6,'import','import',?7)",
                        &parametres![
                            uuid::Uuid::new_v4().to_string(), art_id.clone(), depot_defaut.clone(), stock,
                            "Import CSV du catalogue", now.clone(), dossier.clone()
                        ],
                    )
                    .map_err(|e| e.0)?;
                }
                connus.insert(
                    normaliser_nom(nom),
                    Connu { id: art_id.clone(), nom: nom.to_string(), gere_en_stock: suivre, actif: true },
                );
                crees_ici.insert(art_id);
                crees += 1;
            }
        }
    }

    let bilan = serde_json::json!({
        "crees": crees,
        "mis_a_jour": majs,
        "ignores": ignores,
        "nb_ignores": ignores.len(),
        "erreurs": erreurs,
        "nb_erreurs": erreurs.len(),
        "avertissements": avertissements,
        "simule": simuler,
    });
    if simuler {
        // La transaction retombe : rien n'est ecrit.
        drop(tx);
        return Ok(bilan);
    }
    if crees + majs > 0 {
        tx.executer(
            "INSERT INTO journal
               (id, type_evenement, entite_type, entite_id, auteur_id,
                nouveau_valeur, origine, date_evenement, dossier_id)
             VALUES (?1,'import_articles','article','import',?2,?3,'import',?4,?5)",
            &parametres![
                uuid::Uuid::new_v4().to_string(),
                auteur,
                serde_json::json!({ "crees": crees, "mis_a_jour": majs, "ignores": ignores.len(), "refuses": erreurs.len() })
                    .to_string(),
                now,
                dossier
            ],
        )
        .map_err(|e| e.0)?;
    }
    tx.valider().map_err(|e| e.0)?;
    Ok(bilan)
}

pub fn lire_etat_stock_sur_base(
    base: &mut Base,
    depot_id: Option<String>,
    avec_zero: Option<bool>,
) -> Result<serde_json::Value, String> {
    let dossier = base.dossier().to_string();
    let dep: Option<String> = depot_id.filter(|d| !d.is_empty());
    let zero = avec_zero.unwrap_or(false) as i64;
    let lignes: Vec<serde_json::Value> = base
        .lire_plusieurs(
            "SELECT a.nom, COALESCE(c.nom, '—'), d.nom,
                    COALESCE(uv.libelle, a.unite_base),
                    sd.quantite,
                    CAST(COALESCE(a.dernier_prix_achat, 0) AS BIGINT),
                    CAST(COALESCE(uv.prix_reference, 0) AS BIGINT)
             FROM stock_depot sd
             JOIN article a ON a.id = sd.article_id
             JOIN depot d ON d.id = sd.depot_id
             LEFT JOIN categorie c ON c.id = a.categorie_id
             LEFT JOIN unite_vente uv ON uv.article_id = a.id AND uv.actif = 1 AND uv.facteur = 1.0
             WHERE a.actif = 1 AND d.actif = 1 AND sd.dossier_id = ?3
               AND (CAST(?1 AS TEXT) IS NULL OR sd.depot_id = ?1)
               AND (CAST(?2 AS BIGINT) = 1 OR sd.quantite <> 0)
             ORDER BY c.nom, a.nom",
            &parametres![dep, zero, dossier],
            |r| {
                let qte: f64 = r.get::<f64>(4)?;
                let pa: i64 = r.get::<i64>(5)?;
                Ok(serde_json::json!({
                    "article":    r.get::<String>(0)?,
                    "categorie":  r.get::<String>(1)?,
                    "depot":      r.get::<String>(2)?,
                    "unite":      r.get::<String>(3)?,
                    "quantite":   qte,
                    "prix_achat": pa,
                    "prix_vente": r.get::<i64>(6)?,
                    "valeur":     (pa as f64 * qte).round() as i64,
                }))
            },
        )
        .map_err(|e| e.0)?;
    let valeur_totale: i64 = lignes.iter().filter_map(|l| l["valeur"].as_i64()).sum();
    Ok(serde_json::json!({
        "lignes": lignes,
        "valeur_totale": valeur_totale,
        "nb_articles": lignes.len(),
        "societe": crate::creances::societe_sur(base),
    }))
}
