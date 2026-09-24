//! C-1 — les permissions de LECTURE, sur les deux moteurs.
//!
//! La preuve du plan : le caissier recoit `prix_achat: null`. Chaque
//! scenario lit la VRAIE reponse d'une commande, puis lui applique la
//! regle que le serveur applique (`coeur::lecture`) avec les
//! permissions du role livre — celles que la base porte, pas une liste
//! recopiee dans le test.

mod commun;

use std::collections::HashSet;

use commun::*;
use gescom_noyau::base::Base;
use gescom_noyau::coeur::lecture;
use gescom_noyau::{amorcage, caisse, catalogue, catalogue_csv, parametres, pieces, portes};
use serde_json::Value;

fn droits(base: &mut Base, role: &str) -> HashSet<String> {
    portes::permissions_de_sur(base, "personne", role)
}

fn vu_par(base: &mut Base, role: &str, commande: &str, mut v: Value, utilisateur: &str) -> Value {
    let d = droits(base, role);
    lecture::filtrer(commande, |p| d.contains(p), utilisateur, &mut v);
    v
}

#[test]
fn le_caissier_recoit_prix_achat_null_le_comptable_le_lit() {
    let mut base = base_avec_demo();
    let etat = catalogue_csv::lire_etat_stock_sur_base(&mut base, None, None).unwrap();
    assert!(etat["lignes"][0]["prix_achat"].is_i64(), "la démo a des prix d'achat : {etat}");

    let caissier = vu_par(&mut base, "caissier", "lire_etat_stock", etat.clone(), "u");
    let l = &caissier["lignes"][0];
    assert_eq!(l["prix_achat"], Value::Null);
    assert_eq!(l["valeur"], Value::Null);
    assert_eq!(caissier["valeur_totale"], Value::Null, "pas un zéro qui ressemblerait à une vraie valeur");
    assert_eq!(l["quantite"], etat["lignes"][0]["quantite"], "le stock, lui, se lit");

    for role in ["magasinier", "employe"] {
        let v = vu_par(&mut base, role, "lire_etat_stock", etat.clone(), "u");
        assert_eq!(v["lignes"][0]["prix_achat"], Value::Null, "{role}");
    }
    for role in ["comptable", "patron"] {
        let v = vu_par(&mut base, role, "lire_etat_stock", etat.clone(), "u");
        assert_eq!(v, etat, "{role} lit tout");
    }

    // Le catalogue du comptoir : le coût part, le prix de vente reste.
    let cat = catalogue::lire_articles_avec_unites_sur(&mut base, true, None).unwrap();
    let cat = vu_par(&mut base, "caissier", "lire_articles_avec_unites", Value::Array(cat), "u");
    assert!(cat.as_array().unwrap().iter().all(|a| a["dernier_prix_achat"].is_null()));
    assert!(cat[0]["unites"][0]["prix_reference"].is_i64());
}

#[test]
fn le_chiffre_de_la_boutique_se_refuse_a_qui_n_a_pas_rapports_lire() {
    let mut base = base_avec_demo();
    for (role, attendu) in [
        ("caissier", Some(lecture::RAPPORTS_LIRE)),
        ("magasinier", Some(lecture::RAPPORTS_LIRE)),
        ("employe", Some(lecture::RAPPORTS_LIRE)),
        ("comptable", None),
        ("patron", None),
    ] {
        let d = droits(&mut base, role);
        for commande in ["lire_resume_dashboard", "lire_journal_du_jour", "lire_rapport_ca_mensuel"] {
            assert_eq!(lecture::refus(commande, |p| d.contains(p)), attendu, "{role} · {commande}");
        }
    }
    // Les écarts de caisse : un rapport ET les caisses des autres — le
    // comptable n'a pas `caisse:lire_autres`.
    let d = droits(&mut base, "comptable");
    assert_eq!(lecture::refus("lire_rapport_ecarts", |p| d.contains(p)), Some(lecture::CAISSE_LIRE_AUTRES));
}

