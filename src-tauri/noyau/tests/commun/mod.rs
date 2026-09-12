//! Ce que les scénarios `_sur_base` ont en commun : une base démo, et
//! de quoi y retrouver un article, un magasin, un client, une caisse.
//!
//! Sur SQLite en mémoire par défaut. Si `GESCOM_PG` est défini, la
//! même base se prépare sur PostgreSQL (`--test-threads=1` : les
//! scénarios partagent une base et la vident chacun). C'est là que
//! les écarts de moteur se voient — le commerçant sera sur celui-là.

#![allow(dead_code)]

use gescom_noyau::amorcage;
use gescom_noyau::base::Base;
use gescom_noyau::parametres;
use gescom_noyau::utils::maintenant_iso;

pub fn base_avec_demo() -> Base {
    let mut base = match std::env::var("GESCOM_PG") {
        Ok(url) => {
            let mut b = Base::ouvrir(&url).expect("PostgreSQL");
            b.executer_lot("DROP SCHEMA public CASCADE; CREATE SCHEMA public;").unwrap();
            b
        }
        Err(_) => Base::ouvrir(":memory:").expect("base en mémoire"),
    };
    amorcage::amorcer(&mut base).expect("amorçage");
    amorcage::donnees_demo(&mut base).expect("démo");
    base
}

/// (article_id, unite_vente_id, facteur, prix_reference) de l'unité de
/// base d'un article de la démo (facteur 1.0).
pub fn article_unite(base: &mut Base, nom: &str) -> (String, String, f64, i64) {
    base.lire_une(
        "SELECT a.id, u.id, u.facteur, u.prix_reference
         FROM article a JOIN unite_vente u ON u.article_id = a.id
         WHERE a.nom = ?1 AND u.facteur = 1.0",
        &parametres![nom],
        |r| Ok((r.get::<String>(0)?, r.get::<String>(1)?, r.get::<f64>(2)?, r.get::<i64>(3)?)),
    )
    .unwrap()
    .unwrap()
}

pub fn depot_defaut(base: &mut Base) -> String {
    let dossier = base.dossier().to_string();
    base.lire_une(
        "SELECT id FROM depot WHERE est_defaut = 1 AND dossier_id = ?1",
        &parametres![dossier],
        |r| r.get::<String>(0),
    )
    .unwrap()
    .unwrap()
}

pub fn client_reel(base: &mut Base) -> String {
    let dossier = base.dossier().to_string();
    base.lire_une(
        "SELECT id FROM client WHERE est_generique = 0 AND dossier_id = ?1 ORDER BY code LIMIT 1",
        &parametres![dossier],
        |r| r.get::<String>(0),
    )
    .unwrap()
    .unwrap()
}

pub fn client_generique(base: &mut Base) -> String {
    let dossier = base.dossier().to_string();
    base.lire_une(
        "SELECT id FROM client WHERE est_generique = 1 AND dossier_id = ?1",
        &parametres![dossier],
        |r| r.get::<String>(0),
    )
    .unwrap()
    .unwrap()
}

/// Un fournisseur du dossier courant, créé à la demande (la démo n'en
/// pose aucun).
pub fn fournisseur(base: &mut Base, nom: &str) -> String {
    let dossier = base.dossier().to_string();
    let id = uuid::Uuid::new_v4().to_string();
    let now = maintenant_iso();
    base.executer(
        "INSERT INTO fournisseur (id, nom, cree_le, modifie_le, dossier_id)
         VALUES (?1, ?2, ?3, ?3, ?4)",
        &parametres![id.clone(), nom, now, dossier],
    )
    .unwrap();
    id
}

pub fn stock(base: &mut Base, article_id: &str, depot_id: &str) -> f64 {
    let dossier = base.dossier().to_string();
    base.lire_une(
        "SELECT COALESCE(quantite, 0) FROM stock_depot
         WHERE article_id = ?1 AND depot_id = ?2 AND dossier_id = ?3",
        &parametres![article_id, depot_id, dossier],
        |r| r.get::<f64>(0),
    )
    .unwrap()
    .unwrap_or(0.0)
}

pub fn ouvrir_caisse(base: &mut Base) -> String {
    gescom_noyau::caisse::ouvrir_session_caisse_sur(base, 0, "patron".into())
        .expect("ouvrir la caisse")
}

pub fn compter(base: &mut Base, sql: &str, params: &[gescom_noyau::base::Valeur]) -> i64 {
    base.lire_une(sql, params, |r| r.get::<i64>(0)).unwrap().unwrap_or(0)
}

pub fn statut_piece(base: &mut Base, piece_id: &str) -> String {
    base.lire_une(
        "SELECT statut FROM piece_commerciale WHERE id = ?1",
        &parametres![piece_id],
        |r| r.get::<String>(0),
    )
    .unwrap()
    .expect("pièce")
}

pub fn ligne(article: &(String, String, f64, i64), quantite: f64) -> gescom_noyau::pieces::LignePieceInput {
    gescom_noyau::pieces::LignePieceInput {
        article_id: article.0.clone(),
        unite_vente_id: article.1.clone(),
        quantite,
        prix_unitaire: article.3,
        remise_pct: 0.0,
        taux_tva: 0.0,
    }
}
