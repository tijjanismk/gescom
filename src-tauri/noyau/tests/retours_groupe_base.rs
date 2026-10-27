//! Le retour de plusieurs articles en un geste, sur les deux moteurs.
//!
//! La fenetre de retour coche les articles d'une vente (et, en echange,
//! ceux que le client prend) : tout passe ou rien. Une seule piece AVC
//! pour le geste, la difference d'un echange reglee une fois, le plafond
//! de remboursement juge sur le total. Stock, caisse et avoir verifies
//! apres chaque geste.
//!
//!   cargo test -p gescom-noyau --test retours_groupe_base
//!   GESCOM_PG=postgresql://…/gescom_test cargo test -p gescom-noyau --test retours_groupe_base -- --test-threads=1

mod commun;

use commun::*;
use gescom_noyau::argent::{self, ParamsLigneInput};
use gescom_noyau::base::Base;
use gescom_noyau::coeur::plafonds::Plafonds;
use gescom_noyau::retours::{self, ArticlePris, ArticleRendu};
use gescom_noyau::{auth, parametres, plafonds};

type Article = (String, String, f64, i64);

fn ligne_vente(art: &Article, depot: &str, quantite: f64) -> ParamsLigneInput {
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

/// Une vente payee comptant de 3 kg de sucre et 2 kg de riz. Rend
/// (vente, ligne du sucre, ligne du riz).
struct Vente {
    id: String,
    sucre: String,
    riz: String,
}

fn vendre(base: &mut Base, client: String) -> Vente {
    let depot = depot_defaut(base);
    let (sucre, riz) = (article_unite(base, "Sucre"), article_unite(base, "Riz local"));
    let total = 3 * sucre.3 + 2 * riz.3;
    let v = argent::creer_vente_sur_base(
        base, client, depot.clone(), "comptant".into(),
        vec![ligne_vente(&sucre, &depot, 3.0), ligne_vente(&riz, &depot, 2.0)],
        None, Some(total), Some("especes".into()), None,
    )
    .expect("vente");
    let id = v["vente_id"].as_str().unwrap().to_string();
    let ligne = |base: &mut Base, article: &str| -> String {
        let dossier = base.dossier().to_string();
        base.lire_une(
            "SELECT id FROM ligne_vente WHERE vente_id = ?1 AND article_id = ?2 AND dossier_id = ?3",
            &parametres![id.clone(), article, dossier],
            |r| r.get::<String>(0),
        )
        .unwrap()
        .unwrap()
    };
    let (ls, lr) = (ligne(base, &sucre.0), ligne(base, &riz.0));
    Vente { id, sucre: ls, riz: lr }
}

fn rendu(ligne: &str, quantite: f64) -> ArticleRendu {
    ArticleRendu { ligne_vente_id: ligne.to_string(), quantite }
}

fn pris(article: &Article, quantite: f64) -> ArticlePris {
    ArticlePris { article_id: article.0.clone(), unite_vente_id: article.1.clone(), quantite }
}

fn caisse(base: &mut Base, sens: &str, motif: &str) -> i64 {
    compter(
        base,
        "SELECT CAST(COALESCE(SUM(montant), 0) AS BIGINT) FROM mouvement_caisse WHERE sens = ?1 AND motif = ?2",
        &parametres![sens, motif],
    )
}

fn nb(base: &mut Base, table_et_filtre: &str) -> i64 {
    compter(base, &format!("SELECT CAST(COUNT(*) AS BIGINT) FROM {table_et_filtre}"), &[])
}

#[test]
fn rembourser_plusieurs_articles_en_un_geste() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let depot = depot_defaut(&mut base);
    let (sucre, riz) = (article_unite(&mut base, "Sucre"), article_unite(&mut base, "Riz local"));
    let comptant = client_generique(&mut base);
    let v = vendre(&mut base, comptant);
    let (s0, r0) = (stock(&mut base, &sucre.0, &depot), stock(&mut base, &riz.0, &depot));

    let r = retours::enregistrer_retour_groupe_sur_base(
        &mut base, v.id.clone(), vec![rendu(&v.sucre, 2.0), rendu(&v.riz, 2.0)],
        "remboursement".into(), Some("especes".into()), vec![], None, None,
    )
    .expect("le geste passe");
    let attendu = 2 * sucre.3 + 2 * riz.3;
    assert_eq!(r["retours"].as_array().map(|a| a.len()), Some(2));
    assert_eq!(r["montant_credit"], attendu);
    assert_eq!(r["part_especes"], attendu, "un seul total à rendre");
    assert_eq!(stock(&mut base, &sucre.0, &depot), s0 + 2.0, "le sucre revient");
    assert_eq!(stock(&mut base, &riz.0, &depot), r0 + 2.0, "le riz revient");
    assert_eq!(caisse(&mut base, "sortie", "remboursement"), attendu, "l'argent sort du tiroir");
    assert_eq!(nb(&mut base, "retour"), 2);
    // La fenetre de retour le lit : ce qui est deja revenu, par article,
    // et pas d'avoir pour un passant.
    let ventes = retours::lire_ventes_recentes_sur_base(&mut base).unwrap();
    let lue = ventes.iter().find(|x| x["id"] == v.id.as_str()).expect("la vente");
    assert_eq!(lue["client_generique"], true);
    let deja = |ligne: &str| lue["lignes"].as_array().unwrap().iter()
        .find(|l| l["id"] == ligne).unwrap()["deja_retourne"].as_f64();
    assert_eq!((deja(&v.sucre), deja(&v.riz)), (Some(2.0), Some(2.0)));

    // Pas deux fois : le riz est tout revenu, le sucre en reste 1.
    let e = retours::enregistrer_retour_groupe_sur_base(
        &mut base, v.id.clone(), vec![rendu(&v.sucre, 1.0), rendu(&v.riz, 1.0)],
        "remboursement".into(), Some("especes".into()), vec![], None, None,
    )
    .unwrap_err();
    assert!(e.contains("Quantité trop élevée"), "{e}");
    assert_eq!(nb(&mut base, "retour"), 2, "le sucre n'est pas passé seul");
}

