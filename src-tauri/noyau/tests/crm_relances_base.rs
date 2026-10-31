//! EQ-4 — les clients a relancer (PLAN-EQUIPE, D50), sur les deux
//! moteurs et sous le compte limite. Un client dormant, un en retard,
//! un meilleur client qui ralentit, un actif absent de la liste ; le
//! contact recent qui met de cote ; le reglage et la liste restent
//! dans leur dossier.

mod commun;

use commun::*;
use gescom_noyau::argent::{self, ParamsLigneInput};
use gescom_noyau::base::Base;
use gescom_noyau::{comptoir, crm, crm_relances, dossiers, parametres};

fn il_y_a(jours: i64) -> String {
    (chrono::Local::now().date_naive() - chrono::Duration::days(jours))
        .format("%Y-%m-%dT10:00:00")
        .to_string()
}

fn nouveau_client(base: &mut Base, nom: &str) -> String {
    comptoir::creer_client_rapide_sur(base, nom.into(), None).unwrap()["id"].as_str().unwrap().to_string()
}

/// Une vente de `quantite` kg de sucre, payee ou a credit, datee d'il
/// y a `jours` jours ; rend son montant.
fn vendre(base: &mut Base, client: &str, quantite: f64, payee: bool, jours: i64) -> i64 {
    let depot = depot_defaut(base);
    let sucre = article_unite(base, "Sucre");
    let montant = (sucre.3 as f64 * quantite).round() as i64;
    let v = argent::creer_vente_sur_base(
        base, client.to_string(), depot.clone(), "credit".into(),
        vec![ParamsLigneInput {
            article_id: sucre.0.clone(), unite_vente_id: sucre.1.clone(), depot_source_id: depot,
            source_approvisionnement: "stock".into(), quantite, facteur: sucre.2,
            prix_reference: sucre.3, prix_pratique: sucre.3, taux_tva: None, a_decouvert: None,
        }],
        None,
        payee.then_some(montant),
        payee.then(|| "especes".to_string()),
        None,
    )
    .expect("vente");
    let vente = v["vente_id"].as_str().unwrap().to_string();
    let dossier = base.dossier().to_string();
    base.executer(
        "UPDATE vente SET date_vente = ?1 WHERE id = ?2 AND dossier_id = ?3",
        &parametres![il_y_a(jours), vente, dossier],
    )
    .unwrap();
    montant
}

fn liste(base: &mut Base, avec_creances: bool) -> serde_json::Value {
    crm_relances::lire_clients_a_relancer_sur(base, avec_creances).unwrap()
}

fn ligne<'a>(l: &'a serde_json::Value, client: &str) -> Option<&'a serde_json::Value> {
    l["lignes"].as_array().unwrap().iter().find(|x| x["client_id"] == client)
}

fn motifs(l: &serde_json::Value, client: &str) -> Vec<String> {
    ligne(l, client)
        .map(|x| x["motifs"].as_array().unwrap().iter().map(|m| m.as_str().unwrap().to_string()).collect())
        .unwrap_or_default()
}

#[test]
fn un_dormant_un_en_retard_un_actif_absent() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let dormant = nouveau_client(&mut base, "Awa Dormante");
    let en_retard = nouveau_client(&mut base, "Bakary Doit");
    let actif = nouveau_client(&mut base, "Coumba Active");
    let solde = nouveau_client(&mut base, "Drissa Solde");

    vendre(&mut base, &dormant, 1.0, true, 75);
    let du = vendre(&mut base, &en_retard, 2.0, false, 40);
    vendre(&mut base, &actif, 1.0, true, 5);
    vendre(&mut base, &actif, 1.0, true, 100);
    // Un achat recent, paye : rien a dire.
    vendre(&mut base, &solde, 1.0, true, 3);

    let l = liste(&mut base, true);
    assert_eq!(l["jours_dormant"], 60);
    assert_eq!(motifs(&l, &dormant), vec!["dormant"]);
    assert_eq!(ligne(&l, &dormant).unwrap()["jours_sans_achat"], 75);
    assert_eq!(ligne(&l, &dormant).unwrap()["texte_propose"], "Relance : pas d'achat depuis 75 jours");

    assert_eq!(motifs(&l, &en_retard), vec!["creance"], "40 jours : en retard, pas encore dormant");
    let r = ligne(&l, &en_retard).unwrap();
    assert_eq!(r["reste_en_retard"], du);
    assert_eq!(r["jours_plus_vieille_creance"], 40);

    assert!(ligne(&l, &actif).is_none(), "un client qui achete n'est pas a relancer");
    assert!(ligne(&l, &solde).is_none());

    // L'ordre de D50 : la creance d'abord.
    let ids: Vec<&str> = l["lignes"].as_array().unwrap().iter().map(|x| x["client_id"].as_str().unwrap()).collect();
    assert!(ids.iter().position(|x| *x == en_retard) < ids.iter().position(|x| *x == dormant));
}

