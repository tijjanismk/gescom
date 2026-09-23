//! A-1 — les réglages des documents imprimés, sur les deux moteurs.
//!
//! Ce qui compte pour la boutique : une facture sort toujours, même
//! avec un réglage abîmé ; les libellés réglés en v2 survivent au
//! passage en v3 ; une signature retirée n'emporte pas son cachet dans
//! la suivante.

mod commun;

use commun::*;
use gescom_noyau::documents;
use serde_json::json;

const PNG: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==";

#[test]
fn sans_reglage_chaque_genre_a_ses_defauts_d_usine() {
    let mut base = base_avec_demo();
    let v = documents::lire_reglages_sur(&mut base).unwrap();
    assert_eq!(v["ordre"].as_array().unwrap().len(), 7);
    let f = &v["genres"]["facture"];
    assert_eq!(f["format"], "a4");
    assert_eq!(f["montant_lettres"], true);
    assert_eq!(f["colonne_remise"], "auto");
    assert_eq!(f["signatures"][0]["libelle"], "Pour acquit");
    assert_eq!(f["signatures"][0]["image"], serde_json::Value::Null);
    assert_eq!(v["genres"]["ticket"]["format"], "thermique_80");
    assert_eq!(v["genres"]["recu"]["signatures"][0]["libelle"], "Le caissier");
    assert_eq!(v["genres"]["ticket"]["signatures"].as_array().unwrap().len(), 0);
}

#[test]
fn les_libelles_de_la_v2_passent_en_v3() {
    let mut base = base_avec_demo();
    base.executer(
        "INSERT INTO config_app (cle, valeur) VALUES ('signature_livraison_gauche', 'Le magasinier'),
                                                      ('signature_livraison_droite', '')",
        &[],
    )
    .unwrap();
    let bl = documents::lire_reglage_sur(&mut base, "bon_livraison").unwrap();
    let libelles: Vec<_> = bl["signatures"].as_array().unwrap().iter().map(|s| s["libelle"].clone()).collect();
    assert_eq!(libelles, vec![json!("Le magasinier")], "une paire vidée en v2 reste vide");
}

#[test]
fn un_reglage_enregistre_se_relit_et_les_autres_genres_ne_bougent_pas() {
    let mut base = base_avec_demo();
    let avant_devis = documents::lire_reglage_sur(&mut base, "devis").unwrap();
    let r = documents::enregistrer_reglage_sur(
        &mut base,
        "bon_livraison",
        json!({
            "format": "a5", "colonne_remise": "non", "colonne_tva": "oui", "recap_tva": "auto",
            "montant_lettres": false, "reference_article": true, "mention": " Marchandise vérifiée ",
            "signatures": [{ "libelle": "Le magasinier" }, { "libelle": "Le transporteur" }, { "libelle": "Reçu par" }]
        }),
    )
    .unwrap();
    assert_eq!(r["format"], "a5");
    assert_eq!(r["reference_article"], true);
    assert_eq!(r["mention"], "Marchandise vérifiée");
    assert_eq!(r["signatures"].as_array().unwrap().len(), 3);

    let relu = documents::lire_reglage_sur(&mut base, "bon_livraison").unwrap();
    assert_eq!(relu, r);
    assert_eq!(documents::lire_reglage_sur(&mut base, "devis").unwrap(), avant_devis);
}

#[test]
fn un_genre_jamais_regle_suit_l_usine() {
    let mut base = base_avec_demo();
    documents::enregistrer_reglage_sur(&mut base, "facture", json!({ "format": "a5" })).unwrap();
    let brut: String = base
        .lire_une("SELECT valeur FROM config_app WHERE cle = 'documents_reglages'", &[], |r| r.get::<String>(0))
        .unwrap()
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(&brut).unwrap();
    assert_eq!(v.as_object().unwrap().len(), 1, "seul le genre réglé est enregistré : {v}");
    documents::retablir_defaut_sur(&mut base, "facture").unwrap();
    let brut: String = base
        .lire_une("SELECT valeur FROM config_app WHERE cle = 'documents_reglages'", &[], |r| r.get::<String>(0))
        .unwrap()
        .unwrap();
    assert_eq!(brut, "{}");
}

