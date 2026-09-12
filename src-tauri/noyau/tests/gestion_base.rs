//! `depots`, `avoirs`, `chantiers`, `creances` — sur `Base`.
//! Testé sur SQLite par défaut, sur PostgreSQL avec `GESCOM_PG`.

mod commun;

use commun::*;
use gescom_noyau::argent::{self, ParamsLigneInput};
use gescom_noyau::base::Base;
use gescom_noyau::parametres;
use gescom_noyau::{achats, avoirs, chantiers, codebarre, creances, depots, pieces};

fn vendre_credit(base: &mut Base, quantite: f64) -> (String, i64) {
    let depot = depot_defaut(base);
    let sucre = article_unite(base, "Sucre");
    let client = client_reel(base);
    let v = argent::creer_vente_sur_base(
        base,
        client,
        depot.clone(),
        "credit".into(),
        vec![ParamsLigneInput {
            article_id: sucre.0.clone(),
            unite_vente_id: sucre.1.clone(),
            depot_source_id: depot,
            source_approvisionnement: "stock".into(),
            quantite,
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
    .expect("vente à crédit");
    (v["vente_id"].as_str().unwrap().to_string(), (sucre.3 as f64 * quantite).round() as i64)
}

fn statut_vente(base: &mut Base, id: &str) -> String {
    base.lire_une("SELECT statut FROM vente WHERE id = ?1", &parametres![id], |r| r.get::<String>(0))
        .unwrap()
        .unwrap()
}

#[test]
fn un_magasin_se_cree_se_renomme_et_ne_se_ferme_pas_sur_du_stock() {
    let mut base = base_avec_demo();
    let sucre = article_unite(&mut base, "Sucre");
    let avant = depots::lire_depots_detail_sur_base(&mut base).unwrap();
    assert_eq!(avant.len(), 1);
    assert_eq!(avant[0]["est_defaut"], true);

    let r = depots::creer_depot_sur_base(&mut base, " Annexe ".into(), None).unwrap();
    let id = r["id"].as_str().unwrap().to_string();
    assert_eq!(r["nom"], "Annexe");
    let n = compter(&mut base, "SELECT COUNT(*) FROM stock_depot WHERE depot_id = ?1", &parametres![id.clone()]);
    assert!(n > 0, "le stock à zéro est posé pour chaque article");
    assert_eq!(stock(&mut base, &sucre.0, &id), 0.0);

    depots::renommer_depot_sur_base(&mut base, id.clone(), "Annexe Nord".into()).unwrap();
    let detail = depots::lire_depots_detail_sur_base(&mut base).unwrap();
    assert!(detail.iter().any(|d| d["nom"] == "Annexe Nord" && d["nb_articles"] == 0));

    // Le magasin par défaut ne se ferme pas ; le nouveau, vide, oui.
    let defaut = depot_defaut(&mut base);
    let refus = depots::desactiver_depot_sur_base(&mut base, defaut.clone(), None).unwrap_err();
    assert!(refus.contains("par défaut"));
    depots::desactiver_depot_sur_base(&mut base, id.clone(), None).unwrap();
    assert!(depots::reactiver_depot_sur_base(&mut base, id.clone()).is_ok());
    assert!(depots::reactiver_depot_sur_base(&mut base, id.clone()).is_err(), "déjà actif");

    // Avec du stock : refus, sauf forcé — et alors une trace au journal.
    depots::definir_depot_defaut_sur_base(&mut base, id.clone()).unwrap();
    assert_eq!(depot_defaut(&mut base), id);
    let refus = depots::desactiver_depot_sur_base(&mut base, defaut.clone(), None).unwrap_err();
    assert!(refus.contains("unité(s) en stock"), "{refus}");
    depots::desactiver_depot_sur_base(&mut base, defaut.clone(), Some(true)).unwrap();
    assert_eq!(compter(&mut base, "SELECT COUNT(*) FROM journal WHERE type_evenement = 'depot_desactive_avec_stock'", &[]), 1);
    let visible = depots::lire_stock_article_depots_sur_base(&mut base, sucre.0.clone()).unwrap();
    assert_eq!(visible.len(), 1, "le magasin fermé sort des écrans");
    assert_eq!(visible[0]["depot_id"], id);
    assert_eq!(stock(&mut base, &sucre.0, &defaut), 200.0, "le stock gelé est intact");
}

#[test]
fn les_vues_de_stock_et_les_mouvements_se_lisent() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let sucre = article_unite(&mut base, "Sucre");
    let depot = depot_defaut(&mut base);
    let (vente_id, prix) = vendre_credit(&mut base, 2.0);

    let s = depots::lire_stock_depot_sur_base(&mut base, depot.clone()).unwrap();
    assert!(s.iter().any(|a| a["article_id"] == sucre.0 && a["quantite"] == 198.0));
    let multi = depots::lire_stock_multi_depots_sur_base(&mut base).unwrap();
    assert!(multi.iter().any(|m| m["article_id"] == sucre.0 && m["est_defaut"] == true));

    let resume = depots::lire_resume_par_depot_sur_base(&mut base, None, None).unwrap();
    assert_eq!(resume.len(), 1);
    assert_eq!(resume[0]["ca"], prix);
    assert_eq!(resume[0]["impaye"], prix);

    let m = depots::lire_mouvements_stock_sur_base(&mut base, Some(sucre.0.clone()), None, Some("vente".into()), None, None, None).unwrap();
    assert_eq!(m.len(), 1);
    assert_eq!(m[0]["quantite"], 2.0);
    assert_eq!(m[0]["entrant"], false);
    let m = depots::lire_mouvements_stock_sur_base(&mut base, None, None, None, Some("2099-01-01".into()), None, None).unwrap();
    assert!(m.is_empty());
    let _ = vente_id;
}

#[test]
fn un_avoir_s_applique_a_une_vente_puis_se_rembourse() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let client = client_reel(&mut base);
    let sucre = article_unite(&mut base, "Sucre");
    let avc = pieces::creer_piece_sur_base(&mut base, client.clone(), "avoir_client".into(), vec![ligne(&sucre, 5.0)], None, None, None, None, None).unwrap();
    let avc_id = avc["id"].as_str().unwrap().to_string();
    let credit = 5 * sucre.3;
    assert_eq!(avoirs::total_avoirs_client_sur_base(&mut base, client.clone()).unwrap(), credit);
    assert_eq!(avoirs::lire_avoirs_client_sur_base(&mut base, client.clone()).unwrap().len(), 1);

    // Vente à crédit de 2 kg : l'avoir en couvre le prix, le solde reste ouvert.
    let (vente_id, prix) = vendre_credit(&mut base, 2.0);
    let applique = avoirs::appliquer_avoir_vente_sur_base(&mut base, vente_id.clone(), client.clone(), prix).unwrap();
    assert_eq!(applique, prix);
    assert_eq!(statut_vente(&mut base, &vente_id), "payee");
    assert_eq!(avoirs::total_avoirs_client_sur_base(&mut base, client.clone()).unwrap(), credit - prix);
    let ouverts = avoirs::lire_avoirs_client_sur_base(&mut base, client.clone()).unwrap();
    assert_eq!(ouverts.len(), 1, "le solde est un nouvel avoir, rattaché à la même AVC");

    // Le client de passage n'a pas d'avoir.
    let generique = client_generique(&mut base);
    assert!(avoirs::appliquer_avoir_vente_sur_base(&mut base, vente_id, generique, 1).is_err());

    // Remboursement du reste, plafonné au crédit, avec sortie de caisse.
    let r = avoirs::rembourser_avoir_sur_base(&mut base, avc_id.clone(), 1_000_000, "especes".into(), None).unwrap();
    assert_eq!(r["montant_rembourse"], credit - prix);
    assert_eq!(r["solde"], true);
    assert_eq!(statut_piece(&mut base, &avc_id), "paye");
    let refus = avoirs::rembourser_avoir_sur_base(&mut base, avc_id, 1, "especes".into(), None).unwrap_err();
    assert!(refus.contains("consommé ou remboursé"));
}

#[test]
fn le_scanner_retrouve_un_article_par_son_code() {
    let mut base = base_avec_demo();
    let sucre = article_unite(&mut base, "Sucre");
    assert!(avoirs::chercher_article_par_code_barre_sur_base(&mut base, "0000000000000".into()).unwrap().is_none());
    let code = codebarre::generer_code_barre_sur_base(&mut base, sucre.0.clone()).unwrap();
    let a = avoirs::chercher_article_par_code_barre_sur_base(&mut base, code).unwrap().expect("trouvé");
    assert_eq!(a["nom"], "Sucre");
    assert_eq!(a["stock"], 200.0);
    assert_eq!(a["unites"].as_array().unwrap().len(), 2);
    assert!(a["unite_scannee_id"].is_null());

    avoirs::sauvegarder_config_scanner_sur_base(&mut base, true).unwrap();
    assert!(gescom_noyau::comptoir::lire_config_scanner_sur(&mut base).unwrap());
    avoirs::sauvegarder_code_barre_article_sur_base(&mut base, sucre.0.clone(), "3017620422003".into()).unwrap();
    let liste = avoirs::lire_articles_avec_codes_barres_sur_base(&mut base).unwrap();
    assert!(liste.iter().any(|a| a["id"] == sucre.0 && a["code_barre"] == "3017620422003"));
}

#[test]
fn tva_dettes_irrecouvrable_et_expiration() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let sucre = article_unite(&mut base, "Sucre");

    chantiers::sauvegarder_tva_article_sur_base(&mut base, sucre.0.clone(), 0.18).unwrap();
    let taux = chantiers::lire_taux_tva_sur_base(&mut base).unwrap();
    assert!(taux.iter().any(|t| t["id"] == sucre.0 && t["taux_tva"] == 0.18));
    let tva = chantiers::lire_resume_tva_sur_base(&mut base, "2000-01-01".into(), "2099-01-01".into()).unwrap();
    assert_eq!(tva["total_tva"], 0);

    // Dette fournisseur : un achat à crédit de 5 000, moins rien.
    let f = fournisseur(&mut base, "Grossiste");
    achats::enregistrer_achat_sur_base(
        &mut base, Some(f.clone()), None,
        vec![achats::LigneAchat { article_id: sucre.0.clone(), unite_vente_id: sucre.1.clone(), quantite: 10.0, facteur: 1.0, prix_achat: 500 }],
        Some("credit".into()), None, Some(1_000), None, None, None,
    )
    .unwrap();
    let dettes = chantiers::lire_dettes_fournisseurs_sur_base(&mut base).unwrap();
    let g = dettes.iter().find(|d| d["id"] == f).unwrap();
    assert_eq!(g["total_achats"], 5_000);
    assert_eq!(g["total_paye"], 1_000);
    assert_eq!(g["dette"], 4_000);
    let ouvertes = chantiers::lire_factures_fournisseur_ouvertes_sur_base(&mut base, f).unwrap();
    assert_eq!(ouvertes.len(), 1);
    assert_eq!(ouvertes[0]["reste"], 4_000);

    // Irrécouvrable.
    let (vente_id, prix) = vendre_credit(&mut base, 1.0);
    chantiers::marquer_irrecouvrable_sur_base(&mut base, vente_id.clone(), "parti sans adresse".into()).unwrap();
    assert_eq!(statut_vente(&mut base, &vente_id), "irrecouvrable");
    let refus = chantiers::marquer_irrecouvrable_sur_base(&mut base, vente_id, "encore".into()).unwrap_err();
    assert!(refus.contains("déjà"));
    let perdues = chantiers::lire_irrecouvrable_sur_base(&mut base).unwrap();
    assert_eq!(perdues.len(), 1);
    assert_eq!(perdues[0]["montant_perdu"], prix);

    // Expiration : un avoir vieux de 100 jours expire, un frais non.
    let client = client_reel(&mut base);
    let dossier = base.dossier().to_string();
    let vieux = (chrono::Local::now() - chrono::Duration::days(100)).format("%Y-%m-%dT10:00:00").to_string();
    base.executer(
        "INSERT INTO avoir (id, client_id, montant, statut, cree_le, origine, dossier_id)
         VALUES ('a-vieux', ?1, 100, 'ouvert', ?2, 'test', ?3)",
        &parametres![client.clone(), vieux, dossier.clone()],
    )
    .unwrap();
    base.executer(
        "INSERT INTO avoir (id, client_id, montant, statut, cree_le, origine, dossier_id)
         VALUES ('a-frais', ?1, 100, 'ouvert', ?2, 'test', ?3)",
        &parametres![client, gescom_noyau::utils::maintenant_iso(), dossier],
    )
    .unwrap();
    assert_eq!(chantiers::expirer_avoirs_sur_base(&mut base).unwrap(), 0, "inactif par défaut");
    assert!(chantiers::sauvegarder_config_avoirs_sur_base(&mut base, true, 10).is_err(), "minimum 30 jours");
    chantiers::sauvegarder_config_avoirs_sur_base(&mut base, true, 90).unwrap();
    let cfg = chantiers::lire_config_avoirs_sur_base(&mut base).unwrap();
    assert_eq!(cfg["active"], true);
    assert_eq!(cfg["duree_jours"], 90);
    assert_eq!(chantiers::expirer_avoirs_sur_base(&mut base).unwrap(), 1);
    let expires = chantiers::lire_avoirs_expires_sur_base(&mut base).unwrap();
    assert_eq!(expires.len(), 1);
    assert_eq!(expires[0]["id"], "a-vieux");
    assert!(chantiers::reactiver_avoir_sur_base(&mut base, "a-vieux".into(), Some("employe".into())).is_err());
    assert!(chantiers::reactiver_avoir_sur_base(&mut base, "a-frais".into(), Some("patron".into())).is_err(), "pas expiré");
    chantiers::reactiver_avoir_sur_base(&mut base, "a-vieux".into(), Some("patron".into())).unwrap();
    assert!(chantiers::lire_avoirs_expires_sur_base(&mut base).unwrap().is_empty());
}

