//! D58 — les colonnes des pièces commerciales se cochent, sans « auto ».
//!
//! Ce qui compte pour la boutique : d'office, ni TVA ni remise ; un
//! ancien « auto » devient décoché ; le choix fait à la saisie reste
//! avec la pièce ; et **cacher une colonne ne change jamais un
//! montant** — même TVA, même total, même créance, même stock.

mod commun;

use commun::*;
use gescom_noyau::{documents, parametres, pieces};
use serde_json::json;

const FIXES: [&str; 4] = ["designation", "quantite", "prix_unitaire", "total"];

/// Les fixes et `extra`, dans l'ordre du catalogue — celui que le
/// serveur rend.
fn avec(extra: &[&str]) -> Vec<String> {
    gescom_noyau::coeur::documents::COLONNES
        .iter()
        .filter(|c| FIXES.contains(c) || extra.contains(c))
        .map(|c| c.to_string())
        .collect()
}

/// Une ligne de sucre à 18 % de TVA et 10 % de remise.
fn ligne_taxee(article: &(String, String, f64, i64)) -> pieces::LignePieceInput {
    pieces::LignePieceInput { remise_pct: 10.0, taux_tva: 0.18, ..ligne(article, 3.0) }
}

#[test]
fn d_office_ni_tva_ni_remise_ni_recapitulatif() {
    let mut base = base_avec_demo();
    let v = documents::lire_reglages_sur(&mut base).unwrap();
    for g in ["facture", "devis", "bon_commande", "bon_livraison"] {
        assert_eq!(v["genres"][g]["colonnes"], json!(FIXES), "{g}");
        assert_eq!(v["genres"][g]["recap_tva"], false, "{g}");
        assert!(v["genres"][g].get("colonne_tva").is_none(), "{g} : plus d'« auto »");
    }
}

#[test]
fn un_ancien_reglage_auto_devient_decoche_et_oui_reste_coche() {
    let mut base = base_avec_demo();
    // Ce qu'une base d'avant D58 garde dans config_app.
    let ancien = json!({
        "facture": { "format": "a4", "colonne_remise": "auto", "colonne_tva": "auto", "recap_tva": "auto" },
        "devis": { "format": "a4", "colonne_remise": "oui", "colonne_tva": "non", "recap_tva": "oui",
                   "reference_article": true }
    })
    .to_string();
    base.executer(
        "INSERT INTO config_app (cle, valeur) VALUES ('documents_reglages', ?1)
         ON CONFLICT (cle) DO UPDATE SET valeur = excluded.valeur",
        &parametres![ancien],
    )
    .unwrap();
    let f = documents::lire_reglage_sur(&mut base, "facture").unwrap();
    assert_eq!(f["colonnes"], json!(FIXES), "auto = décoché");
    assert_eq!(f["recap_tva"], false);
    let d = documents::lire_reglage_sur(&mut base, "devis").unwrap();
    assert_eq!(d["colonnes"], json!(["reference", "designation", "quantite", "prix_unitaire", "remise", "total"]));
    assert_eq!(d["recap_tva"], true);
}

#[test]
fn le_reglage_d_un_genre_se_coche_et_une_colonne_fixe_ne_se_retire_pas() {
    let mut base = base_avec_demo();
    let r = documents::enregistrer_reglage_sur(
        &mut base,
        "facture",
        json!({ "format": "a4", "colonnes": ["tva", "designation", "quantite", "prix_unitaire", "total"], "recap_tva": true }),
    )
    .unwrap();
    assert_eq!(r["colonnes"], json!(["designation", "quantite", "prix_unitaire", "tva", "total"]), "rangées");
    let avant = documents::lire_reglage_sur(&mut base, "facture").unwrap();
    let refus = documents::enregistrer_reglage_sur(
        &mut base,
        "facture",
        json!({ "format": "a4", "colonnes": ["designation", "quantite", "total"] }),
    )
    .unwrap_err();
    assert!(refus.contains("Prix unitaire"), "{refus}");
    assert_eq!(documents::lire_reglage_sur(&mut base, "facture").unwrap(), avant, "rien n'a changé");
    // Une caisse pas encore mise à jour envoie l'ancienne forme.
    let r = documents::enregistrer_reglage_sur(&mut base, "devis", json!({ "format": "a4", "colonne_tva": "oui" })).unwrap();
    assert_eq!(r["colonnes"], json!(["designation", "quantite", "prix_unitaire", "tva", "total"]));
}

