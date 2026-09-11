//! Qui a le droit de faire quoi, et comment on le change.
//!
//! Les droits etaient codes en dur sur le NOM du role. Ajouter un
//! caissier demandait de recompiler, et donner une permission de plus a
//! UNE personne etait impossible.
//!
//! Un systeme de droits a deux facons de rater, et elles sont
//! opposees : laisser passer ce qui devait etre ferme, ou fermer ce qui
//! devait passer. La seconde est la plus probable le jour d'une mise a
//! jour — c'est elle qui ferait arriver un commercant devant une
//! application qui lui refuse tout.

use rusqlite::Connection;

use gescom_noyau::persistance;
use gescom_noyau::portes::{self, ContexteUtilisateur};

fn base() -> Connection {
    let conn = Connection::open_in_memory().expect("base en mémoire");
    persistance::initialiser_tables(&conn).expect("schéma");
    conn
}

/// Une base v1 : les roles existent, leurs permissions sont vides.
fn base_v1() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(include_str!("../src/persistance/schema.sql"))
        .unwrap();
    for role in ["patron", "employe"] {
        conn.execute(
            "INSERT INTO role (id, nom, permissions, cree_le, modifie_le, origine)
             VALUES (?1, ?1, '[]', '2026-01-01', '2026-01-01', 'test')",
            rusqlite::params![role],
        )
        .unwrap();
    }
    conn
}

fn utilisateur(conn: &Connection, id: &str, role: &str) {
    conn.execute(
        "INSERT OR IGNORE INTO role (id, nom, cree_le, modifie_le)
         VALUES (?1, ?1, '2026-01-01', '2026-01-01')",
        rusqlite::params![role],
    )
    .ok();
    // L'identifiant du role, pas son nom : les roles poses par la
    // migration ont un identifiant tire au hasard, et `role_id` est une
    // cle etrangere.
    let role_id: String = conn
        .query_row(
            "SELECT id FROM role WHERE nom = ?1",
            rusqlite::params![role],
            |r| r.get(0),
        )
        .unwrap_or_else(|_| role.to_string());
    conn.execute(
        "INSERT INTO utilisateur (id, nom, role_id, actif, cree_le, modifie_le, origine)
         VALUES (?1, ?1, ?2, 1, '2026-01-01', '2026-01-01', 'test')",
        rusqlite::params![id, role_id],
    )
    .unwrap();
}

fn ctx(id: &str, role: &str) -> ContexteUtilisateur {
    ContexteUtilisateur {
        id: id.into(),
        role: role.into(),
    }
}

fn peut(conn: &Connection, id: &str, role: &str, permission: &str) -> bool {
    portes::verifier_permission(conn, &ctx(id, role), permission).is_ok()
}

// =====================================================================
//  Le catalogue
// =====================================================================

#[test]
fn le_catalogue_n_a_pas_de_doublon() {
    let mut codes: Vec<&str> = portes::CATALOGUE.iter().map(|p| p.code).collect();
    let total = codes.len();
    codes.sort_unstable();
    codes.dedup();
    assert_eq!(codes.len(), total, "un code apparaît deux fois");
}

#[test]
fn chaque_permission_a_un_libelle_et_un_groupe() {
    // Une case a cocher sans texte lisible n'est pas une case a cocher.
    for p in portes::CATALOGUE {
        assert!(!p.libelle.trim().is_empty(), "{} sans libellé", p.code);
        assert!(!p.groupe.trim().is_empty(), "{} sans groupe", p.code);
        assert!(p.code.contains(':'), "{} mal formé", p.code);
    }
}

#[test]
fn une_permission_hors_catalogue_est_refusee_a_tous() {
    // Y compris au superadmin : elle ne correspond a aucune commande.
    // La laisser passer masquerait une faute de frappe cote code.
    let conn = base();
    assert!(!peut(&conn, "u", "superadmin", "compta:cloturer"));
    assert!(!peut(&conn, "u", "patron", "compta:cloturer"));
}

// =====================================================================
//  Le superadmin ne s'enferme pas dehors
// =====================================================================

#[test]
fn le_superadmin_passe_meme_sans_role_en_base() {
    // Meme si la table etait vide ou abimee, il entre. C'est le compte
    // de secours : sans cette garantie, une mauvaise manipulation sur
    // les roles rendrait l'application definitivement inadministrable.
    let conn = base();
    for p in portes::CATALOGUE {
        assert!(peut(&conn, "u", "superadmin", p.code), "{}", p.code);
    }
}

