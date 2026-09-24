//! C-3 — des plafonds, pas seulement des portes, sur les deux moteurs.
//!
//! Le caissier peut remettre 15 %, pas 40 ; il peut rendre 50 000 F,
//! pas plus. Chaque scenario verifie qu'un refus n'a RIEN ecrit : ni
//! mouvement de caisse, ni stock, ni paiement.

mod commun;

use commun::*;
use gescom_noyau::argent::{self, ParamsLigneInput};
use gescom_noyau::base::Base;
use gescom_noyau::coeur::plafonds::Plafonds;
use gescom_noyau::{auth, creances, parametres, plafonds, retours};

fn caissier(base: &mut Base) -> String {
    auth::creer_utilisateur_sur_base(
        base,
        "Fanta".into(),
        "fanta".into(),
        None,
        "fanta-secret".into(),
        "caissier".into(),
        "test".into(),
    )
    .expect("caissier")
}

fn admin(base: &mut Base) -> String {
    base.lire_une("SELECT utilisateur_id FROM utilisateur_auth WHERE pseudo = 'admin'", &[], |r| r.get::<String>(0))
        .unwrap()
        .unwrap()
}

fn plafonner_caissier(base: &mut Base, remboursement: i64) {
    plafonds::definir_role_sur(
        base,
        "caissier",
        Plafonds { remise_max_pct: Some(15.0), remboursement_max: Some(remboursement), credit_max: Some(100_000) },
    )
    .expect("plafonds du caissier");
}

fn sorties(base: &mut Base) -> i64 {
    compter(
        base,
        "SELECT CAST(COALESCE(SUM(montant), 0) AS BIGINT) FROM mouvement_caisse WHERE sens = 'sortie'",
        &[],
    )
}

#[test]
fn le_role_porte_ses_plafonds_la_personne_les_ajuste_le_patron_n_en_a_pas() {
    let mut base = base_avec_demo();
    let fanta = caissier(&mut base);
    let patron = admin(&mut base);
    assert!(plafonds::de_sur(&mut base, &fanta).est_vide(), "par défaut : aucun plafond");

    plafonner_caissier(&mut base, 50_000);
    let p = plafonds::de_sur(&mut base, &fanta);
    assert_eq!(p.remise_max_pct, Some(15.0));
    assert_eq!(p.remboursement_max, Some(50_000));

    // Le sur-mesure l'emporte, plafond par plafond.
    plafonds::definir_utilisateur_sur(&mut base, &fanta, Plafonds { remise_max_pct: Some(25.0), ..Default::default() })
        .unwrap();
    let p = plafonds::de_sur(&mut base, &fanta);
    assert_eq!(p.remise_max_pct, Some(25.0));
    assert_eq!(p.remboursement_max, Some(50_000), "le reste suit le rôle");
    // Tout vide : la personne revient à son rôle.
    plafonds::definir_utilisateur_sur(&mut base, &fanta, Plafonds::default()).unwrap();
    assert_eq!(plafonds::de_sur(&mut base, &fanta).remise_max_pct, Some(15.0));
    assert_eq!(compter(&mut base, "SELECT CAST(COUNT(*) AS BIGINT) FROM utilisateur_plafond", &[]), 0);

    // Le patron : aucun plafond, et on ne lui en pose pas.
    assert!(plafonds::de_sur(&mut base, &patron).est_vide());
    let e = plafonds::definir_role_sur(&mut base, "patron", Plafonds { credit_max: Some(1), ..Default::default() })
        .unwrap_err();
    assert!(e.contains("donne tout"), "{e}");
    // Un plafond se saisit, il se juge.
    assert!(plafonds::definir_role_sur(&mut base, "caissier", Plafonds { remise_max_pct: Some(150.0), ..Default::default() })
        .is_err());

    // Le journal le dit.
    let n = compter(
        &mut base,
        "SELECT CAST(COUNT(*) AS BIGINT) FROM journal WHERE type_evenement = 'plafonds_modifies'",
        &[],
    );
    assert_eq!(n, 3);
    let v = plafonds::lire_sur(&mut base).unwrap();
    let c = v["roles"].as_array().unwrap().iter().find(|r| r["nom"] == "caissier").unwrap();
    assert_eq!(c["plafonds"]["remboursement_max"], 50_000);
}