#[test]
fn cacher_la_tva_et_la_remise_ne_change_aucun_montant() {
    let mut base = base_avec_demo();
    let client = client_reel(&mut base);
    let sucre = article_unite(&mut base, "Sucre");
    let depot = depot_defaut(&mut base);
    let stock_avant = stock(&mut base, &sucre.0, &depot);

    // La même facture, colonnes cachées puis montrées.
    let cachee = pieces::creer_piece_colonnes_sur_base(
        &mut base, Some(avec(&[])), client.clone(), "facture".into(), vec![ligne_taxee(&sucre)],
        None, None, None, None, None, None,
    )
    .unwrap();
    let montree = pieces::creer_piece_colonnes_sur_base(
        &mut base, Some(avec(&["tva", "remise", "reference"])), client.clone(), "facture".into(),
        vec![ligne_taxee(&sucre)], None, None, None, None, None, None,
    )
    .unwrap();
    let lire = |base: &mut gescom_noyau::base::Base, p: &serde_json::Value| {
        pieces::lire_donnees_piece_sur_base(base, p["id"].as_str().unwrap().to_string()).unwrap()
    };
    let c = lire(&mut base, &cachee);
    let m = lire(&mut base, &montree);
    assert_eq!(c["piece"]["colonnes"], json!(FIXES), "gardées avec la pièce");
    assert_eq!(m["piece"]["colonnes"], json!(["reference", "designation", "quantite", "prix_unitaire", "remise", "tva", "total"]));
    assert_eq!(c["totaux"], m["totaux"], "mêmes totaux, colonnes cachées ou non");
    for champ in ["montant_ht", "montant_tva", "remise_pct", "taux_tva"] {
        assert_eq!(c["lignes"][0][champ], m["lignes"][0][champ], "{champ}");
    }
    assert!(c["totaux"]["total_tva"].as_i64().unwrap() > 0, "la TVA cachée reste due");
    assert_eq!(c["totaux"]["reste_du"], c["totaux"]["total_ttc"], "la créance compte la TVA");
    assert_eq!(stock(&mut base, &sucre.0, &depot), stock_avant, "un brouillon ne sort rien du stock");

    // La version `Connection` (le serveur sur fichier SQLite) rend la même chose.
    if let Some(conn) = base.sqlite() {
        let d = pieces::lire_donnees_piece(conn, cachee["id"].as_str().unwrap().to_string()).unwrap();
        assert_eq!(d["piece"]["colonnes"], json!(FIXES));
        assert_eq!(d["totaux"]["total_ttc"], c["totaux"]["total_ttc"]);
    }
}

/// Les chiffres que `src/lib/colonnes.test.ts` attend de `totauxSaisie` :
/// la saisie montre ce que le papier et la créance diront, au franc.
#[test]
fn les_totaux_de_la_saisie_sont_ceux_du_serveur() {
    let mut base = base_avec_demo();
    let client = client_reel(&mut base);
    let sucre = article_unite(&mut base, "Sucre");
    let lignes = vec![
        pieces::LignePieceInput { quantite: 3.0, prix_unitaire: 1000, remise_pct: 10.0, taux_tva: 0.18, ..ligne(&sucre, 1.0) },
        pieces::LignePieceInput { quantite: 2.0, prix_unitaire: 500, ..ligne(&sucre, 1.0) },
    ];
    let p = pieces::creer_piece_colonnes_sur_base(
        &mut base, Some(avec(&[])), client, "facture".into(), lignes, Some(5.0), None, None, None, None, None,
    )
    .unwrap();
    let d = pieces::lire_donnees_piece_sur_base(&mut base, p["id"].as_str().unwrap().to_string()).unwrap();
    assert_eq!(d["totaux"]["total_ht"], 3700);
    assert_eq!(d["totaux"]["remise_montant"], 185);
    assert_eq!(d["totaux"]["total_tva"], 462);
    assert_eq!(d["totaux"]["total_ttc"], 3977);
}

#[test]
fn une_piece_sans_choix_suit_son_genre() {
    let mut base = base_avec_demo();
    let client = client_reel(&mut base);
    let sucre = article_unite(&mut base, "Sucre");
    let p = pieces::creer_piece_sur_base(
        &mut base, client, "devis".into(), vec![ligne(&sucre, 1.0)], None, None, None, None, None, None,
    )
    .unwrap();
    let d = pieces::lire_donnees_piece_sur_base(&mut base, p["id"].as_str().unwrap().to_string()).unwrap();
    assert_eq!(d["piece"]["colonnes"], serde_json::Value::Null);
}

