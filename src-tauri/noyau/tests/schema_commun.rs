//! Le filet : la base que la FENÊTRE prépare et celle que `Base`
//! amorce doivent avoir les mêmes colonnes.
//!
//! Deux chemins créent une base Gescom : `persistance::initialiser_tables`
//! (la fenêtre, SQLite seulement, avec ses `ALTER TABLE ADD COLUMN`
//! accumulés au fil des versions) et `amorcage::amorcer` (le serveur,
//! les deux moteurs, depuis `schema.sql`). Six fois cette semaine, une
//! colonne ou une table n'existait que sur le premier — `avoir.piece_id`,
//! `unite_vente.code_barre`, `cheque_recu`… — et aucune base PostgreSQL
//! ne l'avait. On ne le découvrait qu'en portant le module qui la lit.
//!
//! Ce test compare les deux structures, colonne par colonne. Il tourne
//! sur SQLite (les deux chemins savent y écrire) : ce qui manque à
//! `schema.sql` manque aussi à PostgreSQL, puisque c'est le même
//! fichier qui l'amorce.

use std::collections::BTreeSet;

use gescom_noyau::amorcage;
use gescom_noyau::base::Base;

/// (table, colonne) de toutes les tables utilisateur d'une base SQLite.
fn colonnes(conn: &rusqlite::Connection) -> BTreeSet<(String, String)> {
    let mut tables = conn
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'")
        .unwrap();
    let noms: Vec<String> = tables.query_map([], |r| r.get(0)).unwrap().map(|r| r.unwrap()).collect();
    let mut out = BTreeSet::new();
    for t in noms {
        let mut st = conn.prepare(&format!("PRAGMA table_info({t})")).unwrap();
        for c in st.query_map([], |r| r.get::<_, String>(1)).unwrap() {
            out.insert((t.clone(), c.unwrap()));
        }
    }
    out
}

#[test]
fn tout_ce_que_la_fenetre_migre_existe_dans_le_schema_commun() {
    // Le chemin de la fenêtre.
    let fenetre = rusqlite::Connection::open_in_memory().unwrap();
    gescom_noyau::persistance::initialiser_tables(&fenetre).unwrap();
    let de_la_fenetre = colonnes(&fenetre);

    // Le chemin du serveur, celui qui amorce aussi PostgreSQL.
    let mut base = Base::ouvrir(":memory:").unwrap();
    amorcage::amorcer(&mut base).unwrap();
    let du_serveur = colonnes(base.sqlite().unwrap());

    let manquantes: Vec<String> = de_la_fenetre
        .difference(&du_serveur)
        .map(|(t, c)| format!("{t}.{c}"))
        .collect();

    assert!(
        manquantes.is_empty(),
        "Colonnes que la fenêtre pose (persistance/mod.rs) mais que `Base` n'amorce pas — \
         aucune base PostgreSQL ne les a. Les ajouter à schema.sql, et à amorcage.rs \
         pour les bases déjà amorcées :\n  {}",
        manquantes.join("\n  ")
    );
}

#[test]
fn le_serveur_n_invente_que_le_cloisonnement_et_le_reseau() {
    // Dans l'autre sens, `Base` ajoute des choses que la fenêtre n'a
    // pas : `dossier_id` partout (le cloisonnement), les tables du
    // réseau v2, les dossiers et exercices. C'est voulu — mais la
    // liste doit rester CONNUE, pour qu'une table inventée par erreur
    // d'un seul côté se voie ici.
    let fenetre = rusqlite::Connection::open_in_memory().unwrap();
    gescom_noyau::persistance::initialiser_tables(&fenetre).unwrap();
    let de_la_fenetre = colonnes(&fenetre);
    let mut base = Base::ouvrir(":memory:").unwrap();
    amorcage::amorcer(&mut base).unwrap();
    let du_serveur = colonnes(base.sqlite().unwrap());

    let tables_reseau = ["poste", "session_reseau", "modele_document", "dossier", "exercice"];
    let colonnes_serveur = ["dossier_id", "poste_id", "utilisateur_id", "acces_total", "protege", "description"];
    let inattendues: Vec<String> = du_serveur
        .difference(&de_la_fenetre)
        .filter(|(t, c)| !tables_reseau.contains(&t.as_str()) && !colonnes_serveur.contains(&c.as_str()))
        .map(|(t, c)| format!("{t}.{c}"))
        .collect();
    assert!(
        inattendues.is_empty(),
        "Le serveur amorce des colonnes que la fenêtre ne connaît pas et qui ne sont ni \
         le cloisonnement ni le réseau — à ajouter aussi à persistance/mod.rs, ou à \
         inscrire ici si c'est voulu :\n  {}",
        inattendues.join("\n  ")
    );
}
