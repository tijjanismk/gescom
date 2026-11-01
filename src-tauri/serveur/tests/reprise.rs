//! D54, par le vrai binaire : ce que l'installeur appelle et lit.
//!
//! `--reprendre … --examiner --installeur` puis `--reprendre …
//! --installeur` : les codes de sortie et la ligne ASCII que le script
//! NSIS découpe ; le garde-fou de la marque ; `--restaurer` sans `--base`
//! sur un ordinateur qui a Gescom Serveur. Tout dans des dossiers
//! jetables : `--base`, `GESCOM_DONNEES` et `GESCOM_SERVEUR_JSON` (build
//! de débogage) remplacent `AppData` et `ProgramData`.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use gescom_noyau::amorcage;
use gescom_noyau::base::Base;

struct Bac {
    racine: PathBuf,
}

impl Bac {
    fn nouveau(nom: &str) -> Bac {
        let racine = std::env::temp_dir().join(format!("gescom_reprise_cli_{}_{nom}", std::process::id()));
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
}

impl Drop for Bac {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.racine);
    }
}

/// Une base de poste qui a des articles (la démo) : pas neuve.
fn poste_avec_articles(chemin: &Path) {
    let mut base = Base::ouvrir(&chemin.to_string_lossy()).unwrap();
    amorcage::amorcer(&mut base).unwrap();
    amorcage::donnees_demo(&mut base).unwrap();
}

fn serveur(args: &[&str], retouche: impl FnOnce(&mut Command)) -> Output {
    let mut c = Command::new(env!("CARGO_BIN_EXE_gescom-serveur"));
    c.args(args).env("GESCOM_DECOUVERTE_PORT", "0").env_remove("GESCOM_SERVEUR_JSON").env_remove("GESCOM_DONNEES");
    retouche(&mut c);
    c.output().expect("lancer gescom-serveur")
}

fn texte(o: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr))
}

#[test]
fn l_installeur_examine_reprend_puis_ne_repropose_plus() {
    let bac = Bac::nouveau("installeur");
    poste_avec_articles(&bac.poste());
    let p = bac.poste().to_string_lossy().to_string();
    let s = bac.serveur().to_string_lossy().to_string();

    let examen = serveur(&["--reprendre", &p, "--base", &s, "--examiner", "--installeur"], |_| {});
    assert_eq!(examen.status.code(), Some(10), "{}", texte(&examen));
    // Une ligne ASCII, quatre champs : ventes|articles|clients|derniere.
    let ligne = String::from_utf8_lossy(&examen.stdout).trim().to_string();
    assert!(ligne.is_ascii() && ligne.split('|').count() == 4, "« {ligne} »");
    assert!(examen.stderr.is_empty(), "rien sur stderr : nsExec mêle les deux");
    assert!(!bac.serveur().exists(), "examiner ne crée rien");

    let fait = serveur(&["--reprendre", &p, "--base", &s, "--installeur"], |_| {});
    assert_eq!(fait.status.code(), Some(0), "{}", texte(&fait));
    assert_eq!(String::from_utf8_lossy(&fait.stdout).trim(), ligne, "les mêmes nombres après la copie");
    assert!(bac.serveur().exists());
    let journal = std::fs::read_to_string(bac.serveur().with_extension("log")).unwrap_or_default();
    assert!(journal.contains("Base du poste reprise"), "{journal}");

    let encore = serveur(&["--reprendre", &p, "--base", &s, "--examiner", "--installeur"], |_| {});
    assert_eq!(encore.status.code(), Some(0), "la marque fait taire l'examen : {}", texte(&encore));

    // Une seconde reprise à la main : la base du serveur n'est plus neuve.
    let refus = serveur(&["--reprendre", &p, "--base", &s], |_| {});
    assert_eq!(refus.status.code(), Some(11), "{}", texte(&refus));
    assert!(texte(&refus).contains("n'est pas neuve"), "{}", texte(&refus));
}