#[test]
fn un_reglage_invalide_est_refuse_et_rien_ne_change() {
    let mut base = base_avec_demo();
    let avant = documents::lire_reglages_sur(&mut base).unwrap();
    let quatre = json!({ "format": "a4", "signatures": [
        { "libelle": "A" }, { "libelle": "B" }, { "libelle": "C" }, { "libelle": "D" } ] });
    assert!(documents::enregistrer_reglage_sur(&mut base, "facture", quatre).unwrap_err().contains("Trois"));
    assert!(documents::enregistrer_reglage_sur(&mut base, "ticket", json!({ "format": "a4" })).is_err());
    assert!(documents::enregistrer_reglage_sur(&mut base, "affiche", json!({ "format": "a4" })).is_err());
    assert!(documents::enregistrer_reglage_sur(&mut base, "facture", json!({ "format": 3 })).is_err());
    assert_eq!(documents::lire_reglages_sur(&mut base).unwrap(), avant);
}

#[test]
fn un_reglage_abime_en_base_n_empeche_pas_d_imprimer() {
    let mut base = base_avec_demo();
    base.executer(
        "INSERT INTO config_app (cle, valeur) VALUES ('documents_reglages', '{pas du json')",
        &[],
    )
    .unwrap();
    let v = documents::lire_reglages_sur(&mut base).expect("les défauts, pas une erreur");
    assert_eq!(v["genres"]["facture"]["format"], "a4");
}

#[test]
fn le_cachet_se_pose_se_relit_et_part_avec_sa_signature() {
    let mut base = base_avec_demo();
    let r = documents::poser_image_signature_sur(&mut base, "facture", 1, Some(PNG.into())).unwrap();
    assert_eq!(r["signatures"][1]["image"], PNG);
    assert_eq!(r["signatures"][0]["image"], serde_json::Value::Null);

    // Pas d'emplacement n° 3 : refus.
    assert!(documents::poser_image_signature_sur(&mut base, "facture", 2, Some(PNG.into())).is_err());
    // Une image qui n'en est pas une : refus.
    assert!(documents::poser_image_signature_sur(&mut base, "facture", 0, Some("data:text/html;base64,PGI+".into())).is_err());
    assert!(documents::poser_image_signature_sur(&mut base, "facture", 0, Some("pas une image".into())).is_err());

    // On retire la seconde signature : son cachet part avec elle, et
    // ne revient pas quand on en rajoute une.
    documents::enregistrer_reglage_sur(&mut base, "facture", json!({ "format": "a4", "signatures": [{ "libelle": "Le client" }] })).unwrap();
    let r = documents::enregistrer_reglage_sur(
        &mut base, "facture",
        json!({ "format": "a4", "signatures": [{ "libelle": "Le client" }, { "libelle": "Pour la société" }] }),
    )
    .unwrap();
    assert_eq!(r["signatures"][1]["image"], serde_json::Value::Null);
}

#[test]
fn une_image_trop_lourde_est_refusee() {
    let mut base = base_avec_demo();
    let lourd = format!("data:image/png;base64,{}", "A".repeat(800 * 1024));
    let e = documents::poser_image_signature_sur(&mut base, "facture", 0, Some(lourd)).unwrap_err();
    assert!(e.contains("trop lourde"), "{e}");
}

#[test]
fn retablir_remet_l_usine_et_retire_les_cachets() {
    let mut base = base_avec_demo();
    documents::enregistrer_reglage_sur(&mut base, "devis", json!({ "format": "a5", "signatures": [{ "libelle": "X" }] })).unwrap();
    documents::poser_image_signature_sur(&mut base, "devis", 0, Some(PNG.into())).unwrap();
    let r = documents::retablir_defaut_sur(&mut base, "devis").unwrap();
    assert_eq!(r["format"], "a4");
    assert_eq!(r["signatures"][0]["libelle"], "Le vendeur");
    assert_eq!(r["signatures"][0]["image"], serde_json::Value::Null);
}

