//! L'amorcage doit produire la meme boutique sur les deux moteurs.
//!
//! Ces scenarios ne tournent que si `GESCOM_PG` designe une base
//! PostgreSQL : sans elle, ils passent en silence. Un test qui exige un
//! service tiers ne doit pas faire echouer la suite de quelqu'un qui ne
//! l'a pas installe.

use gescom_noyau::amorcage;
use gescom_noyau::base::Base;
use gescom_noyau::parametres;

fn pg() -> Option<Base> {
    let url = std::env::var("GESCOM_PG").ok()?;
    Base::ouvrir(&url).ok()
}

#[test]
fn postgresql_s_amorce_et_donne_de_quoi_se_connecter() {
    let Some(mut base) = pg() else { return };

    // Table rase : le test doit partir du meme etat a chaque fois.
    base.executer_lot("DROP SCHEMA public CASCADE; CREATE SCHEMA public;")
        .expect("schéma remis à zéro");

    assert!(amorcage::amorcer(&mut base).expect("amorçage"));

    let roles = base
        .lire_plusieurs("SELECT nom FROM role ORDER BY nom", &[], |r| {
            r.get::<String>(0)
        })
        .expect("rôles");
    for attendu in ["caissier", "comptable", "employe", "magasinier", "patron", "superadmin"] {
        assert!(roles.contains(&attendu.to_string()), "{attendu} absent : {roles:?}");
    }

    let comptes = base
        .lire_une("SELECT COUNT(*) FROM utilisateur_auth", &[], |r| r.get::<i64>(0))
        .unwrap()
        .unwrap();
    assert_eq!(comptes, 2);

    // Le minimum pour vendre.
    let depots = base
        .lire_une(
            "SELECT COUNT(*) FROM depot WHERE est_defaut = 1",
            &[],
            |r| r.get::<i64>(0),
        )
        .unwrap()
        .unwrap();
    assert_eq!(depots, 1);
}

#[test]
fn l_amorcage_ne_se_rejoue_pas_sur_postgresql() {
    let Some(mut base) = pg() else { return };
    base.executer_lot("DROP SCHEMA public CASCADE; CREATE SCHEMA public;").unwrap();

    assert!(amorcage::amorcer(&mut base).unwrap());
    assert!(!amorcage::amorcer(&mut base).unwrap(), "il ne rejoue pas");

    let comptes = base
        .lire_une("SELECT COUNT(*) FROM utilisateur_auth", &[], |r| r.get::<i64>(0))
        .unwrap()
        .unwrap();
    assert_eq!(comptes, 2, "aucun compte en double");
}

#[test]
fn les_donnees_de_demonstration_passent_sur_postgresql() {
    // Ce test vaut surtout pour ce qu'il traverse : article,
    // unite_vente, categorie, client, mouvement_stock, stock_depot.
    // C'est la que se trouvent les ecarts de dialecte qu'un amorcage
    // minimal ne rencontre jamais.
    let Some(mut base) = pg() else { return };
    base.executer_lot("DROP SCHEMA public CASCADE; CREATE SCHEMA public;").unwrap();
    amorcage::amorcer(&mut base).unwrap();

    let (articles, clients) = amorcage::donnees_demo(&mut base).expect("démo");
    assert_eq!(articles, 8);
    assert_eq!(clients, 4);

    let unites = base
        .lire_une("SELECT COUNT(*) FROM unite_vente", &[], |r| r.get::<i64>(0))
        .unwrap()
        .unwrap();
    assert!(unites >= 12, "les unités de vente sont posées : {unites}");

    // Le stock doit valoir la somme de ses mouvements — l'invariant
    // pose a l'etape 2 du projet.
    let compteur = base
        .lire_une(
            "SELECT COALESCE(SUM(quantite), 0) FROM stock_depot",
            &[],
            |r| r.get::<f64>(0),
        )
        .unwrap()
        .unwrap();
    let somme = base
        .lire_une(
            "SELECT COALESCE(SUM(quantite_delta), 0) FROM mouvement_stock",
            &[],
            |r| r.get::<f64>(0),
        )
        .unwrap()
        .unwrap();
    assert_eq!(compteur, somme, "le stock doit valoir la somme de ses mouvements");
    assert!(compteur > 0.0);
}

