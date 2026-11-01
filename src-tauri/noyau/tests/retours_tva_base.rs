//! D53 — un article rendu sort de la TVA et du chiffre d'affaires À LA
//! DATE DU RETOUR, au prorata de la quantité rendue ; une période déjà
//! déclarée ne bouge pas.
//!
//! - la TVA rendue se déduit du rapport TVA et du résumé de l'onglet
//!   Chantiers à la date du retour, au taux de la ligne vendue, cumulée
//!   ligne par ligne (`coeur::tva`) ; l'échange n'en sort pas ;
//! - le cahier du jour et le CA par magasin sont nets des retours ;
//!   l'encaissé n'y compte plus la dette éteinte par un retour ;
//! - le relevé client imprimé met cette dette dans « Rendu », pas dans
//!   « Réglé » ; sa ligne de règlement n'a ni reçu ni annulation.
//!
//! Chaque scénario vérifie, après le geste, stock, caisse, créance, CA
//! et TVA. Testé sur SQLite par défaut, sur PostgreSQL avec `GESCOM_PG`
//! (`--test-threads=1`). Le dernier rejoue tout sous le détecteur de
//! requêtes non cloisonnées.

mod commun;

use commun::*;
use gescom_noyau::argent::{self, ParamsLigneInput};
use gescom_noyau::base::Base;
use gescom_noyau::parametres;
use gescom_noyau::{chantiers, creances, depots, journal, rapports, retours, tableau_bord};

// =====================================================================
//  Aides
// =====================================================================

const SUCRE: i64 = 800;
const RIZ: i64 = 600;
const TVA: f64 = 0.18;

/// Une vente au client donné : des (article, quantité), au taux `tva`
/// (TVA comprise dans le prix), datée du jour ou de `date`, avec `paye`
/// encaissé. Rend (vente, lignes dans l'ordre des articles).
fn vendre(
    base: &mut Base,
    client: &str,
    articles: &[(&str, f64)],
    tva: Option<f64>,
    paye: Option<i64>,
    date: Option<String>,
) -> (String, Vec<String>) {
    let depot = depot_defaut(base);
    let mut lignes = Vec::new();
    let mut ids_articles = Vec::new();
    for (nom, quantite) in articles {
        let art = article_unite(base, nom);
        ids_articles.push(art.0.clone());
        lignes.push(ParamsLigneInput {
            article_id: art.0.clone(),
            unite_vente_id: art.1.clone(),
            depot_source_id: depot.clone(),
            source_approvisionnement: "stock".into(),
            quantite: *quantite,
            facteur: art.2,
            prix_reference: art.3,
            prix_pratique: art.3,
            taux_tva: tva,
            a_decouvert: None,
        });
    }
    let v = argent::creer_vente_datee_sur_base(
        base,
        client.to_string(),
        depot,
        if paye.is_some() { "comptant" } else { "credit" }.into(),
        lignes,
        Some("patron".into()),
        paye,
        Some("especes".into()),
        None,
        date,
    )
    .expect("vente");
    let vente_id = v["vente_id"].as_str().unwrap().to_string();
    let dossier = base.dossier().to_string();
    let ids = ids_articles
        .into_iter()
        .map(|art| {
            base.lire_une(
                "SELECT id FROM ligne_vente WHERE vente_id = ?1 AND article_id = ?2 AND dossier_id = ?3",
                &parametres![vente_id.clone(), art, dossier.clone()],
                |r| r.get::<String>(0),
            )
            .unwrap()
            .expect("ligne de vente")
        })
        .collect();
    (vente_id, ids)
}

fn rendre(base: &mut Base, vente: &str, ligne: &str, quantite: f64) {
    retours::enregistrer_retour_sur_base(
        base, vente.into(), ligne.into(), quantite, "remboursement".into(),
        Some("especes".into()), None, None, None, None, None,
    )
    .expect("retour");
}

fn aujourd_hui() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

fn hier() -> String {
    (chrono::Local::now().date_naive() - chrono::Duration::days(1)).format("%Y-%m-%d").to_string()
}

