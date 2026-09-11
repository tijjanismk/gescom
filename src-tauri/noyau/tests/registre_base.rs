//! `Registre::aussi_sur_base` — brancher la version `Base` d'une
//! commande déjà enregistrée, sans toucher à son chemin `Connection`.
//!
//! C'est le mécanisme qui permet à `gescom-serveur` de servir une
//! commande portée sur PostgreSQL (D11) tout en laissant le chemin
//! fichier strictement inchangé. Sans scénario, une faute de frappe
//! dans un nom de commande resterait invisible jusqu'à la première
//! caisse PostgreSQL qui l'essaierait.

use gescom_noyau::amorcage;
use gescom_noyau::base::Base;
use gescom_noyau::registre::{Appelant, Contexte, ContexteBase, Registre};
use serde_json::{json, Value};

fn appelant() -> Appelant {
    Appelant {
        utilisateur_id: "u1".into(),
        role: "patron".into(),
        poste_id: "p1".into(),
    }
}

#[test]
fn aussi_sur_base_complete_une_entree_existante() {
    let mut r = Registre::nouveau();
    r.lecture("ping", |_c, _p| Ok(json!("pong-connection")));
    r.aussi_sur_base("ping", |_c, _p| Ok(json!("pong-base")));

    let entree = r.trouver("ping").expect("l'entrée existe");
    assert!(entree.poignee_base.is_some(), "la version Base doit être branchée");
}

#[test]
fn aussi_sur_base_sur_un_nom_inconnu_ne_cree_rien() {
    let mut r = Registre::nouveau();
    r.aussi_sur_base("commande_qui_n_existe_pas", |_c, _p| Ok(Value::Null));
    assert!(
        r.trouver("commande_qui_n_existe_pas").is_none(),
        "brancher Base sur un nom jamais enregistré ne doit rien créer"
    );
}

#[test]
fn une_commande_sans_aussi_sur_base_n_a_pas_de_version_base() {
    let mut r = Registre::nouveau();
    r.lecture("seule_sur_connection", |_c, _p| Ok(Value::Null));
    let entree = r.trouver("seule_sur_connection").unwrap();
    assert!(entree.poignee_base.is_none());
}

/// Le vrai test : les deux chemins d'une MEME commande, appelés pour
/// de vrai, rendent la même chose — l'un via `rusqlite::Connection`,
/// l'autre via `Base`, sur la même base amorcée.
#[test]
fn les_deux_chemins_d_une_commande_rendent_la_meme_reponse() {
    let mut conn = rusqlite::Connection::open_in_memory().expect("connexion sqlite");
    gescom_noyau::persistance::initialiser_tables(&conn).expect("schema");
    gescom_noyau::seed::amorcer_si_vide(&conn).expect("amorçage");

    let mut base = Base::ouvrir(":memory:").expect("base en mémoire");
    amorcage::amorcer(&mut base).expect("amorçage base");

    let mut r = Registre::nouveau();
    r.lecture("lire_clients", |c, _p| {
        serde_json::to_value(gescom_noyau::catalogue::lire_clients(c.conn)?)
            .map_err(|e| e.to_string())
    });
    r.aussi_sur_base("lire_clients", |c, _p| {
        serde_json::to_value(gescom_noyau::catalogue::lire_clients_sur(c.base)?)
            .map_err(|e| e.to_string())
    });

    let entree = r.trouver("lire_clients").unwrap();
    let appel = appelant();

    let via_connection = (entree.poignee)(
        &mut Contexte { conn: &mut conn, appelant: &appel },
        Value::Null,
    )
    .expect("chemin Connection");

    let poignee_base = entree.poignee_base.expect("la version Base doit être branchée");
    let via_base = poignee_base(
        &mut ContexteBase { base: &mut base, appelant: &appel },
        Value::Null,
    )
    .expect("chemin Base");

    // Les deux amorçages posent le même client générique ("Comptant"),
    // et `lire_clients` ne filtre pas `est_generique` : c'est la seule
    // ligne attendue (les clients de démo ne sont créés que sous
    // GESCOM_DEMO=1, absent ici). L'identifiant, lui, est un UUID tiré
    // au hasard par CHAQUE amorçage — deux bases distinctes n'ont donc
    // jamais le même ; seul le reste doit concorder.
    let sans_id = |v: &Value| {
        v.as_array()
            .map(|a| {
                a.iter()
                    .map(|o| (o["code"].clone(), o["nom"].clone(), o["telephone"].clone()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    };
    assert_eq!(sans_id(&via_connection), sans_id(&via_base), "les deux chemins doivent s'accorder");
    assert_eq!(via_connection.as_array().map(|a| a.len()), Some(1));
}