// =====================================================================
//  SE CONNECTER — le premier module porte
// =====================================================================

#[test]
fn on_se_connecte_a_postgresql_avec_ses_permissions() {
    let Some(mut base) = pg() else { return };
    base.executer_lot("DROP SCHEMA public CASCADE; CREATE SCHEMA public;").unwrap();
    amorcage::amorcer(&mut base).unwrap();

    let u = gescom_noyau::auth::connexion_sur(
        &mut base,
        "admin".to_string(),
        "admin123".to_string(),
    )
    .expect("le patron doit pouvoir se connecter");

    assert_eq!(u["role"], "patron");
    assert_eq!(u["doit_changer_mdp"], true);

    // Les permissions voyagent avec l'identite : sans elles, l'ecran
    // n'affiche ni menu ni onglet.
    let perms = u["permissions"].as_array().expect("des permissions");
    assert_eq!(
        perms.len(),
        gescom_noyau::portes::CATALOGUE.len(),
        "le patron a l'accès total"
    );

    // Par l'email aussi : le champ accepte les deux.
    assert!(gescom_noyau::auth::connexion_sur(
        &mut base, "admin@gescom.ml".to_string(), "admin123".to_string()
    ).is_ok());
}

#[test]
fn un_mot_de_passe_faux_est_refuse_sur_postgresql() {
    let Some(mut base) = pg() else { return };
    base.executer_lot("DROP SCHEMA public CASCADE; CREATE SCHEMA public;").unwrap();
    amorcage::amorcer(&mut base).unwrap();

    let r = gescom_noyau::auth::connexion_sur(
        &mut base, "admin".to_string(), "admin".to_string(),
    );
    assert!(r.is_err());
    // Le meme message que pour un identifiant inconnu : les distinguer
    // confirmerait qu'un compte existe a qui essaie des noms au hasard.
    let inconnu = gescom_noyau::auth::connexion_sur(
        &mut base, "personne".to_string(), "admin123".to_string(),
    );
    assert_eq!(format!("{r:?}"), format!("{inconnu:?}"));
}

#[test]
fn une_session_s_ouvre_et_se_revoque_sur_postgresql() {
    use gescom_noyau::sessions::{self, Etat};
    let Some(mut base) = pg() else { return };
    base.executer_lot("DROP SCHEMA public CASCADE; CREATE SCHEMA public;").unwrap();
    amorcage::amorcer(&mut base).unwrap();

    let utilisateur: String = base
        .lire_une("SELECT utilisateur_id FROM utilisateur_auth WHERE pseudo = 'admin'",
                  &[], |r| r.get::<String>(0))
        .unwrap()
        .unwrap();

    // Un poste, comme en inscrit une connexion reseau.
    base.executer(
        "INSERT INTO poste (id, nom, empreinte, genre, actif, cree_le, modifie_le)
         VALUES ('p1', 'Caisse 1', 'emp-1', 'caisse', 1, '2026-01-01', '2026-01-01')",
        &[],
    )
    .unwrap();

    let (session, jeton, _expire) =
        sessions::ouvrir_sur(&mut base, "p1", &utilisateur).expect("session ouverte");
    assert_eq!(jeton.len(), 64, "256 bits d'aléa");

    match sessions::etat_sur(&mut base, &session) {
        Etat::Valide { role, .. } => assert_eq!(role, "patron"),
        _ => panic!("la session devrait être valide"),
    }

    assert_eq!(sessions::lister_actives_sur(&mut base).unwrap().len(), 1);

    sessions::revoquer_sur(&mut base, &session, "test").unwrap();
    assert!(matches!(
        sessions::etat_sur(&mut base, &session),
        Etat::Revoquee
    ));
    assert!(sessions::lister_actives_sur(&mut base).unwrap().is_empty());
}

