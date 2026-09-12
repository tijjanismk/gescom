//! Le dernier lot : `caisse` (dépenses, historique, écarts),
//! `pieces_pos`, `argent::enregistrer_paiement`, utilisateurs, postes,
//! mode de caisse, images, sauvegarde — sur `Base`. Testé sur SQLite
//! par défaut, sur PostgreSQL avec `GESCOM_PG`.

mod commun;

use commun::*;
use gescom_noyau::argent::{self, ParamsLigneInput};
use gescom_noyau::base::Base;
use gescom_noyau::parametres;
use gescom_noyau::{auth, caisse, caisses, images, pieces_pos, postes, sauvegarde};

fn vendre_credit(base: &mut Base, quantite: f64) -> (String, i64) {
    let depot = depot_defaut(base);
    let sucre = article_unite(base, "Sucre");
    let client = client_reel(base);
    let v = argent::creer_vente_sur_base(
        base, client, depot.clone(), "credit".into(),
        vec![ParamsLigneInput {
            article_id: sucre.0.clone(), unite_vente_id: sucre.1.clone(), depot_source_id: depot,
            source_approvisionnement: "stock".into(), quantite, facteur: sucre.2,
            prix_reference: sucre.3, prix_pratique: sucre.3, taux_tva: None, a_decouvert: None,
        }],
        None, None, None, None,
    )
    .expect("vente");
    (v["vente_id"].as_str().unwrap().to_string(), (sucre.3 as f64 * quantite).round() as i64)
}

#[test]
fn une_depense_se_saisit_se_corrige_et_se_retrouve_dans_la_journee() {
    let mut base = base_avec_demo();
    assert!(caisse::enregistrer_depense_sur_base(&mut base, 500, "Taxi".into(), None, None, None).unwrap_err().contains("ouvrir la caisse"));
    let sid = ouvrir_caisse(&mut base);
    assert!(caisse::enregistrer_depense_sur_base(&mut base, 0, "Taxi".into(), None, None, None).is_err());
    let id = caisse::enregistrer_depense_sur_base(&mut base, 500, " Taxi ".into(), Some("transport".into()), None, None).unwrap();

    let j = caisse::lire_depenses_du_jour_sur_base(&mut base).unwrap();
    assert_eq!(j["total"], 500);
    assert_eq!(j["depenses"][0]["libelle"], "Taxi");
    assert_eq!(j["par_categorie"][0]["categorie"], "transport");
    let m = caisse::lire_mouvements_caisse_du_jour_sur_base(&mut base).unwrap();
    assert!(m.iter().any(|x| x["id"] == id && x["sens"] == "sortie"));

    caisse::modifier_depense_sur_base(&mut base, id.clone(), Some(700), None, Some("carburant".into())).unwrap();
    let j = caisse::lire_depenses_du_jour_sur_base(&mut base).unwrap();
    assert_eq!(j["total"], 700);
    assert_eq!(j["par_categorie"][0]["categorie"], "carburant");

    let mv = caisse::lire_mouvements_session_sur_base(&mut base, sid.clone()).unwrap();
    assert_eq!(mv.len(), 1, "la dépense seule : le fond d'ouverture à 0 n'écrit rien");

    gescom_noyau::caisse::fermer_session_caisse_sur(&mut base, sid.clone(), 0).unwrap();
    assert!(caisse::modifier_depense_sur_base(&mut base, id, Some(1), None, None).unwrap_err().contains("clôturée"));

    let sessions = caisse::lire_sessions_caisse_sur_base(&mut base, None).unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0]["statut"], "fermee");
    assert_eq!(sessions[0]["sorties_especes"], 700);
    assert_eq!(sessions[0]["ecart"], 700, "0 compté, -700 théorique : l'écart est ce qui manque… ou l'inverse");

    let e = caisse::lire_rapport_ecarts_sur_base(&mut base, Some(7)).unwrap();
    assert_eq!(e["nb_clotures"], 1);
    assert!(e["diagnostic"].is_string());
    assert_eq!(caisse::lire_rapport_ecarts_sur_base(&mut base, Some(0)).unwrap()["nb_clotures"], 1);
}

