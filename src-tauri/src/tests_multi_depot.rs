//! Scenarios multi-depot, joues sur les VRAIES fonctions.
//!
//! Les tests du `coeur` verifient des formules. Ceux-ci verifient ce que
//! le SQL fait a la base : quel depot est decremente, ce qui reste apres
//! un refus, ce qu'un retour rend et a qui. C'est la seule facon
//! d'attraper les bugs corriges ici — aucun ne vivait dans une formule,
//! tous vivaient dans une requete.
//!
//! Chaque test part d'une base neuve en memoire, semee comme au premier
//! lancement, puis appelle les fonctions `*_sur` — celles-la memes que
//! les commandes Tauri appellent une fois le verrou pris.

use rusqlite::Connection;

use crate::commandes::ventes::ParamsLigneInput;

// =====================================================================
//  Banc d'essai
// =====================================================================

struct Banc {
    conn: Connection,
    /// Depot par defaut, cree par le seed.
    principal: String,
    /// Second depot, cree par le test.
    annexe: String,
    article: String,
    unite: String,
    /// Conditionnement : 1 sac = 50 unites de base.
    unite_sac: String,
    client: String,
}

impl Banc {
    /// Quantite en stock, en unites de base.
    fn stock(&self, depot: &str) -> f64 {
        self.conn.query_row(
            "SELECT COALESCE(quantite, 0) FROM stock_depot
             WHERE article_id = ?1 AND depot_id = ?2",
            rusqlite::params![self.article, depot],
            |r| r.get(0),
        ).unwrap_or(0.0)
    }

    fn compte(&self, sql: &str) -> i64 {
        self.conn.query_row(sql, [], |r| r.get(0)).unwrap_or(-1)
    }

    fn executer(&self, sql: &str) {
        self.conn.execute(sql, []).unwrap();
    }

    /// Ouvre une session de caisse — sans elle, tout encaissement est
    /// refuse, et c'est justement l'un des cas testes.
    fn ouvrir_caisse(&self) {
        self.executer(
            "INSERT INTO session_caisse
             (id, statut, fond_ouverture, cree_le, modifie_le, origine)
             VALUES ('sess-test', 'ouverte', 0, '2026-01-01', '2026-01-01', 'test')"
        );
    }

    /// Pose un stock de depart PAR UN MOUVEMENT, comme le ferait une
    /// entree reelle.
    ///
    /// Ecrire le compteur directement laisserait les tests eprouver un
    /// chemin que le logiciel n'emprunte plus : depuis que le stock est
    /// la consequence de ses mouvements, un stock sans mouvement est
    /// precisement l'incoherence qu'on cherche a rendre impossible.
    fn poser_stock(&self, depot: &str, quantite: f64) {
        let actuel = self.stock(depot);
        let delta = quantite - actuel;
        if delta == 0.0 {
            return;
        }
        self.conn.execute(
            "INSERT INTO mouvement_stock
             (id, article_id, depot_id, type_mouvement, quantite_delta,
              motif, auteur_id, date_mouvement, cree_le, cree_par, origine)
             VALUES (?1,?2,?3,'ajustement',?4,'stock de depart','test',
                     '2026-01-01','2026-01-01','test','test')",
            rusqlite::params![
                uuid::Uuid::new_v4().to_string(), self.article, depot, delta
            ],
        ).unwrap();
    }

    /// Lignes d'une vente, dans l'ordre d'insertion.
    fn lignes_vente(&self, vente_id: &str) -> Vec<(String, String, f64)> {
        let mut st = self.conn.prepare(
            "SELECT id, depot_source_id, quantite FROM ligne_vente
             WHERE vente_id = ?1 ORDER BY rowid"
        ).unwrap();
        let v = st.query_map(rusqlite::params![vente_id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        }).unwrap().filter_map(|r| r.ok()).collect();
        v
    }
}

