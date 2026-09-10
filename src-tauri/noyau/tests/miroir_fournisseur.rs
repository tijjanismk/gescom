//! Le fournisseur se gere comme le client, en miroir.
//!
//! Cote client : facture en brouillon, puis VALIDATION qui sort le
//! stock et encaisse. Cote fournisseur il n'y avait qu'un geste unique,
//! `enregistrer_achat`, et la chaine s'arretait au bon de reception —
//! la conversion BRF -> FAF etait refusee, faute d'une etape capable de
//! produire la dette et le decaissement.
//!
//! Ces scenarios verifient les deux erreurs que le miroir rend
//! possibles : que la marchandise entre ou que l'argent sorte DEUX
//! fois, ou qu'aucun des deux n'arrive parce que chacun croit que
//! l'autre s'en charge.

use rusqlite::Connection;

use gescom_noyau::{achats, livraisons, persistance, pieces};

fn base() -> Connection {
    let conn = Connection::open_in_memory().expect("base en mémoire");
    persistance::initialiser_tables(&conn).expect("schéma");
    conn.execute_batch(
        "INSERT INTO depot (id, nom, est_defaut, actif, cree_le, modifie_le, origine)
           VALUES ('d1', 'Principal', 1, 1, '2026-01-01', '2026-01-01', 'test');
         INSERT INTO article (id, nom, unite_base, actif, cree_le, modifie_le, origine)
           VALUES ('a1', 'Ciment', 'sac', 1, '2026-01-01', '2026-01-01', 'test');
         INSERT INTO unite_vente
           (id, article_id, libelle, facteur, prix_reference, actif,
            cree_le, modifie_le, origine)
           VALUES ('u1', 'a1', 'sac', 1, 5000, 1,
                   '2026-01-01', '2026-01-01', 'test');
         INSERT INTO fournisseur (id, nom, actif, cree_le, modifie_le, origine)
           VALUES ('f1', 'Ciments du Mali', 1, '2026-01-01', '2026-01-01', 'test');
         INSERT INTO role (id, nom, cree_le, modifie_le)
           VALUES ('patron', 'patron', '2026-01-01', '2026-01-01');
         INSERT INTO utilisateur (id, nom, role_id, actif, cree_le, modifie_le, origine)
           VALUES ('p1', 'Patron', 'patron', 1, '2026-01-01', '2026-01-01', 'test');",
    )
    .expect("jeu d'essai");
    conn
}

fn stock(conn: &Connection) -> f64 {
    conn.query_row(
        "SELECT COALESCE(SUM(quantite_delta), 0) FROM mouvement_stock
         WHERE article_id = 'a1'",
        [],
        |r| r.get(0),
    )
    .unwrap_or(0.0)
}

/// Ce que le fournisseur a reellement encaisse, tous paiements
/// confondus. C'est le chiffre qui doit rester juste : un double
/// decaissement se voit ici et nulle part ailleurs.
fn regle(conn: &Connection) -> i64 {
    conn.query_row(
        "SELECT COALESCE(SUM(montant), 0) FROM paiement_fournisseur",
        [],
        |r| r.get(0),
    )
    .unwrap_or(0)
}

fn ligne(quantite: f64, prix: i64) -> pieces::LignePieceInput {
    pieces::LignePieceInput {
        article_id: "a1".to_string(),
        unite_vente_id: "u1".to_string(),
        quantite,
        prix_unitaire: prix,
        remise_pct: 0.0,
        taux_tva: 0.0,
    }
}

fn creer_fournisseur(conn: &Connection, type_piece: &str, quantite: f64) -> String {
    let v = pieces::creer_piece_fournisseur(
        conn,
        "f1".to_string(),
        type_piece.to_string(),
        vec![ligne(quantite, 4000)],
        None,
        None,
        None,
        None,
    )
    .expect("pièce fournisseur créée");
    v["id"].as_str().unwrap().to_string()
}

fn ouvrir_caisse(conn: &Connection) {
    conn.execute(
        "INSERT INTO session_caisse
           (id, statut, fond_ouverture, cree_le, modifie_le, origine)
         VALUES ('s1', 'ouverte', 0, '2026-01-01', '2026-01-01', 'test')",
        [],
    )
    .expect("caisse ouverte");
}

