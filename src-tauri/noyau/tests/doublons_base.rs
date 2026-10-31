//! D44 — l'import et l'export des articles et des fournisseurs, et
//! leurs doublons, sur les deux moteurs.
//!
//! Articles : le nom se reconnait sans la casse ni les espaces en trop ;
//! une ligne repetee dans le fichier est refusee ; un code-barre deja
//! pris refuse SA ligne, pas tout l'import ; l'apercu n'ecrit rien ;
//! « ne pas mettre a jour » laisse l'existant et le dit ; une colonne
//! TVA vide garde le taux d'un article existant et donne celui du
//! dossier a un article neuf.
//!
//! Fournisseurs : export sans le « Fournisseur divers » ; import qui
//! cree, complete un doublon (meme NIF ou meme nom) sans le renommer,
//! ou l'ignore ; saisie a la main : NIF deja connu refuse, homonyme a
//! confirmer.
//!
//!   cargo test -p gescom-noyau --test doublons_base
//!   GESCOM_PG=postgresql://…/gescom_test cargo test -p gescom-noyau --test doublons_base -- --test-threads=1

mod commun;

use commun::*;
use gescom_noyau::base::Base;
use gescom_noyau::catalogue_csv::importer_articles_csv_sur_base as importer;
use gescom_noyau::fournisseurs::creer_fournisseur_sur_base as creer_f;
use gescom_noyau::fournisseurs_csv as fcsv;
use gescom_noyau::parametres;

const ENTETE: &str = "Nom;Categorie;Unite;Facteur;Prix;Prix achat;TVA %;Code barre;Stock";

fn n(v: &serde_json::Value, cle: &str) -> i64 {
    v[cle].as_i64().unwrap_or(-1)
}

fn articles(base: &mut Base) -> i64 {
    compter(base, "SELECT COUNT(*) FROM article", &[])
}

fn fournisseurs(base: &mut Base) -> i64 {
    let dossier = base.dossier().to_string();
    compter(base, "SELECT COUNT(*) FROM fournisseur WHERE dossier_id = ?1", &parametres![dossier])
}

fn taux(base: &mut Base, nom: &str) -> f64 {
    base.lire_une("SELECT taux_tva_defaut FROM article WHERE nom = ?1", &parametres![nom], |r| r.get::<f64>(0))
        .unwrap()
        .unwrap()
}

// ---------------------------------------------------------------------
//  Articles
// ---------------------------------------------------------------------

#[test]
fn un_article_se_reconnait_sans_la_casse_ni_les_espaces() {
    let mut base = base_avec_demo();
    let avant = articles(&mut base);
    let csv = format!("{ENTETE}\n  RIZ   LOCAL ;;kg;1;650;;;;");
    let r = importer(&mut base, csv, Some(true), true, false).unwrap();
    assert_eq!((n(&r, "crees"), n(&r, "mis_a_jour")), (0, 1), "{r}");
    assert_eq!(articles(&mut base), avant, "pas de second « Riz local »");
    assert_eq!(article_unite(&mut base, "Riz local").3, 650, "le prix a suivi");
    // A la main aussi.
    let e = gescom_noyau::comptoir::creer_article_rapide_sur(&mut base, "riz  local".into(), "kg".into(), 1, None, None).unwrap_err();
    assert!(e.contains("existe déjà"), "{e}");
    let e = parametres::creer_article_complet_sur_base(&mut base, " SUCRE ".into(), None, "kg".into(), 1, None).unwrap_err();
    assert!(e.contains("existe déjà"), "{e}");
    assert!(parametres::creer_article_complet_sur_base(&mut base, "  ".into(), None, "kg".into(), 1, None).is_err());
}

