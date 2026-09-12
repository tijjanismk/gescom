//! `retours::*_sur_base` — les retours client, sur `Base`.
//!
//! Trois modes, deux gardes, et de l'argent qui sort du tiroir. Chaque
//! scénario vérifie le stock, la caisse et la créance APRÈS le retour.
//! Testé sur SQLite par défaut, sur PostgreSQL avec `GESCOM_PG`.

mod commun;

use commun::*;
use gescom_noyau::argent::{self, ParamsLigneInput};
use gescom_noyau::base::Base;
use gescom_noyau::parametres;
use gescom_noyau::retours;

fn ligne_vente(art: &(String, String, f64, i64), depot: &str, quantite: f64) -> ParamsLigneInput {
    ParamsLigneInput {
        article_id: art.0.clone(),
        unite_vente_id: art.1.clone(),
        depot_source_id: depot.to_string(),
        source_approvisionnement: "stock".into(),
        quantite,
        facteur: art.2,
        prix_reference: art.3,
        prix_pratique: art.3,
        taux_tva: None,
        a_decouvert: None,
    }
}

/// Une vente de `quantite` kg de sucre, comptant si `paye`, sinon à
/// crédit à un vrai client. Rend (vente_id, ligne_vente_id, prix).
fn vendre(base: &mut Base, quantite: f64, paye: bool) -> (String, String, i64) {
    let depot = depot_defaut(base);
    let sucre = article_unite(base, "Sucre");
    let client = if paye { client_generique(base) } else { client_reel(base) };
    let montant = (sucre.3 as f64 * quantite).round() as i64;
    let v = argent::creer_vente_sur_base(
        base,
        client,
        depot.clone(),
        if paye { "comptant" } else { "credit" }.into(),
        vec![ligne_vente(&sucre, &depot, quantite)],
        None,
        if paye { Some(montant) } else { None },
        None,
        None,
    )
    .expect("vente");
    let vente_id = v["vente_id"].as_str().unwrap().to_string();
    let ligne_id: String = base
        .lire_une(
            "SELECT id FROM ligne_vente WHERE vente_id = ?1",
            &parametres![vente_id.clone()],
            |r| r.get::<String>(0),
        )
        .unwrap()
        .unwrap();
    (vente_id, ligne_id, sucre.3)
}

fn sorties(base: &mut Base, motif: &str) -> i64 {
    compter(
        base,
        "SELECT CAST(COALESCE(SUM(montant), 0) AS BIGINT) FROM mouvement_caisse
         WHERE sens = 'sortie' AND motif = ?1",
        &parametres![motif],
    )
}

#[test]
fn les_ventes_recentes_portent_leurs_lignes() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let (vente_id, _, prix) = vendre(&mut base, 2.0, true);
    let ventes = retours::lire_ventes_recentes_sur_base(&mut base).unwrap();
    assert_eq!(ventes.len(), 1);
    assert_eq!(ventes[0]["id"], vente_id);
    assert_eq!(ventes[0]["total"], 2 * prix);
    assert_eq!(ventes[0]["lignes"][0]["article_nom"], "Sucre");
    assert_eq!(ventes[0]["lignes"][0]["montant"], 2 * prix);
}

#[test]
fn un_remboursement_rend_le_stock_et_l_argent() {
    let mut base = base_avec_demo();
    let sucre = article_unite(&mut base, "Sucre");
    let depot = depot_defaut(&mut base);
    ouvrir_caisse(&mut base);
    let avant = stock(&mut base, &sucre.0, &depot);
    let (vente_id, ligne_id, prix) = vendre(&mut base, 3.0, true);
    assert_eq!(stock(&mut base, &sucre.0, &depot), avant - 3.0);

    let r = retours::enregistrer_retour_sur_base(
        &mut base, vente_id.clone(), ligne_id.clone(), 1.0, "remboursement".into(),
        Some("especes".into()), None, None, None, None, None,
    )
    .unwrap();
    assert_eq!(r["montant_credit"], prix);
    assert_eq!(r["part_creance"], 0, "vente soldée : rien à imputer sur une dette");
    assert_eq!(r["part_client"], prix);
    assert_eq!(stock(&mut base, &sucre.0, &depot), avant - 2.0, "un kilo revient");
    assert_eq!(sorties(&mut base, "remboursement"), prix);

    // Pas plus que vendu.
    let refus = retours::enregistrer_retour_sur_base(
        &mut base, vente_id.clone(), ligne_id.clone(), 2.5, "remboursement".into(),
        None, None, None, None, None, None,
    )
    .unwrap_err();
    assert!(refus.contains("Quantité trop élevée"), "{refus}");
    let refus = retours::enregistrer_retour_sur_base(
        &mut base, vente_id, ligne_id, 0.0, "remboursement".into(), None, None, None, None, None, None,
    )
    .unwrap_err();
    assert!(refus.contains("positive"));
}

