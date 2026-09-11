//! Le cloisonnement par dossier, sur une vraie base.
//!
//! Les tests unitaires de `dossiers` verifient le detecteur sur des
//! chaines. Ici on verifie ce qui compte vraiment : qu'une base
//! existante accepte les colonnes, que les lignes deja ecrites
//! atterrissent dans le dossier d'origine, et que deux dossiers ne se
//! voient pas.

use gescom_noyau::amorcage;
use gescom_noyau::base::Base;
use gescom_noyau::dossiers::DOSSIER_DEFAUT;
use gescom_noyau::parametres;

fn base_amorcee() -> Base {
    let mut base = Base::ouvrir(":memory:").expect("base en mémoire");
    amorcage::amorcer(&mut base).expect("amorçage");
    base
}

#[test]
fn l_amorcage_cree_le_dossier_d_origine() {
    let mut base = base_amorcee();

    let (id, code, clos) = base
        .lire_une(
            "SELECT id, code, clos FROM dossier",
            &[],
            |r| Ok((r.get::<String>(0)?, r.get::<String>(1)?, r.get::<i64>(2)?)),
        )
        .unwrap()
        .expect("le dossier d'origine existe");

    assert_eq!(id, DOSSIER_DEFAUT);
    assert_eq!(code, "PRINCIPAL");
    assert_eq!(clos, 0, "un dossier neuf est ouvert");
}

#[test]
fn les_lignes_de_l_amorcage_appartiennent_au_dossier_d_origine() {
    // Le magasin et le client generique sont ecrits par l'amorcage,
    // donc AVANT que qui que ce soit ait choisi un dossier. Ils doivent
    // malgre tout etre rattaches — sinon une base migree aurait des
    // lignes orphelines que plus aucune requete cloisonnee ne verrait.
    let mut base = base_amorcee();

    for table in ["depot", "client"] {
        let orphelines = base
            .lire_une(
                &format!("SELECT COUNT(*) FROM {table} WHERE dossier_id <> ?1"),
                &parametres![DOSSIER_DEFAUT],
                |r| r.get::<i64>(0),
            )
            .unwrap()
            .unwrap();
        assert_eq!(orphelines, 0, "{table} a des lignes hors dossier");
    }
}

#[test]
fn une_insertion_sans_dossier_tombe_dans_le_dossier_d_origine() {
    let mut base = base_amorcee();

    base.executer(
        "INSERT INTO client (id, code, nom, est_generique, actif, cree_le, modifie_le, origine)
         VALUES ('c1', 'CLIENT00001', 'Awa', 0, 1, '2026-09-11', '2026-09-11', 'test')",
        &[],
    )
    .unwrap();

    let dossier = base
        .lire_une(
            "SELECT dossier_id FROM client WHERE id = 'c1'",
            &[],
            |r| r.get::<String>(0),
        )
        .unwrap()
        .unwrap();
    assert_eq!(dossier, DOSSIER_DEFAUT);
}

#[test]
fn deux_dossiers_ne_voient_pas_les_clients_l_un_de_l_autre() {
    let mut base = base_amorcee();

    for (id, code, nom, dossier) in [
        ("c1", "CLIENT00001", "Awa", DOSSIER_DEFAUT),
        ("c2", "CLIENT00002", "Modibo", "dossier-b"),
    ] {
        base.executer(
            "INSERT INTO client
               (id, code, nom, est_generique, actif, cree_le, modifie_le, origine, dossier_id)
             VALUES (?1, ?2, ?3, 0, 1, '2026-09-11', '2026-09-11', 'test', ?4)",
            &parametres![id, code, nom, dossier],
        )
        .unwrap();
    }

    let noms = base
        .lire_plusieurs(
            "SELECT nom FROM client WHERE est_generique = 0 AND dossier_id = ?1",
            &parametres!["dossier-b"],
            |r| r.get::<String>(0),
        )
        .unwrap();
    assert_eq!(noms, vec!["Modibo".to_string()], "chacun chez soi");
}

