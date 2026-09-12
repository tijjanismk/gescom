//! `tableau_bord::*_sur` — les cinq lectures de l'écran d'accueil, sur
//! `Base`.
//!
//! C'était le dernier morceau qui manquait à l'écran POS sur
//! PostgreSQL : une caisse s'y connectait, vendait, mais arrivait sur
//! des widgets vides. Ces scénarios vérifient ce que les requêtes
//! rendent RÉELLEMENT après une vente passée par `creer_vente_sur_base`
//! — pas seulement qu'elles rendent `Ok`. Le détecteur de
//! cloisonnement tourne sur chacune d'elles.
//!
//! Sur SQLite en mémoire par défaut. Si `GESCOM_PG` est défini, les
//! mêmes scénarios tournent sur PostgreSQL (`--test-threads=1` : ils
//! partagent une base et la vident chacun) — c'est là que `SUM`
//! rend `NUMERIC`, que `julianday` n'existe pas, et qu'un paramètre
//! NULL doit annoncer son type. Le commerçant sera sur ce moteur-là.

use gescom_noyau::argent::{self, ParamsLigneInput};
use gescom_noyau::base::Base;
use gescom_noyau::parametres;
use gescom_noyau::{amorcage, caisse, tableau_bord};

fn base_avec_demo() -> Base {
    let mut base = match std::env::var("GESCOM_PG") {
        Ok(url) => {
            let mut b = Base::ouvrir(&url).expect("PostgreSQL");
            b.executer_lot("DROP SCHEMA public CASCADE; CREATE SCHEMA public;").unwrap();
            b
        }
        Err(_) => Base::ouvrir(":memory:").expect("base en mémoire"),
    };
    amorcage::amorcer(&mut base).expect("amorçage");
    amorcage::donnees_demo(&mut base).expect("démo");
    base
}

/// (article_id, unite_vente_id, facteur, prix_reference) de l'unité de
/// base d'un article de la démo.
fn article_unite(base: &mut Base, nom: &str) -> (String, String, f64, i64) {
    base.lire_une(
        "SELECT a.id, u.id, u.facteur, u.prix_reference
         FROM article a JOIN unite_vente u ON u.article_id = a.id
         WHERE a.nom = ?1 AND u.facteur = 1.0",
        &parametres![nom],
        |r| Ok((r.get::<String>(0)?, r.get::<String>(1)?, r.get::<f64>(2)?, r.get::<i64>(3)?)),
    )
    .unwrap()
    .unwrap()
}

fn depot_defaut(base: &mut Base) -> String {
    let dossier = base.dossier().to_string();
    base.lire_une(
        "SELECT id FROM depot WHERE est_defaut = 1 AND dossier_id = ?1",
        &parametres![dossier],
        |r| r.get::<String>(0),
    )
    .unwrap()
    .unwrap()
}

fn client_ou(base: &mut Base, generique: bool) -> String {
    let dossier = base.dossier().to_string();
    base.lire_une(
        "SELECT id FROM client WHERE est_generique = ?1 AND dossier_id = ?2 ORDER BY code LIMIT 1",
        &parametres![generique as i64, dossier],
        |r| r.get::<String>(0),
    )
    .unwrap()
    .unwrap()
}

fn ligne(art: &(String, String, f64, i64), depot_id: &str, quantite: f64) -> ParamsLigneInput {
    ParamsLigneInput {
        article_id: art.0.clone(),
        unite_vente_id: art.1.clone(),
        depot_source_id: depot_id.to_string(),
        source_approvisionnement: "stock".into(),
        quantite,
        facteur: art.2,
        prix_reference: art.3,
        prix_pratique: art.3,
        taux_tva: None,
        a_decouvert: None,
    }
}

/// Une vente d'un kilo de sucre, comptant, encaissée. Rend son montant.
fn vendre_du_sucre_comptant(base: &mut Base, depot: &str, client: &str) -> i64 {
    let sucre = article_unite(base, "Sucre");
    let prix = sucre.3;
    argent::creer_vente_sur_base(
        base,
        client.to_string(),
        depot.to_string(),
        "comptant".into(),
        vec![ligne(&sucre, depot, 1.0)],
        None,
        Some(prix),
        None,
        None,
    )
    .expect("vente comptant");
    prix
}

