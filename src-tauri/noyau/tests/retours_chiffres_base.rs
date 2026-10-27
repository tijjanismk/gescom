//! Lot B — des chiffres justes après un retour.
//!
//! - B.2 : le chiffre d'affaires déduit les retours À LA DATE DU RETOUR
//!   (comme un avoir) ; un mois passé ne bouge plus. L'échange ne se
//!   déduit pas (`coeur::calcul::RETOUR_HORS_CA`).
//! - B.3 : une dette éteinte par un retour (paiement `mode = 'retour'`)
//!   n'est pas de l'argent encaissé : relances, créances ouvertes,
//!   facture imprimée, rapport mensuel le montrent à part.
//! - B.4 : un résidu de 1 à 5 F (D41) se solde dans la transaction du
//!   règlement qui le crée — pas un résidu de 6 F.
//!
//! Chaque scénario vérifie, après le geste, le chiffre, la créance, la
//! caisse et le stock. Testé sur SQLite par défaut, sur PostgreSQL avec
//! `GESCOM_PG` (`--test-threads=1`). Le dernier scénario rejoue tout
//! sous le détecteur de requêtes non cloisonnées.

mod commun;

use commun::*;
use gescom_noyau::argent::{self, ParamsLigneInput};
use gescom_noyau::base::Base;
use gescom_noyau::parametres;
use gescom_noyau::{avoirs, chantiers, creances, pieces, rapports, relances, retours, tableau_bord};

// =====================================================================
//  Aides
// =====================================================================

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