#[test]
fn on_ne_peut_pas_retirer_une_permission_au_superadmin() {
    let conn = base();
    utilisateur(&conn, "u1", "superadmin");
    conn.execute(
        "INSERT INTO utilisateur_permission
           (utilisateur_id, permission, accorde, cree_le)
         VALUES ('u1', 'ventes:creer', 0, '2026-01-01')",
        [],
    )
    .unwrap();
    assert!(peut(&conn, "u1", "superadmin", "ventes:creer"));
}

// =====================================================================
//  La reprise d'une base existante
// =====================================================================

/// Le scenario qui compte : la mise a jour ne doit retirer aucun droit.
///
/// Les roles existants ont `permissions = '[]'`. Basculer sur la
/// lecture en base sans les remplir retirerait TOUT a TOUT LE MONDE, le
/// matin de la mise a jour.
#[test]
fn la_reprise_ne_retire_aucun_droit() {
    let conn = base_v1();
    utilisateur(&conn, "p1", "patron");
    utilisateur(&conn, "e1", "employe");

    persistance::v2::migrer(&conn).unwrap();

    // Le patron pouvait tout, il peut toujours tout.
    for p in portes::CATALOGUE {
        assert!(peut(&conn, "p1", "patron", p.code), "patron : {}", p.code);
    }

    // L'employe retrouve exactement sa liste d'avant.
    for code in [
        "ventes:creer",
        "paiements:creer",
        "clients:creer",
        "articles:creer",
        "caisse:mouvementer",
        "pieces:creer",
        "retours:creer",
    ] {
        assert!(peut(&conn, "e1", "employe", code), "employé : {code}");
    }
    // Et ce qu'il ne pouvait pas, il ne le peut toujours pas.
    assert!(!peut(&conn, "e1", "employe", "utilisateurs:gerer"));
    assert!(!peut(&conn, "e1", "employe", "parametres:modifier"));
    assert!(!peut(&conn, "e1", "employe", "sauvegarde:lancer"));
}

#[test]
fn la_reprise_ajoute_les_nouveaux_roles() {
    let conn = base_v1();
    persistance::v2::migrer(&conn).unwrap();

    let noms: Vec<String> = {
        let mut st = conn.prepare("SELECT nom FROM role ORDER BY nom").unwrap();
        let v = st
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        v
    };
    for attendu in ["caissier", "comptable", "magasinier", "superadmin"] {
        assert!(noms.contains(&attendu.to_string()), "{attendu} absent : {noms:?}");
    }
}

#[test]
fn les_roles_livres_ont_des_permissions_du_catalogue() {
    // Une faute de frappe dans un role livre donnerait une case cochee
    // sans effet : le role paraitrait complet et ne le serait pas.
    let conn = base_v1();
    persistance::v2::migrer(&conn).unwrap();

    let mut st = conn
        .prepare("SELECT nom, permissions FROM role WHERE origine = 'migration'")
        .unwrap();
    let roles: Vec<(String, String)> = st
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    assert!(!roles.is_empty());

    for (nom, json) in roles {
        let codes: Vec<String> = serde_json::from_str(&json)
            .unwrap_or_else(|e| panic!("{nom} : JSON illisible ({e}) — {json}"));
        for code in codes {
            assert!(portes::existe(&code), "{nom} : « {code} » hors catalogue");
        }
    }
}

#[test]
fn la_reprise_ne_se_rejoue_pas() {
    // Rejouee, elle ecraserait les permissions qu'un commercant vient
    // d'ajuster.
    let conn = base_v1();
    utilisateur(&conn, "e1", "employe");
    persistance::v2::migrer(&conn).unwrap();

    // Le patron retire une permission a l'employe.
    conn.execute(
        r#"UPDATE role SET permissions = '["ventes:creer"]' WHERE nom = 'employe'"#,
        [],
    )
    .unwrap();
    assert!(!peut(&conn, "e1", "employe", "pieces:creer"));

    persistance::v2::migrer(&conn).unwrap();
    persistance::v2::migrer(&conn).unwrap();

    assert!(
        !peut(&conn, "e1", "employe", "pieces:creer"),
        "la reprise a rejoué et a écrasé le réglage du commerçant"
    );
}

// =====================================================================
//  Les roles livres
// =====================================================================