#[test]
fn les_coordonnees_se_choisissent_une_fois_pour_tous_les_documents() {
    let mut base = base_avec_demo();
    let v = documents::lire_reglages_sur(&mut base).unwrap();
    assert_eq!(v["coordonnees"], json!(["adresse", "telephone", "telephone2", "nif", "rccm"]));
    documents::enregistrer_coordonnees_sur(&mut base, vec!["telephone".into(), "adresse".into()]).unwrap();
    let v = documents::lire_reglages_sur(&mut base).unwrap();
    assert_eq!(v["coordonnees"], json!(["adresse", "telephone"]));
    assert!(documents::enregistrer_coordonnees_sur(&mut base, vec!["iban".into()]).is_err());
    assert_eq!(documents::lire_reglages_sur(&mut base).unwrap()["coordonnees"], json!(["adresse", "telephone"]));
}

#[test]
fn tout_ce_qui_est_touche_passe_le_detecteur() {
    let mut base = base_avec_demo();
    base.auditer(true);
    documents::lire_reglages_sur(&mut base).unwrap();
    documents::enregistrer_reglage_sur(&mut base, "recu", json!({ "format": "a5" })).unwrap();
    documents::poser_image_signature_sur(&mut base, "facture", 0, Some(PNG.into())).unwrap();
    documents::retablir_defaut_sur(&mut base, "facture").unwrap();
    documents::enregistrer_coordonnees_sur(&mut base, vec!["nif".into()]).unwrap();
}

/// A-2 : la pièce à imprimer porte la référence de l'article et toutes
/// les coordonnées de la société — le document choisit lesquelles
/// montrer. Les deux versions de la lecture (le serveur sur fichier
/// SQLite passe par la version `Connection`).
#[test]
fn la_piece_a_imprimer_porte_reference_et_coordonnees() {
    let mut base = base_avec_demo();
    let client = client_reel(&mut base);
    let sucre = article_unite(&mut base, "Sucre");
    base.executer("UPDATE article SET code_barre = '2000000000017' WHERE id = ?1", &gescom_noyau::parametres![sucre.0.clone()]).unwrap();
    base.executer("UPDATE parametres_societe SET site_web = 'www.boutique.ml' WHERE id = 1", &[]).unwrap();
    let p = gescom_noyau::pieces::creer_piece_sur_base(
        &mut base, client, "facture".into(), vec![ligne(&sucre, 2.0)], None, None, None, None, None, None,
    )
    .unwrap();
    let id = p["id"].as_str().unwrap().to_string();
    let d = gescom_noyau::pieces::lire_donnees_piece_sur_base(&mut base, id.clone()).unwrap();
    assert_eq!(d["lignes"][0]["article_reference"], "2000000000017");
    assert_eq!(d["societe"]["site_web"], "www.boutique.ml");
    if let Some(conn) = base.sqlite() {
        let d = gescom_noyau::pieces::lire_donnees_piece(conn, id).unwrap();
        assert_eq!(d["lignes"][0]["article_reference"], "2000000000017");
        assert_eq!(d["societe"]["site_web"], "www.boutique.ml");
    }
}

/// A-3 : l'atelier part, ses tables aussi — sur une base qui les avait,
/// et sans casser un second passage (la migration se rejoue).
#[test]
fn les_tables_de_l_atelier_partent_a_l_amorcage() {
    let mut base = base_avec_demo();
    base.executer_lot(
        "CREATE TABLE IF NOT EXISTS modele_document (id TEXT PRIMARY KEY, genre TEXT);
         CREATE TABLE IF NOT EXISTS image_document (id TEXT PRIMARY KEY, nom TEXT);
         INSERT INTO modele_document (id, genre) VALUES ('m1', 'facture');",
    )
    .unwrap();
    gescom_noyau::amorcage::amorcer(&mut base).expect("second amorçage");
    gescom_noyau::amorcage::amorcer(&mut base).expect("troisième, idempotent");
    let reste = |base: &mut gescom_noyau::base::Base, t: &str| {
        base.lire_une(&format!("SELECT COUNT(*) FROM {t}"), &[], |r| r.get::<i64>(0)).is_ok()
    };
    assert!(!reste(&mut base, "modele_document"), "modele_document supprimée");
    assert!(!reste(&mut base, "image_document"), "image_document supprimée");
}
