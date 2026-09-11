//! Preparer une base neuve, sur SQLite comme sur PostgreSQL.
//!
//! ## Pourquoi un module a part
//!
//! `persistance` et `seed` parlent rusqlite directement — 739 points
//! d'appel dans le noyau, qui se porteront module par module. Celui-ci
//! passe par [`crate::base::Base`] et fonctionne donc des aujourd'hui
//! sur les deux moteurs.
//!
//! C'est la premiere tranche verticale du portage : creer le schema,
//! poser les roles, creer les comptes. Assez pour qu'une base
//! PostgreSQL neuve devienne utilisable et qu'on puisse s'y connecter.
//!
//! ## Ce que PostgreSQL ne comprend pas
//!
//! `INSERT OR IGNORE` est du SQLite. L'equivalent portable est
//! `ON CONFLICT DO NOTHING`, que les deux acceptent — c'est donc lui
//! qu'on ecrit. Le principe vaut pour tout le portage : quand une
//! forme marche des deux cotes, on l'ecrit une fois.

use crate::base::{Base, Resultat};
use crate::utils::maintenant_iso;
use crate::parametres;

/// Le schema, adapte au moteur.
///
/// `schema.sql` est dialect-neutre depuis l'etape 1 : les deux moteurs
/// l'acceptent. Restent DEUX ecarts de TYPE que SQLite tolere et que
/// PostgreSQL refuse net :
///
/// - `INTEGER` vaut 4 octets en PostgreSQL. Le noyau manipule des `i64`
///   partout — des francs CFA, des quantites, des horodatages — et
///   PostgreSQL refuse de ranger un entier 64 bits dans une colonne 32
///   bits. `BIGINT` leve l'ecart.
/// - `REAL` vaut 4 octets aussi. Les quantites sont des `f64`
///   (2,5 sacs) : `DOUBLE PRECISION`.
///
/// SQLite ignore ces largeurs — elles ne changent donc rien de son
/// cote, et on garde UN seul fichier de schema.
pub fn creer_schema(base: &mut Base) -> Resultat<()> {
    let schema = include_str!("persistance/schema.sql");
    if base.est_postgres() {
        base.executer_lot(&types_postgres(schema))
    } else {
        base.executer_lot(schema)
    }
}

