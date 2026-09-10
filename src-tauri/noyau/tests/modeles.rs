//! Les invariants des modèles de documents.
//!
//! Ce qui compte ici tient en trois phrases : il y a toujours
//! exactement un modèle actif par genre, un modèle d'usine ne se perd
//! pas, et importer le lot d'une autre boutique ne change pas la
//! facture qui sort de votre imprimante.

use rusqlite::Connection;
use serde_json::json;

use gescom_noyau::modeles::{self, Modele, MARQUEUR, VERSION_ECHANGE};
use gescom_noyau::persistance;

fn base() -> Connection {
    let conn = Connection::open_in_memory().expect("base en mémoire");
    persistance::initialiser_tables(&conn).expect("schéma");
    conn
}

fn modele(id: &str, genre: &str, nom: &str, est_defaut: bool) -> Modele {
    Modele {
        id: id.to_string(),
        genre: genre.to_string(),
        nom: nom.to_string(),
        format: "a4".to_string(),
        contenu: json!({ "version": 1, "page": {}, "blocs": [] }),
        est_defaut,
        actif: false,
        modifie_le: String::new(),
    }
}

fn actif_de(conn: &Connection, genre: &str) -> Option<String> {
    modeles::lire_actif(conn, genre).map(|m| m.id)
}

// =====================================================================
//  Un actif par genre, toujours
// =====================================================================

#[test]
fn le_premier_modele_dun_genre_devient_actif() {
    let conn = base();
    // Sans cette règle, on crée un modèle, on imprime, et rien ne
    // change — sans que rien n'explique pourquoi.
    modeles::enregistrer(&conn, &modele("m1", "facture", "A4", false), "test").unwrap();
    assert_eq!(actif_de(&conn, "facture").as_deref(), Some("m1"));
}

#[test]
fn le_second_ne_vole_pas_la_place_du_premier() {
    let conn = base();
    modeles::enregistrer(&conn, &modele("m1", "facture", "A4", false), "test").unwrap();
    modeles::enregistrer(&conn, &modele("m2", "facture", "Ticket", false), "test").unwrap();
    assert_eq!(actif_de(&conn, "facture").as_deref(), Some("m1"));
}

#[test]
fn activer_desactive_lautre() {
    let conn = base();
    modeles::enregistrer(&conn, &modele("m1", "facture", "A4", false), "test").unwrap();
    modeles::enregistrer(&conn, &modele("m2", "facture", "Ticket", false), "test").unwrap();
    modeles::definir_actif(&conn, "m2").unwrap();

    assert_eq!(actif_de(&conn, "facture").as_deref(), Some("m2"));
    // Deux actifs, et le document imprimé dépendrait de l'ordre de
    // lecture — donc changerait sans raison visible.
    let n: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM modele_document WHERE genre='facture' AND actif=1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(n, 1);
}

#[test]
fn les_genres_ne_se_marchent_pas_dessus() {
    let conn = base();
    modeles::enregistrer(&conn, &modele("f1", "facture", "A4", false), "test").unwrap();
    modeles::enregistrer(&conn, &modele("r1", "recu_paiement", "Reçu", false), "test").unwrap();
    assert_eq!(actif_de(&conn, "facture").as_deref(), Some("f1"));
    assert_eq!(actif_de(&conn, "recu_paiement").as_deref(), Some("r1"));
}

#[test]
fn supprimer_lactif_designe_le_suivant() {
    let conn = base();
    modeles::enregistrer(&conn, &modele("m1", "facture", "A4", false), "test").unwrap();
    modeles::enregistrer(&conn, &modele("m2", "facture", "Ticket", false), "test").unwrap();
    modeles::supprimer(&conn, "m1").unwrap();

    // Sans repli, le genre resterait sans modèle : l'impression
    // retomberait en silence sur le générateur historique et le
    // commerçant croirait à une panne.
    assert_eq!(actif_de(&conn, "facture").as_deref(), Some("m2"));
}

#[test]
fn un_modele_dusine_ne_se_supprime_pas() {
    let conn = base();
    modeles::enregistrer(&conn, &modele("std", "facture", "Usine", true), "test").unwrap();
    // Il faut toujours un chemin de retour après une mise en page ratée
    // un soir de clôture.
    assert!(modeles::supprimer(&conn, "std").is_err());
    assert!(modeles::lire(&conn, "std").is_ok());
}

