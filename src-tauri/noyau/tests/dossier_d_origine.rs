//! v3, D-5 : la migration d'une base existante. Tout ce qu'elle
//! contient est deja dans le dossier `defaut` ; au premier demarrage
//! du serveur v3, ce dossier prend le nom de la societe et des dates
//! qui couvrent ses plus anciennes ecritures — sinon un reglement
//! tardif d'une vente de 2024 serait refuse par le garde-fou (D-4).
//!
//! Sur SQLite en memoire par defaut, sur PostgreSQL avec `GESCOM_PG` ;
//! le dernier scenario suit le chemin FICHIER du serveur, celui d'un
//! commercant qui passe de la v2 a la v3 sans rien installer.

mod commun;

use commun::*;
use gescom_noyau::base::Base;
use gescom_noyau::dossiers::{self, DOSSIER_DEFAUT};
use gescom_noyau::parametres;

/// Une base comme la v2 la laissait : pas de marque de migration, le
/// dossier au nom d'usine, une societe nommee, une depense de 2024.
fn comme_en_v2(base: &mut Base, societe: &str, vieille_date: &str) {
    base.executer("DELETE FROM config_app WHERE cle = 'migration_v3_dossier_origine'", &[]).unwrap();
    base.executer(
        "UPDATE dossier SET societe = 'Ma boutique' WHERE id = ?1",
        &parametres![DOSSIER_DEFAUT],
    )
    .unwrap();
    base.executer("UPDATE parametres_societe SET nom = ?1 WHERE id = 1", &parametres![societe]).unwrap();
    // Une dépense ancienne : la démo sur `Base` n'a pas de vente.
    ouvrir_caisse(base);
    gescom_noyau::caisse::enregistrer_depense_sur_base(base, 1500, "Ampoules".into(), None, None, None).unwrap();
    base.executer(
        "UPDATE mouvement_caisse SET date_mouvement = ?1 WHERE dossier_id = ?2",
        &parametres![vieille_date, DOSSIER_DEFAUT],
    )
    .unwrap();
}

fn societe_du_dossier(base: &mut Base) -> String {
    dossiers::dossier_ouvert_sur(base, DOSSIER_DEFAUT).unwrap().societe
}

fn journal_d_origine(base: &mut Base) -> i64 {
    compter(
        base,
        "SELECT CAST(COUNT(*) AS BIGINT) FROM journal WHERE type_evenement = 'dossier_d_origine' AND dossier_id = ?1",
        &parametres![DOSSIER_DEFAUT],
    )
}

#[test]
fn une_base_v2_devient_un_dossier_a_son_nom_et_a_ses_dates() {
    let mut base = base_avec_demo();
    comme_en_v2(&mut base, "Quincaillerie Traoré", "2024-06-15T10:30:00");
    let annee = &gescom_noyau::utils::maintenant_iso()[..4];

    // Avant : un règlement daté du 20 juin 2024 serait refusé.
    assert!(dossiers::verifier_date_sur(&mut base, "2024-06-20").is_err());

    let fait = dossiers::migrer_dossier_d_origine_sur(&mut base).unwrap().expect("la migration a lieu");
    assert_eq!(fait["societe"], "Quincaillerie Traoré");
    assert_eq!(fait["debut"], "2024-01-01");
    assert_eq!(fait["plus_ancienne_ecriture"], "2024-06-15");

    assert_eq!(societe_du_dossier(&mut base), "Quincaillerie Traoré", "le dossier porte le nom de la société");
    let ex = dossiers::lire_exercices_sur(&mut base).unwrap();
    assert_eq!(ex.len(), 1, "un seul exercice, reculé — pas un de plus");
    assert_eq!(ex[0]["date_debut"], "2024-01-01");
    assert_eq!(ex[0]["date_fin"], format!("{annee}-12-31"), "la fin ne bouge pas");
    assert!(dossiers::verifier_date_sur(&mut base, "2024-06-20").is_ok(), "le règlement tardif passe");
    assert!(dossiers::verifier_date_sur(&mut base, &gescom_noyau::utils::maintenant_iso()).is_ok(), "aujourd'hui aussi");
    assert!(dossiers::verifier_date_sur(&mut base, "2023-12-31").is_err(), "avant la plus ancienne année : non");
    assert_eq!(journal_d_origine(&mut base), 1, "au journal : ce que la migration a fait");

    // Tout ce qui existait est dans ce dossier, et il est seul.
    let liste = dossiers::lire_dossiers_sur(&mut base).unwrap();
    assert_eq!(liste.len(), 1);
    assert_eq!(liste[0]["societe"], "Quincaillerie Traoré");
    let hors = compter(
        &mut base,
        "SELECT CAST(COUNT(*) AS BIGINT) FROM mouvement_caisse WHERE dossier_id <> ?1",
        &parametres![DOSSIER_DEFAUT],
    );
    assert_eq!(hors, 0);

    // Une fois : au redémarrage suivant, rien.
    assert!(dossiers::migrer_dossier_d_origine_sur(&mut base).unwrap().is_none());
    assert_eq!(journal_d_origine(&mut base), 1);
}

