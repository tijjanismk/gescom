//! D43 — l'assistant de creation du dossier, au premier demarrage.
//!
//! Une base NEUVE porte la marque `assistant_dossier = a_faire`, posee
//! par l'amorcage. Tant qu'elle y est, le serveur ne sert que
//! l'assistant (`COMMANDES_PERMISES`) : personne ne vend dans un
//! dossier sans nom, avec le mot de passe d'usine et des dates posees
//! par defaut. Une base sans la marque — toutes celles d'avant D43 — ne
//! voit jamais l'assistant : rien n'est devine.
//!
//! L'assistant ecrit TOUT en une transaction (`terminer_sur`) : le
//! patron, la societe, les dates du premier exercice, la TVA par
//! defaut, le premier numero de facture, le magasin, le fond de caisse
//! habituel, les vendeurs. Le logo, lui, part par sa commande
//! (`sauvegarder_logo`), comme dans Parametres.
//!
//! Version `Base` seule : la fenetre ne passe plus par les facades
//! `Connection` (D22 revise, lot E), comme les autres reglages de la v3.

use crate::base::{Acces, Base};
use crate::coeur::assistant as regles;
use crate::parametres;
use crate::utils::maintenant_iso;

/// Ce que le serveur sert tant que l'assistant reste a faire : se
/// situer, lire ce qu'on propose, le logo, finir l'assistant.
pub const COMMANDES_PERMISES: &[&str] = &[
    "lire_assistant",
    "terminer_assistant",
    "lire_parametres_societe",
    "lire_logo_base64",
    "sauvegarder_logo",
    "supprimer_logo",
    "lire_dossiers",
    "choisir_dossier",
];

fn cle_tva(dossier: &str) -> String {
    format!("tva_par_defaut:{dossier}")
}

fn cle_fond(dossier: &str) -> String {
    format!("fond_caisse_habituel:{dossier}")
}

fn cle_seuil(dossier: &str) -> String {
    format!("seuil_alerte_defaut:{dossier}")
}

/// Pose la marque d'une base neuve. Rejouable.
pub fn marquer_a_faire(acces: &mut impl Acces) {
    let _ = acces.executer(
        "INSERT INTO config_app (cle, valeur) VALUES (?1, ?2) ON CONFLICT (cle) DO NOTHING",
        &parametres![regles::ASSISTANT_CLE, regles::ASSISTANT_A_FAIRE],
    );
}

/// Retire la marque : une base de demonstration n'est pas une boutique,
/// le banc et les essais l'ouvrent directement.
pub fn marquer_fait(acces: &mut impl Acces) {
    let _ = acces.executer(
        "INSERT INTO config_app (cle, valeur) VALUES (?1, ?2)
         ON CONFLICT (cle) DO UPDATE SET valeur = excluded.valeur",
        &parametres![regles::ASSISTANT_CLE, format!("fait:{}", maintenant_iso())],
    );
}

/// L'assistant reste-t-il a faire ? Une lecture en echec vaut « non » :
/// on ne bloque pas une boutique qui tourne sur un doute.
pub fn a_faire_sur(acces: &mut impl Acces) -> bool {
    acces
        .lire_une(
            "SELECT valeur FROM config_app WHERE cle = ?1",
            &parametres![regles::ASSISTANT_CLE],
            |r| r.get::<String>(0),
        )
        .ok()
        .flatten()
        .is_some_and(|v| v == regles::ASSISTANT_A_FAIRE)
}

/// Le refus que rend le serveur a toute autre commande, tant que
/// l'assistant n'est pas fait. `None` : la commande passe.
pub fn refus_si_a_faire(acces: &mut impl Acces, commande: &str) -> Option<String> {
    if COMMANDES_PERMISES.contains(&commande) || !a_faire_sur(acces) {
        return None;
    }
    Some(format!(
        "{} : le dossier n'est pas encore créé. Le patron le crée à sa première connexion.",
        regles::CODE_ASSISTANT_A_FAIRE
    ))
}