#[test]
fn ce_que_doit_un_client_ne_se_lit_qu_avec_tiers_lire_solde() {
    let mut base = base_avec_demo();
    let client = client_reel(&mut base);
    let fiche = pieces::lire_fiche_client_sur_base(&mut base, client).unwrap();
    assert!(fiche["stats"].get("encours").is_some(), "{fiche}");

    let caissier = vu_par(&mut base, "caissier", "lire_fiche_client", fiche.clone(), "u");
    assert_eq!(caissier["stats"]["encours"], Value::Null);
    assert_eq!(caissier["client"], fiche["client"], "la fiche elle-même se lit");
    let comptable = vu_par(&mut base, "comptable", "lire_fiche_client", fiche.clone(), "u");
    assert_eq!(comptable, fiche);

    let d = droits(&mut base, "caissier");
    assert_eq!(lecture::refus("lire_etat_creances_client", |p| d.contains(p)), Some(lecture::TIERS_LIRE_SOLDE));
    // Et le tri par dette de la liste des clients est neutralisé.
    let mut params = serde_json::json!({ "page": 0, "limite": 20, "avecCreancesSeulement": true });
    lecture::neutraliser("lire_clients_pagines", |p| d.contains(p), &mut params);
    assert_eq!(params["avecCreancesSeulement"], false);
    assert_eq!(params["tri"], "nom");
}

fn utilisateur(base: &mut Base, pseudo: &str) -> String {
    base.lire_une(
        "SELECT utilisateur_id FROM utilisateur_auth WHERE pseudo = ?1",
        &parametres![pseudo],
        |r| r.get::<String>(0),
    )
    .unwrap()
    .unwrap()
}

#[test]
fn sans_caisse_lire_autres_on_ne_lit_que_ses_sessions() {
    let mut base = base_avec_demo();
    let admin = utilisateur(&mut base, "admin");
    let employe = utilisateur(&mut base, "employe");
    let session_admin = {
        let _g = gescom_noyau::auteur::poser(&admin);
        let s = caisse::ouvrir_session_caisse_sur(&mut base, 0, "patron".into()).unwrap();
        caisse::fermer_session_caisse_sur(&mut base, s.clone(), 0).unwrap();
        s
    };
    let session_employe = {
        let _g = gescom_noyau::auteur::poser(&employe);
        caisse::ouvrir_session_caisse_sur(&mut base, 0, "employe".into()).unwrap()
    };
    let toutes = Value::Array(caisse::lire_sessions_caisse_sur_base(&mut base, None).unwrap());
    assert_eq!(toutes.as_array().unwrap().len(), 2);

    let vues = vu_par(&mut base, "employe", "lire_sessions_caisse", toutes.clone(), &employe);
    let ids: Vec<&str> = vues.as_array().unwrap().iter().map(|s| s["id"].as_str().unwrap()).collect();
    assert_eq!(ids, vec![session_employe.as_str()], "la sienne seulement");
    let patron = vu_par(&mut base, "patron", "lire_sessions_caisse", toutes.clone(), &admin);
    assert_eq!(patron.as_array().unwrap().len(), 2);

    assert_eq!(caisse::session_ouverte_par_sur(&mut base, &session_admin).unwrap().as_deref(), Some(admin.as_str()));
    let d = droits(&mut base, "employe");
    assert!(lecture::session_caisse_a_verifier("lire_mouvements_session", |p| d.contains(p)));
}

#[test]
fn une_base_installee_donne_ses_lectures_au_comptable_une_seule_fois() {
    let mut base = base_avec_demo();
    if !base.peut_migrer() {
        return; // compte limité (D-6) : les migrations sont au propriétaire
    }
    // Une base d'avant C-1 : le comptable d'origine, sans marque.
    base.executer(
        "UPDATE role SET permissions = ?1 WHERE nom = 'comptable'",
        &parametres![r#"["creances:gerer","cheques:gerer"]"#],
    )
    .unwrap();
    base.executer("DELETE FROM config_app WHERE cle = 'migration_v3_lectures'", &[]).unwrap();

    amorcage::amorcer(&mut base).unwrap();
    let d = droits(&mut base, "comptable");
    for p in ["journal:lire", "achats:lire_prix", "rapports:lire", "tiers:lire_solde"] {
        assert!(d.contains(p), "{p} donné au comptable installé");
    }
    assert!(!d.contains("caisse:lire_autres"));
    assert!(d.contains("creances:gerer"), "ce qu'il avait reste");

    // Le patron lui retire les rapports : le démarrage suivant ne les lui rend pas.
    base.executer(
        "UPDATE role SET permissions = ?1 WHERE nom = 'comptable'",
        &parametres![r#"["creances:gerer","tiers:lire_solde"]"#],
    )
    .unwrap();
    amorcage::amorcer(&mut base).unwrap();
    assert!(!droits(&mut base, "comptable").contains("rapports:lire"));
}
