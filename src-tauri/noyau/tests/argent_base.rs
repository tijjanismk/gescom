//! Scénarios dédiés à `creer_vente_sur_base` et `valider_facture_sur_base`.
//!
//! ETAPES.md les avait mises de côté exprès : « 280 et 290 lignes de
//! logique d'argent, où une erreur ne se corrige pas par un clic —
//! elles méritent leur propre séance et leurs propres scénarios, pas
//! d'être expédiées à la fin d'une autre. » Ce fichier est cette
//! séance. Il vérifie ce que le SQL fait RÉELLEMENT à la base — stock,
//! caisse, avoir, cloisonnement — pas seulement que la fonction rend
//! `Ok`, dans l'esprit de `tests_multi_depot` côté SQLite.

use gescom_noyau::amorcage;
use gescom_noyau::argent::{self, ParamsLigneInput};
use gescom_noyau::base::Base;
use gescom_noyau::parametres;
use gescom_noyau::utils::maintenant_iso;

fn base_avec_demo() -> Base {
    let mut base = Base::ouvrir(":memory:").expect("base en mémoire");
    amorcage::amorcer(&mut base).expect("amorçage");
    amorcage::donnees_demo(&mut base).expect("démo");
    base
}

fn ouvrir_caisse(base: &mut Base) {
    let dossier = base.dossier().to_string();
    let now = maintenant_iso();
    base.executer(
        "INSERT INTO session_caisse
           (id, statut, fond_ouverture, cree_le, modifie_le, origine, dossier_id)
         VALUES (?1,'ouverte',0,?2,?2,'test',?3)",
        &parametres![uuid::Uuid::new_v4().to_string(), now, dossier],
    )
    .expect("ouvrir la caisse");
}