// =====================================================================
//  LES REGLAGES DU DOSSIER QUI SERVENT ENSUITE
// =====================================================================

fn lire_config(acces: &mut impl Acces, cle: &str) -> Option<String> {
    acces
        .lire_une("SELECT valeur FROM config_app WHERE cle = ?1", &parametres![cle], |r| r.get::<String>(0))
        .ok()
        .flatten()
}

/// Le taux de TVA par defaut du dossier, en pourcent (0 si rien).
pub fn tva_par_defaut_pct_sur(acces: &mut impl Acces) -> i64 {
    let dossier = acces.dossier().to_string();
    lire_config(acces, &cle_tva(&dossier)).and_then(|v| v.parse().ok()).unwrap_or(0)
}

/// Le taux que prend un article NOUVEAU, en fraction (0.18), comme
/// `article.taux_tva_defaut`. Les articles deja crees ne bougent pas.
pub fn taux_nouvel_article_sur(acces: &mut impl Acces) -> f64 {
    tva_par_defaut_pct_sur(acces) as f64 / 100.0
}

/// D45 — le seuil d'alerte d'un article qui n'en a pas : celui du
/// dossier, 5 d'office (`coeur::stock::SEUIL_ALERTE_DEFAUT`).
pub fn seuil_defaut_sur(acces: &mut impl Acces) -> f64 {
    let dossier = acces.dossier().to_string();
    lire_config(acces, &cle_seuil(&dossier))
        .and_then(|v| v.parse().ok())
        .unwrap_or(crate::coeur::stock::SEUIL_ALERTE_DEFAUT)
}

/// Le fond de caisse propose a l'ouverture (0 si rien).
pub fn fond_habituel_sur(acces: &mut impl Acces) -> i64 {
    let dossier = acces.dossier().to_string();
    lire_config(acces, &cle_fond(&dossier)).and_then(|v| v.parse().ok()).unwrap_or(0)
}

fn ecrire_config(acces: &mut impl Acces, cle: &str, valeur: &str) -> Result<(), String> {
    acces
        .executer(
            "INSERT INTO config_app (cle, valeur) VALUES (?1, ?2)
             ON CONFLICT (cle) DO UPDATE SET valeur = excluded.valeur",
            &parametres![cle, valeur],
        )
        .map(|_| ())
        .map_err(|e| e.0)
}

/// Les reglages du dossier : TVA par defaut et fond de caisse habituel ;
/// de quoi poser la question « appliquer aux articles existants ? » :
/// combien d'articles actifs, combien n'ont pas deja ce taux, combien de
/// dossiers les partagent (les articles sont communs a tous).
pub fn lire_reglages_dossier_sur(base: &mut Base) -> Result<serde_json::Value, String> {
    let compter = |base: &mut Base, sql: &str| -> Result<i64, String> {
        base.lire_une(sql, &[], |r| r.get::<i64>(0)).map_err(|e| e.0).map(|n| n.unwrap_or(0))
    };
    let articles = compter(base, "SELECT COUNT(*) FROM article WHERE actif = 1")?;
    let dossiers = compter(base, "SELECT COUNT(*) FROM dossier WHERE clos = 0")?;
    let tva_pct = tva_par_defaut_pct_sur(base);
    Ok(serde_json::json!({
        "tva_pct": tva_pct,
        "fond_caisse": fond_habituel_sur(base),
        "seuil_defaut": seuil_defaut_sur(base),
        "taux_proposes": regles::TAUX_TVA_PROPOSES,
        "nb_articles": articles,
        "nb_dossiers": dossiers,
    }))
}

