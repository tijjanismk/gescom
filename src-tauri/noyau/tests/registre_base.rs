//! Le registre des commandes (v3, D-2) : une poignee par commande, sur
//! `Base`, avec sa permission et son drapeau d'ecriture.
//!
//! Le registre avait deux poignees par commande (`Connection` et
//! `Base`, D11) ; le serveur sert maintenant tout par `Base` (D22).

use gescom_noyau::amorcage;
use gescom_noyau::base::Base;
use gescom_noyau::registre::{Appelant, ContexteBase, Registre};
use serde_json::{json, Value};

fn appelant() -> Appelant {
    Appelant {
        utilisateur_id: "u1".into(),
        role: "patron".into(),
        poste_id: "p1".into(),
        dossier_id: gescom_noyau::dossiers::DOSSIER_DEFAUT.into(),
        session_id: "s1".into(),
    }
}

#[test]
fn une_commande_porte_sa_permission_et_son_drapeau() {
    let mut r = Registre::nouveau();
    r.sur_base("ping", None, false, |_c, _p| Ok(json!("pong")));
    r.sur_base("ecrire", Some("ventes:creer"), true, |_c, _p| Ok(Value::Null));
    let lecture = r.trouver("ping").unwrap();
    assert_eq!(lecture.permission, None);
    assert!(!lecture.ecrit);
    let ecriture = r.trouver("ecrire").unwrap();
    assert_eq!(ecriture.permission, Some("ventes:creer"));
    assert!(ecriture.ecrit);
    assert!(r.trouver("inconnue").is_none());
    assert_eq!(r.noms(), vec!["ecrire", "ping"]);
}

#[test]
fn la_poignee_sert_la_base_de_la_session() {
    let mut base = Base::ouvrir(":memory:").expect("base en mémoire");
    amorcage::amorcer(&mut base).expect("amorçage");

    let mut r = Registre::nouveau();
    r.sur_base("lire_clients", None, false, |c, _p| {
        serde_json::to_value(gescom_noyau::catalogue::lire_clients_sur(c.base)?).map_err(|e| e.to_string())
    });
    let appel = appelant();
    let v = (r.trouver("lire_clients").unwrap().poignee_base)(
        &mut ContexteBase { base: &mut base, appelant: &appel },
        Value::Null,
    )
    .expect("lire_clients");
    // L'amorçage pose le seul client générique (la démo n'est pas là).
    assert_eq!(v.as_array().map(|a| a.len()), Some(1));
}
