//! `caisse::{ouvrir,fermer,lire_resume}_session_caisse_sur`.
//!
//! Sans elles, une vente comptant sur PostgreSQL échouait toujours sur
//! `CAISSE_FERMEE` : rien ne pouvait ouvrir de session. Écrites pour
//! débloquer le premier essai manuel sur une vraie base PostgreSQL —
//! testées ici sur SQLite, seul moteur disponible dans cet
//! environnement.

use gescom_noyau::base::Base;
use gescom_noyau::{amorcage, argent, caisse};

fn base_avec_demo() -> Base {
    let mut base = Base::ouvrir(":memory:").expect("base en mémoire");
    amorcage::amorcer(&mut base).expect("amorçage");
    amorcage::donnees_demo(&mut base).expect("démo");
    base
}

#[test]
fn une_base_neuve_n_a_aucune_caisse() {
    let mut base = base_avec_demo();
    let resume = caisse::lire_resume_caisse_sur(&mut base).unwrap();
    assert_eq!(resume["statut"], "aucune");
    assert_eq!(resume["session_id"], serde_json::Value::Null);
}

#[test]
fn ouvrir_la_caisse_avec_un_fond_pose_un_mouvement_d_ouverture() {
    let mut base = base_avec_demo();
    let session_id = caisse::ouvrir_session_caisse_sur(&mut base, 10_000, "patron".into())
        .expect("ouverture");

    let resume = caisse::lire_resume_caisse_sur(&mut base).unwrap();
    assert_eq!(resume["statut"], "ouverte");
    assert_eq!(resume["session_id"], session_id);
    assert_eq!(resume["fond_ouverture"], 10_000);
    // Le mouvement d'ouverture est exclu du total des entrées (sinon le
    // solde théorique le compterait deux fois).
    assert_eq!(resume["total_entrees"], 0);
    assert_eq!(resume["solde_theorique"], 10_000);
}

#[test]
fn ouvrir_une_deuxieme_caisse_est_refuse() {
    let mut base = base_avec_demo();
    caisse::ouvrir_session_caisse_sur(&mut base, 0, "patron".into()).unwrap();
    let err = caisse::ouvrir_session_caisse_sur(&mut base, 0, "patron".into())
        .expect_err("une session déjà ouverte doit bloquer la suivante");
    assert!(err.contains("déjà ouverte"));
}

#[test]
fn fermer_la_caisse_calcule_l_ecart() {
    let mut base = base_avec_demo();
    let session_id =
        caisse::ouvrir_session_caisse_sur(&mut base, 5_000, "patron".into()).unwrap();

    // Le compte du soir tombe juste : écart nul.
    caisse::fermer_session_caisse_sur(&mut base, session_id.clone(), 5_000).unwrap();
    let sessions = base
        .lire_plusieurs(
            "SELECT statut, ecart, especes_comptees FROM session_caisse WHERE id = ?1",
            &gescom_noyau::parametres![session_id],
            |r| Ok((r.get::<String>(0)?, r.get::<i64>(1)?, r.get::<i64>(2)?)),
        )
        .unwrap();
    let (statut, ecart, comptees) = sessions.into_iter().next().unwrap();
    assert_eq!(statut, "fermee");
    assert_eq!(ecart, 0);
    assert_eq!(comptees, 5_000);
}

#[test]
fn un_manque_a_la_cloture_se_lit_dans_l_ecart() {
    let mut base = base_avec_demo();
    let session_id =
        caisse::ouvrir_session_caisse_sur(&mut base, 5_000, "patron".into()).unwrap();
    // Le tiroir compte 4 500 F au lieu des 5 000 théoriques.
    caisse::fermer_session_caisse_sur(&mut base, session_id.clone(), 4_500).unwrap();
    let ecart: i64 = base
        .lire_une(
            "SELECT ecart FROM session_caisse WHERE id = ?1",
            &gescom_noyau::parametres![session_id],
            |r| r.get::<i64>(0),
        )
        .unwrap()
        .unwrap();
    assert_eq!(ecart, -500, "un manque doit apparaître négatif");
}

#[test]
fn deux_dossiers_ont_chacun_leur_caisse() {
    let mut base = base_avec_demo();
    caisse::ouvrir_session_caisse_sur(&mut base, 1_000, "patron".into()).unwrap();

    base.choisir_dossier("dossier-b").unwrap();
    let resume = caisse::lire_resume_caisse_sur(&mut base).unwrap();
    assert_eq!(
        resume["statut"], "aucune",
        "la caisse ouverte dans l'autre dossier ne doit pas se voir ici"
    );

    // Rien n'empêche d'ouvrir la sienne, indépendamment.
    caisse::ouvrir_session_caisse_sur(&mut base, 0, "patron".into())
        .expect("dossier-b ouvre sa propre caisse sans être bloqué par l'autre");
}

/// Le scénario qui débloquait le premier essai manuel : sans caisse
/// ouverte, une vente comptant refusait ; avec elle, elle passe.
#[test]
fn une_vente_comptant_marche_desormais_avec_la_caisse_portee() {
    let mut base = base_avec_demo();
    let depot: String = base
        .lire_une("SELECT id FROM depot WHERE est_defaut = 1", &[], |r| r.get::<String>(0))
        .unwrap()
        .unwrap();
    let (article_id, unite_id, facteur, prix): (String, String, f64, i64) = base
        .lire_une(
            "SELECT a.id, u.id, u.facteur, u.prix_reference
             FROM article a JOIN unite_vente u ON u.article_id = a.id
             WHERE a.nom = 'Sucre' AND u.facteur = 1.0",
            &[],
            |r| {
                Ok((r.get::<String>(0)?, r.get::<String>(1)?, r.get::<f64>(2)?, r.get::<i64>(3)?))
            },
        )
        .unwrap()
        .unwrap();
    let client: String = base
        .lire_une("SELECT id FROM client WHERE est_generique = 1", &[], |r| r.get::<String>(0))
        .unwrap()
        .unwrap();

    let sans_caisse = argent::creer_vente_sur_base(
        &mut base,
        client.clone(),
        depot.clone(),
        "comptant".into(),
        vec![argent::ParamsLigneInput {
            article_id: article_id.clone(),
            unite_vente_id: unite_id.clone(),
            depot_source_id: depot.clone(),
            source_approvisionnement: "stock".into(),
            quantite: 1.0,
            facteur,
            prix_reference: prix,
            prix_pratique: prix,
            taux_tva: None,
            a_decouvert: None,
        }],
        None,
        Some(prix),
        None,
        None,
    );
    assert!(sans_caisse.unwrap_err().contains("CAISSE_FERMEE"));

    caisse::ouvrir_session_caisse_sur(&mut base, 0, "patron".into()).unwrap();

    let avec_caisse = argent::creer_vente_sur_base(
        &mut base,
        client,
        depot.clone(),
        "comptant".into(),
        vec![argent::ParamsLigneInput {
            article_id,
            unite_vente_id: unite_id,
            depot_source_id: depot,
            source_approvisionnement: "stock".into(),
            quantite: 1.0,
            facteur,
            prix_reference: prix,
            prix_pratique: prix,
            taux_tva: None,
            a_decouvert: None,
        }],
        None,
        Some(prix),
        None,
        None,
    );
    assert!(avec_caisse.is_ok(), "{:?}", avec_caisse.err());
    assert_eq!(avec_caisse.unwrap()["statut"], "payee");
}
