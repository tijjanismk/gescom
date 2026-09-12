//! `pieces::*_sur_base` — le cycle des pièces, sur `Base`.
//!
//! Devis → commande → bon de livraison → facture, et ce que chaque
//! étape fait RÉELLEMENT à la base : la pièce source fermée, les
//! lignes copiées, le stock sorti UNE fois, l'avoir qui rend ce qu'il
//! doit rendre. Testé sur SQLite par défaut, sur PostgreSQL avec
//! `GESCOM_PG` (voir `commun`).

mod commun;

use commun::*;
use gescom_noyau::base::Base;
use gescom_noyau::parametres;
use gescom_noyau::{argent, pieces};

fn devis(base: &mut Base, quantite: f64) -> serde_json::Value {
    let client = client_reel(base);
    let sucre = article_unite(base, "Sucre");
    pieces::creer_piece_sur_base(
        base,
        client,
        "devis".into(),
        vec![ligne(&sucre, quantite)],
        None,
        None,
        Some("Devis de test".into()),
        None,
        None,
    )
    .expect("créer un devis")
}

fn id(v: &serde_json::Value) -> String {
    v["id"].as_str().unwrap().to_string()
}

#[test]
fn un_devis_nait_en_brouillon_avec_ses_lignes_et_son_numero() {
    let mut base = base_avec_demo();
    let sucre = article_unite(&mut base, "Sucre");
    let d = devis(&mut base, 3.0);
    assert_eq!(d["statut"], "brouillon");
    assert!(d["numero"].as_str().unwrap().starts_with("DEV-"), "{d}");

    let lignes = pieces::lire_lignes_piece_sur_base(&mut base, id(&d)).unwrap();
    assert_eq!(lignes.len(), 1);
    assert_eq!(lignes[0]["quantite"], 3.0);
    assert_eq!(lignes[0]["montant_ht"], 3 * sucre.3);
    assert_eq!(lignes[0]["article_nom"], "Sucre");

    // Le magasin par défaut est posé sur la pièce dès sa création.
    let depot = depot_defaut(&mut base);
    let sur_piece: Option<String> = base
        .lire_une(
            "SELECT depot_id FROM piece_commerciale WHERE id = ?1",
            &parametres![id(&d)],
            |r| r.get::<Option<String>>(0),
        )
        .unwrap()
        .flatten();
    assert_eq!(sur_piece.as_deref(), Some(depot.as_str()));
}

#[test]
fn la_liste_des_pieces_filtre_par_type_recherche_et_client() {
    let mut base = base_avec_demo();
    let client = client_reel(&mut base);
    let d = devis(&mut base, 1.0);
    let numero = d["numero"].as_str().unwrap().to_string();

    let toutes = pieces::lire_toutes_pieces_client_sur_base(
        &mut base, None, None, None, None, None, None, None, None, None, None,
    )
    .unwrap();
    assert_eq!(toutes.len(), 1);
    assert_eq!(toutes[0]["numero"], numero);
    assert_eq!(toutes[0]["etat_livraison"], "non_livre");

    // « devis » couvre devis ET proforma ; « facture » n'a rien.
    let devis_seuls = pieces::lire_pieces_client_sur_base(&mut base, client.clone(), Some("devis".into())).unwrap();
    assert_eq!(devis_seuls.len(), 1);
    let factures = pieces::lire_pieces_client_sur_base(&mut base, client, Some("facture".into())).unwrap();
    assert!(factures.is_empty());

    // La recherche ignore la casse sur les deux moteurs.
    let cherche = pieces::lire_toutes_pieces_client_sur_base(
        &mut base, None, None, Some(numero.to_lowercase()), None, None, None, None, None, None, None,
    )
    .unwrap();
    assert_eq!(cherche.len(), 1, "recherche en minuscules sur un numéro en majuscules");

    // Un client inconnu ne voit rien ; un montant minimum trop haut non plus.
    let personne = pieces::lire_pieces_client_sur_base(&mut base, "personne".into(), None).unwrap();
    assert!(personne.is_empty());
    let trop_cher = pieces::lire_toutes_pieces_client_sur_base(
        &mut base, None, None, None, None, None, Some(10_000_000), None, None, None, None,
    )
    .unwrap();
    assert!(trop_cher.is_empty());
}