/// Elargit les types entiers et flottants pour PostgreSQL.
///
/// On ne touche qu'aux declarations de colonnes : `CAST(x AS INTEGER)`
/// dans une requete n'est pas concerne, et le remplacer casserait des
/// expressions qui marchent.
fn types_postgres(schema: &str) -> String {
    schema
        .lines()
        .map(|ligne| {
            // Une declaration de colonne finit par une virgule ou une
            // contrainte ; une expression SQL, non.
            if ligne.contains("CAST(") || ligne.trim_start().starts_with("--") {
                return ligne.to_string();
            }
            ligne
                .replace("INTEGER", "BIGINT")
                .replace("REAL", "DOUBLE PRECISION")
        })
        .collect::<Vec<_>>()
        .join("
")
}

/// Y a-t-il un moyen d'entrer ?
///
/// « Vide » ne veut pas dire « aucune ligne » : les migrations posent
/// des roles livres. Compter les roles avait fait croire qu'une base
/// neuve etait deja amorcee, et personne ne pouvait se connecter.
pub fn base_est_vide(base: &mut Base) -> Resultat<bool> {
    let n = base
        .lire_une("SELECT COUNT(*) FROM utilisateur_auth", &[], |r| r.get::<i64>(0))?
        .unwrap_or(0);
    Ok(n == 0)
}

/// Les roles livres et leurs permissions.
///
/// La meme liste que la migration SQLite : les deux chemins doivent
/// produire la meme boutique.
fn roles() -> Vec<(&'static str, &'static str, bool, bool, &'static str)> {
    vec![
        (
            "superadmin",
            "Accès complet, non modifiable. Le compte de secours.",
            true,
            true,
            "[]",
        ),
        ("patron", "Accès complet à la boutique.", true, false, "[]"),
        (
            "employe",
            "Vend, encaisse, crée clients et articles.",
            false,
            false,
            r#"["ventes:creer","paiements:creer","clients:creer","clients:modifier","articles:creer","caisse:mouvementer","pieces:creer","retours:creer"]"#,
        ),
        (
            "caissier",
            "Encaisse, rend la monnaie, enregistre les retours.",
            false,
            false,
            r#"["ventes:creer","paiements:creer","retours:creer","clients:creer","caisse:mouvementer","pieces:creer"]"#,
        ),
        (
            "magasinier",
            "Tient le stock, reçoit et livre la marchandise.",
            false,
            false,
            r#"["articles:creer","stock:transferer","depots:gerer","livraisons:enregistrer","achats:creer"]"#,
        ),
        (
            "comptable",
            "Créances, chèques, TVA, règlements fournisseurs.",
            false,
            false,
            r#"["creances:gerer","cheques:gerer","fournisseurs:regler","avoirs:gerer","chantiers:gerer"]"#,
        ),
    ]
}

/// Les tables et colonnes ajoutees par la v2.
///
/// Elles vivent dans `persistance/v2.rs`, qui parle encore rusqlite.
/// Tant qu'il n'est pas porte, l'amorcage les pose lui-meme — sinon une
/// base PostgreSQL neuve n'a NI poste NI session, et la premiere
/// connexion reseau echoue sur « la relation poste n'existe pas ».
fn tables_v2(base: &mut Base) {
    let pg = base.est_postgres();
    let adapter = |sql: &str| if pg { types_postgres(sql) } else { sql.to_string() };

    for sql in [
        "CREATE TABLE IF NOT EXISTS poste (
            id              TEXT PRIMARY KEY,
            nom             TEXT NOT NULL,
            empreinte       TEXT NOT NULL UNIQUE,
            genre           TEXT NOT NULL DEFAULT 'caisse',
            actif           INTEGER NOT NULL DEFAULT 1,
            dernier_contact TEXT,
            derniere_ip     TEXT,
            cree_le         TEXT NOT NULL,
            modifie_le      TEXT NOT NULL
         )",
        "CREATE TABLE IF NOT EXISTS session_reseau (
            id              TEXT PRIMARY KEY,
            jeton_hash      TEXT NOT NULL UNIQUE,
            poste_id        TEXT NOT NULL REFERENCES poste(id),
            utilisateur_id  TEXT NOT NULL REFERENCES utilisateur(id),
            ouvert_le       TEXT NOT NULL,
            expire_le       TEXT NOT NULL,
            revoque_le      TEXT,
            revoque_par     TEXT,
            derniere_vue    TEXT
         )",
        "CREATE TABLE IF NOT EXISTS modele_document (
            id           TEXT PRIMARY KEY,
            genre        TEXT NOT NULL,
            nom          TEXT NOT NULL,
            format       TEXT NOT NULL,
            contenu      TEXT NOT NULL,
            est_defaut   INTEGER NOT NULL DEFAULT 0,
            actif        INTEGER NOT NULL DEFAULT 0,
            cree_le      TEXT NOT NULL,
            modifie_le   TEXT NOT NULL,
            modifie_par  TEXT
         )",
        "ALTER TABLE session_caisse ADD COLUMN poste_id TEXT",
        "ALTER TABLE session_caisse ADD COLUMN utilisateur_id TEXT",
        "ALTER TABLE ligne_piece ADD COLUMN quantite_livree REAL NOT NULL DEFAULT 0",
    ] {
        let _ = base.executer(&adapter(sql), &[]);
    }

    // Le tiroir reste unique par defaut : c'est le comportement de la
    // boutique type, et l'imposer nominatif obligerait chaque
    // commercant a ouvrir deux caisses pour un seul comptoir.
    let _ = base.executer(
        "INSERT INTO config_app (cle, valeur) VALUES ('caisse_par_utilisateur', '0')
         ON CONFLICT (cle) DO NOTHING",
        &[],
    );
}

