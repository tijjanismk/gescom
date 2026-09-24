//! Les commandes du serveur : un nom, une permission, une poignee sur
//! `Base`.
//!
//! Depuis la v3 (D-2, D22), chaque commande n'a plus qu'une poignee,
//! sur `Base`, servie de la meme facon sur SQLite et PostgreSQL. Les
//! poignees `Connection` (D11) sont parties avec le chemin du meme nom ;
//! les fonctions `fn(conn)` du noyau restent pour la fenetre monoposte.

use serde_json::{json, Value};

use gescom_noyau::registre::Registre;
use gescom_noyau::{
    achats, argent, auth, avoirs, caisse, caisses, catalogue, catalogue_csv, chantiers, cheques, codebarre, comptoir, creances, depots, fournisseurs, journal, livraisons, pagination, parametres, pieces, pieces_pos, postes, rapports, relances, retours, sauvegarde, sessions, societe, tableau_bord, transferts,
};

pub fn registre() -> Registre {
    let mut r = Registre::nouveau();

    // ---- v3 : dossiers et exercices. ----
    r.sur_base("lire_dossiers", None, false, |c, _| {
        serde_json::to_value(gescom_noyau::dossiers::lire_dossiers_sur(c.base)?).map_err(|e| e.to_string())
    });
    r.sur_base("creer_dossier", Some("dossiers:gerer"), true, |c, p| {
        gescom_noyau::dossiers::creer_dossier_sur(
            c.base,
            arg(&p, "code", "code")?,
            arg(&p, "societe", "societe")?,
            // D21 : les dates de travail ; absentes, l'annee civile.
            arg(&p, "dateDebut", "date_debut")?,
            arg(&p, "dateFin", "date_fin")?,
        )
    });
    // Choisir le dossier d'une session ouverte sans : une fois, pas
    // plus. `memoriser` retient le choix pour cette personne.
    r.sur_base("choisir_dossier", None, false, |c, p| {
        let dossier_id: String = arg(&p, "dossierId", "dossier_id")?;
        let memoriser: Option<bool> = arg(&p, "memoriser", "memoriser")?;
        let d = gescom_noyau::dossiers::dossier_ouvert_sur(c.base, &dossier_id)?;
        // C-2 : le role qu'on a DANS ce dossier, ou pas d'entree.
        let role = gescom_noyau::acces_dossiers::role_dans_sur(c.base, &c.appelant.utilisateur_id, &d.id)?
            .ok_or_else(|| "Ce dossier ne vous est pas ouvert.".to_string())?;
        // D22 : servi par `Base` sur les deux moteurs (D-2) — plus de
        // refus sur une base fichier.
        sessions::choisir_dossier_session_sur(c.base, &c.appelant.session_id, &d.id)?;
        if memoriser == Some(true) {
            gescom_noyau::dossiers::memoriser_dossier_sur(c.base, &c.appelant.utilisateur_id, Some(&d.id))?;
        }
        let mut permissions: Vec<String> =
            gescom_noyau::portes::permissions_de_sur(c.base, &c.appelant.utilisateur_id, &role).into_iter().collect();
        permissions.sort();
        Ok(serde_json::json!({ "dossier_id": d.id, "societe": d.societe, "role": role, "permissions": permissions }))
    });
    r.sur_base("oublier_dossier_memorise", None, false, |c, _| {
        gescom_noyau::dossiers::memoriser_dossier_sur(c.base, &c.appelant.utilisateur_id, None)?;
        Ok(serde_json::Value::Null)
    });
    r.sur_base("lire_exercices", None, false, |c, _| {
        serde_json::to_value(gescom_noyau::dossiers::lire_exercices_sur(c.base)?).map_err(|e| e.to_string())
    });
    r.sur_base("ouvrir_exercice", Some("dossiers:gerer"), true, |c, p| {
        gescom_noyau::dossiers::ouvrir_exercice_sur(
            c.base,
            arg(&p, "dateDebut", "date_debut")?,
            arg(&p, "dateFin", "date_fin")?,
        )
    });
    r.sur_base("prolonger_exercice", Some("dossiers:gerer"), true, |c, p| {
        gescom_noyau::dossiers::prolonger_exercice_sur(
            c.base,
            arg(&p, "exerciceId", "exercice_id")?,
            arg(&p, "prolongeJusquAu", "prolonge_jusqu_au")?,
        )?;
        Ok(serde_json::Value::Null)
    });
    // v3, E-1 (D23) : le plan SYSCOHADA, commun, et les sous-comptes du
    // dossier. Le plan se lit par tous (c'est un vocabulaire).
    r.sur_base("lire_plan_comptable", None, false, |c, _| {
        serde_json::to_value(gescom_noyau::plan_comptable::lire_sur(c.base)?).map_err(|e| e.to_string())
    });
    r.sur_base("ajouter_sous_compte", Some("comptabilite:gerer"), true, |c, p| {
        gescom_noyau::plan_comptable::ajouter_sous_compte_sur(c.base, arg(&p, "numero", "numero")?, arg(&p, "libelle", "libelle")?)
    });
    // v3, E-2 : quelle operation va sur quel compte, par dossier.
    r.sur_base("lire_affectations", Some("comptabilite:gerer"), false, |c, _| {
        serde_json::to_value(gescom_noyau::affectations::lire_sur(c.base)?).map_err(|e| e.to_string())
    });
    r.sur_base("definir_affectation", Some("comptabilite:gerer"), true, |c, p| {
        gescom_noyau::affectations::definir_sur(c.base, arg(&p, "operation", "operation")?, arg(&p, "compte", "compte")?)
    });
    // Gescom Equipe, F-2 (D30) : les fiches du personnel. La lecture se
    // regle dans `coeur::lecture` (qui gere ou paie ; les montants,
    // la paie seulement).
    r.sur_base("lire_personnel", None, false, |c, p| {
        let avec_partis: Option<bool> = arg(&p, "avecPartis", "avec_partis")?;
        serde_json::to_value(gescom_noyau::personnel::lister_sur(c.base, avec_partis.unwrap_or(false))?)
            .map_err(|e| e.to_string())
    });
    r.sur_base("lire_employe", None, false, |c, p| {
        gescom_noyau::personnel::lire_sur(c.base, &arg::<String>(&p, "employeId", "employe_id")?)
    });
    r.sur_base("creer_employe", Some("personnel:gerer"), true, |c, p| {
        gescom_noyau::personnel::creer_sur(c.base, arg(&p, "fiche", "fiche")?)
    });
    r.sur_base("modifier_employe", Some("personnel:gerer"), true, |c, p| {
        gescom_noyau::personnel::modifier_sur(c.base, arg(&p, "employeId", "employe_id")?, arg(&p, "fiche", "fiche")?)
    });
    r.sur_base("faire_partir_employe", Some("personnel:gerer"), true, |c, p| {
        gescom_noyau::personnel::faire_partir_sur(
            c.base,
            arg(&p, "employeId", "employe_id")?,
            arg(&p, "date", "date")?,
            arg(&p, "motif", "motif")?,
        )
    });
    r.sur_base("faire_revenir_employe", Some("personnel:gerer"), true, |c, p| {
        gescom_noyau::personnel::faire_revenir_sur(c.base, arg(&p, "employeId", "employe_id")?)
    });
    // F-3 : les jours travailles.
    r.sur_base("lire_presences_mois", None, false, |c, p| {
        gescom_noyau::presences::lire_mois_sur(c.base, &arg::<String>(&p, "mois", "mois")?)
    });
    r.sur_base("marquer_presence", Some("personnel:gerer"), true, |c, p| {
        gescom_noyau::presences::marquer_sur(
            c.base,
            arg(&p, "employeId", "employe_id")?,
            arg(&p, "jour", "jour")?,
            arg(&p, "etat", "etat")?,
        )
    });
    r.sur_base("marquer_tous_presents", Some("personnel:gerer"), true, |c, p| {
        gescom_noyau::presences::tous_presents_sur(c.base, arg(&p, "jour", "jour")?)
    });
    // v3, E-3 : les journaux, fabriques a la lecture, et leur export.
    // Ce sont les chiffres de la boutique : `rapports:lire`.
    r.sur_base("lire_journaux_comptables", Some("rapports:lire"), false, |c, p| {
        gescom_noyau::journaux_comptables::lire_sur(
            c.base,
            arg(&p, "du", "du")?,
            arg(&p, "au", "au")?,
            arg(&p, "journal", "journal")?,
        )
    });
    r.sur_base("exporter_journaux_csv", Some("rapports:lire"), false, |c, p| {
        gescom_noyau::journaux_comptables::csv_sur(
            c.base,
            arg(&p, "du", "du")?,
            arg(&p, "au", "au")?,
            arg(&p, "journal", "journal")?,
        )
    });
    // v3, C-2 : les dossiers d'une personne, et son role dans chacun.
    r.sur_base("lire_dossiers_utilisateur", Some("utilisateurs:gerer"), false, |c, p| {
        gescom_noyau::acces_dossiers::lire_sur(c.base, &arg::<String>(&p, "utilisateurId", "utilisateur_id")?)
    });
    r.sur_base("definir_dossiers_utilisateur", Some("utilisateurs:gerer"), true, |c, p| {
        #[derive(serde::Deserialize)]
        struct Ligne {
            dossier_id: String,
            role: String,
        }
        let utilisateur: String = arg(&p, "utilisateurId", "utilisateur_id")?;
        let lignes: Option<Vec<Ligne>> = arg(&p, "dossiers", "dossiers")?;
        gescom_noyau::acces_dossiers::definir_sur(
            c.base,
            &utilisateur,
            lignes.map(|v| v.into_iter().map(|l| (l.dossier_id, l.role)).collect()),
        )
    });
    // v3, D-5 : le dossier d'origine prend le nom que le patron lui donne.
    r.sur_base("renommer_dossier", Some("dossiers:gerer"), true, |c, p| {
        let dossier: String = arg(&p, "dossierId", "dossier_id")?;
        // C-2 : on ne renomme que les dossiers ou l'on entre.
        if gescom_noyau::acces_dossiers::role_dans_sur(c.base, &c.appelant.utilisateur_id, &dossier)?.is_none() {
            return Err("Ce dossier ne vous est pas ouvert.".to_string());
        }
        gescom_noyau::dossiers::renommer_dossier_sur(c.base, dossier, arg(&p, "societe", "societe")?)
    });
    r.sur_base("clore_exercice", Some("dossiers:gerer"), true, |c, p| {
        gescom_noyau::dossiers::clore_exercice_sur(c.base, arg(&p, "exerciceId", "exercice_id")?)?;
        Ok(serde_json::Value::Null)
    });

    // ---- v3, A-1 : les reglages des documents imprimes (D17, D18).
    // Lus par toute caisse avant d'imprimer ; regles par qui modifie les
    // parametres. ----
    r.sur_base("lire_reglages_documents", None, false, |c, _| {
        gescom_noyau::documents::lire_reglages_sur(c.base)
    });
    r.sur_base("enregistrer_reglage_document", Some("parametres:modifier"), true, |c, p| {
        let genre: String = arg(&p, "genre", "genre")?;
        let reglage: Value = arg(&p, "reglage", "reglage")?;
        gescom_noyau::documents::enregistrer_reglage_sur(c.base, &genre, reglage)
    });
    r.sur_base("retablir_reglage_document", Some("parametres:modifier"), true, |c, p| {
        let genre: String = arg(&p, "genre", "genre")?;
        gescom_noyau::documents::retablir_defaut_sur(c.base, &genre)
    });
    // ---- v3, B-1 : l'Historique. La premiere lecture filtree (C1). ----
    r.sur_base("lire_historique", Some("journal:lire"), false, |c, p| {
        let brut = p.get("filtre").cloned().unwrap_or(p);
        let filtre: gescom_noyau::historique::Filtre = serde_json::from_value(brut)
            .map_err(|e| format!("Filtre illisible : {e}"))?;
        gescom_noyau::historique::lire_historique_sur(c.base, filtre)
    });
    r.sur_base("lire_filtres_historique", Some("journal:lire"), false, |c, _| {
        gescom_noyau::historique::filtres_sur(c.base)
    });
    // B-4 : le compteur rouge du tableau de bord, et « vu » par qui.
    r.sur_base("lire_anomalies_a_verifier", Some("journal:lire"), false, |c, _| {
        Ok(serde_json::json!({ "nombre": gescom_noyau::historique::anomalies_a_verifier_sur(c.base)? }))
    });
    r.sur_base("marquer_anomalie_vue", Some("journal:lire"), true, |c, p| {
        let id: String = arg(&p, "journalId", "journal_id")?;
        gescom_noyau::historique::marquer_anomalie_vue_sur(c.base, &id)
    });

    r.sur_base("enregistrer_coordonnees_documents", Some("parametres:modifier"), true, |c, p| {
        let choix: Vec<String> = arg(&p, "coordonnees", "coordonnees")?;
        serde_json::to_value(gescom_noyau::documents::enregistrer_coordonnees_sur(c.base, choix)?)
            .map_err(|e| e.to_string())
    });
    r.sur_base("poser_image_signature", Some("parametres:modifier"), true, |c, p| {
        let genre: String = arg(&p, "genre", "genre")?;
        let rang: usize = arg(&p, "rang", "rang")?;
        let image: Option<String> = arg(&p, "image", "image")?;
        gescom_noyau::documents::poser_image_signature_sur(c.base, &genre, rang, image)
    });

    r.sur_base("lire_stock_multi_depots", None, false, |c, _| {
        serde_json::to_value(depots::lire_stock_multi_depots_sur_base(c.base)?)
            .map_err(|e| e.to_string())
    });

    r.sur_base("lire_config_scanner", None, false, |c, _| {
        Ok(Value::Bool(comptoir::lire_config_scanner_sur(c.base)?))
    });

    r.sur_base("creer_client_rapide", Some("clients:creer"), true, |c, p| {
        comptoir::creer_client_rapide_sur(c.base, texte(&p, "nom")?, option_texte(&p, "telephone"))
    });

    r.sur_base("creer_article_rapide", Some("articles:creer"), true, |c, p| {
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





    // Les memes cinq, sur `Base` — le dernier morceau qui manquait a
    // l'ecran POS sur PostgreSQL : sans elles, les widgets restaient
    // vides.
    r.sur_base("lire_resume_dashboard", None, false, |c, p| {
        tableau_bord::lire_resume_dashboard_sur(c.base, option_texte(&p, "depotId"))
    });
    r.sur_base("lire_ventes_periode", None, false, |c, p| {
        tableau_bord::lire_ventes_periode_sur(
            c.base,
            option_texte(&p, "periode"),
            option_texte(&p, "depotId"),
        )
    });
    r.sur_base("lire_top_clients", None, false, |c, _| {
        let v = tableau_bord::lire_top_clients_sur(c.base)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });
    r.sur_base("lire_top_articles", None, false, |c, _| {
        let v = tableau_bord::lire_top_articles_sur(c.base)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });
    r.sur_base("lire_ventes_a_decouvert", None, false, |c, p| {
        tableau_bord::lire_ventes_a_decouvert_sur(
            c.base,
            option_texte(&p, "dateDebut").or_else(|| option_texte(&p, "date_debut")),
            option_texte(&p, "dateFin").or_else(|| option_texte(&p, "date_fin")),
        )
    });

    // ---- Les deux reglements ----
    //
    // Encaisser une creance et payer un fournisseur : sans eux, une
    // caisse vend mais ne peut pas recevoir l'argent du lendemain.
    r.sur_base("enregistrer_paiement", Some("paiements:creer"), true, |c, p| {
        argent::enregistrer_paiement_sur_base(
            c.base,
            texte(&p, "venteId").or_else(|_| texte(&p, "vente_id"))?,
            entier(&p, "montant").unwrap_or(0),
            texte(&p, "mode")?,
            Some(c.appelant.role.clone()),
        )?;
        Ok(json!({ "etat": "enregistre" }))
    });

    r.sur_base("regler_dette_fournisseur", Some("fournisseurs:regler"), true, |c, p| {
        let date = option_texte(&p, "datePaiement").or_else(|| option_texte(&p, "date_paiement"));
        exiger_antidatage_base(c.base, c.appelant, date.as_deref())?;
        argent::regler_dette_fournisseur_datee_sur_base(
            c.base,
            texte(&p, "fournisseurId").or_else(|_| texte(&p, "fournisseur_id"))?,
            entier(&p, "montant").unwrap_or(0),
            texte(&p, "mode")?,
            option_texte(&p, "note"),
            option_texte(&p, "pieceId").or_else(|| option_texte(&p, "piece_id")),
            date,
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
    // `creer_vente_sur_base`, testee dans argent_base.rs.
    r.sur_base("creer_vente", Some("ventes:creer"), true, |c, p| {
        let lignes: Vec<argent::ParamsLigneInput> = serde_json::from_value(
            p.get("lignes").cloned().unwrap_or(Value::Array(vec![])),
        )
        .map_err(|e| format!("Lignes de vente illisibles : {e}"))?;

        let date_vente = option_texte(&p, "dateVente").or_else(|| option_texte(&p, "date_vente"));
        exiger_antidatage_base(c.base, c.appelant, date_vente.as_deref())?;
        exiger_plafonds_vente(&gescom_noyau::plafonds::de_sur(c.base, &c.appelant.utilisateur_id), &p, &lignes)?;

        argent::creer_vente_datee_sur_base(
            c.base,
            texte(&p, "clientId").or_else(|_| texte(&p, "client_id"))?,
            texte(&p, "depotId").or_else(|_| texte(&p, "depot_id"))?,
            texte(&p, "modeReglement").or_else(|_| texte(&p, "mode_reglement"))?,
            lignes,
            Some(c.appelant.role.clone()),
            entier(&p, "montantPaye").or_else(|| entier(&p, "montant_paye")),
            option_texte(&p, "modePaiement").or_else(|| option_texte(&p, "mode_paiement")),
            entier(&p, "avoirMontant").or_else(|| entier(&p, "avoir_montant")),
            date_vente,
        )
    });

    // La facture automatique du point de vente. Son echec ne bloque pas
    // le caissier devant son client — mais sans elle, rien a imprimer.
    r.sur_base("creer_facture_depuis_vente", Some("pieces:creer"), true, |c, p| {
        argent::creer_facture_depuis_vente_sur_base(
            c.base,
            texte(&p, "venteId").or_else(|_| texte(&p, "vente_id"))?,
            texte(&p, "clientId").or_else(|_| texte(&p, "client_id"))?,
            texte(&p, "modeReglement").or_else(|_| texte(&p, "mode_reglement"))?,
            Some(c.appelant.role.clone()),
        )
    });

    r.sur_base("valider_facture", Some("pieces:creer"), true, |c, p| {
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
    r.sur_base("lire_clients", None, false, |c, _| {
        serde_json::to_value(catalogue::lire_clients_sur(c.base)?).map_err(|e| e.to_string())
    });

    r.sur_base("lire_client_generique", None, false, |c, _| {
        catalogue::lire_client_generique_sur(c.base)
    });

    r.sur_base("lire_depots", None, false, |c, _| {
        serde_json::to_value(catalogue::lire_depots_sur(c.base)?).map_err(|e| e.to_string())
    });

    r.sur_base("lire_depot_defaut", None, false, |c, _| {
        catalogue::lire_depot_defaut_sur(c.base)
    });

    r.sur_base("lire_articles_avec_unites", None, false, |c, p| {
        let depot = p.get("depotId")
            .or_else(|| p.get("depot_id"))
            .and_then(Value::as_str)
            .map(str::to_string);
        let v = catalogue::lire_articles_avec_unites_sur(c.base, true, depot)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_postes", None, false, |c, _| {
        serde_json::to_value(postes::lister_sur_base(c.base)?).map_err(|e| e.to_string())
    });

    r.sur_base("desactiver_poste", Some("postes:gerer"), true, |c, p| {
        let id = texte(&p, "poste_id")?;
        if id == c.appelant.poste_id {
            return Err("Un poste ne peut pas se désactiver lui-même.".to_string());
        }
        postes::desactiver_sur_base(c.base, &id, &c.appelant.utilisateur_id)?;
        Ok(json!({ "poste_id": id }))
    });

    r.sur_base("reactiver_poste", Some("postes:gerer"), true, |c, p| {
        let id = texte(&p, "poste_id")?;
        postes::reactiver_sur_base(c.base, &id)?;
        Ok(json!({ "poste_id": id }))
    });

    r.sur_base("lire_sessions_reseau", None, false, |c, _| {
        let v = sessions::lister_actives_sur(c.base).map_err(|e| e.to_string())?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("revoquer_session_reseau", Some("postes:gerer"), true, |c, p| {
        let id = texte(&p, "session_id")?;
        let n = sessions::revoquer_sur(c.base, &id, &c.appelant.utilisateur_id)
            .map_err(|e| e.to_string())?;
        Ok(json!({ "revoquees": n }))
    });

    r.sur_base("lire_mode_caisse", None, false, |c, _| {
        Ok(json!({ "par_utilisateur": caisses::par_utilisateur_sur(c.base) }))
    });

    r.sur_base("definir_mode_caisse", Some("caisse:configurer"), true, |c, p| {
        let actif = p.get("par_utilisateur").and_then(Value::as_bool).ok_or(
            "Paramètre « par_utilisateur » manquant (true ou false).".to_string(),
        )?;
        caisses::definir_par_utilisateur_sur(c.base, actif)?;
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





    // ---- Les memes six, sur `Base` ----
    r.sur_base("enregistrer_achat", Some("achats:creer"), true, |c, p| {
        let date_reception = option_texte(&p, "dateReception").or_else(|| option_texte(&p, "date_reception"));
        exiger_antidatage_base(c.base, &c.appelant, date_reception.as_deref())?;
        let v = achats::enregistrer_achat_date_sur_base(
            c.base,
            arg(&p, "fournisseurId", "fournisseur_id")?,
            arg(&p, "depotId", "depot_id")?,
            arg(&p, "lignes", "lignes")?,
            arg(&p, "modeReglement", "mode_reglement")?,
            arg(&p, "modePaiement", "mode_paiement")?,
            arg(&p, "acompte", "acompte")?,
            arg(&p, "note", "note")?,
            Some(c.appelant.role.clone()),
            arg(&p, "pieceOrigineId", "piece_origine_id")?,
            date_reception,
        )?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });
    r.sur_base("enregistrer_retour_fournisseur", Some("achats:creer"), true, |c, p| {
        let v = achats::enregistrer_retour_fournisseur_sur_base(
            c.base,
            arg(&p, "fournisseurId", "fournisseur_id")?,
            arg(&p, "depotId", "depot_id")?,
            arg(&p, "lignes", "lignes")?,
            arg(&p, "pieceOrigineId", "piece_origine_id")?,
            arg(&p, "modeResolution", "mode_resolution")?,
            arg(&p, "modeEncaissement", "mode_encaissement")?,
            arg(&p, "motif", "motif")?,
            Some(c.appelant.role.clone()),
        )?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });
    r.sur_base("valider_facture_fournisseur", Some("achats:creer"), true, |c, p| {
        let v = achats::valider_facture_fournisseur_sur_base(
            c.base,
            arg(&p, "pieceId", "piece_id")?,
            arg(&p, "modeReglement", "mode_reglement")?,
            arg(&p, "modePaiement", "mode_paiement")?,
            arg(&p, "acompte", "acompte")?,
            Some(c.appelant.role.clone()),
        )?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });
    r.sur_base("annuler_facture_fournisseur_par_avoir", Some("achats:creer"), true, |c, p| {
        let v = achats::annuler_facture_fournisseur_par_avoir_sur_base(
            c.base,
            arg(&p, "pieceId", "piece_id")?,
            arg(&p, "modeResolution", "mode_resolution")?,
            arg(&p, "modeEncaissement", "mode_encaissement")?,
            arg(&p, "motif", "motif")?,
            Some(c.appelant.role.clone()),
        )?;
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
    r.sur_base("modifier_client", Some("clients:modifier"), true, |c, p| {
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

    r.sur_base("lire_clients_avec_creances", None, false, |c, _| {
        let v = gescom_noyau::comptoir::lire_clients_avec_creances_sur(c.base)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_logo_base64", None, false, |c, _| {
        // Le MEME dossier qu'a l'ecriture et a la suppression : sur
        // PostgreSQL, `sqlite()` ne rend rien et le repli « a cote du
        // fichier » laissait les images sur le disque introuvables
        // des que la colonne etait vide.
        let dossier = dossier_des_images_base(c.base);
        let v = gescom_noyau::images::lire_base64_sur_base(c.base, "logo", dossier.as_deref())?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_entete_base64", None, false, |c, _| {
        // Le MEME dossier qu'a l'ecriture et a la suppression : sur
        // PostgreSQL, `sqlite()` ne rend rien et le repli « a cote du
        // fichier » laissait les images sur le disque introuvables
        // des que la colonne etait vide.
        let dossier = dossier_des_images_base(c.base);
        let v = gescom_noyau::images::lire_base64_sur_base(c.base, "entete", dossier.as_deref())?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_pied_base64", None, false, |c, _| {
        // Le MEME dossier qu'a l'ecriture et a la suppression : sur
        // PostgreSQL, `sqlite()` ne rend rien et le repli « a cote du
        // fichier » laissait les images sur le disque introuvables
        // des que la colonne etait vide.
        let dossier = dossier_des_images_base(c.base);
        let v = gescom_noyau::images::lire_base64_sur_base(c.base, "pied", dossier.as_deref())?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    // L'ecriture (D8) : la caisse envoie les OCTETS en base64, jamais
    // un chemin — un chemin de caisse ne designe rien chez le serveur.
    // Celui-ci range le fichier dans SON dossier d'images et enregistre
    // le chemin ; les caisses le relisent par `lire_*_base64` ci-dessus.
    r.sur_base("sauvegarder_logo", Some("parametres:modifier"), true, |c, p| {
        let nom: String = arg(&p, "nom", "nom")?;
        let contenu: String = arg(&p, "contenu", "contenu")?;
        let octets = gescom_noyau::images::decoder_base64(&contenu)?;
        let dossier = dossier_des_images_base(c.base);
        gescom_noyau::images::ecrire_sur_base(c.base, "logo", &nom, &octets, dossier.as_deref())?;
        Ok(Value::Null)
    });

    r.sur_base("sauvegarder_entete", Some("parametres:modifier"), true, |c, p| {
        let nom: String = arg(&p, "nom", "nom")?;
        let contenu: String = arg(&p, "contenu", "contenu")?;
        let octets = gescom_noyau::images::decoder_base64(&contenu)?;
        let dossier = dossier_des_images_base(c.base);
        gescom_noyau::images::ecrire_sur_base(c.base, "entete", &nom, &octets, dossier.as_deref())?;
        Ok(Value::Null)
    });

    r.sur_base("sauvegarder_pied", Some("parametres:modifier"), true, |c, p| {
        let nom: String = arg(&p, "nom", "nom")?;
        let contenu: String = arg(&p, "contenu", "contenu")?;
        let octets = gescom_noyau::images::decoder_base64(&contenu)?;
        let dossier = dossier_des_images_base(c.base);
        gescom_noyau::images::ecrire_sur_base(c.base, "pied", &nom, &octets, dossier.as_deref())?;
        Ok(Value::Null)
    });

    r.sur_base("supprimer_logo", Some("parametres:modifier"), true, |c, _| {
        let dossier = dossier_des_images_base(c.base);
        gescom_noyau::images::supprimer_sur_base(c.base, "logo", dossier.as_deref())?;
        Ok(Value::Null)
    });

    r.sur_base("supprimer_entete", Some("parametres:modifier"), true, |c, _| {
        let dossier = dossier_des_images_base(c.base);
        gescom_noyau::images::supprimer_sur_base(c.base, "entete", dossier.as_deref())?;
        Ok(Value::Null)
    });

    r.sur_base("supprimer_pied", Some("parametres:modifier"), true, |c, _| {
        let dossier = dossier_des_images_base(c.base);
        gescom_noyau::images::supprimer_sur_base(c.base, "pied", dossier.as_deref())?;
        Ok(Value::Null)
    });

    r.sur_base("lire_factures_fournisseur_retournables", None, false, |c, p| {
        let v = achats::lire_factures_fournisseur_retournables_sur_base(
            c.base,
            arg(&p, "fournisseurId", "fournisseur_id")?,
        )?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("connexion", None, true, |c, p| {
        let identifiant: String = arg(&p, "identifiant", "identifiant")?;
        let mot_de_passe: String = arg(&p, "motDePasse", "mot_de_passe")?;
        let v = auth::connexion_sur(c.base, identifiant, mot_de_passe)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    // Sans elle, le changement de mot de passe OBLIGATOIRE a la
    // premiere connexion (amorcage.rs) bloquerait tout essai manuel sur
    // PostgreSQL des l'ecran suivant le login — la modale ne se ferme
    // pas tant que la commande n'a pas reussi.
    r.sur_base("changer_mot_de_passe", None, true, |c, p| {
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
    // Aucune base a lire — mais sans poignee `Base`, le registre refuse
    // sur PostgreSQL, et l'onglet Roles s'ouvre vide. Trouve par
    // outils/caisse_pg.py.
    r.sur_base("lire_catalogue_permissions", None, false, |_c, _p| {
        Ok(gescom_noyau::roles::lire_catalogue_permissions())
    });

    r.sur_base("lire_roles", None, false, |c, _p| {
        gescom_noyau::roles::lire_roles_sur_base(c.base)
    });

    r.sur_base("lire_permissions_utilisateur", None, false, |c, p| {
        let utilisateur_id: String = arg(&p, "utilisateurId", "utilisateur_id")?;
        gescom_noyau::roles::lire_permissions_utilisateur_sur_base(c.base, utilisateur_id)
    });

    r.sur_base("creer_role", Some("utilisateurs:gerer"), true, |c, p| {
        let nom: String = arg(&p, "nom", "nom")?;
        let description: Option<String> = arg(&p, "description", "description")?;
        let permissions: Vec<String> = arg(&p, "permissions", "permissions")?;
        gescom_noyau::roles::creer_role_sur_base(c.base, nom, description, permissions)
    });

    r.sur_base("modifier_role", Some("utilisateurs:gerer"), true, |c, p| {
        let role_id: String = arg(&p, "roleId", "role_id")?;
        let description: Option<String> = arg(&p, "description", "description")?;
        let permissions: Vec<String> = arg(&p, "permissions", "permissions")?;
        gescom_noyau::roles::modifier_role_sur_base(c.base, role_id, description, permissions)
    });

    r.sur_base("supprimer_role", Some("utilisateurs:gerer"), true, |c, p| {
        let role_id: String = arg(&p, "roleId", "role_id")?;
        gescom_noyau::roles::supprimer_role_sur_base(c.base, role_id)
    });

    r.sur_base("definir_permission_utilisateur", Some("utilisateurs:gerer"), true, |c, p| {
        let utilisateur_id: String = arg(&p, "utilisateurId", "utilisateur_id")?;
        let permission: String = arg(&p, "permission", "permission")?;
        let accorde: Option<bool> = arg(&p, "accorde", "accorde")?;
        let par = Some(c.appelant.utilisateur_id.clone());
        gescom_noyau::roles::definir_permission_utilisateur_sur_base(
            c.base, utilisateur_id, permission, accorde, par,
        )
    });

    r.sur_base("creer_utilisateur", Some("utilisateurs:gerer"), true, |c, p| {
        let nom: String = arg(&p, "nom", "nom")?;
        let pseudo: String = arg(&p, "pseudo", "pseudo")?;
        let email: Option<String> = arg(&p, "email", "email")?;
        let mot_de_passe: String = arg(&p, "motDePasse", "mot_de_passe")?;
        let role_nom: String = arg(&p, "roleNom", "role_nom")?;
        let v = auth::creer_utilisateur_sur_base(c.base, nom, pseudo, email, mot_de_passe, role_nom, c.appelant.utilisateur_id.clone())?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    // C-4 : desactiver un compte ferme ses sessions dans le meme geste.
    r.sur_base("activer_utilisateur", Some("utilisateurs:gerer"), true, |c, p| {
        let id: String = arg(&p, "utilisateurId", "utilisateur_id")?;
        let actif: bool = arg(&p, "actif", "actif")?;
        auth::activer_utilisateur_sur(c.base, &id, actif)
    });

    // C-3 : les plafonds, par role et par personne.
    r.sur_base("lire_plafonds", Some("utilisateurs:gerer"), false, |c, _| {
        gescom_noyau::plafonds::lire_sur(c.base)
    });
    r.sur_base("definir_plafonds_role", Some("utilisateurs:gerer"), true, |c, p| {
        let role: String = arg(&p, "role", "role")?;
        let plafonds: gescom_noyau::coeur::plafonds::Plafonds =
            serde_json::from_value(p.get("plafonds").cloned().unwrap_or(Value::Null))
                .map_err(|e| format!("Plafonds illisibles : {e}"))?;
        gescom_noyau::plafonds::definir_role_sur(c.base, &role, plafonds)
    });
    r.sur_base("definir_plafonds_utilisateur", Some("utilisateurs:gerer"), true, |c, p| {
        let id: String = arg(&p, "utilisateurId", "utilisateur_id")?;
        let plafonds: gescom_noyau::coeur::plafonds::Plafonds =
            serde_json::from_value(p.get("plafonds").cloned().unwrap_or(Value::Null))
                .map_err(|e| format!("Plafonds illisibles : {e}"))?;
        gescom_noyau::plafonds::definir_utilisateur_sur(c.base, &id, plafonds)
    });

    r.sur_base("lire_utilisateurs", None, false, |c, _p| {
        let v = auth::lire_utilisateurs_sur(c.base)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_avoirs_client", None, false, |c, p| {
        let client_id: String = arg(&p, "clientId", "client_id")?;
        let v = avoirs::lire_avoirs_client_sur_base(c.base, client_id)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("total_avoirs_client", None, false, |c, p| {
        let client_id: String = arg(&p, "clientId", "client_id")?;
        let v = avoirs::total_avoirs_client_sur_base(c.base, client_id)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("appliquer_avoir_vente", Some("avoirs:gerer"), true, |c, p| {
        let vente_id: String = arg(&p, "venteId", "vente_id")?;
        let client_id: String = arg(&p, "clientId", "client_id")?;
        let montant_demande: i64 = arg(&p, "montantDemande", "montant_demande")?;
        let v = avoirs::appliquer_avoir_vente_sur_base(c.base, vente_id, client_id, montant_demande)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("chercher_article_par_code_barre", None, false, |c, p| {
        let code_barre: String = arg(&p, "codeBarre", "code_barre")?;
        let v = avoirs::chercher_article_par_code_barre_sur_base(c.base, code_barre)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("sauvegarder_config_scanner", Some("avoirs:gerer"), true, |c, p| {
        let actif: bool = arg(&p, "actif", "actif")?;
        let v = avoirs::sauvegarder_config_scanner_sur_base(c.base, actif)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("sauvegarder_code_barre_article", Some("avoirs:gerer"), true, |c, p| {
        let article_id: String = arg(&p, "articleId", "article_id")?;
        let code_barre: String = arg(&p, "codeBarre", "code_barre")?;
        let v = avoirs::sauvegarder_code_barre_article_sur_base(c.base, article_id, code_barre)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_articles_avec_codes_barres", None, false, |c, _p| {
        let v = avoirs::lire_articles_avec_codes_barres_sur_base(c.base)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("rembourser_avoir", Some("avoirs:gerer"), true, |c, p| {
        let piece_id: String = arg(&p, "pieceId", "piece_id")?;
        let montant: i64 = arg(&p, "montant", "montant")?;
        let mode: String = arg(&p, "mode", "mode")?;
        gescom_noyau::coeur::plafonds::verifier_remboursement(
            montant, &gescom_noyau::plafonds::de_sur(c.base, &c.appelant.utilisateur_id))?;
        let v = avoirs::rembourser_avoir_sur_base(c.base, piece_id, montant, mode, Some(c.appelant.role.clone()))?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    // L'AVOIR ACCORDE — sans marchandise en face. Un credit qui sort de
    // nulle part est le geste le plus facile a detourner : la permission
    // n'entre dans aucun role livre, seul l'acces total la porte.
    r.sur_base("accorder_avoir_client", Some("avoirs:accorder"), true, |c, p| {
        let client_id: String = arg(&p, "clientId", "client_id")?;
        let montant: i64 = arg(&p, "montant", "montant")?;
        let motif: String = arg(&p, "motif", "motif")?;
        avoirs::accorder_avoir_client_sur_base(c.base, client_id, montant, motif, Some(c.appelant.role.clone()))
    });

    r.sur_base("lire_resume_caisse", None, false, |c, _p| {
        caisse::lire_resume_caisse_sur(c.base)
    });

    r.sur_base("lire_mouvements_caisse_du_jour", None, false, |c, _p| {
        let v = caisse::lire_mouvements_caisse_du_jour_sur_base(c.base)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("ouvrir_session_caisse", Some("caisse:mouvementer"), true, |c, p| {
        let fond_ouverture: i64 = arg(&p, "fondOuverture", "fond_ouverture")?;
        let v =
            caisse::ouvrir_session_caisse_sur(c.base, fond_ouverture, c.appelant.role.clone())?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("fermer_session_caisse", Some("caisse:mouvementer"), true, |c, p| {
        let session_id: String = arg(&p, "sessionId", "session_id")?;
        let especes_comptees: i64 = arg(&p, "especesComptees", "especes_comptees")?;
        let v = caisse::fermer_session_caisse_sur(c.base, session_id, especes_comptees)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("enregistrer_depense", Some("caisse:mouvementer"), true, |c, p| {
        let montant: i64 = arg(&p, "montant", "montant")?;
        let libelle: String = arg(&p, "libelle", "libelle")?;
        let categorie: Option<String> = arg(&p, "categorie", "categorie")?;
        let moyen: Option<String> = arg(&p, "moyen", "moyen")?;
        let v = caisse::enregistrer_depense_sur_base(c.base, montant, libelle, categorie, moyen, Some(c.appelant.role.clone()))?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_depenses_du_jour", None, false, |c, _p| {
        let v = caisse::lire_depenses_du_jour_sur_base(c.base)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_sessions_caisse", None, false, |c, p| {
        let limite: Option<i64> = arg(&p, "limite", "limite")?;
        let v = caisse::lire_sessions_caisse_sur_base(c.base, limite)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_mouvements_session", None, false, |c, p| {
        let session_id: String = arg(&p, "sessionId", "session_id")?;
        let v = caisse::lire_mouvements_session_sur_base(c.base, session_id)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_rapport_ecarts", None, false, |c, p| {
        let jours: Option<i64> = arg(&p, "jours", "jours")?;
        let v = caisse::lire_rapport_ecarts_sur_base(c.base, jours)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("modifier_depense", Some("caisse:mouvementer"), true, |c, p| {
        let mouvement_id: String = arg(&p, "mouvementId", "mouvement_id")?;
        let montant: Option<i64> = arg(&p, "montant", "montant")?;
        let libelle: Option<String> = arg(&p, "libelle", "libelle")?;
        let categorie: Option<String> = arg(&p, "categorie", "categorie")?;
        let v = caisse::modifier_depense_sur_base(c.base, mouvement_id, montant, libelle, categorie)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("exporter_articles_csv", None, false, |c, _p| {
        let v = catalogue_csv::exporter_articles_csv_sur_base(c.base)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("importer_articles_csv", Some("articles:creer"), true, |c, p| {
        let contenu: String = arg(&p, "contenu", "contenu")?;
        let mettre_a_jour: Option<bool> = arg(&p, "mettreAJour", "mettre_a_jour")?;
        let v = catalogue_csv::importer_articles_csv_sur_base(c.base, contenu, mettre_a_jour)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_etat_stock", None, false, |c, p| {
        let depot_id: Option<String> = arg(&p, "depotId", "depot_id")?;
        let avec_zero: Option<bool> = arg(&p, "avecZero", "avec_zero")?;
        let v = catalogue_csv::lire_etat_stock_sur_base(c.base, depot_id, avec_zero)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_taux_tva", None, false, |c, _p| {
        let v = chantiers::lire_taux_tva_sur_base(c.base)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("sauvegarder_tva_article", Some("chantiers:gerer"), true, |c, p| {
        let article_id: String = arg(&p, "articleId", "article_id")?;
        let taux_tva: f64 = arg(&p, "tauxTva", "taux_tva")?;
        let v = chantiers::sauvegarder_tva_article_sur_base(c.base, article_id, taux_tva)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_resume_tva", None, false, |c, p| {
        let date_debut: String = arg(&p, "dateDebut", "date_debut")?;
        let date_fin: String = arg(&p, "dateFin", "date_fin")?;
        let v = chantiers::lire_resume_tva_sur_base(c.base, date_debut, date_fin)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_dettes_fournisseurs", None, false, |c, _p| {
        let v = chantiers::lire_dettes_fournisseurs_sur_base(c.base)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("marquer_irrecouvrable", Some("chantiers:gerer"), true, |c, p| {
        let vente_id: String = arg(&p, "venteId", "vente_id")?;
        let motif: String = arg(&p, "motif", "motif")?;
        let v = chantiers::marquer_irrecouvrable_sur_base(c.base, vente_id, motif)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    // Le REGLEMENT EXCEPTIONNEL : l'argent d'une creance irrecouvrable
    // qui revient malgre tout. Meme droit que la mise en irrecouvrable —
    // c'est le meme geste de gestion, dans l'autre sens.
    r.sur_base("regler_creance_exceptionnel", Some("chantiers:gerer"), true, |c, p| {
        let vente_id: String = arg(&p, "venteId", "vente_id")?;
        let montant: i64 = arg(&p, "montant", "montant")?;
        let mode: String = arg(&p, "mode", "mode")?;
        let motif: Option<String> = arg(&p, "motif", "motif")?;
        creances::regler_creance_exceptionnel_sur_base(c.base, vente_id, montant, mode, motif, Some(c.appelant.role.clone()))
    });

    r.sur_base("lire_irrecouvrable", None, false, |c, _p| {
        let v = chantiers::lire_irrecouvrable_sur_base(c.base)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_config_avoirs", None, false, |c, _p| {
        let v = chantiers::lire_config_avoirs_sur_base(c.base)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("sauvegarder_config_avoirs", Some("chantiers:gerer"), true, |c, p| {
        let active: bool = arg(&p, "active", "active")?;
        let duree_jours: i64 = arg(&p, "dureeJours", "duree_jours")?;
        let v = chantiers::sauvegarder_config_avoirs_sur_base(c.base, active, duree_jours)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("expirer_avoirs", Some("chantiers:gerer"), true, |c, _p| {
        let v = chantiers::expirer_avoirs_sur_base(c.base)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_factures_fournisseur_ouvertes", None, false, |c, p| {
        let fournisseur_id: String = arg(&p, "fournisseurId", "fournisseur_id")?;
        let v = chantiers::lire_factures_fournisseur_ouvertes_sur_base(c.base, fournisseur_id)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("reactiver_avoir", Some("chantiers:gerer"), true, |c, p| {
        let avoir_id: String = arg(&p, "avoirId", "avoir_id")?;
        let v = chantiers::reactiver_avoir_sur_base(c.base, avoir_id, Some(c.appelant.role.clone()))?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_avoirs_expires", None, false, |c, _p| {
        let v = chantiers::lire_avoirs_expires_sur_base(c.base)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("enregistrer_cheque", Some("cheques:gerer"), true, |c, p| {
        let paiement_id: Option<String> = arg(&p, "paiementId", "paiement_id")?;
        let vente_id: Option<String> = arg(&p, "venteId", "vente_id")?;
        let numero: String = arg(&p, "numero", "numero")?;
        let banque: String = arg(&p, "banque", "banque")?;
        let tireur: Option<String> = arg(&p, "tireur", "tireur")?;
        let montant: i64 = arg(&p, "montant", "montant")?;
        let date_emission: Option<String> = arg(&p, "dateEmission", "date_emission")?;
        let date_echeance: Option<String> = arg(&p, "dateEcheance", "date_echeance")?;
        let v = cheques::enregistrer_cheque_sur_base(c.base, paiement_id, vente_id, numero, banque, tireur, montant, date_emission, date_echeance)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_cheques", None, false, |c, p| {
        let statut: Option<String> = arg(&p, "statut", "statut")?;
        let v = cheques::lire_cheques_sur_base(c.base, statut)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("changer_statut_cheque", Some("cheques:gerer"), true, |c, p| {
        let cheque_id: String = arg(&p, "chequeId", "cheque_id")?;
        let statut: String = arg(&p, "statut", "statut")?;
        let motif: Option<String> = arg(&p, "motif", "motif")?;
        let v = cheques::changer_statut_cheque_sur_base(c.base, cheque_id, statut, motif)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("generer_code_barre", Some("articles:creer"), true, |c, p| {
        let article_id: String = arg(&p, "articleId", "article_id")?;
        let v = codebarre::generer_code_barre_sur_base(c.base, article_id)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("generer_codes_barres_manquants", Some("articles:creer"), true, |c, _p| {
        let v = codebarre::generer_codes_barres_manquants_sur_base(c.base)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("definir_code_barre", Some("articles:creer"), true, |c, p| {
        let article_id: String = arg(&p, "articleId", "article_id")?;
        let code: String = arg(&p, "code", "code")?;
        let v = codebarre::definir_code_barre_sur_base(c.base, article_id, code)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_articles_codes_barres", None, false, |c, p| {
        let sans_code_seulement: Option<bool> = arg(&p, "sansCodeSeulement", "sans_code_seulement")?;
        let v = codebarre::lire_articles_codes_barres_sur_base(c.base, sans_code_seulement)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_etat_creances_client", None, false, |c, p| {
        let client_id: String = arg(&p, "clientId", "client_id")?;
        let v = creances::lire_etat_creances_client_sur_base(c.base, client_id)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_etat_creances_global", None, false, |c, _p| {
        let v = creances::lire_etat_creances_global_sur_base(c.base)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_reglements_client", None, false, |c, p| {
        let client_id: String = arg(&p, "clientId", "client_id")?;
        let v = creances::lire_reglements_client_sur_base(c.base, client_id)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("annuler_reglement", Some("creances:gerer"), true, |c, p| {
        let paiement_id: String = arg(&p, "paiementId", "paiement_id")?;
        let motif: String = arg(&p, "motif", "motif")?;
        let remboursement: bool = arg(&p, "remboursement", "remboursement")?;
        let v = creances::annuler_reglement_sur_base(c.base, paiement_id, motif, remboursement, Some(c.appelant.role.clone()))?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_donnees_recu", None, false, |c, p| {
        let paiement_id: String = arg(&p, "paiementId", "paiement_id")?;
        let cote: String = arg(&p, "cote", "cote")?;
        let v = creances::lire_donnees_recu_sur_base(c.base, paiement_id, cote)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_creances_ouvertes", None, false, |c, p| {
        let recherche: Option<String> = arg(&p, "recherche", "recherche")?;
        let v = creances::lire_creances_ouvertes_sur_base(c.base, recherche)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("regler_creance", Some("creances:gerer"), true, |c, p| {
        let vente_id: String = arg(&p, "venteId", "vente_id")?;
        let montant: i64 = arg(&p, "montant", "montant")?;
        let mode: String = arg(&p, "mode", "mode")?;
        let date: Option<String> = arg(&p, "datePaiement", "date_paiement")?;
        exiger_antidatage_base(c.base, c.appelant, date.as_deref())?;
        let v = creances::regler_creance_datee_sur_base(c.base, vente_id, montant, mode, Some(c.appelant.role.clone()), date)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("solder_residus_creances", Some("creances:gerer"), true, |c, p| {
        let simulation: Option<bool> = arg(&p, "simulation", "simulation")?;
        let v = creances::solder_residus_creances_sur_base(c.base, Some(c.appelant.role.clone()), simulation)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_depots_detail", None, false, |c, _p| {
        let v = depots::lire_depots_detail_sur_base(c.base)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("creer_depot", Some("depots:gerer"), true, |c, p| {
        let nom: String = arg(&p, "nom", "nom")?;
        let est_defaut: Option<bool> = arg(&p, "estDefaut", "est_defaut")?;
        let v = depots::creer_depot_sur_base(c.base, nom, est_defaut)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("renommer_depot", Some("depots:gerer"), true, |c, p| {
        let depot_id: String = arg(&p, "depotId", "depot_id")?;
        let nom: String = arg(&p, "nom", "nom")?;
        let v = depots::renommer_depot_sur_base(c.base, depot_id, nom)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("definir_depot_defaut", Some("depots:gerer"), true, |c, p| {
        let depot_id: String = arg(&p, "depotId", "depot_id")?;
        let v = depots::definir_depot_defaut_sur_base(c.base, depot_id)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("desactiver_depot", Some("depots:gerer"), true, |c, p| {
        let depot_id: String = arg(&p, "depotId", "depot_id")?;
        let force: Option<bool> = arg(&p, "force", "force")?;
        let v = depots::desactiver_depot_sur_base(c.base, depot_id, force)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("reactiver_depot", Some("depots:gerer"), true, |c, p| {
        let depot_id: String = arg(&p, "depotId", "depot_id")?;
        let v = depots::reactiver_depot_sur_base(c.base, depot_id)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_stock_depot", None, false, |c, p| {
        let depot_id: String = arg(&p, "depotId", "depot_id")?;
        let v = depots::lire_stock_depot_sur_base(c.base, depot_id)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_resume_par_depot", None, false, |c, p| {
        let date_debut: Option<String> = arg(&p, "dateDebut", "date_debut")?;
        let date_fin: Option<String> = arg(&p, "dateFin", "date_fin")?;
        let v = depots::lire_resume_par_depot_sur_base(c.base, date_debut, date_fin)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_stock_article_depots", None, false, |c, p| {
        let article_id: String = arg(&p, "articleId", "article_id")?;
        let v = depots::lire_stock_article_depots_sur_base(c.base, article_id)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_mouvements_stock", None, false, |c, p| {
        let article_id: Option<String> = arg(&p, "articleId", "article_id")?;
        let depot_id: Option<String> = arg(&p, "depotId", "depot_id")?;
        let type_mouvement: Option<String> = arg(&p, "typeMouvement", "type_mouvement")?;
        let date_debut: Option<String> = arg(&p, "dateDebut", "date_debut")?;
        let date_fin: Option<String> = arg(&p, "dateFin", "date_fin")?;
        let limite: Option<i64> = arg(&p, "limite", "limite")?;
        let v = depots::lire_mouvements_stock_sur_base(c.base, article_id, depot_id, type_mouvement, date_debut, date_fin, limite)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_fournisseurs", None, false, |c, _p| {
        let v = fournisseurs::lire_fournisseurs_sur_base(c.base)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_fournisseurs_avec_dettes", None, false, |c, _p| {
        let v = fournisseurs::lire_fournisseurs_avec_dettes_sur_base(c.base)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("creer_fournisseur", Some("fournisseurs:regler"), true, |c, p| {
        let nom: String = arg(&p, "nom", "nom")?;
        let telephone: Option<String> = arg(&p, "telephone", "telephone")?;
        let adresse: Option<String> = arg(&p, "adresse", "adresse")?;
        let nif: Option<String> = arg(&p, "nif", "nif")?;
        let email: Option<String> = arg(&p, "email", "email")?;
        let est_voisin: Option<bool> = arg(&p, "estVoisin", "est_voisin")?;
        let v = fournisseurs::creer_fournisseur_sur_base(c.base, nom, telephone, adresse, nif, email, est_voisin)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("modifier_fournisseur", Some("fournisseurs:regler"), true, |c, p| {
        let fournisseur_id: String = arg(&p, "fournisseurId", "fournisseur_id")?;
        let nom: String = arg(&p, "nom", "nom")?;
        let telephone: Option<String> = arg(&p, "telephone", "telephone")?;
        let adresse: Option<String> = arg(&p, "adresse", "adresse")?;
        let nif: Option<String> = arg(&p, "nif", "nif")?;
        let email: Option<String> = arg(&p, "email", "email")?;
        let est_voisin: Option<bool> = arg(&p, "estVoisin", "est_voisin")?;
        let v = fournisseurs::modifier_fournisseur_sur_base(c.base, fournisseur_id, nom, telephone, adresse, nif, email, est_voisin)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_etat_dette_fournisseur", None, false, |c, p| {
        let fournisseur_id: String = arg(&p, "fournisseurId", "fournisseur_id")?;
        let v = fournisseurs::lire_etat_dette_fournisseur_sur_base(c.base, fournisseur_id)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_etat_dettes_global", None, false, |c, _p| {
        let v = fournisseurs::lire_etat_dettes_global_sur_base(c.base)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("enregistrer_entree_stock", Some("fournisseurs:regler"), true, |c, p| {
        let article_id: String = arg(&p, "articleId", "article_id")?;
        let depot_id: Option<String> = arg(&p, "depotId", "depot_id")?;
        let quantite: f64 = arg(&p, "quantite", "quantite")?;
        let prix_achat: Option<i64> = arg(&p, "prixAchat", "prix_achat")?;
        let fournisseur_id: Option<String> = arg(&p, "fournisseurId", "fournisseur_id")?;
        let v = fournisseurs::enregistrer_entree_stock_sur_base(c.base, article_id, depot_id, quantite, prix_achat, fournisseur_id, Some(c.appelant.role.clone()))?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("enregistrer_retour_sans_facture", Some("fournisseurs:regler"), true, |c, p| {
        let article_id: String = arg(&p, "articleId", "article_id")?;
        let depot_id: Option<String> = arg(&p, "depotId", "depot_id")?;
        let quantite: f64 = arg(&p, "quantite", "quantite")?;
        let fournisseur_id: Option<String> = arg(&p, "fournisseurId", "fournisseur_id")?;
        let motif: Option<String> = arg(&p, "motif", "motif")?;
        let v = fournisseurs::enregistrer_retour_sans_facture_sur_base(c.base, article_id, depot_id, quantite, fournisseur_id, motif, Some(c.appelant.role.clone()))?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("enregistrer_ajustement_inventaire", Some("fournisseurs:regler"), true, |c, p| {
        let article_id: String = arg(&p, "articleId", "article_id")?;
        let depot_id: String = arg(&p, "depotId", "depot_id")?;
        let quantite_reelle: f64 = arg(&p, "quantiteReelle", "quantite_reelle")?;
        let motif: Option<String> = arg(&p, "motif", "motif")?;
        let v = fournisseurs::enregistrer_ajustement_inventaire_sur_base(c.base, article_id, depot_id, quantite_reelle, motif, Some(c.appelant.role.clone()))?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_fournisseur_detail", None, false, |c, p| {
        let fournisseur_id: String = arg(&p, "fournisseurId", "fournisseur_id")?;
        let v = fournisseurs::lire_fournisseur_detail_sur_base(c.base, fournisseur_id)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_fiche_fournisseur", None, false, |c, p| {
        let fournisseur_id: String = arg(&p, "fournisseurId", "fournisseur_id")?;
        let v = fournisseurs::lire_fiche_fournisseur_sur_base(c.base, fournisseur_id)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("annuler_paiement_fournisseur", Some("fournisseurs:regler"), true, |c, p| {
        let paiement_id: String = arg(&p, "paiementId", "paiement_id")?;
        let motif: String = arg(&p, "motif", "motif")?;
        let remboursement: bool = arg(&p, "remboursement", "remboursement")?;
        let v = fournisseurs::annuler_paiement_fournisseur_sur_base(c.base, paiement_id, motif, remboursement, Some(c.appelant.role.clone()))?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_journal_du_jour", None, false, |c, p| {
        let date: Option<String> = arg(&p, "date", "date")?;
        let depot_id: Option<String> = arg(&p, "depotId", "depot_id")?;
        let v = journal::lire_journal_du_jour_sur_base(c.base, date, depot_id)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_livraison_piece", None, false, |c, p| {
        let piece_id: String = arg(&p, "pieceId", "piece_id")?;
        let v = livraisons::lire_livraison_piece_sur_base(c.base, piece_id)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("enregistrer_livraison", Some("livraisons:enregistrer"), true, |c, p| {
        let piece_id: String = arg(&p, "pieceId", "piece_id")?;
        let lignes: Vec<livraisons::LigneLivraison> = arg(&p, "lignes", "lignes")?;
        let v = livraisons::enregistrer_livraison_sur_base(c.base, piece_id, lignes)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_ventes_paginees", None, false, |c, p| {
        let page: i64 = arg(&p, "page", "page")?;
        let limite: i64 = arg(&p, "limite", "limite")?;
        let recherche: Option<String> = arg(&p, "recherche", "recherche")?;
        let statut: Option<String> = arg(&p, "statut", "statut")?;
        let periode: Option<String> = arg(&p, "periode", "periode")?;
        let date_debut: Option<String> = arg(&p, "dateDebut", "date_debut")?;
        let date_fin: Option<String> = arg(&p, "dateFin", "date_fin")?;
        let v = pagination::lire_ventes_paginees_sur_base(c.base, page, limite, recherche, statut, periode, date_debut, date_fin)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_clients_pagines", None, false, |c, p| {
        let page: i64 = arg(&p, "page", "page")?;
        let limite: i64 = arg(&p, "limite", "limite")?;
        let recherche: Option<String> = arg(&p, "recherche", "recherche")?;
        let avec_creances_seulement: bool = arg(&p, "avecCreancesSeulement", "avec_creances_seulement")?;
        let ventes_filtre: Option<String> = arg(&p, "ventesFiltre", "ventes_filtre")?;
        let tri: Option<String> = arg(&p, "tri", "tri")?;
        let v = pagination::lire_clients_pagines_sur_base(c.base, page, limite, recherche, avec_creances_seulement, ventes_filtre, tri)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_stocks_pagines", None, false, |c, p| {
        let page: i64 = arg(&p, "page", "page")?;
        let limite: i64 = arg(&p, "limite", "limite")?;
        let recherche: Option<String> = arg(&p, "recherche", "recherche")?;
        let a_regulariser_seulement: bool = arg(&p, "aRegulariserSeulement", "a_regulariser_seulement")?;
        let categorie_id: Option<String> = arg(&p, "categorieId", "categorie_id")?;
        let v = pagination::lire_stocks_pagines_sur_base(c.base, page, limite, recherche, a_regulariser_seulement, categorie_id)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_fournisseurs_pagines", None, false, |c, p| {
        let page: i64 = arg(&p, "page", "page")?;
        let limite: i64 = arg(&p, "limite", "limite")?;
        let recherche: Option<String> = arg(&p, "recherche", "recherche")?;
        let v = pagination::lire_fournisseurs_pagines_sur_base(c.base, page, limite, recherche)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_ventes_recentes_paginee", None, false, |c, p| {
        let page: i64 = arg(&p, "page", "page")?;
        let limite: i64 = arg(&p, "limite", "limite")?;
        let recherche: Option<String> = arg(&p, "recherche", "recherche")?;
        let periode: Option<String> = arg(&p, "periode", "periode")?;
        let date_debut: Option<String> = arg(&p, "dateDebut", "date_debut")?;
        let date_fin: Option<String> = arg(&p, "dateFin", "date_fin")?;
        let v = pagination::lire_ventes_recentes_paginee_sur_base(c.base, page, limite, recherche, periode, date_debut, date_fin)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_categories", None, false, |c, _p| {
        let v = parametres::lire_categories_sur_base(c.base)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("creer_categorie", Some("parametres:modifier"), true, |c, p| {
        let nom: String = arg(&p, "nom", "nom")?;
        let v = parametres::creer_categorie_sur_base(c.base, nom)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_articles_complets", None, false, |c, _p| {
        let v = parametres::lire_articles_complets_sur_base(c.base)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("creer_article_complet", Some("parametres:modifier"), true, |c, p| {
        let nom: String = arg(&p, "nom", "nom")?;
        let categorie_id: Option<String> = arg(&p, "categorieId", "categorie_id")?;
        let unite_base: String = arg(&p, "uniteBase", "unite_base")?;
        let prix_reference: i64 = arg(&p, "prixReference", "prix_reference")?;
        let v = parametres::creer_article_complet_sur_base(c.base, nom, categorie_id, unite_base, prix_reference)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("ajouter_unite_vente", Some("parametres:modifier"), true, |c, p| {
        let article_id: String = arg(&p, "articleId", "article_id")?;
        let libelle: String = arg(&p, "libelle", "libelle")?;
        let facteur: f64 = arg(&p, "facteur", "facteur")?;
        let prix_reference: i64 = arg(&p, "prixReference", "prix_reference")?;
        let code_barre: Option<String> = arg(&p, "codeBarre", "code_barre")?;
        let v = parametres::ajouter_unite_vente_sur_base(c.base, article_id, libelle, facteur, prix_reference, code_barre)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("modifier_unite_vente", Some("parametres:modifier"), true, |c, p| {
        let unite_id: String = arg(&p, "uniteId", "unite_id")?;
        let libelle: Option<String> = arg(&p, "libelle", "libelle")?;
        let facteur: Option<f64> = arg(&p, "facteur", "facteur")?;
        let prix_reference: Option<i64> = arg(&p, "prixReference", "prix_reference")?;
        let code_barre: Option<String> = arg(&p, "codeBarre", "code_barre")?;
        let v = parametres::modifier_unite_vente_sur_base(c.base, unite_id, libelle, facteur, prix_reference, code_barre)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("desactiver_unite_vente", Some("parametres:modifier"), true, |c, p| {
        let unite_id: String = arg(&p, "uniteId", "unite_id")?;
        let v = parametres::desactiver_unite_vente_sur_base(c.base, unite_id)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_config_bon_sortie", None, false, |c, _p| {
        let v = parametres::lire_config_bon_sortie_sur_base(c.base)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("sauvegarder_config_bon_sortie", Some("parametres:modifier"), true, |c, p| {
        let actif: bool = arg(&p, "actif", "actif")?;
        let v = parametres::sauvegarder_config_bon_sortie_sur_base(c.base, actif)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_config_suivi_livraison", None, false, |c, _p| {
        let v = parametres::lire_config_suivi_livraison_sur_base(c.base)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("sauvegarder_config_suivi_livraison", Some("parametres:modifier"), true, |c, p| {
        let actif: bool = arg(&p, "actif", "actif")?;
        let v = parametres::sauvegarder_config_suivi_livraison_sur_base(c.base, actif)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_config_signatures", None, false, |c, _p| {
        let v = parametres::lire_config_signatures_sur_base(c.base)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("sauvegarder_config_signatures", Some("parametres:modifier"), true, |c, p| {
        let valeurs: std::collections::HashMap<String, String> = arg(&p, "valeurs", "valeurs")?;
        let v = parametres::sauvegarder_config_signatures_sur_base(c.base, valeurs)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_stocks", None, false, |c, _p| {
        let v = parametres::lire_stocks_sur_base(c.base)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("diagnostiquer_base", None, false, |c, _p| {
        let v = parametres::diagnostiquer_base_sur_base(c.base)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });







    // La reference du tiers : le numero que le fournisseur porte sur sa
    // propre facture. Se pose a tout moment — le papier arrive souvent
    // apres la marchandise.
    r.sur_base("definir_reference_piece", Some("pieces:creer"), true, |c, p| {
        let piece_id: String = arg(&p, "pieceId", "piece_id")?;
        let reference = p.get("reference").and_then(Value::as_str).map(str::to_string);
        pieces::definir_reference_piece_sur_base(c.base, piece_id, reference)?;
        Ok(Value::Null)
    });












    // ---- Les pieces ----
    r.sur_base("lire_toutes_pieces_client", None, false, |c, p| {
        let v = pieces::lire_toutes_pieces_client_sur_base(
            c.base,
            arg(&p, "typeFiltre", "type_filtre")?,
            arg(&p, "statut", "statut")?,
            arg(&p, "recherche", "recherche")?,
            arg(&p, "dateDebut", "date_debut")?,
            arg(&p, "dateFin", "date_fin")?,
            arg(&p, "montantMin", "montant_min")?,
            arg(&p, "montantMax", "montant_max")?,
            arg(&p, "impayeSeulement", "impaye_seulement")?,
            arg(&p, "enRetardSeulement", "en_retard_seulement")?,
            arg(&p, "clientId", "client_id")?,
        )?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });
    r.sur_base("lire_pieces_client", None, false, |c, p| {
        let v = pieces::lire_pieces_client_sur_base(
            c.base,
            arg(&p, "clientId", "client_id")?,
            arg(&p, "typeFiltre", "type_filtre")?,
        )?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });
    r.sur_base("lire_lignes_piece", None, false, |c, p| {
        let v = pieces::lire_lignes_piece_sur_base(c.base, arg(&p, "pieceId", "piece_id")?)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });
    r.sur_base("creer_piece", Some("pieces:creer"), true, |c, p| {
        let date_piece: Option<String> = arg(&p, "datePiece", "date_piece")?;
        exiger_antidatage_base(c.base, c.appelant, date_piece.as_deref())?;
        exiger_plafonds_piece(&gescom_noyau::plafonds::de_sur(c.base, &c.appelant.utilisateur_id), &p)?;
        pieces::creer_piece_sur_base(
            c.base,
            arg(&p, "clientId", "client_id")?,
            arg(&p, "typePiece", "type_piece")?,
            arg(&p, "lignes", "lignes")?,
            arg(&p, "remiseGlobale", "remise_globale")?,
            arg(&p, "dateEcheance", "date_echeance")?,
            arg(&p, "note", "note")?,
            arg(&p, "pieceOrigineId", "piece_origine_id")?,
            arg(&p, "depotId", "depot_id")?,
            date_piece,
        )
    });
    r.sur_base("convertir_piece", Some("pieces:creer"), true, |c, p| {
        pieces::convertir_piece_sur_base(
            c.base,
            arg(&p, "pieceId", "piece_id")?,
            arg(&p, "nouveauType", "nouveau_type")?,
        )
    });
    r.sur_base("convertir_commande_en_livraison_et_facture", Some("pieces:creer"), true, |c, p| {
        pieces::convertir_commande_en_livraison_et_facture_sur_base(
            c.base,
            arg(&p, "pieceId", "piece_id")?,
        )
    });
    r.sur_base("changer_statut_piece", Some("pieces:creer"), true, |c, p| {
        pieces::changer_statut_piece_sur_base(
            c.base,
            arg(&p, "pieceId", "piece_id")?,
            arg(&p, "nouveauStatut", "nouveau_statut")?,
        )?;
        Ok(Value::Null)
    });
    r.sur_base("lire_donnees_piece", None, false, |c, p| {
        pieces::lire_donnees_piece_sur_base(c.base, arg(&p, "pieceId", "piece_id")?)
    });
    r.sur_base("lire_fiche_client", None, false, |c, p| {
        pieces::lire_fiche_client_sur_base(c.base, arg(&p, "clientId", "client_id")?)
    });
    r.sur_base("lire_toutes_pieces_fournisseur", None, false, |c, p| {
        let v = pieces::lire_toutes_pieces_fournisseur_sur_base(
            c.base,
            arg(&p, "typeFiltre", "type_filtre")?,
            arg(&p, "statut", "statut")?,
            arg(&p, "recherche", "recherche")?,
            arg(&p, "fournisseurId", "fournisseur_id")?,
            arg(&p, "dateDebut", "date_debut")?,
            arg(&p, "dateFin", "date_fin")?,
        )?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });
    r.sur_base("creer_piece_fournisseur", Some("pieces:creer"), true, |c, p| {
        let date_piece: Option<String> = arg(&p, "datePiece", "date_piece")?;
        exiger_antidatage_base(c.base, c.appelant, date_piece.as_deref())?;
        pieces::creer_piece_fournisseur_sur_base(
            c.base,
            arg(&p, "fournisseurId", "fournisseur_id")?,
            arg(&p, "typePiece", "type_piece")?,
            arg(&p, "lignes", "lignes")?,
            arg(&p, "remiseGlobale", "remise_globale")?,
            arg(&p, "dateEcheance", "date_echeance")?,
            arg(&p, "note", "note")?,
            arg(&p, "pieceOrigineId", "piece_origine_id")?,
            date_piece,
        )
    });
    r.sur_base("modifier_piece", Some("pieces:creer"), true, |c, p| {
        let date_piece: Option<String> = arg(&p, "datePiece", "date_piece")?;
        exiger_antidatage_base(c.base, c.appelant, date_piece.as_deref())?;
        exiger_plafonds_piece(&gescom_noyau::plafonds::de_sur(c.base, &c.appelant.utilisateur_id), &p)?;
        pieces::modifier_piece_sur_base(
            c.base,
            arg(&p, "pieceId", "piece_id")?,
            arg(&p, "note", "note")?,
            arg(&p, "dateEcheance", "date_echeance")?,
            arg(&p, "remiseGlobale", "remise_globale")?,
            arg(&p, "lignes", "lignes")?,
            date_piece,
        )?;
        Ok(Value::Null)
    });
    r.sur_base("annuler_piece", Some("pieces:creer"), true, |c, p| {
        pieces::annuler_piece_sur_base(
            c.base,
            arg(&p, "pieceId", "piece_id")?,
            arg(&p, "motif", "motif")?,
        )?;
        Ok(Value::Null)
    });
    r.sur_base("dupliquer_piece", Some("pieces:creer"), true, |c, p| {
        pieces::dupliquer_piece_sur_base(c.base, arg(&p, "pieceId", "piece_id")?)
    });
    r.sur_base("lire_piece_de_vente", None, false, |c, p| {
        let v = pieces::lire_piece_de_vente_sur_base(c.base, arg(&p, "venteId", "vente_id")?)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });
    r.sur_base("annuler_facture_par_avoir", Some("pieces:creer"), true, |c, p| {
        pieces::annuler_facture_par_avoir_sur_base(
            c.base,
            arg(&p, "pieceId", "piece_id")?,
            arg(&p, "modeRemboursement", "mode_remboursement")?,
            arg(&p, "moyen", "moyen")?,
            arg(&p, "motif", "motif")?,
        )
    });
    r.sur_base("lire_vente_de_piece", None, false, |c, p| {
        let v = pieces::lire_vente_de_piece_sur_base(c.base, arg(&p, "pieceId", "piece_id")?)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("modifier_facture_pos", Some("ventes:creer"), true, |c, p| {
        let piece_id: String = arg(&p, "pieceId", "piece_id")?;
        let note: Option<String> = arg(&p, "note", "note")?;
        let date_echeance: Option<String> = arg(&p, "dateEcheance", "date_echeance")?;
        let v = pieces_pos::modifier_facture_pos_sur_base(c.base, piece_id, note, date_echeance)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("valider_facture_credit", Some("ventes:creer"), true, |c, p| {
        let piece_id: String = arg(&p, "pieceId", "piece_id")?;
        let v = pieces_pos::valider_facture_credit_sur_base(c.base, piece_id)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_rapport_ca_mensuel", None, false, |c, p| {
        let nb_mois: Option<i64> = arg(&p, "nbMois", "nb_mois")?;
        let v = rapports::lire_rapport_ca_mensuel_sur_base(c.base, nb_mois)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_rapport_top_clients", None, false, |c, p| {
        let date_debut: String = arg(&p, "dateDebut", "date_debut")?;
        let date_fin: String = arg(&p, "dateFin", "date_fin")?;
        let limite: Option<i64> = arg(&p, "limite", "limite")?;
        let v = rapports::lire_rapport_top_clients_sur_base(c.base, date_debut, date_fin, limite)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_rapport_top_articles", None, false, |c, p| {
        let date_debut: String = arg(&p, "dateDebut", "date_debut")?;
        let date_fin: String = arg(&p, "dateFin", "date_fin")?;
        let limite: Option<i64> = arg(&p, "limite", "limite")?;
        let v = rapports::lire_rapport_top_articles_sur_base(c.base, date_debut, date_fin, limite)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_rapport_creances", None, false, |c, _p| {
        let v = rapports::lire_rapport_creances_sur_base(c.base)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_rapport_stock", None, false, |c, _p| {
        let v = rapports::lire_rapport_stock_sur_base(c.base)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_rapport_tva", None, false, |c, p| {
        let date_debut: String = arg(&p, "dateDebut", "date_debut")?;
        let date_fin: String = arg(&p, "dateFin", "date_fin")?;
        let v = rapports::lire_rapport_tva_sur_base(c.base, date_debut, date_fin)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_creances_relances", None, false, |c, p| {
        let en_retard_seulement: Option<bool> = arg(&p, "enRetardSeulement", "en_retard_seulement")?;
        let v = relances::lire_creances_relances_sur_base(c.base, en_retard_seulement)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("enregistrer_relance", Some("creances:gerer"), true, |c, p| {
        let vente_id: String = arg(&p, "venteId", "vente_id")?;
        let canal: String = arg(&p, "canal", "canal")?;
        let note: Option<String> = arg(&p, "note", "note")?;
        let v = relances::enregistrer_relance_sur_base(c.base, vente_id, canal, note)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_historique_relances", None, false, |c, p| {
        let vente_id: String = arg(&p, "venteId", "vente_id")?;
        let v = relances::lire_historique_relances_sur_base(c.base, vente_id)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_stats_relances", None, false, |c, _p| {
        let v = relances::lire_stats_relances_sur_base(c.base)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });




    // ---- Les retours, sur `Base` ----
    r.sur_base("lire_ventes_recentes", None, false, |c, _p| {
        let v = retours::lire_ventes_recentes_sur_base(c.base)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });
    r.sur_base("enregistrer_retour", Some("retours:creer"), true, |c, p| {
        retours::enregistrer_retour_sur_base(
            c.base,
            arg(&p, "venteId", "vente_id")?,
            arg(&p, "ligneVenteId", "ligne_vente_id")?,
            arg(&p, "quantite", "quantite")?,
            arg(&p, "modeResolution", "mode_resolution")?,
            arg(&p, "modeEncaissement", "mode_encaissement")?,
            arg(&p, "articleRemplacementId", "article_remplacement_id")?,
            arg(&p, "uniteRemplacementId", "unite_remplacement_id")?,
            arg(&p, "quantiteRemplacement", "quantite_remplacement")?,
            arg(&p, "modeReliquatPositif", "mode_reliquat_positif")?,
            arg(&p, "modeEncaissementReliquat", "mode_encaissement_reliquat")?,
        )
    });
    r.sur_base("lire_avoirs_ouverts_tous", None, false, |c, _p| {
        let v = retours::lire_avoirs_ouverts_tous_sur_base(c.base)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("sauvegarder_base", Some("sauvegarde:lancer"), true, |c, p| {
        let dossier_destination: String = arg(&p, "dossierDestination", "dossier_destination")?;
        let v = sauvegarde::sauvegarder_base_sur_base(c.base, dossier_destination)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_config_sauvegarde", None, false, |c, _p| {
        let v = sauvegarde::lire_config_sauvegarde_sur_base(c.base)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("sauvegarde_auto_si_necessaire", Some("sauvegarde:lancer"), true, |c, _p| {
        let v = sauvegarde::sauvegarde_auto_si_necessaire_sur_base(c.base)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("sauvegarder_config_sauvegarde", Some("sauvegarde:lancer"), true, |c, p| {
        let dossier_sauvegarde: Option<String> = arg(&p, "dossierSauvegarde", "dossier_sauvegarde")?;
        let sauvegarde_auto: bool = arg(&p, "sauvegardeAuto", "sauvegarde_auto")?;
        let v = sauvegarde::sauvegarder_config_sauvegarde_sur_base(c.base, dossier_sauvegarde, sauvegarde_auto)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_parametres_societe", None, false, |c, _p| {
        let v = societe::lire_parametres_societe_sur_base(c.base)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("sauvegarder_parametres_societe", Some("parametres:modifier"), true, |c, p| {
        let nom: String = arg(&p, "nom", "nom")?;
        let adresse: Option<String> = arg(&p, "adresse", "adresse")?;
        let telephone: Option<String> = arg(&p, "telephone", "telephone")?;
        let telephone2: Option<String> = arg(&p, "telephone2", "telephone2")?;
        let email: Option<String> = arg(&p, "email", "email")?;
        let nif: Option<String> = arg(&p, "nif", "nif")?;
        let rccm: Option<String> = arg(&p, "rccm", "rccm")?;
        let site_web: Option<String> = arg(&p, "siteWeb", "site_web")?;
        let pied_facture: Option<String> = arg(&p, "piedFacture", "pied_facture")?;
        let v = societe::sauvegarder_parametres_societe_sur_base(c.base, nom, adresse, telephone, telephone2, email, nif, rccm, site_web, pied_facture)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("enregistrer_transfert", Some("stock:transferer"), true, |c, p| {
        let depot_source: String = arg(&p, "depotSource", "depot_source")?;
        let depot_dest: String = arg(&p, "depotDest", "depot_dest")?;
        let lignes: Vec<transferts::LigneTransfert> = arg(&p, "lignes", "lignes")?;
        let motif: Option<String> = arg(&p, "motif", "motif")?;
        let v = transferts::enregistrer_transfert_sur_base(c.base, depot_source, depot_dest, lignes, motif, Some(c.appelant.role.clone()))?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_transferts", None, false, |c, p| {
        let limite: Option<i64> = arg(&p, "limite", "limite")?;
        let v = transferts::lire_transferts_sur_base(c.base, limite)?;
        serde_json::to_value(v).map_err(|e| e.to_string())
    });

    r.sur_base("lire_bon_transfert", None, false, |c, p| {
        let bon: String = arg(&p, "bon", "bon")?;
        let v = transferts::lire_bon_transfert_sur_base(c.base, bon)?;
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

/// Antidater exige une permission A PART.
///
/// La commande porte deja sa permission de base (`pieces:creer`,
/// `ventes:creer`…). Mais saisir une date PASSEE est un autre geste :
/// c'est ainsi qu'on masque un trou dans le tiroir — on pousse la vente
/// en especes sur un autre jour, et le comptage du soir tombe juste.
/// Saisir la date du jour n'antidate pas : chaque vente le fait deja.
fn exiger_antidatage_base(
    base: &mut gescom_noyau::base::Base,
    appelant: &gescom_noyau::registre::Appelant,
    date: Option<&str>,
) -> Result<(), String> {
    let Some(d) = date.filter(|d| !d.trim().is_empty()) else { return Ok(()) };
    if !gescom_noyau::coeur::dates::est_antidatee(d, chrono::Local::now().date_naive()) {
        return Ok(());
    }
    let ctx = gescom_noyau::portes::ContexteUtilisateur {
        id: appelant.utilisateur_id.clone(),
        role: appelant.role.clone(),
    };
    gescom_noyau::portes::verifier_permission_sur(base, &ctx, "pieces:antidater")
        .map_err(|e| e.to_string())
}

// ---- v3, C-3 : les plafonds, juges sur l'argument ----
//
// Meme mecanisme que `pieces:antidater` : la commande a sa permission,
// l'ARGUMENT a son plafond. La regle est dans `coeur::plafonds` ; ici
// on ne fait que lui apporter les plafonds de la personne de la session.

/// (prix de reference, prix pratique, quantite) de chaque ligne.
fn lignes_pour_plafond(lignes: &[argent::ParamsLigneInput]) -> Vec<(i64, i64, f64)> {
    lignes.iter().map(|l| (l.prix_reference, l.prix_pratique, l.quantite)).collect()
}

fn exiger_plafonds_vente(
    plafonds: &gescom_noyau::coeur::plafonds::Plafonds,
    p: &Value,
    lignes: &[argent::ParamsLigneInput],
) -> Result<(), String> {
    let mode = option_texte(p, "modeReglement").or_else(|| option_texte(p, "mode_reglement")).unwrap_or_default();
    gescom_noyau::coeur::plafonds::verifier_vente(
        &lignes_pour_plafond(lignes),
        mode == "credit",
        entier(p, "montantPaye").or_else(|| entier(p, "montant_paye")).unwrap_or(0),
        entier(p, "avoirMontant").or_else(|| entier(p, "avoir_montant")).unwrap_or(0),
        plafonds,
    )
}

fn exiger_plafonds_piece(
    plafonds: &gescom_noyau::coeur::plafonds::Plafonds,
    p: &Value,
) -> Result<(), String> {
    let remises: Vec<f64> = p
        .get("lignes")
        .and_then(Value::as_array)
        .map(|t| t.iter().filter_map(|l| l.get("remise_pct").and_then(Value::as_f64)).collect())
        .unwrap_or_default();
    let globale = p
        .get("remiseGlobale")
        .or_else(|| p.get("remise_globale"))
        .and_then(Value::as_f64);
    gescom_noyau::coeur::plafonds::verifier_piece(&remises, globale, plafonds)
}

fn dossier_des_images(conn: &rusqlite::Connection) -> Option<std::path::PathBuf> {
    let chemin = conn.path()?;
    std::path::Path::new(chemin)
        .parent()
        .map(|d| d.to_path_buf())
}

/// Le dossier ou ranger les images, selon le moteur : a cote du
/// fichier pour SQLite ; un dossier fixe pour PostgreSQL, qui n'a pas
/// de fichier a cote duquel les poser.
fn dossier_des_images_base(base: &gescom_noyau::base::Base) -> Option<std::path::PathBuf> {
    if let Some(conn) = base.sqlite() {
        return dossier_des_images(conn);
    }
    dirs::data_dir().map(|d| d.join("ml.gescom.app"))
}
