//! La base d'un poste seul, reprise par Gescom Serveur (D54, 01/11/2026).
//!
//! Un poste qui travaillait seul garde sa base dans le dossier de son
//! utilisateur (`%APPDATA%\ml.gescom.app\gescom.db`). Le jour ou l'on
//! installe Gescom Serveur sur ce poste, le service demarre sur SA base,
//! dans `C:\ProgramData\Gescom` — neuve et vide — et la caisse du poste,
//! qui s'y branche, parait vide. Rien n'etait perdu, mais rien ne le
//! disait, et le passage se faisait a la main, avec un technicien.
//!
//! Ici : l'examen (y a-t-il quelque chose a proposer ?) et la copie.
//! L'installeur demande ; `gescom-serveur --reprendre` fait.
//!
//! ## Ce qui est promis
//!
//! - **L'originale n'est jamais ouverte en ecriture.** La copie passe par
//!   `VACUUM INTO` : un instantane coherent, journal WAL compris (une
//!   copie du fichier seul perdrait les dernieres ventes).
//! - **La copie est verifiee avant de servir** : integrite, et le meme
//!   nombre de lignes dans chaque table que l'originale.
//! - **On ne remplace qu'une base neuve**, et on ne l'efface pas : elle est
//!   mise de cote a cote (`…avant-reprise-<date>`).
//! - **Une marque** (`reprise.json`) reste a cote de l'originale : le
//!   serveur lance sans `--base` refuse ensuite de la servir, pour qu'une
//!   caisse ne vende jamais dans l'ancienne base pendant que les autres
//!   vendent dans la vraie.
//!
//! SQLite seulement, des deux cotes : vers PostgreSQL, refus qui le dit.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags};

use crate::utils::maintenant_iso;

/// Le fichier de la marque, a cote de la base du poste.
pub const MARQUE: &str = "reprise.json";

/// Les codes de sortie de `gescom-serveur --reprendre` : l'installeur
/// (NSIS) les lit et affiche son propre texte.
pub mod codes {
    /// Fait (`--reprendre`), ou rien a proposer (`--examiner`).
    pub const RIEN_OU_FAIT: i32 = 0;
    /// La base du poste a des ventes, celle du serveur est neuve.
    pub const A_PROPOSER: i32 = 10;
    /// Les deux bases ont des ventes : on ne choisit pas.
    pub const DEUX_BASES: i32 = 11;
    /// La base du serveur est sur PostgreSQL.
    pub const CIBLE_POSTGRES: i32 = 12;
    /// La base du poste est illisible, abimee, ou n'est pas une base Gescom.
    pub const SOURCE_ILLISIBLE: i32 = 13;
    /// La base du serveur existe mais ne se lit pas : on n'y touche pas.
    pub const CIBLE_ILLISIBLE: i32 = 14;
    /// `--reprendre` sur une base de poste absente ou neuve.
    pub const RIEN_A_REPRENDRE: i32 = 15;
    /// La copie ne correspond pas a l'originale.
    pub const COPIE_DIFFERENTE: i32 = 20;
    /// La base du serveur est ouverte (le service tourne).
    pub const CIBLE_OUVERTE: i32 = 21;
    /// Ecriture impossible dans le dossier du serveur (disque, droits).
    pub const ECRITURE_IMPOSSIBLE: i32 = 22;
}

/// Ce qu'une base contient, de quoi la reconnaitre.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Contenu {
    pub ventes: i64,
    pub pieces: i64,
    pub articles: i64,
    /// Sans le client de passage, que toute base a.
    pub clients: i64,
    /// `AAAA-MM-JJ` de la derniere vente.
    pub derniere_vente: Option<String>,
}

impl Contenu {
    /// Neuve : ni vente, ni piece, ni article. Un dossier cree par
    /// l'assistant sans rien d'autre compte comme neuf — ce n'est pas une
    /// boutique qui a travaille.
    pub fn est_neuve(&self) -> bool {
        self.ventes + self.pieces + self.articles == 0
    }

