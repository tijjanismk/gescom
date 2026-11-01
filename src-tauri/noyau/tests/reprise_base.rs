//! D54 : la base d'un poste seul, reprise par Gescom Serveur.
//!
//! Sur des dossiers jetables (`%TEMP%`), jamais le vrai `ProgramData` ni
//! le vrai `AppData` : un dossier « poste » (la base de la caisse, comme
//! `ml.gescom.app`) et un dossier « serveur » (comme `C:\ProgramData\
//! Gescom`). On vérifie ce que la copie promet : le stock, la caisse et
//! les créances de la copie sont ceux de l'originale ; l'originale n'a
//! pas bougé ; rien n'est remplacé quand ce n'est pas sûr.

mod commun;

use std::path::{Path, PathBuf};

use commun::{article_unite, client_reel, depot_defaut, ouvrir_caisse, stock};
use gescom_noyau::argent::{self, ParamsLigneInput};
use gescom_noyau::base::Base;
use gescom_noyau::reprise::{self, codes, Examen};
use gescom_noyau::{amorcage, caisse, creances, images};

/// Deux dossiers jetables, effacés à la fin.
struct Bac {
    racine: PathBuf,
}

impl Bac {
    fn nouveau(nom: &str) -> Bac {
        let racine = std::env::temp_dir().join(format!("gescom_reprise_{}_{nom}", std::process::id()));
        let _ = std::fs::remove_dir_all(&racine);
        std::fs::create_dir_all(racine.join("poste")).unwrap();
        std::fs::create_dir_all(racine.join("serveur")).unwrap();
        Bac { racine }
    }
    fn poste(&self) -> PathBuf {
        self.racine.join("poste").join("gescom.db")
    }
    fn serveur(&self) -> PathBuf {
        self.racine.join("serveur").join("gescom.db")
    }
    fn fichiers_serveur(&self) -> Vec<String> {
        let mut n: Vec<String> = std::fs::read_dir(self.racine.join("serveur"))
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        n.sort();
        n
    }
}

impl Drop for Bac {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.racine);
    }
}

fn ligne(art: &(String, String, f64, i64), depot: &str, quantite: f64) -> ParamsLigneInput {
    ParamsLigneInput {
        article_id: art.0.clone(),
        unite_vente_id: art.1.clone(),
        depot_source_id: depot.to_string(),
        source_approvisionnement: "stock".into(),
        quantite,
        facteur: art.2,
        prix_reference: art.3,
        prix_pratique: art.3,
        taux_tva: None,
        a_decouvert: None,
    }
}

/// Un poste qui a travaillé seul : la démo, une caisse ouverte, une vente
/// comptant et une vente à crédit, un logo. La `Base` reste OUVERTE : le
/// journal WAL n'est pas reversé dans le fichier — c'est le cas d'une
/// caisse qu'on vient de fermer brutalement, et une copie du fichier seul
/// perdrait ces ventes.
fn poste_qui_a_vendu(chemin: &Path) -> Base {
    let mut base = Base::ouvrir(&chemin.to_string_lossy()).expect("base du poste");
    amorcage::amorcer(&mut base).unwrap();
    amorcage::donnees_demo(&mut base).unwrap();
    let depot = depot_defaut(&mut base);
    let sucre = article_unite(&mut base, "Sucre");
    let client = client_reel(&mut base);
    ouvrir_caisse(&mut base);
    argent::creer_vente_sur_base(
        &mut base,
        client.clone(),
        depot.clone(),
        "comptant".into(),
        vec![ligne(&sucre, &depot, 2.0)],
        None,
        Some(sucre.3 * 2),
        None,
        None,
    )
    .expect("vente comptant");
    argent::creer_vente_sur_base(&mut base, client, depot.clone(), "credit".into(), vec![ligne(&sucre, &depot, 3.0)], None, None, None, None)
        .expect("vente à crédit");
    images::ecrire_sur_base(&mut base, "logo", "logo.png", b"\x89PNG faux logo", chemin.parent()).expect("logo");
    base
}