fn recevoir(conn: &mut Connection, piece_id: &str, quantite: f64) {
    let ligne_id: String = conn
        .query_row(
            "SELECT id FROM ligne_piece WHERE piece_id = ?1",
            rusqlite::params![piece_id],
            |r| r.get(0),
        )
        .unwrap();
    livraisons::enregistrer_livraison(
        conn,
        piece_id.to_string(),
        vec![livraisons::LigneLivraison {
            ligne_id,
            quantite_livree: quantite,
        }],
    )
    .expect("réception enregistrée");
}

// =====================================================================
//  La chaine complete : BCF -> BRF -> FAF
// =====================================================================

#[test]
fn la_chaine_fournisseur_va_maintenant_jusqu_a_la_facture() {
    // C'etait un bouton mort : l'ecran proposait « → Facture fourn. »
    // sur un bon de reception, et le noyau repondait « conversion non
    // autorisée ».
    let conn = base();
    let bcf = creer_fournisseur(&conn, "bon_commande_fournisseur", 10.0);

    let brf = pieces::convertir_piece(&conn, bcf, "bon_reception".to_string())
        .expect("BCF -> BRF");
    let brf = brf["id"].as_str().unwrap().to_string();

    let faf = pieces::convertir_piece(&conn, brf, "facture_fournisseur".to_string())
        .expect("BRF -> FAF");

    assert!(faf["numero"].as_str().unwrap().starts_with("FAF-"));
    assert_eq!(
        faf["statut"], "brouillon",
        "elle naît en brouillon : la dette demande un geste explicite"
    );
}

#[test]
fn une_facture_issue_d_un_bon_de_reception_ne_fait_pas_entrer_deux_fois() {
    // L'erreur a ne pas commettre : la marchandise entre a la
    // reception, puis une seconde fois a la facturation.
    let mut conn = base();
    let brf = creer_fournisseur(&conn, "bon_reception", 10.0);
    recevoir(&mut conn, &brf, 10.0);
    assert_eq!(stock(&conn), 10.0);

    let faf = pieces::convertir_piece(&conn, brf, "facture_fournisseur".to_string())
        .expect("BRF -> FAF");
    let faf = faf["id"].as_str().unwrap().to_string();

    let r = achats::valider_facture_fournisseur(
        &mut conn,
        faf,
        "credit".to_string(),
        None,
        None,
        Some("patron".to_string()),
    )
    .expect("FAF validée");

    assert_eq!(r["stock_entre"], false, "le bon l'a déjà fait entrer");
    assert_eq!(stock(&conn), 10.0, "dix sacs, pas vingt");
    assert_eq!(r["total"], 40_000);
    assert_eq!(r["statut"], "emis", "à crédit : la dette reste");
}

#[test]
fn une_facture_sans_bon_fait_entrer_la_marchandise_elle_meme() {
    // L'autre erreur : que plus personne ne fasse entrer le stock.
    let mut conn = base();
    let faf = creer_fournisseur(&conn, "facture_fournisseur", 10.0);

    let r = achats::valider_facture_fournisseur(
        &mut conn,
        faf,
        "credit".to_string(),
        None,
        None,
        Some("patron".to_string()),
    )
    .expect("FAF validée");

    assert_eq!(r["stock_entre"], true);
    assert_eq!(stock(&conn), 10.0);
}

// =====================================================================
//  L'argent
// =====================================================================