#[test]
fn sans_les_soldes_le_motif_creance_n_existe_pas() {
    let mut base = base_avec_demo();
    let en_retard = nouveau_client(&mut base, "Bakary Doit");
    vendre(&mut base, &en_retard, 2.0, false, 40);
    let l = liste(&mut base, false);
    assert!(ligne(&l, &en_retard).is_none(), "sa presence trahirait sa dette");
    assert_eq!(l["avec_creances"], false);

    // Dormant ET en retard : il reste, pour le seul motif dormant.
    vendre(&mut base, &en_retard, 1.0, false, 90);
    let dossier = base.dossier().to_string();
    base.executer(
        "UPDATE vente SET date_vente = ?1 WHERE client_id = ?2 AND dossier_id = ?3",
        &parametres![il_y_a(90), en_retard.clone(), dossier],
    )
    .unwrap();
    let l = liste(&mut base, false);
    assert_eq!(motifs(&l, &en_retard), vec!["dormant"]);
    assert_eq!(ligne(&l, &en_retard).unwrap()["reste_en_retard"], serde_json::Value::Null);
    assert!(!ligne(&l, &en_retard).unwrap()["texte_propose"].as_str().unwrap().contains("impayée"));
}

#[test]
fn un_meilleur_client_qui_ralentit() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let fidele = nouveau_client(&mut base, "Fanta Fidèle");
    let m = vendre(&mut base, &fidele, 3.0, true, 120);
    vendre(&mut base, &fidele, 3.0, true, 200);
    vendre(&mut base, &fidele, 3.0, true, 300);
    // Le recent : un dixieme de l'habitude, il y a 10 jours.
    vendre(&mut base, &fidele, 0.3, true, 10);

    let l = liste(&mut base, true);
    assert_eq!(motifs(&l, &fidele), vec!["ralentit"]);
    let r = ligne(&l, &fidele).unwrap();
    assert_eq!(r["ca_reference"], 3 * m);
    assert_eq!(r["nb_achats_reference"], 3);
    assert_eq!(r["texte_propose"], "Relance : achète moins que d'habitude");

    // Il revient a son habitude : il sort de la liste.
    vendre(&mut base, &fidele, 3.0, true, 2);
    assert!(ligne(&liste(&mut base, true), &fidele).is_none());
}

#[test]
fn un_contact_recent_ou_un_rappel_met_de_cote() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let awa = nouveau_client(&mut base, "Awa Dormante");
    let bintou = nouveau_client(&mut base, "Bintou Dormante");
    vendre(&mut base, &awa, 1.0, true, 80);
    vendre(&mut base, &bintou, 1.0, true, 80);
    let avant = liste(&mut base, true);
    assert!(ligne(&avant, &awa).is_some() && ligne(&avant, &bintou).is_some());
    let mis_avant = avant["mis_de_cote"].as_i64().unwrap();

    // Le clic de l'ecran : un echange pour l'une, un rappel pour l'autre.
    let texte = ligne(&avant, &awa).unwrap()["texte_propose"].as_str().unwrap().to_string();
    crm::creer_echange_sur(&mut base, awa.clone(), "appel".into(), texte, None).unwrap();
    let moi = base
        .lire_une("SELECT utilisateur_id FROM utilisateur_auth WHERE pseudo = 'admin'", &[], |r| r.get::<String>(0))
        .unwrap()
        .unwrap();
    let demain = (chrono::Local::now().date_naive() + chrono::Duration::days(1)).format("%Y-%m-%d").to_string();
    let rappel = crm::creer_rappel_sur(&mut base, bintou.clone(), moi, demain, "Relance".into()).unwrap();

    let apres = liste(&mut base, true);
    assert!(ligne(&apres, &awa).is_none(), "contactee il y a moins de 14 jours");
    assert!(ligne(&apres, &bintou).is_none(), "un rappel l'attend");
    assert_eq!(apres["mis_de_cote"].as_i64().unwrap(), mis_avant + 2);

    // Le rappel fait, le contact remonte a 15 jours : elles reviennent.
    crm::marquer_rappel_fait_sur(&mut base, rappel["id"].as_str().unwrap().into()).unwrap();
    let dossier = base.dossier().to_string();
    base.executer(
        "UPDATE echange SET date_echange = ?1 WHERE client_id = ?2 AND dossier_id = ?3",
        &parametres![il_y_a(15), awa.clone(), dossier],
    )
    .unwrap();
    let encore = liste(&mut base, true);
    assert!(ligne(&encore, &awa).is_some());
    assert!(ligne(&encore, &bintou).is_some());
}