#[test]
fn apres_la_reprise_le_serveur_lance_sans_base_refuse_l_ancienne() {
    // Le raccourci de démarrage lance le serveur sans --base, sur le
    // dossier de données du compte (`GESCOM_DONNEES` en débogage).
    let bac = Bac::nouveau("garde_fou");
    poste_avec_articles(&bac.poste());
    let p = bac.poste().to_string_lossy().to_string();
    let s = bac.serveur().to_string_lossy().to_string();
    assert_eq!(serveur(&["--reprendre", &p, "--base", &s, "--installeur"], |_| {}).status.code(), Some(0));

    let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port().to_string();
    let conf = bac.racine.join("serveur.json");
    let sortie = serveur(&["--hote", "127.0.0.1", "--port", &port], |c| {
        c.env("GESCOM_DONNEES", bac.racine.join("poste")).env("GESCOM_SERVEUR_JSON", &conf);
    });
    assert!(sortie.status.success(), "il s'efface sans erreur : {}", texte(&sortie));
    let journal = std::fs::read_to_string(bac.racine.join("poste").join("gescom.log")).unwrap_or_default();
    assert!(journal.contains("reprise par Gescom Serveur"), "{journal}");
    assert!(journal.contains("Serveur non démarré"), "{journal}");
}

#[test]
fn laisser_garde_la_base_du_serveur_et_fait_taire_l_installeur() {
    let bac = Bac::nouveau("laisser");
    poste_avec_articles(&bac.poste());
    poste_avec_articles(&bac.serveur());
    let p = bac.poste().to_string_lossy().to_string();
    let s = bac.serveur().to_string_lossy().to_string();
    let examen = serveur(&["--reprendre", &p, "--base", &s, "--examiner", "--installeur"], |_| {});
    assert_eq!(examen.status.code(), Some(11), "deux bases : {}", texte(&examen));
    let laisse = serveur(&["--reprendre", &p, "--base", &s, "--laisser"], |_| {});
    assert_eq!(laisse.status.code(), Some(0), "{}", texte(&laisse));
    let examen = serveur(&["--reprendre", &p, "--base", &s, "--examiner", "--installeur"], |_| {});
    assert_eq!(examen.status.code(), Some(0), "{}", texte(&examen));
}

#[test]
fn sans_base_la_reprise_ne_vise_jamais_le_vrai_programdata_en_debogage() {
    let bac = Bac::nouveau("debogage");
    poste_avec_articles(&bac.poste());
    let sortie = serveur(&["--reprendre", &bac.poste().to_string_lossy(), "--examiner"], |_| {});
    assert_eq!(sortie.status.code(), Some(2), "{}", texte(&sortie));
    assert!(texte(&sortie).contains("--base"), "{}", texte(&sortie));

    // Avec un serveur.json d'essai : sa clé base est la cible.
    let conf = bac.racine.join("serveur").join("serveur.json");
    std::fs::write(&conf, "{\"port\": 40000}").unwrap();
    let sortie = serveur(&["--reprendre", &bac.poste().to_string_lossy(), "--examiner", "--installeur"], |c| {
        c.env("GESCOM_SERVEUR_JSON", &conf);
    });
    assert_eq!(sortie.status.code(), Some(10), "gescom.db à côté de serveur.json, absente : {}", texte(&sortie));
}

#[test]
fn restaurer_sans_base_sur_un_ordinateur_qui_a_le_serveur_refuse_et_nomme_les_deux() {
    let bac = Bac::nouveau("restaurer");
    let conf = bac.racine.join("serveur").join("serveur.json");
    std::fs::write(&conf, "{\"base\": \"C:\\\\Essai\\\\gescom.db\", \"port\": 40000}").unwrap();
    let sortie = serveur(&["--restaurer", "copie.db"], |c| {
        c.env("GESCOM_SERVEUR_JSON", &conf).env("GESCOM_DONNEES", bac.racine.join("poste"));
    });
    assert_eq!(sortie.status.code(), Some(1), "{}", texte(&sortie));
    let t = texte(&sortie);
    assert!(t.contains("dire avec --base"), "{t}");
    assert!(t.contains(r#"--base "C:\Essai\gescom.db""#), "{t}");
    assert!(t.contains(&bac.racine.join("poste").join("gescom.db").to_string_lossy().to_string()), "{t}");
    assert!(!bac.poste().exists(), "rien n'est restauré nulle part");
}
