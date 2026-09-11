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

/// Amorce une base neuve. Ne fait rien si elle ne l'est pas.
///
/// Rend `true` si l'amorcage a eu lieu.
pub fn amorcer(base: &mut Base) -> Resultat<bool> {
    creer_schema(base)?;
    tables_v2(base);
    colonnes_roles(base);

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
         VALUES (?1, 'Dépôt principal', 1, 1, ?2, ?2, 'amorcage')",
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
        .ok_or_else(|| crate::base::Erreur("Aucun dépôt par défaut".into()))?;

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

        // Le declencheur `stock_suit_les_mouvements` n'existe que cote
        // SQLite : il vient des migrations v2, pas du schema. Sur
        // PostgreSQL on pose donc le compteur ici, a partir du meme
        // mouvement — la somme et le compteur restent egaux.
        if base.est_postgres() {
            base.executer(
                "INSERT INTO stock_depot (id, article_id, depot_id, quantite)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT (article_id, depot_id)
                 DO UPDATE SET quantite = stock_depot.quantite + ?4",
                &parametres![
                    uuid::Uuid::new_v4().to_string(),
                    article_id,
                    depot.clone(),
                    *stock
                ],
            )?;
        }
    }

    Ok((nb_articles, clients.len()))
}