/// D43, complete le 28/10/2026 (le proprietaire : « on doit pouvoir
/// changer l'etat de la TVA dans l'app ») — la boutique facture-t-elle
/// la TVA ? Le taux des NOUVEAUX articles change ; avec
/// `appliquer_aux_articles`, tous les articles actifs prennent aussi ce
/// taux, dans la MEME transaction (l'ancien « Appliquer a tous » de
/// l'onglet TVA envoyait une commande par article : une coupure au
/// milieu laissait la moitie du catalogue a l'ancien taux). Les
/// articles etant communs a tous les dossiers, l'ecran le dit avant.
pub fn definir_tva_boutique_sur(
    base: &mut Base,
    tva_pct: i64,
    appliquer_aux_articles: bool,
) -> Result<serde_json::Value, String> {
    regles::verifier_tva(tva_pct)?;
    let dossier = base.dossier().to_string();
    let avant = tva_par_defaut_pct_sur(base);
    let auteur = crate::argent::id_utilisateur_courant_sur(base);
    let maintenant = maintenant_iso();
    let mut tx = base.transaction().map_err(|e| e.0)?;
    ecrire_config(&mut tx, &cle_tva(&dossier), &tva_pct.to_string())?;
    let articles = if appliquer_aux_articles {
        tx.executer(
            "UPDATE article SET taux_tva_defaut = ?1, modifie_le = ?2 WHERE actif = 1",
            &parametres![tva_pct as f64 / 100.0, maintenant.clone()],
        )
        .map_err(|e| e.0)?
    } else {
        0
    };
    tx.executer(
        "INSERT INTO journal
           (id, type_evenement, entite_type, entite_id, auteur_id,
            ancien_valeur, nouveau_valeur, origine, date_evenement, dossier_id)
         VALUES (?1, 'reglages_dossier_modifies', 'dossier', ?2, ?3, ?4, ?5, 'app', ?6, ?2)",
        &parametres![
            uuid::Uuid::new_v4().to_string(),
            dossier.clone(),
            auteur,
            serde_json::json!({ "tva_pct": avant }).to_string(),
            serde_json::json!({ "tva_pct": tva_pct, "articles_mis_au_taux": articles }).to_string(),
            maintenant
        ],
    )
    .map_err(|e| e.0)?;
    tx.valider().map_err(|e| e.0)?;
    let mut r = lire_reglages_dossier_sur(base)?;
    r["articles_modifies"] = serde_json::json!(articles);
    Ok(r)
}

/// Change les reglages du dossier apres l'assistant (Parametres ->
/// Societe). Juges comme dans l'assistant ; trace a l'Historique.
/// `seuil_defaut` (D45) : absent, inchange.
pub fn definir_reglages_dossier_sur(
    base: &mut Base,
    tva_pct: i64,
    fond_caisse: i64,
    seuil_defaut: Option<f64>,
) -> Result<serde_json::Value, String> {
    regles::verifier_tva(tva_pct)?;
    regles::verifier_fond_caisse(fond_caisse)?;
    if let Some(s) = seuil_defaut {
        crate::coeur::stock::verifier_seuil(s)?;
    }
    let dossier = base.dossier().to_string();
    let avant = lire_reglages_dossier_sur(base)?;
    let auteur = crate::argent::id_utilisateur_courant_sur(base);
    let mut tx = base.transaction().map_err(|e| e.0)?;
    ecrire_config(&mut tx, &cle_tva(&dossier), &tva_pct.to_string())?;
    ecrire_config(&mut tx, &cle_fond(&dossier), &fond_caisse.to_string())?;
    if let Some(s) = seuil_defaut {
        ecrire_config(&mut tx, &cle_seuil(&dossier), &s.to_string())?;
    }
    let seuil_apres = seuil_defaut.or(avant["seuil_defaut"].as_f64());
    tx.executer(
        "INSERT INTO journal
           (id, type_evenement, entite_type, entite_id, auteur_id,
            ancien_valeur, nouveau_valeur, origine, date_evenement, dossier_id)
         VALUES (?1, 'reglages_dossier_modifies', 'dossier', ?2, ?3, ?4, ?5, 'app', ?6, ?2)",
        &parametres![
            uuid::Uuid::new_v4().to_string(),
            dossier.clone(),
            auteur,
            serde_json::json!({ "tva_pct": avant["tva_pct"], "fond_caisse": avant["fond_caisse"], "seuil_defaut": avant["seuil_defaut"] }).to_string(),
            serde_json::json!({ "tva_pct": tva_pct, "fond_caisse": fond_caisse, "seuil_defaut": seuil_apres }).to_string(),
            maintenant_iso()
        ],
    )
    .map_err(|e| e.0)?;
    tx.valider().map_err(|e| e.0)?;
    lire_reglages_dossier_sur(base)
}