#[test]
fn une_ligne_repetee_et_un_code_barre_pris_ne_refusent_que_leur_ligne() {
    let mut base = base_avec_demo();
    let avant = articles(&mut base);
    let dossier = base.dossier().to_string();
    let a = article_unite(&mut base, "Sucre").0;
    base.executer("UPDATE article SET code_barre = '6111111111111' WHERE id = ?1", &parametres![a]).unwrap();
    let csv = format!(
        "{ENTETE}\n\
         Savon;;pièce;1;250;;;;10\n\
         savon ;;Pièce;1;300;;;;\n\
         Savon;;carton;24;5500;;;;\n\
         Lait;;boîte;1;900;;;6111111111111;\n\
         Thé;;paquet;1;400;;;;"
    );
    let r = importer(&mut base, csv, Some(true), true, false).unwrap();
    assert_eq!(n(&r, "crees"), 2, "Savon et Thé : {r}");
    assert_eq!(n(&r, "nb_erreurs"), 2, "{r}");
    let erreurs = r["erreurs"].to_string();
    assert!(erreurs.contains("Ligne 3") && erreurs.contains("ligne 2"), "doublon dans le fichier : {erreurs}");
    assert!(erreurs.contains("Ligne 5") && erreurs.contains("6111111111111"), "code-barre pris : {erreurs}");
    assert_eq!(articles(&mut base), avant + 2);
    // Le carton du savon est bien venu, et le stock importe une fois.
    let savon: String = base.lire_une("SELECT id FROM article WHERE nom = 'Savon'", &[], |r| r.get::<String>(0)).unwrap().unwrap();
    assert_eq!(compter(&mut base, "SELECT COUNT(*) FROM unite_vente WHERE article_id = ?1", &parametres![savon.clone()]), 2);
    let depot = depot_defaut(&mut base);
    assert_eq!(stock(&mut base, &savon, &depot), 10.0);
    assert_eq!(
        compter(&mut base, "SELECT COUNT(*) FROM journal WHERE type_evenement = 'import_articles' AND dossier_id = ?1", &parametres![dossier]),
        1
    );
}

#[test]
fn l_apercu_dit_tout_et_n_ecrit_rien() {
    let mut base = base_avec_demo();
    let avant = articles(&mut base);
    let csv = format!("{ENTETE}\nSavon;Hygiène;pièce;1;250;;;;10\nSucre;;kg;1;850;;;;\n;;;;");
    let r = importer(&mut base, csv.clone(), Some(true), true, true).unwrap();
    assert_eq!((n(&r, "crees"), n(&r, "mis_a_jour"), n(&r, "nb_erreurs")), (1, 1, 1), "{r}");
    assert_eq!(r["simule"], true);
    assert_eq!(articles(&mut base), avant, "rien de créé");
    assert_eq!(article_unite(&mut base, "Sucre").3, 800, "rien de mis à jour");
    // L'import pour de vrai fait ce que l'apercu a dit.
    let vrai = importer(&mut base, csv, Some(true), true, false).unwrap();
    assert_eq!((n(&vrai, "crees"), n(&vrai, "mis_a_jour")), (1, 1));
}

#[test]
fn sans_mise_a_jour_l_existant_est_laisse_et_un_article_neuf_garde_ses_unites() {
    let mut base = base_avec_demo();
    let csv = format!("{ENTETE}\nSucre;;kg;1;999;;;;\nSavon;;pièce;1;250;;;;\nSavon;;carton;24;5500;;;;");
    let r = importer(&mut base, csv, Some(false), true, false).unwrap();
    assert_eq!((n(&r, "crees"), n(&r, "mis_a_jour"), n(&r, "nb_ignores")), (1, 0, 1), "{r}");
    assert!(r["ignores"].to_string().contains("Sucre"));
    assert_eq!(article_unite(&mut base, "Sucre").3, 800);
    let savon: String = base.lire_une("SELECT id FROM article WHERE nom = 'Savon'", &[], |r| r.get::<String>(0)).unwrap().unwrap();
    assert_eq!(compter(&mut base, "SELECT COUNT(*) FROM unite_vente WHERE article_id = ?1", &parametres![savon]), 2, "le carton");
}

#[test]
fn une_tva_vide_garde_celle_de_l_article_et_donne_celle_du_dossier_au_neuf() {
    let mut base = base_avec_demo();
    gescom_noyau::assistant::definir_reglages_dossier_sur(&mut base, 18, 0, None).unwrap();
    base.executer("UPDATE article SET taux_tva_defaut = 0.05 WHERE nom = 'Sucre'", &[]).unwrap();
    let csv = "Nom;Categorie;Unite;Prix\nSucre;;kg;850\nSavon;;pièce;250".to_string();
    importer(&mut base, csv, Some(true), true, false).unwrap();
    assert!((taux(&mut base, "Sucre") - 0.05).abs() < 1e-9, "avant D44, remis à 0");
    assert!((taux(&mut base, "Savon") - 0.18).abs() < 1e-9);
}

