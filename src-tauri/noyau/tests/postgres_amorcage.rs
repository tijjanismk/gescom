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

#[test]
fn les_donnees_de_demonstration_passent_sur_postgresql() {
    // Ce test vaut surtout pour ce qu'il traverse : article,
    // unite_vente, categorie, client, mouvement_stock, stock_depot.
    // C'est la que se trouvent les ecarts de dialecte qu'un amorcage
    // minimal ne rencontre jamais.
    let Some(mut base) = pg() else { return };
    base.executer_lot("DROP SCHEMA public CASCADE; CREATE SCHEMA public;").unwrap();
    amorcage::amorcer(&mut base).unwrap();

    let (articles, clients) = amorcage::donnees_demo(&mut base).expect("démo");
    assert_eq!(articles, 8);
    assert_eq!(clients, 4);

    let unites = base
        .lire_une("SELECT COUNT(*) FROM unite_vente", &[], |r| r.get::<i64>(0))
        .unwrap()
        .unwrap();
    assert!(unites >= 12, "les unités de vente sont posées : {unites}");

    // Le stock doit valoir la somme de ses mouvements — l'invariant
    // pose a l'etape 2 du projet.
    let compteur = base
        .lire_une(
            "SELECT COALESCE(SUM(quantite), 0) FROM stock_depot",
            &[],
            |r| r.get::<f64>(0),
        )
        .unwrap()
        .unwrap();
    let somme = base
        .lire_une(
            "SELECT COALESCE(SUM(quantite_delta), 0) FROM mouvement_stock",
            &[],
            |r| r.get::<f64>(0),
        )
        .unwrap()
        .unwrap();
    assert_eq!(compteur, somme, "le stock doit valoir la somme de ses mouvements");
    assert!(compteur > 0.0);
}

// =====================================================================
//  SE CONNECTER — le premier module porte
// =====================================================================

#[test]
fn on_se_connecte_a_postgresql_avec_ses_permissions() {
    let Some(mut base) = pg() else { return };
    base.executer_lot("DROP SCHEMA public CASCADE; CREATE SCHEMA public;").unwrap();
    amorcage::amorcer(&mut base).unwrap();

    let u = gescom_noyau::auth::connexion_sur(
        &mut base,
        "admin".to_string(),
        "admin123".to_string(),
    )
    .expect("le patron doit pouvoir se connecter");

    assert_eq!(u["role"], "patron");
    assert_eq!(u["doit_changer_mdp"], true);

    // Les permissions voyagent avec l'identite : sans elles, l'ecran
    // n'affiche ni menu ni onglet.
    let perms = u["permissions"].as_array().expect("des permissions");
    assert_eq!(
        perms.len(),
        gescom_noyau::portes::CATALOGUE.len(),
        "le patron a l'accès total"
    );

    // Par l'email aussi : le champ accepte les deux.
    assert!(gescom_noyau::auth::connexion_sur(
        &mut base, "admin@gescom.ml".to_string(), "admin123".to_string()
    ).is_ok());
}

#[test]
fn un_mot_de_passe_faux_est_refuse_sur_postgresql() {
    let Some(mut base) = pg() else { return };
    base.executer_lot("DROP SCHEMA public CASCADE; CREATE SCHEMA public;").unwrap();
    amorcage::amorcer(&mut base).unwrap();

    let r = gescom_noyau::auth::connexion_sur(
        &mut base, "admin".to_string(), "admin".to_string(),
    );
    assert!(r.is_err());
    // Le meme message que pour un identifiant inconnu : les distinguer
    // confirmerait qu'un compte existe a qui essaie des noms au hasard.
    let inconnu = gescom_noyau::auth::connexion_sur(
        &mut base, "personne".to_string(), "admin123".to_string(),
    );
    assert_eq!(format!("{r:?}"), format!("{inconnu:?}"));
}

#[test]
fn une_session_s_ouvre_et_se_revoque_sur_postgresql() {
    use gescom_noyau::sessions::{self, Etat};
    let Some(mut base) = pg() else { return };
    base.executer_lot("DROP SCHEMA public CASCADE; CREATE SCHEMA public;").unwrap();
    amorcage::amorcer(&mut base).unwrap();

    let utilisateur: String = base
        .lire_une("SELECT utilisateur_id FROM utilisateur_auth WHERE pseudo = 'admin'",
                  &[], |r| r.get::<String>(0))
        .unwrap()
        .unwrap();

    // Un poste, comme en inscrit une connexion reseau.
    base.executer(
        "INSERT INTO poste (id, nom, empreinte, genre, actif, cree_le, modifie_le)
         VALUES ('p1', 'Caisse 1', 'emp-1', 'caisse', 1, '2026-01-01', '2026-01-01')",
        &[],
    )
    .unwrap();

    let (session, jeton, _expire) =
        sessions::ouvrir_sur(&mut base, "p1", &utilisateur).expect("session ouverte");
    assert_eq!(jeton.len(), 64, "256 bits d'aléa");

    match sessions::etat_sur(&mut base, &session) {
        Etat::Valide { role, .. } => assert_eq!(role, "patron"),
        _ => panic!("la session devrait être valide"),
    }

    assert_eq!(sessions::lister_actives_sur(&mut base).unwrap().len(), 1);

    sessions::revoquer_sur(&mut base, &session, "test").unwrap();
    assert!(matches!(
        sessions::etat_sur(&mut base, &session),
        Etat::Revoquee
    ));
    assert!(sessions::lister_actives_sur(&mut base).unwrap().is_empty());
}

#[test]
fn un_poste_desactive_coupe_la_session_sur_postgresql() {
    use gescom_noyau::sessions::{self, Etat};
    let Some(mut base) = pg() else { return };
    base.executer_lot("DROP SCHEMA public CASCADE; CREATE SCHEMA public;").unwrap();
    amorcage::amorcer(&mut base).unwrap();

    let utilisateur: String = base
        .lire_une("SELECT utilisateur_id FROM utilisateur_auth WHERE pseudo = 'admin'",
                  &[], |r| r.get::<String>(0))
        .unwrap()
        .unwrap();
    base.executer(
        "INSERT INTO poste (id, nom, empreinte, genre, actif, cree_le, modifie_le)
         VALUES ('p2', 'Caisse 2', 'emp-2', 'caisse', 1, '2026-01-01', '2026-01-01')",
        &[],
    )
    .unwrap();
    let (session, _, _) = sessions::ouvrir_sur(&mut base, "p2", &utilisateur).unwrap();

    base.executer("UPDATE poste SET actif = 0 WHERE id = 'p2'", &[]).unwrap();
    assert!(matches!(
        sessions::etat_sur(&mut base, &session),
        Etat::PosteFerme
    ));
}
