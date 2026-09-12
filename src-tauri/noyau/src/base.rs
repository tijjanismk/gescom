//! Parler a SQLite ou a PostgreSQL, sans que l'appelant le sache.
//!
//! ## Pourquoi une facade et pas un remplacement
//!
//! Le noyau compte 739 points d'appel a rusqlite, 480 `params!` et 435
//! fermetures de lecture. Les reecrire tous d'un coup, c'est des
//! milliers d'editions sans filet : la premiere erreur se verrait chez
//! un commercant, pas ici.
//!
//! Cette facade permet de porter MODULE PAR MODULE. Ce qui n'est pas
//! encore porte continue de fonctionner sur SQLite, exactement comme
//! avant.
//!
//! ## Les deux ecarts qui comptent
//!
//! - **Les parametres.** SQLite ecrit `?1`, PostgreSQL `$1`. La
//!   traduction est mecanique et se fait ici : recrire 1448
//!   placeholders a la main, c'est 1448 occasions de se tromper.
//! - **Les lignes.** `rusqlite::Row` et `postgres::Row` n'ont pas la
//!   meme API. [`Ligne`] leur donne la meme, pour que les fermetures
//!   `|r| r.get(0)?` survivent au portage sans etre touchees.
//!
//! ## Ce que la facade ne fait PAS
//!
//! Elle ne traduit pas le SQL au-dela des placeholders. `julianday`,
//! `strftime`, `INSERT OR IGNORE`, `substr(x, -5)` n'existent pas cote
//! PostgreSQL et doivent etre reecrits **a la main, une fois**, en SQL
//! que les deux moteurs comprennent. Une traduction automatique de SQL
//! est un piege : elle marche sur les cas qu'on a essayes, et elle
//! ment sur les autres.

use std::fmt;

// =====================================================================
//  L'ERREUR
// =====================================================================

#[derive(Debug)]
pub struct Erreur(pub String);