    /// « 1 234 ventes, 210 articles, 56 clients ; dernière vente le
    /// 31/10/2026 ».
    pub fn en_clair(&self) -> String {
        let mut t = format!(
            "{} vente{}, {} article{}, {} client{}",
            milliers(self.ventes),
            pluriel(self.ventes),
            milliers(self.articles),
            pluriel(self.articles),
            milliers(self.clients),
            pluriel(self.clients),
        );
        if let Some(d) = self.derniere_vente.as_deref().and_then(date_fr) {
            t.push_str(&format!(" ; dernière vente le {d}"));
        }
        t
    }

    /// Pour l'installeur : `1 234 ventes|210 articles|56 clients|31/10/2026`
    /// (la derniere vente, ou `-`). ASCII seulement — les mots le sont, la
    /// date aussi : la sortie que `nsExec` capture ne garantit pas les
    /// accents ; « dernière vente le » est dans le script NSIS.
    pub fn pour_installeur(&self) -> String {
        format!(
            "{} vente{}|{} article{}|{} client{}|{}",
            milliers(self.ventes),
            pluriel(self.ventes),
            milliers(self.articles),
            pluriel(self.articles),
            milliers(self.clients),
            pluriel(self.clients),
            self.derniere_vente.as_deref().and_then(date_fr).unwrap_or_else(|| "-".to_string())
        )
    }
}

fn pluriel(n: i64) -> &'static str {
    if n > 1 {
        "s"
    } else {
        ""
    }
}

/// `1234567` → `1 234 567`.
pub fn milliers(n: i64) -> String {
    let chiffres = n.abs().to_string();
    let mut t = String::new();
    for (i, c) in chiffres.chars().enumerate() {
        if i > 0 && (chiffres.len() - i) % 3 == 0 {
            t.push(' ');
        }
        t.push(c);
    }
    if n < 0 {
        format!("-{t}")
    } else {
        t
    }
}

/// `2026-10-31…` → `31/10/2026`.
fn date_fr(iso: &str) -> Option<String> {
    let j = iso.get(0..10)?;
    let (a, reste) = j.split_once('-')?;
    let (m, d) = reste.split_once('-')?;
    Some(format!("{d}/{m}/{a}"))
}

/// Un refus : le code de sortie et la phrase qui le dit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refus {
    pub code: i32,
    pub message: String,
}

impl Refus {
    fn new(code: i32, message: impl Into<String>) -> Refus {
        Refus { code, message: message.into() }
    }
}

impl std::fmt::Display for Refus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

fn est_postgres(cible: &str) -> bool {
    cible.starts_with("postgres://") || cible.starts_with("postgresql://")
}

/// Une table existe-t-elle ? Une base ancienne n'a pas toutes les tables :
/// absente vaut zero ligne, mais une ERREUR de lecture ne vaut jamais zero
/// — sinon une base illisible passerait pour neuve, et serait remplacee.
fn table_existe(conn: &Connection, table: &str) -> Result<bool, String> {
    conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
        [table],
        |r| r.get::<_, i64>(0),
    )
    .map(|n| n > 0)
    .map_err(|e| e.to_string())
}

fn compter(conn: &Connection, table: &str, filtre: &str) -> Result<i64, String> {
    if !table_existe(conn, table)? {
        return Ok(0);
    }
    // `table` et `filtre` viennent d'ici, jamais de l'exterieur.
    conn.query_row(&format!("SELECT COUNT(*) FROM {table} {filtre}"), [], |r| r.get(0))
        .map_err(|e| e.to_string())
}

/// Ouvre une base en lecture seule, verifie qu'elle est saine et que
/// c'est une base Gescom.
fn ouvrir_saine(chemin: &Path) -> Result<Connection, String> {
    let conn = Connection::open_with_flags(chemin, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)
        .map_err(|e| format!("{} n'est pas une base lisible : {e}", chemin.display()))?;
    conn.busy_timeout(std::time::Duration::from_secs(5)).ok();
    let verdict: String = conn
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .map_err(|e| format!("{} : contrôle d'intégrité impossible : {e}", chemin.display()))?;
    if verdict != "ok" {
        return Err(format!("{} est abîmée ({verdict}).", chemin.display()));
    }
    if !table_existe(&conn, "vente")? {
        return Err(format!("{} n'est pas une base Gescom (pas de table vente).", chemin.display()));
    }
    Ok(conn)
}