/// Une base de serveur neuve, comme le service la laisse à son premier
/// démarrage : amorcée, l'assistant à faire, rien d'autre.
fn serveur_neuf(chemin: &Path) {
    let mut base = Base::ouvrir(&chemin.to_string_lossy()).unwrap();
    amorcage::amorcer(&mut base).unwrap();
}

/// Ce qu'on compare entre l'originale et la copie : le stock du sucre, la
/// caisse, les créances, le nombre de ventes.
fn etat(base: &mut Base) -> (f64, serde_json::Value, serde_json::Value, i64) {
    let depot = depot_defaut(base);
    let sucre = article_unite(base, "Sucre");
    (
        stock(base, &sucre.0, &depot),
        caisse::lire_resume_caisse_sur(base).unwrap(),
        creances::lire_etat_creances_global_sur_base(base).unwrap(),
        commun::compter(base, "SELECT COUNT(*) FROM vente", &[]),
    )
}

fn empreinte(chemin: &Path) -> Vec<u8> {
    std::fs::read(chemin).unwrap()
}

#[test]
fn la_base_du_poste_passe_au_serveur_avec_son_stock_sa_caisse_et_ses_creances() {
    let bac = Bac::nouveau("complete");
    let mut poste = poste_qui_a_vendu(&bac.poste());
    let avant = etat(&mut poste);
    assert_eq!(avant.3, 2, "deux ventes au poste");
    serveur_neuf(&bac.serveur());

    // L'installeur demande : il y a quelque chose à proposer.
    let examen = reprise::examiner(&bac.poste(), &bac.serveur().to_string_lossy());
    assert_eq!(examen.code(), codes::A_PROPOSER, "{examen:?}");
    let c = examen.contenu_du_poste().unwrap().clone();
    assert_eq!((c.ventes, c.clients > 0, c.articles > 0), (2, true, true), "{c:?}");
    assert!(c.pour_installeur().starts_with("2 ventes|"), "{}", c.pour_installeur());

    let fichier_poste = empreinte(&bac.poste());
    let fait = reprise::reprendre(&bac.poste(), &bac.serveur()).expect("reprise");
    assert_eq!(fait.contenu.ventes, 2);
    assert!(fait.marque_manquante.is_none(), "{:?}", fait.marque_manquante);

    // La copie : le même stock, la même caisse, les mêmes créances — et
    // les ventes qui n'étaient encore que dans le journal WAL.
    let mut copie = Base::ouvrir(&bac.serveur().to_string_lossy()).unwrap();
    assert_eq!(etat(&mut copie), avant);

    // L'originale n'a pas bougé (la Base du poste est encore ouverte).
    assert_eq!(empreinte(&bac.poste()), fichier_poste, "le fichier du poste ne doit pas changer");
    assert_eq!(etat(&mut poste), avant);

    // La base neuve du serveur est mise de côté, pas effacée ; aucun
    // fichier de travail ne traîne.
    let a_cote = fait.neuve_mise_de_cote.clone().expect("base neuve mise de côté");
    assert!(a_cote.exists());
    assert!(reprise::lire_contenu(&a_cote).unwrap().unwrap().est_neuve());
    assert!(
        !bac.fichiers_serveur().iter().any(|n| n.contains("reprise-en-cours")),
        "{:?}",
        bac.fichiers_serveur()
    );

    // Le logo est copié chez le serveur, et la COPIE le désigne là ;
    // l'originale garde son chemin.
    let logo_serveur = bac.racine.join("serveur").join("logo.png");
    assert!(logo_serveur.exists(), "{:?}", bac.fichiers_serveur());
    let chemin_logo = |b: &mut Base| {
        b.lire_une("SELECT logo_chemin FROM parametres_societe WHERE id = 1", &[], |r| r.get::<String>(0))
            .unwrap()
            .unwrap()
    };
    assert_eq!(PathBuf::from(chemin_logo(&mut copie)), logo_serveur);
    assert_eq!(PathBuf::from(chemin_logo(&mut poste)), bac.racine.join("poste").join("logo.png"));
    let logo = images::lire_base64_sur_base(&mut copie, "logo", Some(&bac.racine.join("serveur"))).unwrap();
    assert!(logo.is_some_and(|l| l.starts_with("data:image/png;base64,")));

    // La marque est posée : l'installeur ne repropose plus, et le serveur
    // lancé sans --base refusera l'ancienne base.
    let marque = reprise::marque(&bac.racine.join("poste")).expect("marque");
    assert!(!marque.laissee);
    assert!(marque.raison().contains("reprise par Gescom Serveur"), "{}", marque.raison());
    assert!(matches!(reprise::examiner(&bac.poste(), &bac.serveur().to_string_lossy()), Examen::DejaFaite(_)));
}

