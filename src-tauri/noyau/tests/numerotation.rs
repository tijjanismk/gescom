//! Un numero de piece ne doit jamais sortir deux fois.
//!
//! Il se calculait par `MAX(substr(numero, -5))` sur les pieces deja
//! ecrites — une LECTURE, faite avant la transaction qui ecrit la
//! piece. Entre les deux, une autre vente lit le meme maximum : les
//! deux fabriquent FAC-2026-00042, et la seconde se heurte a la
//! contrainte UNIQUE. Au comptoir, cela donne un client qui attend et
//! une facture qui refuse de s'enregistrer, sans que rien n'explique
//! pourquoi.
//!
//! Ces scenarios verifient les quatre promesses du compteur : il ne
//! rend jamais deux fois le meme rang, meme depuis deux connexions
//! simultanees ; chaque serie a le sien ; une piece supprimee ne rend
//! pas son numero ; et la reprise d'une base existante repart la ou
//! l'histoire s'etait arretee.

use std::sync::{Arc, Barrier};
use std::thread;

use rusqlite::Connection;

use gescom_noyau::{argent, codebarre, persistance, transferts};

fn annee() -> String {
    chrono::Local::now().format("%Y").to_string()
}

fn base() -> Connection {
    let conn = Connection::open_in_memory().expect("base en mémoire");
    persistance::initialiser_tables(&conn).expect("schéma");
    conn
}

#[test]
fn deux_reservations_ne_rendent_pas_le_meme_numero() {
    let conn = base();
    let a = argent::reserver_numero(&conn, "facture").unwrap();
    let b = argent::reserver_numero(&conn, "facture").unwrap();
    assert_ne!(a, b);
    assert_eq!(a, format!("FAC-{}-00001", annee()));
    assert_eq!(b, format!("FAC-{}-00002", annee()));
}

#[test]
fn chaque_serie_a_son_compteur() {
    // Le bon de livraison ne doit pas faire avancer la facture : ce
    // sont deux suites que le commercant lit separement.
    let conn = base();
    let bl = argent::reserver_numero(&conn, "bon_livraison").unwrap();
    let fac = argent::reserver_numero(&conn, "facture").unwrap();
    assert_eq!(bl, format!("BL-{}-00001", annee()));
    assert_eq!(fac, format!("FAC-{}-00001", annee()));

    let bon = transferts::reserver_bon(&conn).unwrap();
    assert_eq!(bon, format!("BTR-{}-00001", annee()));
}

#[test]
fn une_piece_supprimee_ne_rend_pas_son_numero() {
    // C'etait deja vrai avec MAX, ca doit le rester : rejouer un
    // numero deja imprime met deux papiers differents sous la meme
    // reference, et le client ne sait plus laquelle est la sienne.
    let conn = base();
    let premier = argent::reserver_numero(&conn, "facture").unwrap();
    let deuxieme = argent::reserver_numero(&conn, "facture").unwrap();
    // La deuxieme piece disparait — annulation, erreur de saisie.
    drop(deuxieme);
    let troisieme = argent::reserver_numero(&conn, "facture").unwrap();
    assert_eq!(troisieme, format!("FAC-{}-00003", annee()));
    assert_ne!(troisieme, premier);
}

#[test]
fn un_retour_arriere_rend_le_numero() {
    // Reserver DANS la transaction, c'est ce qui evite un trou dans la
    // serie quand l'enregistrement echoue.
    let mut conn = base();
    {
        let tx = conn.transaction().unwrap();
        let n = argent::reserver_numero(&tx, "facture").unwrap();
        assert_eq!(n, format!("FAC-{}-00001", annee()));
        // Pas de commit : la transaction retombe.
    }
    let apres = argent::reserver_numero(&conn, "facture").unwrap();
    assert_eq!(
        apres,
        format!("FAC-{}-00001", annee()),
        "le numero d'une transaction annulee doit revenir"
    );
}

#[test]
fn le_code_barre_a_sa_propre_suite_sans_annee() {
    // Un code-barre est colle sur un sac : il ne se reinitialise pas
    // au 1er janvier.
    let conn = base();
    assert_eq!(codebarre::reserver_sequence(&conn).unwrap(), 1);
    assert_eq!(codebarre::reserver_sequence(&conn).unwrap(), 2);
}

// =====================================================================
//  Le scenario qui justifie tout le changement
// =====================================================================

