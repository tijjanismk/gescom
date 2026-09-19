//! Le stock bouge une fois, au document qui constate le mouvement.
//!
//! Avant, il sortait a la validation de la facture. Une commande
//! facturee lundi et livree jeudi faisait donc sortir la marchandise
//! lundi, alors qu'elle etait encore empilee dans le magasin.
//!
//! Depuis que le bon de livraison deplace le stock, deux erreurs
//! deviennent possibles et ce sont elles qu'on verifie ici : que la
//! marchandise sorte DEUX fois — une au bon, une a la facture — ou
//! qu'elle ne sorte JAMAIS, parce que chacun croit que l'autre s'en
//! charge.

use rusqlite::Connection;

use gescom_noyau::{argent, livraisons, persistance, pieces};

fn base() -> Connection {
    let conn = Connection::open_in_memory().expect("base en mémoire");
    persistance::initialiser_tables(&conn).expect("schéma");
    conn.execute_batch(
        "INSERT INTO depot (id, nom, est_defaut, actif, cree_le, modifie_le, origine)
           VALUES ('d1', 'Principal', 1, 1, '2026-01-01', '2026-01-01', 'test');
         INSERT INTO article (id, nom, unite_base, actif, cree_le, modifie_le, origine)
           VALUES ('a1', 'Ciment', 'sac', 1, '2026-01-01', '2026-01-01', 'test');
         INSERT INTO unite_vente
           (id, article_id, libelle, facteur, prix_reference, actif, cree_le, modifie_le, origine)
           VALUES ('u1', 'a1', 'sac', 1, 5000, 1, '2026-01-01', '2026-01-01', 'test');
         INSERT INTO client
           (id, code, nom, est_generique, actif, cree_le, modifie_le, origine)
           VALUES ('c1', 'CLIENT00001', 'Awa', 0, 1,
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

/// Stock de depart, pose par un mouvement — jamais en ecrivant le
/// compteur, qui n'est qu'un cache depuis l'etape 2.
fn poser_stock(conn: &Connection, quantite: f64) {
    conn.execute(
        "INSERT INTO mouvement_stock
           (id, article_id, depot_id, type_mouvement, quantite_delta,
            auteur_id, date_mouvement, cree_le, cree_par, origine)
         VALUES ('m-init', 'a1', 'd1', 'entree', ?1, 'p1',
                 '2026-01-01', '2026-01-01', 'p1', 'test')",
        rusqlite::params![quantite],
    )
    .unwrap();
}

fn stock(conn: &Connection) -> f64 {
    conn.query_row(
        "SELECT COALESCE(SUM(quantite_delta), 0) FROM mouvement_stock
         WHERE article_id = 'a1' AND depot_id = 'd1'",
        [],
        |r| r.get(0),
    )
    .unwrap_or(0.0)
}

fn ligne(quantite: f64) -> pieces::LignePieceInput {
    pieces::LignePieceInput {
        article_id: "a1".to_string(),
        unite_vente_id: "u1".to_string(),
        quantite,
        prix_unitaire: 5000,
        remise_pct: 0.0,
        taux_tva: 0.0,
    }
}

fn creer(conn: &Connection, type_piece: &str, quantite: f64) -> String {
    let v = pieces::creer_piece(
        conn,
        "c1".to_string(),
        type_piece.to_string(),
        vec![ligne(quantite)],
        None,
        None,
        None,
        None,
        Some("d1".to_string()),
        None,
    )
    .expect("pièce créée");
    v["id"].as_str().unwrap().to_string()
}

/// Un bon se prepare en brouillon et ne bouge rien ; c'est son EMISSION
/// qui livre tout. La saisie ligne a ligne vient ensuite, pour corriger.
fn emettre(conn: &Connection, piece_id: &str) {
    pieces::changer_statut_piece(conn, piece_id.to_string(), "emis".to_string()).expect("émettre");
}

fn lignes_de(conn: &Connection, piece_id: &str) -> Vec<String> {
    let mut st = conn
        .prepare("SELECT id FROM ligne_piece WHERE piece_id = ?1")
        .unwrap();
    let v = st
        .query_map(rusqlite::params![piece_id], |r| r.get::<_, String>(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    v
}

fn livrer(conn: &mut Connection, piece_id: &str, quantite: f64) -> serde_json::Value {
    let ligne_id = lignes_de(conn, piece_id).remove(0);
    livraisons::enregistrer_livraison(
        conn,
        piece_id.to_string(),
        vec![livraisons::LigneLivraison {
            ligne_id,
            quantite_livree: quantite,
        }],
    )
    .expect("livraison enregistrée")
}

// =====================================================================
//  Le bon de livraison sort la marchandise
// =====================================================================

#[test]
fn le_bon_de_livraison_sort_ce_qui_part() {
    let mut conn = base();
    poser_stock(&conn, 100.0);
    let bl = creer(&conn, "bon_livraison", 10.0);

    assert_eq!(stock(&conn), 100.0, "créer le bon ne sort rien : il se prépare");
    // Livrer sur un brouillon : refus, rien ne bouge.
    let ligne_id = lignes_de(&conn, &bl).remove(0);
    let refus = livraisons::enregistrer_livraison(
        &mut conn, bl.clone(),
        vec![livraisons::LigneLivraison { ligne_id, quantite_livree: 10.0 }],
    ).unwrap_err();
    assert!(refus.contains("émettre"), "{refus}");
    assert_eq!(stock(&conn), 100.0);

    emettre(&conn, &bl);
    assert_eq!(stock(&conn), 90.0, "l'émission livre tout");
    let r = livrer(&mut conn, &bl, 10.0);
    assert_eq!(r["etat"], "livre");
    assert_eq!(stock(&conn), 90.0, "resaisir la même quantité ne bouge rien");
}

#[test]
fn une_livraison_partielle_ne_sort_que_ce_qui_part() {
    // Le bon est emis (tout sort), puis le livreur revient : six sacs
    // n'ont pas ete pris. La correction les fait rentrer.
    let mut conn = base();
    poser_stock(&conn, 100.0);
    let bl = creer(&conn, "bon_livraison", 10.0);
    emettre(&conn, &bl);

    let r = livrer(&mut conn, &bl, 4.0);
    assert_eq!(r["etat"], "partiel");
    assert_eq!(stock(&conn), 96.0, "quatre sacs partis, six en magasin");
}

#[test]
fn le_stock_bouge_de_l_ecart_pas_du_total() {
    // C'est l'erreur qui coute le plus cher : corriger « 6 livrés » en
    // « 7 livrés » doit sortir UN sac, pas sept.
    let mut conn = base();
    poser_stock(&conn, 100.0);
    let bl = creer(&conn, "bon_livraison", 10.0);
    emettre(&conn, &bl);
    assert_eq!(stock(&conn), 90.0);

    livrer(&mut conn, &bl, 6.0);
    assert_eq!(stock(&conn), 94.0, "quatre sacs rentrent");

    livrer(&mut conn, &bl, 7.0);
    assert_eq!(stock(&conn), 93.0, "un sac de plus, pas sept");
}

#[test]
fn corriger_une_livraison_a_la_baisse_rend_la_marchandise() {
    // Le livreur revient : deux sacs n'ont pas ete acceptes.
    let mut conn = base();
    poser_stock(&conn, 100.0);
    let bl = creer(&conn, "bon_livraison", 10.0);
    emettre(&conn, &bl);
    assert_eq!(stock(&conn), 90.0);

    livrer(&mut conn, &bl, 8.0);
    assert_eq!(stock(&conn), 92.0, "les deux sacs refusés rentrent");
}

#[test]
fn on_ne_peut_pas_livrer_plus_que_commande() {
    let mut conn = base();
    poser_stock(&conn, 100.0);
    let bl = creer(&conn, "bon_livraison", 10.0);
    emettre(&conn, &bl);

    livrer(&mut conn, &bl, 999.0);
    assert_eq!(stock(&conn), 90.0, "plafonné à la quantité du bon");
}

#[test]
fn le_bon_de_reception_fait_entrer_la_marchandise() {
    // Meme geste, sens oppose : cote fournisseur la marchandise arrive.
    let mut conn = base();
    poser_stock(&conn, 20.0);
    let brf = pieces::creer_piece_fournisseur(
        &conn,
        "f1".to_string(),
        "bon_reception".to_string(),
        vec![ligne(15.0)],
        None,
        None,
        None,
        None,
        None,
    )
    .expect("BRF créé");
    let brf = brf["id"].as_str().unwrap().to_string();

    assert_eq!(stock(&conn), 20.0, "en brouillon, rien n'est entré");
    emettre(&conn, &brf);
    assert_eq!(stock(&conn), 35.0, "l'émission fait entrer la marchandise");
    livrer(&mut conn, &brf, 15.0);
    assert_eq!(stock(&conn), 35.0);
}

// =====================================================================
//  La facture ne sort pas ce que le bon a deja sorti
// =====================================================================

#[test]
fn une_facture_issue_d_un_bon_ne_sort_rien() {
    // L'erreur a ne pas commettre : sortir deux fois la meme
    // marchandise, une au bon et une a la facture.
    let mut conn = base();
    poser_stock(&conn, 100.0);
    let bl = creer(&conn, "bon_livraison", 10.0);
    emettre(&conn, &bl);
    assert_eq!(stock(&conn), 90.0);

    let facture = pieces::convertir_piece(&conn, bl.clone(), "facture".to_string())
        .expect("conversion");
    let facture_id = facture["id"].as_str().unwrap().to_string();
    assert!(pieces::stock_confie_a_un_bon(&conn, &facture_id));

    argent::valider_facture_sur(
        &mut conn,
        facture_id,
        "credit".to_string(),
        None,
        None,
        Some("patron".to_string()),
    )
    .expect("facture validée");

    assert_eq!(stock(&conn), 90.0, "la marchandise ne sort pas deux fois");
}

#[test]
fn une_facture_sans_bon_sort_le_stock_comme_avant() {
    // L'autre erreur : que plus personne ne sorte le stock. C'est le
    // cas de toutes les pieces deja en base, et du commercant qui
    // remet la marchandise au comptoir.
    let mut conn = base();
    poser_stock(&conn, 100.0);
    let facture = creer(&conn, "facture", 10.0);
    assert!(!pieces::stock_confie_a_un_bon(&conn, &facture));

    argent::valider_facture_sur(
        &mut conn,
        facture,
        "credit".to_string(),
        None,
        None,
        Some("patron".to_string()),
    )
    .expect("facture validée");

    assert_eq!(stock(&conn), 90.0);
}

#[test]
fn un_bon_en_brouillon_ne_se_facture_pas_et_rien_ne_sort() {
    // Un bon cree a la main nait en brouillon : rien n'est parti. Le
    // facturer dans cet etat donnerait une facture dont la marchandise
    // ne sortirait jamais (la facture issue d'un bon ne bouge rien) :
    // refus, et les sacs sont encore la. Une fois emis, il se facture.
    let mut conn = base();
    poser_stock(&conn, 100.0);
    let bl = creer(&conn, "bon_livraison", 10.0);
    assert_eq!(stock(&conn), 100.0, "en brouillon, rien ne sort");

    let refus = pieces::convertir_piece(&conn, bl.clone(), "facture".to_string()).unwrap_err();
    assert!(refus.contains("émettre"), "{refus}");
    assert_eq!(stock(&conn), 100.0, "rien n'est parti, rien n'est sorti");

    pieces::changer_statut_piece(&conn, bl.clone(), "emis".to_string()).expect("émettre");
    assert_eq!(stock(&conn), 90.0, "l'émission sort la marchandise");
    let facture = pieces::convertir_piece(&conn, bl, "facture".to_string()).expect("conversion");
    argent::valider_facture_sur(
        &mut conn,
        facture["id"].as_str().unwrap().to_string(),
        "credit".to_string(),
        None,
        None,
        Some("patron".to_string()),
    )
    .expect("facture validée");
    assert_eq!(stock(&conn), 90.0, "la facture ne sort pas une seconde fois");
}

#[test]
fn la_chaine_commande_bon_facture_ne_sort_qu_une_fois() {
    // Le cas complet : commande -> bon -> facture. Le bon est au
    // milieu, il faut le trouver en remontant deux crans.
    let mut conn = base();
    poser_stock(&conn, 100.0);
    let commande = creer(&conn, "commande_client", 10.0);

    let bl = pieces::convertir_piece(&conn, commande, "bon_livraison".to_string())
        .expect("commande -> BL");
    let bl = bl["id"].as_str().unwrap().to_string();
    livrer(&mut conn, &bl, 10.0);
    assert_eq!(stock(&conn), 90.0);

    let facture = pieces::convertir_piece(&conn, bl, "facture".to_string())
        .expect("BL -> facture");
    argent::valider_facture_sur(
        &mut conn,
        facture["id"].as_str().unwrap().to_string(),
        "credit".to_string(),
        None,
        None,
        Some("patron".to_string()),
    )
    .expect("facture validée");

    assert_eq!(stock(&conn), 90.0);
}

#[test]
fn une_commande_ne_bouge_aucun_stock() {
    // Seuls les bons constatent un mouvement physique. Enregistrer une
    // « livraison » sur une commande ne doit rien sortir : la commande
    // ne constate rien.
    let mut conn = base();
    poser_stock(&conn, 100.0);
    let commande = creer(&conn, "commande_client", 10.0);

    livrer(&mut conn, &commande, 10.0);
    assert_eq!(stock(&conn), 100.0);
}


/// L'action combinee « commande -> BL + facture », en un geste.
///
/// Elle marque les deux pieces entierement livrees. Le BL doit donc
/// sortir la marchandise, et la facture ne rien sortir du tout — sinon
/// ce raccourci sort le double de ce qu'un commercant a mis dans son
/// camion.
#[test]
fn livrer_et_facturer_en_un_geste_ne_sort_qu_une_fois() {
    let mut conn = base();
    poser_stock(&conn, 100.0);
    let commande = creer(&conn, "commande_client", 10.0);

    let r = pieces::convertir_commande_en_livraison_et_facture(&mut conn, commande)
        .expect("commande -> BL + facture");

    assert_eq!(stock(&conn), 90.0, "dix sacs sortis, pas vingt");

    // Et valider la facture qui suit n'en sort pas dix de plus.
    argent::valider_facture_sur(
        &mut conn,
        r["facture"]["id"].as_str().unwrap().to_string(),
        "credit".to_string(),
        None,
        None,
        Some("patron".to_string()),
    )
    .expect("facture validée");
    assert_eq!(stock(&conn), 90.0);
}

// =====================================================================
//  Le mouvement doit etre VERIFIABLE
// =====================================================================

/// Un mouvement qu'on ne peut pas rattacher a son document ne sert a
/// rien.
///
/// L'historique de stock retrouve le numero en joignant sur le TYPE et
/// `operation_id`. Ecrire une livraison en « vente » ferait chercher un
/// identifiant de piece dans la table des ventes : le mouvement
/// s'afficherait sans numero, et le commercant n'aurait aucun moyen de
/// savoir d'ou viennent ses dix sacs manquants.
#[test]
fn un_mouvement_de_livraison_porte_son_numero_de_bon() {
    let mut conn = base();
    poser_stock(&conn, 100.0);
    let bl = creer(&conn, "bon_livraison", 10.0);
    let numero: String = conn
        .query_row(
            "SELECT numero FROM piece_commerciale WHERE id = ?1",
            rusqlite::params![bl],
            |r| r.get(0),
        )
        .unwrap();
    // L'emission ecrit le mouvement ; le resaisir a la meme quantite
    // n'en ecrit pas un second.
    emettre(&conn, &bl);
    livrer(&mut conn, &bl, 10.0);

    let (type_mouvement, operation): (String, String) = conn
        .query_row(
            "SELECT type_mouvement, COALESCE(operation_id, '')
             FROM mouvement_stock WHERE id <> 'm-init'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .expect("un mouvement écrit");

    assert_eq!(type_mouvement, gescom_noyau::coeur::stock::LIVRAISON);
    assert_eq!(operation, bl, "le mouvement pointe sur le bon");

    // Et l'historique affiche bien ce numéro.
    let hist = gescom_noyau::depots::lire_mouvements_stock(
        &conn, None, None, None, None, None, Some(50),
    )
    .expect("historique");
    let ligne = hist
        .iter()
        .find(|m| m["type"] == "livraison")
        .expect("la livraison figure dans l'historique");
    assert_eq!(ligne["numero_facture"], numero);
    assert_eq!(ligne["libelle"], "Livraison");
    assert_eq!(ligne["entrant"], false);
}

#[test]
fn une_reception_porte_son_numero_et_entre() {
    let mut conn = base();
    let brf = pieces::creer_piece_fournisseur(
        &conn,
        "f1".to_string(),
        "bon_reception".to_string(),
        vec![ligne(15.0)],
        None,
        None,
        None,
        None,
        None,
    )
    .expect("BRF créé");
    let brf = brf["id"].as_str().unwrap().to_string();
    emettre(&conn, &brf);

    let hist = gescom_noyau::depots::lire_mouvements_stock(
        &conn, None, None, None, None, None, Some(50),
    )
    .expect("historique");
    let ligne = hist
        .iter()
        .find(|m| m["type"] == "reception")
        .expect("la réception figure dans l'historique");
    assert_eq!(ligne["libelle"], "Réception");
    assert_eq!(ligne["entrant"], true);
    assert_ne!(ligne["numero_facture"], "");
}