/// Le dernier jour du mois précédent — antidatable (31 jours au plus).
fn dernier_jour_du_mois_dernier() -> String {
    let premier = chrono::NaiveDate::parse_from_str(
        &chrono::Local::now().format("%Y-%m-01").to_string(),
        "%Y-%m-%d",
    )
    .unwrap();
    (premier - chrono::Duration::days(1)).format("%Y-%m-%d").to_string()
}

/// Le rapport TVA du `du` au `au` (jours), comme l'écran le demande.
fn rapport_tva(base: &mut Base, du: &str, au: &str) -> serde_json::Value {
    rapports::lire_rapport_tva_sur_base(base, format!("{du}T00:00:00"), format!("{au}T23:59:59")).unwrap()
}

fn resume_tva(base: &mut Base, du: &str, au: &str) -> serde_json::Value {
    chantiers::lire_resume_tva_sur_base(base, format!("{du}T00:00:00"), format!("{au}T23:59:59")).unwrap()
}

/// La TVA extraite d'un montant TTC au taux de 18 %, comme à la vente.
fn tva_de(ttc: i64) -> i64 {
    (ttc as f64 * TVA / (1.0 + TVA)).round() as i64
}

fn sorties(base: &mut Base, motif: &str) -> i64 {
    compter(
        base,
        "SELECT CAST(COALESCE(SUM(montant), 0) AS BIGINT) FROM mouvement_caisse
         WHERE sens = 'sortie' AND motif = ?1",
        &parametres![motif],
    )
}

fn entrees(base: &mut Base) -> i64 {
    compter(
        base,
        "SELECT CAST(COALESCE(SUM(montant), 0) AS BIGINT) FROM mouvement_caisse WHERE sens = 'entree'",
        &[],
    )
}

/// Le reste dû d'une vente, tel que la liste des créances le dit.
fn reste_du(base: &mut Base, vente: &str) -> i64 {
    creances::lire_creances_ouvertes_sur_base(base, None)
        .unwrap()
        .into_iter()
        .find(|c| c["vente_id"] == vente)
        .map(|c| c["reste"].as_i64().unwrap())
        .unwrap_or(0)
}

// =====================================================================
//  La TVA, nette des retours à leur date
// =====================================================================

#[test]
fn la_tva_rendue_se_deduit_a_la_date_du_retour_pas_a_celle_de_la_vente() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let sucre = article_unite(&mut base, "Sucre");
    let depot = depot_defaut(&mut base);
    let client = client_reel(&mut base);
    let stock_avant = stock(&mut base, &sucre.0, &depot);
    assert_eq!(sucre.3, SUCRE, "prix de la démo");

    // Le mois dernier, à crédit : 3 kg de sucre à 18 %, 2 400 F TTC.
    let veille = dernier_jour_du_mois_dernier();
    let debut_dernier = format!("{}-01", &veille[..7]);
    let (vente, lignes) = vendre(&mut base, &client, &[("Sucre", 3.0)], Some(TVA), None, Some(veille.clone()));
    let tva_vente = tva_de(3 * SUCRE);
    let declare = rapport_tva(&mut base, &debut_dernier, &veille);
    assert_eq!((declare["total_tva"].as_i64(), declare["par_taux"][0]["total_ht"].as_i64()),
               (Some(tva_vente), Some(3 * SUCRE - tva_vente)));

    // Aujourd'hui, un kilo revient : il éteint 800 F de dette.
    rendre(&mut base, &vente, &lignes[0], 1.0);
    let tva_rendue = (tva_vente as f64 / 3.0).round() as i64;

    // La période déjà déclarée ne bouge pas.
    let declare_apres = rapport_tva(&mut base, &debut_dernier, &veille);
    assert_eq!(declare_apres, declare, "une déclaration faite n'est pas réécrite");

    // Le mois du retour porte la TVA rendue, en moins, au taux de la vente.
    let debut_mois = chrono::Local::now().format("%Y-%m-01").to_string();
    let ce_mois = rapport_tva(&mut base, &debut_mois, &aujourd_hui());
    let t = &ce_mois["par_taux"][0];
    assert_eq!(ce_mois["par_taux"].as_array().unwrap().len(), 1);
    assert!((t["taux"].as_f64().unwrap() - TVA).abs() < 1e-6);
    assert_eq!(t["total_tva"], -tva_rendue);
    assert_eq!(t["total_ht"], -(SUCRE - tva_rendue));
    assert_eq!((t["tva_retours"].as_i64(), t["nb_retours"].as_i64(), t["nb_ventes"].as_i64()),
               (Some(tva_rendue), Some(1), Some(0)));
    assert_eq!(ce_mois["total_tva"], -tva_rendue);
    assert_eq!(resume_tva(&mut base, &debut_mois, &aujourd_hui())["total_tva"], -tva_rendue,
               "l'onglet Chantiers dit la même chose");

    // Le reste : CA, créance, stock, caisse.
    assert_eq!(tableau_bord::lire_resume_dashboard_sur(&mut base, None).unwrap()["ca_mois"], -SUCRE);
    assert_eq!(reste_du(&mut base, &vente), 2 * SUCRE);
    assert_eq!(stock(&mut base, &sucre.0, &depot), stock_avant - 2.0);
    assert_eq!(sorties(&mut base, "remboursement"), 0, "la dette éteinte ne sort pas d'argent");
}