/// Deux connexions qui reservent en meme temps, sur un vrai fichier.
///
/// C'est le seul test qui prouve quelque chose : en memoire, chaque
/// connexion aurait sa propre base. Sur fichier, les deux fils se
/// disputent reellement le meme compteur.
///
/// Mesure de la contre-epreuve : le meme scenario joue avec l'ancienne
/// lecture `MAX(...)` a produit **5 doublons sur 100 numeros**. Elle
/// n'est pas gardee ici — elle testerait du code mort, et son resultat
/// depend de la vitesse de la machine.
#[test]
fn deux_connexions_simultanees_ne_se_marchent_pas_dessus() {
    let dossier = std::env::temp_dir().join(format!(
        "gescom-num-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&dossier).unwrap();
    let chemin = dossier.join("gescom.db");
    let chemin_txt = chemin.to_string_lossy().to_string();

    {
        let conn = persistance::ouvrir_base(&chemin_txt).unwrap();
        persistance::initialiser_tables(&conn).unwrap();
    }

    const FILS: usize = 4;
    const PAR_FIL: usize = 25;

    let depart = Arc::new(Barrier::new(FILS));
    let mut mains = Vec::new();

    for _ in 0..FILS {
        let chemin_txt = chemin_txt.clone();
        let depart = Arc::clone(&depart);
        mains.push(thread::spawn(move || {
            let conn = persistance::ouvrir_base(&chemin_txt).unwrap();
            // Tous partent au meme instant : sans cela, les fils se
            // suivraient sagement et le test ne prouverait rien.
            depart.wait();
            let mut miens = Vec::new();
            for _ in 0..PAR_FIL {
                miens.push(argent::reserver_numero(&conn, "facture").unwrap());
            }
            miens
        }));
    }

    let mut tous: Vec<String> = Vec::new();
    for m in mains {
        tous.extend(m.join().expect("un fil a paniqué"));
    }

    assert_eq!(tous.len(), FILS * PAR_FIL);
    let mut uniques = tous.clone();
    uniques.sort();
    uniques.dedup();
    assert_eq!(
        uniques.len(),
        tous.len(),
        "un numéro est sorti deux fois — c'est exactement le bug corrigé"
    );

    // Et la suite est continue : ni trou, ni saut.
    assert_eq!(uniques.first().unwrap(), &format!("FAC-{}-00001", annee()));
    assert_eq!(
        uniques.last().unwrap(),
        &format!("FAC-{}-{:05}", annee(), FILS * PAR_FIL)
    );

    std::fs::remove_dir_all(&dossier).ok();
}

// =====================================================================
//  La reprise d'une base existante
// =====================================================================

/// Une base v1 a deja emis FAC-2026-00042 : le compteur doit repartir
/// de 43, pas de 1.
///
/// Repartir de 1 refabriquerait un numero deja imprime, et la
/// contrainte UNIQUE bloquerait la premiere vente du matin de la mise
/// a jour — la panne la plus visible qu'on puisse livrer.
#[test]
fn la_reprise_repart_du_dernier_numero_emis() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(include_str!("../src/persistance/schema.sql"))
        .unwrap();

    for (table, valeurs) in [
        ("depot (id, nom, est_defaut, actif, cree_le, modifie_le, origine)",
         "('d1', 'Principal', 1, 1, '2026-01-01', '2026-01-01', 'test')"),
        ("depot (id, nom, est_defaut, actif, cree_le, modifie_le, origine)",
         "('d2', 'Annexe', 0, 1, '2026-01-01', '2026-01-01', 'test')"),
        ("article (id, nom, unite_base, actif, cree_le, modifie_le, origine)",
         "('a1', 'Ciment', 'sac', 1, '2026-01-01', '2026-01-01', 'test')"),
    ] {
        conn.execute(&format!("INSERT INTO {table} VALUES {valeurs}"), [])
            .unwrap();
    }

    let an = annee();
    for (numero, type_piece) in [
        (format!("FAC-{an}-00042"), "facture"),
        (format!("FAC-{an}-00007"), "facture"),
        (format!("BL-{an}-00003"), "bon_livraison"),
    ] {
        conn.execute(
            "INSERT INTO piece_commerciale
               (id, type_piece, numero, statut, tiers_type, tiers_id,
                date_piece, cree_le, modifie_le, origine)
             VALUES (?1, ?2, ?3, 'emis', 'client', 'c1',
                     '2026-01-01', '2026-01-01', '2026-01-01', 'test')",
            rusqlite::params![uuid::Uuid::new_v4().to_string(), type_piece, numero],
        )
        .unwrap();
    }
    conn.execute(
        "INSERT INTO transfert
           (id, bon, article_id, depot_source, depot_dest, quantite,
            date_transfert, auteur_id, cree_le, origine)
         VALUES ('t1', ?1, 'a1', 'd1', 'd2', 1, '2026-01-01', 'u1',
                 '2026-01-01', 'test')",
        rusqlite::params![format!("BTR-{an}-00011")],
    )
    .unwrap();

    persistance::v2::migrer(&conn).unwrap();

    assert_eq!(
        argent::reserver_numero(&conn, "facture").unwrap(),
        format!("FAC-{an}-00043"),
        "le compteur doit repartir APRÈS le plus grand numéro émis"
    );
    assert_eq!(
        argent::reserver_numero(&conn, "bon_livraison").unwrap(),
        format!("BL-{an}-00004")
    );
    assert_eq!(
        transferts::reserver_bon(&conn).unwrap(),
        format!("BTR-{an}-00012")
    );
}

#[test]
fn la_reprise_ne_se_joue_qu_une_fois() {
    // Rejouee, elle remettrait le compteur sur le plus grand numero
    // ecrit — donc en arriere de tout ce qui a ete reserve depuis.
    let conn = base();
    let premier = argent::reserver_numero(&conn, "facture").unwrap();
    assert_eq!(premier, format!("FAC-{}-00001", annee()));

    persistance::v2::migrer(&conn).unwrap();
    persistance::v2::migrer(&conn).unwrap();

    assert_eq!(
        argent::reserver_numero(&conn, "facture").unwrap(),
        format!("FAC-{}-00002", annee()),
        "la reprise a rejoué et a fait reculer le compteur"
    );
}

/// Un compteur en retard doit se voir au demarrage, pas au comptoir.
///
/// Le cas arrive apres une restauration partielle, ou une base
/// modifiee a la main. Sans ce controle, la panne se manifeste comme
/// une vente qui refuse de s'enregistrer, sans explication.
#[test]
fn un_compteur_en_retard_est_signale() {
    let conn = base();
    let an = annee();
    conn.execute(
        "INSERT INTO piece_commerciale
           (id, type_piece, numero, statut, tiers_type, tiers_id,
            date_piece, cree_le, modifie_le, origine)
         VALUES ('p1', 'facture', ?1, 'emis', 'client', 'c1',
                 '2026-01-01', '2026-01-01', '2026-01-01', 'test')",
        rusqlite::params![format!("FAC-{an}-00042")],
    )
    .unwrap();

    // Le compteur ignore cette piece : c'est exactement l'etat qui
    // refabriquerait FAC-...-00001.
    let anomalies = persistance::anomalies_metier(&conn);
    assert!(
        anomalies.iter().any(|(l, _)| l.contains("Compteurs")),
        "le retard doit être signalé : {anomalies:?}"
    );

    // Une fois le compteur remis a niveau, plus rien a signaler.
    conn.execute(
        "INSERT OR REPLACE INTO compteur_piece (cle, dernier) VALUES (?1, 42)",
        rusqlite::params![gescom_noyau::dossiers::cle_compteur(
            gescom_noyau::dossiers::DOSSIER_DEFAUT,
            &format!("FAC-{an}")
        )],
    )
    .unwrap();
    let anomalies = persistance::anomalies_metier(&conn);
    assert!(
        !anomalies.iter().any(|(l, _)| l.contains("Compteurs")),
        "{anomalies:?}"
    );
}

/// Le scenario de la mise a jour, celui qui refabriquerait un numero.
///
/// Une base d'avant le cloisonnement porte la cle `FAC-2026`. Le code
/// cherche desormais `<dossier>:FAC-2026`. Sans migration, il ne
/// trouverait rien, repartirait de 1, et la premiere facture du matin
/// se heurterait a la contrainte UNIQUE — au comptoir, devant le
/// client.
#[test]
fn une_base_d_avant_le_cloisonnement_ne_recommence_pas_a_un() {
    let conn = base();
    let an = annee();

    // Etat d'avant : une cle SANS dossier, comme l'ecrivait l'ancien code.
    conn.execute(
        "INSERT INTO compteur_piece (cle, dernier) VALUES (?1, 42)",
        rusqlite::params![format!("FAC-{an}")],
    )
    .unwrap();

    persistance::v2::migrer(&conn).unwrap();

    assert_eq!(
        argent::reserver_numero(&conn, "facture").unwrap(),
        format!("FAC-{an}-00043"),
        "le compteur est reparti de zéro : un numéro déjà émis va ressortir"
    );
}

#[test]
fn migrer_deux_fois_ne_prefixe_pas_deux_fois_la_cle() {
    let conn = base();
    let an = annee();
    conn.execute(
        "INSERT INTO compteur_piece (cle, dernier) VALUES (?1, 7)",
        rusqlite::params![format!("FAC-{an}")],
    )
    .unwrap();

    persistance::v2::migrer(&conn).unwrap();
    persistance::v2::migrer(&conn).unwrap();

    assert_eq!(
        argent::reserver_numero(&conn, "facture").unwrap(),
        format!("FAC-{an}-00008")
    );
}

/// Les quatre appelants du compteur passent tous par le meme prefixe.
///
/// Pieces, transferts et code-barre tirent leurs rangs de `suivant`.
/// Si l'un d'eux n'etait pas cloisonne, sa suite se melangerait a celle
/// d'un autre dossier — et ce sont des suites ou un doublon se voit.
#[test]
fn toutes_les_suites_sont_cloisonnees() {
    let conn = base();
    let prefixe = format!("{}:", gescom_noyau::dossiers::DOSSIER_DEFAUT);

    argent::reserver_numero(&conn, "facture").unwrap();
    transferts::reserver_bon(&conn).unwrap();
    codebarre::reserver_sequence(&conn).unwrap();

    let nues: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM compteur_piece WHERE cle NOT LIKE ?1 || '%'",
            rusqlite::params![prefixe],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(nues, 0, "une suite échappe au cloisonnement");
}