#[test]
fn un_paiement_sur_une_vente_a_credit_la_solde_et_alimente_la_caisse() {
    let mut base = base_avec_demo();
    let (vente_id, prix) = vendre_credit(&mut base, 2.0);
    assert!(argent::enregistrer_paiement_sur_base(&mut base, vente_id.clone(), 100, "especes".into(), None).unwrap_err().contains("CAISSE_FERMEE"));
    ouvrir_caisse(&mut base);
    argent::enregistrer_paiement_sur_base(&mut base, vente_id.clone(), 300, "especes".into(), None).unwrap();
    let statut = |b: &mut Base| b.lire_une("SELECT statut FROM vente WHERE id = ?1", &parametres![vente_id.clone()], |r| r.get::<String>(0)).unwrap().unwrap();
    assert_eq!(statut(&mut base), "partiellement_payee");
    argent::enregistrer_paiement_sur_base(&mut base, vente_id.clone(), prix - 300, "orange_money".into(), None).unwrap();
    assert_eq!(statut(&mut base), "payee");
    let entrees = compter(&mut base, "SELECT CAST(COALESCE(SUM(montant),0) AS BIGINT) FROM mouvement_caisse WHERE sens = 'entree' AND motif = 'vente'", &[]);
    assert_eq!(entrees, prix);
}

#[test]
fn la_facture_pos_se_retouche_puis_se_fige() {
    let mut base = base_avec_demo();
    let (vente_id, _) = vendre_credit(&mut base, 1.0);
    let client = client_reel(&mut base);
    let f = argent::creer_facture_depuis_vente_sur_base(&mut base, vente_id, client, "credit".into(), None).unwrap();
    let piece_id = f["piece_id"].as_str().or(f["id"].as_str()).unwrap().to_string();

    pieces_pos::modifier_facture_pos_sur_base(&mut base, piece_id.clone(), Some("livrer lundi".into()), Some("2030-01-01".into())).unwrap();
    let (note, ech): (Option<String>, Option<String>) = base
        .lire_une("SELECT note, date_echeance FROM piece_commerciale WHERE id = ?1", &parametres![piece_id.clone()], |r| Ok((r.get::<Option<String>>(0)?, r.get::<Option<String>>(1)?)))
        .unwrap()
        .unwrap();
    assert_eq!(note.as_deref(), Some("livrer lundi"));
    assert_eq!(ech.as_deref(), Some("2030-01-01"));

    pieces_pos::valider_facture_credit_sur_base(&mut base, piece_id.clone()).unwrap();
    assert_eq!(statut_piece(&mut base, &piece_id), "validee");
    assert!(pieces_pos::valider_facture_credit_sur_base(&mut base, piece_id.clone()).unwrap_err().contains("déjà"));
    assert!(pieces_pos::modifier_facture_pos_sur_base(&mut base, piece_id, None, None).unwrap_err().contains("non modifiable"));
}

#[test]
fn utilisateurs_postes_et_mode_de_caisse() {
    let mut base = base_avec_demo();
    assert!(auth::creer_utilisateur_sur_base(&mut base, "Awa".into(), "awa".into(), None, "123".into(), "employe".into(), "test".into()).unwrap_err().contains("6 caractères"));
    assert!(auth::creer_utilisateur_sur_base(&mut base, "Awa".into(), "awa".into(), None, "secret1".into(), "inconnu".into(), "test".into()).unwrap_err().contains("introuvable"));
    let id = auth::creer_utilisateur_sur_base(&mut base, "Awa".into(), "awa".into(), Some("a@b.c".into()), "secret1".into(), "employe".into(), "test".into()).unwrap();
    let tous = auth::lire_utilisateurs_sur(&mut base).unwrap();
    assert!(tous.iter().any(|u| u["id"] == id && u["pseudo"] == "awa"));
    let c = auth::connexion_sur(&mut base, "awa".into(), "secret1".into()).unwrap();
    assert_eq!(c["role"], "employe");

    // Postes : une caisse se coupe, le serveur non.
    let dossier = base.dossier().to_string();
    let now = gescom_noyau::utils::maintenant_iso();
    for (pid, genre) in [("p-serveur", "serveur"), ("p-caisse", "caisse")] {
        base.executer(
            "INSERT INTO poste (id, nom, empreinte, genre, actif, cree_le, modifie_le)
             VALUES (?1, ?1, ?1, ?2, 1, ?3, ?3)",
            &parametres![pid, genre, now.clone()],
        )
        .unwrap();
    }
    let _ = dossier;
    assert_eq!(postes::lister_sur_base(&mut base).unwrap().len(), 2);
    assert!(postes::desactiver_sur_base(&mut base, "p-serveur", "test").unwrap_err().contains("ne se désactive pas"));
    postes::desactiver_sur_base(&mut base, "p-caisse", "test").unwrap();
    let p = postes::lire_sur(&mut base, "p-caisse").unwrap();
    assert!(!p.actif);
    postes::reactiver_sur_base(&mut base, "p-caisse").unwrap();
    assert!(postes::lire_sur(&mut base, "p-caisse").unwrap().actif);

    // Mode de caisse : refusé tant qu'une caisse est ouverte.
    assert!(!caisses::par_utilisateur_sur(&mut base));
    let sid = ouvrir_caisse(&mut base);
    assert!(caisses::definir_par_utilisateur_sur(&mut base, true).unwrap_err().contains("Fermer toutes les caisses"));
    gescom_noyau::caisse::fermer_session_caisse_sur(&mut base, sid, 0).unwrap();
    caisses::definir_par_utilisateur_sur(&mut base, true).unwrap();
    assert!(caisses::par_utilisateur_sur(&mut base));
    caisses::definir_par_utilisateur_sur(&mut base, false).unwrap();
}