#[test]
fn le_reglage_du_dossier_change_le_seuil_et_se_juge() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let awa = nouveau_client(&mut base, "Awa Dormante");
    vendre(&mut base, &awa, 1.0, true, 75);
    assert!(ligne(&liste(&mut base, true), &awa).is_some());

    crm_relances::definir_jours_dormant_sur(&mut base, 90).unwrap();
    let l = liste(&mut base, true);
    assert_eq!(l["jours_dormant"], 90);
    assert!(ligne(&l, &awa).is_none(), "75 jours sous un seuil de 90");

    assert!(crm_relances::definir_jours_dormant_sur(&mut base, 10).unwrap_err().contains("15"));
    assert!(crm_relances::definir_jours_dormant_sur(&mut base, 400).unwrap_err().contains("365"));
    assert_eq!(crm_relances::jours_dormant_sur(&mut base), 90, "un refus ne change rien");
}

#[test]
fn la_liste_et_le_reglage_restent_dans_leur_dossier() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let awa = nouveau_client(&mut base, "Awa Dormante");
    vendre(&mut base, &awa, 1.0, true, 75);
    let bakary = nouveau_client(&mut base, "Bakary Doit");
    vendre(&mut base, &bakary, 1.0, false, 40);
    crm_relances::definir_jours_dormant_sur(&mut base, 30).unwrap();

    let b = dossiers::creer_dossier_sur(&mut base, "B".into(), "Boutique B".into(), None, None)
        .unwrap()["id"].as_str().unwrap().to_string();
    base.choisir_dossier(&b).unwrap();
    let l = liste(&mut base, true);
    assert_eq!(l["jours_dormant"], 60, "le reglage de A ne vaut pas pour B");
    assert!(ligne(&l, &awa).is_none());
    assert!(ligne(&l, &bakary).is_none());
    assert!(l["lignes"].as_array().unwrap().is_empty(), "B n'a encore rien vendu");

    base.choisir_dossier(dossiers::DOSSIER_DEFAUT).unwrap();
    assert_eq!(liste(&mut base, true)["jours_dormant"], 30);
}

// --- D50, points 10 a 16 ---

fn fiche(base: &mut Base, client: &str) -> serde_json::Value {
    gescom_noyau::pieces::lire_fiche_client_sur_base(base, client.to_string()).unwrap()["client"].clone()
}