#[test]
fn un_serveur_sans_base_du_tout_recoit_la_copie() {
    // Premiere installation : le service n'a pas encore demarre, sa base
    // n'existe pas.
    let bac = Bac::nouveau("sans_base");
    let _poste = poste_qui_a_vendu(&bac.poste());
    assert_eq!(reprise::examiner(&bac.poste(), &bac.serveur().to_string_lossy()).code(), codes::A_PROPOSER);
    let fait = reprise::reprendre(&bac.poste(), &bac.serveur()).unwrap();
    assert!(fait.neuve_mise_de_cote.is_none());
    assert_eq!(reprise::lire_contenu(&bac.serveur()).unwrap().unwrap().ventes, 2);
}

#[test]
fn rien_a_proposer_sans_base_de_poste_ou_avec_une_base_neuve() {
    let bac = Bac::nouveau("rien");
    let cible = bac.serveur().to_string_lossy().to_string();
    assert_eq!(reprise::examiner(&bac.poste(), &cible), Examen::Rien);
    assert_eq!(reprise::reprendre(&bac.poste(), &bac.serveur()).unwrap_err().code, codes::RIEN_A_REPRENDRE);

    serveur_neuf(&bac.poste());
    assert_eq!(reprise::examiner(&bac.poste(), &cible), Examen::Rien);
    assert_eq!(reprise::reprendre(&bac.poste(), &bac.serveur()).unwrap_err().code, codes::RIEN_A_REPRENDRE);
    assert!(!bac.serveur().exists(), "rien n'est créé chez le serveur");
}

#[test]
fn deux_bases_qui_ont_vendu_ne_se_remplacent_pas() {
    let bac = Bac::nouveau("deux");
    let _poste = poste_qui_a_vendu(&bac.poste());
    {
        let _serveur = poste_qui_a_vendu(&bac.serveur());
    }
    let avant = empreinte(&bac.serveur());
    let examen = reprise::examiner(&bac.poste(), &bac.serveur().to_string_lossy());
    assert_eq!(examen.code(), codes::DEUX_BASES, "{examen:?}");
    let refus = reprise::reprendre(&bac.poste(), &bac.serveur()).unwrap_err();
    assert_eq!(refus.code, codes::DEUX_BASES);
    assert!(refus.message.contains("n'est pas neuve"), "{}", refus.message);
    assert_eq!(empreinte(&bac.serveur()), avant, "la base du serveur n'est pas touchée");

    // Trancher pour le serveur : la marque, sans copie ; l'examen se tait.
    reprise::laisser(&bac.poste(), &bac.serveur().to_string_lossy()).unwrap();
    let examen = reprise::examiner(&bac.poste(), &bac.serveur().to_string_lossy());
    assert_eq!(examen.code(), codes::RIEN_OU_FAIT);
    let Examen::DejaFaite(m) = examen else { panic!("{examen:?}") };
    assert!(m.laissee && m.raison().contains("laissée de côté"), "{}", m.raison());
    assert_eq!(empreinte(&bac.serveur()), avant);
}