#[test]
fn un_avoir_client_cree_a_la_main_porte_son_credit() {
    let mut base = base_avec_demo();
    let client = client_reel(&mut base);
    let sucre = article_unite(&mut base, "Sucre");
    let avc = pieces::creer_piece_sur_base(
        &mut base, client, "avoir_client".into(), vec![ligne(&sucre, 2.0)],
        None, None, None, None, None,
    )
    .unwrap();
    assert_eq!(avc["statut"], "emis");

    let credit = compter(
        &mut base,
        "SELECT CAST(COALESCE(SUM(montant), 0) AS BIGINT) FROM avoir WHERE piece_id = ?1 AND statut = 'ouvert'",
        &parametres![id(&avc)],
    );
    assert_eq!(credit, 2 * sucre.3, "le geste commercial crée son crédit (D47)");

    // Sur la liste, le « reste » d'un AVC est le crédit ouvert (D44).
    let client = client_reel(&mut base);
    let liste = pieces::lire_pieces_client_sur_base(&mut base, client, Some("avoir_client".into())).unwrap();
    assert_eq!(liste[0]["reste"], 2 * sucre.3);

    // Et il ne s'annule pas tant que le crédit est ouvert.
    let refus = pieces::annuler_piece_sur_base(&mut base, id(&avc), None).unwrap_err();
    assert!(refus.contains("crédit non consommé"), "{refus}");
}

#[test]
fn convertir_un_devis_ferme_la_source_et_refuse_un_second_transfert() {
    let mut base = base_avec_demo();
    let d = devis(&mut base, 2.0);

    let cmd = pieces::convertir_piece_sur_base(&mut base, id(&d), "commande_client".into()).unwrap();
    assert_eq!(cmd["type_piece"], "commande_client");
    assert_eq!(cmd["statut"], "brouillon");
    assert!(cmd["numero"].as_str().unwrap().starts_with("CMD-"));
    assert_eq!(statut_piece(&mut base, &id(&d)), "transfere");

    let lignes = pieces::lire_lignes_piece_sur_base(&mut base, id(&cmd)).unwrap();
    assert_eq!(lignes.len(), 1, "les lignes suivent");
    assert_eq!(lignes[0]["quantite"], 2.0);

    let refus = pieces::convertir_piece_sur_base(&mut base, id(&d), "facture".into()).unwrap_err();
    assert!(refus.contains(cmd["numero"].as_str().unwrap()), "le refus nomme la pièce issue : {refus}");

    // Une conversion hors table est refusée avant d'écrire quoi que ce soit.
    let d2 = devis(&mut base, 1.0);
    let refus = pieces::convertir_piece_sur_base(&mut base, id(&d2), "bon_livraison".into()).unwrap_err();
    assert!(refus.contains("non autorisée"));
    assert_eq!(statut_piece(&mut base, &id(&d2)), "brouillon");
}

#[test]
fn un_bon_de_livraison_sort_le_stock_et_la_facture_qui_suit_ne_le_sort_pas() {
    let mut base = base_avec_demo();
    let sucre = article_unite(&mut base, "Sucre");
    let depot = depot_defaut(&mut base);
    let avant = stock(&mut base, &sucre.0, &depot);

    let d = devis(&mut base, 5.0);
    let cmd = pieces::convertir_piece_sur_base(&mut base, id(&d), "commande_client".into()).unwrap();
    assert_eq!(stock(&mut base, &sucre.0, &depot), avant, "une commande ne bouge aucun stock");

    let bl = pieces::convertir_piece_sur_base(&mut base, id(&cmd), "bon_livraison".into()).unwrap();
    assert_eq!(bl["statut"], "emis");
    assert_eq!(stock(&mut base, &sucre.0, &depot), avant - 5.0, "le bon sort la marchandise");
    let liste = pieces::lire_toutes_pieces_client_sur_base(
        &mut base, Some("bon_livraison".into()), None, None, None, None, None, None, None, None, None,
    )
    .unwrap();
    assert_eq!(liste[0]["etat_livraison"], "livre");

    let fac = pieces::convertir_piece_sur_base(&mut base, id(&bl), "facture".into()).unwrap();
    assert_eq!(fac["statut"], "brouillon");
    ouvrir_caisse(&mut base);
    argent::valider_facture_sur_base(&mut base, id(&fac), "comptant".into(), None, None, None)
        .expect("valider la facture");
    assert_eq!(
        stock(&mut base, &sucre.0, &depot),
        avant - 5.0,
        "la facture issue d'un bon ne sort pas la marchandise une seconde fois"
    );
}