#[test]
fn une_facture_comptant_sort_l_argent_une_seule_fois() {
    let mut conn = base();
    ouvrir_caisse(&conn);
    let faf = creer_fournisseur(&conn, "facture_fournisseur", 10.0);

    let r = achats::valider_facture_fournisseur(
        &mut conn,
        faf,
        "comptant".to_string(),
        Some("especes".to_string()),
        None,
        Some("patron".to_string()),
    )
    .expect("FAF validée");

    assert_eq!(r["statut"], "paye");
    assert_eq!(regle(&conn), 40_000);

    let sorties: i64 = conn
        .query_row(
            "SELECT COALESCE(SUM(montant), 0) FROM mouvement_caisse
             WHERE sens = 'sortie'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(sorties, 40_000, "le tiroir sort exactement le montant payé");
}

#[test]
fn un_acompte_ne_sort_que_l_acompte() {
    let mut conn = base();
    ouvrir_caisse(&conn);
    let faf = creer_fournisseur(&conn, "facture_fournisseur", 10.0);

    let r = achats::valider_facture_fournisseur(
        &mut conn,
        faf,
        "credit".to_string(),
        Some("especes".to_string()),
        Some(15_000),
        Some("patron".to_string()),
    )
    .expect("FAF validée");

    assert_eq!(r["statut"], "emis", "il reste une dette");
    assert_eq!(regle(&conn), 15_000);
}

#[test]
fn caisse_fermee_la_facture_comptant_est_refusee() {
    // D46 : refuser, plutot qu'ecrire la dette sans sa sortie de
    // caisse. Sinon le comptage du soir tombe faux d'exactement ce
    // montant, sans rien pour l'expliquer.
    let mut conn = base();
    let faf = creer_fournisseur(&conn, "facture_fournisseur", 10.0);

    let refus = achats::valider_facture_fournisseur(
        &mut conn,
        faf,
        "comptant".to_string(),
        Some("especes".to_string()),
        None,
        Some("patron".to_string()),
    );
    assert!(refus.is_err(), "{refus:?}");
    assert_eq!(regle(&conn), 0, "rien n'a été écrit");
    assert_eq!(stock(&conn), 0.0);
}

#[test]
fn une_facture_deja_validee_ne_se_valide_pas_deux_fois() {
    // Deux clics sur « valider » doubleraient la dette.
    let mut conn = base();
    let faf = creer_fournisseur(&conn, "facture_fournisseur", 10.0);

    achats::valider_facture_fournisseur(
        &mut conn,
        faf.clone(),
        "credit".to_string(),
        None,
        None,
        Some("patron".to_string()),
    )
    .expect("première validation");

    let refus = achats::valider_facture_fournisseur(
        &mut conn,
        faf,
        "credit".to_string(),
        None,
        None,
        Some("patron".to_string()),
    );
    assert!(refus.is_err(), "{refus:?}");
    assert_eq!(stock(&conn), 10.0, "la marchandise n'entre pas deux fois");
}

// =====================================================================
//  L'avoir direct
// =====================================================================

#[test]
fn une_facture_fournisseur_se_convertit_en_avoir() {
    let mut conn = base();
    let faf = creer_fournisseur(&conn, "facture_fournisseur", 10.0);
    achats::valider_facture_fournisseur(
        &mut conn,
        faf.clone(),
        "credit".to_string(),
        None,
        None,
        Some("patron".to_string()),
    )
    .expect("FAF validée");
    assert_eq!(stock(&conn), 10.0);

    achats::annuler_facture_fournisseur_par_avoir(
        &mut conn,
        faf.clone(),
        None,
        None,
        None,
        Some("patron".to_string()),
    )
    .expect("annulation par avoir");

    assert_eq!(stock(&conn), 0.0, "la marchandise repart");

    let statut: String = conn
        .query_row(
            "SELECT statut FROM piece_commerciale WHERE id = ?1",
            rusqlite::params![faf],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(statut, "annule");

    let avf: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM piece_commerciale
             WHERE type_piece = 'avoir_fournisseur'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(avf, 1, "un avoir fournisseur, et un seul");
}

#[test]
fn un_brouillon_ne_s_annule_pas_par_avoir() {
    // Un brouillon n'a produit NI dette NI stock : lui fabriquer un
    // avoir inventerait un credit chez un fournisseur a qui l'on ne
    // doit rien (D42, meme raisonnement que le retour sans facture).
    let mut conn = base();
    let faf = creer_fournisseur(&conn, "facture_fournisseur", 10.0);

    let refus = achats::annuler_facture_fournisseur_par_avoir(
        &mut conn,
        faf,
        None,
        None,
        None,
        Some("patron".to_string()),
    );
    assert!(refus.is_err(), "{refus:?}");
    let avf: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM piece_commerciale
             WHERE type_piece = 'avoir_fournisseur'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(avf, 0);
}

#[test]
fn une_facture_client_ne_se_valide_pas_par_le_chemin_fournisseur() {
    let mut conn = base();
    conn.execute(
        "INSERT INTO client
           (id, code, nom, est_generique, actif, cree_le, modifie_le, origine)
         VALUES ('c1', 'CLIENT00001', 'Awa', 0, 1,
                 '2026-01-01', '2026-01-01', 'test')",
        [],
    )
    .unwrap();
    let facture = pieces::creer_piece(
        &conn,
        "c1".to_string(),
        "facture".to_string(),
        vec![ligne(1.0, 1000)],
        None,
        None,
        None,
        None,
        Some("d1".to_string()),
    )
    .unwrap();

    let refus = achats::valider_facture_fournisseur(
        &mut conn,
        facture["id"].as_str().unwrap().to_string(),
        "credit".to_string(),
        None,
        None,
        Some("patron".to_string()),
    );
    assert!(refus.is_err(), "{refus:?}");
}
