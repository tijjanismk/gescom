//! D57 — qui nomme un passant, qui cree un article hors du stock, sur
//! les deux moteurs.
//!
//! Le caissier « passe en client ordinaire » une fiche de passage avec le
//! droit de creer un client (`passage::DROIT_PROMOUVOIR`), pas celui de
//! modifier une fiche ; stock, caisse et creance ne bougent pas. Creer un
//! article qui ne suit pas son stock demande `suivi_stock::DROIT` des la
//! creation, a la main comme a l'import : l'employe est refuse sans que
//! rien ne s'ecrive, le patron passe et le geste est au journal. Chaque
//! fois sur une base neuve ET sur une base existante (les roles tels que
//! la v2 les ecrivait, l'amorcage rejoue comme au demarrage du serveur).
//!
//!   cargo test -p gescom-noyau --test droits_d57_base
//!   GESCOM_PG=postgresql://…/gescom_test cargo test -p gescom-noyau --test droits_d57_base -- --test-threads=1

mod commun;

use commun::*;
use gescom_noyau::argent::{self, ParamsLigneInput};
use gescom_noyau::base::Base;
use gescom_noyau::parametres;
use gescom_noyau::portes::{self, ContexteUtilisateur};
use gescom_noyau::{amorcage, auteur, auth, caisse, catalogue_csv, passage, suivi_stock};

const ENTETE: &str = "Nom;Categorie;Unite;Prix;Prix achat;TVA %;Code barre;Stock;Suivre le stock";

/// Une base installee par une version plus ancienne : le caissier et
/// l'employe portent la liste que la v2 ecrivait (`persistance/v2.rs`),
/// puis le serveur redemarre (amorcage et migrations de donnees). Sous le
/// compte limite (D-6), rien ne migre : la base reste neuve.
fn base_existante() -> Base {
    let mut base = base_avec_demo();
    if !base.peut_migrer() {
        return base;
    }
    let caissier = r#"["ventes:creer","paiements:creer","retours:creer",
                    "clients:creer","caisse:mouvementer","pieces:creer"]"#;
    let employe = r#"["ventes:creer","paiements:creer","clients:creer","clients:modifier","articles:creer","caisse:mouvementer","pieces:creer","retours:creer"]"#;
    for (role, liste) in [("caissier", caissier), ("employe", employe)] {
        base.executer("UPDATE role SET permissions = ?1 WHERE nom = ?2", &parametres![liste, role]).unwrap();
    }
    amorcage::amorcer(&mut base).expect("amorçage rejoué");
    amorcage::migrations_de_donnees(&mut base);
    base
}

fn patron(base: &mut Base) -> String {
    base.lire_une(
        "SELECT u.id FROM utilisateur u JOIN role r ON r.id = u.role_id
         WHERE r.nom = 'patron' ORDER BY u.cree_le LIMIT 1",
        &[],
        |r| r.get::<String>(0),
    )
    .unwrap()
    .expect("un patron amorcé")
}

/// Un compte de ce role ; rend le contexte que le serveur juge.
fn compte(base: &mut Base, pseudo: &str, role: &str) -> ContexteUtilisateur {
    let par = patron(base);
    let id = auth::creer_utilisateur_sur_base(
        base, pseudo.into(), pseudo.into(), None, format!("{pseudo}-secret-2026"), role.into(), par,
    )
    .expect("compte");
    ContexteUtilisateur { id, role: role.into() }
}

fn a_le_droit(base: &mut Base, qui: &ContexteUtilisateur, droit: &str) -> bool {
    portes::verifier_permission_sur(base, qui, droit).is_ok()
}