fn contenu_de(conn: &Connection) -> Result<Contenu, String> {
    let filtre_clients = if conn.prepare("SELECT est_generique FROM client LIMIT 0").is_ok() {
        "WHERE est_generique = 0"
    } else {
        ""
    };
    Ok(Contenu {
        ventes: compter(conn, "vente", "")?,
        pieces: compter(conn, "piece_commerciale", "")?,
        articles: compter(conn, "article", "")?,
        clients: compter(conn, "client", filtre_clients)?,
        derniere_vente: conn
            .query_row("SELECT MAX(SUBSTR(date_vente, 1, 10)) FROM vente", [], |r| r.get::<_, Option<String>>(0))
            .map_err(|e| e.to_string())?,
    })
}

/// Ce que contient la base de ce fichier ; `None` s'il n'existe pas.
pub fn lire_contenu(chemin: &Path) -> Result<Option<Contenu>, String> {
    if !chemin.exists() {
        return Ok(None);
    }
    let conn = ouvrir_saine(chemin)?;
    contenu_de(&conn).map(Some)
}

/// Le nombre de lignes de chaque table : ce qu'on compare entre
/// l'originale et la copie.
fn lignes_par_table(conn: &Connection) -> Result<BTreeMap<String, i64>, String> {
    let mut stmt = conn
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name")
        .map_err(|e| e.to_string())?;
    let tables: Vec<String> = stmt
        .query_map([], |r| r.get(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    let mut lignes = BTreeMap::new();
    for t in tables {
        // Le nom vient de sqlite_master ; entre guillemets, doubles echappes.
        let n: i64 = conn
            .query_row(&format!("SELECT COUNT(*) FROM \"{}\"", t.replace('"', "\"\"")), [], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        lignes.insert(t, n);
    }
    Ok(lignes)
}

// ---------------------------------------------------------------------
//  La marque
// ---------------------------------------------------------------------

/// Ce que dit `reprise.json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Marque {
    /// La base qui a pris le relais (sans mot de passe : un chemin).
    pub vers: String,
    /// Quand (`maintenant_iso`).
    pub le: String,
    /// `true` : rien n'a ete copie, on a garde la base du serveur.
    pub laissee: bool,
}

impl Marque {
    /// Pourquoi le serveur lance sans `--base` ne sert plus cette base.
    pub fn raison(&self) -> String {
        let quand = date_fr(&self.le).unwrap_or_else(|| self.le.clone());
        let fait = if self.laissee {
            format!("a été laissée de côté le {quand} : Gescom Serveur garde la base de la boutique ({})", self.vers)
        } else {
            format!("a été reprise par Gescom Serveur le {quand} ({})", self.vers)
        };
        format!(
            "la base de ce poste {fait}. Ce serveur ne la sert plus : si les caisses ne trouvent plus \
             le serveur, regarder le service Gescom Serveur (fenêtre « Gescom Serveur », ou sc query GescomServeur)."
        )
    }
}

/// La marque posee a cote de la base du poste, s'il y en a une.
pub fn marque(dossier_poste: &Path) -> Option<Marque> {
    let texte = std::fs::read_to_string(dossier_poste.join(MARQUE)).ok()?;
    let v: serde_json::Value = serde_json::from_str(texte.trim_start_matches('\u{feff}')).ok()?;
    Some(Marque {
        vers: v.get("vers").and_then(|x| x.as_str()).unwrap_or("").to_string(),
        le: v.get("le").and_then(|x| x.as_str()).unwrap_or("").to_string(),
        laissee: v.get("laissee").and_then(|x| x.as_bool()).unwrap_or(false),
    })
}

fn poser_marque(source: &Path, vers: &str, laissee: bool, contenu: Option<&Contenu>) -> Result<(), String> {
    let dossier = source.parent().unwrap_or_else(|| Path::new("."));
    let mut v = serde_json::json!({
        "vers": vers,
        "le": maintenant_iso(),
        "laissee": laissee,
        "base_du_poste": source.to_string_lossy(),
    });
    if let Some(c) = contenu {
        v["ventes"] = c.ventes.into();
        v["articles"] = c.articles.into();
        v["clients"] = c.clients.into();
    }
    let texte = serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?;
    std::fs::write(dossier.join(MARQUE), texte)
        .map_err(|e| format!("Marque {} impossible à écrire : {e}", dossier.join(MARQUE).display()))
}

// ---------------------------------------------------------------------
//  L'examen
// ---------------------------------------------------------------------

/// Ce que l'installeur doit faire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Examen {
    /// Pas de base de poste, ou une base neuve : rien a dire.
    Rien,
    /// La marque est la : deja reprise, ou laissee de cote.
    DejaFaite(Marque),
    /// La base du poste a des ventes, celle du serveur est neuve.
    AProposer(Contenu),
    /// Les deux ont des ventes.
    DeuxBases { poste: Contenu, serveur: Contenu },
    /// La base du serveur est sur PostgreSQL.
    CiblePostgres(Contenu),
    SourceIllisible(String),
    CibleIllisible(String),
}

impl Examen {
    pub fn code(&self) -> i32 {
        match self {
            Examen::Rien | Examen::DejaFaite(_) => codes::RIEN_OU_FAIT,
            Examen::AProposer(_) => codes::A_PROPOSER,
            Examen::DeuxBases { .. } => codes::DEUX_BASES,
            Examen::CiblePostgres(_) => codes::CIBLE_POSTGRES,
            Examen::SourceIllisible(_) => codes::SOURCE_ILLISIBLE,
            Examen::CibleIllisible(_) => codes::CIBLE_ILLISIBLE,
        }
    }

    /// Le contenu de la base du poste, quand il compte pour l'ecran.
    pub fn contenu_du_poste(&self) -> Option<&Contenu> {
        match self {
            Examen::AProposer(c) | Examen::CiblePostgres(c) => Some(c),
            Examen::DeuxBases { poste, .. } => Some(poste),
            _ => None,
        }
    }

    /// La phrase, pour la ligne de commande.
    pub fn en_clair(&self, source: &Path, cible: &str) -> String {
        match self {
            Examen::Rien => format!("Rien à reprendre : pas de base de poste avec des ventes ({}).", source.display()),
            Examen::DejaFaite(m) => format!("Rien à faire : {}", m.raison()),
            Examen::AProposer(c) => format!(
                "À reprendre : la base du poste ({}) a {} ; celle du serveur ({cible}) est neuve.",
                source.display(),
                c.en_clair()
            ),
            Examen::DeuxBases { poste, serveur } => format!(
                "Les deux bases ont des ventes — base du poste ({}) : {} ; base du serveur ({cible}) : {}. \
                 Rien n'est copié. Pour garder celle du serveur : --reprendre … --laisser.",
                source.display(),
                poste.en_clair(),
                serveur.en_clair()
            ),
            Examen::CiblePostgres(_) => {
                "La base du serveur est sur PostgreSQL : la reprise ne copie que vers un fichier SQLite.".to_string()
            }
            Examen::SourceIllisible(e) => format!("La base du poste ne se lit pas : {e}"),
            Examen::CibleIllisible(e) => format!("La base du serveur ne se lit pas : {e}"),
        }
    }
}

/// Faut-il proposer de reprendre la base `source` (celle du poste) dans
/// `cible` (celle du serveur, chemin ou URL) ?
pub fn examiner(source: &Path, cible: &str) -> Examen {
    if let Some(m) = source.parent().and_then(marque) {
        return Examen::DejaFaite(m);
    }
    let poste = match lire_contenu(source) {
        Ok(Some(c)) if !c.est_neuve() => c,
        Ok(_) => return Examen::Rien,
        Err(e) => return Examen::SourceIllisible(e),
    };
    if est_postgres(cible) {
        return Examen::CiblePostgres(poste);
    }
    match lire_contenu(Path::new(cible)) {
        Ok(None) => Examen::AProposer(poste),
        Ok(Some(serveur)) if serveur.est_neuve() => Examen::AProposer(poste),
        Ok(Some(serveur)) => Examen::DeuxBases { poste, serveur },
        Err(e) => Examen::CibleIllisible(e),
    }
}

// ---------------------------------------------------------------------
//  La copie
// ---------------------------------------------------------------------

/// Ce que la reprise a fait.
#[derive(Debug, Clone)]
pub struct Reprise {
    pub contenu: Contenu,
    /// La base neuve du serveur, mise de cote (rien n'est efface).
    pub neuve_mise_de_cote: Option<PathBuf>,
    /// Les images copiees dans le dossier du serveur.
    pub images: Vec<PathBuf>,
    /// La marque n'a pas pu etre posee : la copie est faite, mais le
    /// garde-fou manque. `None` : posee.
    pub marque_manquante: Option<String>,
}

/// Les trois images de la boutique : leur colonne, et le nom du fichier
/// de repli a cote de la base (voir `images.rs`).
const IMAGES: [(&str, &str); 3] = [("logo_chemin", "logo"), ("entete_chemin", "entete"), ("pied_chemin", "pied")];
const EXTENSIONS: [&str; 5] = ["png", "jpg", "jpeg", "webp", "svg"];

/// Copie les images du poste a cote de la copie, et corrige leurs chemins
/// DANS LA COPIE. Une image qui ne se copie pas garde son ancien chemin :
/// l'originale reste en place, le service la lit encore.
fn reprendre_images(copie: &Connection, dossier_poste: &Path, dossier_serveur: &Path) -> Vec<PathBuf> {
    let mut faites = Vec::new();
    // Les fichiers de repli (`logo.png` a cote de la base).
    for (_, nom) in IMAGES {
        for ext in EXTENSIONS {
            let src = dossier_poste.join(format!("{nom}.{ext}"));
            let dst = dossier_serveur.join(format!("{nom}.{ext}"));
            if src.is_file() && std::fs::copy(&src, &dst).is_ok() {
                faites.push(dst);
            }
        }
    }
    // Les chemins enregistres.
    let colonnes_presentes = copie.prepare("SELECT logo_chemin, entete_chemin, pied_chemin FROM parametres_societe LIMIT 0").is_ok();
    if !colonnes_presentes {
        return faites;
    }
    for (colonne, _) in IMAGES {
        let chemins: Vec<String> = copie
            .prepare(&format!(
                "SELECT DISTINCT {colonne} FROM parametres_societe WHERE {colonne} IS NOT NULL AND {colonne} <> ''"
            ))
            .and_then(|mut s| s.query_map([], |r| r.get(0))?.collect())
            .unwrap_or_default();
        for ancien in chemins {
            let src = PathBuf::from(&ancien);
            let Some(nom) = src.file_name() else { continue };
            let dst = dossier_serveur.join(nom);
            if !src.is_file() {
                continue;
            }
            if src != dst && std::fs::copy(&src, &dst).is_err() {
                continue;
            }
            let nouveau = dst.to_string_lossy().to_string();
            if copie
                .execute(
                    &format!("UPDATE parametres_societe SET {colonne} = ?1 WHERE {colonne} = ?2"),
                    [&nouveau, &ancien],
                )
                .is_ok()
                && !faites.contains(&dst)
            {
                faites.push(dst);
            }
        }
    }
    faites
}

/// Retire un fichier SQLite et ses compagnons.
fn retirer_base(chemin: &Path) {
    for suffixe in ["", "-wal", "-shm", "-journal"] {
        let _ = std::fs::remove_file(format!("{}{suffixe}", chemin.display()));
    }
}

/// Copie la base du poste `source` dans `cible` (la base du serveur, un
/// fichier SQLite), si `cible` est absente ou neuve. Voir l'en-tete pour
/// ce qui est promis. La marque ne bloque pas une reprise demandee a la
/// main : seul l'examen de l'installeur la lit.
pub fn reprendre(source: &Path, cible: &Path) -> Result<Reprise, Refus> {
    let cible_texte = cible.to_string_lossy().to_string();
    if est_postgres(&cible_texte) {
        return Err(Refus::new(
            codes::CIBLE_POSTGRES,
            "La base du serveur est sur PostgreSQL : la reprise ne copie que vers un fichier SQLite.",
        ));
    }
    let conn_source = match source.exists() {
        true => ouvrir_saine(source).map_err(|e| Refus::new(codes::SOURCE_ILLISIBLE, e))?,
        false => {
            return Err(Refus::new(
                codes::RIEN_A_REPRENDRE,
                format!("Pas de base de poste ici : {}", source.display()),
            ))
        }
    };
    let contenu = contenu_de(&conn_source).map_err(|e| Refus::new(codes::SOURCE_ILLISIBLE, e))?;
    if contenu.est_neuve() {
        return Err(Refus::new(
            codes::RIEN_A_REPRENDRE,
            format!("La base du poste ({}) n'a ni vente, ni pièce, ni article : rien à reprendre.", source.display()),
        ));
    }
    match lire_contenu(cible) {
        Ok(None) => {}
        Ok(Some(c)) if c.est_neuve() => {}
        Ok(Some(c)) => {
            return Err(Refus::new(
                codes::DEUX_BASES,
                format!(
                    "La base du serveur ({}) n'est pas neuve ({}) : elle n'est pas remplacée.",
                    cible.display(),
                    c.en_clair()
                ),
            ))
        }
        Err(e) => return Err(Refus::new(codes::CIBLE_ILLISIBLE, format!("La base du serveur ne se lit pas : {e}"))),
    }

    let dossier_serveur = cible.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dossier_serveur).map_err(|e| {
        Refus::new(codes::ECRITURE_IMPOSSIBLE, format!("Dossier {} impossible à créer : {e}", dossier_serveur.display()))
    })?;
    let nom = cible.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "gescom.db".to_string());
    let temporaire = dossier_serveur.join(format!("{nom}.reprise-en-cours"));
    retirer_base(&temporaire);

    // 1. L'instantane. Les lignes comptees avant et apres : si elles
    //    bougent, quelqu'un ecrit dans la base du poste pendant la copie.
    let avant = lignes_par_table(&conn_source).map_err(|e| Refus::new(codes::SOURCE_ILLISIBLE, e))?;
    if let Err(e) = conn_source.execute("VACUUM INTO ?1", [temporaire.to_string_lossy().as_ref()]) {
        retirer_base(&temporaire);
        return Err(Refus::new(
            codes::ECRITURE_IMPOSSIBLE,
            format!("Copie impossible dans {} : {e} (disque plein ? droits ?)", dossier_serveur.display()),
        ));
    }
    let apres = lignes_par_table(&conn_source).map_err(|e| Refus::new(codes::SOURCE_ILLISIBLE, e))?;
    drop(conn_source);
    if avant != apres {
        retirer_base(&temporaire);
        return Err(Refus::new(
            codes::COPIE_DIFFERENTE,
            "La base du poste a changé pendant la copie : Gescom est-il encore ouvert ? Le fermer, et recommencer.",
        ));
    }

    // 2. Les images, dans la copie seulement.
    let images = {
        let copie = match Connection::open(&temporaire) {
            Ok(c) => c,
            Err(e) => {
                retirer_base(&temporaire);
                return Err(Refus::new(codes::ECRITURE_IMPOSSIBLE, format!("La copie ne s'ouvre pas : {e}")));
            }
        };
        // Un seul fichier, sans journal a cote : c'est lui qu'on renomme.
        let _ = copie.query_row("PRAGMA journal_mode=DELETE", [], |r| r.get::<_, String>(0));
        let dossier_poste = source.parent().unwrap_or_else(|| Path::new("."));
        reprendre_images(&copie, dossier_poste, dossier_serveur)
    };

    // 3. La verification : saine, et les memes lignes partout.
    let verifiee = ouvrir_saine(&temporaire).and_then(|c| lignes_par_table(&c));
    match verifiee {
        Ok(copie) if copie == avant => {}
        Ok(copie) => {
            retirer_base(&temporaire);
            let ecarts: Vec<String> = avant
                .iter()
                .filter(|(t, n)| copie.get(*t) != Some(n))
                .map(|(t, n)| format!("{t} : {n} → {}", copie.get(t).map(|c| c.to_string()).unwrap_or("absente".into())))
                .take(5)
                .collect();
            return Err(Refus::new(
                codes::COPIE_DIFFERENTE,
                format!("La copie ne correspond pas à l'originale ({}) : rien n'est remplacé.", ecarts.join(", ")),
            ));
        }
        Err(e) => {
            retirer_base(&temporaire);
            return Err(Refus::new(codes::COPIE_DIFFERENTE, format!("La copie ne se vérifie pas : {e}")));
        }
    }

    // 4. La base neuve du serveur, mise de cote ; puis la copie a sa
    //    place. Sous Windows, un fichier que le service tient ouvert ne se
    //    renomme pas : c'est le refus « service en marche ».
    let mut neuve_mise_de_cote = None;
    if cible.exists() {
        let horodatage = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
        let a_cote = dossier_serveur.join(format!("{nom}.avant-reprise-{horodatage}"));
        if let Err(e) = std::fs::rename(cible, &a_cote) {
            retirer_base(&temporaire);
            return Err(Refus::new(
                codes::CIBLE_OUVERTE,
                format!(
                    "La base du serveur ({}) est ouverte par un autre programme : arrêter le service \
                     (sc stop GescomServeur), puis recommencer. ({e})",
                    cible.display()
                ),
            ));
        }
        // Son journal WAL la suit, sous le nom que SQLite cherchera.
        let wal = PathBuf::from(format!("{cible_texte}-wal"));
        if wal.exists() {
            let _ = std::fs::rename(&wal, format!("{}-wal", a_cote.display()));
        }
        let _ = std::fs::remove_file(format!("{cible_texte}-shm"));
        neuve_mise_de_cote = Some(a_cote);
    }
    if let Err(e) = std::fs::rename(&temporaire, cible) {
        // Remettre la base neuve a sa place : l'etat d'avant, rien de perdu.
        if let Some(a_cote) = &neuve_mise_de_cote {
            let _ = std::fs::rename(a_cote, cible);
        }
        retirer_base(&temporaire);
        return Err(Refus::new(
            codes::ECRITURE_IMPOSSIBLE,
            format!("La copie n'a pas pu prendre la place de {} : {e}", cible.display()),
        ));
    }

    // 5. La marque, a cote de l'originale.
    let marque_manquante = poser_marque(source, &cible_texte, false, Some(&contenu)).err();
    Ok(Reprise { contenu, neuve_mise_de_cote, images, marque_manquante })
}