#[test]
fn un_avoir_pour_tout_le_geste() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let (sucre, riz) = (article_unite(&mut base, "Sucre"), article_unite(&mut base, "Riz local"));
    let client = client_reel(&mut base);
    let v = vendre(&mut base, client.clone());

    let r = retours::enregistrer_retour_groupe_sur_base(
        &mut base, v.id.clone(), vec![rendu(&v.sucre, 1.0), rendu(&v.riz, 2.0)],
        "avoir_conserve".into(), None, vec![], None, None,
    )
    .unwrap();
    let total = sucre.3 + 2 * riz.3;
    let numero = r["numero_avoir"].as_str().expect("un avoir").to_string();
    assert!(numero.starts_with("AVC-"), "{numero}");
    assert_eq!(nb(&mut base, "piece_commerciale WHERE type_piece = 'avoir_client'"), 1, "une seule pièce AVC");
    let piece: String = base
        .lire_une("SELECT id FROM piece_commerciale WHERE numero = ?1", &parametres![numero], |l| l.get::<String>(0))
        .unwrap()
        .unwrap();
    assert_eq!(compter(&mut base, "SELECT CAST(COUNT(*) AS BIGINT) FROM ligne_piece WHERE piece_id = ?1",
        &parametres![piece.clone()]), 2, "une ligne par article");
    assert_eq!(compter(&mut base, "SELECT CAST(COALESCE(SUM(montant), 0) AS BIGINT) FROM avoir WHERE piece_id = ?1 AND statut = 'ouvert'",
        &parametres![piece.clone()]), total, "le crédit du client");
    let note: String = base
        .lire_une("SELECT note FROM piece_commerciale WHERE id = ?1", &parametres![piece], |l| l.get::<String>(0))
        .unwrap()
        .unwrap();
    assert!(note.contains(&total.to_string()), "la note dit le total du geste : {note}");
    assert_eq!(caisse(&mut base, "sortie", "remboursement"), 0, "un avoir ne touche pas le tiroir");
}