/// Une vente de sucre (et de riz si `riz > 0`) au client donné, datée
/// du jour ou de `date`, avec `paye` encaissé. Rend (vente, lignes).
fn vendre(
    base: &mut Base,
    client: &str,
    sucre_kg: f64,
    riz_kg: f64,
    paye: Option<i64>,
    date: Option<String>,
) -> (String, Vec<String>) {
    let depot = depot_defaut(base);
    let sucre = article_unite(base, "Sucre");
    let riz = article_unite(base, "Riz local");
    let mut lignes = vec![ligne_vente(&sucre, &depot, sucre_kg)];
    if riz_kg > 0.0 {
        lignes.push(ligne_vente(&riz, &depot, riz_kg));
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
    // Le sucre d'abord, le riz ensuite : par l'article, pas par la position.
    let dossier = base.dossier().to_string();
    let mut ids = Vec::new();
    for art in [&sucre.0, &riz.0] {
        if let Some(id) = base
            .lire_une(
                "SELECT id FROM ligne_vente WHERE vente_id = ?1 AND article_id = ?2 AND dossier_id = ?3",
                &parametres![vente_id.clone(), art.clone(), dossier.clone()],
                |r| r.get::<String>(0),
            )
            .unwrap()
        {
            ids.push(id);
        }
    }
    (vente_id, ids)
}

fn rendre(base: &mut Base, vente: &str, ligne: &str, quantite: f64) -> serde_json::Value {
    retours::enregistrer_retour_sur_base(
        base, vente.into(), ligne.into(), quantite, "remboursement".into(),
        Some("especes".into()), None, None, None, None, None,
    )
    .expect("retour")
}

fn resume(base: &mut Base) -> serde_json::Value {
    tableau_bord::lire_resume_dashboard_sur(base, None).unwrap()
}

fn mois_courant() -> String {
    chrono::Local::now().format("%Y-%m").to_string()
}

/// Le dernier jour du mois précédent — antidatable (31 jours au plus).
fn hier_du_mois_dernier() -> String {
    let premier = chrono::Local::now().date_naive().format("%Y-%m-01").to_string();
    let premier = chrono::NaiveDate::parse_from_str(&premier, "%Y-%m-%d").unwrap();
    (premier - chrono::Duration::days(1)).format("%Y-%m-%d").to_string()
}

fn ligne_du_mois(base: &mut Base, mois: &str) -> serde_json::Value {
    rapports::lire_rapport_ca_mensuel_sur_base(base, Some(3))
        .unwrap()
        .into_iter()
        .find(|m| m["mois"] == mois)
        .unwrap_or(serde_json::Value::Null)
}

fn statut_vente(base: &mut Base, vente: &str) -> String {
    let dossier = base.dossier().to_string();
    base.lire_une("SELECT statut FROM vente WHERE id = ?1 AND dossier_id = ?2", &parametres![vente, dossier], |r| r.get::<String>(0))
        .unwrap()
        .unwrap()
}

/// Les résidus soldés au journal pour cette vente : (nombre, total).
fn residus_journal(base: &mut Base, vente: &str) -> (i64, i64) {
    let dossier = base.dossier().to_string();
    let valeurs: Vec<String> = base
        .lire_plusieurs(
            "SELECT nouveau_valeur FROM journal
             WHERE type_evenement = 'residu_absorbe' AND entite_id = ?1 AND dossier_id = ?2",
            &parametres![vente, dossier],
            |r| r.get::<String>(0),
        )
        .unwrap();
    let total = valeurs
        .iter()
        .map(|v| serde_json::from_str::<serde_json::Value>(v).unwrap()["residu"].as_i64().unwrap())
        .sum();
    (valeurs.len() as i64, total)
}

fn sorties(base: &mut Base, motif: &str) -> i64 {
    compter(
        base,
        "SELECT CAST(COALESCE(SUM(montant), 0) AS BIGINT) FROM mouvement_caisse
         WHERE sens = 'sortie' AND motif = ?1",
        &parametres![motif],
    )
}

fn relance_de(base: &mut Base, vente: &str) -> Option<serde_json::Value> {
    relances::lire_creances_relances_sur_base(base, None)
        .unwrap()
        .into_iter()
        .find(|c| c["vente_id"] == vente)
}

fn facturer(base: &mut Base, vente_id: &str) -> String {
    let client: String = base
        .lire_une("SELECT client_id FROM vente WHERE id = ?1", &parametres![vente_id], |r| r.get::<String>(0))
        .unwrap()
        .unwrap();
    argent::creer_facture_depuis_vente_sur_base(base, vente_id.to_string(), client, "credit".into(), None)
        .expect("facture de la vente");
    base.lire_une("SELECT piece_id FROM vente WHERE id = ?1", &parametres![vente_id], |r| r.get::<String>(0))
        .unwrap()
        .unwrap()
}

const SUCRE: i64 = 800;
const RIZ: i64 = 600;

// =====================================================================
//  B.2 — le chiffre d'affaires, net des retours
// =====================================================================

#[test]
fn un_retour_se_deduit_du_chiffre_du_jour_et_du_mois() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let sucre = article_unite(&mut base, "Sucre");
    let depot = depot_defaut(&mut base);
    let client = client_reel(&mut base);
    let stock_avant = stock(&mut base, &sucre.0, &depot);
    assert_eq!(sucre.3, SUCRE, "prix de la démo");

    // A credit, 3 kg : 2 400 F, rien encaisse.
    let (vente, lignes) = vendre(&mut base, &client, 3.0, 0.0, None, None);
    let r = resume(&mut base);
    assert_eq!((r["ca_jour"].as_i64(), r["ca_mois"].as_i64()), (Some(2_400), Some(2_400)));

    // Un kilo revient : il eteint 800 F de dette, rien ne sort du tiroir.
    let ret = rendre(&mut base, &vente, &lignes[0], 1.0);
    assert_eq!(ret["part_creance"], 800);
    let r = resume(&mut base);
    assert_eq!(r["ca_jour"], 1_600, "le retour se deduit du jour");
    assert_eq!(r["ca_semaine"], 1_600);
    assert_eq!(r["ca_mois"], 1_600);
    assert_eq!(r["nb_ventes_jour"], 1, "un retour n'enleve pas une vente");
    assert_eq!(r["total_creances"], 1_600, "la creance a baisse d'autant");
    assert_eq!(r["caisse_solde"], 0, "la caisse n'a pas bouge");
    assert_eq!(sorties(&mut base, "remboursement"), 0);
    assert_eq!(stock(&mut base, &sucre.0, &depot), stock_avant - 2.0, "un kilo est revenu");

    let courbe = tableau_bord::lire_ventes_periode_sur(&mut base, Some("jour".into()), None).unwrap();
    assert_eq!(courbe["total"], 1_600, "la courbe du jour aussi");

    // Rapport mensuel : le chiffre net, l'encaisse sans la dette eteinte,
    // et la creance qui reste.
    let m = ligne_du_mois(&mut base, &mois_courant());
    assert_eq!(m["ca"], 1_600);
    assert_eq!(m["retours"], 800);
    assert_eq!(m["encaisse"], 0, "eteindre une dette n'est pas encaisser");
    assert_eq!(m["creances"], 1_600);
    assert_eq!(m["nb_ventes"], 1);

    // Une vente comptant au passant, puis un remboursement en especes.
    let passant = client_generique(&mut base);
    let (v2, l2) = vendre(&mut base, &passant, 2.0, 0.0, Some(1_600), None);
    assert_eq!(resume(&mut base)["caisse_solde"], 1_600);
    let ret = rendre(&mut base, &v2, &l2[0], 1.0);
    assert_eq!((ret["part_creance"].as_i64(), ret["part_especes"].as_i64()), (Some(0), Some(800)));
    let r = resume(&mut base);
    assert_eq!(r["ca_jour"], 1_600 + 1_600 - 800);
    assert_eq!(r["caisse_solde"], 800, "800 F sont ressortis du tiroir");
    assert_eq!(sorties(&mut base, "remboursement"), 800);
    assert_eq!(stock(&mut base, &sucre.0, &depot), stock_avant - 3.0);
    let m = ligne_du_mois(&mut base, &mois_courant());
    assert_eq!(m["ca"], 2_400);
    assert_eq!(m["encaisse"], 1_600, "l'argent recu, rembourse ou non");
    assert_eq!(m["creances"], 1_600, "le passant ne doit rien");
}