#[test]
fn l_audit_refuse_une_requete_qui_oublie_le_filtre() {
    let mut base = base_amorcee();
    base.auditer(true);

    let refus = base
        .lire_plusieurs("SELECT id FROM client WHERE actif = 1", &[], |r| {
            r.get::<String>(0)
        })
        .expect_err("une lecture non cloisonnée doit être refusée");
    assert!(refus.0.contains("client"), "le refus nomme la table : {refus}");

    // La meme, cloisonnee, passe.
    base.lire_plusieurs(
        "SELECT id FROM client WHERE actif = 1 AND dossier_id = ?1",
        &parametres![DOSSIER_DEFAUT],
        |r| r.get::<String>(0),
    )
    .expect("une lecture cloisonnée passe");
}

#[test]
fn l_audit_couvre_aussi_les_transactions() {
    // C'est la ou il sert le plus : une vente ecrit ses lignes, son
    // stock et son paiement dans une transaction. Un oubli la-dedans
    // ne se verrait nulle part ailleurs.
    let mut base = base_amorcee();
    base.auditer(true);

    let mut tx = base.transaction().unwrap();
    let refus = tx
        .executer("DELETE FROM ligne_vente WHERE vente_id = 'v1'", &[])
        .expect_err("une écriture non cloisonnée doit être refusée");
    assert!(refus.0.contains("ligne_vente"));
}

#[test]
fn la_connexion_sait_sur_quel_dossier_elle_travaille() {
    let mut base = base_amorcee();
    assert_eq!(base.dossier(), DOSSIER_DEFAUT);

    base.choisir_dossier("dossier-b").unwrap();
    assert_eq!(base.dossier(), "dossier-b");
}

#[test]
fn poser_le_cloisonnement_deux_fois_ne_casse_rien() {
    // Le cas normal au deuxieme demarrage : les colonnes sont la, les
    // `ALTER` echouent, et l'amorcage doit s'en moquer.
    let mut base = base_amorcee();
    amorcage::amorcer(&mut base).expect("second passage");

    let dossiers = base
        .lire_une("SELECT COUNT(*) FROM dossier", &[], |r| r.get::<i64>(0))
        .unwrap()
        .unwrap();
    assert_eq!(dossiers, 1, "le dossier d'origine n'est pas recréé");

    let exercices = base
        .lire_une("SELECT COUNT(*) FROM exercice", &[], |r| r.get::<i64>(0))
        .unwrap()
        .unwrap();
    assert_eq!(exercices, 1, "le premier exercice n'est pas recréé");
}

#[test]
fn deux_dossiers_ont_chacun_leur_suite_de_numeros() {
    // Deux societes qui facturent le meme jour doivent chacune avoir sa
    // FAC-2026-00001. Un compteur partage donnerait a la seconde un
    // numero qui commence a 00043 sans raison visible — et ferait
    // apparaitre des trous dans les deux series.
    use gescom_noyau::argent;

    let mut base = base_amorcee();

    let a1 = argent::reserver_numero_sur(&mut base, "facture").unwrap();
    let a2 = argent::reserver_numero_sur(&mut base, "facture").unwrap();

    base.choisir_dossier("dossier-b").unwrap();
    let b1 = argent::reserver_numero_sur(&mut base, "facture").unwrap();

    assert!(a1.ends_with("00001"), "{a1}");
    assert!(a2.ends_with("00002"), "{a2}");
    assert!(
        b1.ends_with("00001"),
        "la seconde société hérite du compteur de la première : {b1}"
    );
}

#[test]
fn revenir_a_un_dossier_reprend_sa_suite_la_ou_elle_en_etait() {
    use gescom_noyau::argent;

    let mut base = base_amorcee();
    argent::reserver_numero_sur(&mut base, "facture").unwrap();

    base.choisir_dossier("dossier-b").unwrap();
    argent::reserver_numero_sur(&mut base, "facture").unwrap();

    base.choisir_dossier(DOSSIER_DEFAUT).unwrap();
    let suivant = argent::reserver_numero_sur(&mut base, "facture").unwrap();
    assert!(
        suivant.ends_with("00002"),
        "le passage par un autre dossier a perturbé la suite : {suivant}"
    );
}