impl fmt::Display for Erreur {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for Erreur {}

impl From<rusqlite::Error> for Erreur {
    fn from(e: rusqlite::Error) -> Self {
        Erreur(e.to_string())
    }
}

impl From<postgres::Error> for Erreur {
    fn from(e: postgres::Error) -> Self {
        // `e.to_string()` rend « db error » — trois mots qui ne disent
        // rien et qui font perdre un quart d'heure a chaque fois. Le
        // detail du serveur porte la vraie cause : la contrainte
        // violee, la colonne absente, le type refuse.
        if let Some(db) = e.as_db_error() {
            let mut m = db.message().to_string();
            if let Some(d) = db.detail() {
                m.push_str(" — ");
                m.push_str(d);
            }
            if let Some(c) = db.column() {
                m.push_str(&format!(" (colonne « {c} »)"));
            }
            if let Some(t) = db.table() {
                m.push_str(&format!(" [table {t}]"));
            }
            return Erreur(m);
        }
        Erreur(e.to_string())
    }
}

impl From<Erreur> for String {
    fn from(e: Erreur) -> String {
        e.0
    }
}

pub type Resultat<T> = std::result::Result<T, Erreur>;

// =====================================================================
//  LES VALEURS
// =====================================================================

/// Ce qu'on passe en parametre, et ce qu'on lit en retour.
///
/// Un type commun plutot que deux : `rusqlite::ToSql` et
/// `postgres::ToSql` ne se recouvrent pas, et un generique sur les deux
/// obligerait chaque appelant a choisir son moteur — ce qui est
/// exactement ce qu'on veut lui epargner.
#[derive(Debug, Clone, PartialEq)]
pub enum Valeur {
    Nul,
    Entier(i64),
    Reel(f64),
    Texte(String),
    Booleen(bool),
}

impl Valeur {
    pub fn est_nul(&self) -> bool {
        matches!(self, Valeur::Nul)
    }
}

impl From<i64> for Valeur {
    fn from(v: i64) -> Self {
        Valeur::Entier(v)
    }
}
impl From<i32> for Valeur {
    fn from(v: i32) -> Self {
        Valeur::Entier(v as i64)
    }
}
impl From<f64> for Valeur {
    fn from(v: f64) -> Self {
        Valeur::Reel(v)
    }
}
impl From<bool> for Valeur {
    fn from(v: bool) -> Self {
        Valeur::Booleen(v)
    }
}
impl From<&str> for Valeur {
    fn from(v: &str) -> Self {
        Valeur::Texte(v.to_string())
    }
}
impl From<String> for Valeur {
    fn from(v: String) -> Self {
        Valeur::Texte(v)
    }
}
impl From<&String> for Valeur {
    fn from(v: &String) -> Self {
        Valeur::Texte(v.clone())
    }
}
impl<T: Into<Valeur>> From<Option<T>> for Valeur {
    fn from(v: Option<T>) -> Self {
        match v {
            Some(x) => x.into(),
            None => Valeur::Nul,
        }
    }
}

/// `parametres![a, b, c]` — l'equivalent de `rusqlite::params!`.
#[macro_export]
macro_rules! parametres {
    () => { Vec::<$crate::base::Valeur>::new() };
    ($($v:expr),+ $(,)?) => {
        vec![$($crate::base::Valeur::from($v)),+]
    };
}

// =====================================================================
//  UNE LIGNE
// =====================================================================

/// Une ligne de resultat, quel que soit le moteur.
///
/// `get` rend directement la valeur et non un `Result` : les
/// fermetures existantes ecrivent `r.get(0)?`, et un `?` sur un
/// `Result` dont l'erreur se convertit en `Erreur` continue de
/// fonctionner.
pub enum Ligne<'a> {
    Sqlite(&'a rusqlite::Row<'a>),
    Pg(&'a postgres::Row),
}

/// Ce qu'on sait extraire d'une colonne.
pub trait DeLigne: Sized {
    fn extraire(ligne: &Ligne<'_>, index: usize) -> Resultat<Self>;
}

impl<'a> Ligne<'a> {
    pub fn get<T: DeLigne>(&self, index: usize) -> Resultat<T> {
        T::extraire(self, index)
    }
}

impl DeLigne for String {
    fn extraire(l: &Ligne<'_>, i: usize) -> Resultat<Self> {
        match l {
            Ligne::Sqlite(r) => Ok(r.get::<_, String>(i)?),
            Ligne::Pg(r) => Ok(r.try_get::<_, String>(i)?),
        }
    }
}

/// Le type PostgreSQL d'une colonne, pour lire selon ce qu'elle EST.
///
/// `postgres` refuse de lire un `INT4` dans un `i64` (« error
/// deserializing column ») la ou SQLite s'en moque. Or les tables v2
/// creees par du DDL en ligne dans `amorcage.rs` — `exercice.clos`,
/// `dossier.clos`, `role.acces_total` — ne passent pas par
/// `types_postgres` et sont donc en `INT4` ; `COUNT(*)` rend `INT8`,
/// une moyenne rend `NUMERIC`. Exiger que chaque DDL et chaque
/// requete y pensent, c'est garantir qu'un jour l'un ne le fera pas.
/// La facade lit ce que la colonne contient, et rend ce que
/// l'appelant demande.
fn type_pg(r: &postgres::Row, i: usize) -> Option<&postgres::types::Type> {
    r.columns().get(i).map(|c| c.type_())
}

fn pg_entier(r: &postgres::Row, i: usize) -> Resultat<i64> {
    use postgres::types::Type;
    match type_pg(r, i) {
        Some(&Type::INT2) => Ok(r.try_get::<_, i16>(i)? as i64),
        Some(&Type::INT4) => Ok(r.try_get::<_, i32>(i)? as i64),
        Some(&Type::BOOL) => Ok(r.try_get::<_, bool>(i)? as i64),
        Some(&Type::FLOAT4) => Ok(r.try_get::<_, f32>(i)? as i64),
        Some(&Type::FLOAT8) => Ok(r.try_get::<_, f64>(i)? as i64),
        _ => Ok(r.try_get::<_, i64>(i)?),
    }
}

fn pg_reel(r: &postgres::Row, i: usize) -> Resultat<f64> {
    use postgres::types::Type;
    match type_pg(r, i) {
        Some(&Type::FLOAT4) => Ok(r.try_get::<_, f32>(i)? as f64),
        Some(&Type::INT2) => Ok(r.try_get::<_, i16>(i)? as f64),
        Some(&Type::INT4) => Ok(r.try_get::<_, i32>(i)? as f64),
        Some(&Type::INT8) => Ok(r.try_get::<_, i64>(i)? as f64),
        _ => Ok(r.try_get::<_, f64>(i)?),
    }
}

impl DeLigne for i64 {
    fn extraire(l: &Ligne<'_>, i: usize) -> Resultat<Self> {
        match l {
            Ligne::Sqlite(r) => Ok(r.get::<_, i64>(i)?),
            Ligne::Pg(r) => pg_entier(r, i),
        }
    }
}

impl DeLigne for f64 {
    fn extraire(l: &Ligne<'_>, i: usize) -> Resultat<Self> {
        match l {
            Ligne::Sqlite(r) => Ok(r.get::<_, f64>(i)?),
            Ligne::Pg(r) => pg_reel(r, i),
        }
    }
}

impl DeLigne for bool {
    fn extraire(l: &Ligne<'_>, i: usize) -> Resultat<Self> {
        match l {
            // SQLite n'a pas de booleen : 0 ou 1, comme partout ailleurs
            // dans cette base.
            Ligne::Sqlite(r) => Ok(r.get::<_, i64>(i)? != 0),
            // Un vrai booleen, ou un entier 0/1 : les deux se lisent.
            Ligne::Pg(r) => match type_pg(r, i) {
                Some(&postgres::types::Type::BOOL) => Ok(r.try_get::<_, bool>(i)?),
                _ => Ok(pg_entier(r, i)? != 0),
            },
        }
    }
}

impl<T: DeLigne> DeLigne for Option<T> {
    fn extraire(l: &Ligne<'_>, i: usize) -> Resultat<Self> {
        let nul = match l {
            Ligne::Sqlite(r) => {
                matches!(r.get_ref(i)?, rusqlite::types::ValueRef::Null)
            }
            Ligne::Pg(r) => r.try_get::<_, Option<&str>>(i).is_err() && false,
        };
        if nul {
            return Ok(None);
        }
        match T::extraire(l, i) {
            Ok(v) => Ok(Some(v)),
            // Cote PostgreSQL, une colonne NULL fait echouer `try_get`
            // du type concret : c'est ainsi qu'on apprend qu'elle est
            // nulle.
            Err(_) => Ok(None),
        }
    }
}

// =====================================================================
//  UN ACCES : LA BASE, OU UNE TRANSACTION
// =====================================================================

/// Ce qu'une fonction metier attend, sans savoir si elle tourne dans
/// une transaction ou en dehors.
///
/// Sans ce trait, chaque aide — reserver un numero, inserer les lignes
/// d'une piece, trouver la caisse ouverte — existait en deux copies :
/// une pour `Base`, une pour `Transaction`. `caisses.rs` en porte
/// encore la trace (`exiger_sur` / `exiger_dans_tx`). Deux copies d'une
/// regle finissent toujours par diverger, et la divergence se voit chez
/// le commercant, pas ici.
///
/// Les methodes sont celles de `Base` et de `Transaction`, a
/// l'identique ; le trait ne fait que les nommer une fois.
pub trait Acces {
    /// Le dossier sur lequel cet acces travaille.
    fn dossier(&self) -> &str;
    fn executer(&mut self, sql: &str, params: &[Valeur]) -> Resultat<u64>;
    fn lire_une<T>(
        &mut self,
        sql: &str,
        params: &[Valeur],
        lire: impl FnOnce(&Ligne<'_>) -> Resultat<T>,
    ) -> Resultat<Option<T>>;
    fn lire_plusieurs<T>(
        &mut self,
        sql: &str,
        params: &[Valeur],
        lire: impl FnMut(&Ligne<'_>) -> Resultat<T>,
    ) -> Resultat<Vec<T>>;
}

impl Acces for Base {
    fn dossier(&self) -> &str {
        Base::dossier(self)
    }
    fn executer(&mut self, sql: &str, params: &[Valeur]) -> Resultat<u64> {
        Base::executer(self, sql, params)
    }
    fn lire_une<T>(
        &mut self,
        sql: &str,
        params: &[Valeur],
        lire: impl FnOnce(&Ligne<'_>) -> Resultat<T>,
    ) -> Resultat<Option<T>> {
        Base::lire_une(self, sql, params, lire)
    }
    fn lire_plusieurs<T>(
        &mut self,
        sql: &str,
        params: &[Valeur],
        lire: impl FnMut(&Ligne<'_>) -> Resultat<T>,
    ) -> Resultat<Vec<T>> {
        Base::lire_plusieurs(self, sql, params, lire)
    }
}

impl Acces for Transaction<'_> {
    fn dossier(&self) -> &str {
        Transaction::dossier(self)
    }
    fn executer(&mut self, sql: &str, params: &[Valeur]) -> Resultat<u64> {
        Transaction::executer(self, sql, params)
    }
    fn lire_une<T>(
        &mut self,
        sql: &str,
        params: &[Valeur],
        lire: impl FnOnce(&Ligne<'_>) -> Resultat<T>,
    ) -> Resultat<Option<T>> {
        Transaction::lire_une(self, sql, params, lire)
    }
    fn lire_plusieurs<T>(
        &mut self,
        sql: &str,
        params: &[Valeur],
        lire: impl FnMut(&Ligne<'_>) -> Resultat<T>,
    ) -> Resultat<Vec<T>> {
        Transaction::lire_plusieurs(self, sql, params, lire)
    }
}

// =====================================================================
//  LA BASE
// =====================================================================

/// La connexion, quel que soit le moteur.
///
/// Elle porte aussi **le dossier sur lequel elle travaille**. Ce n'est
/// pas un detail de rangement : tant que le dossier vivait ailleurs —
/// dans un argument, dans un reglage, dans la tete de l'appelant — il
/// pouvait manquer. Attache a la connexion, il est toujours la.
pub struct Base {
    moteur: Moteur,
    dossier: String,
    /// Refuser les requetes qui oublient `dossier_id`.
    ///
    /// Eteint par defaut : les 739 points d'appel ne sont pas encore
    /// cloisonnes, et allumer partout d'un coup ferait echouer tout ce
    /// qui marche aujourd'hui. On l'allume module par module, au fur et
    /// a mesure du portage — la meme progression que la facade.
    audit: bool,
}

enum Moteur {
    Sqlite(rusqlite::Connection),
    Pg(Box<postgres::Client>),
}

/// `?1` devient `$1`.
///
/// On ne touche PAS a ce qui est entre apostrophes : un texte litteral
/// peut contenir un point d'interrogation, et le traduire casserait le
/// message affiche au commercant.
pub fn traduire_parametres(sql: &str) -> String {
    let mut sortie = String::with_capacity(sql.len());
    let mut dans_texte = false;
    let mut caracteres = sql.chars().peekable();

    while let Some(c) = caracteres.next() {
        // Un commentaire `-- ...` court jusqu'a la fin de la ligne. Il
        // faut le sauter : une apostrophe dedans (« L'oublier ») ferait
        // croire a un texte ouvert, et plus rien ne serait traduit
        // apres — le placeholder arriverait tel quel a PostgreSQL, qui
        // repondrait « l'operateur n'existe pas : ? integer ».
        if c == '-' && !dans_texte && caracteres.peek() == Some(&'-') {
            sortie.push(c);
            for d in caracteres.by_ref() {
                sortie.push(d);
                if d == '\n' {
                    break;
                }
            }
            continue;
        }
        if c == '\'' {
            dans_texte = !dans_texte;
            sortie.push(c);
            continue;
        }
        if c == '?' && !dans_texte {
            // `?1` → `$1`. Un `?` seul n'existe pas dans ce code : tous
            // les placeholders sont numerotes.
            if caracteres.peek().is_some_and(|n| n.is_ascii_digit()) {
                sortie.push('$');
                continue;
            }
        }
        sortie.push(c);
    }
    sortie
}

// =====================================================================
//  LA PLOMBERIE
// =====================================================================
//
// Extraite pour que la `Base` et la `Transaction` la partagent. Sans
// cela, chacune aurait sa copie des memes quatre operations — et une
// correction appliquee a l'une seule finirait par se voir un jour ou
// une vente s'ecrit dans un cas et pas dans l'autre.
//
// `rusqlite::Transaction` se dereference en `Connection`, et
// `postgres::GenericClient` couvre `Client` comme `Transaction` : les
// deux moteurs se pretent au partage, chacun a sa facon.

fn sqlite_executer(
    c: &rusqlite::Connection,
    sql: &str,
    params: &[Valeur],
) -> Resultat<u64> {
    let p = params_sqlite(params);
    let refs: Vec<&dyn rusqlite::ToSql> =
        p.iter().map(|v| v as &dyn rusqlite::ToSql).collect();
    Ok(c.execute(sql, refs.as_slice())? as u64)
}

fn sqlite_lire_une<T>(
    c: &rusqlite::Connection,
    sql: &str,
    params: &[Valeur],
    lire: impl FnOnce(&Ligne<'_>) -> Resultat<T>,
) -> Resultat<Option<T>> {
    let p = params_sqlite(params);
    let refs: Vec<&dyn rusqlite::ToSql> =
        p.iter().map(|v| v as &dyn rusqlite::ToSql).collect();
    let mut st = c.prepare(sql)?;
    let mut lignes = st.query(refs.as_slice())?;
    match lignes.next()? {
        Some(r) => Ok(Some(lire(&Ligne::Sqlite(r))?)),
        None => Ok(None),
    }
}

fn sqlite_lire_plusieurs<T>(
    c: &rusqlite::Connection,
    sql: &str,
    params: &[Valeur],
    mut lire: impl FnMut(&Ligne<'_>) -> Resultat<T>,
) -> Resultat<Vec<T>> {
    let p = params_sqlite(params);
    let refs: Vec<&dyn rusqlite::ToSql> =
        p.iter().map(|v| v as &dyn rusqlite::ToSql).collect();
    let mut st = c.prepare(sql)?;
    let mut lignes = st.query(refs.as_slice())?;
    let mut sortie = Vec::new();
    while let Some(r) = lignes.next()? {
        sortie.push(lire(&Ligne::Sqlite(r))?);
    }
    Ok(sortie)
}

fn pg_executer<C: postgres::GenericClient>(
    c: &mut C,
    sql: &str,
    params: &[Valeur],
) -> Resultat<u64> {
    let sql = traduire_parametres(sql);
    let p = params_pg(params);
    let refs: Vec<&(dyn postgres::types::ToSql + Sync)> =
        p.iter().map(|v| v.as_ref()).collect();
    Ok(c.execute(sql.as_str(), refs.as_slice())?)
}

fn pg_lire_une<C: postgres::GenericClient, T>(
    c: &mut C,
    sql: &str,
    params: &[Valeur],
    lire: impl FnOnce(&Ligne<'_>) -> Resultat<T>,
) -> Resultat<Option<T>> {
    let sql = traduire_parametres(sql);
    let p = params_pg(params);
    let refs: Vec<&(dyn postgres::types::ToSql + Sync)> =
        p.iter().map(|v| v.as_ref()).collect();
    let lignes = c.query(sql.as_str(), refs.as_slice())?;
    match lignes.first() {
        Some(r) => Ok(Some(lire(&Ligne::Pg(r))?)),
        None => Ok(None),
    }
}

fn pg_lire_plusieurs<C: postgres::GenericClient, T>(
    c: &mut C,
    sql: &str,
    params: &[Valeur],
    mut lire: impl FnMut(&Ligne<'_>) -> Resultat<T>,
) -> Resultat<Vec<T>> {
    let sql = traduire_parametres(sql);
    let p = params_pg(params);
    let refs: Vec<&(dyn postgres::types::ToSql + Sync)> =
        p.iter().map(|v| v.as_ref()).collect();
    let mut sortie = Vec::new();
    for r in c.query(sql.as_str(), refs.as_slice())? {
        sortie.push(lire(&Ligne::Pg(&r))?);
    }
    Ok(sortie)
}

// =====================================================================
//  UNE TRANSACTION
// =====================================================================

/// Tout ou rien.
///
/// Indispensable des qu'on touche a l'argent : une vente ecrit la
/// vente, ses lignes, les mouvements de stock, le paiement et le
/// mouvement de caisse. Si l'un echoue, AUCUN ne doit rester — sinon le
/// stock sort sans que l'argent entre, et le comptage du soir tombe
/// faux sans rien pour l'expliquer.
///
/// **Rien n'est ecrit tant que `valider` n'est pas appele.** Une
/// transaction abandonnee retombe d'elle-meme : c'est le bon defaut,
/// parce qu'un oubli doit couter une vente non enregistree, jamais une
/// demi-vente.
pub struct Transaction<'a> {
    moteur: MoteurTx<'a>,
    dossier: String,
    audit: bool,
}

enum MoteurTx<'a> {
    Sqlite(rusqlite::Transaction<'a>),
    Pg(postgres::Transaction<'a>),
}

impl Transaction<'_> {
    /// Le dossier de la connexion qui a ouvert cette transaction.
    pub fn dossier(&self) -> &str {
        &self.dossier
    }

    fn controler(&self, sql: &str) -> Resultat<()> {
        if self.audit {
            if let Some(reproche) = crate::dossiers::requete_non_cloisonnee(sql) {
                return Err(Erreur(reproche));
            }
        }
        Ok(())
    }

    pub fn executer(&mut self, sql: &str, params: &[Valeur]) -> Resultat<u64> {
        self.controler(sql)?;
        match &mut self.moteur {
            MoteurTx::Sqlite(t) => sqlite_executer(t, sql, params),
            MoteurTx::Pg(t) => pg_executer(t, sql, params),
        }
    }

    pub fn lire_une<T>(
        &mut self,
        sql: &str,
        params: &[Valeur],
        lire: impl FnOnce(&Ligne<'_>) -> Resultat<T>,
    ) -> Resultat<Option<T>> {
        self.controler(sql)?;
        match &mut self.moteur {
            MoteurTx::Sqlite(t) => sqlite_lire_une(t, sql, params, lire),
            MoteurTx::Pg(t) => pg_lire_une(t, sql, params, lire),
        }
    }

    pub fn lire_plusieurs<T>(
        &mut self,
        sql: &str,
        params: &[Valeur],
        lire: impl FnMut(&Ligne<'_>) -> Resultat<T>,
    ) -> Resultat<Vec<T>> {
        self.controler(sql)?;
        match &mut self.moteur {
            MoteurTx::Sqlite(t) => sqlite_lire_plusieurs(t, sql, params, lire),
            MoteurTx::Pg(t) => pg_lire_plusieurs(t, sql, params, lire),
        }
    }

    /// Ecrit tout, pour de bon.
    pub fn valider(self) -> Resultat<()> {
        match self.moteur {
            MoteurTx::Sqlite(t) => Ok(t.commit()?),
            MoteurTx::Pg(t) => Ok(t.commit()?),
        }
    }
}

impl Base {
    /// Ouvre SQLite (un chemin) ou PostgreSQL (une URL).
    ///
    /// L'URL decide : c'est le seul reglage, et il tient dans la ligne
    /// de commande comme dans `poste.json`.
    ///
    /// La connexion s'ouvre sur le dossier d'origine. Une installation
    /// qui n'en connait qu'un — c'est le cas de toutes celles qui
    /// existent aujourd'hui — n'a donc rien a faire.
    pub fn ouvrir(cible: &str) -> Resultat<Self> {
        let moteur = if cible.starts_with("postgres://") || cible.starts_with("postgresql://") {
            Moteur::Pg(Box::new(postgres::Client::connect(cible, postgres::NoTls)?))
        } else {
            let conn = rusqlite::Connection::open(cible)?;
            conn.execute_batch(
                "PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON; \
                 PRAGMA busy_timeout=5000;",
            )?;
            Moteur::Sqlite(conn)
        };
        let mut base = Base {
            moteur,
            dossier: crate::dossiers::DOSSIER_DEFAUT.to_string(),
            audit: false,
        };
        base.annoncer_le_dossier()?;
        Ok(base)
    }

    /// Le dossier sur lequel cette connexion travaille.
    pub fn dossier(&self) -> &str {
        &self.dossier
    }

    /// Change de dossier.
    ///
    /// Cote PostgreSQL, la valeur est aussi posee dans la session :
    /// c'est elle qui alimente la valeur par defaut de `dossier_id`,
    /// pour qu'une insertion tombe dans le bon dossier meme quand
    /// l'appelant ne pense pas a le dire.
    pub fn choisir_dossier(&mut self, dossier_id: &str) -> Resultat<()> {
        self.dossier = dossier_id.to_string();
        self.annoncer_le_dossier()
    }

    fn annoncer_le_dossier(&mut self) -> Resultat<()> {
        let dossier = self.dossier.clone();
        if let Moteur::Pg(c) = &mut self.moteur {
            // `set_config` et non `SET` : `SET` n'accepte pas de
            // parametre, et concatener une valeur dans du SQL est la
            // porte ouverte a l'injection.
            c.execute("SELECT set_config('gescom.dossier', $1, false)", &[&dossier])?;
        }
        Ok(())
    }

    /// Allume ou eteint le refus des requetes non cloisonnees.
    pub fn auditer(&mut self, actif: bool) {
        self.audit = actif;
    }

    /// Refuse la requete si elle oublie le cloisonnement.
    ///
    /// L'erreur arrive AVANT l'execution : une requete qui melange deux
    /// societes ne doit pas rendre un resultat qu'on pourrait lire.
    fn controler(&self, sql: &str) -> Resultat<()> {
        if self.audit {
            if let Some(reproche) = crate::dossiers::requete_non_cloisonnee(sql) {
                return Err(Erreur(reproche));
            }
        }
        Ok(())
    }

    pub fn est_postgres(&self) -> bool {
        matches!(self.moteur, Moteur::Pg(_))
    }

    /// Le nom du moteur, pour les messages de demarrage.
    pub fn moteur(&self) -> &'static str {
        match self.moteur {
            Moteur::Sqlite(_) => "SQLite",
            Moteur::Pg(_) => "PostgreSQL",
        }
    }

    /// Une ecriture. Rend le nombre de lignes touchees.
    pub fn executer(&mut self, sql: &str, params: &[Valeur]) -> Resultat<u64> {
        self.controler(sql)?;
        match &mut self.moteur {
            Moteur::Sqlite(c) => sqlite_executer(c, sql, params),
            Moteur::Pg(c) => pg_executer(c.as_mut(), sql, params),
        }
    }

    /// Ouvre une transaction : tout ou rien.
    pub fn transaction(&mut self) -> Resultat<Transaction<'_>> {
        let audit = self.audit;
        let dossier = self.dossier.clone();
        let moteur = match &mut self.moteur {
            Moteur::Sqlite(c) => MoteurTx::Sqlite(c.transaction()?),
            Moteur::Pg(c) => MoteurTx::Pg(c.transaction()?),
        };
        Ok(Transaction { moteur, dossier, audit })
    }

    /// Plusieurs instructions d'un coup (schema, migrations).
    pub fn executer_lot(&mut self, sql: &str) -> Resultat<()> {
        match &mut self.moteur {
            Moteur::Sqlite(c) => Ok(c.execute_batch(sql)?),
            Moteur::Pg(c) => Ok(c.batch_execute(sql)?),
        }
    }

    /// Une ligne, ou `None`.
    ///
    /// `None` et non une erreur quand il n'y a rien : « pas de ligne »
    /// est une reponse courante et attendue, pas un incident.
    pub fn lire_une<T>(
        &mut self,
        sql: &str,
        params: &[Valeur],
        lire: impl FnOnce(&Ligne<'_>) -> Resultat<T>,
    ) -> Resultat<Option<T>> {
        self.controler(sql)?;
        match &mut self.moteur {
            Moteur::Sqlite(c) => sqlite_lire_une(c, sql, params, lire),
            Moteur::Pg(c) => pg_lire_une(c.as_mut(), sql, params, lire),
        }
    }

    /// Toutes les lignes.
    pub fn lire_plusieurs<T>(
        &mut self,
        sql: &str,
        params: &[Valeur],
        lire: impl FnMut(&Ligne<'_>) -> Resultat<T>,
    ) -> Resultat<Vec<T>> {
        self.controler(sql)?;
        match &mut self.moteur {
            Moteur::Sqlite(c) => sqlite_lire_plusieurs(c, sql, params, lire),
            Moteur::Pg(c) => pg_lire_plusieurs(c.as_mut(), sql, params, lire),
        }
    }

    /// La connexion SQLite en ecriture, quand il en FAUT une.
    ///
    /// Rend `None` sur PostgreSQL : l'appelant doit le dire clairement
    /// plutot que d'echouer sur un message incomprehensible.
    pub fn sqlite_mut(&mut self) -> Option<&mut rusqlite::Connection> {
        match &mut self.moteur {
            Moteur::Sqlite(c) => Some(c),
            Moteur::Pg(_) => None,
        }
    }

    /// La connexion SQLite en lecture.
    pub fn sqlite(&self) -> Option<&rusqlite::Connection> {
        match &self.moteur {
            Moteur::Sqlite(c) => Some(c),
            Moteur::Pg(_) => None,
        }
    }
}

fn params_sqlite(params: &[Valeur]) -> Vec<rusqlite::types::Value> {
    use rusqlite::types::Value as V;
    params
        .iter()
        .map(|v| match v {
            Valeur::Nul => V::Null,
            Valeur::Entier(i) => V::Integer(*i),
            Valeur::Reel(f) => V::Real(*f),
            Valeur::Texte(s) => V::Text(s.clone()),
            // SQLite n'a pas de booleen : 0 ou 1, comme le reste de
            // cette base depuis l'origine.
            Valeur::Booleen(b) => V::Integer(*b as i64),
        })
        .collect()
}

/// Un NULL qui n'annonce aucun type.
///
/// `Option::<String>::None` annoncait TEXT. PostgreSQL, lui, deduit le
/// type attendu de la colonne : pour `dernier_prix_achat`, il attend un
/// entier. Recevant un NULL etiquete TEXT, il refusait la requete avec
/// « error serializing parameter 3 » — un message qui ne nomme ni la
/// colonne, ni la table, ni le type attendu.
///
/// Le cas n'a rien d'exotique : c'est un article cree sans prix
/// d'achat, une adresse laissee vide, une date non renseignee. Il ne
/// s'etait pas vu parce que SQLite accepte tout, et que les scenarios
/// PostgreSQL renseignaient tous ces champs.
///
/// `accepts` rend `true` pour n'importe quel type : un NULL n'a pas de
/// contenu a serialiser, donc rien qui puisse etre du mauvais type.
#[derive(Debug)]
struct Nul;

impl postgres::types::ToSql for Nul {
    fn to_sql(
        &self,
        _: &postgres::types::Type,
        _: &mut bytes::BytesMut,
    ) -> Result<postgres::types::IsNull, Box<dyn std::error::Error + Sync + Send>> {
        Ok(postgres::types::IsNull::Yes)
    }

    fn accepts(_: &postgres::types::Type) -> bool {
        true
    }

    postgres::types::to_sql_checked!();
}

fn params_pg(params: &[Valeur]) -> Vec<Box<dyn postgres::types::ToSql + Sync>> {
    params
        .iter()
        .map(|v| -> Box<dyn postgres::types::ToSql + Sync> {
            match v {
                Valeur::Nul => Box::new(Nul),
                Valeur::Entier(i) => Box::new(*i),
                Valeur::Reel(f) => Box::new(*f),
                Valeur::Texte(s) => Box::new(s.clone()),
                Valeur::Booleen(b) => Box::new(*b),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_placeholders_se_traduisent() {
        assert_eq!(
            traduire_parametres("SELECT * FROM a WHERE id = ?1 AND n > ?2"),
            "SELECT * FROM a WHERE id = $1 AND n > $2"
        );
    }

    #[test]
    fn un_point_d_interrogation_dans_un_texte_reste_intact() {
        // Le cas qui casserait un message affiche au commercant.
        let sql = "SELECT 'Continuer ?' , x FROM t WHERE id = ?1";
        assert_eq!(
            traduire_parametres(sql),
            "SELECT 'Continuer ?' , x FROM t WHERE id = $1"
        );
    }

    #[test]
    fn un_sql_sans_parametre_ne_bouge_pas() {
        let sql = "SELECT COUNT(*) FROM article";
        assert_eq!(traduire_parametres(sql), sql);
    }

    #[test]
    fn une_apostrophe_dans_un_commentaire_ne_bloque_pas_la_traduction() {
        // Le cas reel : un commentaire explicatif dans une requete du
        // tableau de bord, avec « L'oublier » dedans.
        let sql = "SELECT a\n  -- L'oublier compte deux fois\n  FROM t WHERE id = ?1";
        assert_eq!(
            traduire_parametres(sql),
            "SELECT a\n  -- L'oublier compte deux fois\n  FROM t WHERE id = $1"
        );
    }

    #[test]
    fn un_double_tiret_dans_un_texte_reste_du_texte() {
        let sql = "SELECT '--' FROM t WHERE id = ?1";
        assert_eq!(traduire_parametres(sql), "SELECT '--' FROM t WHERE id = $1");
    }

    #[test]
    fn les_nombres_a_deux_chiffres_passent() {
        assert_eq!(traduire_parametres("VALUES (?1,?10,?11)"), "VALUES ($1,$10,$11)");
    }
}