#[test]
fn images_et_sauvegarde_repondent_selon_le_moteur() {
    let mut base = base_avec_demo();
    assert!(images::lire_base64_sur_base(&mut base, "logo", None).unwrap().is_none(), "pas d'image : pas une erreur");
    assert!(images::lire_base64_sur_base(&mut base, "../etc".into(), None).is_err());

    let cfg = sauvegarde::lire_config_sauvegarde_sur_base(&mut base).unwrap();
    assert_eq!(cfg["sauvegarde_auto"], false);
    assert!(cfg["derniere_sauvegarde"].is_null());
    sauvegarde::sauvegarder_config_sauvegarde_sur_base(&mut base, Some("C:/sauvegardes".into()), true).unwrap();
    let cfg = sauvegarde::lire_config_sauvegarde_sur_base(&mut base).unwrap();
    assert_eq!(cfg["sauvegarde_auto"], true);
    assert_eq!(cfg["dossier_sauvegarde"], "C:/sauvegardes");

    // En mémoire sur SQLite, pas un fichier sur PostgreSQL : dans les
    // deux cas, un refus qui le dit, jamais une fausse sauvegarde.
    let r = sauvegarde::sauvegarder_base_sur_base(&mut base, "C:/sauvegardes".into()).unwrap_err();
    assert!(r.contains("mémoire") || r.contains("pg_dump"), "{r}");
    let auto = sauvegarde::sauvegarde_auto_si_necessaire_sur_base(&mut base).unwrap();
    assert_eq!(auto["effectuee"], false);
    assert!(auto["raison"] == "postgresql" || auto["raison"] == "echec", "{auto}");
}

#[test]
fn tout_ce_qui_est_porte_ici_passe_le_detecteur() {
    let mut base = base_avec_demo();
    let sid = ouvrir_caisse(&mut base);
    let (vente_id, _) = vendre_credit(&mut base, 1.0);
    base.auditer(true);

    caisse::lire_mouvements_caisse_du_jour_sur_base(&mut base).expect("mouvements");
    let d = caisse::enregistrer_depense_sur_base(&mut base, 100, "x".into(), None, None, None).expect("dépense");
    caisse::lire_depenses_du_jour_sur_base(&mut base).expect("dépenses");
    caisse::modifier_depense_sur_base(&mut base, d, Some(200), Some("y".into()), None).expect("modifier");
    caisse::lire_sessions_caisse_sur_base(&mut base, Some(5)).expect("sessions");
    caisse::lire_mouvements_session_sur_base(&mut base, sid).expect("mouvements session");
    caisse::lire_rapport_ecarts_sur_base(&mut base, None).expect("écarts");
    argent::enregistrer_paiement_sur_base(&mut base, vente_id.clone(), 100, "especes".into(), None).expect("paiement");
    let client = client_reel(&mut base);
    let f = argent::creer_facture_depuis_vente_sur_base(&mut base, vente_id, client, "credit".into(), None).expect("facture");
    let pid = f["piece_id"].as_str().or(f["id"].as_str()).unwrap().to_string();
    pieces_pos::modifier_facture_pos_sur_base(&mut base, pid.clone(), None, None).expect("retoucher");
    pieces_pos::valider_facture_credit_sur_base(&mut base, pid).expect("valider");
    auth::creer_utilisateur_sur_base(&mut base, "T".into(), "t".into(), None, "secret1".into(), "employe".into(), "test".into()).expect("utilisateur");
    postes::lister_sur_base(&mut base).expect("postes");
    sauvegarde::lire_config_sauvegarde_sur_base(&mut base).expect("config");
    sauvegarde::sauvegarder_config_sauvegarde_sur_base(&mut base, None, false).expect("config");
    sauvegarde::sauvegarde_auto_si_necessaire_sur_base(&mut base).expect("auto");
    images::lire_base64_sur_base(&mut base, "pied", None).expect("image");
}
