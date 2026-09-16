//! L'entretien de la base, sur `Base` — le travail du serveur (D9).
//!
//! D9 : l'entretien a quitte l'ecran des caisses pour la console du
//! serveur. Ces scenarios verifient le geste complet — reimputation
//! des reglements globaux, copie de securite avant, journal — sur
//! SQLite comme sur PostgreSQL. SQLite est teste SUR FICHIER : la
//! memoire refuse, la copie avant y serait une fiction.

use std::path::{Path, PathBuf};

use gescom_noyau::amorcage;
use gescom_noyau::base::Base;
use gescom_noyau::parametres;
use gescom_noyau::{achats, argent};

mod commun;
use commun::*;

/// Un dossier de copies jetable, nettoye a la fin du scenario.
struct DossierEssai(PathBuf);

impl DossierEssai {
    fn nouveau() -> Self {
        let dossier = std::env::temp_dir()
            .join(format!("gescom-entretien-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dossier).expect("dossier d'essai");
        DossierEssai(dossier)
    }

    fn chemin(&self) -> &Path {
        &self.0
    }
}

impl Drop for DossierEssai {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Une base prete pour l'entretien : sur fichier quand le moteur est
/// SQLite (la memoire est refusee), PostgreSQL jetable sinon — la meme
/// base amorcee et garnie de demo que `base_avec_demo`. Le fichier
/// SQLite et ses voisins WAL disparaissent avec la structure.
struct BaseEntretien {
    base: Base,
    fichiers: Vec<PathBuf>,
}

impl BaseEntretien {
    fn nouvelle() -> Self {
        if std::env::var("GESCOM_PG").is_ok() {
            return BaseEntretien { base: base_avec_demo(), fichiers: vec![] };
        }
        let fichier = std::env::temp_dir()
            .join(format!("gescom-entretien-{}.db", uuid::Uuid::new_v4()));
        let mut base = Base::ouvrir(&fichier.to_string_lossy()).expect("base fichier");
        amorcage::amorcer(&mut base).expect("amorçage");
        amorcage::donnees_demo(&mut base).expect("démo");
        BaseEntretien { base, fichiers: vec![fichier] }
    }
}

impl Drop for BaseEntretien {
    fn drop(&mut self) {
        for f in &self.fichiers {
            let nom = f.to_string_lossy();
            let _ = std::fs::remove_file(f);
            let _ = std::fs::remove_file(format!("{nom}-wal"));
            let _ = std::fs::remove_file(format!("{nom}-shm"));
        }
    }
}

/// Un achat à crédit de `quantite` kg de sucre à 500 F : une FAF de
/// `quantite × 500` due. Rend l'id de la pièce.
fn acheter_a_credit(base: &mut Base, f: &str, quantite: f64) -> String {
    let sucre = article_unite(base, "Sucre");
    let r = achats::enregistrer_achat_sur_base(
        base, Some(f.to_string()), None,
        vec![achats::LigneAchat { article_id: sucre.0.clone(), unite_vente_id: sucre.1.clone(), quantite, facteur: 1.0, prix_achat: 500 }],
        Some("credit".into()), None, None, None, None, None,
    )
    .expect("achat");
    r["piece_id"].as_str().unwrap().to_string()
}

/// L'etat « d'avant la repartition ecrite » : le reglement global du
/// fournisseur n'est plus rattache a aucune facture.
fn oublier_l_affectation(base: &mut Base, f: &str) {
    let dossier = base.dossier().to_string();
    base.executer(
        "UPDATE paiement_fournisseur SET piece_id = NULL
         WHERE fournisseur_id = ?1 AND dossier_id = ?2",
        &parametres![f, dossier],
    )
    .expect("fabriquer l'etat herite");
}

/// La somme des paiements du fournisseur, quelle que soit leur
/// affectation.
fn total_paye(base: &mut Base, f: &str) -> i64 {
    compter(
        base,
        "SELECT CAST(COALESCE(SUM(montant),0) AS BIGINT) FROM paiement_fournisseur
         WHERE fournisseur_id = ?1",
        &parametres![f],
    )
}

#[test]
fn l_entretien_reaffecte_les_reglements_globaux_sans_changer_les_montants() {
    let mut e = BaseEntretien::nouvelle();
    let dossier = DossierEssai::nouveau();
    let f = fournisseur(&mut e.base, "Grossiste");
    let _ = acheter_a_credit(&mut e.base, &f, 2.0); // 1 000
    let _ = acheter_a_credit(&mut e.base, &f, 5.0); // 2 500
    let _ = acheter_a_credit(&mut e.base, &f, 3.0); // 1 500
    ouvrir_caisse(&mut e.base);
    argent::regler_dette_fournisseur_sur_base(&mut e.base, f.clone(), 5_000, "especes".into(), None, None)
        .expect("reglement global");
    oublier_l_affectation(&mut e.base, &f);
    let avant = total_paye(&mut e.base, &f);

    let r = parametres::entretenir_base_sur_base(&mut e.base, dossier.chemin())
        .expect("entretien");

    assert!(r["reimputes"].as_i64().unwrap() >= 1, "le global doit etre reaffecte : {r}");
    assert_eq!(total_paye(&mut e.base, &f), avant, "aucun franc cree ni perdu");
    assert_eq!(
        compter(&mut e.base,
            "SELECT COUNT(*) FROM paiement_fournisseur
             WHERE piece_id IS NULL AND fournisseur_id = ?1",
            &parametres![f]),
        0,
        "plus aucune ligne globale"
    );
    assert_eq!(
        compter(&mut e.base,
            "SELECT COUNT(*) FROM journal
             WHERE type_evenement = 'entretien' AND origine = 'serveur'",
            &[]),
        1,
        "l'entretien laisse sa trace"
    );

    // La copie de securite existe, sous le nom du moteur.
    let copies: Vec<PathBuf> = std::fs::read_dir(dossier.chemin())
        .expect("dossier des copies")
        .filter_map(|en| en.ok().map(|en| en.path()))
        .collect();
    assert!(!copies.is_empty(), "une copie avant doit exister");
    if e.base.sqlite().is_some() {
        assert!(copies.iter().any(|p| p
            .file_name().unwrap().to_string_lossy().starts_with("gescom_avant_entretien_")),
            "copie SQLite attendue parmi : {copies:?}");
    } else {
        assert!(copies.iter().any(|p| p
            .file_name().unwrap().to_string_lossy().ends_with(".dump")),
            "dump attendu parmi : {copies:?}");
    }
}

#[test]
fn un_second_entretien_ne_touche_plus_a_rien() {
    let mut e = BaseEntretien::nouvelle();
    let dossier = DossierEssai::nouveau();
    let f = fournisseur(&mut e.base, "Grossiste");
    let _ = acheter_a_credit(&mut e.base, &f, 2.0);
    ouvrir_caisse(&mut e.base);
    argent::regler_dette_fournisseur_sur_base(&mut e.base, f.clone(), 1_000, "especes".into(), None, None)
        .expect("reglement global");
    oublier_l_affectation(&mut e.base, &f);

    let r1 = parametres::entretenir_base_sur_base(&mut e.base, dossier.chemin())
        .expect("premier entretien");
    assert!(r1["reimputes"].as_i64().unwrap() >= 1);
    let r2 = parametres::entretenir_base_sur_base(&mut e.base, dossier.chemin())
        .expect("second entretien");
    assert_eq!(r2["reimputes"].as_i64().unwrap(), 0, "idempotent : {r2}");
    assert_eq!(
        compter(&mut e.base,
            "SELECT COUNT(*) FROM journal WHERE type_evenement = 'entretien'",
            &[]),
        2,
        "chaque entretien s'ecrit"
    );
}

#[test]
fn une_base_saine_s_entretient_sans_rien_a_reaffecter() {
    let mut e = BaseEntretien::nouvelle();
    let dossier = DossierEssai::nouveau();

    let r = parametres::entretenir_base_sur_base(&mut e.base, dossier.chemin())
        .expect("entretien");

    assert_eq!(r["reimputes"].as_i64().unwrap(), 0);
    assert!(r["gagne"].as_i64().unwrap() >= 0, "gain jamais negatif : {r}");
    assert_eq!(
        compter(&mut e.base,
            "SELECT COUNT(*) FROM journal WHERE type_evenement = 'entretien'",
            &[]),
        1
    );
}

#[test]
fn en_memoire_l_entretien_refuse_plutot_que_faire_semblant() {
    if std::env::var("GESCOM_PG").is_ok() {
        return; // la memoire est une affaire SQLite
    }
    let mut base = base_avec_demo();
    let dossier = DossierEssai::nouveau();

    let refus = parametres::entretenir_base_sur_base(&mut base, dossier.chemin())
        .unwrap_err();
    assert!(refus.contains("mémoire"), "message inattendu : {refus}");
}

/// R2 : la copie de securite est prise AVANT la reimputation.
///
/// La reimputation deplace de l'argent d'une facture a l'autre. Une
/// copie prise apres elle contiendrait deja le deplacement, et ne
/// permettrait pas d'y revenir — le filet ne couvrirait que le VACUUM.
/// SQLite seulement : un dump PostgreSQL ne se relit pas comme une base.
#[test]
fn la_copie_de_securite_precede_la_reimputation() {
    if std::env::var("GESCOM_PG").is_ok() {
        return;
    }
    let mut e = BaseEntretien::nouvelle();
    let dossier = DossierEssai::nouveau();
    let f = fournisseur(&mut e.base, "Grossiste");
    let _ = acheter_a_credit(&mut e.base, &f, 2.0);
    ouvrir_caisse(&mut e.base);
    argent::regler_dette_fournisseur_sur_base(&mut e.base, f.clone(), 1_000, "especes".into(), None, None)
        .expect("reglement global");
    oublier_l_affectation(&mut e.base, &f);

    let r = parametres::entretenir_base_sur_base(&mut e.base, dossier.chemin())
        .expect("entretien");
    assert!(r["reimputes"].as_i64().unwrap() >= 1, "il y avait bien à réaffecter : {r}");

    let globales = "SELECT COUNT(*) FROM paiement_fournisseur
                    WHERE piece_id IS NULL AND fournisseur_id = ?1";
    assert_eq!(
        compter(&mut e.base, globales, &parametres![f.clone()]),
        0,
        "la base vivante est réaffectée"
    );

    // Et la copie, elle, montre l'etat d'AVANT : c'est tout son interet.
    let chemin = r["copie"].as_str().expect("le chemin de la copie");
    let mut copie = Base::ouvrir(chemin).expect("relire la copie");
    assert!(
        compter(&mut copie, globales, &parametres![f]) >= 1,
        "la copie doit précéder la réimputation, pas la suivre"
    );
}

#[test]
fn tout_ce_qui_est_porte_ici_passe_le_detecteur() {
    let mut e = BaseEntretien::nouvelle();
    let dossier = DossierEssai::nouveau();
    let f = fournisseur(&mut e.base, "Grossiste");
    let _ = acheter_a_credit(&mut e.base, &f, 2.0);
    ouvrir_caisse(&mut e.base);
    argent::regler_dette_fournisseur_sur_base(&mut e.base, f.clone(), 1_000, "especes".into(), None, None)
        .expect("reglement global");
    oublier_l_affectation(&mut e.base, &f);
    e.base.auditer(true);

    parametres::entretenir_base_sur_base(&mut e.base, dossier.chemin())
        .expect("entretien sous le detecteur");
    parametres::entretenir_base_sur_base(&mut e.base, dossier.chemin())
        .expect("second entretien sous le detecteur");
}