#[test]
fn une_colonne_inconnue_refuse_la_piece_entiere() {
    let mut base = base_avec_demo();
    let client = client_reel(&mut base);
    let sucre = article_unite(&mut base, "Sucre");
    let avant = compter(&mut base, "SELECT COUNT(*) FROM piece_commerciale", &[]);
    let refus = pieces::creer_piece_colonnes_sur_base(
        &mut base, Some(FIXES.iter().chain(&["marge"]).map(|c| c.to_string()).collect()), client, "facture".into(), vec![ligne(&sucre, 1.0)],
        None, None, None, None, None, None,
    )
    .unwrap_err();
    assert!(refus.contains("inconnue"), "{refus}");
    assert_eq!(compter(&mut base, "SELECT COUNT(*) FROM piece_commerciale", &[]), avant, "rien d'écrit");
}

#[test]
fn une_piece_fournisseur_garde_aussi_ses_colonnes() {
    let mut base = base_avec_demo();
    let f = fournisseur(&mut base, "Grossiste Sikasso");
    let sucre = article_unite(&mut base, "Sucre");
    let p = pieces::creer_piece_fournisseur_colonnes_sur_base(
        &mut base, Some(avec(&["tva"])), f, "bon_commande_fournisseur".into(), vec![ligne_taxee(&sucre)],
        None, None, None, None, None,
    )
    .unwrap();
    let d = pieces::lire_donnees_piece_sur_base(&mut base, p["id"].as_str().unwrap().to_string()).unwrap();
    assert_eq!(d["piece"]["colonnes"], json!(["designation", "quantite", "prix_unitaire", "tva", "total"]));
}

// --- Suite de D58 (02/11) : les colonnes suivent la pièce -------------

fn id(v: &serde_json::Value) -> String {
    v["id"].as_str().unwrap().to_string()
}

fn colonnes_de(base: &mut gescom_noyau::base::Base, piece_id: &str) -> serde_json::Value {
    pieces::lire_donnees_piece_sur_base(base, piece_id.to_string()).unwrap()["piece"]["colonnes"].clone()
}

fn total_ttc(base: &mut gescom_noyau::base::Base, piece_id: &str) -> i64 {
    pieces::lire_donnees_piece_sur_base(base, piece_id.to_string()).unwrap()["totaux"]["total_ttc"].as_i64().unwrap()
}

#[test]
fn dupliquer_une_piece_recopie_ses_colonnes() {
    let mut base = base_avec_demo();
    let client = client_reel(&mut base);
    let sucre = article_unite(&mut base, "Sucre");
    let choix = avec(&["reference", "tva"]);
    let d = pieces::creer_piece_colonnes_sur_base(
        &mut base, Some(choix.clone()), client.clone(), "devis".into(), vec![ligne_taxee(&sucre)],
        None, None, None, None, None, None,
    )
    .unwrap();
    let copie = pieces::dupliquer_piece_sur_base(&mut base, id(&d)).unwrap();
    assert_eq!(colonnes_de(&mut base, &id(&copie)), json!(choix), "le double garde les colonnes");
    assert_eq!(total_ttc(&mut base, &id(&copie)), total_ttc(&mut base, &id(&d)), "et les mêmes montants");

    // Une pièce qui suit son genre donne un double qui le suit aussi.
    let sans = pieces::creer_piece_sur_base(
        &mut base, client, "devis".into(), vec![ligne(&sucre, 1.0)], None, None, None, None, None, None,
    )
    .unwrap();
    let copie = pieces::dupliquer_piece_sur_base(&mut base, id(&sans)).unwrap();
    assert_eq!(colonnes_de(&mut base, &id(&copie)), serde_json::Value::Null);

    // La version `Connection` (le serveur sur fichier SQLite) fait pareil.
    if let Some(conn) = base.sqlite() {
        let c = pieces::dupliquer_piece(conn, id(&d)).unwrap();
        let lu = pieces::lire_donnees_piece(conn, id(&c)).unwrap();
        assert_eq!(lu["piece"]["colonnes"], json!(choix));
    }
}

