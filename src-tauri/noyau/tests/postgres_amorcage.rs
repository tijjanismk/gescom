//! L'amorcage doit produire la meme boutique sur les deux moteurs.
//!
//! Ces scenarios ne tournent que si `GESCOM_PG` designe une base
//! PostgreSQL : sans elle, ils passent en silence. Un test qui exige un
//! service tiers ne doit pas faire echouer la suite de quelqu'un qui ne
//! l'a pas installe.

use gescom_noyau::amorcage;
use gescom_noyau::base::Base;

fn pg() -> Option<Base> {
    let url = std::env::var("GESCOM_PG").ok()?;
    Base::ouvrir(&url).ok()
}

#[test]
fn postgresql_s_amorce_et_donne_de_quoi_se_connecter() {
    let Some(mut base) = pg() else { return };

    // Table rase : le test doit partir du meme etat a chaque fois.
    base.executer_lot("DROP SCHEMA public CASCADE; CREATE SCHEMA public;")
        .expect("schéma remis à zéro");

    assert!(amorcage::amorcer(&mut base).expect("amorçage"));

    let roles = base
        .lire_plusieurs("SELECT nom FROM role ORDER BY nom", &[], |r| {
            r.get::<String>(0)
        })
        .expect("rôles");
    for attendu in ["caissier", "comptable", "employe", "magasinier", "patron", "superadmin"] {
        assert!(roles.contains(&attendu.to_string()), "{attendu} absent : {roles:?}");
    }

    let comptes = base
        .lire_une("SELECT COUNT(*) FROM utilisateur_auth", &[], |r| r.get::<i64>(0))
        .unwrap()
        .unwrap();
    assert_eq!(comptes, 2);

    // Le minimum pour vendre.
    let depots = base
        .lire_une(
            "SELECT COUNT(*) FROM depot WHERE est_defaut = 1",
            &[],
            |r| r.get::<i64>(0),
        )
        .unwrap()
        .unwrap();
    assert_eq!(depots, 1);
}

#[test]
fn l_amorcage_ne_se_rejoue_pas_sur_postgresql() {
    let Some(mut base) = pg() else { return };
    base.executer_lot("DROP SCHEMA public CASCADE; CREATE SCHEMA public;").unwrap();

    assert!(amorcage::amorcer(&mut base).unwrap());
    assert!(!amorcage::amorcer(&mut base).unwrap(), "il ne rejoue pas");

    let comptes = base
        .lire_une("SELECT COUNT(*) FROM utilisateur_auth", &[], |r| r.get::<i64>(0))
        .unwrap()
        .unwrap();
    assert_eq!(comptes, 2, "aucun compte en double");
}