#[test]
fn une_base_de_poste_abimee_ou_etrangere_est_refusee_et_rien_ne_bouge() {
    let bac = Bac::nouveau("abimee");
    std::fs::write(bac.poste(), b"ceci n'est pas une base").unwrap();
    serveur_neuf(&bac.serveur());
    let avant = bac.fichiers_serveur();
    let examen = reprise::examiner(&bac.poste(), &bac.serveur().to_string_lossy());
    assert_eq!(examen.code(), codes::SOURCE_ILLISIBLE, "{examen:?}");
    assert_eq!(reprise::reprendre(&bac.poste(), &bac.serveur()).unwrap_err().code, codes::SOURCE_ILLISIBLE);
    assert_eq!(bac.fichiers_serveur(), avant);
}

#[test]
fn vers_postgresql_la_reprise_refuse_en_le_disant() {
    let bac = Bac::nouveau("pg");
    let _poste = poste_qui_a_vendu(&bac.poste());
    let url = "postgresql://gescom:secret@127.0.0.1:5432/gescom";
    assert_eq!(reprise::examiner(&bac.poste(), url).code(), codes::CIBLE_POSTGRES);
    let refus = reprise::reprendre(&bac.poste(), Path::new(url)).unwrap_err();
    assert_eq!(refus.code, codes::CIBLE_POSTGRES);
    assert!(refus.message.contains("PostgreSQL"));
}

/// Le service tient sa base ouverte : Windows refuse de la renommer. La
/// reprise s'arrête AVANT de toucher quoi que ce soit, et le dit.
#[cfg(windows)]
#[test]
fn une_base_de_serveur_ouverte_n_est_pas_remplacee() {
    let bac = Bac::nouveau("ouverte");
    let _poste = poste_qui_a_vendu(&bac.poste());
    serveur_neuf(&bac.serveur());
    let service = Base::ouvrir(&bac.serveur().to_string_lossy()).unwrap();
    let refus = reprise::reprendre(&bac.poste(), &bac.serveur()).unwrap_err();
    assert_eq!(refus.code, codes::CIBLE_OUVERTE, "{}", refus.message);
    assert!(refus.message.contains("sc stop GescomServeur"), "{}", refus.message);
    drop(service);
    assert!(reprise::lire_contenu(&bac.serveur()).unwrap().unwrap().est_neuve(), "toujours la base neuve");
    assert!(reprise::marque(&bac.racine.join("poste")).is_none(), "pas de marque sans reprise");
    assert!(
        !bac.fichiers_serveur().iter().any(|n| n.contains("reprise-en-cours") || n.contains("avant-reprise")),
        "{:?}",
        bac.fichiers_serveur()
    );
}

/// Le dernier : la copie se sert comme n'importe quelle base — une vente
/// de plus, sous le détecteur de cloisonnement.
#[test]
fn la_copie_passe_le_detecteur() {
    let bac = Bac::nouveau("detecteur");
    let _poste = poste_qui_a_vendu(&bac.poste());
    reprise::reprendre(&bac.poste(), &bac.serveur()).unwrap();
    let mut base = Base::ouvrir(&bac.serveur().to_string_lossy()).unwrap();
    base.auditer(true);
    let depot = depot_defaut(&mut base);
    let sucre = article_unite(&mut base, "Sucre");
    let client = client_reel(&mut base);
    let avant = stock(&mut base, &sucre.0, &depot);
    argent::creer_vente_sur_base(&mut base, client, depot.clone(), "comptant".into(), vec![ligne(&sucre, &depot, 1.0)], None, Some(sucre.3), None, None)
        .expect("une vente sur la copie");
    assert_eq!(stock(&mut base, &sucre.0, &depot), avant - 1.0);
    caisse::lire_resume_caisse_sur(&mut base).expect("la caisse se lit");
    creances::lire_etat_creances_global_sur_base(&mut base).expect("les créances se lisent");
}