#[test]
fn commande_vers_bon_et_facture_en_un_geste() {
    let mut base = base_avec_demo();
    let sucre = article_unite(&mut base, "Sucre");
    let depot = depot_defaut(&mut base);
    let avant = stock(&mut base, &sucre.0, &depot);

    let d = devis(&mut base, 4.0);
    let cmd = pieces::convertir_piece_sur_base(&mut base, id(&d), "commande_client".into()).unwrap();
    let deux = pieces::convertir_commande_en_livraison_et_facture_sur_base(&mut base, id(&cmd)).unwrap();

    let bl_id = deux["bon_livraison"]["id"].as_str().unwrap().to_string();
    let fac_id = deux["facture"]["id"].as_str().unwrap().to_string();
    assert_eq!(statut_piece(&mut base, &bl_id), "transfere", "le BL a déjà donné sa facture");
    assert_eq!(statut_piece(&mut base, &fac_id), "brouillon");
    assert_eq!(statut_piece(&mut base, &id(&cmd)), "transfere");
    assert_eq!(stock(&mut base, &sucre.0, &depot), avant - 4.0, "sorti une fois, par le bon");

    // La facture descend du BL, qui descend de la commande.
    let origine: Option<String> = base
        .lire_une(
            "SELECT piece_origine_id FROM piece_commerciale WHERE id = ?1",
            &parametres![fac_id],
            |r| r.get::<Option<String>>(0),
        )
        .unwrap()
        .flatten();
    assert_eq!(origine.as_deref(), Some(bl_id.as_str()));

    // Un devis qui n'est pas une commande est refusé.
    let d2 = devis(&mut base, 1.0);
    let refus = pieces::convertir_commande_en_livraison_et_facture_sur_base(&mut base, id(&d2)).unwrap_err();
    assert!(refus.contains("Seule une commande"));
}

#[test]
fn modifier_remplace_les_lignes_d_un_brouillon_et_refuse_une_facture_emise() {
    let mut base = base_avec_demo();
    let sucre = article_unite(&mut base, "Sucre");
    let riz = article_unite(&mut base, "Riz local");
    let d = devis(&mut base, 1.0);

    pieces::modifier_piece_sur_base(
        &mut base, id(&d), Some("modifié".into()), None, Some(10.0),
        Some(vec![ligne(&riz, 7.0), ligne(&sucre, 2.0)]),
    )
    .unwrap();
    let lignes = pieces::lire_lignes_piece_sur_base(&mut base, id(&d)).unwrap();
    assert_eq!(lignes.len(), 2, "les anciennes lignes sont remplacées, pas ajoutées");
    let client = client_reel(&mut base);
    let liste = pieces::lire_pieces_client_sur_base(&mut base, client, None).unwrap();
    assert_eq!(liste[0]["remise_globale"], 10.0);
    assert_eq!(liste[0]["note"], "modifié");

    // Sans remise transmise, l'ancienne est conservée (COALESCE).
    pieces::modifier_piece_sur_base(&mut base, id(&d), None, None, None, None).unwrap();
    let client = client_reel(&mut base);
    let liste = pieces::lire_pieces_client_sur_base(&mut base, client, None).unwrap();
    assert_eq!(liste[0]["remise_globale"], 10.0);

    // Une facture émise est figée.
    let fac = pieces::convertir_piece_sur_base(&mut base, id(&d), "facture".into()).unwrap();
    pieces::changer_statut_piece_sur_base(&mut base, id(&fac), "emis".into()).unwrap();
    let refus = pieces::modifier_piece_sur_base(&mut base, id(&fac), Some("x".into()), None, None, None)
        .unwrap_err();
    assert!(!refus.is_empty());
}

