//! L'authentification, sur `Base` — sessions, postes, permissions.
//!
//! Ce que `api.rs::connexion` fait pas à pas sur le serveur, en plus
//! court : identifier le compte, vérifier le mot de passe, inscrire le
//! poste, ouvrir la session, lire les permissions. Aucune de ces
//! étapes n'avait de scénario qui les joue ENSEMBLE avant ce fichier —
//! chacune n'était testée qu'isolément, ou seulement sur PostgreSQL
//! (`postgres_amorcage.rs`, qui ne tourne pas ici sans `GESCOM_PG`).

use gescom_noyau::base::Base;
use gescom_noyau::parametres;
use gescom_noyau::portes::ContexteUtilisateur;
use gescom_noyau::{amorcage, portes, postes, sessions};

fn base_amorcee() -> Base {
    let mut base = Base::ouvrir(":memory:").expect("base en mémoire");
    amorcage::amorcer(&mut base).expect("amorçage");
    base
}

/// (utilisateur_id, hash, role) du compte `admin` posé par l'amorçage.
fn compte_admin(base: &mut Base) -> (String, String, String) {
    base.lire_une(
        "SELECT ua.utilisateur_id, ua.mot_de_passe, r.nom
         FROM utilisateur_auth ua
         JOIN utilisateur u ON u.id = ua.utilisateur_id
         JOIN role r        ON r.id = u.role_id
         WHERE ua.pseudo = 'admin'",
        &[],
        |r| Ok((r.get::<String>(0)?, r.get::<String>(1)?, r.get::<String>(2)?)),
    )
    .unwrap()
    .expect("le compte admin existe depuis l'amorçage")
}

// =====================================================================
//  postes::inscrire_ou_retrouver_sur
// =====================================================================

#[test]
fn un_poste_neuf_s_inscrit_actif() {
    let mut base = base_amorcee();
    let poste = postes::inscrire_ou_retrouver_sur(&mut base, "Caisse 1", "emp-1", "caisse", None)
        .expect("inscription");
    assert_eq!(poste.nom, "Caisse 1");
    assert_eq!(poste.genre, "caisse");
    assert!(poste.actif);
}

#[test]
fn la_meme_empreinte_retrouve_le_meme_poste_sans_le_dupliquer() {
    let mut base = base_amorcee();
    let premier =
        postes::inscrire_ou_retrouver_sur(&mut base, "Caisse 1", "emp-1", "caisse", None).unwrap();
    // Le poste renomme, ou l'IP a change : meme empreinte, meme ligne.
    let second = postes::inscrire_ou_retrouver_sur(
        &mut base,
        "Caisse du fond",
        "emp-1",
        "caisse",
        Some("192.168.1.50"),
    )
    .unwrap();
    assert_eq!(premier.id, second.id, "même empreinte = même poste");
    assert_eq!(second.nom, "Caisse du fond", "le nom se met à jour");

    let total = base
        .lire_une("SELECT COUNT(*) FROM poste WHERE empreinte = 'emp-1'", &[], |r| {
            r.get::<i64>(0)
        })
        .unwrap()
        .unwrap();
    assert_eq!(total, 1, "pas de doublon");
}

// =====================================================================
//  sessions::ouvrir_sur / etat_sur
// =====================================================================

#[test]
fn une_session_ouverte_est_valide() {
    let mut base = base_amorcee();
    let (utilisateur_id, ..) = compte_admin(&mut base);
    let poste =
        postes::inscrire_ou_retrouver_sur(&mut base, "Caisse 1", "emp-1", "caisse", None).unwrap();

    let (session_id, jeton, _expire) =
        sessions::ouvrir_sur(&mut base, &poste.id, &utilisateur_id).unwrap();
    assert_eq!(jeton.len(), 64, "deux UUID v4 simples concaténés");

    match sessions::etat_sur(&mut base, &session_id) {
        sessions::Etat::Valide { utilisateur_id: uid, poste_id, .. } => {
            assert_eq!(uid, utilisateur_id);
            assert_eq!(poste_id, poste.id);
        }
        _ => panic!("une session tout juste ouverte doit être valide"),
    }
}

#[test]
fn une_session_inconnue_le_dit() {
    let mut base = base_amorcee();
    assert!(matches!(
        sessions::etat_sur(&mut base, "session-qui-n-existe-pas"),
        sessions::Etat::Inconnue
    ));
}

