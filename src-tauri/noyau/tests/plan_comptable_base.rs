//! E-1 — le plan comptable SYSCOHADA en base (decision D23), sur les
//! deux moteurs : seme une fois, commun a tous les dossiers, et des
//! sous-comptes par dossier (`4111 Client Coulibaly`).

mod commun;

use commun::*;
use gescom_noyau::base::Base;
use gescom_noyau::coeur::plan_comptable::SYSCOHADA;
use gescom_noyau::dossiers::{self, DOSSIER_DEFAUT};
use gescom_noyau::{amorcage, parametres, plan_comptable, portes};

fn plan(base: &mut Base) -> Vec<serde_json::Value> {
    plan_comptable::lire_sur(base).unwrap()
}

#[test]
fn le_plan_est_seme_une_fois_commun_et_ordonne() {
    let mut base = base_avec_demo();
    let p = plan(&mut base);
    assert_eq!(p.len(), SYSCOHADA.len(), "une centaine de comptes, tous là");
    for classe in 1..=7 {
        assert!(p.iter().any(|c| c["classe"] == classe), "classe {classe}");
    }
    let c411 = p.iter().find(|c| c["numero"] == "411").unwrap();
    assert_eq!(c411["libelle"], "Clients");
    assert_eq!(c411["parent"], "41");
    assert_eq!(c411["sous_compte"], false);
    // L'ordre du plan : 401 avant 4091, 4091 avant 41.
    let pos = |n: &str| p.iter().position(|c| c["numero"] == n).unwrap();
    assert!(pos("401") < pos("4091") && pos("4091") < pos("41"));

    // Rejoué (chaque démarrage) : rien de plus, et un libellé changé
    // par le patron n'est pas écrasé.
    base.executer("UPDATE compte_comptable SET libelle = 'Caisse boutique' WHERE numero = '571'", &[]).unwrap();
    assert_eq!(plan_comptable::semer_sur(&mut base).unwrap(), 0);
    assert_eq!(plan(&mut base).len(), SYSCOHADA.len());
    assert_eq!(plan_comptable::libelle_sur(&mut base, "571").unwrap().as_deref(), Some("Caisse boutique"));
}

#[test]
fn un_sous_compte_appartient_a_son_dossier() {
    let mut base = base_avec_demo();
    let b = dossiers::creer_dossier_sur(&mut base, "B".into(), "Boutique B".into(), None, None).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    let c = plan_comptable::ajouter_sous_compte_sur(&mut base, " 4111 ".into(), " Client Coulibaly ".into()).unwrap();
    assert_eq!(c["numero"], "4111");
    assert_eq!(c["parent"], "411");
    let p = plan(&mut base);
    assert_eq!(p.len(), SYSCOHADA.len() + 1);
    let s = p.iter().find(|x| x["numero"] == "4111").unwrap();
    assert_eq!(s["sous_compte"], true);
    assert_eq!(s["libelle"], "Client Coulibaly");
    // Ni deux fois, ni hors du plan.
    assert!(plan_comptable::ajouter_sous_compte_sur(&mut base, "4111".into(), "Autre".into()).unwrap_err().contains("existe"));
    assert!(plan_comptable::ajouter_sous_compte_sur(&mut base, "4431".into(), "x".into()).unwrap_err().contains("existe"));
    assert!(plan_comptable::ajouter_sous_compte_sur(&mut base, "1901".into(), "x".into()).is_err());

    // Le dossier B ne le voit pas, et peut avoir SON 4111.
    base.choisir_dossier(&b).unwrap();
    assert_eq!(plan(&mut base).len(), SYSCOHADA.len(), "le sous-compte de l'origine n'est pas chez B");
    plan_comptable::ajouter_sous_compte_sur(&mut base, "4111".into(), "Client Traoré".into()).unwrap();
    assert_eq!(plan_comptable::libelle_sur(&mut base, "4111").unwrap().as_deref(), Some("Client Traoré"));
    base.choisir_dossier(DOSSIER_DEFAUT).unwrap();
    assert_eq!(plan_comptable::libelle_sur(&mut base, "4111").unwrap().as_deref(), Some("Client Coulibaly"));

    let n = compter(
        &mut base,
        "SELECT CAST(COUNT(*) AS BIGINT) FROM journal WHERE type_evenement = 'sous_compte_cree' AND dossier_id = ?1",
        &parametres![DOSSIER_DEFAUT],
    );
    assert_eq!(n, 1, "au journal du dossier où on l'a créé");
}

#[test]
fn une_base_installee_recoit_le_plan_et_le_comptable_sa_permission_une_fois() {
    let mut base = base_avec_demo();
    // Une base d'avant E-1 : ni plan, ni marque, un comptable sans la permission.
    base.executer("DELETE FROM compte_comptable", &[]).unwrap();
    base.executer("DELETE FROM config_app WHERE cle = 'migration_v3_comptabilite'", &[]).unwrap();
    base.executer(
        "UPDATE role SET permissions = ?1 WHERE nom = 'comptable'",
        &parametres![r#"["creances:gerer","journal:lire"]"#],
    )
    .unwrap();

    amorcage::migrations_de_donnees(&mut base);
    assert_eq!(plan(&mut base).len(), SYSCOHADA.len());
    assert!(portes::permissions_de_sur(&mut base, "x", "comptable").contains("comptabilite:gerer"));

    // Le patron la lui retire : le démarrage suivant ne la rend pas.
    base.executer(
        "UPDATE role SET permissions = ?1 WHERE nom = 'comptable'",
        &parametres![r#"["creances:gerer","journal:lire"]"#],
    )
    .unwrap();
    amorcage::migrations_de_donnees(&mut base);
    assert!(!portes::permissions_de_sur(&mut base, "x", "comptable").contains("comptabilite:gerer"));
}

#[test]
fn tout_ce_qui_est_porte_ici_passe_le_detecteur() {
    let mut base = base_avec_demo();
    base.auditer(true);
    plan_comptable::lire_sur(&mut base).unwrap();
    plan_comptable::libelle_sur(&mut base, "411").unwrap();
    plan_comptable::ajouter_sous_compte_sur(&mut base, "40111".into(), "Fournisseur Diallo".into()).unwrap();
    plan_comptable::semer_sur(&mut base).unwrap();
}