#[test]
fn convertir_une_piece_garde_les_colonnes_de_sa_source() {
    let mut base = base_avec_demo();
    let client = client_reel(&mut base);
    let sucre = article_unite(&mut base, "Sucre");
    let depot = depot_defaut(&mut base);
    let stock_avant = stock(&mut base, &sucre.0, &depot);
    let choix = avec(&["remise"]);
    let devis = pieces::creer_piece_colonnes_sur_base(
        &mut base, Some(choix.clone()), client.clone(), "devis".into(), vec![ligne_taxee(&sucre)],
        None, None, None, None, None, None,
    )
    .unwrap();
    let ttc = total_ttc(&mut base, &id(&devis));

    // Devis -> commande -> bon de livraison + facture, d'un seul geste.
    let cmd = pieces::convertir_piece_sur_base(&mut base, id(&devis), "commande_client".into()).unwrap();
    assert_eq!(colonnes_de(&mut base, &id(&cmd)), json!(choix), "la commande");
    let bl_fac = pieces::convertir_commande_en_livraison_et_facture_sur_base(&mut base, id(&cmd)).unwrap();
    let (bl, fac) = (
        bl_fac["bl_id"].as_str().or(bl_fac["bon_livraison"]["id"].as_str()).map(String::from),
        bl_fac["facture_id"].as_str().or(bl_fac["facture"]["id"].as_str()).map(String::from),
    );
    let (bl, fac) = (bl.expect(&format!("bl dans {bl_fac}")), fac.expect(&format!("facture dans {bl_fac}")));
    assert_eq!(colonnes_de(&mut base, &bl), json!(choix), "le bon de livraison");
    assert_eq!(colonnes_de(&mut base, &fac), json!(choix), "la facture");
    assert_eq!(total_ttc(&mut base, &fac), ttc, "la facture doit ce que disait le devis");
    // Le BL issu d'une commande sort la marchandise ; la facture, rien de plus.
    let sorti = stock_avant - stock(&mut base, &sucre.0, &depot);
    assert_eq!(sorti, 3.0, "les trois kilos du bon, une seule fois");

    // Une source qui suit son genre : la pièce convertie suit le sien.
    let sans = pieces::creer_piece_sur_base(
        &mut base, client, "devis".into(), vec![ligne(&sucre, 1.0)], None, None, None, None, None, None,
    )
    .unwrap();
    let f = pieces::convertir_piece_sur_base(&mut base, id(&sans), "facture".into()).unwrap();
    assert_eq!(colonnes_de(&mut base, &id(&f)), serde_json::Value::Null);
}

#[test]
fn la_conversion_cote_connection_recopie_aussi() {
    let mut base = base_avec_demo();
    let client = client_reel(&mut base);
    let sucre = article_unite(&mut base, "Sucre");
    let choix = avec(&["tva"]);
    let devis = pieces::creer_piece_colonnes_sur_base(
        &mut base, Some(choix.clone()), client, "devis".into(), vec![ligne_taxee(&sucre)],
        None, None, None, None, None, None,
    )
    .unwrap();
    let Some(conn) = base.sqlite_mut() else { return };
    let cmd = pieces::convertir_piece(conn, id(&devis), "commande_client".into()).unwrap();
    let cmd_id = cmd["id"].as_str().or(cmd["piece_id"].as_str()).unwrap().to_string();
    assert_eq!(pieces::lire_donnees_piece(conn, cmd_id.clone()).unwrap()["piece"]["colonnes"], json!(choix));
    let r = pieces::convertir_commande_en_livraison_et_facture(conn, cmd_id).unwrap();
    let fac = r["facture_id"].as_str().or(r["facture"]["id"].as_str()).expect("facture").to_string();
    assert_eq!(pieces::lire_donnees_piece(conn, fac).unwrap()["piece"]["colonnes"], json!(choix));
}

/// L'avoir qui annule une facture garde les colonnes de la facture : le
/// client lit le même tableau, en négatif. La marchandise revient une
/// fois, la créance s'éteint, la caisse ne bouge pas (avoir à crédit).
#[test]
fn l_avoir_d_annulation_garde_les_colonnes_de_la_facture() {
    let mut base = base_avec_demo();
    let client = client_reel(&mut base);
    let sucre = article_unite(&mut base, "Sucre");
    let depot = depot_defaut(&mut base);
    let choix = avec(&["remise", "tva"]);
    let d = pieces::creer_piece_colonnes_sur_base(
        &mut base, Some(choix.clone()), client.clone(), "devis".into(), vec![ligne_taxee(&sucre)],
        None, None, None, None, None, None,
    )
    .unwrap();
    let cmd = pieces::convertir_piece_sur_base(&mut base, id(&d), "commande_client".into()).unwrap();
    let r = pieces::convertir_commande_en_livraison_et_facture_sur_base(&mut base, id(&cmd)).unwrap();
    let fac = r["facture"]["id"].as_str().unwrap().to_string();
    let apres_bl = stock(&mut base, &sucre.0, &depot);
    let caisse = ouvrir_caisse(&mut base);
    gescom_noyau::argent::valider_facture_sur_base(&mut base, fac.clone(), "credit".into(), None, None, None).unwrap();
    let solde_caisse = |base: &mut gescom_noyau::base::Base| {
        compter(base, "SELECT COUNT(*) FROM mouvement_caisse WHERE session_id = ?1", &parametres![caisse.clone()])
    };
    let mouvements_avant = solde_caisse(&mut base);

    let a = pieces::annuler_facture_par_avoir_sur_base(&mut base, fac.clone(), Some("avoir".into()), None, Some("rendu".into()))
        .expect("annuler par avoir");
    let avoir = a["piece_id"].as_str().unwrap().to_string();
    assert_eq!(colonnes_de(&mut base, &avoir), json!(choix), "l'avoir garde les colonnes de la facture");
    assert_eq!(stock(&mut base, &sucre.0, &depot), apres_bl + 3.0, "la marchandise revient, une fois");
    assert_eq!(solde_caisse(&mut base), mouvements_avant, "un avoir à crédit ne touche pas la caisse");
    // La créance tombe avec la vente : facture « annule », vente « annulee ».
    assert_eq!(statut_piece(&mut base, &fac), "annule");
    let vente = pieces::lire_vente_de_piece_sur_base(&mut base, fac).unwrap().expect("vente liée");
    let statut_vente: String = base
        .lire_une("SELECT statut FROM vente WHERE id = ?1", &parametres![vente], |r| r.get::<String>(0))
        .unwrap()
        .unwrap();
    assert_eq!(statut_vente, "annulee", "la créance de la facture est éteinte");
}

