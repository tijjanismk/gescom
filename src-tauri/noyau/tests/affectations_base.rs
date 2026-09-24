//! E-2 — l'affectation comptable (D23), sur les deux moteurs : chaque
//! type d'operation a un compte ; un reglage vaut pour son dossier ; le
//! defaut ne s'ecrit pas.

mod commun;

use commun::*;
use gescom_noyau::coeur::affectations::OPERATIONS;
use gescom_noyau::dossiers::{self, DOSSIER_DEFAUT};
use gescom_noyau::{affectations, parametres, plan_comptable};

#[test]
fn chaque_type_d_operation_a_un_compte_qui_existe() {
    let mut base = base_avec_demo();
    let lues = affectations::lire_sur(&mut base).unwrap();
    assert_eq!(lues.len(), OPERATIONS.len());
    for a in &lues {
        assert!(a["libelle_compte"].is_string(), "{} : le compte {} n'est pas dans le plan", a["operation"], a["compte"]);
        assert_eq!(a["compte"], a["defaut"]);
        assert_eq!(a["modifiee"], false);
    }
    // Toutes les ventes, achats, trésoreries, dépenses du logiciel trouvent un compte.
    let t = affectations::table_sur(&mut base).unwrap();
    for cle in ["ventes:marchandises", "ventes:tva", "ventes:clients", "achats:fournisseurs", "tresorerie:orange_money",
        "depense:loyer", "caisse:ecart_manquant", "ventes:irrecouvrables"] {
        assert!(t.contains_key(cle), "{cle}");
    }
    assert_eq!(t["tresorerie:especes"], "571");
    assert_eq!(t["ventes:irrecouvrables"], "6511");
}

#[test]
fn un_reglage_vaut_pour_son_dossier_et_le_defaut_revient() {
    let mut base = base_avec_demo();
    let b = dossiers::creer_dossier_sur(&mut base, "B".into(), "Boutique B".into(), None, None).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    // La caisse de la boutique sur un sous-compte à elle.
    plan_comptable::ajouter_sous_compte_sur(&mut base, "5711".into(), "Caisse boutique Bamako".into()).unwrap();
    let r = affectations::definir_sur(&mut base, "tresorerie:especes".into(), Some("5711".into())).unwrap();
    assert_eq!(r["modifiee"], true);
    assert_eq!(affectations::compte_sur(&mut base, "tresorerie:especes").unwrap(), "5711");
    let lue = affectations::lire_sur(&mut base).unwrap();
    let especes = lue.iter().find(|a| a["operation"] == "tresorerie:especes").unwrap();
    assert_eq!(especes["libelle_compte"], "Caisse boutique Bamako");

    // Le dossier B n'est pas touché.
    base.choisir_dossier(&b).unwrap();
    assert_eq!(affectations::compte_sur(&mut base, "tresorerie:especes").unwrap(), "571");
    // Et le sous-compte de l'origine n'existe pas chez lui.
    assert!(affectations::definir_sur(&mut base, "tresorerie:especes".into(), Some("5711".into()))
        .unwrap_err()
        .contains("n'existe pas"));
    base.choisir_dossier(DOSSIER_DEFAUT).unwrap();

    // Ce qui ne convient pas : refusé, en disant quoi mettre.
    let e = affectations::definir_sur(&mut base, "ventes:marchandises".into(), Some("601".into())).unwrap_err();
    assert!(e.contains("commence par 70"), "{e}");
    assert!(affectations::definir_sur(&mut base, "inconnue".into(), Some("571".into())).is_err());

    // Revenir au défaut : plus rien en base, le défaut suivra les versions.
    affectations::definir_sur(&mut base, "tresorerie:especes".into(), None).unwrap();
    assert_eq!(affectations::compte_sur(&mut base, "tresorerie:especes").unwrap(), "571");
    // Poser explicitement le défaut n'écrit rien non plus.
    affectations::definir_sur(&mut base, "ventes:marchandises".into(), Some("701".into())).unwrap();
    let lignes = compter(
        &mut base,
        "SELECT CAST(COUNT(*) AS BIGINT) FROM affectation_comptable WHERE dossier_id = ?1",
        &parametres![DOSSIER_DEFAUT],
    );
    assert_eq!(lignes, 0);
    let n = compter(
        &mut base,
        "SELECT CAST(COUNT(*) AS BIGINT) FROM journal WHERE type_evenement = 'affectation_modifiee' AND dossier_id = ?1",
        &parametres![DOSSIER_DEFAUT],
    );
    assert_eq!(n, 3, "chaque réglage au journal");
}

#[test]
fn tout_ce_qui_est_porte_ici_passe_le_detecteur() {
    let mut base = base_avec_demo();
    base.auditer(true);
    affectations::lire_sur(&mut base).unwrap();
    affectations::table_sur(&mut base).unwrap();
    affectations::definir_sur(&mut base, "depense:loyer".into(), Some("6222".into())).unwrap_err();
    plan_comptable::ajouter_sous_compte_sur(&mut base, "6221".into(), "Loyer du magasin".into()).unwrap();
    affectations::definir_sur(&mut base, "depense:loyer".into(), Some("6221".into())).unwrap();
    assert_eq!(affectations::compte_sur(&mut base, "depense:loyer").unwrap(), "6221");
}