#[test]
fn un_retour_sur_une_vente_du_mois_dernier_se_compte_ce_mois_ci() {
    let mut base = base_avec_demo();
    let client = client_reel(&mut base);
    let (vente, lignes) = vendre(&mut base, &client, 3.0, 0.0, None, Some(hier_du_mois_dernier()));
    let mois_dernier = hier_du_mois_dernier()[..7].to_string();

    let r = resume(&mut base);
    assert_eq!((r["ca_mois_precedent"].as_i64(), r["ca_mois"].as_i64()), (Some(2_400), Some(0)));

    rendre(&mut base, &vente, &lignes[0], 1.0);
    let r = resume(&mut base);
    assert_eq!(r["ca_mois_precedent"], 2_400, "le mois clos ne bouge plus");
    assert_eq!(r["ca_mois"], -800, "le retour se compte le jour ou il a lieu");
    assert_eq!(r["ca_jour"], -800);
    assert_eq!(r["total_creances"], 1_600);

    let dernier = ligne_du_mois(&mut base, &mois_dernier);
    assert_eq!((dernier["ca"].as_i64(), dernier["retours"].as_i64()), (Some(2_400), Some(0)));
    assert_eq!(dernier["creances"], 1_600, "la vente du mois dernier doit encore 1 600 F");
    let ce_mois = ligne_du_mois(&mut base, &mois_courant());
    assert_eq!(ce_mois["ca"], -800, "un mois de retours sans vente a sa ligne");
    assert_eq!(ce_mois["nb_ventes"], 0);
}

#[test]
fn un_echange_ne_bouge_pas_le_chiffre() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let sucre = article_unite(&mut base, "Sucre");
    let riz = article_unite(&mut base, "Riz local");
    let depot = depot_defaut(&mut base);
    let passant = client_generique(&mut base);
    let (s0, r0) = (stock(&mut base, &sucre.0, &depot), stock(&mut base, &riz.0, &depot));

    let (vente, lignes) = vendre(&mut base, &passant, 1.0, 0.0, Some(SUCRE), None);
    retours::enregistrer_retour_sur_base(
        &mut base, vente, lignes[0].clone(), 1.0, "echange".into(), None,
        Some(riz.0.clone()), Some(riz.1.clone()), Some(1.0), Some("remboursement".into()), Some("especes".into()),
    )
    .expect("échange");

    let r = resume(&mut base);
    assert_eq!(r["ca_jour"], SUCRE, "l'echange remplace, il ne rend pas la vente");
    assert_eq!(ligne_du_mois(&mut base, &mois_courant())["ca"], SUCRE);
    assert_eq!(sorties(&mut base, "remboursement_reliquat"), SUCRE - RIZ, "la difference rendue");
    assert_eq!(r["caisse_solde"], RIZ);
    assert_eq!(stock(&mut base, &sucre.0, &depot), s0, "le sucre est revenu");
    assert_eq!(stock(&mut base, &riz.0, &depot), r0 - 1.0, "le riz est parti");
}