/// (article_id, unite_vente_id, facteur, prix_reference) de l'unité de
/// BASE d'un article de la démo (facteur 1.0).
fn article_unite(base: &mut Base, nom: &str) -> (String, String, f64, i64) {
    base.lire_une(
        "SELECT a.id, u.id, u.facteur, u.prix_reference
         FROM article a JOIN unite_vente u ON u.article_id = a.id
         WHERE a.nom = ?1 AND u.facteur = 1.0",
        &parametres![nom],
        |r| {
            Ok((
                r.get::<String>(0)?,
                r.get::<String>(1)?,
                r.get::<f64>(2)?,
                r.get::<i64>(3)?,
            ))
        },
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

fn client_reel(base: &mut Base) -> String {
    let dossier = base.dossier().to_string();
    base.lire_une(
        "SELECT id FROM client WHERE est_generique = 0 AND dossier_id = ?1 ORDER BY code LIMIT 1",
        &parametres![dossier],
        |r| r.get::<String>(0),
    )
    .unwrap()
    .unwrap()
}

fn client_generique(base: &mut Base) -> String {
    let dossier = base.dossier().to_string();
    base.lire_une(
        "SELECT id FROM client WHERE est_generique = 1 AND dossier_id = ?1",
        &parametres![dossier],
        |r| r.get::<String>(0),
    )
    .unwrap()
    .unwrap()
}

fn stock(base: &mut Base, article_id: &str, depot_id: &str) -> f64 {
    base.lire_une(
        "SELECT COALESCE(quantite, 0) FROM stock_depot WHERE article_id = ?1 AND depot_id = ?2",
        &parametres![article_id, depot_id],
        |r| r.get::<f64>(0),
    )
    .unwrap()
    .unwrap_or(0.0)
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

// =====================================================================
//  creer_vente_sur_base
// =====================================================================

#[test]
fn une_vente_comptant_solde_encaisse_et_sort_le_stock() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let depot = depot_defaut(&mut base);
    let sucre = article_unite(&mut base, "Sucre");
    let avant = stock(&mut base, &sucre.0, &depot);
    let client = client_generique(&mut base);

    let r = argent::creer_vente_sur_base(
        &mut base,
        client,
        depot.clone(),
        "comptant".into(),
        vec![ligne(&sucre, &depot, 5.0)],
        None,
        Some(4_000),
        Some("especes".into()),
        None,
    )
    .expect("la vente doit s'enregistrer");

    assert_eq!(r["total"], 4_000);
    assert_eq!(r["total_regle"], 4_000);
    assert_eq!(r["reste"], 0);
    assert_eq!(r["statut"], "payee");

    assert_eq!(stock(&mut base, &sucre.0, &depot), avant - 5.0, "le stock doit sortir");

    let paye: i64 = base
        .lire_une(
            "SELECT COALESCE(SUM(montant), 0) FROM paiement WHERE vente_id = ?1",
            &parametres![r["vente_id"].as_str().unwrap()],
            |row| row.get::<i64>(0),
        )
        .unwrap()
        .unwrap();
    assert_eq!(paye, 4_000);

    let en_caisse: i64 = base
        .lire_une(
            "SELECT COALESCE(SUM(montant), 0) FROM mouvement_caisse
             WHERE operation_id = ?1 AND sens = 'entree'",
            &parametres![r["vente_id"].as_str().unwrap()],
            |row| row.get::<i64>(0),
        )
        .unwrap()
        .unwrap();
    assert_eq!(en_caisse, 4_000, "le tiroir doit recevoir la vente");
}

#[test]
fn le_credit_est_refuse_au_client_comptant() {
    let mut base = base_avec_demo();
    let depot = depot_defaut(&mut base);
    let sucre = article_unite(&mut base, "Sucre");
    let client = client_generique(&mut base);

    let err = argent::creer_vente_sur_base(
        &mut base,
        client,
        depot.clone(),
        "credit".into(),
        vec![ligne(&sucre, &depot, 1.0)],
        None,
        None,
        None,
        None,
    )
    .expect_err("D40 : pas de crédit pour un client comptant");
    assert!(err.contains("crédit"), "{err}");
}

#[test]
fn une_vente_a_credit_pour_un_vrai_client_reste_en_creance() {
    let mut base = base_avec_demo();
    let depot = depot_defaut(&mut base);
    let sucre = article_unite(&mut base, "Sucre");
    let client = client_reel(&mut base);

    let r = argent::creer_vente_sur_base(
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
    .expect("une vente à crédit doit s'enregistrer");

    assert_eq!(r["statut"], "creance_ouverte");
    assert_eq!(r["total_regle"], 0);
}

#[test]
fn un_encaissement_exige_une_caisse_ouverte() {
    // Pas de `ouvrir_caisse` ici : c'est le point du test.
    let mut base = base_avec_demo();
    let depot = depot_defaut(&mut base);
    let sucre = article_unite(&mut base, "Sucre");
    let client = client_generique(&mut base);

    let err = argent::creer_vente_sur_base(
        &mut base,
        client,
        depot.clone(),
        "comptant".into(),
        vec![ligne(&sucre, &depot, 1.0)],
        None,
        Some(800),
        None,
        None,
    )
    .expect_err("un encaissement sans caisse ouverte doit être refusé");
    assert!(err.contains("CAISSE_FERMEE"), "{err}");
}

#[test]
fn vendre_au_dela_du_stock_marque_la_ligne_a_decouvert() {
    let mut base = base_avec_demo();
    let depot = depot_defaut(&mut base);
    let sucre = article_unite(&mut base, "Sucre"); // stock démo : 200 kg
    let client = client_generique(&mut base);

    let r = argent::creer_vente_sur_base(
        &mut base,
        client,
        depot.clone(),
        "comptant".into(),
        vec![ligne(&sucre, &depot, 250.0)],
        None,
        None,
        None,
        None,
    )
    .expect("la vente à découvert n'est pas bloquée, seulement signalée");

    let a_decouvert: i64 = base
        .lire_une(
            "SELECT vente_a_decouvert FROM ligne_vente WHERE vente_id = ?1",
            &parametres![r["vente_id"].as_str().unwrap()],
            |row| row.get::<i64>(0),
        )
        .unwrap()
        .unwrap();
    assert_eq!(a_decouvert, 1, "vendre plus que le stock doit lever le drapeau");
}

#[test]
fn un_avoir_plus_grand_que_la_vente_solde_et_laisse_un_reliquat() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let depot = depot_defaut(&mut base);
    let sucre = article_unite(&mut base, "Sucre");
    let client = client_reel(&mut base);
    let dossier = base.dossier().to_string();

    let avoir_id = uuid::Uuid::new_v4().to_string();
    base.executer(
        "INSERT INTO avoir (id, client_id, montant, statut, cree_le, origine, dossier_id)
         VALUES (?1,?2,5000,'ouvert',?3,'test',?4)",
        &parametres![avoir_id.clone(), client.clone(), maintenant_iso(), dossier.clone()],
    )
    .unwrap();

    // 5 kg à 800 F = 4000 F, couverts par un avoir de 5000 F.
    let r = argent::creer_vente_sur_base(
        &mut base,
        client,
        depot.clone(),
        "comptant".into(),
        vec![ligne(&sucre, &depot, 5.0)],
        None,
        None,
        None,
        Some(5_000),
    )
    .expect("la vente réglée par avoir doit s'enregistrer");

    assert_eq!(r["total"], 4_000);
    assert_eq!(r["total_regle"], 4_000, "l'avoir ne règle jamais plus que la vente");
    assert_eq!(r["statut"], "payee");

    let (statut_origine, reliquat): (String, i64) = base
        .lire_une(
            "SELECT statut, (SELECT montant FROM avoir WHERE origine = 'avoir_solde'
                              AND client_id = (SELECT client_id FROM avoir WHERE id = ?1))
             FROM avoir WHERE id = ?1",
            &parametres![avoir_id],
            |row| Ok((row.get::<String>(0)?, row.get::<i64>(1)?)),
        )
        .unwrap()
        .unwrap();
    assert_eq!(statut_origine, "consomme", "l'avoir d'origine est soldé");
    assert_eq!(reliquat, 1_000, "le solde non consommé rouvre un avoir de 1000 F");
}

#[test]
fn deux_dossiers_ne_melangent_pas_leurs_ventes() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let depot = depot_defaut(&mut base);
    let sucre = article_unite(&mut base, "Sucre");
    let client = client_generique(&mut base);

    argent::creer_vente_sur_base(
        &mut base,
        client,
        depot.clone(),
        "comptant".into(),
        vec![ligne(&sucre, &depot, 1.0)],
        None,
        Some(800),
        None,
        None,
    )
    .expect("vente dans le dossier par défaut");

    let chez_b: i64 = base
        .lire_une(
            "SELECT COUNT(*) FROM vente WHERE dossier_id = 'dossier-b'",
            &[],
            |row| row.get::<i64>(0),
        )
        .unwrap()
        .unwrap();
    assert_eq!(chez_b, 0, "dossier-b ne doit voir aucune vente de l'autre dossier");
}

