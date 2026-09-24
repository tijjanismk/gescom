//! C-4 — voir qui est connecte, et le deconnecter, sur les deux moteurs.
//!
//! Le trou a fermer en premier : un compte desactive gardait ses
//! sessions « ouvertes » en base. Le jeton tombait a l'appel suivant,
//! mais la liste des sessions le montrait encore connecte, et rien ne
//! disait qu'on l'avait coupe. Desactiver ferme maintenant ses sessions
//! dans le meme geste, et le journal le dit.

mod commun;

use commun::*;
use gescom_noyau::base::Base;
use gescom_noyau::{auth, parametres, postes, sessions};

fn utilisateur(base: &mut Base, pseudo: &str) -> String {
    base.lire_une(
        "SELECT utilisateur_id FROM utilisateur_auth WHERE pseudo = ?1",
        &parametres![pseudo],
        |r| r.get::<String>(0),
    )
    .unwrap()
    .unwrap()
}

fn poste(base: &mut Base, nom: &str) -> String {
    postes::inscrire_ou_retrouver_sur(base, nom, &format!("emp-{nom}"), "caisse", None).unwrap().id
}

fn sessions_de(base: &mut Base, utilisateur_id: &str) -> usize {
    sessions::lister_actives_sur(base).unwrap().iter().filter(|s| s.utilisateur_id == utilisateur_id).count()
}

#[test]
fn desactiver_un_compte_ferme_ses_sessions_dans_le_meme_geste() {
    let mut base = base_avec_demo();
    let admin = utilisateur(&mut base, "admin");
    let employe = utilisateur(&mut base, "employe");
    let (p1, p2) = (poste(&mut base, "Caisse 1"), poste(&mut base, "Caisse 2"));
    let (s1, _, _) = sessions::ouvrir_sur(&mut base, &p1, &employe).unwrap();
    let (s2, _, _) = sessions::ouvrir_sur(&mut base, &p2, &employe).unwrap();
    let (s_admin, _, _) = sessions::ouvrir_sur(&mut base, &p1, &admin).unwrap();
    assert_eq!(sessions_de(&mut base, &employe), 2);

    let _g = gescom_noyau::auteur::poser(&admin);
    let r = auth::activer_utilisateur_sur(&mut base, &employe, false).unwrap();
    assert_eq!(r["sessions_fermees"], 2);
    assert_eq!(sessions_de(&mut base, &employe), 0, "la liste ne le montre plus connecté");
    assert!(matches!(sessions::etat_sur(&mut base, &s1), sessions::Etat::Revoquee));
    assert!(matches!(sessions::etat_sur(&mut base, &s2), sessions::Etat::Revoquee));
    // Qui a coupé : la session de l'admin, pas « system ».
    let par: String = base
        .lire_une("SELECT revoque_par FROM session_reseau WHERE id = ?1", &parametres![s1.clone()], |r| r.get::<String>(0))
        .unwrap()
        .unwrap();
    assert_eq!(par, admin);
    // Les autres sessions ne bougent pas.
    assert!(matches!(sessions::etat_sur(&mut base, &s_admin), sessions::Etat::Valide { .. }));
    // Le journal le dit, signé par l'admin.
    let n = compter(
        &mut base,
        "SELECT CAST(COUNT(*) AS BIGINT) FROM journal
         WHERE type_evenement = 'utilisateur_desactive' AND entite_id = ?1 AND auteur_id = ?2",
        &parametres![employe.clone(), admin.clone()],
    );
    assert_eq!(n, 1);
    // Désactivé : il ne se reconnecte plus.
    assert!(auth::connexion_sur(&mut base, "employe".into(), "employe123".into()).is_err());

    // Réactivé : le compte revient, les sessions fermées restent fermées.
    let r = auth::activer_utilisateur_sur(&mut base, &employe, true).unwrap();
    assert_eq!(r["actif"], true);
    assert!(matches!(sessions::etat_sur(&mut base, &s1), sessions::Etat::Revoquee));
    assert!(auth::connexion_sur(&mut base, "employe".into(), "employe123".into()).is_ok());
    // Deux fois le même geste : rien à faire, pas de seconde ligne.
    auth::activer_utilisateur_sur(&mut base, &employe, true).unwrap();
    let n = compter(
        &mut base,
        "SELECT CAST(COUNT(*) AS BIGINT) FROM journal WHERE type_evenement = 'utilisateur_reactive' AND entite_id = ?1",
        &parametres![employe],
    );
    assert_eq!(n, 1);
}

#[test]
fn on_ne_s_enferme_pas_dehors() {
    let mut base = base_avec_demo();
    let admin = utilisateur(&mut base, "admin");
    let employe = utilisateur(&mut base, "employe");

    // Son propre compte.
    {
        let _g = gescom_noyau::auteur::poser(&admin);
        let e = auth::activer_utilisateur_sur(&mut base, &admin, false).unwrap_err();
        assert!(e.contains("propre compte"), "{e}");
    }
    // Le dernier compte à qui son rôle donne tout.
    {
        let _g = gescom_noyau::auteur::poser(&employe);
        let e = auth::activer_utilisateur_sur(&mut base, &admin, false).unwrap_err();
        assert!(e.contains("dernier"), "{e}");
    }
    // Le compte de secours.
    auth::promouvoir_superadmin_sur(&mut base, "employe").unwrap();
    {
        let _g = gescom_noyau::auteur::poser(&admin);
        let e = auth::activer_utilisateur_sur(&mut base, &employe, false).unwrap_err();
        assert!(e.contains("protégé"), "{e}");
    }
    let actifs = compter(&mut base, "SELECT CAST(COUNT(*) AS BIGINT) FROM utilisateur WHERE actif = 0", &[]);
    assert_eq!(actifs, 0, "aucun refus n'a rien écrit");
    assert!(auth::activer_utilisateur_sur(&mut base, "inconnu", false).is_err());
}

#[test]
fn la_session_dit_sa_derniere_commande() {
    let mut base = base_avec_demo();
    let employe = utilisateur(&mut base, "employe");
    let p = poste(&mut base, "Caisse 1");
    let (s, _, _) = sessions::ouvrir_sur(&mut base, &p, &employe).unwrap();
    sessions::toucher_sur(&mut base, &s, Some("creer_vente"));
    let l = sessions::lister_actives_sur(&mut base).unwrap();
    let la = l.iter().find(|x| x.id == s).unwrap();
    assert_eq!(la.derniere_commande.as_deref(), Some("creer_vente"));
    assert!(la.derniere_vue.is_some());
    // Une route sans commande (le canal) ne l'efface pas.
    sessions::toucher_sur(&mut base, &s, None);
    let l = sessions::lister_actives_sur(&mut base).unwrap();
    assert_eq!(l.iter().find(|x| x.id == s).unwrap().derniere_commande.as_deref(), Some("creer_vente"));
}