fn banc() -> Banc {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
    crate::persistance::initialiser_tables(&conn).unwrap();
    crate::seed::seeder(&conn).unwrap();

    let now = "2026-01-01T08:00:00";

    let principal: String = conn.query_row(
        "SELECT id FROM depot WHERE est_defaut = 1", [], |r| r.get(0),
    ).unwrap();

    let annexe = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO depot (id, nom, est_defaut, actif, cree_le, modifie_le, origine)
         VALUES (?1, 'Djelibougou', 0, 1, ?2, ?2, 'test')",
        rusqlite::params![annexe, now],
    ).unwrap();

    let article = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO article
         (id, nom, unite_base, gere_en_stock, attributs, actif,
          cree_le, modifie_le, cree_par, modifie_par, origine)
         VALUES (?1, 'Ciment', 'kilo', 1, '{}', 1, ?2, ?2, 'test', 'test', 'test')",
        rusqlite::params![article, now],
    ).unwrap();

    let unite = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO unite_vente
         (id, article_id, libelle, facteur, prix_reference, actif,
          cree_le, modifie_le, cree_par, modifie_par, origine)
         VALUES (?1, ?2, 'kilo', 1.0, 500, 1, ?3, ?3, 'test', 'test', 'test')",
        rusqlite::params![unite, article, now],
    ).unwrap();

    let unite_sac = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO unite_vente
         (id, article_id, libelle, facteur, prix_reference, actif,
          cree_le, modifie_le, cree_par, modifie_par, origine)
         VALUES (?1, ?2, 'sac', 50.0, 24000, 1, ?3, ?3, 'test', 'test', 'test')",
        rusqlite::params![unite_sac, article, now],
    ).unwrap();

    let client = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO client
         (id, code, nom, est_generique, actif,
          cree_le, modifie_le, cree_par, modifie_par, origine)
         VALUES (?1, 'CLIENT00042', 'Amadou', 0, 1, ?2, ?2, 'test', 'test', 'test')",
        rusqlite::params![client, now],
    ).unwrap();

    Banc { conn, principal, annexe, article, unite, unite_sac, client }
}

fn ligne(
    b: &Banc, depot: &str, quantite: f64, a_decouvert: bool,
) -> ParamsLigneInput {
    ParamsLigneInput {
        article_id: b.article.clone(),
        unite_vente_id: b.unite.clone(),
        depot_source_id: depot.to_string(),
        source_approvisionnement: "stock".to_string(),
        quantite,
        facteur: 1.0,
        prix_reference: 500,
        prix_pratique: 500,
        taux_tva: Some(0.0),
        a_decouvert: Some(a_decouvert),
    }
}

// =====================================================================
//  1. Le stock lu est celui du depot demande
// =====================================================================

#[test]
fn le_stock_lu_suit_le_depot_demande() {
    let b = banc();
    b.poser_stock(&b.principal, 10.0);
    b.poser_stock(&b.annexe, 300.0);

    let lire = |depot: Option<String>| -> f64 {
        let arts = crate::commandes::ventes::lire_articles_avec_unites_sur(
            &b.conn, Some("patron".to_string()), depot,
        ).unwrap();
        arts.iter()
            .find(|a| a["id"] == serde_json::json!(b.article))
            .unwrap()["stock"].as_f64().unwrap()
    };

    assert_eq!(lire(Some(b.principal.clone())), 10.0);
    assert_eq!(lire(Some(b.annexe.clone())), 300.0, "c'etait LE bug : 10 partout");
    // Sans depot, on retombe sur le defaut — comportement historique.
    assert_eq!(lire(None), 10.0);
    // Un depot inconnu ne fait pas echouer l'ecran.
    assert_eq!(lire(Some("depot-fantome".to_string())), 10.0);
}

// =====================================================================
//  2. Vente repartie : chaque depot baisse de sa part
// =====================================================================

#[test]
fn vente_repartie_puise_dans_chaque_depot() {
    let mut b = banc();
    b.poser_stock(&b.principal, 4.0);
    b.poser_stock(&b.annexe, 20.0);
    b.ouvrir_caisse();

    // Les lectures de `b` sont sorties avant l'appel : la fonction prend
    // la connexion en `&mut`, elle ne peut pas cohabiter avec elles.
    let (client, principal, annexe) =
        (b.client.clone(), b.principal.clone(), b.annexe.clone());
    let lignes = vec![
        ligne(&b, &principal, 4.0, false),
        ligne(&b, &annexe, 6.0, false),
    ];

    let r = crate::commandes::ventes::creer_vente_sur(&mut b.conn,
        client, principal, "comptant".to_string(), lignes,
        Some("patron".to_string()), Some(5000), Some("especes".to_string()), None,
    ).unwrap();

    assert_eq!(r["total"], serde_json::json!(5000));
    assert_eq!(b.stock(&b.principal), 0.0);
    assert_eq!(b.stock(&b.annexe), 14.0, "le second depot doit baisser de 6");
}

// =====================================================================
//  3. Annulation par avoir : chaque ligne revient dans SON depot
// =====================================================================

