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

/// Un modèle qui pose un cachet voyage AVEC son cachet : à l'arrivée,
/// l'image existe sous le même identifiant, et le bloc la retrouve.
/// Une seconde importation ne la pose pas deux fois.
#[test]
fn un_export_emporte_les_images_posees_et_l_import_les_repose() {
    let conn = base();
    let d_source = std::env::temp_dir().join(format!("gescom-export-src-{}", uuid::Uuid::new_v4()));
    let d_cible = std::env::temp_dir().join(format!("gescom-export-dst-{}", uuid::Uuid::new_v4()));

    let cachet = gescom_noyau::images::importer_libre(&conn, "cachet.png", b"le cachet", &d_source)
        .expect("importer le cachet");
    let mut m = modele("m-cachet", "facture", "Avec cachet", false);
    m.contenu = json!({
        "version": 1, "page": {},
        "blocs": [{ "id": "b1", "type": "image", "visible": true, "image": "logo",
                    "imageId": cachet, "largeurMm": 30, "hauteurMm": 0, "alignement": "gauche" }]
    });
    modeles::enregistrer(&conn, &m, "test").unwrap();

    let lot = modeles::exporter(&conn, None).unwrap();
    assert_eq!(lot.version, VERSION_ECHANGE);
    assert_eq!(lot.images.len(), 1, "le cachet doit voyager avec le modèle");
    assert_eq!(lot.images[0].id, cachet);
    assert_eq!(lot.images[0].nom, "cachet.png");

    let json_texte = serde_json::to_string(&lot).unwrap();
    let cible = base();
    let relu: modeles::Lot = serde_json::from_str(&json_texte).unwrap();
    let bilan = modeles::importer_avec_images(&cible, &relu, "test", Some(&d_cible)).unwrap();
    assert_eq!(bilan.ajoutes, 1);
    assert_eq!(bilan.images_ajoutees, 1);
    assert!(bilan.ignores.is_empty(), "{:?}", bilan.ignores);

    // Même identifiant, mêmes octets : le bloc du modèle importé la retrouve.
    let b64 = gescom_noyau::images::lire_libre_base64(&cible, &cachet).unwrap().expect("le cachet est arrivé");
    assert!(b64.ends_with(&base64_de(b"le cachet")), "les octets doivent traverser intacts");

    // Reimporter : rien de plus, pas de doublon.
    let bilan2 = modeles::importer_avec_images(&cible, &relu, "test", Some(&d_cible)).unwrap();
    assert_eq!(bilan2.images_ajoutees, 0);
    assert_eq!(gescom_noyau::images::lister_libres(&cible).unwrap().len(), 1);

    // Sans dossier d'images, le modèle arrive quand même et le bilan le dit.
    let cible2 = base();
    let bilan3 = modeles::importer(&cible2, &relu, "test").unwrap();
    assert_eq!(bilan3.ajoutes, 1);
    assert_eq!(bilan3.images_ajoutees, 0);
    assert!(bilan3.ignores.iter().any(|i| i.contains("image")), "{:?}", bilan3.ignores);

    // Un lot d'AVANT (version 1, sans `images`) se lit toujours.
    let ancien = json!({ "marqueur": MARQUEUR, "version": 1, "exporte_le": "x", "modeles": [m] });
    let ancien: modeles::Lot = serde_json::from_value(ancien).unwrap();
    assert!(ancien.images.is_empty());

    let _ = std::fs::remove_dir_all(&d_source);
    let _ = std::fs::remove_dir_all(&d_cible);
}

fn base64_de(octets: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(octets)
}

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
        images: vec![],
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
        images: vec![],
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
        images: vec![],
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
        images: vec![],
    };
    let bilan = modeles::importer(&conn, &lot, "test").unwrap();
    // Un lot de six dont un est mal formé en installe cinq, et dit
    // lequel manque.
    assert_eq!(bilan.ajoutes, 1);
    assert_eq!(bilan.ignores.len(), 1);
    assert!(modeles::lire(&conn, "bon").is_ok());
}

/// Un modele sans `modifie_le` doit passer.
///
/// L'ecran envoie les modeles d'usine tels qu'il les construit, et il
/// n'y met pas de date de modification — ce n'est pas au client de
/// l'inventer, c'est l'enregistrement qui date. Serde exigeait pourtant
/// le champ : tout le lot etait refuse avec « missing field
/// `modifie_le` », et l'ecran des modeles s'ouvrait sur une erreur au
/// lieu de proposer ses modeles.
#[test]
fn un_modele_sans_date_de_modification_est_accepte() {
    let conn = base();
    let brut = serde_json::json!({
        "id": "facture-usine",
        "genre": "facture",
        "nom": "Facture A4",
        "format": "a4",
        "contenu": { "blocs": [] },
        "est_defaut": true,
        "actif": false
    });

    let m: gescom_noyau::modeles::Modele = serde_json::from_value(brut)
        .expect("un modèle d'usine doit être lisible sans `modifie_le`");
    gescom_noyau::modeles::enregistrer(&conn, &m, "u1").expect("enregistré");

    // Et l'enregistrement lui a bien donne une date.
    let date: String = conn
        .query_row(
            "SELECT modifie_le FROM modele_document WHERE id = 'facture-usine'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(!date.is_empty(), "l'enregistrement doit dater le modèle");
}