#[test]
fn l_etape_d_un_prospect_et_son_compte_du_mois() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let salon = crm::creer_prospect_sur(&mut base, "Fatou Salon".into(), None, Some("salon".into())).unwrap()["id"]
        .as_str().unwrap().to_string();
    let reco = crm::creer_prospect_sur(&mut base, "Modibo Reco".into(), None, Some("recommandation".into())).unwrap()["id"]
        .as_str().unwrap().to_string();
    let lus = crm::lister_prospects_sur(&mut base).unwrap();
    let etape = |l: &Vec<serde_json::Value>, id: &str| l.iter().find(|p| p["id"] == id).unwrap()["etape"].clone();
    assert_eq!(etape(&lus, &salon), "nouveau", "vide vaut nouveau");

    // Un echange le passe « contacte », sans rien saisir.
    crm::creer_echange_sur(&mut base, salon.clone(), "appel".into(), "Premier appel".into(), None).unwrap();
    assert_eq!(etape(&crm::lister_prospects_sur(&mut base).unwrap(), &salon), "contacte");
    crm::changer_etape_prospect_sur(&mut base, salon.clone(), "interesse".into(), None).unwrap();
    crm::creer_echange_sur(&mut base, salon.clone(), "visite".into(), "Passe voir".into(), None).unwrap();
    assert_eq!(etape(&crm::lister_prospects_sur(&mut base).unwrap(), &salon), "interesse", "un echange ne fait pas reculer");

    // L'abandon veut une raison de la liste.
    assert!(crm::changer_etape_prospect_sur(&mut base, reco.clone(), "abandonne".into(), None).unwrap_err().contains("pourquoi"));
    crm::changer_etape_prospect_sur(&mut base, reco.clone(), "abandonne".into(), Some("trop_cher".into())).unwrap();
    let c = crm::lire_compte_prospects_sur(&mut base, None).unwrap();
    assert_eq!(c["crees"], 2);
    assert_eq!(c["devenus_clients"], 0);
    assert_eq!(c["abandons"], serde_json::json!([{ "raison": "trop_cher", "nombre": 1 }]));

    // L'abandonne achete quand meme : il devient client, date du jour.
    vendre(&mut base, &reco, 1.0, true, 0);
    let f = fiche(&mut base, &reco);
    assert_eq!(f["statut"], "client");
    assert_eq!(f["raison_sortie"], serde_json::Value::Null, "plus d'abandon pour un client");
    let c = crm::lire_compte_prospects_sur(&mut base, None).unwrap();
    assert_eq!(c["devenus_clients"], 1);
    assert_eq!(c["abandons"], serde_json::json!([]));
    let reco_origine = c["par_origine"].as_array().unwrap().iter().find(|o| o["origine"] == "recommandation").unwrap().clone();
    assert_eq!((reco_origine["crees"].as_i64(), reco_origine["devenus_clients"].as_i64()), (Some(1), Some(1)));
    assert!(crm::changer_etape_prospect_sur(&mut base, reco, "contacte".into(), None).unwrap_err().contains("plus un prospect"));

    // Un autre mois : rien.
    let vide = crm::lire_compte_prospects_sur(&mut base, Some("2001-01".into())).unwrap();
    assert_eq!((vide["crees"].as_i64(), vide["devenus_clients"].as_i64()), (Some(0), Some(0)));
    assert!(crm::lire_compte_prospects_sur(&mut base, Some("janvier".into())).is_err());
}

#[test]
fn un_parti_sort_de_la_liste_sauf_pour_sa_dette_et_revient_en_rachetant() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let awa = nouveau_client(&mut base, "Awa Partie");
    let bakary = nouveau_client(&mut base, "Bakary Doit");
    vendre(&mut base, &awa, 1.0, true, 75);
    let du = vendre(&mut base, &bakary, 1.0, false, 75);
    assert_eq!(motifs(&liste(&mut base, true), &bakary), vec!["creance", "dormant"]);

    crm::marquer_client_parti_sur(&mut base, awa.clone(), "prix".into()).unwrap();
    crm::marquer_client_parti_sur(&mut base, bakary.clone(), "concurrent".into()).unwrap();
    let l = liste(&mut base, true);
    assert!(ligne(&l, &awa).is_none(), "parti sans dette : plus a relancer");
    assert_eq!(motifs(&l, &bakary), vec!["creance"], "D50, point 15 : la dette reste, pas le dormant");
    assert_eq!(ligne(&l, &bakary).unwrap()["parti"], true);
    assert_eq!(ligne(&l, &bakary).unwrap()["reste_en_retard"], du);
    assert_eq!(l["partis"]["nombre"], 2);
    assert_eq!(l["partis"]["par_raison"], serde_json::json!([{ "raison": "concurrent", "nombre": 1 }, { "raison": "prix", "nombre": 1 }]));
    assert_eq!(l["partis"]["reconquis_mois"], 0);
    let cles: Vec<&String> = l["partis"]["liste"][0].as_object().unwrap().keys().collect();
    assert_eq!(cles, vec!["client_id", "code", "nom", "parti_le", "raison"], "aucun montant dans les partis");
    assert!(fiche(&mut base, &bakary)["parti_le"].is_string());

    assert!(crm::marquer_client_parti_sur(&mut base, awa.clone(), "prix".into()).unwrap_err().contains("déjà"));
    assert!(crm::marquer_client_parti_sur(&mut base, awa.clone(), "trop_cher".into()).unwrap_err().contains("inconnue"));
    let prospect = crm::creer_prospect_sur(&mut base, "Pas encore".into(), None, None).unwrap()["id"].as_str().unwrap().to_string();
    assert!(crm::marquer_client_parti_sur(&mut base, prospect, "prix".into()).unwrap_err().contains("jamais acheté"));

    // Awa rachete : elle revient toute seule, la vente n'est pas bloquee.
    vendre(&mut base, &awa, 1.0, true, 0);
    let f = fiche(&mut base, &awa);
    assert_eq!(f["parti_le"], serde_json::Value::Null);
    assert_eq!(f["raison_sortie"], serde_json::Value::Null);
    let l = liste(&mut base, true);
    assert_eq!(l["partis"]["nombre"], 1);
    assert_eq!(l["partis"]["reconquis_mois"], 1);

    // « Il est revenu » par erreur de clic : pas un reconquis.
    crm::annuler_client_parti_sur(&mut base, bakary.clone()).unwrap();
    let l = liste(&mut base, true);
    assert_eq!(l["partis"]["nombre"], 0);
    assert_eq!(l["partis"]["reconquis_mois"], 1);
    assert_eq!(motifs(&l, &bakary), vec!["creance", "dormant"], "de nouveau un client comme un autre");
    assert!(crm::annuler_client_parti_sur(&mut base, bakary).unwrap_err().contains("pas marqué"));
}