#[test]
fn un_poste_desactive_coupe_la_session_sur_postgresql() {
    use gescom_noyau::sessions::{self, Etat};
    let Some(mut base) = pg() else { return };
    base.executer_lot("DROP SCHEMA public CASCADE; CREATE SCHEMA public;").unwrap();
    amorcage::amorcer(&mut base).unwrap();

    let utilisateur: String = base
        .lire_une("SELECT utilisateur_id FROM utilisateur_auth WHERE pseudo = 'admin'",
                  &[], |r| r.get::<String>(0))
        .unwrap()
        .unwrap();
    base.executer(
        "INSERT INTO poste (id, nom, empreinte, genre, actif, cree_le, modifie_le)
         VALUES ('p2', 'Caisse 2', 'emp-2', 'caisse', 1, '2026-01-01', '2026-01-01')",
        &[],
    )
    .unwrap();
    let (session, _, _) = sessions::ouvrir_sur(&mut base, "p2", &utilisateur).unwrap();

    base.executer("UPDATE poste SET actif = 0 WHERE id = 'p2'", &[]).unwrap();
    assert!(matches!(
        sessions::etat_sur(&mut base, &session),
        Etat::PosteFerme
    ));
}

// =====================================================================
//  CATALOGUE ET COMPTOIR
// =====================================================================

fn base_peuplee() -> Option<gescom_noyau::base::Base> {
    let mut base = pg()?;
    base.executer_lot("DROP SCHEMA public CASCADE; CREATE SCHEMA public;").unwrap();
    amorcage::amorcer(&mut base).unwrap();
    amorcage::donnees_demo(&mut base).unwrap();
    Some(base)
}

#[test]
fn le_catalogue_se_lit_sur_postgresql() {
    use gescom_noyau::catalogue;
    let Some(mut base) = base_peuplee() else { return };

    let clients = catalogue::lire_clients_sur(&mut base).unwrap();
    assert_eq!(clients.len(), 5, "4 clients de démo + le générique");
    assert!(catalogue::lire_client_generique_sur(&mut base).is_ok());

    let depots = catalogue::lire_depots_sur(&mut base).unwrap();
    assert_eq!(depots.len(), 1);
    assert_eq!(depots[0]["est_defaut"], true);

    let articles =
        catalogue::lire_articles_avec_unites_sur(&mut base, Some("patron".into()), None)
            .unwrap();
    assert_eq!(articles.len(), 8);

    // Les unites sont regroupees sous leur article, pas etalees.
    let sucre = articles.iter().find(|a| a["nom"] == "Sucre").expect("Sucre");
    assert_eq!(sucre["unites"].as_array().unwrap().len(), 2);
    assert_eq!(sucre["stock"], 200.0);
}

#[test]
fn le_prix_d_achat_ne_sort_que_pour_le_patron_sur_postgresql() {
    // Le filtre est cote serveur : un employe ne doit pas connaitre la
    // marge parce qu'il sait ouvrir les outils du navigateur.
    use gescom_noyau::catalogue;
    let Some(mut base) = base_peuplee() else { return };

    let employe =
        catalogue::lire_articles_avec_unites_sur(&mut base, Some("employe".into()), None)
            .unwrap();
    assert!(
        employe.iter().all(|a| a.get("dernier_prix_achat").is_none()),
        "aucun prix d'achat pour l'employé"
    );
}

#[test]
fn un_depot_inconnu_retombe_sur_le_defaut_sur_postgresql() {
    // Un depot desactive reste memorise dans la barre laterale : la
    // caisse doit s'ouvrir quand meme.
    use gescom_noyau::catalogue;
    let Some(mut base) = base_peuplee() else { return };
    let a = catalogue::lire_articles_avec_unites_sur(
        &mut base, None, Some("depot-fantome".into()),
    )
    .expect("l'écran doit s'ouvrir");
    assert_eq!(a.len(), 8);
}

