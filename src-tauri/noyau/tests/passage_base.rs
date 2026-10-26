//! D35 — les clients de passage, sur les deux moteurs.
//!
//! Chaque vente au « Comptant » ouvre sa fiche « Client de passage »
//! (code PAS-…), payee en entier ; le refus n'ecrit rien ; les fiches
//! restent hors des listes ; « passer en client ordinaire » garde les
//! achats et laisse aux factures emises le nom sous lequel elles l'ont
//! ete. Stock, caisse et creance verifies apres chaque geste.
//!
//!   cargo test -p gescom-noyau --test passage_base
//!   GESCOM_PG=postgresql://…/gescom_test cargo test -p gescom-noyau --test passage_base -- --test-threads=1

mod commun;

use commun::*;
use gescom_noyau::argent::{self, ParamsLigneInput};
use gescom_noyau::base::Base;
use gescom_noyau::parametres;
use gescom_noyau::utils::maintenant_iso;
use gescom_noyau::{catalogue, comptoir, passage, pieces};

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

/// Une vente au comptoir, au « Comptant », `paye` encaisse en especes.
fn vendre_au_comptant(base: &mut Base, quantite: f64, paye: Option<i64>) -> Result<serde_json::Value, String> {
    let depot = depot_defaut(base);
    let sucre = article_unite(base, "Sucre");
    let comptant = client_generique(base);
    argent::creer_vente_sur_base(
        base,
        comptant,
        depot.clone(),
        "comptant".into(),
        vec![ligne_vente(&sucre, &depot, quantite)],
        None,
        paye,
        Some("especes".into()),
        None,
    )
}

fn entrees_caisse(base: &mut Base) -> i64 {
    let dossier = base.dossier().to_string();
    compter(
        base,
        "SELECT CAST(COALESCE(SUM(montant), 0) AS BIGINT) FROM mouvement_caisse
         WHERE sens = 'entree' AND dossier_id = ?1",
        &parametres![dossier],
    )
}

fn nb_passages(base: &mut Base) -> i64 {
    let dossier = base.dossier().to_string();
    compter(base, "SELECT COUNT(*) FROM client WHERE statut = 'passage' AND dossier_id = ?1", &parametres![dossier])
}

fn client_de_la_vente(base: &mut Base, vente_id: &str) -> String {
    base.lire_une("SELECT client_id FROM vente WHERE id = ?1", &parametres![vente_id], |r| r.get::<String>(0))
        .unwrap()
        .unwrap()
}

/// Une facture brouillon d'une ligne de sucre pour ce client ; rend
/// (piece_id, total).
fn facture_brouillon(base: &mut Base, client: &str, quantite: f64) -> (String, i64) {
    let dossier = base.dossier().to_string();
    let now = maintenant_iso();
    let depot = depot_defaut(base);
    let sucre = article_unite(base, "Sucre");
    let piece_id = uuid::Uuid::new_v4().to_string();
    let total = (sucre.3 as f64 * quantite).round() as i64;
    base.executer(
        "INSERT INTO piece_commerciale
           (id, type_piece, numero, statut, tiers_type, tiers_id, depot_id,
            auteur_id, date_piece, remise_globale, cree_le, modifie_le, origine, dossier_id)
         VALUES (?1,'facture',?2,'brouillon','client',?3,?4,'test',?5,0.0,?5,?5,'test',?6)",
        &parametres![piece_id.clone(), format!("FAC-PAS-{}", &piece_id[..8]), client, depot, now.clone(), dossier.clone()],
    )
    .unwrap();
    base.executer(
        "INSERT INTO ligne_piece
           (id, piece_id, article_id, unite_vente_id, quantite, prix_unitaire,
            remise_pct, remise_montant, taux_tva, montant_tva, montant_ht, cree_le, dossier_id)
         VALUES (?1,?2,?3,?4,?5,?6,0,0,0,0,?7,?8,?9)",
        &parametres![uuid::Uuid::new_v4().to_string(), piece_id.clone(), sucre.0, sucre.1, quantite, sucre.3, total, now, dossier],
    )
    .unwrap();
    (piece_id, total)
}

