//! Lot C — les garde-fous du terrain (27/10/2026). Teste sur SQLite par
//! defaut, sur PostgreSQL avec `GESCOM_PG` (et sous le compte limite
//! avec `GESCOM_PG_LIMITE`).
//!
//! - C.5 : une horloge qui recule. La reference (derniere ecriture
//!   reussie) se pose, l'ecriture est refusee au-dela de cinq minutes de
//!   recul, acceptee a quatre ; le geste du patron ramene une reference
//!   restee dans le futur, et le journal le garde.
//! - C.6 : la caisse par utilisateur est refusee ; une base qui l'avait
//!   garde un seul tiroir par dossier.
//! - C.7 : les lignes d'une piece disent son tiers, pour que le serveur
//!   masque les prix d'achat (`coeur::lecture`).
//! - C.8 : un article non suivi reste achetable, et la lecture des
//!   ecrans d'achat le dit.
//!
//! Les heures sont DONNEES : le scenario ne depend pas de l'heure ou il
//! tourne.

mod commun;

use commun::*;
use gescom_noyau::base::Base;
use gescom_noyau::{caisses, catalogue, horloge, parametres, pieces};

const LE_27_A_10H: &str = "2026-10-27T10:00:00.000";

fn reference(base: &mut Base) -> Option<String> {
    horloge::lire_reference_sur(base).expect("lire la référence")
}

fn patron(base: &mut Base) -> String {
    base.lire_une(
        "SELECT u.id FROM utilisateur u JOIN role r ON r.id = u.role_id WHERE r.nom = 'patron' ORDER BY u.cree_le",
        &[],
        |r| r.get::<String>(0),
    )
    .unwrap()
    .expect("le patron de la démo")
}

// =====================================================================
//  C.5 — l'horloge
// =====================================================================

#[test]
fn une_horloge_qui_recule_de_plus_de_cinq_minutes_bloque_les_ecritures() {
    let mut base = base_avec_demo();

    // Base neuve : aucune reference, tout passe.
    assert_eq!(reference(&mut base), None);
    horloge::verifier_sur(&mut base, LE_27_A_10H).expect("pas de référence : on écrit");

    // Une ecriture reussie pose la reference, en base (`config_app`,
    // commune a tous les dossiers) : elle survit au redemarrage.
    horloge::noter_ecriture_sur(&mut base, LE_27_A_10H).unwrap();
    assert_eq!(reference(&mut base).as_deref(), Some(LE_27_A_10H));
    let en_base = base
        .lire_une("SELECT valeur FROM config_app WHERE cle = ?1", &parametres![horloge::CLE], |r| r.get::<String>(0))
        .unwrap();
    assert_eq!(en_base.as_deref(), Some(LE_27_A_10H));

    // Quatre minutes en arriere : une resynchronisation, ca passe.
    horloge::verifier_sur(&mut base, "2026-10-27T09:56:00.000").expect("4 min de recul");
    // Cinq minutes pile : la borne passe encore.
    horloge::verifier_sur(&mut base, "2026-10-27T09:55:00.000").expect("5 min pile");
    // Au-dela : refuse, et le message dit quoi faire.
    let refus = horloge::verifier_sur(&mut base, "2026-10-27T09:54:00.000").unwrap_err();
    assert!(refus.starts_with("HORLOGE_RECULE — "), "{refus}");
    assert!(refus.contains("dernière écriture le 27/10/2026 à 10:00"), "{refus}");
    assert!(refus.contains("il est maintenant le 27/10/2026 à 09:54"), "{refus}");
    assert!(refus.contains("corriger la date et l'heure de Windows"), "{refus}");
    assert!(refus.contains("Paramètres → Horloge"), "le recours du patron est nommé : {refus}");
    // La pile du BIOS : la machine repart a hier.
    assert!(horloge::verifier_sur(&mut base, "2026-10-26T10:00:00.000").is_err());

    // Une ecriture faite dans la tolerance ne fait pas reculer la
    // reference : quatre minutes par ecriture ne font pas une heure.
    horloge::noter_ecriture_sur(&mut base, "2026-10-27T09:56:00.000").unwrap();
    assert_eq!(reference(&mut base).as_deref(), Some(LE_27_A_10H));
    // Plus tard, elle avance.
    horloge::noter_ecriture_sur(&mut base, "2026-10-27T11:30:00.000").unwrap();
    assert_eq!(reference(&mut base).as_deref(), Some("2026-10-27T11:30:00.000"));

    // L'ecran le dit ; le patron ne peut rien « confirmer » qui ne bloque
    // pas — le geste n'efface pas une trace sans raison.
    let etat = horloge::lire_horloge_sur(&mut base, "2026-10-27T11:31:00.000").unwrap();
    assert_eq!(etat["bloque"], false);
    assert_eq!(etat["tolerance_minutes"], 5);
    assert_eq!(etat["derniere_ecriture_en_clair"], "le 27/10/2026 à 11:30");
    let e = horloge::confirmer_heure_sur(&mut base, "2026-10-27T11:31:00.000").unwrap_err();
    assert!(e.contains("rien à confirmer"), "{e}");
    let dossier = base.dossier().to_string();
    let n = compter(
        &mut base,
        "SELECT CAST(COUNT(*) AS BIGINT) FROM journal WHERE type_evenement = 'horloge_confirmee' AND dossier_id = ?1",
        &parametres![dossier],
    );
    assert_eq!(n, 0, "un refus n'écrit rien au journal");
}

