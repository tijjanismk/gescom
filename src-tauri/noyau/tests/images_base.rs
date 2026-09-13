//! Les images de la boutique, sur `Base` — depot et relecture.
//!
//! D8 : la caisse envoie le CONTENU du fichier en base64, le serveur
//! le range dans SON dossier d'images et enregistre le chemin. Ces
//! scenarios verifient l'aller-retour complet (ecrire puis relire),
//! les refus, et que le chemin est bien enregistre en base — sur
//! SQLite comme sur PostgreSQL.

use std::path::{Path, PathBuf};

use base64::Engine;
use gescom_noyau::images;

mod commun;
use commun::base_avec_demo;

/// Un dossier d'images jetable, nettoye a la fin du scenario.
struct DossierEssai(PathBuf);

impl DossierEssai {
    fn nouveau() -> Self {
        let dossier = std::env::temp_dir()
            .join(format!("gescom-images-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dossier).expect("dossier d'essai");
        DossierEssai(dossier)
    }

    fn chemin(&self) -> &Path {
        &self.0
    }
}

impl Drop for DossierEssai {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Les octets d'un (faux) PNG : la signature puis un corps
/// reconnaissable.
fn png_de_essai() -> Vec<u8> {
    let mut octets = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    octets.extend_from_slice(b"contenu de test");
    octets
}

/// Decode la partie base64 d'une data URL et rend les octets.
fn corps_data_url(data_url: &str) -> Vec<u8> {
    let b64 = data_url.split("base64,").nth(1).expect("data URL sans base64");
    base64::engine::general_purpose::STANDARD
        .decode(b64)
        .expect("base64 relu")
}

#[test]
fn ecrire_puis_relire_fait_l_aller_retour() {
    let mut base = base_avec_demo();
    let dossier = DossierEssai::nouveau();
    let octets = png_de_essai();

    images::ecrire_sur_base(&mut base, "logo", "logo.png", &octets, Some(dossier.chemin()))
        .expect("ecrire le logo");

    // Le chemin est enregistre en base : la relecture ne doit rien
    // devoir au repli du dossier.
    let chemin_bd: String = base
        .lire_une(
            "SELECT logo_chemin FROM parametres_societe WHERE id = 1",
            &[],
            |r| r.get::<String>(0),
        )
        .expect("lire le chemin")
        .expect("chemin du logo");
    assert!(chemin_bd.ends_with("logo.png"), "chemin inattendu : {chemin_bd}");

    let relu = images::lire_base64_sur_base(&mut base, "logo", Some(dossier.chemin()))
        .expect("relire le logo")
        .expect("logo relu");
    assert!(
        relu.starts_with("data:image/png;base64,"),
        "pas une data URL png : {relu}"
    );
    assert_eq!(corps_data_url(&relu), octets, "les octets relus ne sont pas ceux ecrits");
}

#[test]
fn entete_et_pied_sont_distincts_du_logo() {
    let mut base = base_avec_demo();
    let dossier = DossierEssai::nouveau();

    let entete = b"bandeau d'en-tete".to_vec();
    let pied = b"mentions de pied".to_vec();
    images::ecrire_sur_base(&mut base, "entete", "entete.jpg", &entete, Some(dossier.chemin()))
        .expect("ecrire l'en-tete");
    images::ecrire_sur_base(&mut base, "pied", "pied.svg", &pied, Some(dossier.chemin()))
        .expect("ecrire le pied");

    let relu_entete = images::lire_base64_sur_base(&mut base, "entete", Some(dossier.chemin()))
        .expect("relire l'en-tete")
        .expect("en-tete relu");
    let relu_pied = images::lire_base64_sur_base(&mut base, "pied", Some(dossier.chemin()))
        .expect("relire le pied")
        .expect("pied relu");
    assert!(relu_entete.starts_with("data:image/jpeg;base64,"));
    assert!(relu_pied.starts_with("data:image/svg+xml;base64,"));
    assert_eq!(corps_data_url(&relu_entete), entete);
    assert_eq!(corps_data_url(&relu_pied), pied);

    // Aucun logo pose : pas de melange entre les genres.
    let logo = images::lire_base64_sur_base(&mut base, "logo", Some(dossier.chemin()))
        .expect("relire le logo");
    assert!(logo.is_none(), "un logo est apparu sans avoir ete pose");
}

#[test]
fn reecrire_ecrase_l_image_precedente() {
    let mut base = base_avec_demo();
    let dossier = DossierEssai::nouveau();

    images::ecrire_sur_base(&mut base, "logo", "logo.png", b"premiere version", Some(dossier.chemin()))
        .expect("premiere ecriture");
    let deuxieme = b"deuxieme version, plus longue".to_vec();
    images::ecrire_sur_base(&mut base, "logo", "logo.png", &deuxieme, Some(dossier.chemin()))
        .expect("deuxieme ecriture");

    let relu = images::lire_base64_sur_base(&mut base, "logo", Some(dossier.chemin()))
        .expect("relire")
        .expect("logo");
    assert_eq!(corps_data_url(&relu), deuxieme, "l'ancienne image surnage");
}

#[test]
fn format_inconnu_refuse() {
    let mut base = base_avec_demo();
    let dossier = DossierEssai::nouveau();

    let refus = images::ecrire_sur_base(&mut base, "logo", "logo.txt", b"du texte", Some(dossier.chemin()))
        .unwrap_err();
    assert!(refus.contains("Format refusé"), "message inattendu : {refus}");
    assert!(!dossier.chemin().join("logo.txt").exists(), "rien ne doit etre ecrit");
}

#[test]
fn genre_inconnu_refuse() {
    let mut base = base_avec_demo();
    let dossier = DossierEssai::nouveau();

    let refus = images::ecrire_sur_base(&mut base, "banniere", "banniere.png", b"x", Some(dossier.chemin()))
        .unwrap_err();
    assert!(refus.contains("Image inconnue"), "message inattendu : {refus}");
}

#[test]
fn image_trop_lourde_refusee() {
    let mut base = base_avec_demo();
    let dossier = DossierEssai::nouveau();

    let trop = vec![0u8; images::TAILLE_MAX_IMAGE + 1];
    let refus = images::ecrire_sur_base(&mut base, "logo", "logo.png", &trop, Some(dossier.chemin()))
        .unwrap_err();
    assert!(refus.contains("trop lourde"), "message inattendu : {refus}");
    assert!(!dossier.chemin().join("logo.png").exists(), "rien ne doit etre ecrit");
}

#[test]
fn base64_illisible_refuse() {
    let refus = images::decoder_base64("pas du base64 !!!").unwrap_err();
    assert!(refus.contains("base64"), "message inattendu : {refus}");

    // Et un contenu valide passe : encode avec le meme moteur que la
    // lecture, decode par la commande.
    let encode = base64::engine::general_purpose::STANDARD.encode(b"content");
    assert_eq!(images::decoder_base64(&encode).unwrap(), b"content");
}

#[test]
fn sans_dossier_refuse_plutot_que_faire_semblant() {
    let mut base = base_avec_demo();

    let refus = images::ecrire_sur_base(&mut base, "logo", "logo.png", b"x", None).unwrap_err();
    assert!(refus.contains("aucun dossier"), "message inattendu : {refus}");
}

#[test]
fn supprimer_efface_la_colonne_et_le_fichier() {
    let mut base = base_avec_demo();
    let dossier = DossierEssai::nouveau();

    images::ecrire_sur_base(&mut base, "logo", "logo.png", &png_de_essai(), Some(dossier.chemin()))
        .expect("ecrire le logo");
    images::supprimer_sur_base(&mut base, "logo", Some(dossier.chemin()))
        .expect("supprimer le logo");

    // La colonne est vide, le fichier disparu : le repli du dossier ne
    // fait pas revenir une image qu'on vient de supprimer.
    let chemin_bd: Option<String> = base
        .lire_une(
            "SELECT logo_chemin FROM parametres_societe WHERE id = 1",
            &[],
            |r| r.get::<Option<String>>(0),
        )
        .expect("lire le chemin")
        .flatten();
    assert!(chemin_bd.is_none(), "la colonne garde un chemin : {chemin_bd:?}");
    assert!(!dossier.chemin().join("logo.png").exists(), "le fichier reste sur le disque");
    let relu = images::lire_base64_sur_base(&mut base, "logo", Some(dossier.chemin()))
        .expect("relire");
    assert!(relu.is_none(), "l'image supprimee revient par le repli");
}

#[test]
fn tout_ce_qui_est_porte_dans_images_passe_le_detecteur() {
    let mut base = base_avec_demo();
    let dossier = DossierEssai::nouveau();
    base.auditer(true);

    images::ecrire_sur_base(&mut base, "logo", "logo.png", &png_de_essai(), Some(dossier.chemin()))
        .expect("ecrire le logo");
    images::lire_base64_sur_base(&mut base, "logo", Some(dossier.chemin()))
        .expect("relire le logo");
    images::ecrire_sur_base(&mut base, "entete", "entete.jpg", b"bandeau", Some(dossier.chemin()))
        .expect("ecrire l'en-tete");
    images::ecrire_sur_base(&mut base, "pied", "pied.svg", b"mentions", Some(dossier.chemin()))
        .expect("ecrire le pied");
    images::supprimer_sur_base(&mut base, "pied", Some(dossier.chemin()))
        .expect("supprimer le pied");
}