#[test]
fn une_vente_au_comptant_ouvre_sa_fiche_de_passage() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let depot = depot_defaut(&mut base);
    let sucre = article_unite(&mut base, "Sucre");
    let comptant = client_generique(&mut base);
    let stock_avant = stock(&mut base, &sucre.0, &depot);
    let caisse_avant = entrees_caisse(&mut base);

    let r = vendre_au_comptant(&mut base, 5.0, Some(4_000)).expect("la vente au comptant passe");
    assert_eq!(r["statut"], "payee");
    assert_eq!(r["client_code"], "PAS-000001", "{r}");
    let fiche = r["client_id"].as_str().unwrap().to_string();
    assert_ne!(fiche, comptant, "la vente n'est plus au « Comptant »");
    assert_eq!(client_de_la_vente(&mut base, r["vente_id"].as_str().unwrap()), fiche);

    let (nom, generique, statut): (String, i64, String) = base
        .lire_une(
            "SELECT nom, est_generique, statut FROM client WHERE id = ?1",
            &parametres![fiche.clone()],
            |l| Ok((l.get::<String>(0)?, l.get::<i64>(1)?, l.get::<String>(2)?)),
        )
        .unwrap()
        .unwrap();
    assert_eq!((nom.as_str(), generique, statut.as_str()), (passage::NOM, 1, "passage"));
    assert_eq!(stock(&mut base, &sucre.0, &depot), stock_avant - 5.0, "le stock sort");
    assert_eq!(entrees_caisse(&mut base), caisse_avant + 4_000, "l'argent entre");

    // Une deuxieme vente : une deuxieme fiche, le numero suivant.
    let r2 = vendre_au_comptant(&mut base, 1.0, Some(800)).unwrap();
    assert_eq!(r2["client_code"], "PAS-000002");
    assert_ne!(r2["client_id"], r["client_id"]);
    // Le « Comptant » reste la porte de l'ecran, et lui seul.
    let c = catalogue::lire_client_generique_sur(&mut base).unwrap();
    assert_eq!(c["id"], comptant.as_str());
}

#[test]
fn un_passant_paie_en_entier_sinon_rien_ne_s_ecrit() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let depot = depot_defaut(&mut base);
    let sucre = article_unite(&mut base, "Sucre");
    let stock_avant = stock(&mut base, &sucre.0, &depot);
    let caisse_avant = entrees_caisse(&mut base);

    let partiel = vendre_au_comptant(&mut base, 5.0, Some(2_000)).expect_err("2 000 F sur 4 000 F");
    assert!(partiel.contains("paie en entier") && partiel.contains("4000 F"), "{partiel}");
    let rien = vendre_au_comptant(&mut base, 5.0, None).expect_err("rien d'encaissé");
    assert!(rien.contains("paie en entier"), "{rien}");

    // Le credit, au « Comptant » comme a une fiche de passage.
    let comptant = client_generique(&mut base);
    let credit = argent::creer_vente_sur_base(
        &mut base, comptant, depot.clone(), "credit".into(),
        vec![ligne_vente(&sucre, &depot, 1.0)], None, None, None, None,
    )
    .expect_err("pas de crédit");
    assert!(credit.contains("crédit"), "{credit}");

    assert_eq!(stock(&mut base, &sucre.0, &depot), stock_avant, "le stock n'a pas bougé");
    assert_eq!(entrees_caisse(&mut base), caisse_avant, "la caisse non plus");
    assert_eq!(nb_passages(&mut base), 0, "aucune fiche de passage créée");
    let dossier = base.dossier().to_string();
    assert_eq!(compter(&mut base, "SELECT COUNT(*) FROM vente WHERE dossier_id = ?1", &parametres![dossier]), 0);
    // Les refus n'ont pas consomme de numero.
    let r = vendre_au_comptant(&mut base, 1.0, Some(800)).unwrap();
    assert_eq!(r["client_code"], "PAS-000001");

    // Sur la fiche elle-meme, la meme regle (un passant qui revient).
    let fiche = r["client_id"].as_str().unwrap().to_string();
    let err = argent::creer_vente_sur_base(
        &mut base, fiche.clone(), depot.clone(), "comptant".into(),
        vec![ligne_vente(&sucre, &depot, 2.0)], None, Some(1_000), Some("especes".into()), None,
    )
    .expect_err("une fiche de passage paie en entier");
    assert!(err.contains("paie en entier"), "{err}");
    // Payee en entier, elle reste sur SA fiche (pas une nouvelle).
    let r2 = argent::creer_vente_sur_base(
        &mut base, fiche.clone(), depot.clone(), "comptant".into(),
        vec![ligne_vente(&sucre, &depot, 2.0)], None, Some(1_600), Some("especes".into()), None,
    )
    .unwrap();
    assert_eq!(r2["client_id"], fiche.as_str());
    assert!(r2["client_code"].is_null());
    assert_eq!(nb_passages(&mut base), 1);
}