/// Les colonnes de roles ajoutees par la v2.
///
/// `schema.sql` decrit la table d'origine ; ces colonnes sont venues
/// avec les permissions. Sur SQLite elles arrivent par `ALTER TABLE`
/// dans les migrations — ici on les pose de la meme facon, et l'echec
/// est ignore : une colonne deja presente n'est pas une erreur.
fn colonnes_roles(base: &mut Base) {
    // Le DDL ecrit ici suit la MEME traduction de types que
    // `schema.sql`. Sans elle, ces colonnes naissaient en `int4` et
    // PostgreSQL refusait le premier `i64` qu'on leur presentait — avec
    // « error serializing parameter 3 », un message qui ne nomme pas la
    // colonne fautive et fait perdre une heure.
    let pg = base.est_postgres();
    let adapter = |sql: &str| if pg { types_postgres(sql) } else { sql.to_string() };

    for sql in [
        "ALTER TABLE role ADD COLUMN acces_total INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE role ADD COLUMN protege INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE role ADD COLUMN description TEXT",
        "CREATE TABLE IF NOT EXISTS utilisateur_permission (
            utilisateur_id TEXT NOT NULL REFERENCES utilisateur(id),
            permission     TEXT NOT NULL,
            accorde        INTEGER NOT NULL DEFAULT 1,
            cree_le        TEXT NOT NULL,
            cree_par       TEXT,
            PRIMARY KEY (utilisateur_id, permission)
         )",
        "CREATE TABLE IF NOT EXISTS compteur_piece (
            cle     TEXT PRIMARY KEY,
            dernier INTEGER NOT NULL
         )",
    ] {
        // L'echec est ignore : une colonne deja presente n'est pas une
        // erreur, c'est le cas normal au deuxieme demarrage.
        let _ = base.executer(&adapter(sql), &[]);
    }
}

/// Rattrape l'etiquette du magasin d'usine sur les bases deja amorcees.
///
/// A l'ecran, « depot » s'appelle desormais « magasin ». Les bases
/// creees avant portent encore `Depot principal`, et un ecran qui dit
/// « magasin » partout sauf dans sa propre liste deroulante donne
/// l'impression que ce sont deux choses differentes.
///
/// La condition sur le nom EXACT est le garde-fou : un commercant qui a
/// renomme son magasin garde son nom. On ne corrige que l'etiquette
/// qu'on avait posee soi-meme.
///
/// L'entite du code, elle, s'appelle toujours `depot`. Le renommage
/// jusque dans la base touche ~200 requetes et demande une migration :
/// il est prevu, il n'est pas fait ici.
/// Reaffirme l'acces total de `patron` et `superadmin`, a CHAQUE
/// amorcage — meme sur une base deja peuplee.
///
/// `INSERT ... ON CONFLICT (nom) DO NOTHING` (plus bas) ne pose ces
/// deux roles qu'une fois : la premiere ecriture gagne pour toujours,
/// y compris si elle datait d'une version du code qui posait encore
/// `acces_total = 0`. Sans cette correction, le patron d'une base
/// amorcee tot dans le developpement de ce module resterait sans
/// permissions pour toujours — un compte qui semble exister mais ne
/// peut plus rien faire, et rien dans l'ecran ne dit pourquoi.
///
/// Pas de "reprise, une seule fois" ici (contrairement a
/// `cles_de_compteur`) : il n'y a rien a cumuler, seulement une
/// valeur a garantir. La rejouer a chaque demarrage ne coute rien et
/// ferme la porte pour de bon.
fn acces_total_toujours_reaffirme(base: &mut Base) {
    let _ = base.executer(
        "UPDATE role SET acces_total = 1 WHERE nom IN ('patron', 'superadmin')",
        &[],
    );
}

fn etiquette_magasin(base: &mut Base) {
    let _ = base.executer(
        "UPDATE depot SET nom = 'Magasin principal'
         WHERE nom = 'Dépôt principal' AND est_defaut = 1",
        &[],
    );
}

