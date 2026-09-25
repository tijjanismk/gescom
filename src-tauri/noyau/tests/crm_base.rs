//! H-1/H-2 — le suivi client (PLAN-EQUIPE, D34), sur les deux moteurs
//! et sous le compte limite. Des echanges rattaches a une fiche client,
//! fondus avec ses relances de creance existantes ; des rappels
//! attribues a une personne ; un prospect qui redevient client tout
//! seul a sa premiere vente.

mod commun;

use commun::*;
use gescom_noyau::argent::{self, ParamsLigneInput};
use gescom_noyau::base::Base;
use gescom_noyau::{crm, dossiers, parametres, relances};

fn admin(base: &mut Base) -> String {
    base.lire_une("SELECT utilisateur_id FROM utilisateur_auth WHERE pseudo = 'admin'", &[], |r| r.get::<String>(0))
        .unwrap()
        .unwrap()
}

/// Une vente a credit du client reel de la demo ; rend son id.
fn vendre_au_client(base: &mut Base, client_id: &str) -> String {
    let depot = depot_defaut(base);
    let sucre = article_unite(base, "Sucre");
    let r = argent::creer_vente_sur_base(
        base, client_id.to_string(), depot.clone(), "credit".into(),
        vec![ParamsLigneInput {
            article_id: sucre.0.clone(), unite_vente_id: sucre.1.clone(), depot_source_id: depot,
            source_approvisionnement: "stock".into(), quantite: 2.0, facteur: sucre.2,
            prix_reference: sucre.3, prix_pratique: sucre.3, taux_tva: None, a_decouvert: None,
        }],
        None, None, None, None,
    )
    .expect("vente");
    r["vente_id"].as_str().unwrap().to_string()
}

fn statut_client(base: &mut Base, client_id: &str) -> String {
    let dossier = base.dossier().to_string();
    base.lire_une(
        "SELECT statut FROM client WHERE id = ?1 AND dossier_id = ?2",
        &parametres![client_id, dossier],
        |r| r.get::<String>(0),
    )
    .unwrap()
    .unwrap()
}

// =====================================================================
//  H-1 — echanges
// =====================================================================

#[test]
fn un_echange_se_note_et_se_relit() {
    let mut base = base_avec_demo();
    let client = client_reel(&mut base);
    let e = crm::creer_echange_sur(&mut base, client.clone(), "Appel".into(), " veut du sucre en gros ".into(), Some("rappeler vendredi".into())).unwrap();
    assert_eq!(e["genre"], "appel");
    assert_eq!(e["quoi"], "veut du sucre en gros");

    let fil = crm::lister_echanges_sur(&mut base, client.clone()).unwrap();
    assert_eq!(fil.len(), 1);
    assert_eq!(fil[0]["source"], "echange");
    assert_eq!(fil[0]["suite_prevue"], "rappeler vendredi");

    let n = compter(
        &mut base,
        "SELECT CAST(COUNT(*) AS BIGINT) FROM journal WHERE type_evenement = 'echange_cree' AND dossier_id = ?1",
        &parametres![dossiers::DOSSIER_DEFAUT],
    );
    assert_eq!(n, 1);
}

#[test]
fn les_relances_de_creance_sont_un_echange_comme_les_autres() {
    let mut base = base_avec_demo();
    let client = client_reel(&mut base);
    let vente_id = vendre_au_client(&mut base, &client);
    relances::enregistrer_relance_sur_base(&mut base, vente_id, "whatsapp".into(), Some("Rappel amical".into())).unwrap();
    crm::creer_echange_sur(&mut base, client.clone(), "visite".into(), "Passée au magasin".into(), None).unwrap();

    // D34 : pas une liste de plus — le meme fil, tries par date.
    let fil = crm::lister_echanges_sur(&mut base, client).unwrap();
    assert_eq!(fil.len(), 2);
    let sources: Vec<&str> = fil.iter().map(|e| e["source"].as_str().unwrap()).collect();
    assert!(sources.contains(&"echange"));
    assert!(sources.contains(&"relance_creance"));
    assert!(fil.iter().any(|e| e["genre"] == "relance" && e["quoi"] == "Rappel amical"));
}

#[test]
fn un_echange_refuse_un_genre_inconnu_ou_rien_a_dire() {
    let mut base = base_avec_demo();
    let client = client_reel(&mut base);
    assert!(crm::creer_echange_sur(&mut base, client.clone(), "sms".into(), "bonjour".into(), None).unwrap_err().contains("inconnu"));
    assert!(crm::creer_echange_sur(&mut base, client.clone(), "appel".into(), "  ".into(), None).unwrap_err().contains("passé"));
    assert!(crm::creer_echange_sur(&mut base, "inconnu".into(), "appel".into(), "bonjour".into(), None).unwrap_err().contains("introuvable"));
    assert!(crm::lister_echanges_sur(&mut base, client).unwrap().is_empty(), "aucun refus n'a rien écrit");
}

// =====================================================================
//  H-2 — rappels
// =====================================================================