// =====================================================================
//  valider_facture_sur_base
// =====================================================================

/// Crée une facture brouillon avec une ligne, prête à valider. Rend
/// (piece_id, article_id, depot_id, total_ht attendu).
fn facture_brouillon(base: &mut Base, quantite: f64, piece_origine_id: Option<&str>) -> (String, String, String, i64) {
    let dossier = base.dossier().to_string();
    let now = maintenant_iso();
    let depot = depot_defaut(base);
    let client = client_reel(base);
    let sucre = article_unite(base, "Sucre");
    let piece_id = uuid::Uuid::new_v4().to_string();
    let numero = format!("FAC-TEST-{}", &piece_id[..8]);
    let montant_ht = (sucre.3 as f64 * quantite).round() as i64;

    base.executer(
        "INSERT INTO piece_commerciale
           (id, type_piece, numero, statut, tiers_type, tiers_id, depot_id,
            piece_origine_id, auteur_id, date_piece, remise_globale, cree_le,
            modifie_le, origine, dossier_id)
         VALUES (?1,'facture',?2,'brouillon','client',?3,?4,?5,'test',?6,0.0,?6,?6,'test',?7)",
        &parametres![
            piece_id.clone(),
            numero,
            client,
            depot.clone(),
            piece_origine_id,
            now.clone(),
            dossier.clone()
        ],
    )
    .unwrap();

    base.executer(
        "INSERT INTO ligne_piece
           (id, piece_id, article_id, unite_vente_id, quantite, prix_unitaire,
            remise_pct, remise_montant, taux_tva, montant_tva, montant_ht, cree_le,
            dossier_id)
         VALUES (?1,?2,?3,?4,?5,?6,0,0,0,0,?7,?8,?9)",
        &parametres![
            uuid::Uuid::new_v4().to_string(),
            piece_id.clone(),
            sucre.0.clone(),
            sucre.1.clone(),
            quantite,
            sucre.3,
            montant_ht,
            now,
            dossier
        ],
    )
    .unwrap();

    (piece_id, sucre.0, depot, montant_ht)
}

#[test]
fn valider_une_facture_comptant_solde_et_sort_le_stock() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let (piece_id, article_id, depot, total_ht) = facture_brouillon(&mut base, 3.0, None);
    let avant = stock(&mut base, &article_id, &depot);

    let r = argent::valider_facture_sur_base(
        &mut base,
        piece_id.clone(),
        "comptant".into(),
        Some("especes".into()),
        None,
        None,
    )
    .expect("la facture doit se valider");

    assert_eq!(r["total_net"], total_ht);
    assert_eq!(r["statut_piece"], "paye");
    assert_eq!(r["statut_vente"], "payee");
    assert_eq!(stock(&mut base, &article_id, &depot), avant - 3.0);

    let statut_piece: String = base
        .lire_une(
            "SELECT statut FROM piece_commerciale WHERE id = ?1",
            &parametres![piece_id],
            |row| row.get::<String>(0),
        )
        .unwrap()
        .unwrap();
    assert_eq!(statut_piece, "paye");
}