#[test]
fn une_facture_au_comptant_a_aussi_sa_fiche_et_pas_de_credit() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let comptant = client_generique(&mut base);
    let (piece, total) = facture_brouillon(&mut base, &comptant, 3.0);

    let err = argent::valider_facture_sur_base(&mut base, piece.clone(), "credit".into(), Some("especes".into()), Some(1_000), None)
        .expect_err("une facture à crédit au « Comptant » est refusée");
    assert!(err.contains("crédit"), "{err}");
    assert_eq!(statut_piece(&mut base, &piece), "brouillon", "rien n'est émis");
    assert_eq!(nb_passages(&mut base), 0);

    let caisse_avant = entrees_caisse(&mut base);
    let r = argent::valider_facture_sur_base(&mut base, piece.clone(), "comptant".into(), Some("especes".into()), None, None)
        .expect("au comptant, elle passe");
    assert_eq!(r["statut_piece"], "paye");
    assert_eq!(entrees_caisse(&mut base), caisse_avant + total);
    let tiers: String = base
        .lire_une("SELECT tiers_id FROM piece_commerciale WHERE id = ?1", &parametres![piece.clone()], |l| l.get::<String>(0))
        .unwrap()
        .unwrap();
    assert_ne!(tiers, comptant, "la facture est à la fiche de passage");
    let statut: String = base
        .lire_une("SELECT statut FROM client WHERE id = ?1", &parametres![tiers], |l| l.get::<String>(0))
        .unwrap()
        .unwrap();
    assert_eq!(statut, "passage");
}

#[test]
fn les_passages_restent_hors_des_listes_et_se_retrouvent_par_leur_ticket() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let r = vendre_au_comptant(&mut base, 2.0, Some(1_600)).unwrap();
    let fiche = r["client_id"].as_str().unwrap().to_string();

    let clients = catalogue::lire_clients_sur(&mut base).unwrap();
    assert!(!clients.iter().any(|c| c["id"] == fiche.as_str()), "pas dans la liste des clients");
    let pagines = gescom_noyau::pagination::lire_clients_pagines_sur_base(&mut base, 0, 50, None, false, None, None).unwrap();
    assert!(!pagines.to_string().contains(&fiche), "ni dans la liste paginée");

    let tous = passage::lire_passages_sur(&mut base, None).unwrap();
    assert_eq!(tous.len(), 1);
    assert_eq!(tous[0]["code"], "PAS-000001");
    assert_eq!(tous[0]["paye"], 1_600);
    assert_eq!(passage::lire_passages_sur(&mut base, Some("000001".into())).unwrap().len(), 1, "par le code");
    assert_eq!(passage::lire_passages_sur(&mut base, Some("PAS-9".into())).unwrap().len(), 0);

    // Par le numero de sa facture (le ticket qu'il rapporte).
    // L'ecran passe le « Comptant » : la facture suit le client de la VENTE.
    let comptant = client_generique(&mut base);
    let f = argent::creer_facture_depuis_vente_sur_base(
        &mut base, r["vente_id"].as_str().unwrap().to_string(), comptant, "comptant".into(), None)
        .expect("la facture du ticket");
    let numero = f["numero"].as_str().expect("numéro de facture").to_string();
    let piece = f["piece_id"].as_str().or(f["id"].as_str()).unwrap_or_default().to_string();
    if !piece.is_empty() {
        let tiers: String = base
            .lire_une("SELECT tiers_id FROM piece_commerciale WHERE id = ?1", &parametres![piece], |l| l.get::<String>(0))
            .unwrap()
            .unwrap();
        assert_eq!(tiers, fiche, "la facture du ticket est à la fiche de passage");
    }
    let trouves = passage::lire_passages_sur(&mut base, Some(numero.clone())).unwrap();
    assert_eq!(trouves.len(), 1, "retrouvé par sa facture {numero}");
}