#[test]
fn annuler_un_brouillon_passe_et_une_facture_validee_est_refusee() {
    let mut base = base_avec_demo();
    let d = devis(&mut base, 1.0);
    pieces::annuler_piece_sur_base(&mut base, id(&d), Some("erreur".into())).unwrap();
    assert_eq!(statut_piece(&mut base, &id(&d)), "annule");
    let note: Option<String> = base
        .lire_une("SELECT note FROM piece_commerciale WHERE id = ?1", &parametres![id(&d)], |r| r.get::<Option<String>>(0))
        .unwrap()
        .flatten();
    assert_eq!(note.as_deref(), Some("erreur"), "le motif remplace la note");

    let d2 = devis(&mut base, 1.0);
    let fac = pieces::convertir_piece_sur_base(&mut base, id(&d2), "facture".into()).unwrap();
    ouvrir_caisse(&mut base);
    argent::valider_facture_sur_base(&mut base, id(&fac), "comptant".into(), None, None, None).unwrap();
    let refus = pieces::annuler_piece_sur_base(&mut base, id(&fac), None).unwrap_err();
    assert!(!refus.is_empty(), "une facture qui a produit une vente ne s'annule pas ainsi");
    assert_eq!(statut_piece(&mut base, &id(&fac)), "paye");
}

#[test]
fn dupliquer_donne_un_brouillon_independant() {
    let mut base = base_avec_demo();
    let d = devis(&mut base, 3.0);
    let copie = pieces::dupliquer_piece_sur_base(&mut base, id(&d)).unwrap();
    assert_eq!(copie["statut"], "brouillon");
    assert_ne!(copie["numero"], d["numero"]);

    let (note, origine): (Option<String>, Option<String>) = base
        .lire_une(
            "SELECT note, piece_origine_id FROM piece_commerciale WHERE id = ?1",
            &parametres![id(&copie)],
            |r| Ok((r.get::<Option<String>>(0)?, r.get::<Option<String>>(1)?)),
        )
        .unwrap()
        .unwrap();
    assert_eq!(note.as_deref(), Some("[Copie] Devis de test"));
    assert!(origine.is_none(), "une copie n'est pas une pièce issue de l'originale");
    assert_eq!(pieces::lire_lignes_piece_sur_base(&mut base, id(&copie)).unwrap().len(), 1);

    // L'originale reste convertible : la copie ne compte pas comme transfert.
    pieces::convertir_piece_sur_base(&mut base, id(&d), "commande_client".into())
        .expect("l'original se convertit malgré sa copie");
}

#[test]
fn annuler_une_facture_par_avoir_rend_le_stock_et_l_acompte() {
    let mut base = base_avec_demo();
    let sucre = article_unite(&mut base, "Sucre");
    let depot = depot_defaut(&mut base);
    let avant = stock(&mut base, &sucre.0, &depot);

    let d = devis(&mut base, 2.0);
    let fac = pieces::convertir_piece_sur_base(&mut base, id(&d), "facture".into()).unwrap();
    ouvrir_caisse(&mut base);
    argent::valider_facture_sur_base(&mut base, id(&fac), "comptant".into(), None, None, None).unwrap();
    assert_eq!(stock(&mut base, &sucre.0, &depot), avant - 2.0);

    let vente_id = pieces::lire_vente_de_piece_sur_base(&mut base, id(&fac)).unwrap().expect("vente liée");
    assert_eq!(
        pieces::lire_piece_de_vente_sur_base(&mut base, vente_id.clone()).unwrap().as_deref(),
        Some(id(&fac).as_str()),
        "aller-retour vente ↔ pièce"
    );

    let r = pieces::annuler_facture_par_avoir_sur_base(&mut base, id(&fac), None, None, None).unwrap();
    assert!(r["numero_avoir"].as_str().unwrap().starts_with("AVC-"));
    assert_eq!(r["total"], 2 * sucre.3);
    assert_eq!(r["acompte"], 2 * sucre.3, "payée comptant : tout l'acompte");
    assert_eq!(r["rembourse"], true);

    assert_eq!(stock(&mut base, &sucre.0, &depot), avant, "la marchandise revient");
    assert_eq!(statut_piece(&mut base, &id(&fac)), "annule");
    let statut_vente: String = base
        .lire_une("SELECT statut FROM vente WHERE id = ?1", &parametres![vente_id], |r| r.get::<String>(0))
        .unwrap()
        .unwrap();
    assert_eq!(statut_vente, "annulee");
    let avc_id = r["piece_id"].as_str().unwrap().to_string();
    assert_eq!(statut_piece(&mut base, &avc_id), "paye", "une AVC d'annulation naît close (D44)");
    let sorties = compter(
        &mut base,
        "SELECT CAST(COALESCE(SUM(montant), 0) AS BIGINT) FROM mouvement_caisse
         WHERE sens = 'sortie' AND motif = 'remboursement' AND operation_id = ?1",
        &parametres![avc_id],
    );
    assert_eq!(sorties, 2 * sucre.3, "l'acompte ressort du tiroir");

    let refus = pieces::annuler_facture_par_avoir_sur_base(&mut base, id(&fac), None, None, None).unwrap_err();
    assert!(refus.contains("déjà annulée"));
}