#[test]
fn annulation_par_avoir_rend_a_chaque_depot_sa_part() {
    let mut b = banc();
    b.poser_stock(&b.principal, 4.0);
    b.poser_stock(&b.annexe, 20.0);
    b.ouvrir_caisse();

    let (client, principal, annexe) =
        (b.client.clone(), b.principal.clone(), b.annexe.clone());
    let lignes = vec![
        ligne(&b, &principal, 4.0, false),
        ligne(&b, &annexe, 6.0, false),
    ];

    let vente = crate::commandes::ventes::creer_vente_sur(&mut b.conn,
        client.clone(), principal, "credit".to_string(), lignes,
        Some("patron".to_string()), None, None, None,
    ).unwrap();
    let vente_id = vente["vente_id"].as_str().unwrap().to_string();

    let piece = crate::commandes::pieces_pos::creer_facture_depuis_vente_sur(
        &b.conn, vente_id.clone(), client,
        "credit".to_string(), Some("patron".to_string()),
    ).unwrap();
    let piece_id = piece["piece_id"].as_str().unwrap().to_string();

    crate::commandes::pieces::annuler_facture_par_avoir_sur(&mut b.conn, piece_id, Some("avoir".to_string()), None,
        Some("erreur de saisie".to_string()),
    ).unwrap();

    assert_eq!(b.stock(&b.principal), 4.0);
    assert_eq!(b.stock(&b.annexe), 20.0,
        "la part sortie de l'annexe revenait au principal");
}

// =====================================================================
//  4. Retour refuse : la base ne garde AUCUNE trace
// =====================================================================

#[test]
fn retour_refuse_ne_laisse_ni_stock_remonte_ni_ligne() {
    let mut b = banc();
    b.poser_stock(&b.principal, 10.0);
    b.ouvrir_caisse();

    let (client, principal) = (b.client.clone(), b.principal.clone());
    let lignes = vec![ligne(&b, &principal, 2.0, false)];

    let vente = crate::commandes::ventes::creer_vente_sur(&mut b.conn,
        client, principal, "comptant".to_string(), lignes,
        Some("patron".to_string()), Some(1000), Some("especes".to_string()), None,
    ).unwrap();
    let vente_id = vente["vente_id"].as_str().unwrap().to_string();
    let lignes = b.lignes_vente(&vente_id);
    let ligne_id = lignes[0].0.clone();

    assert_eq!(b.stock(&b.principal), 8.0);

    // La caisse ferme : le remboursement devient impossible.
    b.executer("UPDATE session_caisse SET statut = 'fermee'");

    let erreur = crate::commandes::retours::enregistrer_retour_sur(&mut b.conn, vente_id.clone(), ligne_id.clone(), 2.0,
        "remboursement".to_string(), Some("especes".to_string()),
        None, None, None, None, None,
    );

    assert!(erreur.is_err(), "sans caisse ouverte, le remboursement doit echouer");
    assert_eq!(b.stock(&b.principal), 8.0,
        "le stock etait remonte avant le refus, et y restait");
    assert_eq!(b.compte("SELECT COUNT(*) FROM retour"), 0,
        "la ligne de retour survivait au refus et bloquait la 2e tentative");

    // Caisse rouverte : la meme operation passe, une seule fois.
    b.executer("UPDATE session_caisse SET statut = 'ouverte'");
    crate::commandes::retours::enregistrer_retour_sur(&mut b.conn, vente_id, ligne_id, 2.0,
        "remboursement".to_string(), Some("especes".to_string()),
        None, None, None, None, None,
    ).unwrap();

    assert_eq!(b.stock(&b.principal), 10.0);
    assert_eq!(b.compte("SELECT COUNT(*) FROM retour"), 1);
}

// =====================================================================
//  5. Retour d'une vente repartie : les deux lignes sont retournables
// =====================================================================