/// Pose le cloisonnement par dossier.
///
/// Idempotent comme le reste : les `ALTER` echouent en silence quand la
/// colonne existe deja, ce qui est le cas normal au deuxieme demarrage.
///
/// ## Pourquoi une valeur par defaut, et laquelle
///
/// Les bases existantes ont des lignes. Une colonne `NOT NULL` sans
/// defaut les rejetterait toutes. Le defaut les rattache donc au
/// dossier d'origine — ce qui est exactement la verite : elles ont ete
/// ecrites du temps ou il n'y en avait qu'un.
///
/// Sur PostgreSQL, le defaut n'est pas une constante mais
/// `gescom_dossier()`, qui lit le dossier de la session. **Une
/// insertion tombe donc dans le bon dossier sans que l'appelant ait a
/// le dire.** C'est ce qui evite de rouvrir les 480 `params!` du
/// projet pour y glisser un argument de plus — et chaque endroit qu'on
/// ne rouvre pas est un endroit qu'on ne casse pas.
fn cloisonnement(base: &mut Base) {
    let pg = base.est_postgres();
    let defaut = crate::dossiers::DOSSIER_DEFAUT;

    if pg {
        // `STABLE` et non `VOLATILE` : la valeur ne change pas dans une
        // requete, et PostgreSQL peut l'evaluer une seule fois.
        let _ = base.executer_lot(&format!(
            "CREATE OR REPLACE FUNCTION gescom_dossier() RETURNS text
             LANGUAGE sql STABLE AS $$
               SELECT COALESCE(
                        NULLIF(current_setting('gescom.dossier', true), ''),
                        '{defaut}')
             $$;"
        ));
    }

    let _ = base.executer(
        "CREATE TABLE IF NOT EXISTS dossier (
            id                TEXT PRIMARY KEY,
            code              TEXT NOT NULL UNIQUE,
            societe           TEXT NOT NULL,
            clos              INTEGER NOT NULL DEFAULT 0,
            cree_le           TEXT NOT NULL,
            modifie_le        TEXT NOT NULL
         )",
        &[],
    );

    // L'exercice a sa propre table (chapitre 4 du plan multi-societe) :
    // un dossier a UN stock continu mais PLUSIEURS exercices qui se
    // suivent. Une colonne sur `dossier` n'aurait pu en porter qu'un
    // seul a la fois.
    // Pas de `REFERENCES dossier(id)` : aucune autre table cloisonnee
    // n'en porte — un dossier-b utilise avant d'avoir sa ligne `dossier`
    // (un test, une migration en cours) ne doit pas etre bloque ici
    // alors qu'il ne l'est nulle part ailleurs.
    let _ = base.executer(
        "CREATE TABLE IF NOT EXISTS exercice (
            id                TEXT PRIMARY KEY,
            dossier_id        TEXT NOT NULL,
            date_debut        TEXT NOT NULL,
            date_fin          TEXT NOT NULL,
            prolonge_jusqu_au TEXT,
            clos              INTEGER NOT NULL DEFAULT 0,
            cree_le           TEXT NOT NULL,
            modifie_le        TEXT NOT NULL
         )",
        &[],
    );

    // Le dossier d'origine. Son identifiant est fixe : c'est lui que
    // portent les lignes deja ecrites.
    let annee = maintenant_iso().chars().take(4).collect::<String>();
    let now = maintenant_iso();
    let _ = base.executer(
        "INSERT INTO dossier (id, code, societe, clos, cree_le, modifie_le)
         VALUES (?1, 'PRINCIPAL', 'Ma boutique', 0, ?2, ?2)
         ON CONFLICT (id) DO NOTHING",
        &parametres![defaut, now.clone()],
    );

    // Son premier exercice, l'annee en cours. `NOT EXISTS` et non
    // `ON CONFLICT` : un dossier peut deja avoir un exercice ouvert par
    // l'ecran (pas encore le cas ici, mais le deuxieme amorcage ne doit
    // pas en recreer un identique).
    let _ = base.executer(
        "INSERT INTO exercice (id, dossier_id, date_debut, date_fin, clos, cree_le, modifie_le)
         SELECT ?1, ?2, ?3, ?4, 0, ?5, ?5
         WHERE NOT EXISTS (SELECT 1 FROM exercice WHERE dossier_id = ?2)",
        &parametres![
            uuid::Uuid::new_v4().to_string(),
            defaut,
            format!("{annee}-01-01"),
            format!("{annee}-12-31"),
            now
        ],
    );

    let expression_defaut = if pg { "gescom_dossier()".to_string() } else { format!("'{defaut}'") };
    for table in crate::dossiers::TABLES_CLOISONNEES {
        let _ = base.executer(
            &format!(
                "ALTER TABLE {table} ADD COLUMN dossier_id TEXT NOT NULL \
                 DEFAULT {expression_defaut}"
            ),
            &[],
        );
        // Sans index, chaque lecture cloisonnee devient un parcours
        // complet — et le filtre qu'on vient d'imposer couterait plus
        // cher que la separation qu'il apporte.
        let _ = base.executer(
            &format!("CREATE INDEX IF NOT EXISTS idx_{table}_dossier ON {table}(dossier_id)"),
            &[],
        );
    }

    cles_de_compteur(base);
}

