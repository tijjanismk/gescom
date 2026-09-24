//! E-3 — les journaux comptables, fabriques a la lecture (D23), sur les
//! deux moteurs. Une journee de boutique : ventes comptant (dont une
//! avec TVA), vente a credit reglee en Orange Money, achat, paiement
//! fournisseur, depense, creance irrecouvrable, cloture avec un
//! manquant. Puis : chaque journal equilibre, les ventes du jour egales
//! au chiffre du cahier, chaque operation sur le compte affecte, et
//! l'export CSV.

mod commun;

use commun::*;
use gescom_noyau::argent::{self, ParamsLigneInput};
use gescom_noyau::base::Base;
use gescom_noyau::coeur::journaux::Ecriture;
use gescom_noyau::{achats, affectations, caisse, chantiers, creances, journal, journaux_comptables, plan_comptable};

fn aujourd_hui() -> String {
    gescom_noyau::utils::maintenant_iso()[..10].to_string()
}

fn vendre(base: &mut Base, client: String, mode: &str, quantite: f64, tva: Option<f64>) -> (String, i64) {
    let depot = depot_defaut(base);
    let sucre = article_unite(base, "Sucre");
    let (paye, moyen) = if mode == "comptant" {
        (Some((sucre.3 as f64 * quantite).round() as i64), Some("especes".to_string()))
    } else {
        (None, None)
    };
    let v = argent::creer_vente_sur_base(
        base, client, depot.clone(), mode.into(),
        vec![ParamsLigneInput {
            article_id: sucre.0.clone(), unite_vente_id: sucre.1.clone(), depot_source_id: depot,
            source_approvisionnement: "stock".into(), quantite, facteur: sucre.2,
            prix_reference: sucre.3, prix_pratique: sucre.3, taux_tva: tva, a_decouvert: None,
        }],
        None, paye, moyen, None,
    )
    .expect("vente");
    (v["vente_id"].as_str().unwrap().to_string(), (sucre.3 as f64 * quantite).round() as i64)
}

/// La journee, et ce qu'elle doit donner.
struct Journee {
    base: Base,
    ca: i64,
    credit_reste: i64,
    achat: i64,
}

fn journee() -> Journee {
    let mut base = base_avec_demo();
    let sid = ouvrir_caisse(&mut base);
    let comptant = client_generique(&mut base);
    let awa = client_reel(&mut base);
    let (_, v1) = vendre(&mut base, comptant.clone(), "comptant", 2.0, None);
    let (_, v2) = vendre(&mut base, comptant, "comptant", 1.0, Some(18.0));
    let (credit, v3) = vendre(&mut base, awa.clone(), "credit", 3.0, None);
    creances::regler_creance_sur_base(&mut base, credit.clone(), 1000, "orange_money".into(), None).unwrap();
    let (perdue, v4) = vendre(&mut base, awa, "credit", 1.0, None);
    chantiers::marquer_irrecouvrable_sur_base(&mut base, perdue, "Parti sans laisser d'adresse".into()).unwrap();

    let f = fournisseur(&mut base, "Grossiste Diallo");
    let sucre = article_unite(&mut base, "Sucre");
    achats::enregistrer_achat_sur_base(
        &mut base, Some(f.clone()), None,
        vec![achats::LigneAchat { article_id: sucre.0, unite_vente_id: sucre.1, quantite: 10.0, facteur: 1.0, prix_achat: 450 }],
        Some("credit".into()), None, None, None, None, None,
    )
    .expect("achat");
    argent::regler_dette_fournisseur_sur_base(&mut base, f, 2000, "especes".into(), None, None).unwrap();
    caisse::enregistrer_depense_sur_base(&mut base, 1500, "Loyer de septembre".into(), Some("loyer".into()), None, None).unwrap();
    caisse::enregistrer_depense_sur_base(&mut base, 300, "Taxi".into(), None, None, None).unwrap();

    // Clôture avec 200 F de moins que le théorique.
    let theorique = caisse::lire_sessions_caisse_sur_base(&mut base, None).unwrap()[0]["solde_theorique"]
        .as_i64()
        .unwrap_or(0);
    let theorique = if theorique != 0 { theorique } else { v1 + v2 - 2000 - 1500 - 300 };
    caisse::fermer_session_caisse_sur(&mut base, sid, theorique - 200).unwrap();
    Journee { base, ca: v1 + v2 + v3 + v4, credit_reste: v4, achat: 4500 }
}

fn du_jour(base: &mut Base, j: &str) -> Vec<Ecriture> {
    let d = aujourd_hui();
    journaux_comptables::ecritures_sur(base, &d, &d, Some(j)).unwrap()
}

fn somme(e: &[Ecriture], compte: &str) -> (i64, i64) {
    e.iter().flat_map(|x| x.lignes.iter()).filter(|l| l.compte == compte).fold((0, 0), |(d, c), l| (d + l.debit, c + l.credit))
}

