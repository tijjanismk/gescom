//! `journal`, `rapports`, `relances`, `cheques`, `transferts` — sur
//! `Base`. Testé sur SQLite par défaut, sur PostgreSQL avec `GESCOM_PG`.

mod commun;

use commun::*;
use gescom_noyau::argent::{self, ParamsLigneInput};
use gescom_noyau::base::Base;
use gescom_noyau::parametres;
use gescom_noyau::{achats, cheques, journal, rapports, relances, transferts};

fn vendre(base: &mut Base, quantite: f64, paye: bool) -> (String, i64) {
    let depot = depot_defaut(base);
    let sucre = article_unite(base, "Sucre");
    let client = if paye { client_generique(base) } else { client_reel(base) };
    let montant = (sucre.3 as f64 * quantite).round() as i64;
    let v = argent::creer_vente_sur_base(
        base,
        client,
        depot.clone(),
        if paye { "comptant" } else { "credit" }.into(),
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
        if paye { Some(montant) } else { None },
        None,
        None,
    )
    .expect("vente");
    (v["vente_id"].as_str().unwrap().to_string(), montant)
}

fn aujourd_hui() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

#[test]
fn le_journal_du_jour_separe_ventes_encaissements_et_achats() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let (_, comptant) = vendre(&mut base, 2.0, true);
    let (_, credit) = vendre(&mut base, 3.0, false);
    let f = fournisseur(&mut base, "Grossiste");
    let sucre = article_unite(&mut base, "Sucre");
    achats::enregistrer_achat_sur_base(
        &mut base, Some(f), None,
        vec![achats::LigneAchat { article_id: sucre.0.clone(), unite_vente_id: sucre.1.clone(), quantite: 10.0, facteur: 1.0, prix_achat: 500 }],
        Some("comptant".into()), None, None, None, None, None,
    )
    .unwrap();

    let j = journal::lire_journal_du_jour_sur_base(&mut base, None, None).unwrap();
    assert_eq!(j["date"], aujourd_hui());
    assert_eq!(j["totaux"]["nb_ventes"], 2);
    assert_eq!(j["totaux"]["ca_jour"], comptant + credit, "le CA compte les ventes émises, payées ou non");
    assert_eq!(j["totaux"]["encaisse_jour"], comptant, "l'encaissé ne compte que l'argent reçu");
    assert_eq!(j["totaux"]["impayes"], credit);
    assert_eq!(j["totaux"]["achats"], 5_000);
    assert_eq!(j["totaux"]["reglement_fournisseur"], 5_000);
    assert_eq!(j["achats"][0]["fournisseur"], "Grossiste");
    assert!(j["hors_jour"].as_array().unwrap().is_empty());
    let especes = j["caisse_par_moyen"].as_array().unwrap().iter().find(|m| m["moyen"] == "especes").unwrap();
    assert_eq!(especes["entrees"], comptant);
    assert_eq!(especes["sorties"], 5_000);

    // Un autre jour : rien.
    let vide = journal::lire_journal_du_jour_sur_base(&mut base, Some("2000-01-01".into()), None).unwrap();
    assert_eq!(vide["totaux"]["nb_ventes"], 0);
    // Un magasin inconnu : rien non plus, mais les impayés sont filtrés aussi.
    let ailleurs = journal::lire_journal_du_jour_sur_base(&mut base, None, Some("x".into())).unwrap();
    assert_eq!(ailleurs["totaux"]["nb_ventes"], 0);
    assert_eq!(ailleurs["totaux"]["impayes"], 0);
}

#[test]
fn les_rapports_lisent_les_ventes_de_la_periode() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let (_, comptant) = vendre(&mut base, 2.0, true);
    let (_, credit) = vendre(&mut base, 1.0, false);
    let debut = "2000-01-01".to_string();
    let fin = "2099-12-31".to_string();

    let mensuel = rapports::lire_rapport_ca_mensuel_sur_base(&mut base, Some(3)).unwrap();
    assert_eq!(mensuel.len(), 1);
    assert_eq!(mensuel[0]["mois"], chrono::Local::now().format("%Y-%m").to_string());
    assert_eq!(mensuel[0]["ca"], comptant + credit);
    assert_eq!(mensuel[0]["encaisse"], comptant);
    assert_eq!(mensuel[0]["nb_ventes"], 2);

    let clients = rapports::lire_rapport_top_clients_sur_base(&mut base, debut.clone(), fin.clone(), None).unwrap();
    assert_eq!(clients.len(), 1, "le client de passage n'est pas classé");
    assert_eq!(clients[0]["ca"], credit);
    assert_eq!(clients[0]["creances"], credit);

    let articles = rapports::lire_rapport_top_articles_sur_base(&mut base, debut.clone(), fin.clone(), Some(5)).unwrap();
    assert_eq!(articles.len(), 1);
    assert_eq!(articles[0]["nom"], "Sucre");
    assert_eq!(articles[0]["qte_vendue"], 3.0);
    assert_eq!(articles[0]["nb_ventes"], 2);

    let creances = rapports::lire_rapport_creances_sur_base(&mut base).unwrap();
    assert_eq!(creances["moins_30j"]["nb"], 1);
    assert_eq!(creances["moins_30j"]["montant"], credit);
    assert_eq!(creances["plus_90j"]["nb"], 0);

    let stock = rapports::lire_rapport_stock_sur_base(&mut base).unwrap();
    let sucre = stock.iter().find(|s| s["nom"] == "Sucre").unwrap();
    assert_eq!(sucre["quantite"], 197.0);
    assert_eq!(sucre["statut"], "ok");

    let tva = rapports::lire_rapport_tva_sur_base(&mut base, debut, fin).unwrap();
    assert_eq!(tva["total_tva"], 0, "la démo vend hors taxe");
}