#[test]
fn un_rappel_s_attribue_et_se_marque_fait() {
    let mut base = base_avec_demo();
    let client = client_reel(&mut base);
    let moi = admin(&mut base);
    let demain = (chrono::Local::now().date_naive() + chrono::Duration::days(1)).format("%Y-%m-%d").to_string();

    let r = crm::creer_rappel_sur(&mut base, client.clone(), moi.clone(), demain.clone(), " rappeler pour le prix ".into()).unwrap();
    assert_eq!(r["quoi"], "rappeler pour le prix");
    assert_eq!(r["fait"], false);

    let actifs = crm::lister_rappels_sur(&mut base, Some(moi.clone()), None, false).unwrap();
    assert_eq!(actifs.len(), 1);
    assert_eq!(actifs[0]["client_id"], client);

    let id = r["id"].as_str().unwrap().to_string();
    let fait = crm::marquer_rappel_fait_sur(&mut base, id.clone()).unwrap();
    assert_eq!(fait["fait"], true);
    assert!(crm::lister_rappels_sur(&mut base, Some(moi.clone()), None, false).unwrap().is_empty(), "un rappel fait sort de la liste active");
    assert_eq!(crm::lister_rappels_sur(&mut base, Some(moi), None, true).unwrap().len(), 1, "mais reste visible avec les faits");

    assert!(crm::marquer_rappel_fait_sur(&mut base, id).unwrap_err().contains("déjà"));
}

#[test]
fn ce_qui_ne_se_rappelle_pas() {
    let mut base = base_avec_demo();
    let client = client_reel(&mut base);
    let moi = admin(&mut base);
    assert!(crm::creer_rappel_sur(&mut base, client.clone(), moi.clone(), "".into(), "quelque chose".into()).unwrap_err().contains("vide"));
    assert!(crm::creer_rappel_sur(&mut base, client.clone(), moi.clone(), "pas-une-date".into(), "quelque chose".into()).unwrap_err().contains("illisible"));
    assert!(crm::creer_rappel_sur(&mut base, client.clone(), moi, "2026-01-01".into(), "  ".into()).unwrap_err().contains("faire"));
    assert!(crm::creer_rappel_sur(&mut base, client, "inconnu".into(), "2026-01-01".into(), "quelque chose".into()).unwrap_err().contains("existe"));
}

// =====================================================================
//  H-2 — prospects
// =====================================================================

#[test]
fn un_prospect_devient_client_a_sa_premiere_vente() {
    let mut base = base_avec_demo();
    let p = crm::creer_prospect_sur(&mut base, " Boutique Konaté ".into(), Some("76 00 00 00".into()), Some("salon".into())).unwrap();
    let id = p["id"].as_str().unwrap().to_string();
    assert_eq!(p["statut"], "prospect");
    assert_eq!(p["origine_prospect"], "salon");

    let prospects = crm::lister_prospects_sur(&mut base).unwrap();
    assert!(prospects.iter().any(|x| x["id"] == id));
    assert_eq!(statut_client(&mut base, &id), "prospect");

    vendre_au_client(&mut base, &id);

    assert_eq!(statut_client(&mut base, &id), "client", "la premiere vente le rend client");
    let prospects_apres = crm::lister_prospects_sur(&mut base).unwrap();
    assert!(!prospects_apres.iter().any(|x| x["id"] == id), "il ne reparaît plus comme prospect");

    // Une vente de plus ne fait rien d'inhabituel : deja client, elle
    // ne touche plus son statut.
    vendre_au_client(&mut base, &id);
    assert_eq!(statut_client(&mut base, &id), "client");
}

#[test]
fn un_prospect_veut_un_nom() {
    let mut base = base_avec_demo();
    assert!(crm::creer_prospect_sur(&mut base, "   ".into(), None, None).unwrap_err().contains("nom"));
}

#[test]
fn le_suivi_client_reste_dans_son_dossier() {
    let mut base = base_avec_demo();
    let client = client_reel(&mut base);
    crm::creer_echange_sur(&mut base, client.clone(), "appel".into(), "Bonjour".into(), None).unwrap();
    let moi = admin(&mut base);
    crm::creer_rappel_sur(&mut base, client.clone(), moi, "2027-01-01".into(), "Relancer".into()).unwrap();
    let prospect = crm::creer_prospect_sur(&mut base, "Prospect B".into(), None, None).unwrap()["id"].as_str().unwrap().to_string();

    let b = dossiers::creer_dossier_sur(&mut base, "B".into(), "Boutique B".into(), None, None).unwrap()["id"].as_str().unwrap().to_string();
    base.choisir_dossier(&b).unwrap();

    assert!(crm::lister_echanges_sur(&mut base, client).unwrap().is_empty());
    assert!(crm::lister_rappels_sur(&mut base, None, None, true).unwrap().is_empty());
    assert!(!crm::lister_prospects_sur(&mut base).unwrap().iter().any(|x| x["id"] == prospect));
}

#[test]
fn tout_ce_qui_est_porte_ici_passe_le_detecteur() {
    let mut base = base_avec_demo();
    base.auditer(true);
    let client = client_reel(&mut base);
    let moi = admin(&mut base);

    let e = crm::creer_echange_sur(&mut base, client.clone(), "appel".into(), "Bonjour".into(), None).unwrap();
    crm::lister_echanges_sur(&mut base, client.clone()).unwrap();
    let _ = e;

    let r = crm::creer_rappel_sur(&mut base, client.clone(), moi.clone(), "2027-01-01".into(), "Relancer".into()).unwrap();
    crm::lister_rappels_sur(&mut base, Some(moi), Some(client), true).unwrap();
    crm::marquer_rappel_fait_sur(&mut base, r["id"].as_str().unwrap().into()).unwrap();

    let p = crm::creer_prospect_sur(&mut base, "Prospect détecteur".into(), None, Some("passage".into())).unwrap();
    crm::lister_prospects_sur(&mut base).unwrap();
    vendre_au_client(&mut base, p["id"].as_str().unwrap());
}