#[test]
fn chaque_journal_est_equilibre_et_les_ventes_font_le_chiffre_du_cahier() {
    let mut j = journee();
    let d = aujourd_hui();
    let tout = journaux_comptables::ecritures_sur(&mut j.base, &d, &d, None).unwrap();
    assert!(!tout.is_empty());
    for e in &tout {
        let (db, cr) = e.lignes.iter().fold((0, 0), |(a, b), l| (a + l.debit, b + l.credit));
        assert_eq!(db, cr, "{} {} déséquilibrée", e.journal, e.piece);
        assert!(e.lignes.iter().all(|l| l.debit >= 0 && l.credit >= 0), "pas de montant négatif");
    }
    for code in ["VT", "AC", "RG", "CA"] {
        let e = du_jour(&mut j.base, code);
        assert!(!e.is_empty(), "{code} vide");
        let (db, cr) = gescom_noyau::coeur::journaux::totaux(&e);
        assert_eq!(db, cr, "journal {code}");
    }

    // Les ventes du jour = le chiffre d'affaires du cahier du jour.
    let cahier = journal::lire_journal_du_jour_sur_base(&mut j.base, Some(d.clone()), None).unwrap();
    let ca_cahier: i64 = cahier["ventes"].as_array().unwrap().iter().filter_map(|v| v["montant_ttc"].as_i64()).sum();
    let ht_cahier: i64 = cahier["ventes"].as_array().unwrap().iter().filter_map(|v| v["montant_ht"].as_i64()).sum();
    assert_eq!(ca_cahier, j.ca);
    let vt = du_jour(&mut j.base, "VT");
    let ventes: Vec<Ecriture> = vt.iter().filter(|e| e.libelle.starts_with("Vente")).cloned().collect();
    let (_, credit_701) = somme(&ventes, "701");
    let (_, credit_tva) = somme(&ventes, "4431");
    assert_eq!(credit_701 + credit_tva, ca_cahier, "ventes + TVA = CA TTC du cahier");
    assert_eq!(credit_701, ht_cahier, "ventes = CA HT du cahier");
    assert!(credit_tva > 0, "la vente à 18 % porte sa TVA");
    let (debit_411, _) = somme(&ventes, "411");
    assert_eq!(debit_411, ca_cahier);
}

#[test]
fn chaque_operation_va_sur_le_compte_affecte() {
    let mut j = journee();
    // Règlements : espèces des ventes comptant, Orange Money, fournisseur.
    let rg = du_jour(&mut j.base, "RG");
    let (om, _) = somme(&rg, "552");
    assert_eq!(om, 1000, "Orange Money sur 552");
    let (d401, _) = somme(&rg, "401");
    assert_eq!(d401, 2000, "le fournisseur payé au débit de 401");
    // Achats : 10 × 450 au débit de 601, au crédit de 401.
    let ac = du_jour(&mut j.base, "AC");
    assert_eq!(somme(&ac, "601"), (j.achat, 0));
    assert_eq!(somme(&ac, "401"), (0, j.achat));
    // Caisse : loyer 622, taxi sans catégorie en 658, manquant de 200 en 658.
    let ca = du_jour(&mut j.base, "CA");
    assert_eq!(somme(&ca, "622").0, 1500);
    assert_eq!(somme(&ca, "658").0, 300 + 200, "dépense « autre » et manquant");
    // Irrécouvrable : ce qui restait dû, au débit de 6511.
    let vt = du_jour(&mut j.base, "VT");
    assert_eq!(somme(&vt, "6511").0, j.credit_reste);

    // Le comptable met les espèces sur la caisse de la boutique : les
    // journaux suivent, sans rien réécrire.
    plan_comptable::ajouter_sous_compte_sur(&mut j.base, "5711".into(), "Caisse boutique".into()).unwrap();
    affectations::definir_sur(&mut j.base, "tresorerie:especes".into(), Some("5711".into())).unwrap();
    let rg = du_jour(&mut j.base, "RG");
    assert_eq!(somme(&rg, "571"), (0, 0));
    assert!(somme(&rg, "5711").0 > 0);
}

#[test]
fn l_export_csv_rend_chaque_ligne_et_dit_ce_qu_il_contient() {
    let mut j = journee();
    let d = aujourd_hui();
    let r = journaux_comptables::csv_sur(&mut j.base, d.clone(), d.clone(), Some("VT".into())).unwrap();
    assert_eq!(r["nom_fichier"], format!("journaux_VT_{d}_{d}.csv"));
    let contenu = r["contenu"].as_str().unwrap();
    let lignes: Vec<&str> = contenu.trim_end().split("\r\n").collect();
    assert_eq!(lignes[0], "Date;Journal;Compte;Libellé du compte;Libellé;Débit;Crédit;Pièce");
    assert_eq!(lignes.len() - 1, r["lignes"].as_u64().unwrap() as usize);
    assert!(lignes[1..].iter().all(|l| l.split(';').nth(1) == Some("VT")));
    assert!(contenu.contains(";411;Clients;"));
    // Débits et crédits du fichier s'équilibrent aussi.
    let (mut db, mut cr) = (0i64, 0i64);
    for l in &lignes[1..] {
        let c: Vec<&str> = l.split(';').collect();
        db += c[5].parse::<i64>().unwrap_or(0);
        cr += c[6].parse::<i64>().unwrap_or(0);
    }
    assert_eq!(db, cr);

    let lu = journaux_comptables::lire_sur(&mut j.base, d.clone(), d.clone(), None).unwrap();
    assert_eq!(lu["journaux"].as_array().unwrap().len(), 4);
    assert!(journaux_comptables::ecritures_sur(&mut j.base, &d, "2020-01-01", None).unwrap_err().contains("précède"));
    assert!(journaux_comptables::ecritures_sur(&mut j.base, &d, &d, Some("XX")).unwrap_err().contains("inconnu"));
}

#[test]
fn tout_ce_qui_est_porte_ici_passe_le_detecteur() {
    let mut j = journee();
    j.base.auditer(true);
    let d = aujourd_hui();
    journaux_comptables::lire_sur(&mut j.base, d.clone(), d.clone(), None).unwrap();
    journaux_comptables::csv_sur(&mut j.base, d.clone(), d, None).unwrap();
}