// =====================================================================
//  CE QUE L'ASSISTANT PROPOSE
// =====================================================================

/// Ce que l'ecran affiche d'emblee : ce qui est deja en base (nom de la
/// societe, dates posees par l'amorcage, nom du magasin) et le compte
/// de qui fait l'assistant.
pub fn lire_sur(base: &mut Base, utilisateur_id: &str) -> Result<serde_json::Value, String> {
    let dossier = base.dossier().to_string();
    let societe = crate::societe::lire_parametres_societe_sur_base(base).unwrap_or(serde_json::Value::Null);
    let exercices = crate::dossiers::lire_les_exercices(base, &dossier)?;
    let exercice = exercices.iter().find(|e| !e.clos);
    let magasin: Option<String> = base
        .lire_une(
            "SELECT nom FROM depot WHERE est_defaut = 1 AND dossier_id = ?1",
            &parametres![dossier.clone()],
            |r| r.get::<String>(0),
        )
        .map_err(|e| e.0)?;
    let patron: Option<(String, Option<String>)> = base
        .lire_une(
            "SELECT u.nom, ua.pseudo FROM utilisateur u
             LEFT JOIN utilisateur_auth ua ON ua.utilisateur_id = u.id
             WHERE u.id = ?1",
            &parametres![utilisateur_id],
            |r| Ok((r.get::<String>(0)?, r.get::<Option<String>>(1)?)),
        )
        .map_err(|e| e.0)?;
    let aujourd_hui = maintenant_iso();
    Ok(serde_json::json!({
        "a_faire": a_faire_sur(base),
        "societe": societe,
        "date_debut": exercice.map(|e| e.date_debut.clone()),
        "date_fin": exercice.map(|e| e.date_fin.clone()),
        "magasin": magasin,
        "tva_pct": tva_par_defaut_pct_sur(base),
        "fond_caisse": fond_habituel_sur(base),
        "patron_nom": patron.as_ref().map(|p| p.0.clone()),
        "patron_pseudo": patron.and_then(|p| p.1),
        "annee": &aujourd_hui[..4],
        "aujourd_hui": &aujourd_hui[..10],
        "tva_normale_pct": regles::TVA_NORMALE_PCT,
        "numero_depart_max": regles::NUMERO_DEPART_MAX,
        "duree_max_ans": crate::coeur::fin_exercice::DUREE_MAX_ANS,
    }))
}

// =====================================================================
//  FINIR L'ASSISTANT
// =====================================================================

#[derive(Debug, Default, Clone, serde::Deserialize)]
pub struct Vendeur {
    pub nom: String,
    pub pseudo: String,
    pub mot_de_passe: String,
}

/// Ce que l'ecran envoie, en une fois.
#[derive(Debug, Default, Clone, serde::Deserialize)]
#[serde(default)]
pub struct Saisie {
    pub patron_nom: String,
    pub patron_pseudo: String,
    pub mot_de_passe: String,
    pub societe: String,
    pub adresse: Option<String>,
    pub telephone: Option<String>,
    pub telephone2: Option<String>,
    pub email: Option<String>,
    pub nif: Option<String>,
    pub rccm: Option<String>,
    pub site_web: Option<String>,
    pub pied_facture: Option<String>,
    pub date_debut: String,
    pub date_fin: String,
    pub tva_pct: i64,
    pub numero_depart: Option<i64>,
    pub magasin: String,
    pub fond_caisse: i64,
    pub vendeurs: Vec<Vendeur>,
}

/// Une saisie jugee : rien ne s'ecrit avant que tout soit passe.
struct Jugee {
    patron_nom: String,
    patron_pseudo: String,
    societe: String,
    debut: String,
    fin: String,
    magasin: String,
    vendeurs: Vec<(String, String)>,
}

