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

impl DeLigne for i64 {
    fn extraire(l: &Ligne<'_>, i: usize) -> Resultat<Self> {
        match l {
            Ligne::Sqlite(r) => Ok(r.get::<_, i64>(i)?),
            Ligne::Pg(r) => Ok(r.try_get::<_, i64>(i)?),
        }
    }
}

impl DeLigne for f64 {
    fn extraire(l: &Ligne<'_>, i: usize) -> Resultat<Self> {
        match l {
            Ligne::Sqlite(r) => Ok(r.get::<_, f64>(i)?),
            Ligne::Pg(r) => Ok(r.try_get::<_, f64>(i)?),
        }
    }
}

impl DeLigne for bool {
    fn extraire(l: &Ligne<'_>, i: usize) -> Resultat<Self> {
        match l {
            // SQLite n'a pas de booleen : 0 ou 1, comme partout ailleurs
            // dans cette base.
            Ligne::Sqlite(r) => Ok(r.get::<_, i64>(i)? != 0),
            Ligne::Pg(r) => Ok(r.try_get::<_, bool>(i)?),
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
//  LA BASE
// =====================================================================

/// La connexion, quel que soit le moteur.
pub enum Base {
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
pub enum Transaction<'a> {
    Sqlite(rusqlite::Transaction<'a>),
    Pg(postgres::Transaction<'a>),
}

impl Transaction<'_> {
    pub fn executer(&mut self, sql: &str, params: &[Valeur]) -> Resultat<u64> {
        match self {
            Transaction::Sqlite(t) => sqlite_executer(t, sql, params),
            Transaction::Pg(t) => pg_executer(t, sql, params),
        }
    }

    pub fn lire_une<T>(
        &mut self,
        sql: &str,
        params: &[Valeur],
        lire: impl FnOnce(&Ligne<'_>) -> Resultat<T>,
    ) -> Resultat<Option<T>> {
        match self {
            Transaction::Sqlite(t) => sqlite_lire_une(t, sql, params, lire),
            Transaction::Pg(t) => pg_lire_une(t, sql, params, lire),
        }
    }

    pub fn lire_plusieurs<T>(
        &mut self,
        sql: &str,
        params: &[Valeur],
        lire: impl FnMut(&Ligne<'_>) -> Resultat<T>,
    ) -> Resultat<Vec<T>> {
        match self {
            Transaction::Sqlite(t) => sqlite_lire_plusieurs(t, sql, params, lire),
            Transaction::Pg(t) => pg_lire_plusieurs(t, sql, params, lire),
        }
    }

    /// Ecrit tout, pour de bon.
    pub fn valider(self) -> Resultat<()> {
        match self {
            Transaction::Sqlite(t) => Ok(t.commit()?),
            Transaction::Pg(t) => Ok(t.commit()?),
        }
    }
}

impl Base {
    /// Ouvre SQLite (un chemin) ou PostgreSQL (une URL).
    ///
    /// L'URL decide : c'est le seul reglage, et il tient dans la ligne
    /// de commande du serveur.
    pub fn ouvrir(cible: &str) -> Resultat<Self> {
        if cible.starts_with("postgres://") || cible.starts_with("postgresql://") {
            let client = postgres::Client::connect(cible, postgres::NoTls)?;
            Ok(Base::Pg(Box::new(client)))
        } else {
            let conn = rusqlite::Connection::open(cible)?;
            conn.execute_batch(
                "PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON; \
                 PRAGMA busy_timeout=5000;",
            )?;
            Ok(Base::Sqlite(conn))
        }
    }

    pub fn est_postgres(&self) -> bool {
        matches!(self, Base::Pg(_))
    }

    /// Le nom du moteur, pour les messages de demarrage.
    pub fn moteur(&self) -> &'static str {
        match self {
            Base::Sqlite(_) => "SQLite",
            Base::Pg(_) => "PostgreSQL",
        }
    }

    /// Une ecriture. Rend le nombre de lignes touchees.
    pub fn executer(&mut self, sql: &str, params: &[Valeur]) -> Resultat<u64> {
        match self {
            Base::Sqlite(c) => sqlite_executer(c, sql, params),
            Base::Pg(c) => pg_executer(c.as_mut(), sql, params),
        }
    }

    /// Ouvre une transaction : tout ou rien.
    pub fn transaction(&mut self) -> Resultat<Transaction<'_>> {
        match self {
            Base::Sqlite(c) => Ok(Transaction::Sqlite(c.transaction()?)),
            Base::Pg(c) => Ok(Transaction::Pg(c.transaction()?)),
        }
    }

    /// Plusieurs ordres d'un coup, sans parametre. Pour le schema.
    pub fn executer_lot(&mut self, sql: &str) -> Resultat<()> {
        match self {
            Base::Sqlite(c) => Ok(c.execute_batch(sql)?),
            Base::Pg(c) => Ok(c.batch_execute(sql)?),
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
        match self {
            Base::Sqlite(c) => sqlite_lire_une(c, sql, params, lire),
            Base::Pg(c) => pg_lire_une(c.as_mut(), sql, params, lire),
        }
    }

    /// Toutes les lignes.
    pub fn lire_plusieurs<T>(
        &mut self,
        sql: &str,
        params: &[Valeur],
        lire: impl FnMut(&Ligne<'_>) -> Resultat<T>,
    ) -> Resultat<Vec<T>> {
        match self {
            Base::Sqlite(c) => sqlite_lire_plusieurs(c, sql, params, lire),
            Base::Pg(c) => pg_lire_plusieurs(c.as_mut(), sql, params, lire),
        }
    }

    /// La connexion SQLite en ecriture, pour batir un `Contexte`.
    ///
    /// C'est le point de passage de la migration : le serveur detient
    /// une `Base`, et les 187 poignees qui parlent encore rusqlite
    /// recoivent la connexion d'ici. Sur PostgreSQL il n'y en a pas, et
    /// l'appelant doit le dire clairement plutot que d'echouer sur un
    /// message incomprehensible.
    pub fn sqlite_mut(&mut self) -> Option<&mut rusqlite::Connection> {
        match self {
            Base::Sqlite(c) => Some(c),
            Base::Pg(_) => None,
        }
    }

    /// La connexion SQLite, pour tout ce qui n'est pas encore porte.
    ///
    /// C'est le point de passage assume pendant la migration : les
    /// modules qui parlent encore rusqlite directement continuent de
    /// fonctionner. Rend `None` sur PostgreSQL — et l'appelant doit
    /// alors dire clairement que la fonction n'est pas portee, plutot
    /// que d'echouer sur un message incomprehensible.
    pub fn sqlite(&self) -> Option<&rusqlite::Connection> {
        match self {
            Base::Sqlite(c) => Some(c),
            Base::Pg(_) => None,
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

fn params_pg(params: &[Valeur]) -> Vec<Box<dyn postgres::types::ToSql + Sync>> {
    params
        .iter()
        .map(|v| -> Box<dyn postgres::types::ToSql + Sync> {
            match v {
                Valeur::Nul => Box::new(Option::<String>::None),
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
    fn les_nombres_a_deux_chiffres_passent() {
        assert_eq!(traduire_parametres("VALUES (?1,?10,?11)"), "VALUES ($1,$10,$11)");
    }
}