#[test]
fn le_comptoir_ecrit_sur_postgresql() {
    use gescom_noyau::comptoir;
    let Some(mut base) = base_peuplee() else { return };

    let c = comptoir::creer_client_rapide_sur(
        &mut base, "Awa Traoré".into(), Some("76112233".into()),
    )
    .expect("client créé");
    assert_eq!(c["code"], "CLIENT00005", "le générique ne compte pas");

    comptoir::modifier_client_sur(
        &mut base,
        c["id"].as_str().unwrap().to_string(),
        "Awa Traoré Diallo".into(),
        Some("76000000".into()),
        Some("Badalabougou".into()),
        None,
        None,
    )
    .expect("client modifié");

    let relu = base
        .lire_une(
            "SELECT nom, adresse, email FROM client WHERE id = ?1",
            &gescom_noyau::parametres![c["id"].as_str().unwrap()],
            |r| Ok((r.get::<String>(0)?, r.get::<Option<String>>(1)?, r.get::<Option<String>>(2)?)),
        )
        .unwrap()
        .unwrap();
    assert_eq!(relu.0, "Awa Traoré Diallo", "l'accent survit");
    assert_eq!(relu.1.as_deref(), Some("Badalabougou"));
    assert_eq!(relu.2, None, "un champ vide redevient NULL");
}

#[test]
fn un_article_naît_avec_son_unité_ou_pas_du_tout() {
    // La transaction : un article sans unite de vente ne se vend pas,
    // et il faudrait le reparer a la main dans la base.
    use gescom_noyau::comptoir;
    let Some(mut base) = base_peuplee() else { return };

    let a = comptoir::creer_article_rapide_sur(
        &mut base, "Ciment CIMAF".into(), "sac".into(), 5500, Some(4800),
    )
    .expect("article créé");

    let unites = base
        .lire_une(
            "SELECT COUNT(*) FROM unite_vente WHERE article_id = ?1",
            &gescom_noyau::parametres![a["id"].as_str().unwrap()],
            |r| r.get::<i64>(0),
        )
        .unwrap()
        .unwrap();
    assert_eq!(unites, 1);

    // Le doublon est refuse, et rien n'est ecrit.
    let avant = base
        .lire_une("SELECT COUNT(*) FROM article", &[], |r| r.get::<i64>(0))
        .unwrap()
        .unwrap();
    assert!(comptoir::creer_article_rapide_sur(
        &mut base, "ciment cimaf".into(), "sac".into(), 5500, None,
    )
    .is_err(), "le doublon, même en minuscules, doit être refusé");
    let apres = base
        .lire_une("SELECT COUNT(*) FROM article", &[], |r| r.get::<i64>(0))
        .unwrap()
        .unwrap();
    assert_eq!(avant, apres);
}

// =====================================================================
//  LA NUMEROTATION — la piece la plus sensible a la concurrence
// =====================================================================

#[test]
fn la_numerotation_marche_sur_postgresql() {
    use gescom_noyau::argent;
    let Some(mut base) = base_peuplee() else { return };

    assert_eq!(argent::reserver_numero_sur(&mut base, "facture").unwrap(),
               format!("FAC-{}-00001", chrono::Local::now().format("%Y")));
    assert_eq!(argent::reserver_numero_sur(&mut base, "facture").unwrap(),
               format!("FAC-{}-00002", chrono::Local::now().format("%Y")));

    // Chaque serie a son compteur : un bon de livraison ne doit pas
    // faire avancer les factures.
    assert_eq!(argent::reserver_numero_sur(&mut base, "bon_livraison").unwrap(),
               format!("BL-{}-00001", chrono::Local::now().format("%Y")));
}

