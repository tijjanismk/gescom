//! Le duplicata (26/10/2026, PLAN-DUPLICATA) : le serveur compte les
//! impressions, sur les deux moteurs.
//!
//! La premiere impression d'une piece ou d'un recu est l'original, les
//! suivantes des duplicatas — depuis n'importe quel poste, par n'importe
//! qui. L'original reimprime est le droit du patron, et va au journal.
//! Un brouillon ne se compte pas. Puis le cloisonnement, et le detecteur.

mod commun;

use commun::*;
use gescom_noyau::argent::{self, ParamsLigneInput};
use gescom_noyau::base::Base;
use gescom_noyau::historique::{self, Filtre};
use gescom_noyau::impression;
use gescom_noyau::{creances, documents, dossiers, parametres, pieces, pieces_pos, portes};

const PIECE: &str = "piece_commerciale";
const RECU: &str = "paiement";

/// Une vente a credit au premier vrai client et un acompte. Rend
/// (piece de la vente, paiement de l'acompte).
fn facture_et_recu(base: &mut Base) -> (String, String) {
    ouvrir_caisse(base);
    let depot = depot_defaut(base);
    let sucre = article_unite(base, "Sucre");
    let client = client_reel(base);
    let v = argent::creer_vente_sur_base(
        base,
        client.clone(),
        depot.clone(),
        "credit".into(),
        vec![ParamsLigneInput {
            article_id: sucre.0.clone(),
            unite_vente_id: sucre.1.clone(),
            depot_source_id: depot,
            source_approvisionnement: "stock".into(),
            quantite: 2.0,
            facteur: sucre.2,
            prix_reference: sucre.3,
            prix_pratique: sucre.3,
            taux_tva: None,
            a_decouvert: None,
        }],
        None,
        None,
        None,
        None,
    )
    .expect("vente a credit");
    let vente_id = v["vente_id"].as_str().unwrap().to_string();
    creances::regler_creance_sur_base(base, vente_id.clone(), 300, "especes".into(), None).expect("acompte");
    // La facture du POS : creee apres la vente, retouchable, puis figee.
    let f = argent::creer_facture_depuis_vente_sur_base(base, vente_id.clone(), client, "credit".into(), None)
        .expect("facture de la vente");
    let piece = f["piece_id"].as_str().or(f["id"].as_str()).unwrap().to_string();
    if statut_piece(base, &piece) == "brouillon" {
        pieces_pos::valider_facture_credit_sur_base(base, piece.clone()).expect("figer la facture");
    }
    assert_ne!(statut_piece(base, &piece), "brouillon");
    let paiement = base
        .lire_une("SELECT id FROM paiement WHERE vente_id = ?1", &parametres![vente_id], |r| r.get::<String>(0))
        .unwrap()
        .expect("l'acompte");
    (piece, paiement)
}

/// Un utilisateur de la demo, par son nom de connexion.
fn utilisateur(base: &mut Base, pseudo: &str) -> String {
    base.lire_une(
        "SELECT u.id FROM utilisateur u JOIN utilisateur_auth a ON a.utilisateur_id = u.id WHERE a.pseudo = ?1",
        &parametres![pseudo],
        |r| r.get::<String>(0),
    )
    .unwrap()
    .unwrap()
}

/// Coche (ou decoche) le duplicata d'un genre, comme Paramètres → Documents.
fn regler_duplicata(base: &mut Base, genre: &str, actif: bool) {
    let mut r = documents::lire_reglage_sur(base, genre).unwrap();
    r["duplicata"] = serde_json::json!(actif);
    documents::enregistrer_reglage_sur(base, genre, r).expect("réglage");
}

/// La demo, le duplicata coche sur la facture et le recu. Le ticket reste
/// decoche : sa case est la sienne.
fn base_duplicata() -> Base {
    let mut base = base_avec_demo();
    regler_duplicata(&mut base, "facture", true);
    regler_duplicata(&mut base, "recu", true);
    base
}