#[test]
fn les_classements_sont_nets_des_retours() {
    let mut base = base_avec_demo();
    let client = client_reel(&mut base);
    let (vente, lignes) = vendre(&mut base, &client, 3.0, 2.0, None, None);
    rendre(&mut base, &vente, &lignes[0], 2.0);

    let clients = tableau_bord::lire_top_clients_sur(&mut base).unwrap();
    assert_eq!(clients[0]["ca"], 3 * SUCRE + 2 * RIZ - 2 * SUCRE);
    let articles = tableau_bord::lire_top_articles_sur(&mut base).unwrap();
    // Le riz (1 200) passe devant le sucre (2 400 - 1 600 = 800).
    assert_eq!(articles[0]["nom"], "Riz local", "le classement se fait sur le net");
    let s = articles.iter().find(|a| a["nom"] == "Sucre").unwrap();
    assert_eq!((s["ca"].as_i64(), s["qte_vendue"].as_f64()), (Some(800), Some(1.0)));

    let (debut, fin) = ("2000-01-01".to_string(), "2099-12-31".to_string());
    let clients = rapports::lire_rapport_top_clients_sur_base(&mut base, debut.clone(), fin.clone(), None).unwrap();
    assert_eq!(clients[0]["ca"], 2_000);
    assert_eq!(clients[0]["creances"], 2_000, "la creance compte la dette eteinte");
    let articles = rapports::lire_rapport_top_articles_sur_base(&mut base, debut, fin, Some(1)).unwrap();
    assert_eq!(articles.len(), 1, "la limite s'applique apres le classement net");
    assert_eq!(articles[0]["nom"], "Riz local");

    let fiche = pieces::lire_fiche_client_sur_base(&mut base, client).unwrap();
    assert_eq!(fiche["stats"]["ca_total"], 2_000, "la fiche client est nette aussi");
    assert_eq!(fiche["stats"]["encours"], 2_000);
}

// =====================================================================
//  B.3 — une dette éteinte par un retour n'est pas un paiement
// =====================================================================

#[test]
fn la_dette_eteinte_par_un_retour_se_montre_a_part() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let sucre = article_unite(&mut base, "Sucre");
    let client = client_reel(&mut base);

    // Facture de 3 kg (2 400 F) validee a credit avec 1 000 F d'acompte.
    let piece = pieces::creer_piece_sur_base(
        &mut base, client.clone(), "facture".into(), vec![ligne(&sucre, 3.0)], None, None, None, None, None, None,
    )
    .unwrap();
    let piece_id = piece["id"].as_str().unwrap().to_string();
    let v = argent::valider_facture_sur_base(
        &mut base, piece_id.clone(), "credit".into(), Some("especes".into()), Some(1_000), Some("patron".into()),
    )
    .unwrap();
    let vente = v["vente_id"].as_str().unwrap().to_string();
    let ligne_id: String = base
        .lire_une("SELECT id FROM ligne_vente WHERE vente_id = ?1", &parametres![vente.clone()], |r| r.get::<String>(0))
        .unwrap()
        .unwrap();

    // Un kilo revient : 800 F de dette eteinte.
    rendre(&mut base, &vente, &ligne_id, 1.0);

    // Relances : paye = l'argent verse, le retour a part, le reste dû intact.
    let c = relance_de(&mut base, &vente).expect("la créance est relancée");
    assert_eq!(c["total_paye"], 1_800, "total_paye garde la dette eteinte (il fait le reste)");
    assert_eq!(c["encaisse"], 1_000);
    assert_eq!(c["eteint_par_retour"], 800);
    assert_eq!(c["reste"], 600);

    // Les creances ouvertes (ecran Clients) disent la meme chose.
    let o = creances::lire_creances_ouvertes_sur_base(&mut base, None).unwrap();
    let o = o.iter().find(|c| c["vente_id"] == vente).unwrap();
    assert_eq!((o["encaisse"].as_i64(), o["eteint_par_retour"].as_i64(), o["reste"].as_i64()),
        (Some(1_000), Some(800), Some(600)));

    // La facture imprimee : « Acompte recu » 1 000, « Eteint par retour »
    // 800, RESTE DU 600 — le total ne change pas.
    let d = pieces::lire_donnees_piece_sur_base(&mut base, piece_id).unwrap();
    let t = &d["totaux"];
    assert_eq!(t["total_ttc"], 2_400);
    assert_eq!(t["total_paye"], 1_800);
    assert_eq!(t["encaisse"], 1_000);
    assert_eq!(t["eteint_par_retour"], 800);
    assert_eq!(t["reste_du"], 600);

    // La caisse n'a vu que l'acompte ; le stock a recu son kilo.
    assert_eq!(resume(&mut base)["caisse_solde"], 1_000);
    assert_eq!(sorties(&mut base, "remboursement"), 0);
    assert_eq!(resume(&mut base)["total_creances"], 600);
    let m = ligne_du_mois(&mut base, &mois_courant());
    assert_eq!((m["ca"].as_i64(), m["encaisse"].as_i64(), m["creances"].as_i64()),
        (Some(1_600), Some(1_000), Some(600)));
}