#[test]
fn ce_que_le_patron_a_deja_decide_n_est_pas_touche() {
    let mut base = base_avec_demo();
    comme_en_v2(&mut base, "Quincaillerie Traoré", "2024-06-15");
    // Il a déjà renommé son dossier et ouvert un second exercice.
    let annee: i32 = gescom_noyau::utils::maintenant_iso()[..4].parse().unwrap();
    dossiers::renommer_dossier_sur(&mut base, DOSSIER_DEFAUT.into(), "Chez Awa".into()).unwrap();
    dossiers::ouvrir_exercice_sur(&mut base, format!("{}-01-01", annee + 1), format!("{}-12-31", annee + 1)).unwrap();
    base.executer("DELETE FROM config_app WHERE cle = 'migration_v3_dossier_origine'", &[]).unwrap();
    let avant = compter(&mut base, "SELECT CAST(COUNT(*) AS BIGINT) FROM journal WHERE dossier_id = ?1", &parametres![DOSSIER_DEFAUT]);

    let fait = dossiers::migrer_dossier_d_origine_sur(&mut base).unwrap().expect("passe, et pose sa marque");
    assert!(fait["societe"].is_null() && fait["debut"].is_null(), "{fait}");
    assert_eq!(societe_du_dossier(&mut base), "Chez Awa");
    let ex = dossiers::lire_exercices_sur(&mut base).unwrap();
    assert_eq!(ex[0]["date_debut"], format!("{annee}-01-01"), "deux exercices : le patron a décidé");
    let apres = compter(&mut base, "SELECT CAST(COUNT(*) AS BIGINT) FROM journal WHERE dossier_id = ?1", &parametres![DOSSIER_DEFAUT]);
    assert_eq!(apres, avant, "rien fait, rien au journal");
    assert!(dossiers::migrer_dossier_d_origine_sur(&mut base).unwrap().is_none());
}

#[test]
fn renommer_un_dossier_garde_son_code_et_le_dit_au_journal() {
    let mut base = base_avec_demo();
    base.auditer(true);
    let r = dossiers::renommer_dossier_sur(&mut base, DOSSIER_DEFAUT.into(), "  Quincaillerie Traoré ".into()).unwrap();
    assert_eq!(r["societe"], "Quincaillerie Traoré");
    let d = dossiers::dossier_ouvert_sur(&mut base, DOSSIER_DEFAUT).unwrap();
    assert_eq!(d.code, "PRINCIPAL", "le code ne change pas : il préfixe les codes des tiers");
    assert_eq!(d.societe, "Quincaillerie Traoré");
    assert!(dossiers::renommer_dossier_sur(&mut base, DOSSIER_DEFAUT.into(), " ".into()).unwrap_err().contains("vide"));
    assert_eq!(dossiers::renommer_dossier_sur(&mut base, "inconnu".into(), "X".into()).unwrap_err(), "Dossier introuvable.");
    let n = compter(
        &mut base,
        "SELECT CAST(COUNT(*) AS BIGINT) FROM journal WHERE type_evenement = 'dossier_renomme' AND dossier_id = ?1 AND ancien_valeur LIKE '%Ma boutique%'",
        &parametres![DOSSIER_DEFAUT],
    );
    assert_eq!(n, 1);
    dossiers::migrer_dossier_d_origine_sur(&mut base).unwrap();
}

/// Le chemin d'un commercant : son fichier v2, le serveur v3 lance
/// dessus. Le fichier est prepare comme le serveur le prepare
/// (`initialiser_tables`, `amorcer_si_vide`), puis la `Base` migre.
#[test]
fn le_fichier_sqlite_d_un_commercant_migre_au_premier_demarrage() {
    if std::env::var("GESCOM_PG").is_ok() {
        return; // le chemin fichier est propre a SQLite
    }
    let fichier = std::env::temp_dir().join(format!("gescom-d5-{}.db", uuid::Uuid::new_v4()));
    let chemin = fichier.to_string_lossy().to_string();
    {
        let conn = gescom_noyau::persistance::ouvrir_base(&chemin).unwrap();
        gescom_noyau::persistance::initialiser_tables(&conn).unwrap();
        gescom_noyau::seed::amorcer_si_vide(&conn).unwrap();
    }
    // Ce que la v2 y avait écrit : une dépense de mars 2025, un nom.
    {
        let mut base = Base::ouvrir(&chemin).unwrap();
        base.executer("UPDATE parametres_societe SET nom = 'Boutique Keita' WHERE id = 1", &[]).unwrap();
        ouvrir_caisse(&mut base);
        gescom_noyau::caisse::enregistrer_depense_sur_base(&mut base, 1500, "Ampoules".into(), None, None, None).unwrap();
        base.executer("UPDATE mouvement_caisse SET date_mouvement = '2025-03-10T09:00:00' WHERE dossier_id = ?1", &parametres![DOSSIER_DEFAUT])
            .unwrap();
    }
    // Premier démarrage v3.
    let mut base = Base::ouvrir(&chemin).unwrap();
    let fait = dossiers::migrer_dossier_d_origine_sur(&mut base).unwrap().expect("migration");
    assert_eq!(fait["societe"], "Boutique Keita");
    assert_eq!(fait["debut"], "2025-01-01");
    assert!(dossiers::verifier_date_sur(&mut base, "2025-03-10").is_ok());
    drop(base);
    // Second démarrage : la marque est dans le fichier.
    let mut base = Base::ouvrir(&chemin).unwrap();
    assert!(dossiers::migrer_dossier_d_origine_sur(&mut base).unwrap().is_none());
    drop(base);
    for suffixe in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{chemin}{suffixe}"));
    }
}