#[test]
fn une_base_neuve_a_un_tableau_de_bord_a_zero() {
    let mut base = base_avec_demo();
    let r = tableau_bord::lire_resume_dashboard_sur(&mut base, None).unwrap();
    assert_eq!(r["ca_jour"], 0);
    assert_eq!(r["ca_mois"], 0);
    assert_eq!(r["nb_ventes_jour"], 0);
    assert_eq!(r["total_creances"], 0);
    assert_eq!(r["caisse_session_ouverte"], false);
    assert_eq!(r["caisse_solde"], 0);
    // La démo pose du stock partout : rien en rupture.
    assert_eq!(r["stock_ruptures"], 0);

    assert!(tableau_bord::lire_top_clients_sur(&mut base).unwrap().is_empty());
    assert!(tableau_bord::lire_top_articles_sur(&mut base).unwrap().is_empty());
    assert_eq!(tableau_bord::lire_ventes_a_decouvert_sur(&mut base, None, None).unwrap()["nb"], 0);
}

#[test]
fn une_vente_comptant_se_lit_dans_le_ca_du_jour_de_la_semaine_et_du_mois() {
    let mut base = base_avec_demo();
    caisse::ouvrir_session_caisse_sur(&mut base, 0, "patron".into()).unwrap();
    let depot = depot_defaut(&mut base);
    let client = client_ou(&mut base, true);
    let prix = vendre_du_sucre_comptant(&mut base, &depot, &client);

    let r = tableau_bord::lire_resume_dashboard_sur(&mut base, None).unwrap();
    assert_eq!(r["ca_jour"], prix);
    assert_eq!(r["ca_semaine"], prix);
    assert_eq!(r["ca_mois"], prix);
    assert_eq!(r["ca_mois_precedent"], 0);
    assert_eq!(r["nb_ventes_jour"], 1);
    assert_eq!(r["nb_ventes_mois"], 1);
    // Comptant, encaissée : aucune créance.
    assert_eq!(r["total_creances"], 0);
    assert_eq!(r["nb_creances_ouvertes"], 0);
}

#[test]
fn la_caisse_ouverte_se_voit_avec_son_solde() {
    let mut base = base_avec_demo();
    caisse::ouvrir_session_caisse_sur(&mut base, 5_000, "patron".into()).unwrap();
    let depot = depot_defaut(&mut base);
    let client = client_ou(&mut base, true);
    let prix = vendre_du_sucre_comptant(&mut base, &depot, &client);

    let r = tableau_bord::lire_resume_dashboard_sur(&mut base, None).unwrap();
    assert_eq!(r["caisse_session_ouverte"], true);
    // Le fond compté UNE fois, plus l'encaissement.
    assert_eq!(r["caisse_solde"], 5_000 + prix);
}

#[test]
fn une_vente_a_credit_ouvre_une_creance() {
    let mut base = base_avec_demo();
    let depot = depot_defaut(&mut base);
    let client = client_ou(&mut base, false);
    let sucre = article_unite(&mut base, "Sucre");
    argent::creer_vente_sur_base(
        &mut base,
        client,
        depot.clone(),
        "credit".into(),
        vec![ligne(&sucre, &depot, 2.0)],
        None,
        None,
        None,
        None,
    )
    .expect("vente à crédit");

    let r = tableau_bord::lire_resume_dashboard_sur(&mut base, None).unwrap();
    assert_eq!(r["total_creances"], 2 * sucre.3);
    assert_eq!(r["nb_creances_ouvertes"], 1);
    assert_eq!(r["ca_jour"], 2 * sucre.3, "une vente à crédit compte dans le CA");
}

#[test]
fn le_filtre_par_magasin_ne_montre_que_ses_ventes() {
    let mut base = base_avec_demo();
    caisse::ouvrir_session_caisse_sur(&mut base, 0, "patron".into()).unwrap();
    let depot = depot_defaut(&mut base);
    let client = client_ou(&mut base, true);
    let prix = vendre_du_sucre_comptant(&mut base, &depot, &client);

    let ici = tableau_bord::lire_resume_dashboard_sur(&mut base, Some(depot.clone())).unwrap();
    assert_eq!(ici["ca_jour"], prix);

    let ailleurs =
        tableau_bord::lire_resume_dashboard_sur(&mut base, Some("magasin-inconnu".into()))
            .unwrap();
    assert_eq!(ailleurs["ca_jour"], 0);
    assert_eq!(ailleurs["nb_ventes_jour"], 0);

    // Chaîne vide = vue consolidée, comme `None`.
    let tous = tableau_bord::lire_resume_dashboard_sur(&mut base, Some(String::new())).unwrap();
    assert_eq!(tous["ca_jour"], prix);
}