// =====================================================================
//  LES MODULES DEJA PORTES, DETECTEUR ALLUME
// =====================================================================

/// Le test qui garde le portage honnete.
///
/// Detecteur allume, chaque fonction portee doit passer. Un filtre
/// oublie echoue ICI — pas chez un commercant qui verrait le catalogue
/// d'une autre societe sans jamais s'en douter.
///
/// C'est aussi ce qui rend le portage des modules suivants mecanique :
/// on ajoute l'appel a cette liste, et le detecteur dit ce qui manque.
#[test]
fn tout_ce_qui_est_porte_passe_le_detecteur() {
    use gescom_noyau::{catalogue, comptoir};

    let mut base = base_amorcee();
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

    comptoir::creer_article_rapide_sur(
        &mut base,
        "Ciment".into(),
        "sac".into(),
        5_000,
        Some(4_000),
    )
    .expect("créer un article");

    comptoir::lire_clients_avec_creances_sur(&mut base).expect("créances");
    comptoir::lire_config_scanner_sur(&mut base).expect("scanner");

    use gescom_noyau::dossiers;
    dossiers::lire_exercices_sur(&mut base).expect("exercices");
    dossiers::verifier_date_sur(&mut base, "2026-09-11").expect("date dans l'exercice");
    let exercice = dossiers::ouvrir_exercice_sur(
        &mut base,
        "2027-01-01".into(),
        "2027-12-31".into(),
    )
    .expect("ouvrir un exercice");
    dossiers::prolonger_exercice_sur(
        &mut base,
        exercice["id"].as_str().unwrap().to_string(),
        "2028-01-31".into(),
    )
    .expect("prolonger");
    dossiers::clore_exercice_sur(&mut base, exercice["id"].as_str().unwrap().to_string())
        .expect("clore");
}

#[test]
fn un_second_dossier_ne_voit_pas_les_clients_du_premier_mais_voit_ses_articles() {
    use gescom_noyau::{catalogue, comptoir};

    let mut base = base_amorcee();
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

    // `article` n'est pas cloisonne (D3 du plan multi-societe) : un
    // article cree dans un dossier reste visible depuis l'autre.
    let visible = base
        .lire_une("SELECT COUNT(*) FROM article WHERE nom = 'Ciment'", &[], |r| {
            r.get::<i64>(0)
        })
        .unwrap()
        .unwrap();
    assert_eq!(visible, 1, "l'article devrait être partagé entre les deux dossiers");
}

/// Un dossier neuf n'a pas de magasin — et le dit.
///
/// Ce n'est pas un defaut a corriger ici : c'est la preuve que le
/// cloisonnement mord. Cela dit surtout ce que devra faire l'ecran de
/// creation d'un dossier : lui poser son magasin par defaut et son
/// client de passage, comme l'amorcage le fait pour le premier.
#[test]
fn un_dossier_neuf_reclame_son_magasin_avant_de_vendre() {
    use gescom_noyau::catalogue;

    let mut base = base_amorcee();
    base.auditer(true);
    base.choisir_dossier("dossier-b").unwrap();

    let erreur = catalogue::lire_articles_avec_unites_sur(&mut base, None, None)
        .expect_err("un dossier sans magasin ne peut pas servir de catalogue");
    assert!(
        erreur.contains("magasin"),
        "le refus doit dire ce qui manque : {erreur}"
    );
}

// =====================================================================
//  LES EXERCICES
// =====================================================================

#[test]
fn l_amorcage_cree_l_exercice_d_origine() {
    let mut base = base_amorcee();
    let annee = gescom_noyau::utils::maintenant_iso().chars().take(4).collect::<String>();

    let exercices = gescom_noyau::dossiers::lire_exercices_sur(&mut base).expect("exercices");
    assert_eq!(exercices.len(), 1, "un seul exercice au depart");
    assert_eq!(exercices[0]["date_debut"], format!("{annee}-01-01"));
    assert_eq!(exercices[0]["date_fin"], format!("{annee}-12-31"));
    assert_eq!(exercices[0]["clos"], false);
}