#[test]
fn annuler_par_avoir_en_credit_garde_l_acompte_comme_avoir() {
    let mut base = base_avec_demo();
    let sucre = article_unite(&mut base, "Sucre");
    let d = devis(&mut base, 1.0);
    let fac = pieces::convertir_piece_sur_base(&mut base, id(&d), "facture".into()).unwrap();
    ouvrir_caisse(&mut base);
    argent::valider_facture_sur_base(&mut base, id(&fac), "credit".into(), None, Some(300), None).unwrap();

    let r = pieces::annuler_facture_par_avoir_sur_base(&mut base, id(&fac), Some("avoir".into()), None, None).unwrap();
    assert_eq!(r["acompte"], 300);
    assert_eq!(r["rembourse"], false);
    let client = client_reel(&mut base);
    let credit = compter(
        &mut base,
        "SELECT CAST(COALESCE(SUM(montant), 0) AS BIGINT) FROM avoir
         WHERE client_id = ?1 AND statut = 'ouvert' AND origine = 'annulation'",
        &parametres![client],
    );
    assert_eq!(credit, 300, "l'acompte devient un crédit client");
    let _ = sucre;
}

#[test]
fn les_donnees_d_impression_tiennent_pour_un_client_et_un_fournisseur() {
    let mut base = base_avec_demo();
    let sucre = article_unite(&mut base, "Sucre");
    let d = devis(&mut base, 2.0);
    let donnees = pieces::lire_donnees_piece_sur_base(&mut base, id(&d)).unwrap();
    assert_eq!(donnees["piece"]["tiers_type"], "client");
    assert_eq!(donnees["lignes"].as_array().unwrap().len(), 1);
    assert_eq!(donnees["totaux"]["total_ht"], 2 * sucre.3);
    assert_eq!(donnees["totaux"]["total_ttc"], 2 * sucre.3);
    assert_eq!(donnees["totaux"]["reste_du"], 2 * sucre.3);
    assert!(donnees["societe"]["nom"].is_string());

    let f = fournisseur(&mut base, "Grossiste Sikasso");
    let bcf = pieces::creer_piece_fournisseur_sur_base(
        &mut base, f, "bon_commande_fournisseur".into(), vec![ligne(&sucre, 10.0)],
        None, None, None, None,
    )
    .unwrap();
    assert_eq!(bcf["statut"], "emis");
    let donnees = pieces::lire_donnees_piece_sur_base(&mut base, id(&bcf)).unwrap();
    assert_eq!(donnees["piece"]["tiers_type"], "fournisseur");
    assert_eq!(donnees["piece"]["client_nom"], "Grossiste Sikasso");
    assert!(donnees["piece"]["client_code"].is_null(), "un fournisseur n'a pas de code");

    let liste = pieces::lire_toutes_pieces_fournisseur_sur_base(&mut base, None, None, Some("sikasso".into()), None).unwrap();
    assert_eq!(liste.len(), 1, "recherche fournisseur insensible à la casse");
    assert_eq!(liste[0]["total_ht"], 10 * sucre.3);
}

#[test]
fn la_fiche_client_resume_ses_ventes() {
    let mut base = base_avec_demo();
    let sucre = article_unite(&mut base, "Sucre");
    let client = client_reel(&mut base);
    let d = devis(&mut base, 3.0);
    let fac = pieces::convertir_piece_sur_base(&mut base, id(&d), "facture".into()).unwrap();
    argent::valider_facture_sur_base(&mut base, id(&fac), "credit".into(), None, None, None).unwrap();

    let fiche = pieces::lire_fiche_client_sur_base(&mut base, client).unwrap();
    assert_eq!(fiche["stats"]["nb_ventes"], 1);
    assert_eq!(fiche["stats"]["ca_total"], 3 * sucre.3);
    assert_eq!(fiche["stats"]["encours"], 3 * sucre.3, "à crédit, tout reste dû");
    assert_eq!(fiche["stats"]["nb_pieces"], 2, "le devis et la facture");
    assert!(fiche["stats"]["derniere_vente"].is_string());

    let refus = pieces::lire_fiche_client_sur_base(&mut base, "inconnu".into()).unwrap_err();
    assert!(refus.contains("introuvable"));
}