#[test]
fn les_partis_et_le_compte_restent_dans_leur_dossier() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let awa = nouveau_client(&mut base, "Awa Partie");
    vendre(&mut base, &awa, 1.0, true, 75);
    crm::marquer_client_parti_sur(&mut base, awa.clone(), "demenage".into()).unwrap();
    crm::creer_prospect_sur(&mut base, "Fatou Salon".into(), None, Some("salon".into())).unwrap();

    let b = dossiers::creer_dossier_sur(&mut base, "B".into(), "Boutique B".into(), None, None)
        .unwrap()["id"].as_str().unwrap().to_string();
    base.choisir_dossier(&b).unwrap();
    assert_eq!(liste(&mut base, true)["partis"]["nombre"], 0);
    assert_eq!(crm::lire_compte_prospects_sur(&mut base, None).unwrap()["crees"], 0);
    assert!(crm::annuler_client_parti_sur(&mut base, awa.clone()).unwrap_err().contains("introuvable"));
    assert!(crm::marquer_client_parti_sur(&mut base, awa, "prix".into()).unwrap_err().contains("introuvable"));
}

#[test]
fn tout_ce_qui_est_porte_ici_passe_le_detecteur() {
    let mut base = base_avec_demo();
    base.auditer(true);
    ouvrir_caisse(&mut base);
    let awa = nouveau_client(&mut base, "Awa Dormante");
    vendre(&mut base, &awa, 1.0, true, 75);
    let bakary = nouveau_client(&mut base, "Bakary Doit");
    vendre(&mut base, &bakary, 1.0, false, 40);
    crm::creer_echange_sur(&mut base, bakary.clone(), "appel".into(), "Relance".into(), None).unwrap();
    crm_relances::definir_jours_dormant_sur(&mut base, 45).unwrap();
    // D50, 10 a 16 : etape, abandon, parti, retour, compte.
    let p = crm::creer_prospect_sur(&mut base, "Fatou".into(), None, Some("salon".into())).unwrap()["id"].as_str().unwrap().to_string();
    crm::creer_echange_sur(&mut base, p.clone(), "appel".into(), "Bonjour".into(), None).unwrap();
    crm::changer_etape_prospect_sur(&mut base, p.clone(), "abandonne".into(), Some("autre".into())).unwrap();
    vendre(&mut base, &p, 1.0, true, 0);
    crm::marquer_client_parti_sur(&mut base, awa.clone(), "prix".into()).unwrap();
    crm::annuler_client_parti_sur(&mut base, awa.clone()).unwrap();
    crm::marquer_client_parti_sur(&mut base, awa.clone(), "mecontent".into()).unwrap();
    vendre(&mut base, &awa, 1.0, true, 0);
    crm::lire_compte_prospects_sur(&mut base, None).unwrap();
    crm::lister_prospects_sur(&mut base).unwrap();
    gescom_noyau::pieces::lire_fiche_client_sur_base(&mut base, bakary).unwrap();
    crm_relances::lire_clients_a_relancer_sur(&mut base, true).unwrap();
    crm_relances::lire_clients_a_relancer_sur(&mut base, false).unwrap();
}
