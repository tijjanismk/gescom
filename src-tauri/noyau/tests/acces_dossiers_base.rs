//! C-2 — les droits valent par dossier (decision C2), sur les deux
//! moteurs. « Ton frere est patron de sa boutique et n'a rien a faire
//! dans la tienne. Ta comptable voit les deux. »

mod commun;

use commun::*;
use gescom_noyau::acces_dossiers as acces;
use gescom_noyau::base::Base;
use gescom_noyau::dossiers::{self, DOSSIER_DEFAUT};
use gescom_noyau::{auth, parametres, portes, postes, sessions};

fn utilisateur(base: &mut Base, pseudo: &str) -> String {
    base.lire_une(
        "SELECT utilisateur_id FROM utilisateur_auth WHERE pseudo = ?1",
        &parametres![pseudo],
        |r| r.get::<String>(0),
    )
    .unwrap()
    .unwrap()
}

/// La boutique d'origine, la quincaillerie du frere, et trois comptes :
/// le frere (patron), la comptable, un caissier sans ligne.
struct Monde {
    base: Base,
    admin: String,
    quinc: String,
    frere: String,
    comptable: String,
    caissier: String,
}

fn monde() -> Monde {
    let mut base = base_avec_demo();
    let admin = utilisateur(&mut base, "admin");
    let _g = gescom_noyau::auteur::poser(&admin);
    let quinc = dossiers::creer_dossier_sur(&mut base, "QUINC".into(), "Quincaillerie du frère".into(), None, None)
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let mut creer = |pseudo: &str, role: &str| {
        auth::creer_utilisateur_sur_base(
            &mut base,
            pseudo.into(),
            pseudo.into(),
            None,
            "secret-123".into(),
            role.into(),
            admin.clone(),
        )
        .unwrap()
    };
    let frere = creer("frere", "patron");
    let comptable = creer("awa", "comptable");
    let caissier = creer("moussa", "caissier");
    Monde { base, admin, quinc, frere, comptable, caissier }
}

#[test]
fn le_frere_est_patron_chez_lui_et_n_entre_pas_chez_toi() {
    let mut m = monde();
    let _g = gescom_noyau::auteur::poser(&m.admin);

    // Avant toute ligne : comme avant la v3.
    assert_eq!(acces::role_dans_sur(&mut m.base, &m.frere, &m.quinc).unwrap().as_deref(), Some("patron"),
        "patron global : il voit tout, comme l'admin");
    assert_eq!(acces::role_dans_sur(&mut m.base, &m.caissier, DOSSIER_DEFAUT).unwrap().as_deref(), Some("caissier"));
    assert_eq!(acces::role_dans_sur(&mut m.base, &m.caissier, &m.quinc).unwrap(), None,
        "un caissier sans ligne : le dossier d'origine seulement");

    acces::definir_sur(&mut m.base, &m.frere, Some(vec![(m.quinc.clone(), "patron".into())])).unwrap();
    acces::definir_sur(
        &mut m.base,
        &m.comptable,
        Some(vec![(DOSSIER_DEFAUT.into(), "comptable".into()), (m.quinc.clone(), "comptable".into())]),
    )
    .unwrap();

    assert_eq!(acces::role_dans_sur(&mut m.base, &m.frere, &m.quinc).unwrap().as_deref(), Some("patron"));
    assert_eq!(acces::role_dans_sur(&mut m.base, &m.frere, DOSSIER_DEFAUT).unwrap(), None, "rien à faire chez toi");
    // Patron chez lui — mais pas de ce qui est commun à tous les dossiers :
    // sinon il se créerait un compte qui voit ta boutique.
    let perms = portes::permissions_de_sur(&mut m.base, &m.frere, "patron");
    assert!(perms.contains("ventes:creer") && perms.contains("dossiers:gerer"));
    for p in portes::PERMISSIONS_DE_TOUTE_LA_BASE {
        assert!(!perms.contains(*p), "{p} retirée au compte restreint");
    }
    assert!(portes::permissions_de_sur(&mut m.base, &m.admin, "patron").contains("utilisateurs:gerer"));
    let tous = vec![DOSSIER_DEFAUT.to_string(), m.quinc.clone()];
    assert_eq!(acces::dossiers_ouverts_a_sur(&mut m.base, &m.frere, &tous).unwrap(), vec![m.quinc.clone()]);
    assert_eq!(acces::dossiers_ouverts_a_sur(&mut m.base, &m.comptable, &tous).unwrap().len(), 2, "la comptable voit les deux");
    assert_eq!(acces::dossiers_ouverts_a_sur(&mut m.base, &m.admin, &tous).unwrap().len(), 2, "l'admin, sans ligne, voit tout");

    // Les permissions suivent le rôle DANS le dossier.
    let perms = portes::permissions_de_sur(&mut m.base, &m.comptable, "comptable");
    assert!(perms.contains("journal:lire") && !perms.contains("caisse:lire_autres"));

    // L'écran dit ce qui est posé.
    let lu = acces::lire_sur(&mut m.base, &m.frere).unwrap();
    assert_eq!(lu["par_dossier"], true);
    let origine = lu["dossiers"].as_array().unwrap().iter().find(|d| d["id"] == DOSSIER_DEFAUT).unwrap();
    assert!(origine["role"].is_null());

    // Retour à la règle d'avant.
    acces::definir_sur(&mut m.base, &m.frere, None).unwrap();
    assert_eq!(acces::role_dans_sur(&mut m.base, &m.frere, DOSSIER_DEFAUT).unwrap().as_deref(), Some("patron"));
    let n = compter(
        &mut m.base,
        "SELECT CAST(COUNT(*) AS BIGINT) FROM journal WHERE type_evenement = 'droits_dossiers_modifies' AND dossier_id = ?1",
        &parametres![DOSSIER_DEFAUT],
    );
    assert_eq!(n, 3, "chaque réglage au journal");
}