#[test]
fn deux_dossiers_ont_chacun_leurs_exercices() {
    use gescom_noyau::dossiers;

    let mut base = base_amorcee();
    base.auditer(true);

    base.choisir_dossier("dossier-b").unwrap();
    dossiers::ouvrir_exercice_sur(&mut base, "2026-01-01".into(), "2026-12-31".into())
        .expect("ouvrir l'exercice du second dossier");

    let chez_b = dossiers::lire_exercices_sur(&mut base).expect("exercices de dossier-b");
    assert_eq!(chez_b.len(), 1, "dossier-b ne doit voir que le sien");

    base.choisir_dossier(DOSSIER_DEFAUT).unwrap();
    let chez_defaut = dossiers::lire_exercices_sur(&mut base).expect("exercices du defaut");
    assert_eq!(
        chez_defaut.len(),
        1,
        "le dossier par defaut ne doit pas voir l'exercice de dossier-b"
    );
}

#[test]
fn ouvrir_un_exercice_qui_chevauche_est_refuse() {
    use gescom_noyau::dossiers;
    let mut base = base_amorcee();

    // L'amorcage a deja pose l'exercice de l'annee en cours : n'importe
    // quelle plage qui le touche doit etre refusee.
    let annee = gescom_noyau::utils::maintenant_iso().chars().take(4).collect::<String>();
    let erreur = dossiers::ouvrir_exercice_sur(
        &mut base,
        format!("{annee}-06-01"),
        format!("{annee}-08-31"),
    )
    .expect_err("un exercice qui chevauche doit être refusé");
    assert!(erreur.contains("Chevauche"), "{erreur}");
}

#[test]
fn verifier_date_sur_suit_l_exercice_ouvert() {
    use gescom_noyau::dossiers;
    let mut base = base_amorcee();
    let annee = gescom_noyau::utils::maintenant_iso().chars().take(4).collect::<String>();

    dossiers::verifier_date_sur(&mut base, &format!("{annee}-06-15"))
        .expect("une date dans l'exercice de l'amorçage passe");

    let hors = dossiers::verifier_date_sur(&mut base, "2019-01-01")
        .expect_err("une date hors de tout exercice doit être refusée");
    assert!(hors.contains("aucun exercice"), "{hors}");
}

#[test]
fn prolonger_puis_clore_un_exercice() {
    use gescom_noyau::dossiers;
    let mut base = base_amorcee();

    let exercices = dossiers::lire_exercices_sur(&mut base).unwrap();
    let id = exercices[0]["id"].as_str().unwrap().to_string();
    let annee: i32 = gescom_noyau::utils::maintenant_iso()[..4].parse().unwrap();

    // Sans prolongation, une date de janvier suivant est hors exercice.
    let debut_annee_suivante = format!("{}-01-15", annee + 1);
    dossiers::verifier_date_sur(&mut base, &debut_annee_suivante)
        .expect_err("hors exercice avant prolongation");

    dossiers::prolonger_exercice_sur(&mut base, id.clone(), format!("{}-01-31", annee + 1))
        .expect("prolonger");
    dossiers::verifier_date_sur(&mut base, &debut_annee_suivante)
        .expect("accepté après prolongation");

    dossiers::clore_exercice_sur(&mut base, id).expect("clore");
    let refus = dossiers::verifier_date_sur(&mut base, &debut_annee_suivante)
        .expect_err("un exercice clos refuse toute écriture");
    assert!(refus.contains("clos"), "{refus}");
}

#[test]
fn prolonger_ou_clore_l_exercice_d_un_autre_dossier_echoue() {
    use gescom_noyau::dossiers;
    let mut base = base_amorcee();

    let id = dossiers::lire_exercices_sur(&mut base).unwrap()[0]["id"]
        .as_str()
        .unwrap()
        .to_string();

    base.choisir_dossier("dossier-b").unwrap();
    assert!(
        dossiers::prolonger_exercice_sur(&mut base, id.clone(), "2099-01-01".into()).is_err(),
        "un autre dossier ne doit pas pouvoir prolonger cet exercice"
    );
    assert!(
        dossiers::clore_exercice_sur(&mut base, id).is_err(),
        "un autre dossier ne doit pas pouvoir clore cet exercice"
    );
}