#[test]
fn passer_en_client_ordinaire_garde_les_achats_et_le_nom_des_factures() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let comptant = client_generique(&mut base);
    let (piece, _) = facture_brouillon(&mut base, &comptant, 2.0);
    argent::valider_facture_sur_base(&mut base, piece.clone(), "comptant".into(), Some("especes".into()), None, None).unwrap();
    let fiche: String = base
        .lire_une("SELECT tiers_id FROM piece_commerciale WHERE id = ?1", &parametres![piece.clone()], |l| l.get::<String>(0))
        .unwrap()
        .unwrap();
    let nom_facture = |base: &mut Base| -> String {
        pieces::lire_donnees_piece_sur_base(base, piece.clone()).unwrap()["piece"]["client_nom"]
            .as_str()
            .unwrap_or_default()
            .to_string()
    };
    assert_eq!(nom_facture(&mut base), passage::NOM);

    // Le « Comptant » et un client ordinaire ne se promeuvent pas ; un nom vide non plus.
    assert!(passage::promouvoir_sur(&mut base, comptant.clone(), "X".into(), None).is_err());
    let reel = client_reel(&mut base);
    assert!(passage::promouvoir_sur(&mut base, reel, "X".into(), None).is_err());
    assert!(passage::promouvoir_sur(&mut base, fiche.clone(), "  ".into(), None).unwrap_err().contains("nom"));

    let r = passage::promouvoir_sur(&mut base, fiche.clone(), "Moussa Traoré".into(), Some("76 11 22 33".into())).unwrap();
    assert_eq!(r["code"], "PAS-000001", "il garde son code");
    let (nom, generique, statut, tel): (String, i64, String, Option<String>) = base
        .lire_une(
            "SELECT nom, est_generique, statut, telephone FROM client WHERE id = ?1",
            &parametres![fiche.clone()],
            |l| Ok((l.get::<String>(0)?, l.get::<i64>(1)?, l.get::<String>(2)?, l.get::<Option<String>>(3)?)),
        )
        .unwrap()
        .unwrap();
    assert_eq!((nom.as_str(), generique, statut.as_str()), ("Moussa Traoré", 0, "client"));
    assert_eq!(tel.as_deref(), Some("76 11 22 33"));
    // La facture emise garde le nom sous lequel elle l'a ete.
    assert_eq!(nom_facture(&mut base), passage::NOM, "facture émise : nom figé");
    // Il est dans la liste des clients, plus dans les passages.
    assert!(catalogue::lire_clients_sur(&mut base).unwrap().iter().any(|c| c["id"] == fiche.as_str()));
    assert!(passage::lire_passages_sur(&mut base, None).unwrap().is_empty());
    // Son achat le suit, et le credit lui est maintenant ouvert.
    let dossier = base.dossier().to_string();
    assert_eq!(compter(&mut base, "SELECT COUNT(*) FROM vente WHERE client_id = ?1 AND dossier_id = ?2",
        &parametres![fiche.clone(), dossier.clone()]), 1);
    let depot = depot_defaut(&mut base);
    let sucre = article_unite(&mut base, "Sucre");
    let credit = argent::creer_vente_sur_base(
        &mut base, fiche.clone(), depot.clone(), "credit".into(),
        vec![ligne_vente(&sucre, &depot, 1.0)], None, None, None, None,
    )
    .expect("client ordinaire : le crédit passe");
    assert_eq!(credit["statut"], "creance_ouverte");
    // L'Historique dit le geste.
    assert_eq!(compter(&mut base, "SELECT COUNT(*) FROM journal WHERE type_evenement = 'passage_promu' AND entite_id = ?1 AND dossier_id = ?2",
        &parametres![fiche.clone(), dossier]), 1);

    // Renommer un client ordinaire fige aussi ses factures emises.
    comptoir::modifier_client_sur(&mut base, fiche.clone(), "Moussa T.".into(), None, None, None, None).unwrap();
    assert_eq!(nom_facture(&mut base), passage::NOM);
}

#[test]
fn tout_passe_le_detecteur() {
    let mut base = base_avec_demo();
    base.auditer(true);
    ouvrir_caisse(&mut base);
    let r = vendre_au_comptant(&mut base, 1.0, Some(800)).unwrap();
    let _ = vendre_au_comptant(&mut base, 1.0, Some(100));
    let comptant = client_generique(&mut base);
    let (piece, _) = facture_brouillon(&mut base, &comptant, 1.0);
    argent::valider_facture_sur_base(&mut base, piece, "comptant".into(), Some("especes".into()), None, None).unwrap();
    passage::lire_passages_sur(&mut base, Some("PAS".into())).unwrap();
    passage::promouvoir_sur(&mut base, r["client_id"].as_str().unwrap().to_string(), "Awa".into(), None).unwrap();
    catalogue::lire_clients_sur(&mut base).unwrap();
}