/// Rattache au dossier d'origine les compteurs deja en place.
///
/// **Sans cette migration, la numerotation repartirait a 1.** Les cles
/// existantes s'appellent `FAC-2026` ; le code cherche desormais
/// `<dossier>:FAC-2026`, ne le trouve pas, et refabrique FAC-2026-00001
/// — un numero deja emis. La contrainte UNIQUE bloquerait alors la
/// premiere vente du matin de la mise a jour, au comptoir, devant le
/// client.
///
/// Le `NOT LIKE '%:%'` rend la migration rejouable : une cle deja
/// prefixee ne l'est pas deux fois.
fn cles_de_compteur(base: &mut Base) {
    let _ = base.executer(
        "UPDATE compteur_piece SET cle = ?1 || cle WHERE cle NOT LIKE '%:%'",
        &parametres![format!("{}:", crate::dossiers::DOSSIER_DEFAUT)],
    );
}

/// Le stock est une CONSEQUENCE de ses mouvements, jamais ecrit en
/// direct (voir livraison-stock.md). `mouvement_stock` n'a ni UPDATE ni
/// DELETE : c'est ce qui rend un simple `AFTER INSERT` suffisant sur
/// les deux moteurs.
///
/// Ce declencheur existait deja cote SQLite, mais seulement dans
/// `persistance/v2.rs` — le chemin de la fenetre, jamais rejoue par ce
/// module. Une base amorcee UNIQUEMENT par `Base` (tous les tests, et
/// demain le serveur) n'avait donc aucun declencheur, et `stock_depot`
/// restait a zero apres chaque mouvement — une base qui semble vendre
/// mais dont le stock ne bouge jamais. Trouve en ecrivant les
/// scenarios de `creer_vente_sur_base` / `valider_facture_sur_base`.
///
/// PostgreSQL n'en avait pas non plus : `donnees_demo` ecrivait
/// `stock_depot` a la main pour compenser (voir l'historique de ce
/// fichier). Poser le meme declencheur sur les deux moteurs supprime
/// ce contournement — et le risque qu'un futur point d'ecriture
/// PostgreSQL oublie de le refaire.
fn trigger_stock(base: &mut Base) {
    if base.est_postgres() {
        let _ = base.executer_lot(
            "CREATE OR REPLACE FUNCTION gescom_stock_suit_les_mouvements()
             RETURNS trigger LANGUAGE plpgsql AS $$
             BEGIN
                 INSERT INTO stock_depot (id, article_id, depot_id, quantite)
                 VALUES (md5(random()::text || clock_timestamp()::text),
                         NEW.article_id, NEW.depot_id, NEW.quantite_delta)
                 ON CONFLICT (article_id, depot_id)
                 DO UPDATE SET quantite = stock_depot.quantite + NEW.quantite_delta;
                 RETURN NEW;
             END;
             $$;
             DROP TRIGGER IF EXISTS stock_suit_les_mouvements ON mouvement_stock;
             CREATE TRIGGER stock_suit_les_mouvements
                 AFTER INSERT ON mouvement_stock
                 FOR EACH ROW EXECUTE FUNCTION gescom_stock_suit_les_mouvements();",
        );
    } else {
        let _ = base.executer_lot(
            "CREATE TRIGGER IF NOT EXISTS stock_suit_les_mouvements
             AFTER INSERT ON mouvement_stock
             BEGIN
               INSERT INTO stock_depot (id, article_id, depot_id, quantite)
               VALUES (lower(hex(randomblob(16))), NEW.article_id, NEW.depot_id,
                       NEW.quantite_delta)
               ON CONFLICT(article_id, depot_id)
               DO UPDATE SET quantite = quantite + NEW.quantite_delta;
             END;",
        );
    }
}

