//! Les commandes que le serveur sait deja executer.
//!
//! Le v1 a 174 commandes, toutes ecrites contre `State<EtatApp>` de
//! Tauri. Elles migreront ici une par une, chacune avec son test — pas
//! toutes d'un coup : une bascule en bloc de 174 chemins d'ecriture
//! sans filet est exactement le scenario qui coute ses livres a un
//! commercant.
//!
//! Ce fichier porte le socle : celles qui n'existaient pas avant parce
//! qu'elles n'avaient pas de sens en monoposte.

use serde_json::{json, Value};

use gescom_noyau::registre::{Contexte, Registre};
use gescom_noyau::{caisses, postes, sessions};

pub fn registre() -> Registre {
    let mut r = Registre::nouveau();

    r.lecture("lire_postes", |c, _| {
        let v = postes::lister(c.conn).map_err(|e| e.to_string())?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("desactiver_poste", "postes:gerer", |c, p| {
        let id = texte(&p, "poste_id")?;
        // Se couper soi-meme, c'est perdre la main sur le serveur
        // depuis l'ecran qu'on a sous les yeux.
        if id == c.appelant.poste_id {
            return Err("Un poste ne peut pas se désactiver lui-même.".to_string());
        }
        postes::desactiver(c.conn, &id, &c.appelant.utilisateur_id)
            .map_err(|e| e.to_string())?;
        Ok(json!({ "poste_id": id }))
    });

    r.ecriture("reactiver_poste", "postes:gerer", |c, p| {
        let id = texte(&p, "poste_id")?;
        postes::reactiver(c.conn, &id).map_err(|e| e.to_string())?;
        Ok(json!({ "poste_id": id }))
    });

    r.lecture("lire_sessions_reseau", |c, _| {
        let v = sessions::lister_actives(c.conn).map_err(|e| e.to_string())?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("revoquer_session_reseau", "postes:gerer", |c, p| {
        let id = texte(&p, "session_id")?;
        let n = sessions::revoquer(c.conn, &id, &c.appelant.utilisateur_id)
            .map_err(|e| e.to_string())?;
        Ok(json!({ "revoquees": n }))
    });

    r.lecture("lire_mode_caisse", |c, _| {
        Ok(json!({ "par_utilisateur": caisses::par_utilisateur(c.conn) }))
    });

    r.ecriture("definir_mode_caisse", "caisse:configurer", |c, p| {
        let actif = p.get("par_utilisateur").and_then(Value::as_bool).ok_or(
            "Paramètre « par_utilisateur » manquant (true ou false).".to_string(),
        )?;
        caisses::definir_par_utilisateur(c.conn, actif)?;
        Ok(json!({ "par_utilisateur": actif }))
    });

    r
}

fn texte(p: &Value, cle: &str) -> Result<String, String> {
    p.get(cle)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("Paramètre « {cle} » manquant."))
}

/// Le contexte n'est pas utilise par toutes les poignees ; ce garde-fou
/// evite un avertissement sans supprimer le parametre, qui fait partie
/// de la signature commune.
#[allow(dead_code)]
fn _muet(_: &Contexte) {}