#[test]
fn trois_retours_d_un_tiers_rendent_toute_la_tva_de_la_ligne() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let sucre = article_unite(&mut base, "Sucre");
    let depot = depot_defaut(&mut base);
    let passant = client_generique(&mut base);
    let stock_avant = stock(&mut base, &sucre.0, &depot);
    let jour = aujourd_hui();

    let (vente, lignes) = vendre(&mut base, &passant, &[("Sucre", 3.0)], Some(TVA), Some(3 * SUCRE), None);
    for _ in 0..3 {
        rendre(&mut base, &vente, &lignes[0], 1.0);
    }

    let r = rapport_tva(&mut base, &jour, &jour);
    let t = &r["par_taux"][0];
    assert_eq!(t["tva_retours"], tva_de(3 * SUCRE), "ni 1 F de trop ni 1 F de moins");
    assert_eq!((t["total_tva"].as_i64(), t["total_ht"].as_i64()), (Some(0), Some(0)));
    assert_eq!((t["nb_ventes"].as_i64(), t["nb_retours"].as_i64()), (Some(1), Some(3)));

    // Le cahier du jour : tout est revenu, le chiffre net est nul.
    let j = journal::lire_journal_du_jour_sur_base(&mut base, None, None).unwrap();
    let tot = &j["totaux"];
    assert_eq!((tot["ventes_jour"].as_i64(), tot["retours_jour"].as_i64()), (Some(3 * SUCRE), Some(3 * SUCRE)));
    assert_eq!((tot["ca_jour"].as_i64(), tot["ca_jour_ht"].as_i64()), (Some(0), Some(0)));
    assert_eq!(tot["encaisse_jour"], 3 * SUCRE, "l'argent est bien entré, puis ressorti");

    assert_eq!(stock(&mut base, &sucre.0, &depot), stock_avant);
    assert_eq!(sorties(&mut base, "remboursement"), 3 * SUCRE);
    assert_eq!(entrees(&mut base), 3 * SUCRE);
    assert_eq!(reste_du(&mut base, &vente), 0);
}

#[test]
fn l_echange_ne_sort_pas_de_la_tva() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let sucre = article_unite(&mut base, "Sucre");
    let riz = article_unite(&mut base, "Riz local");
    let depot = depot_defaut(&mut base);
    let passant = client_generique(&mut base);
    let (s0, r0) = (stock(&mut base, &sucre.0, &depot), stock(&mut base, &riz.0, &depot));
    let jour = aujourd_hui();

    let (vente, lignes) = vendre(&mut base, &passant, &[("Sucre", 1.0)], Some(TVA), Some(SUCRE), None);
    retours::enregistrer_retour_sur_base(
        &mut base, vente, lignes[0].clone(), 1.0, "echange".into(), None,
        Some(riz.0.clone()), Some(riz.1.clone()), Some(1.0), Some("remboursement".into()), Some("especes".into()),
    )
    .expect("échange");

    let r = rapport_tva(&mut base, &jour, &jour);
    assert_eq!(r["total_tva"], tva_de(SUCRE), "le remplacement est couvert par la vente d'origine");
    assert_eq!(r["tva_retours"], 0);
    let j = journal::lire_journal_du_jour_sur_base(&mut base, None, None).unwrap();
    assert_eq!((j["totaux"]["ca_jour"].as_i64(), j["totaux"]["retours_jour"].as_i64()), (Some(SUCRE), Some(0)));
    assert_eq!(stock(&mut base, &sucre.0, &depot), s0);
    assert_eq!(stock(&mut base, &riz.0, &depot), r0 - 1.0);
    assert_eq!(sorties(&mut base, "remboursement_reliquat"), SUCRE - RIZ);
}