fn noter(base: &mut Base, objet: &str, id: &str, poste: &str) -> serde_json::Value {
    impression::noter_impression_sur(base, objet.into(), id.into(), None, Some(poste.into())).expect("noter")
}

fn etat(base: &mut Base, objet: &str, id: &str) -> serde_json::Value {
    impression::lire_etat_impression_sur(base, objet.into(), id.into(), None).expect("etat")
}

fn impressions(base: &mut Base, id: &str) -> i64 {
    compter(base, "SELECT COUNT(*) FROM impression WHERE objet_id = ?1", &parametres![id])
}

#[test]
fn la_premiere_est_l_original_les_suivantes_des_duplicatas() {
    let mut base = base_duplicata();
    let (piece, paiement) = facture_et_recu(&mut base);

    for (objet, id) in [(PIECE, &piece), (RECU, &paiement)] {
        // L'apercu ne compte pas : le lire deux fois ne change rien.
        let avant = etat(&mut base, objet, id);
        assert_eq!(avant["duplicata"], false, "{objet} : {avant}");
        assert_eq!(etat(&mut base, objet, id), avant);
        assert_eq!(avant["impressions"], 0);
        assert!(avant["original_le"].is_null());

        let premiere = noter(&mut base, objet, id, "poste-a");
        assert_eq!(premiere["duplicata"], false, "{objet} : {premiere}");
        assert_eq!(premiere["rang"], 1);
        let original_le = premiere["original_le"].as_str().unwrap().to_string();
        assert!(original_le.len() >= 10);

        // L'apercu suivant sait deja que ce sera un duplicata.
        let apres = etat(&mut base, objet, id);
        assert_eq!(apres["duplicata"], true, "{objet} : {apres}");
        assert_eq!(apres["original_le"], original_le.as_str());

        let seconde = noter(&mut base, objet, id, "poste-a");
        assert_eq!(seconde["duplicata"], true, "{objet} : {seconde}");
        assert_eq!(seconde["rang"], 2);
        assert_eq!(seconde["original_le"], original_le.as_str(), "la date est celle de l'original");
        assert_eq!(impressions(&mut base, id), 2);
    }
}

#[test]
fn un_autre_poste_une_autre_personne_impriment_un_duplicata() {
    let mut base = base_duplicata();
    let (piece, _) = facture_et_recu(&mut base);
    let admin = utilisateur(&mut base, "admin");
    let employe = utilisateur(&mut base, "employe");
    {
        let _g = gescom_noyau::auteur::poser(&admin);
        assert_eq!(noter(&mut base, PIECE, &piece, "poste-a")["duplicata"], false);
    }
    {
        let _g = gescom_noyau::auteur::poser(&employe);
        assert_eq!(noter(&mut base, PIECE, &piece, "poste-b")["duplicata"], true, "autre poste, autre personne");
    }
    // Qui, d'ou : c'est la session qui signe (D26).
    let lignes = base
        .lire_plusieurs(
            "SELECT rang, original, auteur_id, poste_id FROM impression WHERE objet_id = ?1 ORDER BY rang",
            &parametres![piece.clone()],
            |r| Ok((r.get::<i64>(0)?, r.get::<i64>(1)?, r.get::<Option<String>>(2)?, r.get::<Option<String>>(3)?)),
        )
        .unwrap();
    assert_eq!(
        lignes,
        vec![
            (1, 1, Some(admin), Some("poste-a".to_string())),
            (2, 0, Some(employe), Some("poste-b".to_string())),
        ]
    );
}