#[test]
fn le_retour_eteint_d_abord_la_dette() {
    let mut base = base_avec_demo();
    let (vente_id, ligne_id, prix) = vendre(&mut base, 3.0, false);

    // Rien n'a été versé : le retour s'impute sur la créance, la
    // caisse ne bouge pas — même sans caisse ouverte (D33).
    let r = retours::enregistrer_retour_sur_base(
        &mut base, vente_id.clone(), ligne_id, 2.0, "remboursement".into(),
        None, None, None, None, None, None,
    )
    .unwrap();
    assert_eq!(r["part_creance"], 2 * prix);
    assert_eq!(r["part_client"], 0);
    assert_eq!(sorties(&mut base, "remboursement"), 0);

    let (statut, paye): (String, i64) = base
        .lire_une(
            "SELECT v.statut,
                    (SELECT CAST(COALESCE(SUM(montant), 0) AS BIGINT) FROM paiement WHERE vente_id = v.id AND mode = 'retour')
             FROM vente v WHERE v.id = ?1",
            &parametres![vente_id],
            |r| Ok((r.get::<String>(0)?, r.get::<i64>(1)?)),
        )
        .unwrap()
        .unwrap();
    assert_eq!(paye, 2 * prix, "un paiement de mode 'retour' porte l'imputation");
    assert_eq!(statut, "partiellement_payee");
}

#[test]
fn un_avoir_conserve_cree_sa_piece_et_refuse_le_client_de_passage() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);

    // Client réel, payé à crédit puis réglé : ici on simule un
    // versement réel via une vente comptant... au client réel.
    let depot = depot_defaut(&mut base);
    let sucre = article_unite(&mut base, "Sucre");
    let client = client_reel(&mut base);
    let v = argent::creer_vente_sur_base(
        &mut base, client.clone(), depot.clone(), "comptant".into(),
        vec![ligne_vente(&sucre, &depot, 2.0)], None, Some(2 * sucre.3), None, None,
    )
    .unwrap();
    let vente_id = v["vente_id"].as_str().unwrap().to_string();
    let ligne_id: String = base
        .lire_une("SELECT id FROM ligne_vente WHERE vente_id = ?1", &parametres![vente_id.clone()], |r| r.get::<String>(0))
        .unwrap()
        .unwrap();

    let r = retours::enregistrer_retour_sur_base(
        &mut base, vente_id, ligne_id, 1.0, "avoir_conserve".into(), None, None, None, None, None, None,
    )
    .unwrap();
    assert!(r["numero_avoir"].as_str().unwrap().starts_with("AVC-"));
    let avoirs = retours::lire_avoirs_ouverts_tous_sur_base(&mut base).unwrap();
    assert_eq!(avoirs.len(), 1);
    assert_eq!(avoirs[0]["montant"], sucre.3);
    // L'avoir est relié à sa pièce (D44).
    let lie = compter(&mut base, "SELECT COUNT(*) FROM avoir WHERE piece_id IS NOT NULL AND client_id = ?1", &parametres![client]);
    assert_eq!(lie, 1);

    // Le client de passage n'a pas d'avoir (D40).
    let (vente_id, ligne_id, _) = vendre(&mut base, 1.0, true);
    let refus = retours::enregistrer_retour_sur_base(
        &mut base, vente_id, ligne_id, 1.0, "avoir_conserve".into(), None, None, None, None, None, None,
    )
    .unwrap_err();
    assert!(refus.contains("client comptant"), "{refus}");
}

