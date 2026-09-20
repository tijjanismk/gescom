//! Les listes filtrées du poste (SQLite) lient leurs filtres en
//! paramètres : une recherche avec une apostrophe ou un `%` est une
//! valeur, pas un morceau de SQL. Avant, le texte était collé dans le
//! WHERE (échappé à la main) — `%` ou `_` devenaient des jokers.

mod commun;

use commun::*;
use gescom_noyau::base::Base;
use gescom_noyau::{pagination, pieces};

fn base_sqlite() -> Base {
    // Ce test ne porte que sur les versions `conn` : sur PostgreSQL il
    // ne prouve rien, on le joue toujours en memoire.
    let mut base = Base::ouvrir(":memory:").expect("base en mémoire");
    gescom_noyau::amorcage::amorcer(&mut base).expect("amorçage");
    gescom_noyau::amorcage::donnees_demo(&mut base).expect("démo");
    base
}

#[test]
fn une_recherche_piegee_est_une_valeur_pas_du_sql() {
    let mut base = base_sqlite();
    let client = client_reel(&mut base);
    base.executer(
        "UPDATE client SET nom = ?1 WHERE id = ?2",
        &gescom_noyau::parametres!["Coulibaly O'Brien 100%", client.clone()],
    )
    .unwrap();
    let fournisseur = fournisseur(&mut base, "Grossiste d'Ali & Fils");
    let sucre = article_unite(&mut base, "Sucre");
    let piece = pieces::creer_piece_sur_base(
        &mut base, client.clone(), "devis".into(), vec![ligne(&sucre, 2.0)],
        None, None, None, None, None, None,
    )
    .expect("devis");
    pieces::creer_piece_fournisseur_sur_base(
        &mut base, fournisseur.clone(), "bon_commande_fournisseur".into(), vec![ligne(&sucre, 5.0)],
        None, None, None, None, None,
    )
    .expect("commande fournisseur");

    let conn = base.sqlite().expect("SQLite");

    // Une apostrophe dans la recherche : trouvee, sans casser la requete.
    let trouves = pagination::lire_clients_pagines(conn, 0, 10, Some("O'Brien".into()), false, None, None).unwrap();
    assert_eq!(trouves["total"], 1);
    // « % » seul n'est plus un joker : il cherche un pourcentage.
    let pct = pagination::lire_clients_pagines(conn, 0, 10, Some("100%".into()), false, None, None).unwrap();
    assert_eq!(pct["total"], 1);
    let rien = pagination::lire_clients_pagines(conn, 0, 10, Some("zzz'zzz".into()), false, None, None).unwrap();
    assert_eq!(rien["total"], 0);

    let f = pagination::lire_fournisseurs_pagines(conn, 0, 10, Some("d'Ali".into())).unwrap();
    assert_eq!(f["total"], 1);

    let s = pagination::lire_stocks_pagines(conn, 0, 10, Some("Sucre".into()), false, Some("categorie'inconnue".into())).unwrap();
    assert_eq!(s["total"], 0);
    let s = pagination::lire_stocks_pagines(conn, 0, 10, Some("Sucre".into()), false, None).unwrap();
    assert!(s["total"].as_i64().unwrap() >= 1);

    let pc = pieces::lire_toutes_pieces_client(
        conn, Some("devis".into()), None, Some("O'Brien".into()), None, None, None, None, None, None, Some(client.clone()),
    )
    .unwrap();
    assert_eq!(pc.len(), 1);
    assert_eq!(pc[0]["id"], piece["id"]);
    let pc = pieces::lire_toutes_pieces_client(
        conn, None, Some("statut'bidon".into()), None, None, None, None, None, None, None, None,
    )
    .unwrap();
    assert!(pc.is_empty());

    let pf = pieces::lire_toutes_pieces_fournisseur(
        conn, Some("bon_commande_fournisseur".into()), None, Some("d'Ali".into()), Some(fournisseur), Some("".into()), Some("".into()),
    )
    .unwrap();
    assert_eq!(pf.len(), 1);
}