// =====================================================================
//  B.4 — le résidu de 1 à 5 F se solde avec le règlement qui le crée
// =====================================================================

#[test]
fn un_reglement_a_cinq_francs_pres_solde_la_vente_a_six_non() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let client = client_reel(&mut base);

    let (vente, _) = vendre(&mut base, &client, 1.0, 0.0, None, None);
    let piece = facturer(&mut base, &vente);
    let r = creances::regler_creance_sur_base(&mut base, vente.clone(), SUCRE - 5, "especes".into(), None).unwrap();
    assert_eq!(r["residu_absorbe"], 5);
    assert_eq!(r["soldee"], true);
    assert_eq!(statut_vente(&mut base, &vente), "payee");
    assert_eq!(statut_piece(&mut base, &piece), "paye");
    assert_eq!(residus_journal(&mut base, &vente), (1, 5), "une trace, une seule");
    assert!(relance_de(&mut base, &vente).is_none(), "plus rien a relancer");
    assert_eq!(resume(&mut base)["caisse_solde"], SUCRE - 5, "aucun paiement fictif");

    let (vente6, _) = vendre(&mut base, &client, 1.0, 0.0, None, None);
    let r = creances::regler_creance_sur_base(&mut base, vente6.clone(), SUCRE - 6, "especes".into(), None).unwrap();
    assert_eq!(r["residu_absorbe"], 0);
    assert_eq!(statut_vente(&mut base, &vente6), "partiellement_payee");
    assert_eq!(residus_journal(&mut base, &vente6), (0, 0));
    assert_eq!(relance_de(&mut base, &vente6).expect("6 F restent dus")["reste"], 6);
    assert_eq!(resume(&mut base)["total_creances"], 6);
}