fn vendre_paye(base: &mut Base) -> (String, String, i64) {
    let depot = depot_defaut(base);
    let sucre = article_unite(base, "Sucre");
    let client = client_generique(base);
    let v = argent::creer_vente_sur_base(
        base,
        client,
        depot.clone(),
        "comptant".into(),
        vec![ParamsLigneInput {
            article_id: sucre.0.clone(),
            unite_vente_id: sucre.1.clone(),
            depot_source_id: depot,
            source_approvisionnement: "stock".into(),
            quantite: 3.0,
            facteur: sucre.2,
            prix_reference: sucre.3,
            prix_pratique: sucre.3,
            taux_tva: None,
            a_decouvert: None,
        }],
        None,
        Some(sucre.3 * 3),
        None,
        None,
    )
    .expect("vente");
    let vente_id = v["vente_id"].as_str().unwrap().to_string();
    let ligne_id: String = base
        .lire_une("SELECT id FROM ligne_vente WHERE vente_id = ?1", &parametres![vente_id.clone()], |r| r.get::<String>(0))
        .unwrap()
        .unwrap();
    (vente_id, ligne_id, sucre.3)
}

#[test]
fn un_retour_rembourse_au_dela_du_plafond_est_refuse_et_n_ecrit_rien() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let fanta = caissier(&mut base);
    let (vente_id, ligne_id, prix) = vendre_paye(&mut base);
    // Plafond sous le prix d'un kilo.
    plafonner_caissier(&mut base, prix - 1);
    let sucre = article_unite(&mut base, "Sucre");
    let depot = depot_defaut(&mut base);
    let (stock_avant, sorties_avant) = (stock(&mut base, &sucre.0, &depot), sorties(&mut base));

    {
        let _g = gescom_noyau::auteur::poser(&fanta);
        let e = retours::enregistrer_retour_sur_base(
            &mut base, vente_id.clone(), ligne_id.clone(), 1.0, "remboursement".into(),
            Some("especes".into()), None, None, None, None, None,
        )
        .unwrap_err();
        assert!(e.starts_with("Remboursement de") && e.ends_with("Demander au patron."), "{e}");
    }
    assert_eq!(stock(&mut base, &sucre.0, &depot), stock_avant, "le stock n'a pas bougé");
    assert_eq!(sorties(&mut base), sorties_avant, "le tiroir non plus");
    assert_eq!(compter(&mut base, "SELECT CAST(COUNT(*) AS BIGINT) FROM retour", &[]), 0);

    // Le patron passe et fait le geste avec son compte.
    let patron = admin(&mut base);
    {
        let _g = gescom_noyau::auteur::poser(&patron);
        retours::enregistrer_retour_sur_base(
            &mut base, vente_id, ligne_id, 1.0, "remboursement".into(),
            Some("especes".into()), None, None, None, None, None,
        )
        .expect("le patron n'a pas de plafond");
    }
    assert_eq!(sorties(&mut base), sorties_avant + prix);
}

#[test]
fn rendre_un_reglement_au_dela_du_plafond_est_refuse_mais_corriger_une_saisie_passe() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let fanta = caissier(&mut base);
    plafonner_caissier(&mut base, 100);
    let (vente_id, _, _) = {
        let depot = depot_defaut(&mut base);
        let sucre = article_unite(&mut base, "Sucre");
        let client = client_reel(&mut base);
        let v = argent::creer_vente_sur_base(
            &mut base,
            client,
            depot.clone(),
            "credit".into(),
            vec![ParamsLigneInput {
                article_id: sucre.0.clone(),
                unite_vente_id: sucre.1.clone(),
                depot_source_id: depot,
                source_approvisionnement: "stock".into(),
                quantite: 2.0,
                facteur: sucre.2,
                prix_reference: sucre.3,
                prix_pratique: sucre.3,
                taux_tva: None,
                a_decouvert: None,
            }],
            None,
            None,
            None,
            None,
        )
        .unwrap();
        (v["vente_id"].as_str().unwrap().to_string(), (), ())
    };
    creances::regler_creance_sur_base(&mut base, vente_id.clone(), 300, "especes".into(), None).unwrap();
    let client = client_reel(&mut base);
    let regs = creances::lire_reglements_client_sur_base(&mut base, client).unwrap();
    let p = regs.iter().find(|r| r["montant"] == 300).unwrap()["id"].as_str().unwrap().to_string();
    let paiements_avant = compter(&mut base, "SELECT CAST(COUNT(*) AS BIGINT) FROM paiement", &[]);

    let _g = gescom_noyau::auteur::poser(&fanta);
    let e = creances::annuler_reglement_sur_base(&mut base, p.clone(), "rendu".into(), true, None).unwrap_err();
    assert!(e.starts_with("Remboursement de 300 F — votre plafond est 100 F."), "{e}");
    assert_eq!(compter(&mut base, "SELECT CAST(COUNT(*) AS BIGINT) FROM paiement", &[]), paiements_avant);
    // Corriger une erreur de saisie n'est pas rendre de l'argent au client.
    creances::annuler_reglement_sur_base(&mut base, p, "erreur de saisie".into(), false, None)
        .expect("une correction ne se plafonne pas");
}