#[test]
fn retourner_une_ligne_ne_bloque_pas_sa_jumelle() {
    let mut b = banc();
    b.poser_stock(&b.principal, 4.0);
    b.poser_stock(&b.annexe, 20.0);
    b.ouvrir_caisse();

    let (client, principal, annexe) =
        (b.client.clone(), b.principal.clone(), b.annexe.clone());
    let lignes_vendues = vec![
        ligne(&b, &principal, 4.0, false),
        ligne(&b, &annexe, 6.0, false),
    ];

    let vente = crate::commandes::ventes::creer_vente_sur(&mut b.conn,
        client, principal, "comptant".to_string(), lignes_vendues,
        Some("patron".to_string()), Some(5000), Some("especes".to_string()), None,
    ).unwrap();
    let vente_id = vente["vente_id"].as_str().unwrap().to_string();
    let lignes = b.lignes_vente(&vente_id);
    assert_eq!(lignes.len(), 2);

    // Ligne du depot principal, retournee en entier.
    crate::commandes::retours::enregistrer_retour_sur(&mut b.conn, vente_id.clone(), lignes[0].0.clone(), 4.0,
        "remboursement".to_string(), Some("especes".to_string()),
        None, None, None, None, None,
    ).unwrap();

    // Ligne de l'annexe : le meme article, mais une AUTRE ligne.
    crate::commandes::retours::enregistrer_retour_sur(&mut b.conn, vente_id.clone(), lignes[1].0.clone(), 6.0,
        "remboursement".to_string(), Some("especes".to_string()),
        None, None, None, None, None,
    ).unwrap();

    assert_eq!(b.stock(&b.principal), 4.0);
    assert_eq!(b.stock(&b.annexe), 20.0);

    // La garde tient toujours : au-dela du vendu, c'est non.
    let trop = crate::commandes::retours::enregistrer_retour_sur(&mut b.conn, vente_id, lignes[1].0.clone(), 1.0,
        "remboursement".to_string(), Some("especes".to_string()),
        None, None, None, None, None,
    );
    assert!(trop.is_err(), "on ne retourne pas plus qu'on n'a vendu");
}

// =====================================================================
//  6. Transfert : le controle porte sur le total par article
// =====================================================================

#[test]
fn transfert_refuse_le_meme_article_scinde_en_deux_lignes() {
    use crate::commandes::transferts::LigneTransfert;
    let mut b = banc();
    b.poser_stock(&b.principal, 15.0);

    let (principal, annexe, article, unite) = (
        b.principal.clone(), b.annexe.clone(),
        b.article.clone(), b.unite.clone(),
    );
    let une_ligne = |quantite: f64| LigneTransfert {
        article_id: article.clone(), unite_vente_id: unite.clone(),
        quantite, facteur: 1.0,
    };

    let deux_lignes = vec![une_ligne(10.0), une_ligne(10.0)];
    let r = crate::commandes::transferts::enregistrer_transfert_sur(&mut b.conn,
        principal.clone(), annexe.clone(), deux_lignes,
        None, Some("patron".to_string()),
    );
    assert!(r.is_err(), "20 demandes sur 15 disponibles : refus attendu");
    assert_eq!(b.stock(&b.principal), 15.0);
    assert_eq!(b.stock(&b.annexe), 0.0);

    // Une quantite negative ne doit pas se faufiler sous le controle.
    let ligne_negative = vec![une_ligne(-5.0)];
    let negatif = crate::commandes::transferts::enregistrer_transfert_sur(&mut b.conn,
        principal.clone(), annexe.clone(), ligne_negative,
        None, Some("patron".to_string()),
    );
    assert!(negatif.is_err());
    assert_eq!(b.stock(&b.annexe), 0.0);

    // Ce qui tient dans le stock passe.
    let ligne_tenable = vec![une_ligne(7.0)];
    crate::commandes::transferts::enregistrer_transfert_sur(&mut b.conn,
        principal, annexe, ligne_tenable,
        None, Some("patron".to_string()),
    ).unwrap();
    assert_eq!(b.stock(&b.principal), 8.0);
    assert_eq!(b.stock(&b.annexe), 7.0);
}

// =====================================================================
//  7. Facture de l'ecran Pieces : bon depot, decouvert signale
// =====================================================================

#[test]
fn facture_validee_sort_du_depot_de_la_piece() {
    use crate::commandes::pieces::LignePieceInput;
    let mut b = banc();
    b.poser_stock(&b.principal, 100.0);
    b.poser_stock(&b.annexe, 3.0);
    b.ouvrir_caisse();

    let piece = crate::commandes::pieces::creer_piece_sur(
        &b.conn, b.client.clone(), "facture".to_string(),
        vec![LignePieceInput {
            article_id: b.article.clone(),
            unite_vente_id: b.unite.clone(),
            quantite: 5.0, prix_unitaire: 500,
            remise_pct: 0.0, taux_tva: 0.0,
        }],
        None, None, None, None,
        // La piece est etablie a l'annexe.
        Some(b.annexe.clone()),
    ).unwrap();
    // creer_piece renvoie la cle `id`, pas `piece_id` comme le POS.
    let piece_id = piece["id"].as_str().unwrap().to_string();

    crate::commandes::pieces::valider_facture_sur(&mut b.conn, piece_id, "comptant".to_string(),
        Some("especes".to_string()), None, Some("patron".to_string()),
    ).unwrap();

    assert_eq!(b.stock(&b.principal), 100.0, "le principal ne doit pas bouger");
    assert_eq!(b.stock(&b.annexe), -2.0, "5 sortis d'un depot qui en avait 3");
    assert_eq!(
        b.compte("SELECT COUNT(*) FROM ligne_vente WHERE vente_a_decouvert = 1"),
        1,
        "sans ce drapeau, le decouvert n'est jamais regularise"
    );
}

