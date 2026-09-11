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
    achats, argent, auth, avoirs, caisse, caisses, catalogue, catalogue_csv, chantiers, cheques, codebarre, comptoir, creances, depots, fournisseurs, journal, livraisons, modeles, pagination, parametres, pieces, pieces_pos, postes, rapports, relances, retours, sauvegarde, sessions, societe, tableau_bord, transferts,
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
    r.aussi_sur_base("lire_config_scanner", |c, _| {
        Ok(Value::Bool(comptoir::lire_config_scanner_sur(c.base)?))
    });

    r.ecriture("creer_client_rapide", "clients:creer", |c, p| {
        comptoir::creer_client_rapide(
            c.conn,
            texte(&p, "nom")?,
            option_texte(&p, "telephone"),
        )
    });
    r.aussi_sur_base("creer_client_rapide", |c, p| {
        comptoir::creer_client_rapide_sur(c.base, texte(&p, "nom")?, option_texte(&p, "telephone"))
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
    r.aussi_sur_base("creer_article_rapide", |c, p| {
        comptoir::creer_article_rapide_sur(
            c.base,
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
    // Meme commande, sur PostgreSQL (D11) : `creer_vente_sur_base`
    // porte a part, testee dans argent_base.rs. Meme lecture des
    // parametres, pour que les deux chemins n'aient qu'une facon de
    // se tromper de nom de champ.
    r.aussi_sur_base("creer_vente", |c, p| {
        let lignes: Vec<argent::ParamsLigneInput> = serde_json::from_value(
            p.get("lignes").cloned().unwrap_or(Value::Array(vec![])),
        )
        .map_err(|e| format!("Lignes de vente illisibles : {e}"))?;

        argent::creer_vente_sur_base(
            c.base,
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
    r.aussi_sur_base("creer_facture_depuis_vente", |c, p| {
        argent::creer_facture_depuis_vente_sur_base(
            c.base,
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
    r.aussi_sur_base("valider_facture", |c, p| {
        argent::valider_facture_sur_base(
            c.base,
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
    r.aussi_sur_base("lire_clients", |c, _| {
        serde_json::to_value(catalogue::lire_clients_sur(c.base)?).map_err(|e| e.to_string())
    });

    r.lecture("lire_client_generique", |c, _| {
        catalogue::lire_client_generique(c.conn)
    });
    r.aussi_sur_base("lire_client_generique", |c, _| {
        catalogue::lire_client_generique_sur(c.base)
    });

    r.lecture("lire_depots", |c, _| {
        serde_json::to_value(catalogue::lire_depots(c.conn)?)
            .map_err(|e| e.to_string())
    });
    r.aussi_sur_base("lire_depots", |c, _| {
        serde_json::to_value(catalogue::lire_depots_sur(c.base)?).map_err(|e| e.to_string())
    });

    r.lecture("lire_depot_defaut", |c, _| {
        catalogue::lire_depot_defaut(c.conn)
    });
    r.aussi_sur_base("lire_depot_defaut", |c, _| {
        catalogue::lire_depot_defaut_sur(c.base)
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
    r.aussi_sur_base("lire_articles_avec_unites", |c, p| {
        let depot = p.get("depotId")
            .or_else(|| p.get("depot_id"))
            .and_then(Value::as_str)
            .map(str::to_string);
        let v = catalogue::lire_articles_avec_unites_sur(
            c.base, Some(c.appelant.role.clone()), depot,
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
        postes::desactiver(c.conn, &id, &c.appelant.utilisateur_id)?;
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

// <<< POIGNEES GENEREES >>>
    // =================================================================
    //  Le reste du metier, genere depuis les facades Tauri
    // =================================================================
    //
    //  Regenerer avec : python outils/generer_socle.py
    //
    //  Les lectures ne demandent aucune permission, le jeton suffit.
    //  Les ecritures portent celle de leur domaine ; la liste blanche
    //  de `portes` fait le tri.
    //
    //  Le role et l'identite viennent de la SESSION, jamais du JSON :
    //  un poste qui enverrait « patron » franchirait sinon les
    //  controles reserves au patron.

    r.ecriture("enregistrer_achat", "achats:creer", |c, p| {
        let fournisseur_id: Option<String> = arg(&p, "fournisseurId", "fournisseur_id")?;
        let depot_id: Option<String> = arg(&p, "depotId", "depot_id")?;
        let lignes: Vec<achats::LigneAchat> = arg(&p, "lignes", "lignes")?;
        let mode_reglement: Option<String> = arg(&p, "modeReglement", "mode_reglement")?;
        let mode_paiement: Option<String> = arg(&p, "modePaiement", "mode_paiement")?;
        let acompte: Option<i64> = arg(&p, "acompte", "acompte")?;
        let note: Option<String> = arg(&p, "note", "note")?;
        let piece_origine_id: Option<String> = arg(&p, "pieceOrigineId", "piece_origine_id")?;
        let v = achats::enregistrer_achat(c.conn, fournisseur_id, depot_id, lignes, mode_reglement, mode_paiement, acompte, note, Some(c.appelant.role.clone()), piece_origine_id)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("enregistrer_retour_fournisseur", "achats:creer", |c, p| {
        let fournisseur_id: String = arg(&p, "fournisseurId", "fournisseur_id")?;
        let depot_id: Option<String> = arg(&p, "depotId", "depot_id")?;
        let lignes: Vec<achats::LigneRetourFournisseur> = arg(&p, "lignes", "lignes")?;
        let piece_origine_id: Option<String> = arg(&p, "pieceOrigineId", "piece_origine_id")?;
        let mode_resolution: Option<String> = arg(&p, "modeResolution", "mode_resolution")?;
        let mode_encaissement: Option<String> = arg(&p, "modeEncaissement", "mode_encaissement")?;
        let motif: Option<String> = arg(&p, "motif", "motif")?;
        let v = achats::enregistrer_retour_fournisseur(c.conn, fournisseur_id, depot_id, lignes, piece_origine_id, mode_resolution, mode_encaissement, motif, Some(c.appelant.role.clone()))?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("valider_facture_fournisseur", "achats:creer", |c, p| {
        let piece_id: String = arg(&p, "pieceId", "piece_id")?;
        let mode_reglement: String = arg(&p, "modeReglement", "mode_reglement")?;
        let mode_paiement: Option<String> = arg(&p, "modePaiement", "mode_paiement")?;
        let acompte: Option<i64> = arg(&p, "acompte", "acompte")?;
        let v = achats::valider_facture_fournisseur(c.conn, piece_id, mode_reglement, mode_paiement, acompte, Some(c.appelant.role.clone()))?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("annuler_facture_fournisseur_par_avoir", "achats:creer", |c, p| {
        let piece_id: String = arg(&p, "pieceId", "piece_id")?;
        let mode_resolution: Option<String> = arg(&p, "modeResolution", "mode_resolution")?;
        let mode_encaissement: Option<String> = arg(&p, "modeEncaissement", "mode_encaissement")?;
        let motif: Option<String> = arg(&p, "motif", "motif")?;
        let v = achats::annuler_facture_fournisseur_par_avoir(c.conn, piece_id, mode_resolution, mode_encaissement, motif, Some(c.appelant.role.clone()))?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    // -----------------------------------------------------------------
    //  Les images de la boutique
    // -----------------------------------------------------------------
    // Logo, en-tete et pied sont des FICHIERS sur le disque ; la base
    // ne garde que leur chemin. Une caisse en reseau lisait donc son
    // propre disque, ou il n'y a rien : elle imprimait des factures
    // sans logo ni en-tete, pendant que le poste serveur les imprimait
    // completes. Deux factures differentes pour la meme boutique.
    //
    // Le dossier de repli se deduit du fichier de la base : c'est la
    // que l'application depose ses images.
    // Metier pur, restees dans le crate applicatif par oubli : une
    // caisse en reseau les executait sur SA base locale — vide.
    // Modifier un client y semblait reussir, et la modification
    // n'existait nulle part.
    r.ecriture("modifier_client", "clients:modifier", |c, p| {
        let client_id: String = arg(&p, "clientId", "client_id")?;
        let nom: String = arg(&p, "nom", "nom")?;
        let telephone: Option<String> = arg(&p, "telephone", "telephone")?;
        let adresse: Option<String> = arg(&p, "adresse", "adresse")?;
        let email: Option<String> = arg(&p, "email", "email")?;
        let nif: Option<String> = arg(&p, "nif", "nif")?;
        gescom_noyau::comptoir::modifier_client(
            c.conn, client_id, nom, telephone, adresse, email, nif,
        )?;
        Ok(serde_json::Value::Null)
    });
    r.aussi_sur_base("modifier_client", |c, p| {
        let client_id: String = arg(&p, "clientId", "client_id")?;
        let nom: String = arg(&p, "nom", "nom")?;
        let telephone: Option<String> = arg(&p, "telephone", "telephone")?;
        let adresse: Option<String> = arg(&p, "adresse", "adresse")?;
        let email: Option<String> = arg(&p, "email", "email")?;
        let nif: Option<String> = arg(&p, "nif", "nif")?;
        gescom_noyau::comptoir::modifier_client_sur(
            c.base, client_id, nom, telephone, adresse, email, nif,
        )?;
        Ok(serde_json::Value::Null)
    });

    r.lecture("lire_clients_avec_creances", |c, _| {
        let v = gescom_noyau::comptoir::lire_clients_avec_creances(c.conn)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });
    r.aussi_sur_base("lire_clients_avec_creances", |c, _| {
        let v = gescom_noyau::comptoir::lire_clients_avec_creances_sur(c.base)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_modele", |c, p| {
        let id: String = arg(&p, "id", "id")?;
        let v = gescom_noyau::modeles::lire(c.conn, &id)
            .map_err(|_| "Modèle introuvable.".to_string())?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_logo_base64", |c, _| {
        let v = gescom_noyau::images::lire_base64(
            c.conn, "logo", dossier_des_images(c.conn).as_deref(),
        )?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_entete_base64", |c, _| {
        let v = gescom_noyau::images::lire_base64(
            c.conn, "entete", dossier_des_images(c.conn).as_deref(),
        )?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_pied_base64", |c, _| {
        let v = gescom_noyau::images::lire_base64(
            c.conn, "pied", dossier_des_images(c.conn).as_deref(),
        )?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_factures_fournisseur_retournables", |c, p| {
        let fournisseur_id: String = arg(&p, "fournisseurId", "fournisseur_id")?;
        let v = achats::lire_factures_fournisseur_retournables(c.conn, fournisseur_id)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture_libre("connexion", |c, p| {
        let identifiant: String = arg(&p, "identifiant", "identifiant")?;
        let mot_de_passe: String = arg(&p, "motDePasse", "mot_de_passe")?;
        let v = auth::connexion(c.conn, identifiant, mot_de_passe)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture_libre("changer_mot_de_passe", |c, p| {
        let ancien_mdp: String = arg(&p, "ancienMdp", "ancien_mdp")?;
        let nouveau_mdp: String = arg(&p, "nouveauMdp", "nouveau_mdp")?;
        let v = auth::changer_mot_de_passe(c.conn, c.appelant.utilisateur_id.clone(), ancien_mdp, nouveau_mdp)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });
    // Sans elle, le changement de mot de passe OBLIGATOIRE a la
    // premiere connexion (amorcage.rs) bloquerait tout essai manuel sur
    // PostgreSQL des l'ecran suivant le login — la modale ne se ferme
    // pas tant que la commande n'a pas reussi.
    r.aussi_sur_base("changer_mot_de_passe", |c, p| {
        let ancien_mdp: String = arg(&p, "ancienMdp", "ancien_mdp")?;
        let nouveau_mdp: String = arg(&p, "nouveauMdp", "nouveau_mdp")?;
        let v = auth::changer_mot_de_passe_sur(
            c.base, c.appelant.utilisateur_id.clone(), ancien_mdp, nouveau_mdp,
        )?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    // -----------------------------------------------------------------
    //  Roles et permissions
    // -----------------------------------------------------------------
    // Le CATALOGUE ne se lit qu'ici : c'est du code, il ne se modifie
    // pas depuis l'ecran. Ce qui se modifie, c'est qui a quoi.
    r.lecture("lire_catalogue_permissions", |_c, _p| {
        Ok(gescom_noyau::roles::lire_catalogue_permissions())
    });

    r.lecture("lire_roles", |c, _p| {
        gescom_noyau::roles::lire_roles(c.conn)
    });

    r.lecture("lire_permissions_utilisateur", |c, p| {
        let utilisateur_id: String = arg(&p, "utilisateurId", "utilisateur_id")?;
        gescom_noyau::roles::lire_permissions_utilisateur(c.conn, utilisateur_id)
    });

    r.ecriture("creer_role", "utilisateurs:gerer", |c, p| {
        let nom: String = arg(&p, "nom", "nom")?;
        let description: Option<String> = arg(&p, "description", "description")?;
        let permissions: Vec<String> = arg(&p, "permissions", "permissions")?;
        gescom_noyau::roles::creer_role(c.conn, nom, description, permissions)
    });

    r.ecriture("modifier_role", "utilisateurs:gerer", |c, p| {
        let role_id: String = arg(&p, "roleId", "role_id")?;
        let description: Option<String> = arg(&p, "description", "description")?;
        let permissions: Vec<String> = arg(&p, "permissions", "permissions")?;
        gescom_noyau::roles::modifier_role(c.conn, role_id, description, permissions)
    });

    r.ecriture("supprimer_role", "utilisateurs:gerer", |c, p| {
        let role_id: String = arg(&p, "roleId", "role_id")?;
        gescom_noyau::roles::supprimer_role(c.conn, role_id)
    });

    r.ecriture("definir_permission_utilisateur", "utilisateurs:gerer", |c, p| {
        let utilisateur_id: String = arg(&p, "utilisateurId", "utilisateur_id")?;
        let permission: String = arg(&p, "permission", "permission")?;
        let accorde: Option<bool> = arg(&p, "accorde", "accorde")?;
        let par = Some(c.appelant.utilisateur_id.clone());
        gescom_noyau::roles::definir_permission_utilisateur(
            c.conn, utilisateur_id, permission, accorde, par,
        )
    });

    r.ecriture("creer_utilisateur", "utilisateurs:gerer", |c, p| {
        let nom: String = arg(&p, "nom", "nom")?;
        let pseudo: String = arg(&p, "pseudo", "pseudo")?;
        let email: Option<String> = arg(&p, "email", "email")?;
        let mot_de_passe: String = arg(&p, "motDePasse", "mot_de_passe")?;
        let role_nom: String = arg(&p, "roleNom", "role_nom")?;
        let v = auth::creer_utilisateur(c.conn, nom, pseudo, email, mot_de_passe, role_nom, c.appelant.utilisateur_id.clone())?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_utilisateurs", |c, _p| {
        let v = auth::lire_utilisateurs(c.conn)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_avoirs_client", |c, p| {
        let client_id: String = arg(&p, "clientId", "client_id")?;
        let v = avoirs::lire_avoirs_client(c.conn, client_id)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("total_avoirs_client", |c, p| {
        let client_id: String = arg(&p, "clientId", "client_id")?;
        let v = avoirs::total_avoirs_client(c.conn, client_id)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("appliquer_avoir_vente", "avoirs:gerer", |c, p| {
        let vente_id: String = arg(&p, "venteId", "vente_id")?;
        let client_id: String = arg(&p, "clientId", "client_id")?;
        let montant_demande: i64 = arg(&p, "montantDemande", "montant_demande")?;
        let v = avoirs::appliquer_avoir_vente(c.conn, vente_id, client_id, montant_demande)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("chercher_article_par_code_barre", |c, p| {
        let code_barre: String = arg(&p, "codeBarre", "code_barre")?;
        let v = avoirs::chercher_article_par_code_barre(c.conn, code_barre)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("sauvegarder_config_scanner", "avoirs:gerer", |c, p| {
        let actif: bool = arg(&p, "actif", "actif")?;
        let v = avoirs::sauvegarder_config_scanner(c.conn, actif)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("sauvegarder_code_barre_article", "avoirs:gerer", |c, p| {
        let article_id: String = arg(&p, "articleId", "article_id")?;
        let code_barre: String = arg(&p, "codeBarre", "code_barre")?;
        let v = avoirs::sauvegarder_code_barre_article(c.conn, article_id, code_barre)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_articles_avec_codes_barres", |c, _p| {
        let v = avoirs::lire_articles_avec_codes_barres(c.conn)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("rembourser_avoir", "avoirs:gerer", |c, p| {
        let piece_id: String = arg(&p, "pieceId", "piece_id")?;
        let montant: i64 = arg(&p, "montant", "montant")?;
        let mode: String = arg(&p, "mode", "mode")?;
        let v = avoirs::rembourser_avoir(c.conn, piece_id, montant, mode, Some(c.appelant.role.clone()))?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_resume_caisse", |c, _p| {
        let v = caisse::lire_resume_caisse(c.conn)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });
    r.aussi_sur_base("lire_resume_caisse", |c, _p| {
        caisse::lire_resume_caisse_sur(c.base)
    });

    r.lecture("lire_mouvements_caisse_du_jour", |c, _p| {
        let v = caisse::lire_mouvements_caisse_du_jour(c.conn)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("ouvrir_session_caisse", "caisse:mouvementer", |c, p| {
        let fond_ouverture: i64 = arg(&p, "fondOuverture", "fond_ouverture")?;
        let v = caisse::ouvrir_session_caisse(c.conn, fond_ouverture, c.appelant.role.clone())?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });
    r.aussi_sur_base("ouvrir_session_caisse", |c, p| {
        let fond_ouverture: i64 = arg(&p, "fondOuverture", "fond_ouverture")?;
        let v =
            caisse::ouvrir_session_caisse_sur(c.base, fond_ouverture, c.appelant.role.clone())?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("fermer_session_caisse", "caisse:mouvementer", |c, p| {
        let session_id: String = arg(&p, "sessionId", "session_id")?;
        let especes_comptees: i64 = arg(&p, "especesComptees", "especes_comptees")?;
        let v = caisse::fermer_session_caisse(c.conn, session_id, especes_comptees)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });
    r.aussi_sur_base("fermer_session_caisse", |c, p| {
        let session_id: String = arg(&p, "sessionId", "session_id")?;
        let especes_comptees: i64 = arg(&p, "especesComptees", "especes_comptees")?;
        let v = caisse::fermer_session_caisse_sur(c.base, session_id, especes_comptees)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("enregistrer_depense", "caisse:mouvementer", |c, p| {
        let montant: i64 = arg(&p, "montant", "montant")?;
        let libelle: String = arg(&p, "libelle", "libelle")?;
        let categorie: Option<String> = arg(&p, "categorie", "categorie")?;
        let moyen: Option<String> = arg(&p, "moyen", "moyen")?;
        let v = caisse::enregistrer_depense(c.conn, montant, libelle, categorie, moyen, Some(c.appelant.role.clone()))?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_depenses_du_jour", |c, _p| {
        let v = caisse::lire_depenses_du_jour(c.conn)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_sessions_caisse", |c, p| {
        let limite: Option<i64> = arg(&p, "limite", "limite")?;
        let v = caisse::lire_sessions_caisse(c.conn, limite)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_mouvements_session", |c, p| {
        let session_id: String = arg(&p, "sessionId", "session_id")?;
        let v = caisse::lire_mouvements_session(c.conn, session_id)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_rapport_ecarts", |c, p| {
        let jours: Option<i64> = arg(&p, "jours", "jours")?;
        let v = caisse::lire_rapport_ecarts(c.conn, jours)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("modifier_depense", "caisse:mouvementer", |c, p| {
        let mouvement_id: String = arg(&p, "mouvementId", "mouvement_id")?;
        let montant: Option<i64> = arg(&p, "montant", "montant")?;
        let libelle: Option<String> = arg(&p, "libelle", "libelle")?;
        let categorie: Option<String> = arg(&p, "categorie", "categorie")?;
        let v = caisse::modifier_depense(c.conn, mouvement_id, montant, libelle, categorie)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("exporter_articles_csv", |c, _p| {
        let v = catalogue_csv::exporter_articles_csv(c.conn)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("importer_articles_csv", "articles:creer", |c, p| {
        let contenu: String = arg(&p, "contenu", "contenu")?;
        let mettre_a_jour: Option<bool> = arg(&p, "mettreAJour", "mettre_a_jour")?;
        let v = catalogue_csv::importer_articles_csv(c.conn, contenu, mettre_a_jour)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_etat_stock", |c, p| {
        let depot_id: Option<String> = arg(&p, "depotId", "depot_id")?;
        let avec_zero: Option<bool> = arg(&p, "avecZero", "avec_zero")?;
        let v = catalogue_csv::lire_etat_stock(c.conn, depot_id, avec_zero)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_taux_tva", |c, _p| {
        let v = chantiers::lire_taux_tva(c.conn)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("sauvegarder_tva_article", "chantiers:gerer", |c, p| {
        let article_id: String = arg(&p, "articleId", "article_id")?;
        let taux_tva: f64 = arg(&p, "tauxTva", "taux_tva")?;
        let v = chantiers::sauvegarder_tva_article(c.conn, article_id, taux_tva)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_resume_tva", |c, p| {
        let date_debut: String = arg(&p, "dateDebut", "date_debut")?;
        let date_fin: String = arg(&p, "dateFin", "date_fin")?;
        let v = chantiers::lire_resume_tva(c.conn, date_debut, date_fin)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_dettes_fournisseurs", |c, _p| {
        let v = chantiers::lire_dettes_fournisseurs(c.conn)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("marquer_irrecouvrable", "chantiers:gerer", |c, p| {
        let vente_id: String = arg(&p, "venteId", "vente_id")?;
        let motif: String = arg(&p, "motif", "motif")?;
        let v = chantiers::marquer_irrecouvrable(c.conn, vente_id, motif)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_irrecouvrable", |c, _p| {
        let v = chantiers::lire_irrecouvrable(c.conn)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_config_avoirs", |c, _p| {
        let v = chantiers::lire_config_avoirs(c.conn)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("sauvegarder_config_avoirs", "chantiers:gerer", |c, p| {
        let active: bool = arg(&p, "active", "active")?;
        let duree_jours: i64 = arg(&p, "dureeJours", "duree_jours")?;
        let v = chantiers::sauvegarder_config_avoirs(c.conn, active, duree_jours)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("expirer_avoirs", "chantiers:gerer", |c, _p| {
        let v = chantiers::expirer_avoirs(c.conn)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_factures_fournisseur_ouvertes", |c, p| {
        let fournisseur_id: String = arg(&p, "fournisseurId", "fournisseur_id")?;
        let v = chantiers::lire_factures_fournisseur_ouvertes(c.conn, fournisseur_id)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("reactiver_avoir", "chantiers:gerer", |c, p| {
        let avoir_id: String = arg(&p, "avoirId", "avoir_id")?;
        let v = chantiers::reactiver_avoir(c.conn, avoir_id, Some(c.appelant.role.clone()))?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_avoirs_expires", |c, _p| {
        let v = chantiers::lire_avoirs_expires(c.conn)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("enregistrer_cheque", "cheques:gerer", |c, p| {
        let paiement_id: Option<String> = arg(&p, "paiementId", "paiement_id")?;
        let vente_id: Option<String> = arg(&p, "venteId", "vente_id")?;
        let numero: String = arg(&p, "numero", "numero")?;
        let banque: String = arg(&p, "banque", "banque")?;
        let tireur: Option<String> = arg(&p, "tireur", "tireur")?;
        let montant: i64 = arg(&p, "montant", "montant")?;
        let date_emission: Option<String> = arg(&p, "dateEmission", "date_emission")?;
        let date_echeance: Option<String> = arg(&p, "dateEcheance", "date_echeance")?;
        let v = cheques::enregistrer_cheque(c.conn, paiement_id, vente_id, numero, banque, tireur, montant, date_emission, date_echeance)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_cheques", |c, p| {
        let statut: Option<String> = arg(&p, "statut", "statut")?;
        let v = cheques::lire_cheques(c.conn, statut)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("changer_statut_cheque", "cheques:gerer", |c, p| {
        let cheque_id: String = arg(&p, "chequeId", "cheque_id")?;
        let statut: String = arg(&p, "statut", "statut")?;
        let motif: Option<String> = arg(&p, "motif", "motif")?;
        let v = cheques::changer_statut_cheque(c.conn, cheque_id, statut, motif)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("generer_code_barre", "articles:creer", |c, p| {
        let article_id: String = arg(&p, "articleId", "article_id")?;
        let v = codebarre::generer_code_barre(c.conn, article_id)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("generer_codes_barres_manquants", "articles:creer", |c, _p| {
        let v = codebarre::generer_codes_barres_manquants(c.conn)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("definir_code_barre", "articles:creer", |c, p| {
        let article_id: String = arg(&p, "articleId", "article_id")?;
        let code: String = arg(&p, "code", "code")?;
        let v = codebarre::definir_code_barre(c.conn, article_id, code)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_articles_codes_barres", |c, p| {
        let sans_code_seulement: Option<bool> = arg(&p, "sansCodeSeulement", "sans_code_seulement")?;
        let v = codebarre::lire_articles_codes_barres(c.conn, sans_code_seulement)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_etat_creances_client", |c, p| {
        let client_id: String = arg(&p, "clientId", "client_id")?;
        let v = creances::lire_etat_creances_client(c.conn, client_id)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_etat_creances_global", |c, _p| {
        let v = creances::lire_etat_creances_global(c.conn)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_reglements_client", |c, p| {
        let client_id: String = arg(&p, "clientId", "client_id")?;
        let v = creances::lire_reglements_client(c.conn, client_id)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("annuler_reglement", "creances:gerer", |c, p| {
        let paiement_id: String = arg(&p, "paiementId", "paiement_id")?;
        let motif: String = arg(&p, "motif", "motif")?;
        let remboursement: bool = arg(&p, "remboursement", "remboursement")?;
        let v = creances::annuler_reglement(c.conn, paiement_id, motif, remboursement, Some(c.appelant.role.clone()))?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_donnees_recu", |c, p| {
        let paiement_id: String = arg(&p, "paiementId", "paiement_id")?;
        let cote: String = arg(&p, "cote", "cote")?;
        let v = creances::lire_donnees_recu(c.conn, paiement_id, cote)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_creances_ouvertes", |c, p| {
        let recherche: Option<String> = arg(&p, "recherche", "recherche")?;
        let v = creances::lire_creances_ouvertes(c.conn, recherche)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("regler_creance", "creances:gerer", |c, p| {
        let vente_id: String = arg(&p, "venteId", "vente_id")?;
        let montant: i64 = arg(&p, "montant", "montant")?;
        let mode: String = arg(&p, "mode", "mode")?;
        let v = creances::regler_creance(c.conn, vente_id, montant, mode, Some(c.appelant.role.clone()))?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("solder_residus_creances", "creances:gerer", |c, p| {
        let simulation: Option<bool> = arg(&p, "simulation", "simulation")?;
        let v = creances::solder_residus_creances(c.conn, Some(c.appelant.role.clone()), simulation)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_depots_detail", |c, _p| {
        let v = depots::lire_depots_detail(c.conn)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("creer_depot", "depots:gerer", |c, p| {
        let nom: String = arg(&p, "nom", "nom")?;
        let est_defaut: Option<bool> = arg(&p, "estDefaut", "est_defaut")?;
        let v = depots::creer_depot(c.conn, nom, est_defaut)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("renommer_depot", "depots:gerer", |c, p| {
        let depot_id: String = arg(&p, "depotId", "depot_id")?;
        let nom: String = arg(&p, "nom", "nom")?;
        let v = depots::renommer_depot(c.conn, depot_id, nom)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("definir_depot_defaut", "depots:gerer", |c, p| {
        let depot_id: String = arg(&p, "depotId", "depot_id")?;
        let v = depots::definir_depot_defaut(c.conn, depot_id)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("desactiver_depot", "depots:gerer", |c, p| {
        let depot_id: String = arg(&p, "depotId", "depot_id")?;
        let force: Option<bool> = arg(&p, "force", "force")?;
        let v = depots::desactiver_depot(c.conn, depot_id, force)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("reactiver_depot", "depots:gerer", |c, p| {
        let depot_id: String = arg(&p, "depotId", "depot_id")?;
        let v = depots::reactiver_depot(c.conn, depot_id)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_stock_depot", |c, p| {
        let depot_id: String = arg(&p, "depotId", "depot_id")?;
        let v = depots::lire_stock_depot(c.conn, depot_id)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_resume_par_depot", |c, p| {
        let date_debut: Option<String> = arg(&p, "dateDebut", "date_debut")?;
        let date_fin: Option<String> = arg(&p, "dateFin", "date_fin")?;
        let v = depots::lire_resume_par_depot(c.conn, date_debut, date_fin)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_stock_article_depots", |c, p| {
        let article_id: String = arg(&p, "articleId", "article_id")?;
        let v = depots::lire_stock_article_depots(c.conn, article_id)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_mouvements_stock", |c, p| {
        let article_id: Option<String> = arg(&p, "articleId", "article_id")?;
        let depot_id: Option<String> = arg(&p, "depotId", "depot_id")?;
        let type_mouvement: Option<String> = arg(&p, "typeMouvement", "type_mouvement")?;
        let date_debut: Option<String> = arg(&p, "dateDebut", "date_debut")?;
        let date_fin: Option<String> = arg(&p, "dateFin", "date_fin")?;
        let limite: Option<i64> = arg(&p, "limite", "limite")?;
        let v = depots::lire_mouvements_stock(c.conn, article_id, depot_id, type_mouvement, date_debut, date_fin, limite)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_fournisseurs", |c, _p| {
        let v = fournisseurs::lire_fournisseurs(c.conn)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_fournisseurs_avec_dettes", |c, _p| {
        let v = fournisseurs::lire_fournisseurs_avec_dettes(c.conn)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("creer_fournisseur", "fournisseurs:regler", |c, p| {
        let nom: String = arg(&p, "nom", "nom")?;
        let telephone: Option<String> = arg(&p, "telephone", "telephone")?;
        let adresse: Option<String> = arg(&p, "adresse", "adresse")?;
        let nif: Option<String> = arg(&p, "nif", "nif")?;
        let email: Option<String> = arg(&p, "email", "email")?;
        let est_voisin: Option<bool> = arg(&p, "estVoisin", "est_voisin")?;
        let v = fournisseurs::creer_fournisseur(c.conn, nom, telephone, adresse, nif, email, est_voisin)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("modifier_fournisseur", "fournisseurs:regler", |c, p| {
        let fournisseur_id: String = arg(&p, "fournisseurId", "fournisseur_id")?;
        let nom: String = arg(&p, "nom", "nom")?;
        let telephone: Option<String> = arg(&p, "telephone", "telephone")?;
        let adresse: Option<String> = arg(&p, "adresse", "adresse")?;
        let nif: Option<String> = arg(&p, "nif", "nif")?;
        let email: Option<String> = arg(&p, "email", "email")?;
        let est_voisin: Option<bool> = arg(&p, "estVoisin", "est_voisin")?;
        let v = fournisseurs::modifier_fournisseur(c.conn, fournisseur_id, nom, telephone, adresse, nif, email, est_voisin)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_etat_dette_fournisseur", |c, p| {
        let fournisseur_id: String = arg(&p, "fournisseurId", "fournisseur_id")?;
        let v = fournisseurs::lire_etat_dette_fournisseur(c.conn, fournisseur_id)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_etat_dettes_global", |c, _p| {
        let v = fournisseurs::lire_etat_dettes_global(c.conn)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("enregistrer_entree_stock", "fournisseurs:regler", |c, p| {
        let article_id: String = arg(&p, "articleId", "article_id")?;
        let depot_id: Option<String> = arg(&p, "depotId", "depot_id")?;
        let quantite: f64 = arg(&p, "quantite", "quantite")?;
        let prix_achat: Option<i64> = arg(&p, "prixAchat", "prix_achat")?;
        let fournisseur_id: Option<String> = arg(&p, "fournisseurId", "fournisseur_id")?;
        let v = fournisseurs::enregistrer_entree_stock(c.conn, article_id, depot_id, quantite, prix_achat, fournisseur_id, Some(c.appelant.role.clone()))?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("enregistrer_retour_sans_facture", "fournisseurs:regler", |c, p| {
        let article_id: String = arg(&p, "articleId", "article_id")?;
        let depot_id: Option<String> = arg(&p, "depotId", "depot_id")?;
        let quantite: f64 = arg(&p, "quantite", "quantite")?;
        let fournisseur_id: Option<String> = arg(&p, "fournisseurId", "fournisseur_id")?;
        let motif: Option<String> = arg(&p, "motif", "motif")?;
        let v = fournisseurs::enregistrer_retour_sans_facture(c.conn, article_id, depot_id, quantite, fournisseur_id, motif, Some(c.appelant.role.clone()))?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("enregistrer_ajustement_inventaire", "fournisseurs:regler", |c, p| {
        let article_id: String = arg(&p, "articleId", "article_id")?;
        let depot_id: String = arg(&p, "depotId", "depot_id")?;
        let quantite_reelle: f64 = arg(&p, "quantiteReelle", "quantite_reelle")?;
        let motif: Option<String> = arg(&p, "motif", "motif")?;
        let v = fournisseurs::enregistrer_ajustement_inventaire(c.conn, article_id, depot_id, quantite_reelle, motif, Some(c.appelant.role.clone()))?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_fournisseur_detail", |c, p| {
        let fournisseur_id: String = arg(&p, "fournisseurId", "fournisseur_id")?;
        let v = fournisseurs::lire_fournisseur_detail(c.conn, fournisseur_id)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_fiche_fournisseur", |c, p| {
        let fournisseur_id: String = arg(&p, "fournisseurId", "fournisseur_id")?;
        let v = fournisseurs::lire_fiche_fournisseur(c.conn, fournisseur_id)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("annuler_paiement_fournisseur", "fournisseurs:regler", |c, p| {
        let paiement_id: String = arg(&p, "paiementId", "paiement_id")?;
        let motif: String = arg(&p, "motif", "motif")?;
        let remboursement: bool = arg(&p, "remboursement", "remboursement")?;
        let v = fournisseurs::annuler_paiement_fournisseur(c.conn, paiement_id, motif, remboursement, Some(c.appelant.role.clone()))?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_journal_du_jour", |c, p| {
        let date: Option<String> = arg(&p, "date", "date")?;
        let depot_id: Option<String> = arg(&p, "depotId", "depot_id")?;
        let v = journal::lire_journal_du_jour(c.conn, date, depot_id)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_livraison_piece", |c, p| {
        let piece_id: String = arg(&p, "pieceId", "piece_id")?;
        let v = livraisons::lire_livraison_piece(c.conn, piece_id)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("enregistrer_livraison", "livraisons:enregistrer", |c, p| {
        let piece_id: String = arg(&p, "pieceId", "piece_id")?;
        let lignes: Vec<livraisons::LigneLivraison> = arg(&p, "lignes", "lignes")?;
        let v = livraisons::enregistrer_livraison(c.conn, piece_id, lignes)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_ventes_paginees", |c, p| {
        let page: i64 = arg(&p, "page", "page")?;
        let limite: i64 = arg(&p, "limite", "limite")?;
        let recherche: Option<String> = arg(&p, "recherche", "recherche")?;
        let statut: Option<String> = arg(&p, "statut", "statut")?;
        let periode: Option<String> = arg(&p, "periode", "periode")?;
        let date_debut: Option<String> = arg(&p, "dateDebut", "date_debut")?;
        let date_fin: Option<String> = arg(&p, "dateFin", "date_fin")?;
        let v = pagination::lire_ventes_paginees(c.conn, page, limite, recherche, statut, periode, date_debut, date_fin)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_clients_pagines", |c, p| {
        let page: i64 = arg(&p, "page", "page")?;
        let limite: i64 = arg(&p, "limite", "limite")?;
        let recherche: Option<String> = arg(&p, "recherche", "recherche")?;
        let avec_creances_seulement: bool = arg(&p, "avecCreancesSeulement", "avec_creances_seulement")?;
        let ventes_filtre: Option<String> = arg(&p, "ventesFiltre", "ventes_filtre")?;
        let tri: Option<String> = arg(&p, "tri", "tri")?;
        let v = pagination::lire_clients_pagines(c.conn, page, limite, recherche, avec_creances_seulement, ventes_filtre, tri)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_stocks_pagines", |c, p| {
        let page: i64 = arg(&p, "page", "page")?;
        let limite: i64 = arg(&p, "limite", "limite")?;
        let recherche: Option<String> = arg(&p, "recherche", "recherche")?;
        let a_regulariser_seulement: bool = arg(&p, "aRegulariserSeulement", "a_regulariser_seulement")?;
        let categorie_id: Option<String> = arg(&p, "categorieId", "categorie_id")?;
        let v = pagination::lire_stocks_pagines(c.conn, page, limite, recherche, a_regulariser_seulement, categorie_id)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_fournisseurs_pagines", |c, p| {
        let page: i64 = arg(&p, "page", "page")?;
        let limite: i64 = arg(&p, "limite", "limite")?;
        let recherche: Option<String> = arg(&p, "recherche", "recherche")?;
        let v = pagination::lire_fournisseurs_pagines(c.conn, page, limite, recherche)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_ventes_recentes_paginee", |c, p| {
        let page: i64 = arg(&p, "page", "page")?;
        let limite: i64 = arg(&p, "limite", "limite")?;
        let recherche: Option<String> = arg(&p, "recherche", "recherche")?;
        let periode: Option<String> = arg(&p, "periode", "periode")?;
        let date_debut: Option<String> = arg(&p, "dateDebut", "date_debut")?;
        let date_fin: Option<String> = arg(&p, "dateFin", "date_fin")?;
        let v = pagination::lire_ventes_recentes_paginee(c.conn, page, limite, recherche, periode, date_debut, date_fin)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_categories", |c, _p| {
        let v = parametres::lire_categories(c.conn)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("creer_categorie", "parametres:modifier", |c, p| {
        let nom: String = arg(&p, "nom", "nom")?;
        let v = parametres::creer_categorie(c.conn, nom)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_articles_complets", |c, _p| {
        let v = parametres::lire_articles_complets(c.conn)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("creer_article_complet", "parametres:modifier", |c, p| {
        let nom: String = arg(&p, "nom", "nom")?;
        let categorie_id: Option<String> = arg(&p, "categorieId", "categorie_id")?;
        let unite_base: String = arg(&p, "uniteBase", "unite_base")?;
        let prix_reference: i64 = arg(&p, "prixReference", "prix_reference")?;
        let v = parametres::creer_article_complet(c.conn, nom, categorie_id, unite_base, prix_reference)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("ajouter_unite_vente", "parametres:modifier", |c, p| {
        let article_id: String = arg(&p, "articleId", "article_id")?;
        let libelle: String = arg(&p, "libelle", "libelle")?;
        let facteur: f64 = arg(&p, "facteur", "facteur")?;
        let prix_reference: i64 = arg(&p, "prixReference", "prix_reference")?;
        let code_barre: Option<String> = arg(&p, "codeBarre", "code_barre")?;
        let v = parametres::ajouter_unite_vente(c.conn, article_id, libelle, facteur, prix_reference, code_barre)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("modifier_unite_vente", "parametres:modifier", |c, p| {
        let unite_id: String = arg(&p, "uniteId", "unite_id")?;
        let libelle: Option<String> = arg(&p, "libelle", "libelle")?;
        let facteur: Option<f64> = arg(&p, "facteur", "facteur")?;
        let prix_reference: Option<i64> = arg(&p, "prixReference", "prix_reference")?;
        let code_barre: Option<String> = arg(&p, "codeBarre", "code_barre")?;
        let v = parametres::modifier_unite_vente(c.conn, unite_id, libelle, facteur, prix_reference, code_barre)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("desactiver_unite_vente", "parametres:modifier", |c, p| {
        let unite_id: String = arg(&p, "uniteId", "unite_id")?;
        let v = parametres::desactiver_unite_vente(c.conn, unite_id)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_config_bon_sortie", |c, _p| {
        let v = parametres::lire_config_bon_sortie(c.conn)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("sauvegarder_config_bon_sortie", "parametres:modifier", |c, p| {
        let actif: bool = arg(&p, "actif", "actif")?;
        let v = parametres::sauvegarder_config_bon_sortie(c.conn, actif)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_config_suivi_livraison", |c, _p| {
        let v = parametres::lire_config_suivi_livraison(c.conn)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("sauvegarder_config_suivi_livraison", "parametres:modifier", |c, p| {
        let actif: bool = arg(&p, "actif", "actif")?;
        let v = parametres::sauvegarder_config_suivi_livraison(c.conn, actif)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_config_signatures", |c, _p| {
        let v = parametres::lire_config_signatures(c.conn)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("sauvegarder_config_signatures", "parametres:modifier", |c, p| {
        let valeurs: std::collections::HashMap<String, String> = arg(&p, "valeurs", "valeurs")?;
        let v = parametres::sauvegarder_config_signatures(c.conn, valeurs)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_stocks", |c, _p| {
        let v = parametres::lire_stocks(c.conn)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("diagnostiquer_base", |c, _p| {
        let v = parametres::diagnostiquer_base(c.conn)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_toutes_pieces_client", |c, p| {
        let type_filtre: Option<String> = arg(&p, "typeFiltre", "type_filtre")?;
        let statut: Option<String> = arg(&p, "statut", "statut")?;
        let recherche: Option<String> = arg(&p, "recherche", "recherche")?;
        let date_debut: Option<String> = arg(&p, "dateDebut", "date_debut")?;
        let date_fin: Option<String> = arg(&p, "dateFin", "date_fin")?;
        let montant_min: Option<i64> = arg(&p, "montantMin", "montant_min")?;
        let montant_max: Option<i64> = arg(&p, "montantMax", "montant_max")?;
        let impaye_seulement: Option<bool> = arg(&p, "impayeSeulement", "impaye_seulement")?;
        let en_retard_seulement: Option<bool> = arg(&p, "enRetardSeulement", "en_retard_seulement")?;
        let client_id: Option<String> = arg(&p, "clientId", "client_id")?;
        let v = pieces::lire_toutes_pieces_client(c.conn, type_filtre, statut, recherche, date_debut, date_fin, montant_min, montant_max, impaye_seulement, en_retard_seulement, client_id)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_pieces_client", |c, p| {
        let client_id: String = arg(&p, "clientId", "client_id")?;
        let type_filtre: Option<String> = arg(&p, "typeFiltre", "type_filtre")?;
        let v = pieces::lire_pieces_client(c.conn, client_id, type_filtre)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_lignes_piece", |c, p| {
        let piece_id: String = arg(&p, "pieceId", "piece_id")?;
        let v = pieces::lire_lignes_piece(c.conn, piece_id)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("creer_piece", "pieces:creer", |c, p| {
        let client_id: String = arg(&p, "clientId", "client_id")?;
        let type_piece: String = arg(&p, "typePiece", "type_piece")?;
        let lignes: Vec<pieces::LignePieceInput> = arg(&p, "lignes", "lignes")?;
        let remise_globale: Option<f64> = arg(&p, "remiseGlobale", "remise_globale")?;
        let date_echeance: Option<String> = arg(&p, "dateEcheance", "date_echeance")?;
        let note: Option<String> = arg(&p, "note", "note")?;
        let piece_origine_id: Option<String> = arg(&p, "pieceOrigineId", "piece_origine_id")?;
        let depot_id: Option<String> = arg(&p, "depotId", "depot_id")?;
        let v = pieces::creer_piece(c.conn, client_id, type_piece, lignes, remise_globale, date_echeance, note, piece_origine_id, depot_id)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("convertir_piece", "pieces:creer", |c, p| {
        let piece_id: String = arg(&p, "pieceId", "piece_id")?;
        let nouveau_type: String = arg(&p, "nouveauType", "nouveau_type")?;
        let v = pieces::convertir_piece(c.conn, piece_id, nouveau_type)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("convertir_commande_en_livraison_et_facture", "pieces:creer", |c, p| {
        let piece_id: String = arg(&p, "pieceId", "piece_id")?;
        let v = pieces::convertir_commande_en_livraison_et_facture(c.conn, piece_id)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("changer_statut_piece", "pieces:creer", |c, p| {
        let piece_id: String = arg(&p, "pieceId", "piece_id")?;
        let nouveau_statut: String = arg(&p, "nouveauStatut", "nouveau_statut")?;
        let v = pieces::changer_statut_piece(c.conn, piece_id, nouveau_statut)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_donnees_piece", |c, p| {
        let piece_id: String = arg(&p, "pieceId", "piece_id")?;
        let v = pieces::lire_donnees_piece(c.conn, piece_id)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_fiche_client", |c, p| {
        let client_id: String = arg(&p, "clientId", "client_id")?;
        let v = pieces::lire_fiche_client(c.conn, client_id)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_toutes_pieces_fournisseur", |c, p| {
        let type_filtre: Option<String> = arg(&p, "typeFiltre", "type_filtre")?;
        let statut: Option<String> = arg(&p, "statut", "statut")?;
        let recherche: Option<String> = arg(&p, "recherche", "recherche")?;
        let fournisseur_id: Option<String> = arg(&p, "fournisseurId", "fournisseur_id")?;
        let v = pieces::lire_toutes_pieces_fournisseur(c.conn, type_filtre, statut, recherche, fournisseur_id)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("creer_piece_fournisseur", "pieces:creer", |c, p| {
        let fournisseur_id: String = arg(&p, "fournisseurId", "fournisseur_id")?;
        let type_piece: String = arg(&p, "typePiece", "type_piece")?;
        let lignes: Vec<pieces::LignePieceInput> = arg(&p, "lignes", "lignes")?;
        let remise_globale: Option<f64> = arg(&p, "remiseGlobale", "remise_globale")?;
        let date_echeance: Option<String> = arg(&p, "dateEcheance", "date_echeance")?;
        let note: Option<String> = arg(&p, "note", "note")?;
        let piece_origine_id: Option<String> = arg(&p, "pieceOrigineId", "piece_origine_id")?;
        let v = pieces::creer_piece_fournisseur(c.conn, fournisseur_id, type_piece, lignes, remise_globale, date_echeance, note, piece_origine_id)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("modifier_piece", "pieces:creer", |c, p| {
        let piece_id: String = arg(&p, "pieceId", "piece_id")?;
        let note: Option<String> = arg(&p, "note", "note")?;
        let date_echeance: Option<String> = arg(&p, "dateEcheance", "date_echeance")?;
        let remise_globale: Option<f64> = arg(&p, "remiseGlobale", "remise_globale")?;
        let lignes: Option<Vec<pieces::LignePieceInput>> = arg(&p, "lignes", "lignes")?;
        let v = pieces::modifier_piece(c.conn, piece_id, note, date_echeance, remise_globale, lignes)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("annuler_piece", "pieces:creer", |c, p| {
        let piece_id: String = arg(&p, "pieceId", "piece_id")?;
        let motif: Option<String> = arg(&p, "motif", "motif")?;
        let v = pieces::annuler_piece(c.conn, piece_id, motif)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("dupliquer_piece", "pieces:creer", |c, p| {
        let piece_id: String = arg(&p, "pieceId", "piece_id")?;
        let v = pieces::dupliquer_piece(c.conn, piece_id)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_piece_de_vente", |c, p| {
        let vente_id: String = arg(&p, "venteId", "vente_id")?;
        let v = pieces::lire_piece_de_vente(c.conn, vente_id)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("annuler_facture_par_avoir", "pieces:creer", |c, p| {
        let piece_id: String = arg(&p, "pieceId", "piece_id")?;
        let mode_remboursement: Option<String> = arg(&p, "modeRemboursement", "mode_remboursement")?;
        let moyen: Option<String> = arg(&p, "moyen", "moyen")?;
        let motif: Option<String> = arg(&p, "motif", "motif")?;
        let v = pieces::annuler_facture_par_avoir(c.conn, piece_id, mode_remboursement, moyen, motif)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_vente_de_piece", |c, p| {
        let piece_id: String = arg(&p, "pieceId", "piece_id")?;
        let v = pieces::lire_vente_de_piece(c.conn, piece_id)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("modifier_facture_pos", "ventes:creer", |c, p| {
        let piece_id: String = arg(&p, "pieceId", "piece_id")?;
        let note: Option<String> = arg(&p, "note", "note")?;
        let date_echeance: Option<String> = arg(&p, "dateEcheance", "date_echeance")?;
        let v = pieces_pos::modifier_facture_pos(c.conn, piece_id, note, date_echeance)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("valider_facture_credit", "ventes:creer", |c, p| {
        let piece_id: String = arg(&p, "pieceId", "piece_id")?;
        let v = pieces_pos::valider_facture_credit(c.conn, piece_id)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_rapport_ca_mensuel", |c, p| {
        let nb_mois: Option<i64> = arg(&p, "nbMois", "nb_mois")?;
        let v = rapports::lire_rapport_ca_mensuel(c.conn, nb_mois)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_rapport_top_clients", |c, p| {
        let date_debut: String = arg(&p, "dateDebut", "date_debut")?;
        let date_fin: String = arg(&p, "dateFin", "date_fin")?;
        let limite: Option<i64> = arg(&p, "limite", "limite")?;
        let v = rapports::lire_rapport_top_clients(c.conn, date_debut, date_fin, limite)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_rapport_top_articles", |c, p| {
        let date_debut: String = arg(&p, "dateDebut", "date_debut")?;
        let date_fin: String = arg(&p, "dateFin", "date_fin")?;
        let limite: Option<i64> = arg(&p, "limite", "limite")?;
        let v = rapports::lire_rapport_top_articles(c.conn, date_debut, date_fin, limite)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_rapport_creances", |c, _p| {
        let v = rapports::lire_rapport_creances(c.conn)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_rapport_stock", |c, _p| {
        let v = rapports::lire_rapport_stock(c.conn)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_rapport_tva", |c, p| {
        let date_debut: String = arg(&p, "dateDebut", "date_debut")?;
        let date_fin: String = arg(&p, "dateFin", "date_fin")?;
        let v = rapports::lire_rapport_tva(c.conn, date_debut, date_fin)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_creances_relances", |c, p| {
        let en_retard_seulement: Option<bool> = arg(&p, "enRetardSeulement", "en_retard_seulement")?;
        let v = relances::lire_creances_relances(c.conn, en_retard_seulement)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("enregistrer_relance", "creances:gerer", |c, p| {
        let vente_id: String = arg(&p, "venteId", "vente_id")?;
        let canal: String = arg(&p, "canal", "canal")?;
        let note: Option<String> = arg(&p, "note", "note")?;
        let v = relances::enregistrer_relance(c.conn, vente_id, canal, note)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_historique_relances", |c, p| {
        let vente_id: String = arg(&p, "venteId", "vente_id")?;
        let v = relances::lire_historique_relances(c.conn, vente_id)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_stats_relances", |c, _p| {
        let v = relances::lire_stats_relances(c.conn)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_ventes_recentes", |c, _p| {
        let v = retours::lire_ventes_recentes(c.conn)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("enregistrer_retour", "retours:creer", |c, p| {
        let vente_id: String = arg(&p, "venteId", "vente_id")?;
        let ligne_vente_id: String = arg(&p, "ligneVenteId", "ligne_vente_id")?;
        let quantite: f64 = arg(&p, "quantite", "quantite")?;
        let mode_resolution: String = arg(&p, "modeResolution", "mode_resolution")?;
        let mode_encaissement: Option<String> = arg(&p, "modeEncaissement", "mode_encaissement")?;
        let article_remplacement_id: Option<String> = arg(&p, "articleRemplacementId", "article_remplacement_id")?;
        let unite_remplacement_id: Option<String> = arg(&p, "uniteRemplacementId", "unite_remplacement_id")?;
        let quantite_remplacement: Option<f64> = arg(&p, "quantiteRemplacement", "quantite_remplacement")?;
        let mode_reliquat_positif: Option<String> = arg(&p, "modeReliquatPositif", "mode_reliquat_positif")?;
        let mode_encaissement_reliquat: Option<String> = arg(&p, "modeEncaissementReliquat", "mode_encaissement_reliquat")?;
        let v = retours::enregistrer_retour(c.conn, vente_id, ligne_vente_id, quantite, mode_resolution, mode_encaissement, article_remplacement_id, unite_remplacement_id, quantite_remplacement, mode_reliquat_positif, mode_encaissement_reliquat)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_avoirs_ouverts_tous", |c, _p| {
        let v = retours::lire_avoirs_ouverts_tous(c.conn)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("sauvegarder_base", "sauvegarde:lancer", |c, p| {
        let dossier_destination: String = arg(&p, "dossierDestination", "dossier_destination")?;
        let v = sauvegarde::sauvegarder_base(c.conn, dossier_destination)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_config_sauvegarde", |c, _p| {
        let v = sauvegarde::lire_config_sauvegarde(c.conn)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("sauvegarde_auto_si_necessaire", "sauvegarde:lancer", |c, _p| {
        let v = sauvegarde::sauvegarde_auto_si_necessaire(c.conn)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("sauvegarder_config_sauvegarde", "sauvegarde:lancer", |c, p| {
        let dossier_sauvegarde: Option<String> = arg(&p, "dossierSauvegarde", "dossier_sauvegarde")?;
        let sauvegarde_auto: bool = arg(&p, "sauvegardeAuto", "sauvegarde_auto")?;
        let v = sauvegarde::sauvegarder_config_sauvegarde(c.conn, dossier_sauvegarde, sauvegarde_auto)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_parametres_societe", |c, _p| {
        let v = societe::lire_parametres_societe(c.conn)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("sauvegarder_parametres_societe", "parametres:modifier", |c, p| {
        let nom: String = arg(&p, "nom", "nom")?;
        let adresse: Option<String> = arg(&p, "adresse", "adresse")?;
        let telephone: Option<String> = arg(&p, "telephone", "telephone")?;
        let telephone2: Option<String> = arg(&p, "telephone2", "telephone2")?;
        let email: Option<String> = arg(&p, "email", "email")?;
        let nif: Option<String> = arg(&p, "nif", "nif")?;
        let rccm: Option<String> = arg(&p, "rccm", "rccm")?;
        let site_web: Option<String> = arg(&p, "siteWeb", "site_web")?;
        let pied_facture: Option<String> = arg(&p, "piedFacture", "pied_facture")?;
        let v = societe::sauvegarder_parametres_societe(c.conn, nom, adresse, telephone, telephone2, email, nif, rccm, site_web, pied_facture)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.ecriture("enregistrer_transfert", "stock:transferer", |c, p| {
        let depot_source: String = arg(&p, "depotSource", "depot_source")?;
        let depot_dest: String = arg(&p, "depotDest", "depot_dest")?;
        let lignes: Vec<transferts::LigneTransfert> = arg(&p, "lignes", "lignes")?;
        let motif: Option<String> = arg(&p, "motif", "motif")?;
        let v = transferts::enregistrer_transfert(c.conn, depot_source, depot_dest, lignes, motif, Some(c.appelant.role.clone()))?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_transferts", |c, p| {
        let limite: Option<i64> = arg(&p, "limite", "limite")?;
        let v = transferts::lire_transferts(c.conn, limite)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.lecture("lire_bon_transfert", |c, p| {
        let bon: String = arg(&p, "bon", "bon")?;
        let v = transferts::lire_bon_transfert(c.conn, bon)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });
    // <<< POIGNEES GENEREES >>>

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

/// Lit un parametre, quel que soit son type.
///
/// Le front envoie du camelCase (c'est ce que fait le pont Tauri), le
/// Rust attend du snake_case. On accepte les deux : un poste plus
/// ancien qui enverrait l'un ou l'autre continue de fonctionner.
///
/// `Option<T>` se deserialise depuis `null`, ce qui evite un cas
/// particulier par parametre facultatif.
fn arg<T: serde::de::DeserializeOwned>(
    p: &Value,
    camel: &str,
    snake: &str,
) -> Result<T, String> {
    let v = p.get(camel).or_else(|| p.get(snake)).cloned().unwrap_or(Value::Null);
    serde_json::from_value(v)
        .map_err(|e| format!("Paramètre « {camel} » illisible : {e}"))
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

/// Le dossier ou l'application depose ses images, deduit du fichier de
/// la base.
///
/// C'est le repli de `images::lire_base64` : une image posee a cote de
/// la base sans que son chemin ait ete enregistre. Le noyau ne connait
/// pas Tauri et ne peut pas demander `app_data_dir` ; cote serveur il
/// n'y a de toute facon pas d'application.
fn dossier_des_images(conn: &rusqlite::Connection) -> Option<std::path::PathBuf> {
    let chemin = conn.path()?;
    std::path::Path::new(chemin)
        .parent()
        .map(|d| d.to_path_buf())
}