// =====================================================================
//  Le cahier du jour et le CA par magasin
// =====================================================================

#[test]
fn le_cahier_du_jour_et_le_ca_par_magasin_sont_nets_des_retours() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let sucre = article_unite(&mut base, "Sucre");
    let depot = depot_defaut(&mut base);
    let client = client_reel(&mut base);
    let passant = client_generique(&mut base);
    let stock_avant = stock(&mut base, &sucre.0, &depot);

    // Hier, à crédit : 3 kg (2 400 F). Aujourd'hui, un kilo revient
    // (800 F de dette éteinte), et un passant achète du riz au comptant.
    let (v1, l1) = vendre(&mut base, &client, &[("Sucre", 3.0)], None, None, Some(hier()));
    rendre(&mut base, &v1, &l1[0], 1.0);
    vendre(&mut base, &passant, &[("Riz local", 1.0)], None, Some(RIZ), None);

    let j = journal::lire_journal_du_jour_sur_base(&mut base, None, None).unwrap();
    let t = &j["totaux"];
    assert_eq!(t["ventes_jour"], RIZ, "le détail des ventes garde son total");
    assert_eq!(t["retours_jour"], SUCRE);
    assert_eq!((t["ca_jour"].as_i64(), t["ca_jour_ht"].as_i64()), (Some(RIZ - SUCRE), Some(RIZ - SUCRE)),
               "sans TVA, le HT est le TTC");
    assert_eq!(t["encaisse_jour"], RIZ, "la dette éteinte n'est pas un encaissement");
    assert_eq!(t["hors_jour"], 0);
    assert!(j["hors_jour"].as_array().unwrap().is_empty(), "ni un acompte, ni un solde");
    let impaye = j["impayes"].as_array().unwrap().iter().find(|i| i["total"] == 3 * SUCRE).expect("impayé").clone();
    assert_eq!((impaye["rendu"].as_i64(), impaye["paye"].as_i64(), impaye["reste"].as_i64()),
               (Some(SUCRE), Some(0), Some(2 * SUCRE)));

    let magasins = depots::lire_resume_par_depot_sur_base(&mut base, Some(hier()), Some(aujourd_hui())).unwrap();
    let m = magasins.iter().find(|m| m["depot_id"] == depot.as_str()).expect("magasin");
    assert_eq!(m["ca"], 3 * SUCRE + RIZ - SUCRE);
    assert_eq!(m["retours"], SUCRE);
    assert_eq!(m["encaisse"], RIZ);
    assert_eq!(m["impaye"], 2 * SUCRE);

    // Une boutique sans TVA (taux 0) : le retour ne fait apparaître ni
    // ligne « TVA 0 F » ni TVA rendue — seul le CA net des retours se voit.
    for r in [rapport_tva(&mut base, &hier(), &aujourd_hui()), resume_tva(&mut base, &hier(), &aujourd_hui())] {
        assert!(r["par_taux"].as_array().unwrap().is_empty(), "rien à 0 % : {r}");
        assert_eq!((r["total_tva"].as_i64(), r["tva_retours"].as_i64()), (Some(0), Some(0)));
    }

    assert_eq!(stock(&mut base, &sucre.0, &depot), stock_avant - 2.0);
    assert_eq!((entrees(&mut base), sorties(&mut base, "remboursement")), (RIZ, 0));
    assert_eq!(reste_du(&mut base, &v1), 2 * SUCRE);
}

// =====================================================================
//  Le relevé client et l'onglet Règlements
// =====================================================================