/// Amorce une base neuve. Ne fait rien si elle ne l'est pas.
///
/// Rend `true` si l'amorcage a eu lieu.
pub fn amorcer(base: &mut Base) -> Resultat<bool> {
    creer_schema(base)?;
    tables_v2(base);
    colonnes_roles(base);
    acces_total_toujours_reaffirme(base);
    etiquette_magasin(base);
    cloisonnement(base);
    trigger_stock(base);

    if !base_est_vide(base)? {
        return Ok(false);
    }

    let now = maintenant_iso();

    // ---- Roles ----
    for (nom, description, acces_total, protege, permissions) in roles() {
        base.executer(
            "INSERT INTO role
               (id, nom, permissions, acces_total, protege, description,
                cree_le, modifie_le, origine)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7, 'amorcage')
             ON CONFLICT (nom) DO NOTHING",
            &parametres![
                uuid::Uuid::new_v4().to_string(),
                nom,
                permissions,
                acces_total as i64,
                protege as i64,
                description,
                now.clone()
            ],
        )?;
    }

    // ---- Comptes ----
    // Le patron et un employe. Les deux doivent changer leur mot de
    // passe a la premiere connexion : un mot de passe d'usine qui
    // reste en place est un mot de passe public.
    for (pseudo, email, nom_affiche, role, mdp) in [
        ("admin", "admin@gescom.ml", "Patron", "patron", "admin123"),
        ("employe", "employe@gescom.ml", "Employé", "employe", "employe123"),
    ] {
        let role_id: String = base
            .lire_une(
                "SELECT id FROM role WHERE nom = ?1",
                &parametres![role],
                |r| r.get::<String>(0),
            )?
            .ok_or_else(|| crate::base::Erreur(format!("Rôle « {role} » absent")))?;

        let utilisateur_id = uuid::Uuid::new_v4().to_string();
        base.executer(
            "INSERT INTO utilisateur
               (id, nom, role_id, actif, cree_le, modifie_le, origine)
             VALUES (?1, ?2, ?3, 1, ?4, ?4, 'amorcage')",
            &parametres![utilisateur_id.clone(), nom_affiche, role_id, now.clone()],
        )?;

        let hash = crate::auth::hasher_mot_de_passe_pub(mdp)
            .map_err(crate::base::Erreur)?;
        base.executer(
            "INSERT INTO utilisateur_auth
               (utilisateur_id, pseudo, email, mot_de_passe, doit_changer_mdp)
             VALUES (?1, ?2, ?3, ?4, 1)",
            &parametres![utilisateur_id, pseudo, email, hash],
        )?;
    }

    // ---- Le minimum pour vendre ----
    // Sans depot par defaut, aucune vente ne sort de stock ; sans
    // client generique, la vente au comptoir n'a personne a qui
    // s'attacher.
    base.executer(
        "INSERT INTO depot (id, nom, est_defaut, actif, cree_le, modifie_le, origine)
         VALUES (?1, 'Magasin principal', 1, 1, ?2, ?2, 'amorcage')",
        &parametres![uuid::Uuid::new_v4().to_string(), now.clone()],
    )?;

    base.executer(
        "INSERT INTO client
           (id, code, nom, est_generique, actif, cree_le, modifie_le, origine)
         VALUES (?1, 'CLIENT00000', 'Comptant', 1, 1, ?2, ?2, 'amorcage')",
        &parametres![uuid::Uuid::new_v4().to_string(), now.clone()],
    )?;

    base.executer(
        "INSERT INTO parametres_societe (id, nom, devise, modifie_le)
         VALUES (1, 'Ma boutique', 'FCFA', ?1)
         ON CONFLICT (id) DO NOTHING",
        &parametres![now],
    )?;

    Ok(true)
}

/// Les comptes crees, pour que le serveur les annonce au demarrage.
pub const COMPTES_USINE: &[(&str, &str, &str)] = &[
    ("admin", "admin123", "patron"),
    ("employe", "employe123", "employé"),
];

// =====================================================================
//  LES DONNEES DE DEMONSTRATION
// =====================================================================

