//! La reference de l'horloge : la date-heure de la derniere ecriture
//! reussie du serveur (lot C.5). La regle est dans `coeur::horloge` ; ici
//! sa lecture et son ecriture, sur l'un ou l'autre moteur.
//!
//! Rangee dans `config_app`, qui n'est PAS cloisonnee : une seule
//! horloge garde la base, quel que soit le dossier ouvert. Persistee en
//! base, elle survit au redemarrage du serveur — c'est justement apres
//! une coupure que l'horloge repart en arriere.
//!
//! Pas de version `Connection` : le serveur sert tout par `Base` (D22),
//! et c'est lui seul qui juge l'heure d'une ecriture (`api::rpc`).

use serde_json::{json, Value};

use crate::base::{Acces, Base};
use crate::coeur::horloge as regle;
use crate::parametres;

/// La cle de `config_app`.
pub const CLE: &str = "horloge_derniere_ecriture";

/// La date-heure de la derniere ecriture reussie, si elle a ete notee.
pub fn lire_reference_sur(acces: &mut impl Acces) -> Result<Option<String>, String> {
    acces
        .lire_une("SELECT valeur FROM config_app WHERE cle = ?1", &parametres![CLE], |r| r.get::<String>(0))
        .map_err(|e| e.0)
}

fn poser_reference(acces: &mut impl Acces, valeur: &str) -> Result<(), String> {
    acces
        .executer(
            "INSERT INTO config_app (cle, valeur) VALUES (?1, ?2)
             ON CONFLICT (cle) DO UPDATE SET valeur = excluded.valeur",
            &parametres![CLE, valeur],
        )
        .map(|_| ())
        .map_err(|e| e.0)
}

/// Une ecriture peut-elle passer a `maintenant` ? Le refus porte le
/// message a afficher tel quel (`HORLOGE_RECULE — …`).
pub fn verifier_sur(acces: &mut impl Acces, maintenant: &str) -> Result<(), String> {
    let reference = lire_reference_sur(acces)?;
    regle::juger(reference.as_deref(), maintenant)
}

/// Apres une ecriture reussie : la reference devient la plus recente
/// des deux. N'ecrit rien quand elle ne change pas.
pub fn noter_ecriture_sur(acces: &mut impl Acces, maintenant: &str) -> Result<(), String> {
    let reference = lire_reference_sur(acces)?;
    match regle::nouvelle_reference(reference.as_deref(), maintenant) {
        Some(nouvelle) => poser_reference(acces, &nouvelle),
        None => Ok(()),
    }
}

/// Ce que l'ecran Parametres → Horloge montre.
pub fn lire_horloge_sur(acces: &mut impl Acces, maintenant: &str) -> Result<Value, String> {
    let reference = lire_reference_sur(acces)?;
    let refus = regle::juger(reference.as_deref(), maintenant).err();
    Ok(json!({
        "derniere_ecriture": reference,
        "derniere_ecriture_en_clair": reference.as_deref().map(regle::en_clair),
        "maintenant": maintenant,
        "maintenant_en_clair": regle::en_clair(maintenant),
        "recul_secondes": reference.as_deref().and_then(|r| regle::recul_secondes(r, maintenant)),
        "tolerance_minutes": regle::TOLERANCE_RECUL_SECONDES / 60,
        "bloque": refus.is_some(),
        "refus": refus,
    }))
}

/// Le geste du patron : « l'heure actuelle de Windows est juste ». La
/// reference revient a `maintenant`, et le journal du dossier ouvert
/// garde l'ancienne et la nouvelle valeur. Refuse quand rien n'est
/// bloque (`coeur::horloge::confirmation_permise`).
///
/// La permission (`parametres:modifier`, reservee a qui voit tous les
/// dossiers) est verifiee par le serveur.
pub fn confirmer_heure_sur(base: &mut Base, maintenant: &str) -> Result<Value, String> {
    let ancienne = lire_reference_sur(base)?;
    regle::confirmation_permise(ancienne.as_deref(), maintenant)?;
    let auteur = crate::argent::id_utilisateur_courant_sur(base);
    let dossier = base.dossier().to_string();

    // Deux lignes, tout ou rien : une reference ramenee sans trace
    // effacerait la preuve qu'on l'a fait.
    let mut tx = base.transaction().map_err(|e| e.0)?;
    poser_reference(&mut tx, maintenant)?;
    tx.executer(
        "INSERT INTO journal
           (id, type_evenement, entite_type, entite_id, auteur_id,
            ancien_valeur, nouveau_valeur, origine, date_evenement, dossier_id)
         VALUES (?1, 'horloge_confirmee', 'horloge', ?2, ?3, ?4, ?5, 'app', ?6, ?7)",
        &parametres![
            uuid::Uuid::new_v4().to_string(),
            CLE,
            auteur,
            ancienne.clone(),
            maintenant,
            maintenant,
            dossier
        ],
    )
    .map_err(|e| e.0)?;
    tx.valider().map_err(|e| e.0)?;

    Ok(json!({
        "ancienne": ancienne,
        "nouvelle": maintenant,
        "nouvelle_en_clair": regle::en_clair(maintenant),
    }))
}