#[test]
fn revoquer_une_session_la_rend_invalide() {
    let mut base = base_amorcee();
    let (utilisateur_id, ..) = compte_admin(&mut base);
    let poste =
        postes::inscrire_ou_retrouver_sur(&mut base, "Caisse 1", "emp-1", "caisse", None).unwrap();
    let (session_id, ..) = sessions::ouvrir_sur(&mut base, &poste.id, &utilisateur_id).unwrap();

    let touchees = sessions::revoquer_sur(&mut base, &session_id, "test").unwrap();
    assert_eq!(touchees, 1);
    assert!(matches!(
        sessions::etat_sur(&mut base, &session_id),
        sessions::Etat::Revoquee
    ));
}

#[test]
fn desactiver_le_poste_ferme_la_session() {
    let mut base = base_amorcee();
    let (utilisateur_id, ..) = compte_admin(&mut base);
    let poste =
        postes::inscrire_ou_retrouver_sur(&mut base, "Caisse 1", "emp-1", "caisse", None).unwrap();
    let (session_id, ..) = sessions::ouvrir_sur(&mut base, &poste.id, &utilisateur_id).unwrap();

    base.executer(
        "UPDATE poste SET actif = 0 WHERE id = ?1",
        &parametres![poste.id],
    )
    .unwrap();

    assert!(matches!(
        sessions::etat_sur(&mut base, &session_id),
        sessions::Etat::PosteFerme
    ));
}

#[test]
fn revoquer_toutes_les_sessions_ferme_tout() {
    let mut base = base_amorcee();
    let (utilisateur_id, ..) = compte_admin(&mut base);
    let poste =
        postes::inscrire_ou_retrouver_sur(&mut base, "Caisse 1", "emp-1", "caisse", None).unwrap();
    let (s1, ..) = sessions::ouvrir_sur(&mut base, &poste.id, &utilisateur_id).unwrap();
    let (s2, ..) = sessions::ouvrir_sur(&mut base, &poste.id, &utilisateur_id).unwrap();

    let n = sessions::revoquer_toutes_sur(&mut base, "redemarrage_serveur").unwrap();
    assert_eq!(n, 2);
    assert!(matches!(sessions::etat_sur(&mut base, &s1), sessions::Etat::Revoquee));
    assert!(matches!(sessions::etat_sur(&mut base, &s2), sessions::Etat::Revoquee));
}

// =====================================================================
//  portes::permissions_de_sur / verifier_permission_sur
// =====================================================================

/// Un patron dont `acces_total` a ete pose a 0 — par une version
/// anterieure du code, ou une base amorcee tot dans le developpement —
/// doit etre reparé au PROCHAIN amorçage, pas seulement le premier.
///
/// `INSERT ... ON CONFLICT (nom) DO NOTHING` ne pose la ligne qu'une
/// fois : sans `acces_total_toujours_reaffirme`, un compte cassé le
/// resterait pour toujours, sans qu'aucun message n'explique pourquoi
/// le patron ne peut soudain plus rien faire.
#[test]
fn un_patron_sans_acces_total_est_repare_au_prochain_amorcage() {
    let mut base = base_amorcee();
    base.executer("UPDATE role SET acces_total = 0 WHERE nom = 'patron'", &[])
        .unwrap();

    // Verifie que la casse est bien reproduite avant le second amorçage.
    let permissions = portes::permissions_de_sur(&mut base, "quelconque", "patron");
    assert!(
        !permissions.contains("parametres:modifier"),
        "le patron cassé ne doit avoir aucune permission avant réparation"
    );

    amorcage::amorcer(&mut base).expect("second amorçage");

    let permissions = portes::permissions_de_sur(&mut base, "quelconque", "patron");
    assert!(
        permissions.contains("parametres:modifier"),
        "le second amorçage doit réparer acces_total"
    );
    assert!(permissions.contains("ventes:creer"));
}

#[test]
fn le_patron_n_a_que_les_permissions_de_son_role() {
    let mut base = base_amorcee();
    let (utilisateur_id, ..) = compte_admin(&mut base);
    // `patron` a acces_total = 1 (amorcage.rs) : tout le catalogue.
    let permissions = portes::permissions_de_sur(&mut base, &utilisateur_id, "patron");
    assert!(permissions.contains("ventes:creer"));
    assert!(permissions.contains("utilisateurs:gerer"));
}