#[test]
fn la_courbe_du_jour_range_la_vente_dans_son_heure() {
    let mut base = base_avec_demo();
    caisse::ouvrir_session_caisse_sur(&mut base, 0, "patron".into()).unwrap();
    let depot = depot_defaut(&mut base);
    let client = client_ou(&mut base, true);
    let prix = vendre_du_sucre_comptant(&mut base, &depot, &client);

    for periode in ["jour", "semaine", "mois", "annee"] {
        let c = tableau_bord::lire_ventes_periode_sur(&mut base, Some(periode.into()), None)
            .unwrap();
        assert_eq!(c["periode"], periode);
        assert_eq!(c["total"], prix, "période {periode}");
        assert_eq!(c["nb"], 1, "période {periode}");
        let points = c["points"].as_array().unwrap();
        let non_vides: Vec<_> = points.iter().filter(|p| p["nb"] != 0).collect();
        assert_eq!(non_vides.len(), 1, "une seule case remplie en {periode}");
        assert_eq!(non_vides[0]["montant"], prix);
    }

    // Le jour : l'heure de la vente porte le libellé de son heure.
    let heure = chrono::Local::now().format("%H").to_string().parse::<usize>().unwrap();
    let c = tableau_bord::lire_ventes_periode_sur(&mut base, Some("jour".into()), None).unwrap();
    let point = c["points"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["nb"] == 1)
        .unwrap();
    assert_eq!(point["label"], format!("{heure}h"));

    // Une période inconnue retombe sur le jour.
    let c = tableau_bord::lire_ventes_periode_sur(&mut base, Some("n_importe_quoi".into()), None)
        .unwrap();
    assert_eq!(c["periode"], "jour");
}

#[test]
fn la_courbe_du_jour_part_de_six_heures_sauf_vente_plus_tot() {
    let mut base = base_avec_demo();
    let c = tableau_bord::lire_ventes_periode_sur(&mut base, Some("jour".into()), None).unwrap();
    let points = c["points"].as_array().unwrap();
    assert_eq!(points.len(), 18, "de 6h à 23h");
    assert_eq!(points[0]["label"], "6h");

    // Une vente à 3h du matin, posée à la main : la masquer serait mentir.
    let dossier = base.dossier().to_string();
    let depot = depot_defaut(&mut base);
    let client = client_ou(&mut base, true);
    let nuit = chrono::Local::now().format("%Y-%m-%dT03:00:00.000").to_string();
    base.executer(
        "INSERT INTO vente
           (id, client_id, depot_id, mode_reglement, statut, date_vente,
            cree_le, modifie_le, origine, dossier_id)
         VALUES ('v-nuit', ?1, ?2, 'comptant', 'payee', ?3, ?3, ?3, 'test', ?4)",
        &parametres![client, depot, nuit, dossier],
    )
    .unwrap();
    let c = tableau_bord::lire_ventes_periode_sur(&mut base, Some("jour".into()), None).unwrap();
    let points = c["points"].as_array().unwrap();
    assert_eq!(points.len(), 24);
    assert_eq!(points[3]["nb"], 1);
}

#[test]
fn les_meilleurs_clients_et_articles_du_mois() {
    let mut base = base_avec_demo();
    caisse::ouvrir_session_caisse_sur(&mut base, 0, "patron".into()).unwrap();
    let depot = depot_defaut(&mut base);
    let generique = client_ou(&mut base, true);
    let reel = client_ou(&mut base, false);
    let prix = vendre_du_sucre_comptant(&mut base, &depot, &generique);
    vendre_du_sucre_comptant(&mut base, &depot, &reel);

    // Le client de passage n'entre pas au palmarès (D40 : il n'a pas
    // d'identité), le client réel oui.
    let clients = tableau_bord::lire_top_clients_sur(&mut base).unwrap();
    assert_eq!(clients.len(), 1);
    assert_eq!(clients[0]["ca"], prix);
    assert_eq!(clients[0]["nb_ventes"], 1);

    let articles = tableau_bord::lire_top_articles_sur(&mut base).unwrap();
    assert_eq!(articles.len(), 1);
    assert_eq!(articles[0]["nom"], "Sucre");
    assert_eq!(articles[0]["unite"], "kg");
    assert_eq!(articles[0]["ca"], 2 * prix);
    assert_eq!(articles[0]["qte_vendue"], 2.0);
}