#[test]
fn une_creance_se_regle_en_deux_fois_puis_un_reglement_s_annule() {
    let mut base = base_avec_demo();
    let (vente_id, prix) = vendre_credit(&mut base, 2.0);
    let client = client_reel(&mut base);

    let etat = creances::lire_etat_creances_client_sur_base(&mut base, client.clone()).unwrap();
    assert_eq!(etat["total_du"], prix);
    assert_eq!(etat["net_du"], prix);
    let global = creances::lire_etat_creances_global_sur_base(&mut base).unwrap();
    assert_eq!(global["total_general"], prix);
    assert_eq!(global["lignes"][0]["nb"], 1);
    let ouvertes = creances::lire_creances_ouvertes_sur_base(&mut base, None).unwrap();
    assert_eq!(ouvertes.len(), 1);
    assert!(creances::lire_creances_ouvertes_sur_base(&mut base, Some("zzz".into())).unwrap().is_empty());

    // Caisse fermée : refus ; avoir : passe sans caisse.
    let refus = creances::regler_creance_sur_base(&mut base, vente_id.clone(), 100, "especes".into(), None).unwrap_err();
    assert!(refus.contains("CAISSE_FERMEE"));
    ouvrir_caisse(&mut base);
    let r = creances::regler_creance_sur_base(&mut base, vente_id.clone(), 300, "especes".into(), None).unwrap();
    assert_eq!(r["montant_encaisse"], 300);
    assert_eq!(r["soldee"], false);
    assert_eq!(statut_vente(&mut base, &vente_id), "partiellement_payee");
    let r = creances::regler_creance_sur_base(&mut base, vente_id.clone(), 1_000_000, "especes".into(), None).unwrap();
    assert_eq!(r["montant_encaisse"], prix - 300, "plafonné au reste dû");
    assert_eq!(r["soldee"], true);
    assert!(creances::regler_creance_sur_base(&mut base, vente_id.clone(), 1, "especes".into(), None).is_err());
    let entrees = compter(&mut base, "SELECT CAST(COALESCE(SUM(montant),0) AS BIGINT) FROM mouvement_caisse WHERE sens = 'entree' AND motif = 'vente'", &[]);
    assert_eq!(entrees, prix);

    let reglements = creances::lire_reglements_client_sur_base(&mut base, client.clone()).unwrap();
    assert_eq!(reglements.len(), 2);
    assert_eq!(reglements[0]["reste_apres"], 0);
    assert_eq!(reglements[1]["reste_apres"], prix - 300);
    assert_eq!(reglements[1]["deja_annule"], false);

    let recu = creances::lire_donnees_recu_sur_base(&mut base, reglements[1]["id"].as_str().unwrap().to_string(), "client".into()).unwrap();
    assert_eq!(recu["montant"], 300);
    assert_eq!(recu["reste_du"], 0, "le reste du reçu est celui d'aujourd'hui");

    // Annulation du premier règlement, argent rendu : contre-passation + sortie.
    let premier = reglements[1]["id"].as_str().unwrap().to_string();
    let r = creances::annuler_reglement_sur_base(&mut base, premier.clone(), "erreur".into(), true, None).unwrap();
    assert_eq!(r["montant_annule"], 300);
    assert_eq!(r["sortie_de_caisse"], true);
    assert_eq!(r["nouveau_statut"], "partiellement_payee");
    assert_eq!(r["reste_du"], 300);
    assert!(creances::annuler_reglement_sur_base(&mut base, premier, "encore".into(), true, None).is_err(), "pas deux fois");
    let reglements = creances::lire_reglements_client_sur_base(&mut base, client).unwrap();
    assert_eq!(reglements.len(), 3);
    assert!(reglements.iter().any(|p| p["est_annulation"] == true && p["montant"] == -300));
    assert!(reglements.iter().any(|p| p["deja_annule"] == true));

    let sim = creances::solder_residus_creances_sur_base(&mut base, Some("patron".into()), None).unwrap();
    assert_eq!(sim["simulation"], true);
    assert_eq!(sim["concernees"], 0);
}