/// Deux connexions qui reservent en meme temps, sur PostgreSQL.
///
/// C'est le seul test qui prouve quelque chose sur la numerotation :
/// l'ancienne lecture par MAX() sortait 5 doublons sur 100 dans ce
/// scenario. Ici, quatre fils partent ensemble contre la MEME base.
#[test]
fn deux_caisses_qui_facturent_en_meme_temps_sur_postgresql() {
    use gescom_noyau::{argent, base::Base};
    use std::sync::{Arc, Barrier};

    let Some(url) = std::env::var("GESCOM_PG").ok() else { return };
    {
        let Some(mut base) = base_peuplee() else { return };
        // On repart d'un compteur propre.
        base.executer("DELETE FROM compteur_piece", &[]).unwrap();
    }

    const FILS: usize = 4;
    const PAR_FIL: usize = 25;
    let depart = Arc::new(Barrier::new(FILS));
    let mut mains = Vec::new();

    for _ in 0..FILS {
        let url = url.clone();
        let depart = Arc::clone(&depart);
        mains.push(std::thread::spawn(move || {
            let mut base = Base::ouvrir(&url).expect("connexion");
            depart.wait();
            (0..PAR_FIL)
                .map(|_| argent::reserver_numero_sur(&mut base, "facture").unwrap())
                .collect::<Vec<_>>()
        }));
    }

    let mut tous: Vec<String> = Vec::new();
    for m in mains {
        tous.extend(m.join().expect("un fil a paniqué"));
    }

    let mut uniques = tous.clone();
    uniques.sort();
    uniques.dedup();
    assert_eq!(
        uniques.len(),
        tous.len(),
        "un numéro est sorti deux fois — c'est exactement le bug corrigé"
    );
    // Et la suite est continue : ni trou, ni saut.
    let an = chrono::Local::now().format("%Y");
    assert_eq!(uniques.first().unwrap(), &format!("FAC-{an}-00001"));
    assert_eq!(uniques.last().unwrap(), &format!("FAC-{an}-{:05}", FILS * PAR_FIL));
}

// =====================================================================
//  LE CLOISONNEMENT PAR DOSSIER
// =====================================================================

/// Le test qui decide du cout de tout le portage.
///
/// Sur PostgreSQL, `dossier_id` a pour valeur par defaut le dossier de
/// la SESSION. Si cela marche, les 480 `params!` du projet n'ont pas a
/// etre rouverts pour y glisser un argument de plus : une insertion
/// tombe dans le bon dossier parce que la connexion sait lequel c'est.
///
/// Si cela ne marchait pas, il faudrait toucher chaque insertion du
/// noyau — et chaque endroit touche est un endroit qu'on peut casser.
#[test]
fn une_insertion_suit_le_dossier_de_la_session_sur_postgresql() {
    let Some(mut base) = pg() else { return };
    base.executer_lot("DROP SCHEMA public CASCADE; CREATE SCHEMA public;").unwrap();
    amorcage::amorcer(&mut base).unwrap();

    base.choisir_dossier("dossier-2027").unwrap();
    base.executer(
        "INSERT INTO client (id, code, nom, est_generique, actif, cree_le, modifie_le, origine)
         VALUES ('c-2027', 'CLIENT00001', 'Awa', 0, 1, '2027-01-05', '2027-01-05', 'test')",
        &[],
    )
    .unwrap();

    let dossier = base
        .lire_une(
            "SELECT dossier_id FROM client WHERE id = 'c-2027'",
            &[],
            |r| r.get::<String>(0),
        )
        .unwrap()
        .unwrap();
    assert_eq!(
        dossier, "dossier-2027",
        "l'insertion n'a pas suivi le dossier de la session"
    );
}

#[test]
fn deux_dossiers_ne_melangent_pas_leurs_clients_sur_postgresql() {
    let Some(mut base) = pg() else { return };
    base.executer_lot("DROP SCHEMA public CASCADE; CREATE SCHEMA public;").unwrap();
    amorcage::amorcer(&mut base).unwrap();

    for (dossier, id, nom) in [
        ("dossier-a", "c-a", "Awa"),
        ("dossier-b", "c-b", "Modibo"),
    ] {
        base.choisir_dossier(dossier).unwrap();
        base.executer(
            "INSERT INTO client (id, code, nom, est_generique, actif, cree_le, modifie_le, origine)
             VALUES (?1, ?2, ?3, 0, 1, '2026-09-11', '2026-09-11', 'test')",
            &parametres![id, format!("CODE-{id}"), nom],
        )
        .unwrap();
    }

    let chez_b = base
        .lire_plusieurs(
            "SELECT nom FROM client WHERE est_generique = 0 AND dossier_id = ?1",
            &parametres!["dossier-b"],
            |r| r.get::<String>(0),
        )
        .unwrap();
    assert_eq!(chez_b, vec!["Modibo".to_string()], "chacun chez soi");
}