#[test]
fn une_vente_a_decouvert_apparait_dans_sa_liste() {
    let mut base = base_avec_demo();
    let depot = depot_defaut(&mut base);
    let client = client_ou(&mut base, false);
    let sucre = article_unite(&mut base, "Sucre");
    // 200 kg en stock (démo) : en vendre 250 met le magasin à découvert.
    argent::creer_vente_sur_base(
        &mut base,
        client,
        depot.clone(),
        "credit".into(),
        vec![ligne(&sucre, &depot, 250.0)],
        None,
        None,
        None,
        None,
    )
    .expect("vente à découvert acceptée à crédit");

    let l = tableau_bord::lire_ventes_a_decouvert_sur(&mut base, None, None).unwrap();
    assert_eq!(l["nb"], 1);
    assert_eq!(l["lignes"][0]["article"], "Sucre");
    assert_eq!(l["lignes"][0]["quantite"], 250.0);

    // Les bornes de dates portent sur le jour (`AAAA-MM-JJ`).
    let aujourd_hui = chrono::Local::now().format("%Y-%m-%d").to_string();
    let l = tableau_bord::lire_ventes_a_decouvert_sur(
        &mut base,
        Some(aujourd_hui.clone()),
        Some(aujourd_hui),
    )
    .unwrap();
    assert_eq!(l["nb"], 1);
    let l = tableau_bord::lire_ventes_a_decouvert_sur(&mut base, Some("2099-01-01".into()), None)
        .unwrap();
    assert_eq!(l["nb"], 0);
}

#[test]
fn une_facture_dont_l_echeance_est_passee_compte_en_retard() {
    let mut base = base_avec_demo();
    let dossier = base.dossier().to_string();
    let client = client_ou(&mut base, false);
    let now = gescom_noyau::utils::maintenant_iso();
    for (id, echeance, statut) in [
        ("f-retard", "2020-01-01", "emis"),
        ("f-a-venir", "2099-01-01", "emis"),
        ("f-brouillon", "2020-01-01", "brouillon"),
        ("f-payee", "2020-01-01", "paye"),
    ] {
        base.executer(
            "INSERT INTO piece_commerciale
               (id, numero, type_piece, statut, tiers_id, date_piece, date_echeance,
                cree_le, modifie_le, origine, dossier_id)
             VALUES (?1, ?1, 'facture', ?2, ?3, ?4, ?5, ?4, ?4, 'test', ?6)",
            &parametres![id, statut, client.clone(), now.clone(), echeance, dossier.clone()],
        )
        .unwrap();
    }
    let r = tableau_bord::lire_resume_dashboard_sur(&mut base, None).unwrap();
    assert_eq!(r["nb_creances_en_retard"], 2, "émise et brouillon échues, pas la payée");
    assert_eq!(r["factures_brouillon"], 1);
}

#[test]
fn deux_dossiers_ne_partagent_pas_leur_tableau_de_bord() {
    let mut base = base_avec_demo();
    caisse::ouvrir_session_caisse_sur(&mut base, 1_000, "patron".into()).unwrap();
    let depot = depot_defaut(&mut base);
    let client = client_ou(&mut base, true);
    vendre_du_sucre_comptant(&mut base, &depot, &client);

    base.choisir_dossier("dossier-b").unwrap();
    let r = tableau_bord::lire_resume_dashboard_sur(&mut base, None).unwrap();
    assert_eq!(r["ca_jour"], 0, "la vente de l'autre société ne doit pas se voir");
    assert_eq!(r["caisse_session_ouverte"], false);
    assert!(tableau_bord::lire_top_articles_sur(&mut base).unwrap().is_empty());
    let c = tableau_bord::lire_ventes_periode_sur(&mut base, Some("mois".into()), None).unwrap();
    assert_eq!(c["nb"], 0);
}

#[test]
fn tout_le_tableau_de_bord_passe_le_detecteur() {
    let mut base = base_avec_demo();
    caisse::ouvrir_session_caisse_sur(&mut base, 0, "patron".into()).unwrap();
    let depot = depot_defaut(&mut base);
    let client = client_ou(&mut base, true);
    vendre_du_sucre_comptant(&mut base, &depot, &client);
    base.auditer(true);

    // Contrairement à la version SQLite, le résumé fait REMONTER une
    // requête refusée au lieu de retomber à 0 : c'est ce qui permet au
    // détecteur de parler ici. On vérifie aussi le résultat, pour que
    // ce test casse le jour où quelqu'un remettrait un `unwrap_or(0)`.
    let r = tableau_bord::lire_resume_dashboard_sur(&mut base, Some(depot))
        .expect("le résumé passe le détecteur");
    assert_eq!(r["nb_ventes_jour"], 1);
    assert_eq!(r["caisse_session_ouverte"], true);

    tableau_bord::lire_ventes_periode_sur(&mut base, Some("semaine".into()), None)
        .expect("courbe");
    tableau_bord::lire_top_clients_sur(&mut base).expect("clients");
    tableau_bord::lire_top_articles_sur(&mut base).expect("articles");
    tableau_bord::lire_ventes_a_decouvert_sur(&mut base, None, None).expect("découvert");
}