#[test]
fn tout_ce_qui_est_porte_ici_passe_le_detecteur() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let sucre = article_unite(&mut base, "Sucre");
    let client = client_reel(&mut base);
    let f = fournisseur(&mut base, "F");
    let (vente_id, _) = vendre_credit(&mut base, 1.0);
    base.auditer(true);

    depots::lire_depots_detail_sur_base(&mut base).expect("magasins");
    let d = depots::creer_depot_sur_base(&mut base, "D".into(), None).expect("créer");
    let did = d["id"].as_str().unwrap().to_string();
    depots::renommer_depot_sur_base(&mut base, did.clone(), "D2".into()).expect("renommer");
    depots::lire_stock_depot_sur_base(&mut base, did.clone()).expect("stock magasin");
    depots::lire_resume_par_depot_sur_base(&mut base, None, None).expect("résumé");
    depots::lire_stock_article_depots_sur_base(&mut base, sucre.0.clone()).expect("stock article");
    depots::lire_stock_multi_depots_sur_base(&mut base).expect("multi");
    depots::lire_mouvements_stock_sur_base(&mut base, None, None, None, None, None, None).expect("mouvements");
    depots::desactiver_depot_sur_base(&mut base, did.clone(), Some(true)).expect("désactiver");
    depots::reactiver_depot_sur_base(&mut base, did.clone()).expect("réactiver");
    depots::definir_depot_defaut_sur_base(&mut base, did).expect("défaut");

    avoirs::lire_avoirs_client_sur_base(&mut base, client.clone()).expect("avoirs");
    avoirs::total_avoirs_client_sur_base(&mut base, client.clone()).expect("total");
    let avc = pieces::creer_piece_sur_base(&mut base, client.clone(), "avoir_client".into(), vec![ligne(&sucre, 1.0)], None, None, None, None, None).unwrap();
    avoirs::appliquer_avoir_vente_sur_base(&mut base, vente_id.clone(), client.clone(), 100).expect("appliquer");
    avoirs::rembourser_avoir_sur_base(&mut base, avc["id"].as_str().unwrap().to_string(), 100, "especes".into(), None).expect("rembourser");
    avoirs::chercher_article_par_code_barre_sur_base(&mut base, "x".into()).expect("scanner");
    avoirs::sauvegarder_config_scanner_sur_base(&mut base, true).expect("config scanner");
    avoirs::sauvegarder_code_barre_article_sur_base(&mut base, sucre.0.clone(), "3017620422003".into()).expect("code");
    avoirs::lire_articles_avec_codes_barres_sur_base(&mut base).expect("codes");

    chantiers::lire_taux_tva_sur_base(&mut base).expect("tva");
    chantiers::sauvegarder_tva_article_sur_base(&mut base, sucre.0.clone(), 0.18).expect("tva article");
    chantiers::lire_resume_tva_sur_base(&mut base, "2000-01-01".into(), "2099-01-01".into()).expect("résumé tva");
    chantiers::lire_dettes_fournisseurs_sur_base(&mut base).expect("dettes");
    chantiers::lire_factures_fournisseur_ouvertes_sur_base(&mut base, f).expect("factures ouvertes");
    chantiers::lire_irrecouvrable_sur_base(&mut base).expect("irrécouvrables");
    chantiers::lire_config_avoirs_sur_base(&mut base).expect("config avoirs");
    chantiers::sauvegarder_config_avoirs_sur_base(&mut base, true, 60).expect("config avoirs");
    chantiers::expirer_avoirs_sur_base(&mut base).expect("expirer");
    chantiers::lire_avoirs_expires_sur_base(&mut base).expect("expirés");

    creances::lire_etat_creances_client_sur_base(&mut base, client.clone()).expect("état client");
    creances::lire_etat_creances_global_sur_base(&mut base).expect("état global");
    creances::lire_creances_ouvertes_sur_base(&mut base, None).expect("ouvertes");
    creances::regler_creance_sur_base(&mut base, vente_id.clone(), 100, "especes".into(), None).expect("régler");
    let regs = creances::lire_reglements_client_sur_base(&mut base, client).expect("règlements");
    let pid = regs[0]["id"].as_str().unwrap().to_string();
    creances::lire_donnees_recu_sur_base(&mut base, pid.clone(), "client".into()).expect("reçu");
    creances::annuler_reglement_sur_base(&mut base, pid, "m".into(), false, None).expect("annuler");
    creances::solder_residus_creances_sur_base(&mut base, Some("patron".into()), Some(false)).expect("résidus");
    chantiers::marquer_irrecouvrable_sur_base(&mut base, vente_id, "m".into()).expect("irrécouvrable");
}