#[test]
fn un_article_refuse_n_en_laisse_aucun() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let depot = depot_defaut(&mut base);
    let (sucre, riz) = (article_unite(&mut base, "Sucre"), article_unite(&mut base, "Riz local"));
    let comptant = client_generique(&mut base);
    let v = vendre(&mut base, comptant);
    let (s0, r0) = (stock(&mut base, &sucre.0, &depot), stock(&mut base, &riz.0, &depot));

    // Le sucre passerait ; le riz (5 sur 2 vendus) non : rien ne s'écrit.
    let e = retours::enregistrer_retour_groupe_sur_base(
        &mut base, v.id.clone(), vec![rendu(&v.sucre, 1.0), rendu(&v.riz, 5.0)],
        "remboursement".into(), Some("especes".into()), vec![], None, None,
    )
    .unwrap_err();
    assert!(e.contains("Quantité trop élevée"), "{e}");
    assert_eq!(stock(&mut base, &sucre.0, &depot), s0, "le sucre n'est pas remonté");
    assert_eq!(stock(&mut base, &riz.0, &depot), r0);
    assert_eq!(caisse(&mut base, "sortie", "remboursement"), 0, "le tiroir n'a pas bougé");
    assert_eq!(nb(&mut base, "retour"), 0);

    // Les gestes mal formés sont refusés avant d'écrire.
    let vide = retours::enregistrer_retour_groupe_sur_base(
        &mut base, v.id.clone(), vec![], "remboursement".into(), None, vec![], None, None,
    )
    .unwrap_err();
    assert!(vide.contains("Cocher"), "{vide}");
    let double = retours::enregistrer_retour_groupe_sur_base(
        &mut base, v.id.clone(), vec![rendu(&v.sucre, 1.0), rendu(&v.sucre, 1.0)],
        "remboursement".into(), None, vec![], None, None,
    )
    .unwrap_err();
    assert!(double.contains("deux fois"), "{double}");
    let echange_vide = retours::enregistrer_retour_groupe_sur_base(
        &mut base, v.id.clone(), vec![rendu(&v.sucre, 1.0)], "echange".into(), None, vec![], None, None,
    )
    .unwrap_err();
    assert!(echange_vide.contains("prend"), "{echange_vide}");
    // Une ligne d'une autre vente ne se glisse pas dans ce geste.
    let autre_client = client_generique(&mut base);
    let autre = vendre(&mut base, autre_client);
    let melange = retours::enregistrer_retour_groupe_sur_base(
        &mut base, v.id.clone(), vec![rendu(&v.sucre, 1.0), rendu(&autre.riz, 1.0)],
        "remboursement".into(), Some("especes".into()), vec![], None, None,
    )
    .unwrap_err();
    assert!(melange.contains("introuvable"), "{melange}");
    assert_eq!(nb(&mut base, "retour"), 0);
}

#[test]
fn echanger_plusieurs_articles_regle_la_difference_une_fois() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let depot = depot_defaut(&mut base);
    let (sucre, riz) = (article_unite(&mut base, "Sucre"), article_unite(&mut base, "Riz local"));

    // 2 kg de sucre et 1 kg de riz rendus, 2 kg de riz pris : le client
    // de passage recoit la difference en especes.
    let comptant = client_generique(&mut base);
    let v = vendre(&mut base, comptant);
    let (s0, r0) = (stock(&mut base, &sucre.0, &depot), stock(&mut base, &riz.0, &depot));
    let r = retours::enregistrer_retour_groupe_sur_base(
        &mut base, v.id.clone(), vec![rendu(&v.sucre, 2.0), rendu(&v.riz, 1.0)],
        "echange".into(), None, vec![pris(&riz, 2.0)], None, None,
    )
    .unwrap();
    let difference = 2 * sucre.3 + riz.3 - 2 * riz.3;
    assert_eq!(r["reliquat"], difference);
    assert_eq!(stock(&mut base, &sucre.0, &depot), s0 + 2.0, "le sucre revient");
    assert_eq!(stock(&mut base, &riz.0, &depot), r0 + 1.0 - 2.0, "1 kg revient, 2 partent");
    assert_eq!(caisse(&mut base, "sortie", "remboursement_reliquat"), difference, "rendu une fois");

    // Plus cher : 1 kg de sucre rendu, 2 kg de sucre et 1 kg de riz pris.
    let client = client_reel(&mut base);
    let v = vendre(&mut base, client.clone());
    let r = retours::enregistrer_retour_groupe_sur_base(
        &mut base, v.id.clone(), vec![rendu(&v.sucre, 1.0)],
        "echange".into(), Some("especes".into()), vec![pris(&sucre, 2.0), pris(&riz, 1.0)], None, None,
    )
    .unwrap();
    let complement = 2 * sucre.3 + riz.3 - sucre.3;
    assert_eq!(r["reliquat"], -complement);
    assert_eq!(caisse(&mut base, "entree", "complement_echange"), complement, "le client complète");

    // Reliquat laissé en avoir : une pièce, une ligne par article rendu,
    // au prorata, et la somme tombe juste.
    let v = vendre(&mut base, client);
    let r = retours::enregistrer_retour_groupe_sur_base(
        &mut base, v.id.clone(), vec![rendu(&v.sucre, 2.0), rendu(&v.riz, 2.0)],
        "echange".into(), None, vec![pris(&riz, 1.0)], Some("avoir".into()), None,
    )
    .unwrap();
    let reliquat = 2 * sucre.3 + 2 * riz.3 - riz.3;
    assert_eq!(r["reliquat"], reliquat);
    let numero = r["numero_avoir"].as_str().expect("un avoir").to_string();
    let piece: String = base
        .lire_une("SELECT id FROM piece_commerciale WHERE numero = ?1", &parametres![numero], |l| l.get::<String>(0))
        .unwrap()
        .unwrap();
    assert_eq!(compter(&mut base, "SELECT CAST(COALESCE(SUM(montant), 0) AS BIGINT) FROM avoir WHERE piece_id = ?1",
        &parametres![piece.clone()]), reliquat);
    assert_eq!(compter(&mut base, "SELECT CAST(COUNT(*) AS BIGINT) FROM ligne_piece WHERE piece_id = ?1",
        &parametres![piece]), 2);
}