// =====================================================================
//  8. Fermeture d'un depot : refus, gel, puis reouverture
// =====================================================================

#[test]
fn depot_ferme_sur_stock_puis_rouvert_retrouve_sa_marchandise() {
    let b = banc();
    b.poser_stock(&b.annexe, 12.0);

    let refus = crate::commandes::depots::desactiver_depot_sur(
        &b.conn, b.annexe.clone(), None,
    );
    assert!(refus.is_err(), "sans force, le stock restant bloque la fermeture");

    crate::commandes::depots::desactiver_depot_sur(
        &b.conn, b.annexe.clone(), Some(true),
    ).unwrap();

    assert_eq!(
        b.compte(&format!(
            "SELECT actif FROM depot WHERE id = '{}'", b.annexe)),
        0
    );
    assert_eq!(b.stock(&b.annexe), 12.0, "le stock est gele, pas efface");
    assert_eq!(
        b.compte("SELECT COUNT(*) FROM journal
                  WHERE type_evenement = 'depot_desactive_avec_stock'"),
        1,
        "une fermeture sur stock non vide doit laisser une trace"
    );

    // Le depot ferme ne s'offre plus comme source de vente.
    let visibles =
        crate::commandes::depots::lire_stock_multi_depots_sur(&b.conn).unwrap();
    assert!(
        !visibles.iter().any(|d| d["depot_id"] == serde_json::json!(b.annexe)),
        "un depot ferme ne doit plus apparaitre dans les sources"
    );

    crate::commandes::depots::reactiver_depot_sur(&b.conn, b.annexe.clone()).unwrap();
    assert_eq!(b.stock(&b.annexe), 12.0, "la marchandise revient intacte");
    assert_eq!(
        b.compte(&format!(
            "SELECT est_defaut FROM depot WHERE id = '{}'", b.annexe)),
        0,
        "rouvrir un depot ne le fait pas redevenir le depot par defaut"
    );

    // Deux reactivations d'affilee : la seconde n'a plus rien a faire.
    assert!(crate::commandes::depots::reactiver_depot_sur(&b.conn, b.annexe.clone()).is_err());
}

// =====================================================================
//  9. Le depot par defaut reste protege
// =====================================================================

#[test]
fn le_depot_par_defaut_ne_se_ferme_pas_meme_de_force() {
    let b = banc();
    let r = crate::commandes::depots::desactiver_depot_sur(
        &b.conn, b.principal.clone(), Some(true),
    );
    assert!(r.is_err(), "fermer le depot par defaut couperait le POS");
}

// =====================================================================
//  10. Unites : le stock est en base, la vente en sacs
// =====================================================================

#[test]
fn vendre_un_sac_sort_cinquante_kilos_du_bon_depot() {
    let mut b = banc();
    b.poser_stock(&b.annexe, 120.0);
    b.ouvrir_caisse();

    let (client, principal) = (b.client.clone(), b.principal.clone());
    let lignes = vec![ParamsLigneInput {
            article_id: b.article.clone(),
            unite_vente_id: b.unite_sac.clone(),
            depot_source_id: b.annexe.clone(),
            source_approvisionnement: "stock".to_string(),
            quantite: 2.0,
            facteur: 50.0,
            prix_reference: 24000,
            prix_pratique: 24000,
            taux_tva: Some(0.0),
            a_decouvert: Some(false),
    }];

    crate::commandes::ventes::creer_vente_sur(&mut b.conn,
        client, principal, "comptant".to_string(), lignes,
        Some("patron".to_string()), Some(48000), Some("especes".to_string()), None,
    ).unwrap();

    assert_eq!(b.stock(&b.annexe), 20.0, "2 sacs = 100 kilos");
    assert_eq!(b.stock(&b.principal), 0.0);
}

// =====================================================================
//  11. Le decouvert est constate en base, pas declare par l'ecran
// =====================================================================

/// Le POS calcule `a_decouvert` sur le stock lu au chargement de
/// l'ecran. Entre ce chargement et la vente, quelqu'un a pu vendre le
/// dernier sac — au comptoir a cote, ou depuis une autre caisse. Le
/// POS envoie alors `false` de bonne foi.
///
/// Si on le croyait, le stock passerait a -1 SANS que la ligne
/// apparaisse dans « ventes a decouvert » : un decouvert invisible,
/// donc jamais regularise. C'est ce que ce test interdit.
#[test]
fn un_ecran_perime_ne_cache_pas_le_decouvert() {
    let mut b = banc();
    b.poser_stock(&b.principal, 1.0);
    b.ouvrir_caisse();

    let (client, principal) = (b.client.clone(), b.principal.clone());
    // L'ecran croit qu'il reste 3 : il annonce false.
    let lignes = vec![ligne(&b, &principal, 3.0, false)];

    crate::commandes::ventes::creer_vente_sur(&mut b.conn,
        client, principal.clone(), "comptant".to_string(), lignes,
        Some("patron".to_string()), Some(1500), Some("especes".to_string()), None,
    ).unwrap();

    // La vente PASSE — on ne refuse pas un client au comptoir (D32 ne
    // vaut que pour les transferts).
    assert_eq!(b.stock(&principal), -2.0, "la marchandise est sortie");
    // Mais elle est signalee.
    assert_eq!(
        b.compte("SELECT COUNT(*) FROM ligne_vente WHERE vente_a_decouvert = 1"),
        1,
        "le decouvert doit etre constate en base, meme si l'ecran dit non"
    );
}

/// Le cas inverse : l'ecran ne doit pas non plus creer un faux
/// decouvert quand le stock suffit. Sinon la liste « a regulariser »
/// se remplit de lignes qui n'ont rien a regulariser, et le commercant
/// cesse de la lire.
#[test]
fn une_vente_couverte_n_est_pas_signalee() {
    let mut b = banc();
    b.poser_stock(&b.principal, 10.0);
    b.ouvrir_caisse();

    let (client, principal) = (b.client.clone(), b.principal.clone());
    let lignes = vec![ligne(&b, &principal, 3.0, false)];

    crate::commandes::ventes::creer_vente_sur(&mut b.conn,
        client, principal.clone(), "comptant".to_string(), lignes,
        Some("patron".to_string()), Some(1500), Some("especes".to_string()), None,
    ).unwrap();

    assert_eq!(b.stock(&principal), 7.0);
    assert_eq!(
        b.compte("SELECT COUNT(*) FROM ligne_vente WHERE vente_a_decouvert = 1"),
        0,
    );
}

// =====================================================================
//  12. Le retour fournisseur sort du bon depot, et pas a decouvert
// =====================================================================

/// Cree un fournisseur et une facture d'achat rattachee a un depot.
///
/// La facture porte son `depot_id` depuis l'origine : c'est elle qui
/// dit ou la marchandise est entree, donc d'ou elle doit repartir.
fn facture_achat(b: &Banc, depot: &str, quantite: f64) -> (String, String) {
    let fournisseur = uuid::Uuid::new_v4().to_string();
    let piece = uuid::Uuid::new_v4().to_string();
    let now = "2026-01-01T08:00:00.000";

    b.conn.execute(
        "INSERT INTO fournisseur
         (id, nom, actif, cree_le, modifie_le, origine)
         VALUES (?1, 'Ets Diallo', 1, ?2, ?2, 'test')",
        rusqlite::params![fournisseur, now],
    ).unwrap();

    b.conn.execute(
        "INSERT INTO piece_commerciale
         (id, type_piece, numero, statut, tiers_type, tiers_id, depot_id,
          auteur_id, date_piece, remise_globale, cree_le, modifie_le, origine)
         VALUES (?1, 'facture_fournisseur', 'FAF-TEST-1', 'emis',
                 'fournisseur', ?2, ?3, 'test', ?4, 0.0, ?4, ?4, 'test')",
        rusqlite::params![piece, fournisseur, depot, now],
    ).unwrap();

    b.conn.execute(
        "INSERT INTO ligne_piece
         (id, piece_id, article_id, unite_vente_id, quantite,
          prix_unitaire, remise_pct, remise_montant,
          taux_tva, montant_tva, montant_ht, cree_le)
         VALUES (?1, ?2, ?3, ?4, ?5, 4000, 0.0, 0, 0.0, 0, ?6, ?7)",
        rusqlite::params![
            uuid::Uuid::new_v4().to_string(), piece, b.article, b.unite,
            quantite, (quantite * 4000.0) as i64, now
        ],
    ).unwrap();

    (fournisseur, piece)
}

fn ligne_retour(b: &Banc, quantite: f64)
    -> crate::commandes::achats::LigneRetourFournisseur
{
    crate::commandes::achats::LigneRetourFournisseur {
        article_id: b.article.clone(),
        unite_vente_id: b.unite.clone(),
        quantite,
        facteur: 1.0,
        prix_achat: 4000,
    }
}

/// La marchandise repart d'ou elle est entree.
///
/// Sans cette regle, le retour sortait du depot PAR DEFAUT : le magasin
/// annexe gardait un stock qu'il n'avait plus, le principal descendait
/// pour une marchandise qu'il n'avait jamais eue. Les deux faux, et
/// aucun des deux ne le disait.
#[test]
fn un_retour_fournisseur_sort_du_depot_de_la_facture() {
    let mut b = banc();
    b.poser_stock(&b.annexe, 20.0);
    b.poser_stock(&b.principal, 50.0);
    let (fournisseur, piece) = facture_achat(&b, &b.annexe, 20.0);
    let lignes = vec![ligne_retour(&b, 5.0)];

    crate::commandes::achats::enregistrer_retour_fournisseur_sur(
        &mut b.conn,
        fournisseur,
        // Aucun depot precise — c'est ce que fait l'ecran.
        None,
        lignes,
        Some(piece),
        Some("avoir".to_string()),
        None,
        None,
        Some("patron".to_string()),
    ).unwrap();

    assert_eq!(b.stock(&b.annexe), 15.0, "la marchandise repart de l'annexe");
    assert_eq!(b.stock(&b.principal), 50.0, "le principal ne bouge pas");
}

/// On ne retourne pas ce qu'on n'a pas.
///
/// Une vente a decouvert se constate — le client attend, la
/// marchandise est souvent la. Un retour fournisseur, non : elle doit
/// physiquement quitter la boutique.
#[test]
fn un_retour_fournisseur_refuse_le_decouvert() {
    let mut b = banc();
    b.poser_stock(&b.annexe, 3.0);
    let (fournisseur, piece) = facture_achat(&b, &b.annexe, 50.0);
    let lignes = vec![ligne_retour(&b, 50.0)];

    let erreur = crate::commandes::achats::enregistrer_retour_fournisseur_sur(
        &mut b.conn,
        fournisseur,
        None,
        lignes,
        Some(piece),
        Some("avoir".to_string()),
        None,
        None,
        Some("patron".to_string()),
    ).unwrap_err();

    assert!(erreur.contains("Stock insuffisant"), "{erreur}");

    // Le refus doit etre TOTAL : ni stock entame, ni avoir cree, ni
    // mouvement. Un refus qui laisse des traces est pire qu'un refus
    // absent — il faut ensuite deviner ce qui a ete ecrit.
    assert_eq!(b.stock(&b.annexe), 3.0);
    assert_eq!(
        b.compte("SELECT COUNT(*) FROM piece_commerciale
                  WHERE type_piece = 'avoir_fournisseur'"),
        0,
    );
    assert_eq!(
        b.compte("SELECT COUNT(*) FROM mouvement_stock
                  WHERE type_mouvement = 'retour_fournisseur'"),
        0,
    );
}

/// Deux lignes du meme article se cumulent avant le controle.
///
/// Prises separement, trois puis trois passent sur un stock de quatre.
/// Ensemble, elles ne doivent pas.
#[test]
fn deux_lignes_du_meme_article_se_cumulent() {
    let mut b = banc();
    b.poser_stock(&b.annexe, 4.0);
    let (fournisseur, piece) = facture_achat(&b, &b.annexe, 10.0);
    let lignes = vec![ligne_retour(&b, 3.0), ligne_retour(&b, 3.0)];

    let erreur = crate::commandes::achats::enregistrer_retour_fournisseur_sur(
        &mut b.conn,
        fournisseur,
        None,
        lignes,
        Some(piece),
        Some("avoir".to_string()),
        None,
        None,
        Some("patron".to_string()),
    ).unwrap_err();

    assert!(erreur.contains("Stock insuffisant"), "{erreur}");
    assert_eq!(b.stock(&b.annexe), 4.0);
}

/// Un retour qui tient exactement dans le stock passe.
///
/// Le controle doit refuser le depassement, pas le cas limite : sinon
/// on ne peut plus retourner le dernier sac d'une livraison entiere.
#[test]
fn un_retour_egal_au_stock_passe() {
    let mut b = banc();
    b.poser_stock(&b.annexe, 5.0);
    let (fournisseur, piece) = facture_achat(&b, &b.annexe, 5.0);
    let lignes = vec![ligne_retour(&b, 5.0)];

    crate::commandes::achats::enregistrer_retour_fournisseur_sur(
        &mut b.conn,
        fournisseur,
        None,
        lignes,
        Some(piece),
        Some("avoir".to_string()),
        None,
        None,
        Some("patron".to_string()),
    ).unwrap();

    assert_eq!(b.stock(&b.annexe), 0.0);
    assert_eq!(
        b.compte("SELECT COUNT(*) FROM piece_commerciale
                  WHERE type_piece = 'avoir_fournisseur'"),
        1,
    );
}

// =====================================================================
//  13. Le retour d'une marchandise entree SANS facture
// =====================================================================

/// Rendre au voisin les dix sacs empruntes vendredi ne doit creer
/// aucun avoir.
///
/// Un avoir viendrait en deduction de la dette fournisseur. Or cette
/// marchandise n'a jamais ete facturee (D42) : on ne doit rien. Lui
/// fabriquer un avoir inventerait un credit chez un fournisseur a qui
/// l'on ne doit rien, et il serait deduit d'autres factures.
#[test]
fn un_retour_sans_facture_ne_cree_ni_piece_ni_dette() {
    let b = banc();
    b.poser_stock(&b.annexe, 10.0);

    crate::commandes::fournisseurs::enregistrer_retour_sans_facture_sur(
        &b.conn,
        b.article.clone(),
        Some(b.annexe.clone()),
        4.0,
        None,
        Some("Rendu au voisin".to_string()),
        Some("patron".to_string()),
    ).unwrap();

    assert_eq!(b.stock(&b.annexe), 6.0, "la marchandise sort");
    assert_eq!(
        b.compte("SELECT COUNT(*) FROM piece_commerciale"),
        0,
        "aucune piece : rien n'a jamais ete facture",
    );
    assert_eq!(
        b.compte("SELECT COUNT(*) FROM paiement_fournisseur"),
        0,
        "aucun mouvement d'argent",
    );
    assert_eq!(
        b.compte("SELECT COUNT(*) FROM mouvement_caisse"),
        0,
        "le tiroir ne bouge pas",
    );
    // La seule trace exploitable, puisqu'il n'y a pas de document.
    assert_eq!(
        b.compte("SELECT COUNT(*) FROM mouvement_stock
                  WHERE type_mouvement = 'retour_fournisseur'"),
        1,
    );
    assert_eq!(
        b.compte("SELECT COUNT(*) FROM journal
                  WHERE type_evenement = 'retour_sans_facture'"),
        1,
    );
}

#[test]
fn un_retour_sans_facture_refuse_le_decouvert() {
    let b = banc();
    b.poser_stock(&b.annexe, 3.0);

    let erreur = crate::commandes::fournisseurs::enregistrer_retour_sans_facture_sur(
        &b.conn,
        b.article.clone(),
        Some(b.annexe.clone()),
        10.0,
        None,
        None,
        Some("patron".to_string()),
    ).unwrap_err();

    assert!(erreur.contains("Stock insuffisant"), "{erreur}");
    assert_eq!(b.stock(&b.annexe), 3.0, "rien ne bouge sur un refus");
}

#[test]
fn un_retour_sans_facture_sort_du_depot_demande() {
    let b = banc();
    b.poser_stock(&b.principal, 8.0);
    b.poser_stock(&b.annexe, 8.0);

    crate::commandes::fournisseurs::enregistrer_retour_sans_facture_sur(
        &b.conn,
        b.article.clone(),
        Some(b.annexe.clone()),
        5.0,
        None,
        None,
        Some("patron".to_string()),
    ).unwrap();

    assert_eq!(b.stock(&b.annexe), 3.0);
    assert_eq!(b.stock(&b.principal), 8.0, "l'autre magasin ne bouge pas");
}

#[test]
fn une_quantite_nulle_est_refusee() {
    // Un retour de zero n'est pas une operation : l'enregistrer
    // remplirait le journal de lignes qui ne disent rien.
    let b = banc();
    b.poser_stock(&b.annexe, 5.0);
    assert!(
        crate::commandes::fournisseurs::enregistrer_retour_sans_facture_sur(
            &b.conn, b.article.clone(), Some(b.annexe.clone()), 0.0,
            None, None, Some("patron".to_string()),
        ).is_err()
    );
}