fn juger(s: &Saisie, aujourd_hui: &str) -> Result<Jugee, String> {
    let patron_nom = regles::obligatoire(&s.patron_nom, "Votre nom", 80)?;
    let patron_pseudo = regles::verifier_pseudo(&s.patron_pseudo)?;
    regles::verifier_mot_de_passe(&s.mot_de_passe, &patron_pseudo)?;
    let societe = regles::obligatoire(&s.societe, "Raison sociale", 120)?;
    let (debut, fin) =
        crate::dossiers::dates_de_travail(Some(&s.date_debut), Some(&s.date_fin), aujourd_hui)?;
    regles::verifier_dates_couvrent(&debut, &fin, aujourd_hui)?;
    regles::verifier_tva(s.tva_pct)?;
    if let Some(n) = s.numero_depart {
        regles::verifier_numero_depart(n)?;
    }
    let magasin = regles::obligatoire(&s.magasin, "Nom du magasin principal", 80)?;
    regles::verifier_fond_caisse(s.fond_caisse)?;
    let mut vendeurs = Vec::new();
    for v in &s.vendeurs {
        let nom = regles::obligatoire(&v.nom, "Nom du vendeur", 80)?;
        let pseudo = regles::verifier_pseudo(&v.pseudo)?;
        regles::verifier_mot_de_passe(&v.mot_de_passe, &pseudo)
            .map_err(|e| format!("{nom} : {e}"))?;
        vendeurs.push((nom, pseudo));
    }
    let mut pseudos = vec![patron_pseudo.clone()];
    pseudos.extend(vendeurs.iter().map(|(_, p)| p.clone()));
    regles::verifier_pseudos_distincts(&pseudos)?;
    Ok(Jugee { patron_nom, patron_pseudo, societe, debut, fin, magasin, vendeurs })
}

/// Un identifiant deja porte par un AUTRE compte que `sauf`.
fn pseudo_pris(base: &mut Base, pseudo: &str, sauf: &str) -> Result<bool, String> {
    base.lire_une(
        "SELECT COUNT(*) FROM utilisateur_auth
         WHERE LOWER(pseudo) = LOWER(?1) AND utilisateur_id <> ?2",
        &parametres![pseudo, sauf],
        |r| r.get::<i64>(0),
    )
    .map_err(|e| e.0)
    .map(|n| n.unwrap_or(0) > 0)
}

/// Finit l'assistant : tout s'ecrit, ou rien. `utilisateur_id` est
/// celui de la session (D26) — c'est son compte qui recoit le nom,
/// l'identifiant et le mot de passe du patron.
pub fn terminer_sur(base: &mut Base, utilisateur_id: &str, saisie: Saisie) -> Result<serde_json::Value, String> {
    terminer_au_sur(base, utilisateur_id, saisie, &maintenant_iso())
}