#[test]
fn valider_une_facture_a_credit_avec_acompte_reste_partiellement_payee() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let (piece_id, _article_id, _depot, total_ht) = facture_brouillon(&mut base, 3.0, None);
    let acompte = total_ht / 2;

    let r = argent::valider_facture_sur_base(
        &mut base,
        piece_id,
        "credit".into(),
        Some("especes".into()),
        Some(acompte),
        None,
    )
    .expect("la facture avec acompte doit se valider");

    assert_eq!(r["statut_piece"], "emis", "un reste dû garde la pièce ouverte");
    assert_eq!(r["statut_vente"], "partiellement_payee");
}

#[test]
fn valider_facture_refuse_une_piece_deja_validee() {
    let mut base = base_avec_demo();
    let (piece_id, ..) = facture_brouillon(&mut base, 1.0, None);
    base.executer(
        "UPDATE piece_commerciale SET statut = 'paye' WHERE id = ?1",
        &parametres![piece_id.clone()],
    )
    .unwrap();

    let err = argent::valider_facture_sur_base(
        &mut base,
        piece_id,
        "comptant".into(),
        None,
        None,
        None,
    )
    .expect_err("une facture déjà validée doit être refusée");
    assert!(err.contains("statut"), "{err}");
}

#[test]
fn valider_facture_refuse_un_devis() {
    let mut base = base_avec_demo();
    let (piece_id, ..) = facture_brouillon(&mut base, 1.0, None);
    base.executer(
        "UPDATE piece_commerciale SET type_piece = 'devis' WHERE id = ?1",
        &parametres![piece_id.clone()],
    )
    .unwrap();

    let err = argent::valider_facture_sur_base(
        &mut base,
        piece_id,
        "comptant".into(),
        None,
        None,
        None,
    )
    .expect_err("seules les factures se valident");
    assert!(err.contains("factures"), "{err}");
}

#[test]
fn valider_facture_ne_sort_pas_le_stock_si_un_bon_l_a_deja_livre() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    let dossier = base.dossier().to_string();

    // Un bon de livraison, brouillon suffit : seul son TYPE compte pour
    // `stock_confie_a_un_bon_sur`.
    let bon_id = uuid::Uuid::new_v4().to_string();
    let now = maintenant_iso();
    base.executer(
        "INSERT INTO piece_commerciale
           (id, type_piece, numero, statut, tiers_type, tiers_id, auteur_id,
            date_piece, cree_le, modifie_le, origine, dossier_id)
         VALUES (?1,'bon_livraison',?2,'transfere','client','x','test',?3,?3,?3,'test',?4)",
        &parametres![bon_id.clone(), format!("BL-TEST-{}", &bon_id[..8]), now, dossier],
    )
    .unwrap();

    let (piece_id, article_id, depot, _total) = facture_brouillon(&mut base, 3.0, Some(&bon_id));
    let avant = stock(&mut base, &article_id, &depot);

    argent::valider_facture_sur_base(
        &mut base,
        piece_id,
        "comptant".into(),
        Some("especes".into()),
        None,
        None,
    )
    .expect("la facture se valide même quand le stock est déjà sorti par le bon");

    assert_eq!(
        stock(&mut base, &article_id, &depot),
        avant,
        "le bon a déjà sorti la marchandise : la facture ne doit rien reprendre"
    );
}

#[test]
fn tout_ce_qui_est_porte_dans_argent_passe_le_detecteur() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    base.auditer(true);
    let depot = depot_defaut(&mut base);
    let sucre = article_unite(&mut base, "Sucre");
    let client = client_generique(&mut base);

    argent::creer_vente_sur_base(
        &mut base,
        client,
        depot.clone(),
        "comptant".into(),
        vec![ligne(&sucre, &depot, 1.0)],
        None,
        Some(800),
        None,
        None,
    )
    .expect("créer_vente doit passer le détecteur");

    let (piece_id, ..) = facture_brouillon(&mut base, 1.0, None);
    argent::valider_facture_sur_base(
        &mut base,
        piece_id,
        "comptant".into(),
        Some("especes".into()),
        None,
        None,
    )
    .expect("valider_facture doit passer le détecteur");
}