#[test]
fn chaque_chemin_de_reglement_solde_son_residu() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let sucre = article_unite(&mut base, "Sucre");
    let depot = depot_defaut(&mut base);
    let client = client_reel(&mut base);
    let stock_avant = stock(&mut base, &sucre.0, &depot);

    // 1. La vente du comptoir encaissee a 3 F pres.
    let (v1, _) = vendre(&mut base, &client, 1.0, 0.0, Some(SUCRE - 3), None);
    assert_eq!(statut_vente(&mut base, &v1), "payee");
    assert_eq!(residus_journal(&mut base, &v1), (1, 3));

    // 2. La facture validee avec un acompte a 4 F pres.
    let piece = pieces::creer_piece_sur_base(
        &mut base, client.clone(), "facture".into(), vec![ligne(&sucre, 2.0)], None, None, None, None, None, None,
    )
    .unwrap();
    let piece_id = piece["id"].as_str().unwrap().to_string();
    let v = argent::valider_facture_sur_base(
        &mut base, piece_id.clone(), "credit".into(), Some("especes".into()), Some(2 * SUCRE - 4), Some("patron".into()),
    )
    .unwrap();
    let v2 = v["vente_id"].as_str().unwrap().to_string();
    assert_eq!(statut_vente(&mut base, &v2), "payee");
    assert_eq!(statut_piece(&mut base, &piece_id), "paye");
    assert_eq!(residus_journal(&mut base, &v2), (1, 4));

    // 3. Le retour qui eteint la dette a 3 F pres.
    let (v3, l3) = vendre(&mut base, &client, 2.0, 0.0, None, None);
    creances::regler_creance_sur_base(&mut base, v3.clone(), SUCRE - 3, "especes".into(), None).unwrap();
    assert_eq!(residus_journal(&mut base, &v3), (0, 0), "il reste 803 F, ce n'est pas un residu");
    let ret = rendre(&mut base, &v3, &l3[0], 1.0);
    assert_eq!(ret["part_creance"], SUCRE);
    assert_eq!(statut_vente(&mut base, &v3), "payee");
    assert_eq!(residus_journal(&mut base, &v3), (1, 3));

    // 4. Le retour GROUPE : le residu se juge sur le geste entier.
    let (v4, l4) = vendre(&mut base, &client, 2.0, 1.0, None, None);
    creances::regler_creance_sur_base(&mut base, v4.clone(), SUCRE - 3, "especes".into(), None).unwrap();
    retours::enregistrer_retour_groupe_sur_base(
        &mut base, v4.clone(),
        vec![
            retours::ArticleRendu { ligne_vente_id: l4[0].clone(), quantite: 1.0 },
            retours::ArticleRendu { ligne_vente_id: l4[1].clone(), quantite: 1.0 },
        ],
        "remboursement".into(), Some("especes".into()), vec![], None, None,
    )
    .expect("retour groupé");
    assert_eq!(statut_vente(&mut base, &v4), "payee");
    assert_eq!(residus_journal(&mut base, &v4), (1, 3));

    // 5. L'avoir applique a 2 F pres.
    avoirs::accorder_avoir_client_sur_base(&mut base, client.clone(), SUCRE, "geste commercial".into(), Some("patron".into()))
        .expect("avoir accordé");
    let (v5, _) = vendre(&mut base, &client, 1.0, 0.0, None, None);
    let applique = avoirs::appliquer_avoir_vente_sur_base(&mut base, v5.clone(), client.clone(), SUCRE - 2).unwrap();
    assert_eq!(applique, SUCRE - 2);
    assert_eq!(statut_vente(&mut base, &v5), "payee");
    assert_eq!(residus_journal(&mut base, &v5), (1, 2));

    // 6. Le recouvrement exceptionnel d'une creance irrecouvrable, a 1 F pres.
    let (v6, _) = vendre(&mut base, &client, 1.0, 0.0, None, None);
    chantiers::marquer_irrecouvrable_sur_base(&mut base, v6.clone(), "parti".into()).unwrap();
    creances::regler_creance_exceptionnel_sur_base(&mut base, v6.clone(), SUCRE - 1, "especes".into(), None, None)
        .unwrap();
    assert_eq!(statut_vente(&mut base, &v6), "payee");
    assert_eq!(residus_journal(&mut base, &v6), (1, 1));

    // Rien de tout cela n'a ecrit de paiement fictif : la caisse n'a recu
    // que l'argent verse, et aucune vente n'est plus relancee.
    let attendu = (SUCRE - 3) + (2 * SUCRE - 4) + (SUCRE - 3) + (SUCRE - 3) + (SUCRE - 1);
    assert_eq!(resume(&mut base)["caisse_solde"], attendu);
    assert_eq!(sorties(&mut base, "remboursement"), 0, "chaque retour a eteint une dette");
    assert!(relances::lire_creances_relances_sur_base(&mut base, None).unwrap().is_empty());
    assert_eq!(resume(&mut base)["total_creances"], 0);
    // 1 + 2 + 2 + 2 + 1 + 1 kg vendus, 1 + 1 kg de sucre rendus.
    assert_eq!(stock(&mut base, &sucre.0, &depot), stock_avant - 9.0 + 2.0);
}

// =====================================================================
//  La version fenêtre (`Connection`) — mêmes règles, même base
// =====================================================================

#[test]
fn la_version_fenetre_suit_les_memes_regles() {
    // Une base SQLite sur fichier, amorcée comme celle du serveur, puis
    // rouverte par une `Connection` nue : c'est le chemin de la fenêtre.
    let chemin = std::env::temp_dir().join(format!("gescom_retours_chiffres_{}.db", uuid::Uuid::new_v4()));
    let chemin_txt = chemin.to_string_lossy().to_string();
    {
        let mut base = Base::ouvrir(&chemin_txt).expect("base fichier");
        gescom_noyau::amorcage::amorcer(&mut base).expect("amorçage");
        gescom_noyau::amorcage::donnees_demo(&mut base).expect("démo");
    }
    let resultat = std::panic::catch_unwind(|| {
        let mut conn = rusqlite::Connection::open(&chemin).expect("connexion");
        scenario_fenetre(&mut conn);
    });
    for suffixe in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{chemin_txt}{suffixe}"));
    }
    if let Err(e) = resultat {
        std::panic::resume_unwind(e);
    }
}