// ---------------------------------------------------------------------
//  Fournisseurs
// ---------------------------------------------------------------------

#[test]
fn l_export_des_fournisseurs_se_relit_sans_rien_changer() {
    let mut base = base_avec_demo();
    creer_f(&mut base, "Sodima SARL".into(), Some("20 22 00 00".into()), Some("Bamako".into()), Some("084 123".into()), None, Some(true), false).unwrap();
    creer_f(&mut base, "Diarra & Fils; grossiste".into(), None, None, None, None, None, false).unwrap();
    // Le « Fournisseur divers » n'en sort pas.
    gescom_noyau::achats::fournisseur_divers_sur(&mut base).unwrap();
    let csv = fcsv::exporter_fournisseurs_csv_sur_base(&mut base).unwrap();
    assert!(csv.starts_with("\u{FEFF}Nom;Téléphone;Adresse;NIF;Email;Voisin\n"), "{csv}");
    assert!(csv.contains("Sodima SARL;20 22 00 00;Bamako;084 123;;oui"), "{csv}");
    assert!(csv.contains("\"Diarra & Fils; grossiste\""), "le point-virgule protégé : {csv}");
    assert!(!csv.contains(gescom_noyau::coeur::tiers::FOURNISSEUR_DIVERS));
    let avant = fournisseurs(&mut base);
    let r = fcsv::importer_fournisseurs_csv_sur_base(&mut base, csv, true, false).unwrap();
    assert_eq!((n(&r, "crees"), n(&r, "completes"), n(&r, "nb_ignores")), (0, 0, 2), "{r}");
    assert_eq!(fournisseurs(&mut base), avant);
}

#[test]
fn l_import_des_fournisseurs_cree_complete_ou_ignore_sans_renommer() {
    let mut base = base_avec_demo();
    let sodima = creer_f(&mut base, "Sodima SARL".into(), None, None, Some("084123".into()), None, None, false).unwrap();
    creer_f(&mut base, "Ets Koné".into(), Some("66 00 00 00".into()), None, None, None, None, false).unwrap();
    let dossier = base.dossier().to_string();
    let csv = "Nom;Téléphone;Adresse;NIF;Email;Voisin\n\
               SODIMA;20 22 00 00;ACI 2000;084 123;;\n\
               ets  koné;70 00 00 00;Médine;;;non\n\
               Grossiste du Nord;;Ségou;;;oui\n\
               grossiste du nord;;;;;\n\
               Fournisseur divers;;;;;\n\
               Bakary;;;;;peut-être\n\
               ;;;;;"
        .to_string();
    let apercu = fcsv::importer_fournisseurs_csv_sur_base(&mut base, csv.clone(), true, true).unwrap();
    let r = fcsv::importer_fournisseurs_csv_sur_base(&mut base, csv, true, false).unwrap();
    assert_eq!(apercu["crees"], r["crees"], "l'aperçu dit ce que l'import fait");
    assert_eq!((n(&r, "crees"), n(&r, "completes"), n(&r, "nb_ignores"), n(&r, "nb_erreurs")), (1, 2, 0, 4), "{r}");
    // Sodima reconnu par son NIF : complete, pas renomme.
    let s: (String, Option<String>, Option<String>) = base
        .lire_une(
            "SELECT nom, telephone, adresse FROM fournisseur WHERE id = ?1 AND dossier_id = ?2",
            &parametres![sodima["id"].as_str().unwrap(), dossier.clone()],
            |r| Ok((r.get::<String>(0)?, r.get::<Option<String>>(1)?, r.get::<Option<String>>(2)?)),
        )
        .unwrap()
        .unwrap();
    assert_eq!(s, ("Sodima SARL".into(), Some("20 22 00 00".into()), Some("ACI 2000".into())));
    // Ets Kone reconnu par son nom : le telephone qu'il a reste le sien.
    let tel: Option<String> = base
        .lire_une("SELECT telephone FROM fournisseur WHERE nom = 'Ets Koné' AND dossier_id = ?1", &parametres![dossier.clone()], |r| r.get::<Option<String>>(0))
        .unwrap()
        .unwrap();
    assert_eq!(tel.as_deref(), Some("66 00 00 00"));
    let erreurs = r["erreurs"].to_string();
    assert!(erreurs.contains("ligne 4"), "doublon dans le fichier : {erreurs}");
    assert!(erreurs.contains("réservé"), "{erreurs}");
    assert!(erreurs.contains("peut-être"), "{erreurs}");
    assert_eq!(
        compter(&mut base, "SELECT COUNT(*) FROM journal WHERE type_evenement = 'import_fournisseurs' AND dossier_id = ?1", &parametres![dossier]),
        1
    );
    // Sans completer : tout ce qui existe est laisse.
    let r = fcsv::importer_fournisseurs_csv_sur_base(&mut base, "Sodima SARL;;Kati;;;".into(), false, false).unwrap();
    assert_eq!((n(&r, "crees"), n(&r, "completes"), n(&r, "nb_ignores")), (0, 0, 1), "{r}");
}

