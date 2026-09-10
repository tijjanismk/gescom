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
use gescom_noyau::{
    argent, caisses, catalogue, comptoir, modeles, postes, sessions, tableau_bord,
};

pub fn registre() -> Registre {
    let mut r = Registre::nouveau();

    r.lecture("lire_stock_multi_depots", |c, _| {
        serde_json::to_value(comptoir::lire_stock_multi_depots_sur(c.conn)?)
            .map_err(|e| e.to_string())
    });

    r.lecture("lire_config_scanner", |c, _| {
        Ok(Value::Bool(comptoir::lire_config_scanner(c.conn)?))
    });

    r.ecriture("creer_client_rapide", "clients:creer", |c, p| {
        comptoir::creer_client_rapide(
            c.conn,
            texte(&p, "nom")?,
            option_texte(&p, "telephone"),
        )
    });

    r.ecriture("creer_article_rapide", "articles:creer", |c, p| {
        comptoir::creer_article_rapide(
            c.conn,
            texte(&p, "nom")?,
            texte(&p, "uniteBase").or_else(|_| texte(&p, "unite_base"))?,
            entier(&p, "prixReference")
                .or_else(|| entier(&p, "prix_reference"))
                .unwrap_or(0),
            entier(&p, "prixAchat").or_else(|| entier(&p, "prix_achat")),
        )
    });

    // ---- Le tableau de bord ----
    //
    // L'ecran d'accueil : une caisse qui se connecte y arrive avant
    // tout le reste. Cinq « commande_inconnue » en guise de bienvenue
    // donneraient l'impression d'un logiciel casse.
    r.lecture("lire_resume_dashboard", |c, p| {
        tableau_bord::lire_resume_dashboard(c.conn, option_texte(&p, "depotId"))
    });

    r.lecture("lire_ventes_periode", |c, p| {
        let v = tableau_bord::lire_ventes_periode(
            c.conn,
            option_texte(&p, "periode"),
            option_texte(&p, "depotId"),
        )?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_top_clients", |c, _| {
        let v = tableau_bord::lire_top_clients(c.conn)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_top_articles", |c, _| {
        let v = tableau_bord::lire_top_articles(c.conn)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_ventes_a_decouvert", |c, p| {
        tableau_bord::lire_ventes_a_decouvert(
            c.conn,
            option_texte(&p, "dateDebut").or_else(|| option_texte(&p, "date_debut")),
            option_texte(&p, "dateFin").or_else(|| option_texte(&p, "date_fin")),
        )
    });

    // ---- Les deux reglements ----
    //
    // Encaisser une creance et payer un fournisseur : sans eux, une
    // caisse vend mais ne peut pas recevoir l'argent du lendemain.
    r.ecriture("enregistrer_paiement", "paiements:creer", |c, p| {
        argent::enregistrer_paiement(
            c.conn,
            texte(&p, "venteId").or_else(|_| texte(&p, "vente_id"))?,
            entier(&p, "montant").unwrap_or(0),
            texte(&p, "mode")?,
            Some(c.appelant.role.clone()),
        )?;
        Ok(json!({ "etat": "enregistre" }))
    });

    r.ecriture("regler_dette_fournisseur", "fournisseurs:regler", |c, p| {
        argent::regler_dette_fournisseur(
            c.conn,
            texte(&p, "fournisseurId").or_else(|_| texte(&p, "fournisseur_id"))?,
            entier(&p, "montant").unwrap_or(0),
            texte(&p, "mode")?,
            option_texte(&p, "note"),
            option_texte(&p, "pieceId").or_else(|| option_texte(&p, "piece_id")),
        )
    });

    // ---- L'argent ----
    //
    // Les deux endroits ou le stock sort et ou l'argent entre. Portes
    // avec leur code, pas recrits : le texte vient de `commandes/`, et
    // les 26 scenarios de `tests_multi_depot` continuent de l'eprouver.
    //
    // Le role vient de la SESSION, comme pour les lectures. Un poste
    // qui enverrait « patron » dans son JSON pourrait sinon franchir
    // les controles reserves au patron.
    r.ecriture("creer_vente", "ventes:creer", |c, p| {
        let lignes: Vec<argent::ParamsLigneInput> = serde_json::from_value(
            p.get("lignes").cloned().unwrap_or(Value::Array(vec![])),
        )
        .map_err(|e| format!("Lignes de vente illisibles : {e}"))?;

        argent::creer_vente_sur(
            c.conn,
            texte(&p, "clientId").or_else(|_| texte(&p, "client_id"))?,
            texte(&p, "depotId").or_else(|_| texte(&p, "depot_id"))?,
            texte(&p, "modeReglement").or_else(|_| texte(&p, "mode_reglement"))?,
            lignes,
            Some(c.appelant.role.clone()),
            entier(&p, "montantPaye").or_else(|| entier(&p, "montant_paye")),
            option_texte(&p, "modePaiement").or_else(|| option_texte(&p, "mode_paiement")),
            entier(&p, "avoirMontant").or_else(|| entier(&p, "avoir_montant")),
        )
    });

    // La facture automatique du point de vente. Son echec ne bloque pas
    // le caissier devant son client — mais sans elle, rien a imprimer.
    r.ecriture("creer_facture_depuis_vente", "pieces:creer", |c, p| {
        argent::creer_facture_depuis_vente_sur(
            c.conn,
            texte(&p, "venteId").or_else(|_| texte(&p, "vente_id"))?,
            texte(&p, "clientId").or_else(|_| texte(&p, "client_id"))?,
            texte(&p, "modeReglement").or_else(|_| texte(&p, "mode_reglement"))?,
            Some(c.appelant.role.clone()),
        )
    });

    r.ecriture("valider_facture", "pieces:creer", |c, p| {
        argent::valider_facture_sur(
            c.conn,
            texte(&p, "pieceId").or_else(|_| texte(&p, "piece_id"))?,
            texte(&p, "modeReglement").or_else(|_| texte(&p, "mode_reglement"))?,
            option_texte(&p, "modePaiement").or_else(|| option_texte(&p, "mode_paiement")),
            entier(&p, "acompte"),
            Some(c.appelant.role.clone()),
        )
    });

    // ---- Le comptoir ----
    //
    // Les cinq lectures qu'un poste caisse appelle avant de pouvoir
    // afficher quoi que ce soit. Sans elles, une caisse connectee
    // montre un ecran vide, et porter `creer_vente` n'aurait servi a
    // rien : on ne peut pas vendre a un client qu'on ne voit pas.
    r.lecture("lire_clients", |c, _| {
        serde_json::to_value(catalogue::lire_clients(c.conn)?)
            .map_err(|e| e.to_string())
    });

    r.lecture("lire_client_generique", |c, _| {
        catalogue::lire_client_generique(c.conn)
    });

    r.lecture("lire_depots", |c, _| {
        serde_json::to_value(catalogue::lire_depots(c.conn)?)
            .map_err(|e| e.to_string())
    });

    r.lecture("lire_depot_defaut", |c, _| {
        catalogue::lire_depot_defaut(c.conn)
    });

    r.lecture("lire_articles_avec_unites", |c, p| {
        // Le role vient de la SESSION, jamais des parametres : un poste
        // qui enverrait « patron » dans son appel lirait les prix
        // d'achat. C'est tout l'interet de faire tourner la lecture
        // chez le serveur.
        let depot = p.get("depotId")
            .or_else(|| p.get("depot_id"))
            .and_then(Value::as_str)
            .map(str::to_string);
        let v = catalogue::lire_articles_avec_unites(
            c.conn, Some(c.appelant.role.clone()), depot,
        )?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

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

    // Les modeles : c'est par la que le poste principal habille toutes
    // les caisses. Un modele corrige ici est vu par tout le magasin au
    // rechargement suivant — sans clef USB, sans reinstallation.
    r.lecture("lire_modeles", |c, p| {
        let genre = p.get("genre").and_then(Value::as_str);
        let v = modeles::lister(c.conn, genre).map_err(|e| e.to_string())?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_modele_actif", |c, p| {
        let genre = texte(&p, "genre")?;
        serde_json::to_value(modeles::lire_actif(c.conn, &genre))
            .map_err(|e| e.to_string())
    });

    r.ecriture("enregistrer_modele", "modeles:gerer", |c, p| {
        let m: modeles::Modele = serde_json::from_value(
            p.get("modele").cloned().unwrap_or(Value::Null),
        )
        .map_err(|e| format!("Modèle illisible : {e}"))?;
        modeles::enregistrer(c.conn, &m, &c.appelant.utilisateur_id)?;
        Ok(json!({ "id": m.id }))
    });

    r.ecriture("definir_modele_actif", "modeles:gerer", |c, p| {
        let id = texte(&p, "id")?;
        modeles::definir_actif(c.conn, &id)?;
        Ok(json!({ "id": id }))
    });

    r.ecriture("supprimer_modele", "modeles:gerer", |c, p| {
        let id = texte(&p, "id")?;
        modeles::supprimer(c.conn, &id)?;
        Ok(json!({ "id": id }))
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

/// Un entier optionnel — les montants arrivent en nombre JSON.
fn entier(p: &Value, cle: &str) -> Option<i64> {
    p.get(cle).and_then(Value::as_i64)
}

/// Une chaine optionnelle. `null` et absence se valent : l'ecran envoie
/// l'un ou l'autre selon les champs, et distinguer les deux ne
/// changerait rien au comportement.
fn option_texte(p: &Value, cle: &str) -> Option<String> {
    p.get(cle).and_then(Value::as_str).map(str::to_string)
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