#[test]
fn le_plafond_de_remboursement_se_juge_sur_le_geste() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let (sucre, riz) = (article_unite(&mut base, "Sucre"), article_unite(&mut base, "Riz local"));
    let fanta = auth::creer_utilisateur_sur_base(
        &mut base, "Fanta".into(), "fanta".into(), None, "fanta-secret".into(), "caissier".into(), "test".into(),
    )
    .expect("caissier");
    // Chaque article passe sous le plafond, les deux ensemble non.
    let plafond = sucre.3.max(riz.3);
    plafonds::definir_role_sur(
        &mut base, "caissier",
        Plafonds { remise_max_pct: Some(15.0), remboursement_max: Some(plafond), credit_max: Some(100_000) },
    )
    .unwrap();
    let comptant = client_generique(&mut base);
    let v = vendre(&mut base, comptant);
    {
        let _g = gescom_noyau::auteur::poser(&fanta);
        let e = retours::enregistrer_retour_groupe_sur_base(
            &mut base, v.id.clone(), vec![rendu(&v.sucre, 1.0), rendu(&v.riz, 1.0)],
            "remboursement".into(), Some("especes".into()), vec![], None, None,
        )
        .unwrap_err();
        assert!(e.starts_with("Remboursement de") && e.ends_with("Demander au patron."), "{e}");
    }
    assert_eq!(caisse(&mut base, "sortie", "remboursement"), 0, "rien n'est sorti");
    assert_eq!(nb(&mut base, "retour"), 0);
}

#[test]
fn tout_passe_le_detecteur() {
    let mut base = base_avec_demo();
    base.auditer(true);
    ouvrir_caisse(&mut base);
    let riz = article_unite(&mut base, "Riz local");
    let client = client_reel(&mut base);
    let v = vendre(&mut base, client.clone());
    retours::enregistrer_retour_groupe_sur_base(
        &mut base, v.id.clone(), vec![rendu(&v.sucre, 1.0), rendu(&v.riz, 1.0)],
        "avoir_conserve".into(), None, vec![], None, None,
    )
    .expect("avoir");
    retours::enregistrer_retour_groupe_sur_base(
        &mut base, v.id.clone(), vec![rendu(&v.sucre, 1.0)],
        "echange".into(), Some("especes".into()), vec![pris(&riz, 1.0)], Some("avoir".into()), None,
    )
    .expect("échange");
    let comptant = client_generique(&mut base);
    let w = vendre(&mut base, comptant);
    retours::enregistrer_retour_groupe_sur_base(
        &mut base, w.id, vec![rendu(&w.sucre, 1.0), rendu(&w.riz, 1.0)],
        "remboursement".into(), Some("especes".into()), vec![], None, None,
    )
    .expect("remboursement");
}