#[test]
fn a_la_main_un_nif_connu_se_refuse_un_homonyme_se_confirme() {
    let mut base = base_avec_demo();
    let premier = creer_f(&mut base, "Moussa Traoré".into(), Some("76 11 11 11".into()), None, Some("084-123".into()), None, None, false).unwrap();
    let avant = fournisseurs(&mut base);
    let e = creer_f(&mut base, "Autre nom".into(), None, None, Some(" 084 123 ".into()), None, None, false).unwrap_err();
    assert!(e.contains("NIF"), "{e}");
    let e = creer_f(&mut base, "moussa  traoré".into(), None, None, None, None, None, false).unwrap_err();
    assert!(e.starts_with(fcsv::CODE_HOMONYME) && e.contains("76 11 11 11"), "{e}");
    assert!(creer_f(&mut base, " ".into(), None, None, None, None, None, false).is_err());
    assert!(creer_f(&mut base, "fournisseur DIVERS".into(), None, None, None, None, None, true).is_err());
    assert_eq!(fournisseurs(&mut base), avant, "aucun refus n'a écrit");
    // Confirme : un second Moussa Traore.
    creer_f(&mut base, "Moussa Traoré".into(), None, None, None, None, None, true).unwrap();
    assert_eq!(fournisseurs(&mut base), avant + 1);
    // Modifier ne prend pas le NIF d'un autre.
    let autre = creer_f(&mut base, "Awa Diallo".into(), None, None, None, None, None, false).unwrap();
    let e = gescom_noyau::fournisseurs::modifier_fournisseur_sur_base(
        &mut base,
        autre["id"].as_str().unwrap().into(),
        "Awa Diallo".into(),
        None,
        None,
        Some("084123".into()),
        None,
        None,
        false,
    )
    .unwrap_err();
    assert!(e.contains("NIF"), "{e}");
    // Garder son propre NIF en se modifiant passe.
    gescom_noyau::fournisseurs::modifier_fournisseur_sur_base(
        &mut base,
        premier["id"].as_str().unwrap().into(),
        "Moussa Traoré".into(),
        None,
        None,
        Some("084123".into()),
        None,
        None,
        true,
    )
    .unwrap();
}

#[test]
fn tout_passe_le_detecteur() {
    let mut base = base_avec_demo();
    base.auditer(true);
    importer(&mut base, format!("{ENTETE}\nSavon;;pièce;1;250;;;;10\nSucre;;kg;1;850;;;;"), Some(true), true, true).unwrap();
    importer(&mut base, format!("{ENTETE}\nSavon;;pièce;1;250;;;;10\nSucre;;kg;1;850;;;;"), Some(true), true, false).unwrap();
    creer_f(&mut base, "Sodima".into(), None, None, Some("1".into()), None, None, false).unwrap();
    let csv = fcsv::exporter_fournisseurs_csv_sur_base(&mut base).unwrap();
    fcsv::importer_fournisseurs_csv_sur_base(&mut base, csv, true, true).unwrap();
    fcsv::importer_fournisseurs_csv_sur_base(&mut base, "Nouveau;;;2;;".into(), true, false).unwrap();
}