/// Un catalogue de quoi essayer : articles, unites, stock, clients.
///
/// Créées UNIQUEMENT si `GESCOM_DEMO=1`, meme regle que cote SQLite.
/// Chez un commercant, ces articles seraient a supprimer un par un —
/// on ne les cree donc jamais sans le demander.
///
/// Elles servent aussi a autre chose : elles font passer la facade sur
/// bien plus de tables que l'amorcage minimal, et c'est ainsi qu'on
/// trouve les ecarts de dialecte avant un commercant.
pub fn donnees_demo(base: &mut Base) -> Resultat<(usize, usize)> {
    let now = maintenant_iso();

    // ---- Clients ----
    let clients = [
        ("CLIENT00001", "Amadou Diarra", Some("76000001")),
        ("CLIENT00002", "Fatoumata Koné", Some("65000002")),
        ("CLIENT00003", "Ibrahim Traoré", Some("70000003")),
        ("CLIENT00004", "Mariam Coulibaly", None),
    ];
    for (code, nom, tel) in clients {
        base.executer(
            "INSERT INTO client
               (id, code, nom, telephone, est_generique, actif,
                cree_le, modifie_le, origine)
             VALUES (?1, ?2, ?3, ?4, 0, 1, ?5, ?5, 'demo')
             ON CONFLICT (code) DO NOTHING",
            &parametres![
                uuid::Uuid::new_v4().to_string(),
                code,
                nom,
                tel.map(|t| t.to_string()),
                now.clone()
            ],
        )?;
    }

    // ---- Categories ----
    let mut categories = Vec::new();
    for nom in ["Alimentation", "Hygiène", "Boissons"] {
        let id = uuid::Uuid::new_v4().to_string();
        base.executer(
            "INSERT INTO categorie
               (id, nom, schema_attributs, actif, cree_le, modifie_le, origine)
             VALUES (?1, ?2, '[]', 1, ?3, ?3, 'demo')",
            &parametres![id.clone(), nom, now.clone()],
        )?;
        categories.push(id);
    }

    // ---- Le depot ou poser le stock ----
    let depot: String = base
        .lire_une(
            "SELECT id FROM depot WHERE est_defaut = 1 LIMIT 1",
            &[],
            |r| r.get::<String>(0),
        )?
        .ok_or_else(|| crate::base::Erreur("Aucun magasin par défaut".into()))?;

    // ---- Articles, unites, stock ----
    // (nom, categorie, unite de base, [(libelle, facteur, prix)], stock)
    let articles: Vec<(&str, usize, &str, Vec<(&str, f64, i64)>, f64)> = vec![
        ("Sucre", 0, "kg", vec![("kg", 1.0, 800), ("sac 50kg", 50.0, 35000)], 200.0),
        ("Riz local", 0, "kg", vec![("kg", 1.0, 600), ("sac 25kg", 25.0, 13500)], 150.0),
        ("Huile végétale", 0, "litre", vec![("litre", 1.0, 1200), ("bidon 20L", 20.0, 22000)], 80.0),
        ("Lait en poudre", 0, "boîte", vec![("boîte", 1.0, 2500)], 40.0),
        ("Savon de Marseille", 1, "pièce", vec![("pièce", 1.0, 350), ("carton 48", 48.0, 15000)], 300.0),
        ("Eau de javel", 1, "litre", vec![("litre", 1.0, 500)], 60.0),
        ("Coca-Cola 33cl", 2, "unité", vec![("bouteille", 1.0, 600), ("casier 24", 24.0, 12000)], 48.0),
        ("Eau minérale 1,5L", 2, "unité", vec![("bouteille", 1.0, 400), ("pack 6", 6.0, 2200)], 120.0),
    ];

    let mut nb_articles = 0;
    for (nom, cat, unite_base, unites, stock) in &articles {
        let article_id = uuid::Uuid::new_v4().to_string();
        base.executer(
            "INSERT INTO article
               (id, nom, categorie_id, unite_base, gere_en_stock, attributs,
                actif, cree_le, modifie_le, origine)
             VALUES (?1, ?2, ?3, ?4, 1, '{}', 1, ?5, ?5, 'demo')",
            &parametres![
                article_id.clone(),
                *nom,
                categories[*cat].clone(),
                *unite_base,
                now.clone()
            ],
        )?;
        nb_articles += 1;

        for (libelle, facteur, prix) in unites {
            base.executer(
                "INSERT INTO unite_vente
                   (id, article_id, libelle, facteur, prix_reference, actif,
                    cree_le, modifie_le, origine)
                 VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?6, 'demo')",
                &parametres![
                    uuid::Uuid::new_v4().to_string(),
                    article_id.clone(),
                    *libelle,
                    *facteur,
                    *prix,
                    now.clone()
                ],
            )?;
        }

        // Le stock passe par un MOUVEMENT, jamais par le compteur : il
        // est un cache tenu par un declencheur, et l'ecrire en direct
        // ferait diverger le stock de son historique (voir
        // livraison-stock.md).
        base.executer(
            "INSERT INTO mouvement_stock
               (id, article_id, depot_id, type_mouvement, quantite_delta,
                motif, auteur_id, date_mouvement, cree_le, cree_par, origine)
             VALUES (?1, ?2, ?3, 'entree', ?4, 'Stock initial (démo)',
                     'demo', ?5, ?5, 'demo', 'demo')",
            &parametres![
                uuid::Uuid::new_v4().to_string(),
                article_id.clone(),
                depot.clone(),
                *stock,
                now.clone()
            ],
        )?;

        // `stock_suit_les_mouvements` (pose par `trigger_stock`, sur les
        // deux moteurs) met `stock_depot` a jour tout seul : plus besoin
        // de l'ecrire ici a la main, ni de distinguer les moteurs.
    }

    Ok((nb_articles, clients.len()))
}