#[test]
fn deux_dossiers_ne_voient_pas_les_pieces_l_un_de_l_autre() {
    let mut base = base_avec_demo();
    let d = devis(&mut base, 1.0);

    base.choisir_dossier("dossier-b").unwrap();
    let liste = pieces::lire_toutes_pieces_client_sur_base(
        &mut base, None, None, None, None, None, None, None, None, None, None,
    )
    .unwrap();
    assert!(liste.is_empty(), "le devis de l'autre société ne doit pas se voir");
    assert!(pieces::lire_lignes_piece_sur_base(&mut base, id(&d)).unwrap().is_empty());
    let refus = pieces::convertir_piece_sur_base(&mut base, id(&d), "facture".into()).unwrap_err();
    assert!(refus.contains("introuvable"), "on ne convertit pas la pièce d'un autre dossier : {refus}");
}

#[test]
fn tout_ce_qui_est_porte_dans_pieces_passe_le_detecteur() {
    let mut base = base_avec_demo();
    ouvrir_caisse(&mut base);
    base.auditer(true);
    let sucre = article_unite(&mut base, "Sucre");
    let client = client_reel(&mut base);

    let d = devis(&mut base, 2.0);
    pieces::lire_toutes_pieces_client_sur_base(
        &mut base, Some("devis".into()), Some("brouillon".into()), Some("dev".into()),
        Some("2000-01-01".into()), Some("2099-12-31".into()), Some(0), Some(10_000_000),
        Some(true), Some(true), Some(client.clone()),
    )
    .expect("liste client");
    pieces::lire_lignes_piece_sur_base(&mut base, id(&d)).expect("lignes");
    pieces::lire_donnees_piece_sur_base(&mut base, id(&d)).expect("impression");
    pieces::lire_fiche_client_sur_base(&mut base, client.clone()).expect("fiche");
    let copie = pieces::dupliquer_piece_sur_base(&mut base, id(&d)).expect("dupliquer");
    pieces::modifier_piece_sur_base(&mut base, id(&copie), None, None, None, Some(vec![ligne(&sucre, 1.0)]))
        .expect("modifier");
    pieces::annuler_piece_sur_base(&mut base, id(&copie), None).expect("annuler");
    pieces::changer_statut_piece_sur_base(&mut base, id(&copie), "annule".into()).expect("statut");

    let cmd = pieces::convertir_piece_sur_base(&mut base, id(&d), "commande_client".into()).expect("convertir");
    let deux = pieces::convertir_commande_en_livraison_et_facture_sur_base(&mut base, id(&cmd)).expect("BL + facture");
    let fac_id = deux["facture"]["id"].as_str().unwrap().to_string();
    argent::valider_facture_sur_base(&mut base, fac_id.clone(), "comptant".into(), None, None, None).expect("valider");
    pieces::lire_vente_de_piece_sur_base(&mut base, fac_id.clone()).expect("vente de pièce");
    let vente = pieces::lire_vente_de_piece_sur_base(&mut base, fac_id.clone()).unwrap().unwrap();
    pieces::lire_piece_de_vente_sur_base(&mut base, vente).expect("pièce de vente");
    pieces::annuler_facture_par_avoir_sur_base(&mut base, fac_id, None, None, None).expect("annuler par avoir");

    let f = fournisseur(&mut base, "F");
    let bcf = pieces::creer_piece_fournisseur_sur_base(
        &mut base, f.clone(), "bon_commande_fournisseur".into(), vec![ligne(&sucre, 1.0)], None, None, None, None,
    )
    .expect("BCF");
    pieces::lire_toutes_pieces_fournisseur_sur_base(&mut base, Some("bon_commande_fournisseur".into()), Some("emis".into()), Some("f".into()), Some(f))
        .expect("liste fournisseur");
    pieces::convertir_piece_sur_base(&mut base, id(&bcf), "bon_reception".into()).expect("BCF → BRF");
}