#[test]
fn l_original_reimprime_par_le_patron_est_trace() {
    let mut base = base_duplicata();
    let (piece, paiement) = facture_et_recu(&mut base);
    let admin = utilisateur(&mut base, "admin");
    let premiere = noter(&mut base, PIECE, &piece, "poste-a");
    let original_le = premiere["original_le"].as_str().unwrap().to_string();

    {
        let _g = gescom_noyau::auteur::poser(&admin);
        let r = impression::reimprimer_original_sur(&mut base, PIECE.into(), piece.clone(), None, Some("poste-b".into()))
            .expect("original du patron");
        assert_eq!(r["duplicata"], false, "{r}");
        assert_eq!(r["rang"], 2);
    }

    // L'Historique le dit, avec la piece et l'auteur.
    let h = historique::lire_historique_sur(
        &mut base,
        Filtre { type_evenement: Some("original_reimprime".into()), ..Default::default() },
    )
    .unwrap();
    let lignes = h["lignes"].as_array().unwrap();
    assert_eq!(lignes.len(), 1, "{h}");
    assert_eq!(lignes[0]["piece_id"], piece.as_str());
    assert!(lignes[0]["piece_numero"].is_string());
    assert_eq!(lignes[0]["auteur_id"], admin.as_str());
    assert_eq!(lignes[0]["libelle_type"], "Original réimprimé");
    assert_eq!(lignes[0]["ancien"]["original_le"], original_le.as_str());

    // Apres lui, on revient aux duplicatas — et l'original reste date du premier jour.
    let suivante = noter(&mut base, PIECE, &piece, "poste-a");
    assert_eq!(suivante["duplicata"], true);
    assert_eq!(suivante["original_le"], original_le.as_str());

    // Un recu jamais imprime : rien n'est REimprime, donc rien au journal.
    let r = impression::reimprimer_original_sur(&mut base, RECU.into(), paiement.clone(), None, None).unwrap();
    assert_eq!(r["duplicata"], false);
    assert_eq!(
        compter(&mut base, "SELECT COUNT(*) FROM journal WHERE type_evenement = 'original_reimprime'", &[]),
        1
    );
}

#[test]
fn case_decochee_on_compte_mais_rien_ne_dit_duplicata() {
    let mut base = base_avec_demo();
    let (piece, _) = facture_et_recu(&mut base);
    assert_eq!(etat(&mut base, PIECE, &piece)["actif"], false, "non par défaut");
    for rang in 1..=2 {
        let r = noter(&mut base, PIECE, &piece, "poste-a");
        assert_eq!(r["duplicata"], false, "{r}");
        assert_eq!(r["actif"], false);
        assert_eq!(r["rang"], rang, "compté quand même");
    }
    // Rien a rattraper : le patron n'a pas d'original a ressortir.
    assert!(impression::reimprimer_original_sur(&mut base, PIECE.into(), piece.clone(), None, None).is_err());

    // Cochee plus tard : la feuille deja sortie reste l'original.
    regler_duplicata(&mut base, "facture", true);
    let e = etat(&mut base, PIECE, &piece);
    assert_eq!(e["actif"], true);
    assert_eq!(e["duplicata"], true, "{e}");
    assert_eq!(noter(&mut base, PIECE, &piece, "poste-a")["duplicata"], true);
}

#[test]
fn le_ticket_a_sa_case_mais_partage_l_original_de_sa_piece() {
    let mut base = base_duplicata();
    let (piece, _) = facture_et_recu(&mut base);
    let ticket = |base: &mut Base| {
        impression::noter_impression_sur(base, PIECE.into(), piece.clone(), Some("thermique_80".into()), None).unwrap()
    };
    // Facture cochee, ticket non : le rouleau sort sans mention.
    assert_eq!(noter(&mut base, PIECE, &piece, "poste-a")["duplicata"], false);
    let t = ticket(&mut base);
    assert_eq!(t["genre"], "ticket");
    assert_eq!(t["duplicata"], false, "{t}");

    // Ticket coche : c'est la meme piece, l'original est deja sorti en A4.
    regler_duplicata(&mut base, "ticket", true);
    let t = ticket(&mut base);
    assert_eq!(t["duplicata"], true, "{t}");
    assert_eq!(t["rang"], 3);
}

