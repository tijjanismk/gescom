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

use crate::base::{Base, Resultat, Valeur};
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