#[test]
fn un_echange_sort_le_remplacement_du_depot_de_la_vente() {
    let mut base = base_avec_demo();
    let sucre = article_unite(&mut base, "Sucre");
    let riz = article_unite(&mut base, "Riz local");
    let depot = depot_defaut(&mut base);
    ouvrir_caisse(&mut base);
    let sucre_avant = stock(&mut base, &sucre.0, &depot);
    let riz_avant = stock(&mut base, &riz.0, &depot);
    let (vente_id, ligne_id, prix_sucre) = vendre(&mut base, 1.0, true);

    // Un kilo de sucre (800) contre un kilo de riz (600) : reliquat
    // positif, rendu en espèces au client de passage.
    let r = retours::enregistrer_retour_sur_base(
        &mut base, vente_id, ligne_id, 1.0, "echange".into(), None,
        Some(riz.0.clone()), Some(riz.1.clone()), Some(1.0), None, None,
    )
    .unwrap();
    assert_eq!(r["montant_credit"], prix_sucre);
    assert_eq!(stock(&mut base, &sucre.0, &depot), sucre_avant, "le sucre revient");
    assert_eq!(stock(&mut base, &riz.0, &depot), riz_avant - 1.0, "le riz part");
    assert_eq!(sorties(&mut base, "remboursement_reliquat"), prix_sucre - riz.3);
}

#[test]
fn un_echange_plus_cher_fait_payer_la_difference() {
    let mut base = base_avec_demo();
    let sucre = article_unite(&mut base, "Sucre");
    ouvrir_caisse(&mut base);
    let (vente_id, ligne_id, prix_sucre) = vendre(&mut base, 1.0, true);

    // Un kilo de sucre contre deux kilos de sucre : le client complète.
    retours::enregistrer_retour_sur_base(
        &mut base, vente_id, ligne_id, 1.0, "echange".into(), Some("especes".into()),
        Some(sucre.0.clone()), Some(sucre.1.clone()), Some(2.0), None, None,
    )
    .unwrap();
    let complement = compter(
        &mut base,
        "SELECT CAST(COALESCE(SUM(montant), 0) AS BIGINT) FROM mouvement_caisse
         WHERE sens = 'entree' AND motif = 'complement_echange'",
        &[],
    );
    assert_eq!(complement, prix_sucre);
}

#[test]
fn caisse_fermee_le_remboursement_ne_laisse_rien_derriere_lui() {
    let mut base = base_avec_demo();
    let sucre = article_unite(&mut base, "Sucre");
    let depot = depot_defaut(&mut base);
    let sid = ouvrir_caisse(&mut base);
    let (vente_id, ligne_id, _) = vendre(&mut base, 2.0, true);
    gescom_noyau::caisse::fermer_session_caisse_sur(&mut base, sid, 0).unwrap();
    let avant = stock(&mut base, &sucre.0, &depot);

    let refus = retours::enregistrer_retour_sur_base(
        &mut base, vente_id.clone(), ligne_id, 1.0, "remboursement".into(), None, None, None, None, None, None,
    )
    .unwrap_err();
    assert!(refus.contains("CAISSE_FERMEE"), "{refus}");
    assert_eq!(stock(&mut base, &sucre.0, &depot), avant, "le stock n'est pas remonté");
    assert_eq!(compter(&mut base, "SELECT COUNT(*) FROM retour WHERE vente_id = ?1", &parametres![vente_id]), 0, "aucune ligne de retour posée");
}

#[test]
fn deux_dossiers_ne_voient_pas_les_retours_l_un_de_l_autre() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let (vente_id, ligne_id, _) = vendre(&mut base, 1.0, true);

    base.choisir_dossier("dossier-b").unwrap();
    assert!(retours::lire_ventes_recentes_sur_base(&mut base).unwrap().is_empty());
    assert!(retours::lire_avoirs_ouverts_tous_sur_base(&mut base).unwrap().is_empty());
    let refus = retours::enregistrer_retour_sur_base(
        &mut base, vente_id, ligne_id, 1.0, "remboursement".into(), None, None, None, None, None, None,
    )
    .unwrap_err();
    assert!(refus.contains("introuvable"), "on ne retourne pas la vente d'un autre dossier : {refus}");
}

#[test]
fn tout_ce_qui_est_porte_dans_retours_passe_le_detecteur() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let riz = article_unite(&mut base, "Riz local");
    let (vente_id, ligne_id, _) = vendre(&mut base, 3.0, true);
    base.auditer(true);

    retours::lire_ventes_recentes_sur_base(&mut base).expect("ventes récentes");
    retours::enregistrer_retour_sur_base(
        &mut base, vente_id.clone(), ligne_id.clone(), 1.0, "remboursement".into(), None, None, None, None, None, None,
    )
    .expect("remboursement");
    retours::enregistrer_retour_sur_base(
        &mut base, vente_id, ligne_id, 1.0, "echange".into(), None,
        Some(riz.0.clone()), Some(riz.1.clone()), Some(1.0), Some("remboursement".into()), None,
    )
    .expect("échange");
    retours::lire_avoirs_ouverts_tous_sur_base(&mut base).expect("avoirs");
}