/// Une base migree ne laisse aucune ligne derriere elle.
///
/// C'est la peur legitime devant ce genre de changement : que les
/// donnees d'avant se retrouvent dans un dossier que plus personne ne
/// regarde. Les donnees de demonstration servent ici de base « deja
/// remplie » qu'on cloisonne apres coup.
#[test]
fn les_donnees_existantes_restent_visibles_apres_le_cloisonnement() {
    let Some(mut base) = pg() else { return };
    base.executer_lot("DROP SCHEMA public CASCADE; CREATE SCHEMA public;").unwrap();
    amorcage::amorcer(&mut base).unwrap();
    let (articles, _) = amorcage::donnees_demo(&mut base).unwrap();

    // Amorcer une seconde fois rejoue le cloisonnement sur une base qui
    // a desormais des lignes.
    amorcage::amorcer(&mut base).unwrap();

    // `article` n'est plus cloisonne (D3 du plan multi-societe) : pas de
    // filtre de dossier ici, juste le compte brut.
    let visibles = base
        .lire_une("SELECT COUNT(*) FROM article", &[], |r| r.get::<i64>(0))
        .unwrap()
        .unwrap();
    assert_eq!(
        visibles as usize, articles,
        "des articles ont été perdus par le cloisonnement"
    );
}

/// Le meme controle que sur SQLite, sur le moteur de production.
///
/// Les deux moteurs ne se trompent pas de la meme facon : PostgreSQL
/// remplit `dossier_id` tout seul a l'insertion, SQLite non. Une
/// fonction peut donc passer d'un cote et pas de l'autre — ce qui
/// serait le pire des cas, puisque les tests tournent surtout sur
/// SQLite et que le commercant, lui, sera sur PostgreSQL.
#[test]
fn tout_ce_qui_est_porte_passe_le_detecteur_sur_postgresql() {
    use gescom_noyau::{catalogue, comptoir};

    let Some(mut base) = pg() else { return };
    base.executer_lot("DROP SCHEMA public CASCADE; CREATE SCHEMA public;").unwrap();
    amorcage::amorcer(&mut base).unwrap();
    base.auditer(true);

    catalogue::lire_clients_sur(&mut base).expect("clients");
    catalogue::lire_client_generique_sur(&mut base).expect("client générique");
    catalogue::lire_depots_sur(&mut base).expect("magasins");
    catalogue::lire_depot_defaut_sur(&mut base).expect("magasin par défaut");
    catalogue::lire_articles_avec_unites_sur(&mut base, Some("patron".into()), None)
        .expect("catalogue");

    let client = comptoir::creer_client_rapide_sur(&mut base, "Awa".into(), None)
        .expect("créer un client");
    comptoir::modifier_client_sur(
        &mut base,
        client["id"].as_str().unwrap().to_string(),
        "Awa Traoré".into(),
        Some("76000000".into()),
        None,
        None,
        None,
    )
    .expect("modifier un client");
    comptoir::creer_article_rapide_sur(&mut base, "Ciment".into(), "sac".into(), 5_000, None)
        .expect("créer un article");
    comptoir::lire_clients_avec_creances_sur(&mut base).expect("créances");

    // Le tableau de bord : c'est ici que `SUM(bigint)` rend NUMERIC et
    // que `julianday`/`strftime` n'existent pas — le moteur de
    // production est le seul a pouvoir le dire.
    use gescom_noyau::tableau_bord;
    let resume = tableau_bord::lire_resume_dashboard_sur(&mut base, None).expect("résumé");
    assert_eq!(resume["caisse_session_ouverte"], false);
    for periode in ["jour", "semaine", "mois", "annee"] {
        tableau_bord::lire_ventes_periode_sur(&mut base, Some(periode.into()), None)
            .unwrap_or_else(|e| panic!("courbe {periode} : {e}"));
    }
    tableau_bord::lire_top_clients_sur(&mut base).expect("meilleurs clients");
    tableau_bord::lire_top_articles_sur(&mut base).expect("meilleurs articles");
    tableau_bord::lire_ventes_a_decouvert_sur(&mut base, Some("2026-01-01".into()), None)
        .expect("découvert");

    use gescom_noyau::dossiers;
    dossiers::lire_exercices_sur(&mut base).expect("exercices");
    dossiers::verifier_date_sur(&mut base, "2026-09-11").expect("date dans l'exercice");
    let exercice =
        dossiers::ouvrir_exercice_sur(&mut base, "2027-01-01".into(), "2027-12-31".into())
            .expect("ouvrir un exercice");
    let id = exercice["id"].as_str().unwrap().to_string();
    dossiers::prolonger_exercice_sur(&mut base, id.clone(), "2028-01-31".into())
        .expect("prolonger");
    dossiers::clore_exercice_sur(&mut base, id).expect("clore");
}