#[test]
fn modifier_garde_les_colonnes_ou_pose_celles_du_menu() {
    let mut base = base_avec_demo();
    let client = client_reel(&mut base);
    let sucre = article_unite(&mut base, "Sucre");
    let choix = avec(&["tva"]);
    let d = pieces::creer_piece_colonnes_sur_base(
        &mut base, Some(choix.clone()), client.clone(), "devis".into(), vec![ligne_taxee(&sucre)],
        None, None, None, None, None, None,
    )
    .unwrap();
    // Le genre change entre-temps : la modification ne le reprend pas.
    documents::enregistrer_reglage_sur(&mut base, "devis", json!({ "format": "a4", "colonnes": avec(&["reference"]) })).unwrap();
    pieces::modifier_piece_sur_base(&mut base, id(&d), Some("Livrer lundi".into()), None, None, None, None).unwrap();
    assert_eq!(colonnes_de(&mut base, &id(&d)), json!(choix), "sans menu touché, rien ne bouge");

    pieces::modifier_piece_colonnes_sur_base(
        &mut base, Some(avec(&["remise", "tva"])), id(&d), None, None, None, None, None,
    )
    .unwrap();
    assert_eq!(colonnes_de(&mut base, &id(&d)), json!(["designation", "quantite", "prix_unitaire", "remise", "tva", "total"]));

    let refus = pieces::modifier_piece_colonnes_sur_base(
        &mut base, Some(v_sans_prix()), id(&d), None, None, None, None, None,
    )
    .unwrap_err();
    assert!(refus.contains("Prix unitaire"), "{refus}");
    assert_eq!(colonnes_de(&mut base, &id(&d)), json!(["designation", "quantite", "prix_unitaire", "remise", "tva", "total"]), "rien n'a changé");
}

fn v_sans_prix() -> Vec<String> {
    vec!["designation".into(), "quantite".into(), "total".into()]
}

#[test]
fn tout_ce_qui_est_touche_passe_le_detecteur() {
    let mut base = base_avec_demo();
    base.auditer(true);
    let client = client_reel(&mut base);
    let sucre = article_unite(&mut base, "Sucre");
    documents::enregistrer_reglage_sur(&mut base, "facture", json!({ "format": "a4", "colonnes": avec(&["tva"]) })).unwrap();
    let p = pieces::creer_piece_colonnes_sur_base(
        &mut base, Some(avec(&["remise"])), client.clone(), "facture".into(), vec![ligne_taxee(&sucre)],
        None, None, None, None, None, None,
    )
    .unwrap();
    pieces::lire_donnees_piece_sur_base(&mut base, p["id"].as_str().unwrap().to_string()).unwrap();
    // La suite : modifier, dupliquer, convertir.
    let d = pieces::creer_piece_colonnes_sur_base(
        &mut base, Some(avec(&["tva"])), client, "devis".into(), vec![ligne_taxee(&sucre)],
        None, None, None, None, None, None,
    )
    .unwrap();
    pieces::modifier_piece_colonnes_sur_base(&mut base, Some(avec(&["remise"])), id(&d), None, None, None, None, None).unwrap();
    pieces::dupliquer_piece_sur_base(&mut base, id(&d)).unwrap();
    let cmd = pieces::convertir_piece_sur_base(&mut base, id(&d), "commande_client".into()).unwrap();
    pieces::convertir_commande_en_livraison_et_facture_sur_base(&mut base, id(&cmd)).unwrap();
}