#[test]
fn les_relances_se_posent_et_se_comptent() {
    let mut base = base_avec_demo();
    let (vente_id, credit) = vendre(&mut base, 1.0, false);

    let creances = relances::lire_creances_relances_sur_base(&mut base, None).unwrap();
    assert_eq!(creances.len(), 1);
    assert_eq!(creances[0]["reste"], credit);
    assert_eq!(creances[0]["jours_retard"], 0);
    assert_eq!(creances[0]["nb_relances"], 0);
    assert!(relances::lire_creances_relances_sur_base(&mut base, Some(true)).unwrap().is_empty(), "rien en retard aujourd'hui");

    relances::enregistrer_relance_sur_base(&mut base, vente_id.clone(), "whatsapp".into(), Some("rappel".into())).unwrap();
    let h = relances::lire_historique_relances_sur_base(&mut base, vente_id).unwrap();
    assert_eq!(h.len(), 1);
    assert_eq!(h[0]["canal"], "whatsapp");
    let creances = relances::lire_creances_relances_sur_base(&mut base, None).unwrap();
    assert_eq!(creances[0]["nb_relances"], 1);
    assert!(creances[0]["derniere_relance"].is_string());

    let stats = relances::lire_stats_relances_sur_base(&mut base).unwrap();
    assert_eq!(stats["total_creances"], 1);
    assert_eq!(stats["sans_relance"], 0);
    assert_eq!(stats["relances_semaine"], 1);
    assert_eq!(stats["montant_en_jeu"], credit);
}

#[test]
fn un_cheque_suit_son_cycle_et_un_rejet_rouvre_la_creance() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let (vente_id, montant) = vendre(&mut base, 1.0, true);
    let paiement_id: String = base
        .lire_une("SELECT id FROM paiement WHERE vente_id = ?1", &parametres![vente_id.clone()], |r| r.get::<String>(0))
        .unwrap()
        .unwrap();

    let refus = cheques::enregistrer_cheque_sur_base(&mut base, None, None, "".into(), "BDM".into(), None, 100, None, None).unwrap_err();
    assert!(refus.contains("numéro"));
    let id = cheques::enregistrer_cheque_sur_base(
        &mut base, Some(paiement_id), Some(vente_id.clone()), "  123 ".into(), "BDM".into(), Some("Awa".into()), montant, None, None,
    )
    .unwrap();
    let refus = cheques::enregistrer_cheque_sur_base(&mut base, None, None, "123".into(), "BDM".into(), None, 100, None, None).unwrap_err();
    assert!(refus.contains("déjà enregistré"));

    let l = cheques::lire_cheques_sur_base(&mut base, None).unwrap();
    assert_eq!(l["cheques"].as_array().unwrap().len(), 1);
    assert_eq!(l["cheques"][0]["numero"], "123");
    assert_eq!(l["cheques"][0]["jours_detention"], 0);
    assert_eq!(l["total_en_attente"], montant);
    assert_eq!(l["nb_dormants"], 0);
    assert!(cheques::lire_cheques_sur_base(&mut base, Some("rejete".into())).unwrap()["cheques"].as_array().unwrap().is_empty());

    cheques::changer_statut_cheque_sur_base(&mut base, id.clone(), "depose".into(), None).unwrap();
    let r = cheques::changer_statut_cheque_sur_base(&mut base, id.clone(), "rejete".into(), Some("sans provision".into())).unwrap();
    assert_eq!(r["creance_rouverte"], true);
    let statut: String = base
        .lire_une("SELECT statut FROM vente WHERE id = ?1", &parametres![vente_id.clone()], |r| r.get::<String>(0))
        .unwrap()
        .unwrap();
    assert_eq!(statut, "creance_ouverte", "le paiement a disparu, la créance rouvre (D35)");
    assert_eq!(compter(&mut base, "SELECT COUNT(*) FROM paiement WHERE vente_id = ?1", &parametres![vente_id]), 0);

    let refus = cheques::changer_statut_cheque_sur_base(&mut base, id, "n_importe".into(), None).unwrap_err();
    assert!(refus.contains("inconnu"));
}