/// Un dossier neuf ne partage pas les exercices d'un autre — le meme
/// controle que sur SQLite, sur le moteur de production.
#[test]
fn deux_dossiers_ont_chacun_leurs_exercices_sur_postgresql() {
    use gescom_noyau::dossiers;

    let Some(mut base) = pg() else { return };
    base.executer_lot("DROP SCHEMA public CASCADE; CREATE SCHEMA public;").unwrap();
    amorcage::amorcer(&mut base).unwrap();
    base.auditer(true);

    base.choisir_dossier("dossier-b").unwrap();
    dossiers::ouvrir_exercice_sur(&mut base, "2026-01-01".into(), "2026-12-31".into())
        .expect("ouvrir l'exercice du second dossier");
    let chez_b = dossiers::lire_exercices_sur(&mut base).expect("exercices");
    assert_eq!(chez_b.len(), 1, "dossier-b ne doit voir que le sien");

    base.choisir_dossier(gescom_noyau::dossiers::DOSSIER_DEFAUT).unwrap();
    let chez_defaut = dossiers::lire_exercices_sur(&mut base).expect("exercices");
    assert_eq!(chez_defaut.len(), 1, "le défaut ne doit pas voir l'exercice de dossier-b");
}

/// Les clients sont cloisonnes, les articles ne le sont pas (D3 du plan
/// multi-societe) — et les deux moteurs doivent s'accorder.
#[test]
fn deux_dossiers_ne_melangent_pas_leurs_clients_mais_partagent_leurs_articles_sur_postgresql() {
    use gescom_noyau::{catalogue, comptoir};

    let Some(mut base) = pg() else { return };
    base.executer_lot("DROP SCHEMA public CASCADE; CREATE SCHEMA public;").unwrap();
    amorcage::amorcer(&mut base).unwrap();
    base.auditer(true);

    comptoir::creer_client_rapide_sur(&mut base, "Awa".into(), None).unwrap();
    comptoir::creer_article_rapide_sur(&mut base, "Ciment".into(), "sac".into(), 5_000, None)
        .unwrap();

    base.choisir_dossier("dossier-b").unwrap();
    let clients = catalogue::lire_clients_sur(&mut base).expect("clients");
    assert!(
        clients.is_empty(),
        "le second dossier hérite des clients du premier : {clients:?}"
    );

    let visible = base
        .lire_une(
            "SELECT COUNT(*) FROM article WHERE nom = 'Ciment'",
            &[],
            |r| r.get::<i64>(0),
        )
        .unwrap()
        .unwrap();
    assert_eq!(
        visible, 1,
        "un article créé dans un dossier doit rester visible depuis l'autre"
    );
}

/// Un champ laisse vide sur une colonne numerique.
///
/// PostgreSQL deduit le type attendu de la colonne. Un NULL etiquete
/// TEXT presente a un entier faisait echouer la requete sur « error
/// serializing parameter 3 » — sans nommer ni la colonne, ni la table.
///
/// Le cas est banal : un article cree sans prix d'achat. Il ne s'etait
/// pas vu parce que SQLite accepte tout et que les scenarios
/// PostgreSQL renseignaient ce champ.
#[test]
fn un_champ_vide_passe_sur_une_colonne_numerique() {
    use gescom_noyau::comptoir;
    let Some(mut base) = pg() else { return };
    base.executer_lot("DROP SCHEMA public CASCADE; CREATE SCHEMA public;").unwrap();
    amorcage::amorcer(&mut base).unwrap();

    comptoir::creer_article_rapide_sur(
        &mut base,
        "Ciment sans prix d'achat".into(),
        "sac".into(),
        5_000,
        None,
    )
    .expect("un article sans prix d'achat doit pouvoir naître");
}