#[test]
fn le_caissier_encaisse_mais_n_administre_pas() {
    let conn = base_v1();
    persistance::v2::migrer(&conn).unwrap();
    utilisateur(&conn, "c1", "caissier");

    assert!(peut(&conn, "c1", "caissier", "ventes:creer"));
    assert!(peut(&conn, "c1", "caissier", "paiements:creer"));
    assert!(peut(&conn, "c1", "caissier", "caisse:mouvementer"));

    assert!(!peut(&conn, "c1", "caissier", "utilisateurs:gerer"));
    assert!(!peut(&conn, "c1", "caissier", "parametres:modifier"));
    assert!(!peut(&conn, "c1", "caissier", "depots:gerer"));
}

#[test]
fn le_magasinier_touche_au_stock_pas_a_l_argent() {
    let conn = base_v1();
    persistance::v2::migrer(&conn).unwrap();
    utilisateur(&conn, "m1", "magasinier");

    assert!(peut(&conn, "m1", "magasinier", "stock:transferer"));
    assert!(peut(&conn, "m1", "magasinier", "livraisons:enregistrer"));

    assert!(!peut(&conn, "m1", "magasinier", "paiements:creer"));
    assert!(!peut(&conn, "m1", "magasinier", "caisse:mouvementer"));
    assert!(!peut(&conn, "m1", "magasinier", "fournisseurs:regler"));
}

// =====================================================================
//  Les reglages personnels
// =====================================================================

#[test]
fn une_permission_s_ajoute_a_une_seule_personne() {
    // « Ce caissier-la fait aussi les retours fournisseurs. »
    let conn = base_v1();
    persistance::v2::migrer(&conn).unwrap();
    utilisateur(&conn, "c1", "caissier");
    utilisateur(&conn, "c2", "caissier");

    assert!(!peut(&conn, "c1", "caissier", "achats:creer"));

    conn.execute(
        "INSERT INTO utilisateur_permission
           (utilisateur_id, permission, accorde, cree_le)
         VALUES ('c1', 'achats:creer', 1, '2026-01-01')",
        [],
    )
    .unwrap();

    assert!(peut(&conn, "c1", "caissier", "achats:creer"));
    assert!(
        !peut(&conn, "c2", "caissier", "achats:creer"),
        "l'ajout ne doit toucher que la personne visée"
    );
}

#[test]
fn une_permission_se_retire_a_une_seule_personne() {
    // « Ce caissier-la ne touche pas au tiroir. » Fabriquer un role par
    // personne pour repondre a ca rendrait la liste illisible.
    let conn = base_v1();
    persistance::v2::migrer(&conn).unwrap();
    utilisateur(&conn, "c1", "caissier");

    assert!(peut(&conn, "c1", "caissier", "caisse:mouvementer"));
    conn.execute(
        "INSERT INTO utilisateur_permission
           (utilisateur_id, permission, accorde, cree_le)
         VALUES ('c1', 'caisse:mouvementer', 0, '2026-01-01')",
        [],
    )
    .unwrap();
    assert!(!peut(&conn, "c1", "caissier", "caisse:mouvementer"));
}

#[test]
fn un_reglage_personnel_hors_catalogue_ne_donne_rien() {
    let conn = base_v1();
    persistance::v2::migrer(&conn).unwrap();
    utilisateur(&conn, "c1", "caissier");
    conn.execute(
        "INSERT INTO utilisateur_permission
           (utilisateur_id, permission, accorde, cree_le)
         VALUES ('c1', 'compta:tout', 1, '2026-01-01')",
        [],
    )
    .unwrap();
    assert!(!peut(&conn, "c1", "caissier", "compta:tout"));
}

// =====================================================================
//  Un role fabrique par le commercant
// =====================================================================

#[test]
fn un_role_cree_a_la_main_fonctionne_sans_recompiler() {
    // C'est tout l'objet du changement : ajouter un role ne doit plus
    // demander de toucher au code.
    let conn = base_v1();
    persistance::v2::migrer(&conn).unwrap();
    conn.execute(
        r#"INSERT INTO role (id, nom, permissions, cree_le, modifie_le, origine)
           VALUES ('r-liv', 'livreur', '["livraisons:enregistrer"]',
                   '2026-01-01', '2026-01-01', 'app')"#,
        [],
    )
    .unwrap();
    utilisateur(&conn, "l1", "livreur");

    assert!(peut(&conn, "l1", "livreur", "livraisons:enregistrer"));
    assert!(!peut(&conn, "l1", "livreur", "ventes:creer"));
}

#[test]
fn un_role_inconnu_ne_peut_rien() {
    // Liste blanche : ce qui n'est pas accorde est refuse.
    let conn = base_v1();
    persistance::v2::migrer(&conn).unwrap();
    for p in portes::CATALOGUE {
        assert!(!peut(&conn, "x", "role-fantome", p.code), "{}", p.code);
    }
}