#[test]
fn un_dossier_retire_ferme_la_session_qui_y_travaillait() {
    let mut m = monde();
    let _g = gescom_noyau::auteur::poser(&m.admin);
    acces::definir_sur(
        &mut m.base,
        &m.comptable,
        Some(vec![(DOSSIER_DEFAUT.into(), "comptable".into()), (m.quinc.clone(), "caissier".into())]),
    )
    .unwrap();
    let poste = postes::inscrire_ou_retrouver_sur(&mut m.base, "Bureau", "emp-bureau", "caisse", None).unwrap().id;
    let (s, ..) = sessions::ouvrir_dans_dossier_sur(&mut m.base, &poste, &m.comptable, Some(&m.quinc)).unwrap();
    match sessions::etat_sur(&mut m.base, &s) {
        sessions::Etat::Valide { role, .. } => assert_eq!(role, "caissier", "le rôle de CE dossier, pas le global"),
        _ => panic!("session valide attendue"),
    }
    acces::definir_sur(&mut m.base, &m.comptable, Some(vec![(DOSSIER_DEFAUT.into(), "comptable".into())])).unwrap();
    assert!(
        matches!(sessions::etat_sur(&mut m.base, &s), sessions::Etat::Revoquee),
        "le dossier retiré : la session tombe à la requête suivante"
    );
}

#[test]
fn on_ne_s_enferme_pas_dehors() {
    let mut m = monde();
    // Ses propres dossiers.
    {
        let _g = gescom_noyau::auteur::poser(&m.admin);
        let e = acces::definir_sur(&mut m.base, &m.admin, Some(vec![(m.quinc.clone(), "patron".into())])).unwrap_err();
        assert!(e.contains("propres"), "{e}");
        // Aucun dossier : c'est une désactivation, pas un réglage.
        let e = acces::definir_sur(&mut m.base, &m.caissier, Some(vec![])).unwrap_err();
        assert!(e.contains("désactiver"), "{e}");
        let e = acces::definir_sur(&mut m.base, &m.caissier, Some(vec![("inconnu".into(), "caissier".into())])).unwrap_err();
        assert!(e.contains("introuvable"), "{e}");
        let e = acces::definir_sur(&mut m.base, &m.caissier, Some(vec![(m.quinc.clone(), "superadmin".into())])).unwrap_err();
        assert!(e.contains("superadmin"), "{e}");
        let e = acces::definir_sur(&mut m.base, &m.caissier, Some(vec![(m.quinc.clone(), "roi".into())])).unwrap_err();
        assert!(e.contains("introuvable"), "{e}");
        // Le frère restreint : l'admin voit encore tout.
        acces::definir_sur(&mut m.base, &m.frere, Some(vec![(m.quinc.clone(), "patron".into())])).unwrap();
    }
    // Le dernier compte qui voit tous les dossiers ne se restreint pas,
    // même appelé par un autre compte (le serveur refuserait déjà au
    // frère `utilisateurs:gerer` ; le noyau ne compte pas dessus).
    {
        let _g = gescom_noyau::auteur::poser(&m.frere);
        let e = acces::definir_sur(&mut m.base, &m.admin, Some(vec![(DOSSIER_DEFAUT.into(), "patron".into())])).unwrap_err();
        assert!(e.contains("dernier compte"), "{e}");
    }
    // Le compte de secours ne se restreint pas.
    auth::promouvoir_superadmin_sur(&mut m.base, "moussa").unwrap();
    {
        let _g = gescom_noyau::auteur::poser(&m.admin);
        let e = acces::definir_sur(&mut m.base, &m.caissier, Some(vec![(m.quinc.clone(), "caissier".into())])).unwrap_err();
        assert!(e.contains("protégé"), "{e}");
    }
    let lignes = compter(
        &mut m.base,
        "SELECT CAST(COUNT(*) AS BIGINT) FROM utilisateur_dossier WHERE utilisateur_id <> ?1",
        &parametres![m.frere.clone()],
    );
    assert_eq!(lignes, 0, "aucun refus n'a rien écrit");
}

#[test]
fn qui_n_a_que_certains_dossiers_entre_dans_celui_qu_il_cree() {
    let mut m = monde();
    {
        let _g = gescom_noyau::auteur::poser(&m.admin);
        acces::definir_sur(&mut m.base, &m.frere, Some(vec![(m.quinc.clone(), "patron".into())])).unwrap();
    }
    let _g = gescom_noyau::auteur::poser(&m.frere);
    m.base.choisir_dossier(&m.quinc).unwrap();
    let neuf = dossiers::creer_dossier_sur(&mut m.base, "QUINC2".into(), "Seconde boutique".into(), None, None).unwrap();
    let neuf = neuf["id"].as_str().unwrap();
    assert_eq!(acces::role_dans_sur(&mut m.base, &m.frere, neuf).unwrap().as_deref(), Some("patron"));
    assert_eq!(acces::role_dans_sur(&mut m.base, &m.frere, DOSSIER_DEFAUT).unwrap(), None);
    assert_eq!(acces::role_dans_sur(&mut m.base, &m.admin, neuf).unwrap().as_deref(), Some("patron"), "l'admin le voit aussi");
}