#[test]
fn un_transfert_deplace_le_stock_et_refuse_le_decouvert() {
    let mut base = base_avec_demo();
    let sucre = article_unite(&mut base, "Sucre");
    let source = depot_defaut(&mut base);
    let dossier = base.dossier().to_string();
    let dest = uuid::Uuid::new_v4().to_string();
    base.executer(
        "INSERT INTO depot (id, nom, est_defaut, actif, cree_le, modifie_le, dossier_id)
         VALUES (?1, 'Annexe', 0, 1, ?2, ?2, ?3)",
        &parametres![dest.clone(), gescom_noyau::utils::maintenant_iso(), dossier],
    )
    .unwrap();
    let avant = stock(&mut base, &sucre.0, &source);

    let ligne = |q: f64| transferts::LigneTransfert {
        article_id: sucre.0.clone(),
        unite_vente_id: sucre.1.clone(),
        quantite: q,
        facteur: 1.0,
    };
    let refus = transferts::enregistrer_transfert_sur_base(&mut base, source.clone(), source.clone(), vec![ligne(1.0)], None, None).unwrap_err();
    assert!(refus.contains("identiques"));
    let refus = transferts::enregistrer_transfert_sur_base(&mut base, source.clone(), dest.clone(), vec![ligne(avant), ligne(1.0)], None, None).unwrap_err();
    assert!(refus.contains("Stock insuffisant"), "deux lignes se cumulent : {refus}");

    let r = transferts::enregistrer_transfert_sur_base(&mut base, source.clone(), dest.clone(), vec![ligne(5.0), ligne(2.0)], Some("réassort".into()), None).unwrap();
    let bon = r["bon"].as_str().unwrap().to_string();
    assert!(bon.starts_with("BTR-"));
    assert_eq!(r["nb_lignes"], 2);
    assert_eq!(stock(&mut base, &sucre.0, &source), avant - 7.0);
    assert_eq!(stock(&mut base, &sucre.0, &dest), 7.0);

    let liste = transferts::lire_transferts_sur_base(&mut base, None).unwrap();
    assert_eq!(liste.len(), 1);
    assert_eq!(liste[0]["bon"], bon);
    assert_eq!(liste[0]["nb_lignes"], 2);
    assert_eq!(liste[0]["depot_dest"], "Annexe");
    assert_eq!(liste[0]["motif"], "réassort");

    let detail = transferts::lire_bon_transfert_sur_base(&mut base, bon).unwrap();
    assert_eq!(detail["lignes"].as_array().unwrap().len(), 2);
    assert_eq!(detail["depot_dest"], "Annexe");
    assert!(transferts::lire_bon_transfert_sur_base(&mut base, "BTR-0".into()).is_err());

    base.choisir_dossier("dossier-b").unwrap();
    assert!(transferts::lire_transferts_sur_base(&mut base, None).unwrap().is_empty());
}

#[test]
fn tout_ce_qui_est_porte_ici_passe_le_detecteur() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let (vente_id, _) = vendre(&mut base, 1.0, false);
    base.auditer(true);

    journal::lire_journal_du_jour_sur_base(&mut base, None, None).expect("journal");
    rapports::lire_rapport_ca_mensuel_sur_base(&mut base, None).expect("mensuel");
    rapports::lire_rapport_top_clients_sur_base(&mut base, "2000-01-01".into(), "2099-01-01".into(), None).expect("clients");
    rapports::lire_rapport_top_articles_sur_base(&mut base, "2000-01-01".into(), "2099-01-01".into(), None).expect("articles");
    rapports::lire_rapport_creances_sur_base(&mut base).expect("créances");
    rapports::lire_rapport_stock_sur_base(&mut base).expect("stock");
    rapports::lire_rapport_tva_sur_base(&mut base, "2000-01-01".into(), "2099-01-01".into()).expect("tva");
    relances::lire_creances_relances_sur_base(&mut base, Some(true)).expect("relances");
    relances::enregistrer_relance_sur_base(&mut base, vente_id.clone(), "sms".into(), None).expect("relance");
    relances::lire_historique_relances_sur_base(&mut base, vente_id.clone()).expect("historique");
    relances::lire_stats_relances_sur_base(&mut base).expect("stats");
    let ch = cheques::enregistrer_cheque_sur_base(&mut base, None, Some(vente_id), "1".into(), "B".into(), None, 100, None, None).expect("chèque");
    cheques::lire_cheques_sur_base(&mut base, None).expect("chèques");
    cheques::changer_statut_cheque_sur_base(&mut base, ch, "rejete".into(), None).expect("statut chèque");
    transferts::lire_transferts_sur_base(&mut base, Some(10)).expect("transferts");
}