/// Trancher pour la base du serveur : rien n'est copie, la marque est
/// posee (la boite de l'installeur ne revient plus, et le garde-fou
/// s'applique a l'ancienne base).
pub fn laisser(source: &Path, cible: &str) -> Result<(), Refus> {
    if !source.exists() {
        return Err(Refus::new(codes::RIEN_A_REPRENDRE, format!("Pas de base de poste ici : {}", source.display())));
    }
    poser_marque(source, cible, true, None).map_err(|e| Refus::new(codes::ECRITURE_IMPOSSIBLE, e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_nombres_et_les_dates_s_ecrivent_a_la_francaise() {
        assert_eq!(milliers(0), "0");
        assert_eq!(milliers(999), "999");
        assert_eq!(milliers(1234), "1 234");
        assert_eq!(milliers(1234567), "1 234 567");
        assert_eq!(date_fr("2026-10-31T12:00:00").as_deref(), Some("31/10/2026"));
        let c = Contenu { ventes: 1234, pieces: 0, articles: 210, clients: 1, derniere_vente: Some("2026-10-31".into()) };
        assert_eq!(c.en_clair(), "1 234 ventes, 210 articles, 1 client ; dernière vente le 31/10/2026");
        assert_eq!(c.pour_installeur(), "1 234 ventes|210 articles|1 client|31/10/2026");
        assert!(c.pour_installeur().is_ascii());
        assert_eq!(Contenu::default().pour_installeur(), "0 vente|0 article|0 client|-");
        assert!(Contenu::default().est_neuve());
    }
}