fn ligne_vente(art: &(String, String, f64, i64), depot: &str, quantite: f64) -> ParamsLigneInput {
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

fn entrees_caisse(base: &mut Base) -> i64 {
    let dossier = base.dossier().to_string();
    compter(
        base,
        "SELECT CAST(COALESCE(SUM(montant), 0) AS BIGINT) FROM mouvement_caisse
         WHERE sens = 'entree' AND dossier_id = ?1",
        &parametres![dossier],
    )
}

fn creances_ouvertes(base: &mut Base, client: &str) -> i64 {
    let dossier = base.dossier().to_string();
    compter(
        base,
        "SELECT COUNT(*) FROM vente WHERE client_id = ?1 AND statut = 'creance_ouverte' AND dossier_id = ?2",
        &parametres![client, dossier],
    )
}

fn nb_articles(base: &mut Base) -> i64 {
    compter(base, "SELECT COUNT(*) FROM article", &[])
}

fn nb_traces_suivi(base: &mut Base) -> i64 {
    let dossier = base.dossier().to_string();
    compter(
        base,
        "SELECT COUNT(*) FROM journal WHERE type_evenement = 'suivi_stock_change' AND dossier_id = ?1",
        &parametres![dossier],
    )
}

fn suivi(base: &mut Base, article: &str) -> i64 {
    base.lire_une("SELECT gere_en_stock FROM article WHERE id = ?1", &parametres![article], |r| r.get::<i64>(0))
        .unwrap()
        .unwrap()
}

/// Le caissier vend au comptant, puis nomme le passant.
fn le_caissier_promeut(mut base: Base) {
    let caissier = compte(&mut base, "fanta", "caissier");
    // La porte : le droit de creer un client, pas celui de modifier.
    assert!(a_le_droit(&mut base, &caissier, passage::DROIT_PROMOUVOIR), "le caissier passe la porte");
    assert!(!a_le_droit(&mut base, &caissier, "clients:modifier"), "rattacher et modifier lui restent fermés");

    let _auteur = auteur::poser(&caissier.id);
    caisse::ouvrir_session_caisse_sur(&mut base, 0, "caissier".into()).expect("caisse ouverte");
    let depot = depot_defaut(&mut base);
    let sucre = article_unite(&mut base, "Sucre");
    let comptant = client_generique(&mut base);
    let v = argent::creer_vente_sur_base(
        &mut base, comptant, depot.clone(), "comptant".into(),
        vec![ligne_vente(&sucre, &depot, 2.0)], None, Some(2 * sucre.3), Some("especes".into()), None,
    )
    .expect("vente au comptant");
    let fiche = v["client_id"].as_str().unwrap().to_string();

    let stock_avant = stock(&mut base, &sucre.0, &depot);
    let caisse_avant = entrees_caisse(&mut base);
    let r = passage::promouvoir_sur(&mut base, fiche.clone(), "Awa Diarra".into(), Some("76 00 11 22".into()))
        .expect("le caissier nomme le passant");
    assert_eq!(r["nom"], "Awa Diarra");

    // La fiche est un client ordinaire, avec son achat.
    let dossier = base.dossier().to_string();
    let (generique, statut): (i64, String) = base
        .lire_une(
            "SELECT est_generique, statut FROM client WHERE id = ?1 AND dossier_id = ?2",
            &parametres![fiche.clone(), dossier.clone()],
            |l| Ok((l.get::<i64>(0)?, l.get::<String>(1)?)),
        )
        .unwrap()
        .unwrap();
    assert_eq!((generique, statut.as_str()), (0, "client"));
    // Stock, caisse, creance : la promotion ne touche a rien de cela.
    assert_eq!(stock(&mut base, &sucre.0, &depot), stock_avant, "stock inchangé");
    assert_eq!(entrees_caisse(&mut base), caisse_avant, "caisse inchangée");
    assert_eq!(creances_ouvertes(&mut base, &fiche), 0, "rien n'est dû");
    // Le journal dit que c'est le caissier.
    assert_eq!(
        compter(
            &mut base,
            "SELECT COUNT(*) FROM journal WHERE type_evenement = 'passage_promu' AND entite_id = ?1 AND auteur_id = ?2 AND dossier_id = ?3",
            &parametres![fiche, caissier.id.clone(), dossier],
        ),
        1
    );
}

/// L'employe cree, mais pas hors du stock ; le patron, si. A la main
/// (Parametres) comme a l'import.
fn la_creation_hors_du_stock(mut base: Base) {
    let employe = compte(&mut base, "modibo", "employe");
    let patron = ContexteUtilisateur { id: patron(&mut base), role: "patron".into() };
    let peut_employe = a_le_droit(&mut base, &employe, suivi_stock::DROIT);
    let peut_patron = a_le_droit(&mut base, &patron, suivi_stock::DROIT);
    assert!(!peut_employe, "l'employé n'a pas le droit de régler le suivi");
    assert!(peut_patron, "le patron l'a");
    assert!(a_le_droit(&mut base, &employe, "articles:creer"), "l'employé crée des articles");

    // L'employe, hors du stock : refuse avant d'ecrire.
    {
        let _auteur = auteur::poser(&employe.id);
        let (articles, traces) = (nb_articles(&mut base), nb_traces_suivi(&mut base));
        let e = parametres::creer_article_complet_sur_base(
            &mut base, "Sachets d'eau".into(), None, "sachet".into(), 25, Some(10.0), false, peut_employe,
        )
        .unwrap_err();
        assert!(e.contains(suivi_stock::DROIT), "le refus nomme le droit : {e}");
        assert_eq!((nb_articles(&mut base), nb_traces_suivi(&mut base)), (articles, traces), "rien n'a été écrit");
        // Suivi : cree, comme au comptoir.
        let id = parametres::creer_article_complet_sur_base(
            &mut base, "Sachets d'eau".into(), None, "sachet".into(), 25, Some(10.0), true, peut_employe,
        )
        .expect("l'employé crée un article suivi");
        assert_eq!(suivi(&mut base, &id), 1);
        // L'import : la ligne « non » qui creerait l'article est refusee.
        let r = catalogue_csv::importer_articles_csv_sur_base(
            &mut base, format!("{ENTETE}\nRecharges;;u;100;;;;;non"), None, peut_employe, false,
        )
        .unwrap();
        assert!(r.to_string().contains("Recharges"), "la ligne refusée est nommée : {r}");
        assert_eq!(
            compter(&mut base, "SELECT COUNT(*) FROM article WHERE nom = ?1", &parametres!["Recharges"]),
            0,
            "l'import n'a rien créé"
        );
    }

    // Le patron : une seule ecriture, l'article hors du stock, le geste
    // au journal, pas de seuil, sa ligne de stock a zero.
    let _auteur = auteur::poser(&patron.id);
    let traces = nb_traces_suivi(&mut base);
    let id = parametres::creer_article_complet_sur_base(
        &mut base, "Crédit Orange".into(), None, "u".into(), 1000, Some(5.0), false, peut_patron,
    )
    .expect("le patron crée un article non suivi");
    assert_eq!(suivi(&mut base, &id), 0);
    assert_eq!(nb_traces_suivi(&mut base), traces + 1, "l'Historique le dit");
    let dossier = base.dossier().to_string();
    assert_eq!(
        compter(
            &mut base,
            "SELECT COUNT(*) FROM journal WHERE type_evenement = 'suivi_stock_change' AND entite_id = ?1 AND auteur_id = ?2 AND dossier_id = ?3",
            &parametres![id.clone(), patron.id.clone(), dossier],
        ),
        1
    );
    let seuil: Option<f64> = base
        .lire_une("SELECT seuil_alerte FROM article WHERE id = ?1", &parametres![id.clone()], |r| r.get::<Option<f64>>(0))
        .unwrap()
        .unwrap();
    assert_eq!(seuil, None, "un article non suivi n'a pas de seuil d'alerte");
    let depot = depot_defaut(&mut base);
    assert_eq!(stock(&mut base, &id, &depot), 0.0);
    // L'import du patron cree la ligne « non ».
    catalogue_csv::importer_articles_csv_sur_base(&mut base, format!("{ENTETE}\nRecharges;;u;100;;;;;non"), None, peut_patron, false)
        .unwrap();
    let recharges: String = base
        .lire_une("SELECT id FROM article WHERE nom = ?1", &parametres!["Recharges"], |r| r.get::<String>(0))
        .unwrap()
        .expect("créé par l'import du patron");
    assert_eq!(suivi(&mut base, &recharges), 0);
}

#[test]
fn le_caissier_promeut_un_passant_sur_une_base_neuve() {
    le_caissier_promeut(base_avec_demo());
}

#[test]
fn le_caissier_promeut_un_passant_sur_une_base_existante() {
    le_caissier_promeut(base_existante());
}

#[test]
fn l_employe_ne_cree_pas_hors_du_stock_le_patron_si_sur_une_base_neuve() {
    la_creation_hors_du_stock(base_avec_demo());
}

#[test]
fn l_employe_ne_cree_pas_hors_du_stock_le_patron_si_sur_une_base_existante() {
    la_creation_hors_du_stock(base_existante());
}

#[test]
fn tout_passe_le_detecteur() {
    // L'une apres l'autre : sur PostgreSQL, preparer une base vide le
    // schema de la precedente.
    let mut base = base_avec_demo();
    base.auditer(true);
    le_caissier_promeut(base);
    let mut existante = base_existante();
    existante.auditer(true);
    la_creation_hors_du_stock(existante);
}