fn scenario_fenetre(conn: &mut rusqlite::Connection) {
    let un = |conn: &rusqlite::Connection, sql: &str| -> String { conn.query_row(sql, [], |r| r.get(0)).unwrap() };
    let client = un(conn, "SELECT id FROM client WHERE est_generique = 0 ORDER BY code LIMIT 1");
    let depot = un(conn, "SELECT id FROM depot WHERE est_defaut = 1");
    let (article, unite): (String, String) = conn
        .query_row(
            "SELECT a.id, u.id FROM article a JOIN unite_vente u ON u.article_id = a.id
             WHERE a.nom = 'Sucre' AND u.facteur = 1.0",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    gescom_noyau::caisse::ouvrir_session_caisse(conn, 0, "patron".into()).expect("caisse");

    let v = argent::creer_vente_sur(
        conn, client.clone(), depot.clone(), "credit".into(),
        vec![ligne_vente(&(article, unite, 1.0, SUCRE), &depot, 3.0)],
        Some("patron".into()), None, None, None,
    )
    .expect("vente");
    let vente = v["vente_id"].as_str().unwrap().to_string();
    let ligne_id: String = conn
        .query_row("SELECT id FROM ligne_vente WHERE vente_id = ?1", rusqlite::params![vente], |r| r.get(0))
        .unwrap();
    let f = argent::creer_facture_depuis_vente_sur(conn, vente.clone(), client.clone(), "credit".into(), Some("patron".into()))
        .expect("facture");
    let piece_id = f["piece_id"].as_str().unwrap().to_string();
    creances::regler_creance(conn, vente.clone(), 1_000, "especes".into(), Some("patron".into())).expect("acompte");

    // Un kilo revient : 800 F de dette eteinte.
    retours::enregistrer_retour_sur(
        conn, vente.clone(), ligne_id, 1.0, "remboursement".into(), Some("especes".into()),
        None, None, None, None, None,
    )
    .expect("retour");

    // B.2
    let r = tableau_bord::lire_resume_dashboard(conn, None).unwrap();
    assert_eq!((r["ca_jour"].as_i64(), r["ca_mois"].as_i64()), (Some(1_600), Some(1_600)));
    let courbe = tableau_bord::lire_ventes_periode(conn, Some("jour".into()), None).unwrap();
    assert_eq!(courbe["total"], 1_600);
    assert_eq!(tableau_bord::lire_top_clients(conn).unwrap()[0]["ca"], 1_600);
    let a = &tableau_bord::lire_top_articles(conn).unwrap()[0];
    assert_eq!((a["ca"].as_i64(), a["qte_vendue"].as_f64()), (Some(1_600), Some(2.0)));
    let mensuel = rapports::lire_rapport_ca_mensuel(conn, Some(3)).unwrap();
    let m = mensuel.iter().find(|m| m["mois"] == mois_courant()).unwrap();
    assert_eq!((m["ca"].as_i64(), m["encaisse"].as_i64(), m["creances"].as_i64()),
        (Some(1_600), Some(1_000), Some(600)));
    let (debut, fin) = ("2000-01-01".to_string(), "2099-12-31".to_string());
    assert_eq!(rapports::lire_rapport_top_clients(conn, debut.clone(), fin.clone(), None).unwrap()[0]["ca"], 1_600);
    let a = &rapports::lire_rapport_top_articles(conn, debut, fin, None).unwrap()[0];
    assert_eq!((a["ca"].as_i64(), a["qte_vendue"].as_f64()), (Some(1_600), Some(2.0)));
    assert_eq!(pieces::lire_fiche_client(conn, client.clone()).unwrap()["stats"]["ca_total"], 1_600);

    // B.3
    let c = relances::lire_creances_relances(conn, None).unwrap();
    let c = c.iter().find(|c| c["vente_id"] == vente).unwrap();
    assert_eq!((c["encaisse"].as_i64(), c["eteint_par_retour"].as_i64(), c["reste"].as_i64()),
        (Some(1_000), Some(800), Some(600)));
    let o = creances::lire_creances_ouvertes(conn, None).unwrap();
    assert_eq!(o[0]["encaisse"], 1_000);
    let t = pieces::lire_donnees_piece(conn, piece_id.clone()).unwrap()["totaux"].clone();
    assert_eq!((t["encaisse"].as_i64(), t["eteint_par_retour"].as_i64(), t["reste_du"].as_i64()),
        (Some(1_000), Some(800), Some(600)));

    // B.4 : le reste de 600 regle a 5 F pres se solde, avec sa trace.
    let r = creances::regler_creance(conn, vente.clone(), 595, "especes".into(), Some("patron".into())).unwrap();
    assert_eq!(r["residu_absorbe"], 5);
    let statut: String = conn
        .query_row("SELECT statut FROM vente WHERE id = ?1", rusqlite::params![vente], |r| r.get(0))
        .unwrap();
    assert_eq!(statut, "payee");
    let piece_statut: String = conn
        .query_row("SELECT statut FROM piece_commerciale WHERE id = ?1", rusqlite::params![piece_id], |r| r.get(0))
        .unwrap();
    assert_eq!(piece_statut, "paye");
    let traces: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM journal WHERE type_evenement = 'residu_absorbe' AND entite_id = ?1",
            rusqlite::params![vente],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(traces, 1);
}

// =====================================================================
//  Le détecteur
// =====================================================================

#[test]
fn tout_ce_que_touche_le_lot_b_passe_le_detecteur() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let sucre = article_unite(&mut base, "Sucre");
    let client = client_reel(&mut base);
    // La facture POS se fait avant : elle n'est pas du lot B.
    let (v1, l1) = vendre(&mut base, &client, 3.0, 1.0, None, None);
    let piece = facturer(&mut base, &v1);
    base.auditer(true);

    let (v0, _) = vendre(&mut base, &client, 1.0, 0.0, Some(SUCRE - 3), None);
    assert_eq!(residus_journal(&mut base, &v0), (1, 3));
    let (v2, l2) = vendre(&mut base, &client, 2.0, 0.0, None, None);
    creances::regler_creance_sur_base(&mut base, v2.clone(), SUCRE - 3, "especes".into(), None).expect("régler");
    rendre(&mut base, &v2, &l2[0], 1.0);
    retours::enregistrer_retour_groupe_sur_base(
        &mut base, v1.clone(),
        vec![
            retours::ArticleRendu { ligne_vente_id: l1[0].clone(), quantite: 1.0 },
            retours::ArticleRendu { ligne_vente_id: l1[1].clone(), quantite: 1.0 },
        ],
        "remboursement".into(), Some("especes".into()), vec![], None, None,
    )
    .expect("retour groupé");

    let p = pieces::creer_piece_sur_base(
        &mut base, client.clone(), "facture".into(), vec![ligne(&sucre, 1.0)], None, None, None, None, None, None,
    )
    .unwrap();
    argent::valider_facture_sur_base(
        &mut base, p["id"].as_str().unwrap().to_string(), "credit".into(), Some("especes".into()), Some(SUCRE - 4), None,
    )
    .expect("facture validée");
    avoirs::accorder_avoir_client_sur_base(&mut base, client.clone(), SUCRE, "geste".into(), Some("patron".into()))
        .expect("avoir");
    let (v3, _) = vendre(&mut base, &client, 1.0, 0.0, None, None);
    avoirs::appliquer_avoir_vente_sur_base(&mut base, v3, client.clone(), SUCRE - 2).expect("appliquer");
    let (v4, _) = vendre(&mut base, &client, 1.0, 0.0, None, None);
    chantiers::marquer_irrecouvrable_sur_base(&mut base, v4.clone(), "m".into()).expect("irrécouvrable");
    creances::regler_creance_exceptionnel_sur_base(&mut base, v4, SUCRE - 1, "especes".into(), None, None)
        .expect("recouvrement");

    tableau_bord::lire_resume_dashboard_sur(&mut base, None).expect("résumé");
    for p in ["jour", "semaine", "mois", "annee"] {
        tableau_bord::lire_ventes_periode_sur(&mut base, Some(p.into()), None).expect("courbe");
    }
    tableau_bord::lire_top_clients_sur(&mut base).expect("top clients");
    tableau_bord::lire_top_articles_sur(&mut base).expect("top articles");
    rapports::lire_rapport_ca_mensuel_sur_base(&mut base, None).expect("mensuel");
    rapports::lire_rapport_top_clients_sur_base(&mut base, "2000-01-01".into(), "2099-01-01".into(), None).expect("clients");
    rapports::lire_rapport_top_articles_sur_base(&mut base, "2000-01-01".into(), "2099-01-01".into(), None).expect("articles");
    relances::lire_creances_relances_sur_base(&mut base, None).expect("relances");
    creances::lire_creances_ouvertes_sur_base(&mut base, None).expect("créances ouvertes");
    pieces::lire_donnees_piece_sur_base(&mut base, piece).expect("pièce imprimée");
    pieces::lire_fiche_client_sur_base(&mut base, client).expect("fiche client");
}