#[test]
fn un_role_restreint_ne_recoit_que_sa_liste() {
    let mut base = base_amorcee();
    let permissions = portes::permissions_de_sur(&mut base, "quelconque", "caissier");
    assert!(permissions.contains("ventes:creer"));
    assert!(
        !permissions.contains("utilisateurs:gerer"),
        "un caissier ne gère pas les utilisateurs"
    );
}

#[test]
fn une_permission_personnelle_ajoute_sans_changer_le_role() {
    let mut base = base_amorcee();

    // Un caissier auquel on ajoute "stock:transferer" à titre personnel.
    // `utilisateur` n'est pas cloisonné.
    let uid = "u-test-1";
    let now = gescom_noyau::utils::maintenant_iso();
    base.executer(
        "INSERT INTO utilisateur (id, nom, role_id, actif, cree_le, modifie_le, origine)
         SELECT ?1, 'Test', id, 1, ?2, ?2, 'test' FROM role WHERE nom = 'caissier'",
        &parametres![uid, now.clone()],
    )
    .unwrap();
    base.executer(
        "INSERT INTO utilisateur_permission (utilisateur_id, permission, accorde, cree_le)
         VALUES (?1, 'stock:transferer', 1, ?2)",
        &parametres![uid, now],
    )
    .unwrap();

    let permissions = portes::permissions_de_sur(&mut base, uid, "caissier");
    assert!(permissions.contains("stock:transferer"), "l'ajout personnel doit compter");
    assert!(permissions.contains("ventes:creer"), "le rôle reste acquis");
}

#[test]
fn verifier_permission_sur_refuse_ce_qui_n_est_pas_accorde() {
    let mut base = base_amorcee();
    let ctx = ContexteUtilisateur { id: "quelconque".into(), role: "caissier".into() };
    let refus = portes::verifier_permission_sur(&mut base, &ctx, "utilisateurs:gerer");
    assert!(refus.is_err());

    let accepte = portes::verifier_permission_sur(&mut base, &ctx, "ventes:creer");
    assert!(accepte.is_ok());
}

// =====================================================================
//  Le login entier, comme le fait api.rs::connexion
// =====================================================================

#[test]
fn le_login_complet_fonctionne_de_bout_en_bout_sur_base() {
    let mut base = base_amorcee();

    // 1. Identifier le compte.
    let (utilisateur_id, hash, role) = base
        .lire_une(
            "SELECT ua.utilisateur_id, ua.mot_de_passe, r.nom
             FROM utilisateur_auth ua
             JOIN utilisateur u ON u.id = ua.utilisateur_id
             JOIN role r        ON r.id = u.role_id
             WHERE (ua.pseudo = ?1 OR ua.email = ?1) AND u.actif = 1",
            &parametres!["admin"],
            |r| Ok((r.get::<String>(0)?, r.get::<String>(1)?, r.get::<String>(2)?)),
        )
        .unwrap()
        .expect("le compte admin existe");

    // 2. Vérifier le mot de passe — celui posé par amorcage.rs.
    assert!(bcrypt::verify("admin123", &hash).unwrap());
    assert!(!bcrypt::verify("mauvais", &hash).unwrap());

    // 3. Inscrire le poste.
    let poste =
        postes::inscrire_ou_retrouver_sur(&mut base, "Caisse test", "emp-login", "caisse", None)
            .expect("inscription du poste");
    assert!(poste.actif);

    // 4. Ouvrir la session.
    let (session_id, _jeton, _expire) =
        sessions::ouvrir_sur(&mut base, &poste.id, &utilisateur_id).expect("session ouverte");

    // 5. Lire les permissions — patron a accès total.
    let permissions = portes::permissions_de_sur(&mut base, &utilisateur_id, &role);
    assert!(permissions.contains("ventes:creer"));

    // 6. Vérifier que la session tient debout.
    match sessions::etat_sur(&mut base, &session_id) {
        sessions::Etat::Valide { role: r, .. } => assert_eq!(r, role),
        _ => panic!("la session du login doit être valide"),
    }
}