#[test]
fn une_horloge_avancee_puis_corrigee_se_leve_par_le_patron_et_se_trace() {
    let mut base = base_avec_demo();
    let patron = patron(&mut base);
    let _auteur = gescom_noyau::auteur::poser(&patron);

    // Le piege : quelqu'un a regle Windows en 2030, une vente s'est
    // ecrite, puis on a remis la bonne date. La reference est dans le
    // futur : tout serait bloque jusqu'en 2030.
    horloge::noter_ecriture_sur(&mut base, "2030-01-01T08:00:00.000").unwrap();
    let refus = horloge::verifier_sur(&mut base, LE_27_A_10H).unwrap_err();
    assert!(refus.contains("le 01/01/2030 à 08:00"), "{refus}");
    let etat = horloge::lire_horloge_sur(&mut base, LE_27_A_10H).unwrap();
    assert_eq!(etat["bloque"], true);
    assert_eq!(etat["refus"], refus.as_str(), "l'écran montre le refus mot pour mot");

    // Le patron confirme : la reference revient a maintenant.
    let r = horloge::confirmer_heure_sur(&mut base, LE_27_A_10H).expect("confirmer");
    assert_eq!(r["ancienne"], "2030-01-01T08:00:00.000");
    assert_eq!(r["nouvelle"], LE_27_A_10H);
    assert_eq!(reference(&mut base).as_deref(), Some(LE_27_A_10H));
    horloge::verifier_sur(&mut base, "2026-10-27T10:01:00.000").expect("les écritures reprennent");

    // Le journal du dossier garde l'ancienne et la nouvelle valeur, et
    // qui l'a fait.
    let dossier = base.dossier().to_string();
    let trace = base
        .lire_une(
            "SELECT ancien_valeur, nouveau_valeur, auteur_id, date_evenement, entite_type FROM journal
             WHERE type_evenement = 'horloge_confirmee' AND dossier_id = ?1",
            &parametres![dossier],
            |r| {
                Ok((
                    r.get::<Option<String>>(0)?,
                    r.get::<Option<String>>(1)?,
                    r.get::<Option<String>>(2)?,
                    r.get::<String>(3)?,
                    r.get::<String>(4)?,
                ))
            },
        )
        .unwrap()
        .expect("le geste est au journal");
    assert_eq!(trace.0.as_deref(), Some("2030-01-01T08:00:00.000"));
    assert_eq!(trace.1.as_deref(), Some(LE_27_A_10H));
    assert_eq!(trace.2.as_deref(), Some(patron.as_str()));
    assert_eq!(trace.3, LE_27_A_10H);
    assert_eq!(trace.4, "horloge");
    assert_eq!(gescom_noyau::coeur::historique::libelle_type("horloge_confirmee"), "Heure de l'ordinateur confirmée");

    // Confirmer une seconde fois : plus rien ne bloque, refuse.
    assert!(horloge::confirmer_heure_sur(&mut base, "2026-10-27T10:02:00.000").is_err());
}

// =====================================================================
//  C.6 — la caisse par utilisateur, retiree
// =====================================================================