#[test]
fn un_modele_sans_nom_est_refuse() {
    let conn = base();
    let m = modele("m1", "facture", "   ", false);
    assert!(modeles::enregistrer(&conn, &m, "test").is_err());
}

// =====================================================================
//  Transport
// =====================================================================

#[test]
fn un_aller_retour_conserve_le_contenu() {
    let conn = base();
    let mut m = modele("m1", "facture", "A4", false);
    m.contenu = json!({
        "version": 1,
        "page": { "margeMm": 12 },
        "blocs": [{ "id": "b1", "type": "titre", "texte": "FACTURE" }]
    });
    modeles::enregistrer(&conn, &m, "test").unwrap();

    let lot = modeles::exporter(&conn, None).unwrap();
    let json_texte = serde_json::to_string(&lot).unwrap();

    let cible = base();
    let relu: modeles::Lot = serde_json::from_str(&json_texte).unwrap();
    let bilan = modeles::importer(&cible, &relu, "test").unwrap();

    assert_eq!(bilan.ajoutes, 1);
    assert_eq!(bilan.remplaces, 0);
    let arrive = modeles::lire(&cible, "m1").unwrap();
    assert_eq!(arrive.contenu, m.contenu, "le modèle doit traverser intact");
}

#[test]
fn importer_ne_change_pas_le_modele_actif_du_poste() {
    let conn = base();
    modeles::enregistrer(&conn, &modele("local", "facture", "Le mien", false), "test").unwrap();
    modeles::enregistrer(&conn, &modele("venu", "facture", "Le sien", false), "test").unwrap();
    modeles::definir_actif(&conn, "local").unwrap();

    // Le lot arrive avec « venu » marqué actif chez l'expéditeur.
    let mut venu = modele("venu", "facture", "Le sien (corrigé)", false);
    venu.actif = true;
    let lot = modeles::Lot {
        marqueur: MARQUEUR.to_string(),
        version: VERSION_ECHANGE,
        exporte_le: "2026-01-01".to_string(),
        societe: Some("Autre boutique".to_string()),
        modeles: vec![venu],
    };
    modeles::importer(&conn, &lot, "test").unwrap();

    // Le contenu est bien repris…
    assert_eq!(modeles::lire(&conn, "venu").unwrap().nom, "Le sien (corrigé)");
    // …mais importer un lot depuis la boutique du cousin ne doit pas
    // changer la facture qui sort de votre imprimante.
    assert_eq!(actif_de(&conn, "facture").as_deref(), Some("local"));
}

#[test]
fn un_fichier_etranger_est_refuse() {
    let conn = base();
    let lot = modeles::Lot {
        marqueur: "autre-chose".to_string(),
        version: VERSION_ECHANGE,
        exporte_le: "2026-01-01".to_string(),
        societe: None,
        modeles: vec![modele("m1", "facture", "A4", false)],
    };
    assert!(modeles::importer(&conn, &lot, "test").is_err());
}

#[test]
fn un_fichier_trop_recent_est_refuse() {
    let conn = base();
    let lot = modeles::Lot {
        marqueur: MARQUEUR.to_string(),
        version: VERSION_ECHANGE + 1,
        exporte_le: "2026-01-01".to_string(),
        societe: None,
        modeles: vec![modele("m1", "facture", "A4", false)],
    };
    // Laisser passer ferait lire des champs absents comme des zéros :
    // une mise en page cassée, sans message d'erreur.
    let err = modeles::importer(&conn, &lot, "test").unwrap_err();
    assert!(err.contains("plus récente"), "{err}");
}

#[test]
fn un_modele_casse_ne_fait_pas_echouer_le_lot() {
    let conn = base();
    let lot = modeles::Lot {
        marqueur: MARQUEUR.to_string(),
        version: VERSION_ECHANGE,
        exporte_le: "2026-01-01".to_string(),
        societe: None,
        modeles: vec![
            modele("bon", "facture", "Correct", false),
            modele("mauvais", "facture", "", false),
        ],
    };
    let bilan = modeles::importer(&conn, &lot, "test").unwrap();
    // Un lot de six dont un est mal formé en installe cinq, et dit
    // lequel manque.
    assert_eq!(bilan.ajoutes, 1);
    assert_eq!(bilan.ignores.len(), 1);
    assert!(modeles::lire(&conn, "bon").is_ok());
}