#[test]
fn reimprimer_l_original_est_le_droit_du_patron() {
    let mut base = base_duplicata();
    assert!(portes::existe("pieces:reimprimer_original"));
    assert!(portes::permissions_de_sur(&mut base, "x", "patron").contains("pieces:reimprimer_original"));
    for role in ["caissier", "magasinier", "comptable", "employe"] {
        assert!(
            !portes::permissions_de_sur(&mut base, "x", role).contains("pieces:reimprimer_original"),
            "{role} ne l'a pas d'office"
        );
    }
}

#[test]
fn un_brouillon_ne_se_compte_pas() {
    let mut base = base_duplicata();
    let client = client_reel(&mut base);
    let sucre = article_unite(&mut base, "Sucre");
    let devis = pieces::creer_piece_sur_base(
        &mut base, client, "devis".into(), vec![ligne(&sucre, 1.0)], None, None, None, None, None, None,
    )
    .expect("devis");
    let id = devis["id"].as_str().unwrap().to_string();
    assert_eq!(statut_piece(&mut base, &id), "brouillon");

    assert_eq!(etat(&mut base, PIECE, &id)["compte"], false);
    for _ in 0..2 {
        let r = noter(&mut base, PIECE, &id, "poste-a");
        assert_eq!(r["compte"], false);
        assert_eq!(r["duplicata"], false);
    }
    assert_eq!(impressions(&mut base, &id), 0);
    assert!(impression::reimprimer_original_sur(&mut base, PIECE.into(), id, None, None).is_err());
}

#[test]
fn un_objet_inconnu_ou_introuvable_est_refuse() {
    let mut base = base_duplicata();
    let (piece, _) = facture_et_recu(&mut base);
    assert!(impression::noter_impression_sur(&mut base, "vente".into(), piece, None, None).is_err());
    for objet in [PIECE, RECU, "paiement_fournisseur"] {
        assert!(impression::noter_impression_sur(&mut base, objet.into(), "inconnu".into(), None, None).is_err());
        assert!(impression::lire_etat_impression_sur(&mut base, objet.into(), "inconnu".into(), None).is_err());
    }
    assert_eq!(compter(&mut base, "SELECT COUNT(*) FROM impression", &[]), 0);
}

#[test]
fn une_piece_d_un_autre_dossier_ne_se_compte_pas() {
    let mut base = base_duplicata();
    let (piece, _) = facture_et_recu(&mut base);
    noter(&mut base, PIECE, &piece, "poste-a");

    let b = dossiers::creer_dossier_sur(&mut base, "B".into(), "Boutique B".into(), None, None).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    base.choisir_dossier(&b).unwrap();
    assert!(impression::noter_impression_sur(&mut base, PIECE.into(), piece.clone(), None, None).is_err());
    assert!(impression::lire_etat_impression_sur(&mut base, PIECE.into(), piece.clone(), None).is_err());
    assert!(impression::reimprimer_original_sur(&mut base, PIECE.into(), piece.clone(), None, None).is_err());
    let dossier_b = base.dossier().to_string();
    assert_eq!(
        compter(&mut base, "SELECT COUNT(*) FROM impression WHERE dossier_id = ?1", &parametres![dossier_b]),
        0
    );
}

#[test]
fn tout_ce_qui_est_porte_ici_passe_le_detecteur() {
    let mut base = base_duplicata();
    let (piece, paiement) = facture_et_recu(&mut base);
    base.auditer(true);
    etat(&mut base, PIECE, &piece);
    noter(&mut base, PIECE, &piece, "poste-a");
    noter(&mut base, RECU, &paiement, "poste-a");
    impression::reimprimer_original_sur(&mut base, PIECE.into(), piece.clone(), None, None).unwrap();
    etat(&mut base, RECU, &paiement);
    let _ = impression::noter_impression_sur(&mut base, "paiement_fournisseur".into(), "x".into(), None, None);
}