#[test]
fn la_caisse_par_utilisateur_est_refusee_et_une_base_qui_l_avait_garde_un_tiroir() {
    let mut base = base_avec_demo();

    let refus = caisses::definir_par_utilisateur_sur(&mut base, true).unwrap_err();
    assert!(refus.starts_with("MODE_CAISSE_INDISPONIBLE"), "{refus}");
    assert!(refus.contains("un seul tiroir par dossier"), "{refus}");
    assert!(!caisses::par_utilisateur_sur(&mut base));

    // Une base qui porte encore '1' (essai d'avant le retrait) : le mode
    // par defaut. La caisse ouverte du dossier sert a tout le monde, une
    // operation sans utilisateur n'est plus refusee (`CAISSE_SANS_
    // UTILISATEUR` n'existe plus), et une seconde caisse ne s'ouvre pas.
    base.executer("UPDATE config_app SET valeur = '1' WHERE cle = 'caisse_par_utilisateur'", &[]).unwrap();
    assert!(!caisses::par_utilisateur_sur(&mut base));
    let sid = ouvrir_caisse(&mut base);
    assert_eq!(caisses::exiger_sur(&mut base, None).unwrap(), sid);
    assert_eq!(caisses::exiger_sur(&mut base, Some("quelqu-un-d-autre")).unwrap(), sid);
    let depense = gescom_noyau::caisse::enregistrer_depense_sur_base(
        &mut base, 500, "Thé".into(), Some("divers".into()), None, Some("patron".into()),
    );
    assert!(depense.is_ok(), "l'argent sort du seul tiroir : {depense:?}");
    let seconde = gescom_noyau::caisse::ouvrir_session_caisse_sur(&mut base, 0, "patron".into()).unwrap_err();
    assert!(seconde.contains("déjà ouverte"), "{seconde}");

    // Revenir au tiroir unique est permis, caisse ouverte ou non.
    caisses::definir_par_utilisateur_sur(&mut base, false).unwrap();
    let v = base
        .lire_une("SELECT valeur FROM config_app WHERE cle = 'caisse_par_utilisateur'", &[], |r| r.get::<String>(0))
        .unwrap();
    assert_eq!(v.as_deref(), Some("0"));

    // La version `Connection` (fenetre monoposte), meme regle.
    if let Some(conn) = base.sqlite() {
        assert!(caisses::definir_par_utilisateur(conn, true).unwrap_err().starts_with("MODE_CAISSE_INDISPONIBLE"));
        assert!(!caisses::par_utilisateur(conn));
        assert_eq!(caisses::exiger(conn, Some("x")).unwrap(), sid);
    }
}

// =====================================================================
//  C.7 — les prix d'achat des lignes de piece
// =====================================================================

#[test]
fn les_lignes_d_une_piece_disent_leur_tiers_et_le_serveur_en_masque_les_prix() {
    use gescom_noyau::coeur::lecture;
    let mut base = base_avec_demo();
    let sucre = article_unite(&mut base, "Sucre");
    let f = fournisseur(&mut base, "Grossiste");
    let client = client_reel(&mut base);

    let bcf = pieces::creer_piece_fournisseur_sur_base(
        &mut base, f, "bon_commande_fournisseur".into(), vec![ligne(&sucre, 6.0)], None, None, None, None, None,
    )
    .unwrap();
    let bcf_id = bcf["id"].as_str().unwrap().to_string();
    let devis = pieces::creer_piece_sur_base(
        &mut base, client, "devis".into(), vec![ligne(&sucre, 2.0)], None, None, None, None, None, None,
    )
    .unwrap();
    let devis_id = devis["id"].as_str().unwrap().to_string();

    let lignes_f = pieces::lire_lignes_piece_sur_base(&mut base, bcf_id.clone()).unwrap();
    assert_eq!(lignes_f.len(), 1);
    assert_eq!(lignes_f[0]["tiers_type"], "fournisseur");
    assert_eq!(lignes_f[0]["prix_unitaire"], sucre.3);
    let lignes_c = pieces::lire_lignes_piece_sur_base(&mut base, devis_id.clone()).unwrap();
    assert_eq!(lignes_c[0]["tiers_type"], "client");

    // Ce que le serveur rend a qui n'a ni `achats:lire_prix` ni
    // `achats:creer` (un caissier) : la quantite, pas le prix.
    let sans = |_: &str| false;
    let mut v = serde_json::Value::Array(lignes_f.clone());
    lecture::filtrer("lire_lignes_piece", sans, "u", &mut v);
    assert!(v[0]["prix_unitaire"].is_null() && v[0]["montant_ht"].is_null(), "{v}");
    assert_eq!(v[0]["quantite"], 6.0);
    let mut c = serde_json::Value::Array(lignes_c.clone());
    lecture::filtrer("lire_lignes_piece", sans, "u", &mut c);
    assert_eq!(c[0]["prix_unitaire"], sucre.3, "le devis client garde ses prix");

    // La version `Connection` dit le tiers aussi.
    if let Some(conn) = base.sqlite() {
        let l = pieces::lire_lignes_piece(conn, bcf_id).unwrap();
        assert_eq!(l[0]["tiers_type"], "fournisseur");
        let l = pieces::lire_lignes_piece(conn, devis_id).unwrap();
        assert_eq!(l[0]["tiers_type"], "client");
    }
}