#[test]
fn le_releve_met_la_dette_eteinte_dans_rendu_et_sa_ligne_ne_s_annule_pas() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let sucre = article_unite(&mut base, "Sucre");
    let depot = depot_defaut(&mut base);
    let client = client_reel(&mut base);
    let stock_avant = stock(&mut base, &sucre.0, &depot);

    let (vente, lignes) = vendre(&mut base, &client, &[("Sucre", 3.0)], None, None, None);
    creances::regler_creance_sur_base(&mut base, vente.clone(), 500, "especes".into(), None).expect("acompte");
    rendre(&mut base, &vente, &lignes[0], 1.0);

    let releve = creances::lire_etat_creances_client_sur_base(&mut base, client.clone()).unwrap();
    let l = releve["lignes"].as_array().unwrap().iter().find(|l| l["total"] == 3 * SUCRE).expect("facture").clone();
    assert_eq!((l["rendu"].as_i64(), l["paye"].as_i64(), l["reste"].as_i64()),
               (Some(SUCRE), Some(500), Some(3 * SUCRE - SUCRE - 500)),
               "Réglé = l'argent reçu ; total − rendu − réglé = reste");

    // La ligne « retour » de l'onglet Règlements : ni annulation, ni reçu.
    let reglements = creances::lire_reglements_client_sur_base(&mut base, client.clone()).unwrap();
    let eteinte = reglements.iter().find(|r| r["mode"] == "retour").expect("ligne retour")["id"].as_str().unwrap().to_string();
    let especes = reglements.iter().find(|r| r["mode"] == "especes").expect("acompte")["id"].as_str().unwrap().to_string();
    let paiements_avant = compter(&mut base, "SELECT COUNT(*) FROM paiement", &[]);
    for remboursement in [false, true] {
        let refus = creances::annuler_reglement_sur_base(&mut base, eteinte.clone(), "erreur".into(), remboursement, None)
            .expect_err("une dette éteinte par un retour ne s'annule pas");
        assert!(refus.contains("retour"), "{refus}");
    }
    assert_eq!(compter(&mut base, "SELECT COUNT(*) FROM paiement", &[]), paiements_avant, "rien d'écrit");
    let refus = creances::lire_donnees_recu_sur_base(&mut base, eteinte, "client".into()).expect_err("pas de reçu");
    assert!(refus.contains("reçu"), "{refus}");
    creances::lire_donnees_recu_sur_base(&mut base, especes, "client".into()).expect("le reçu de l'acompte");

    assert_eq!(stock(&mut base, &sucre.0, &depot), stock_avant - 2.0);
    assert_eq!((entrees(&mut base), sorties(&mut base, "remboursement")), (500, 0));
    assert_eq!(reste_du(&mut base, &vente), 3 * SUCRE - SUCRE - 500);
}

// =====================================================================
//  Le détecteur
// =====================================================================

#[test]
fn tout_ce_que_touche_d53_passe_le_detecteur() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let client = client_reel(&mut base);
    let depot = depot_defaut(&mut base);
    let jour = aujourd_hui();
    base.auditer(true);

    let (v1, l1) = vendre(&mut base, &client, &[("Sucre", 3.0), ("Riz local", 1.0)], Some(TVA), None, None);
    creances::regler_creance_sur_base(&mut base, v1.clone(), 500, "especes".into(), None).expect("acompte");
    rendre(&mut base, &v1, &l1[0], 1.0);
    rendre(&mut base, &v1, &l1[1], 1.0);

    let r = rapport_tva(&mut base, &jour, &jour);
    assert_eq!(r["tva_retours"], tva_de(3 * SUCRE) - tva_de(2 * SUCRE) + tva_de(RIZ));
    resume_tva(&mut base, &jour, &jour);
    let j = journal::lire_journal_du_jour_sur_base(&mut base, None, Some(depot.clone())).unwrap();
    assert_eq!(j["totaux"]["ca_jour"], 3 * SUCRE + RIZ - SUCRE - RIZ);
    depots::lire_resume_par_depot_sur_base(&mut base, None, None).unwrap();
    creances::lire_etat_creances_client_sur_base(&mut base, client.clone()).unwrap();
    let reglements = creances::lire_reglements_client_sur_base(&mut base, client).unwrap();
    let eteinte = reglements.iter().find(|r| r["mode"] == "retour").unwrap()["id"].as_str().unwrap().to_string();
    assert!(creances::annuler_reglement_sur_base(&mut base, eteinte.clone(), "x".into(), true, None).is_err());
    assert!(creances::lire_donnees_recu_sur_base(&mut base, eteinte, "client".into()).is_err());
}