/// Meme chose a une date donnee : la regle « les dates couvrent
/// aujourd'hui » se teste sans horloge.
pub fn terminer_au_sur(
    base: &mut Base,
    utilisateur_id: &str,
    saisie: Saisie,
    maintenant: &str,
) -> Result<serde_json::Value, String> {
    if !a_faire_sur(base) {
        return Err("Le dossier est déjà créé : ses réglages se changent dans Paramètres.".to_string());
    }
    let j = juger(&saisie, maintenant)?;
    let dossier = base.dossier().to_string();

    // Les identifiants : uniques sur toute la base, sans la casse.
    if pseudo_pris(base, &j.patron_pseudo, utilisateur_id)? {
        return Err(format!("L'identifiant « {} » est déjà pris.", j.patron_pseudo));
    }
    for (_, p) in &j.vendeurs {
        if pseudo_pris(base, p, "")? {
            return Err(format!("L'identifiant « {p} » est déjà pris."));
        }
    }

    // Un seul exercice ouvert : celui que l'amorcage a pose.
    let exercices = crate::dossiers::lire_les_exercices(base, &dossier)?;
    let exercice = match exercices.as_slice() {
        [seul] if !seul.clos => seul.id.clone(),
        _ => {
            return Err("Ce dossier a déjà plusieurs exercices : ses dates se règlent dans Paramètres → Dossiers.".to_string())
        }
    };

    // Le premier numero de facture : seulement si aucune facture n'est
    // sortie cette annee — sinon on creerait un trou ou un doublon.
    let annee = &maintenant[..4];
    let cle_factures = crate::dossiers::cle_compteur(
        &dossier,
        &format!("{}-{annee}", crate::argent::prefixe_de("facture")),
    );
    let depart = saisie.numero_depart.filter(|n| *n > 1);
    if depart.is_some() {
        let deja: Option<i64> = base
            .lire_une(
                "SELECT dernier FROM compteur_piece WHERE cle = ?1",
                &parametres![cle_factures.clone()],
                |r| r.get::<i64>(0),
            )
            .map_err(|e| e.0)?;
        if deja.is_some_and(|d| d > 0) {
            return Err("Des factures sont déjà sorties cette année : le premier numéro ne se règle plus.".to_string());
        }
    }

    // Les mots de passe se hachent HORS de la transaction : c'est lent
    // (bcrypt), et rien ne s'ecrit encore.
    let hash_patron = crate::auth::hasher_mot_de_passe_pub(&saisie.mot_de_passe)?;
    let mut hash_vendeurs = Vec::new();
    for v in &saisie.vendeurs {
        hash_vendeurs.push(crate::auth::hasher_mot_de_passe_pub(&v.mot_de_passe)?);
    }
    let role_employe: Option<String> = if j.vendeurs.is_empty() {
        None
    } else {
        Some(
            base.lire_une("SELECT id FROM role WHERE nom = 'employe'", &[], |r| r.get::<String>(0))
                .map_err(|e| e.0)?
                .ok_or_else(|| "Le rôle « employe » est absent de la base.".to_string())?,
        )
    };

    let txt = |o: &Option<String>| regles::texte(o.as_deref());
    let mut tx = base.transaction().map_err(|e| e.0)?;

    // 1. Le patron.
    tx.executer(
        "UPDATE utilisateur SET nom = ?1, modifie_le = ?2 WHERE id = ?3",
        &parametres![j.patron_nom.clone(), maintenant, utilisateur_id],
    )
    .map_err(|e| e.0)?;
    let touche = tx
        .executer(
            "UPDATE utilisateur_auth SET pseudo = ?1, mot_de_passe = ?2, doit_changer_mdp = 0
             WHERE utilisateur_id = ?3",
            &parametres![j.patron_pseudo.clone(), hash_patron, utilisateur_id],
        )
        .map_err(|e| e.0)?;
    if touche == 0 {
        return Err("Le compte de la session est introuvable.".to_string());
    }

    // 2. La societe : l'en-tete des pieces, et le nom du dossier. Le
    // logo, l'en-tete et le pied images ne sont pas touches.
    tx.executer(
        "INSERT INTO parametres_societe
         (id, nom, adresse, telephone, telephone2, email,
          nif, rccm, site_web, pied_facture, modifie_le)
         VALUES (1, ?1, CAST(?2 AS TEXT), CAST(?3 AS TEXT), CAST(?4 AS TEXT),
                 CAST(?5 AS TEXT), CAST(?6 AS TEXT), CAST(?7 AS TEXT),
                 CAST(?8 AS TEXT), CAST(?9 AS TEXT), ?10)
         ON CONFLICT (id) DO UPDATE SET
           nom = ?1, adresse = ?2, telephone = ?3, telephone2 = ?4,
           email = ?5, nif = ?6, rccm = ?7, site_web = ?8,
           pied_facture = ?9, modifie_le = ?10",
        &parametres![
            j.societe.clone(),
            txt(&saisie.adresse),
            txt(&saisie.telephone),
            txt(&saisie.telephone2),
            txt(&saisie.email),
            txt(&saisie.nif),
            txt(&saisie.rccm),
            txt(&saisie.site_web),
            txt(&saisie.pied_facture),
            maintenant
        ],
    )
    .map_err(|e| e.0)?;
    tx.executer(
        "UPDATE dossier SET societe = ?1, modifie_le = ?2 WHERE id = ?3",
        &parametres![j.societe.clone(), maintenant, dossier.clone()],
    )
    .map_err(|e| e.0)?;

    // 3. Les dates du premier exercice (D21, D38).
    tx.executer(
        "UPDATE exercice SET date_debut = ?1, date_fin = ?2, prolonge_jusqu_au = NULL, modifie_le = ?3
         WHERE id = ?4 AND dossier_id = ?5",
        &parametres![j.debut.clone(), j.fin.clone(), maintenant, exercice, dossier.clone()],
    )
    .map_err(|e| e.0)?;

    // 4. Les reglages qui servent ensuite.
    ecrire_config(&mut tx, &cle_tva(&dossier), &saisie.tva_pct.to_string())?;
    ecrire_config(&mut tx, &cle_fond(&dossier), &saisie.fond_caisse.to_string())?;
    if let Some(n) = depart {
        // Le compteur rend `dernier + 1` : on pose le rang d'avant.
        tx.executer(
            "INSERT INTO compteur_piece (cle, dernier) VALUES (?1, ?2)
             ON CONFLICT (cle) DO UPDATE SET dernier = excluded.dernier",
            &parametres![cle_factures.clone(), n - 1],
        )
        .map_err(|e| e.0)?;
    }
    tx.executer(
        "UPDATE depot SET nom = ?1, modifie_le = ?2 WHERE est_defaut = 1 AND dossier_id = ?3",
        &parametres![j.magasin.clone(), maintenant, dossier.clone()],
    )
    .map_err(|e| e.0)?;

    // 5. Les vendeurs : un mot de passe provisoire, a changer a leur
    // premiere connexion.
    for ((nom, pseudo), hash) in j.vendeurs.iter().zip(hash_vendeurs) {
        let id = uuid::Uuid::new_v4().to_string();
        tx.executer(
            "INSERT INTO utilisateur (id, nom, role_id, actif, cree_le, modifie_le, origine)
             VALUES (?1, ?2, ?3, 1, ?4, ?4, 'assistant')",
            &parametres![id.clone(), nom.clone(), role_employe.clone().unwrap_or_default(), maintenant],
        )
        .map_err(|e| e.0)?;
        tx.executer(
            "INSERT INTO utilisateur_auth (utilisateur_id, pseudo, mot_de_passe, doit_changer_mdp)
             VALUES (?1, ?2, ?3, 1)",
            &parametres![id, pseudo.clone(), hash],
        )
        .map_err(|e| e.0)?;
    }

    // 6. L'Historique — sans aucun mot de passe.
    let resume = serde_json::json!({
        "societe": j.societe,
        "du": j.debut,
        "au": j.fin,
        "tva_pct": saisie.tva_pct,
        "premier_numero_facture": depart,
        "magasin": j.magasin,
        "fond_caisse": saisie.fond_caisse,
        "patron": j.patron_pseudo,
        "vendeurs": j.vendeurs.iter().map(|(_, p)| p.clone()).collect::<Vec<_>>(),
    });
    tx.executer(
        "INSERT INTO journal
           (id, type_evenement, entite_type, entite_id, auteur_id,
            nouveau_valeur, origine, date_evenement, dossier_id)
         VALUES (?1, 'assistant_dossier_fait', 'dossier', ?2, ?3, ?4, 'app', ?5, ?2)",
        &parametres![
            uuid::Uuid::new_v4().to_string(),
            dossier.clone(),
            utilisateur_id,
            resume.to_string(),
            maintenant
        ],
    )
    .map_err(|e| e.0)?;

    marquer_fait(&mut tx);
    tx.valider().map_err(|e| e.0)?;
    Ok(resume)
}