// =====================================================================
//  C.8 — un article non suivi, a l'achat
// =====================================================================

#[test]
fn un_article_non_suivi_reste_achetable_et_la_lecture_des_achats_le_dit() {
    let mut base = base_avec_demo();
    let sucre = article_unite(&mut base, "Sucre");
    let riz = article_unite(&mut base, "Riz local");
    // D36 : la colonne existe ; le geste de Parametres → Articles est
    // eprouve ailleurs (`suivi_stock_base`).
    base.executer("UPDATE article SET gere_en_stock = 0 WHERE id = ?1", &parametres![sucre.0.clone()]).unwrap();

    // La lecture des ecrans d'achat (Achats, nouvelle piece) porte le
    // drapeau, dans ses deux versions.
    let articles = catalogue::lire_articles_avec_unites_sur(&mut base, false, None).unwrap();
    let drapeau = |id: &str| articles.iter().find(|a| a["id"] == id).map(|a| a["gere_en_stock"].clone());
    assert_eq!(drapeau(&sucre.0), Some(serde_json::json!(false)));
    assert_eq!(drapeau(&riz.0), Some(serde_json::json!(true)));
    if let Some(conn) = base.sqlite() {
        let articles = catalogue::lire_articles_avec_unites(conn, false, None).unwrap();
        let a = articles.iter().find(|a| a["id"] == sucre.0.as_str()).unwrap();
        assert_eq!(a["gere_en_stock"], false);
    }

    // Il reste achetable : une piece fournisseur le prend.
    let f = fournisseur(&mut base, "Grossiste");
    let bcf = pieces::creer_piece_fournisseur_sur_base(
        &mut base, f, "bon_commande_fournisseur".into(), vec![ligne(&sucre, 3.0)], None, None, None, None, None,
    );
    assert!(bcf.is_ok(), "{bcf:?}");
}

// =====================================================================
//  Le detecteur
// =====================================================================

#[test]
fn tout_ce_que_touche_le_lot_c_passe_le_detecteur() {
    let mut base = base_avec_demo();
    let sucre = article_unite(&mut base, "Sucre");
    let f = fournisseur(&mut base, "Grossiste");
    base.auditer(true);

    horloge::verifier_sur(&mut base, LE_27_A_10H).unwrap();
    horloge::noter_ecriture_sur(&mut base, "2030-01-01T08:00:00.000").unwrap();
    horloge::lire_horloge_sur(&mut base, LE_27_A_10H).unwrap();
    horloge::confirmer_heure_sur(&mut base, LE_27_A_10H).unwrap();
    horloge::noter_ecriture_sur(&mut base, "2026-10-27T10:05:00.000").unwrap();

    assert!(caisses::definir_par_utilisateur_sur(&mut base, true).is_err());
    caisses::definir_par_utilisateur_sur(&mut base, false).unwrap();
    caisses::par_utilisateur_sur(&mut base);
    let sid = ouvrir_caisse(&mut base);
    assert_eq!(caisses::exiger_sur(&mut base, None).unwrap(), sid);

    let bcf = pieces::creer_piece_fournisseur_sur_base(
        &mut base, f, "bon_commande_fournisseur".into(), vec![ligne(&sucre, 1.0)], None, None, None, None, None,
    )
    .unwrap();
    pieces::lire_lignes_piece_sur_base(&mut base, bcf["id"].as_str().unwrap().to_string()).unwrap();
    catalogue::lire_articles_avec_unites_sur(&mut base, false, None).unwrap();
}
