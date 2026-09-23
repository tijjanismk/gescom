//! B-1 — l'Historique : le journal metier se lit, sur les deux moteurs.
//!
//! La preuve du plan : « qui a annule le reglement de Coulibaly
//! mardi ? » se trouve en tapant le nom du client. Puis les filtres, la
//! pagination, la permission, et le detecteur de cloisonnement.

mod commun;

use commun::*;
use gescom_noyau::argent::{self, ParamsLigneInput};
use gescom_noyau::base::Base;
use gescom_noyau::historique::{self, Filtre};
use gescom_noyau::{creances, parametres, portes};

/// Une vente a credit au premier vrai client, un acompte, puis
/// l'annulation de cet acompte. Rend (client_id, nom du client, vente_id,
/// paiement annule).
fn reglement_annule(base: &mut Base) -> (String, String, String, String) {
    ouvrir_caisse(base);
    let depot = depot_defaut(base);
    let sucre = article_unite(base, "Sucre");
    let client = client_reel(base);
    let v = argent::creer_vente_sur_base(
        base,
        client.clone(),
        depot.clone(),
        "credit".into(),
        vec![ParamsLigneInput {
            article_id: sucre.0.clone(),
            unite_vente_id: sucre.1.clone(),
            depot_source_id: depot,
            source_approvisionnement: "stock".into(),
            quantite: 2.0,
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
    .expect("vente a credit");
    let vente_id = v["vente_id"].as_str().unwrap().to_string();
    creances::regler_creance_sur_base(base, vente_id.clone(), 300, "especes".into(), None).expect("acompte");
    let regs = creances::lire_reglements_client_sur_base(base, client.clone()).unwrap();
    let paiement = regs.iter().find(|r| r["montant"] == 300).unwrap()["id"].as_str().unwrap().to_string();
    creances::annuler_reglement_sur_base(base, paiement.clone(), "Erreur de saisie".into(), true, None)
        .expect("annuler");
    let nom: String = base
        .lire_une("SELECT nom FROM client WHERE id = ?1", &parametres![client.clone()], |r| r.get::<String>(0))
        .unwrap()
        .unwrap();
    (client, nom, vente_id, paiement)
}

fn lire(base: &mut Base, f: Filtre) -> serde_json::Value {
    historique::lire_historique_sur(base, f).expect("historique")
}

#[test]
fn un_reglement_annule_se_retrouve_par_le_nom_du_client() {
    let mut base = base_avec_demo();
    let (client, nom, _vente, paiement) = reglement_annule(&mut base);

    // Un morceau du nom, en minuscules : c'est ce que tape le patron.
    let morceau: String = nom.chars().take(5).collect::<String>().to_lowercase();
    let v = lire(&mut base, Filtre { recherche: Some(morceau), ..Default::default() });
    let lignes = v["lignes"].as_array().unwrap();
    let l = lignes
        .iter()
        .find(|l| l["type"] == "reglement_annule")
        .unwrap_or_else(|| panic!("l'annulation doit sortir par le nom du client : {v:#}"));

    assert_eq!(l["libelle_type"], "Règlement annulé");
    assert_eq!(l["entite_type"], "paiement");
    assert_eq!(l["entite_id"], paiement.as_str());
    // SUR QUOI : le paiement est resolu jusqu'au client.
    assert_eq!(l["tiers_id"], client.as_str());
    assert_eq!(l["tiers_nom"], nom.as_str());
    // QUI : un nom, pas un identifiant.
    assert!(l["auteur_nom"].as_str().map_or(false, |n| !n.is_empty()), "auteur nomme : {l}");
    // AVANT -> APRES : le detail enregistre, lu comme un objet.
    assert_eq!(l["ancien"]["montant"], 300);
    assert_eq!(l["nouveau"]["motif"], "Erreur de saisie");
    assert_eq!(l["nouveau"]["caisse"], "sortie");
    // QUAND : la date complete.
    assert!(l["date"].as_str().unwrap().len() >= 10);
}

#[test]
fn les_filtres_se_combinent() {
    let mut base = base_avec_demo();
    let (client, _nom, vente, _paiement) = reglement_annule(&mut base);
    let tout = lire(&mut base, Filtre::default());
    let total = tout["total"].as_i64().unwrap();
    assert!(total >= 2, "vente + annulation au moins : {tout:#}");

    // Par client : tout ce qui touche ce client, et rien d'autre.
    let du_client = lire(&mut base, Filtre { tiers_id: Some(client.clone()), ..Default::default() });
    assert!(du_client["total"].as_i64().unwrap() >= 1);
    for l in du_client["lignes"].as_array().unwrap() {
        assert_eq!(l["tiers_id"], client.as_str(), "{l}");
    }

    // Par type.
    let annulations = lire(
        &mut base,
        Filtre { type_evenement: Some("reglement_annule".into()), ..Default::default() },
    );
    assert_eq!(annulations["total"], 1);

    // Par personne : l'auteur de l'annulation retrouve sa ligne ; un
    // inconnu ne retrouve rien.
    let auteur = annulations["lignes"][0]["auteur_id"].as_str().unwrap().to_string();
    let de_lui = lire(
        &mut base,
        Filtre { auteur_id: Some(auteur), type_evenement: Some("reglement_annule".into()), ..Default::default() },
    );
    assert_eq!(de_lui["total"], 1);
    let personne = lire(&mut base, Filtre { auteur_id: Some("inconnu".into()), ..Default::default() });
    assert_eq!(personne["total"], 0);

    // Par jour : aujourd'hui trouve, un jour passe ne trouve rien.
    let aujourdhui = gescom_noyau::utils::maintenant_iso()[..10].to_string();
    let jour = lire(&mut base, Filtre { du: Some(aujourdhui.clone()), au: Some(aujourdhui), ..Default::default() });
    assert!(jour["total"].as_i64().unwrap() >= 2);
    let avant = lire(&mut base, Filtre { au: Some("2000-01-01".into()), ..Default::default() });
    assert_eq!(avant["total"], 0);

    // Un filtre vide ne filtre pas : l'ecran envoie "" quand on efface.
    let vides = lire(
        &mut base,
        Filtre { recherche: Some("  ".into()), auteur_id: Some("".into()), ..Default::default() },
    );
    assert_eq!(vides["total"], total);

    // Une date mal ecrite est refusee, pas ignoree.
    assert!(historique::lire_historique_sur(&mut base, Filtre { du: Some("hier".into()), ..Default::default() }).is_err());

    // Par piece : la piece de la vente retrouve au moins l'annulation
    // du reglement (paiement -> vente -> piece).
    let piece: Option<String> = base
        .lire_une("SELECT piece_id FROM vente WHERE id = ?1", &parametres![vente], |r| r.get::<Option<String>>(0))
        .unwrap()
        .flatten();
    if let Some(piece) = piece {
        let de_la_piece = lire(&mut base, Filtre { piece_id: Some(piece.clone()), ..Default::default() });
        let lignes = de_la_piece["lignes"].as_array().unwrap();
        assert!(lignes.iter().any(|l| l["type"] == "reglement_annule"), "{de_la_piece:#}");
        assert!(lignes.iter().all(|l| l["piece_id"] == piece.as_str()));
        assert!(lignes.iter().all(|l| l["piece_numero"].is_string()), "le numero est resolu");
    }
}

#[test]
fn la_pagination_ne_perd_ni_ne_double_une_ligne() {
    let mut base = base_avec_demo();
    reglement_annule(&mut base);
    let tout = lire(&mut base, Filtre { par_page: 200, ..Default::default() });
    let total = tout["total"].as_i64().unwrap() as usize;
    assert!(total >= 2);

    let mut vus = Vec::new();
    for page in 0..total as i64 {
        let p = lire(&mut base, Filtre { page, par_page: 1, ..Default::default() });
        assert_eq!(p["total"].as_i64().unwrap() as usize, total, "le total ne depend pas de la page");
        assert_eq!(p["par_page"], 1);
        let lignes = p["lignes"].as_array().unwrap();
        assert_eq!(lignes.len(), 1, "page {page}");
        vus.push(lignes[0]["id"].as_str().unwrap().to_string());
    }
    let attendus: Vec<String> =
        tout["lignes"].as_array().unwrap().iter().map(|l| l["id"].as_str().unwrap().to_string()).collect();
    assert_eq!(vus, attendus, "page par page = tout d'un coup, dans le meme ordre");

    let apres = lire(&mut base, Filtre { page: total as i64, par_page: 1, ..Default::default() });
    assert_eq!(apres["lignes"].as_array().unwrap().len(), 0);

    // Une page demesuree est bornee.
    let enorme = lire(&mut base, Filtre { par_page: 1_000_000, ..Default::default() });
    assert_eq!(enorme["par_page"], 200);
}

#[test]
fn le_journal_se_lit_avec_sa_permission() {
    let mut base = base_avec_demo();
    assert!(portes::existe("journal:lire"));
    assert!(portes::permissions_de_sur(&mut base, "x", "comptable").contains("journal:lire"));
    assert!(!portes::permissions_de_sur(&mut base, "x", "caissier").contains("journal:lire"));
    assert!(!portes::permissions_de_sur(&mut base, "x", "magasinier").contains("journal:lire"));
    assert!(portes::permissions_de_sur(&mut base, "x", "patron").contains("journal:lire"));
}

#[test]
fn tout_ce_qui_est_porte_ici_passe_le_detecteur() {
    let mut base = base_avec_demo();
    reglement_annule(&mut base);
    base.auditer(true);
    lire(
        &mut base,
        Filtre {
            du: Some("2000-01-01".into()),
            au: Some("2099-12-31".into()),
            auteur_id: Some("x".into()),
            type_evenement: Some("vente_creee".into()),
            tiers_id: Some("x".into()),
            piece_id: Some("x".into()),
            article_id: Some("x".into()),
            recherche: Some("x".into()),
            page: 1,
            par_page: 10,
        },
    );
    let f = historique::filtres_sur(&mut base).expect("filtres");
    assert!(f["types"].as_array().unwrap().len() > 5);
    let auteurs = f["auteurs"].as_array().unwrap();
    assert!(!auteurs.is_empty(), "l'auteur de l'annulation est proposé : {f}");
    assert!(auteurs.iter().all(|a| a["nom"].is_string()));
}
