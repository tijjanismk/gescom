//! Les routes du serveur.

use std::net::TcpStream;
use std::sync::Arc;

use serde_json::{json, Value};

use gescom_noyau::portes::{verifier_permission_sur, ContexteUtilisateur};
use gescom_noyau::protocole::{
    CodeErreur, DemandeConnexion, Identite, LotEvenements, Reponse, Sante,
};
use gescom_noyau::VERSION_PROTOCOLE;
use gescom_noyau::registre::{Appelant, Contexte, ContexteBase};
use gescom_noyau::{caisses, persistance, postes, sessions};

use crate::etat::Serveur;
use crate::http::{repondre_html, repondre_json, repondre_texte, Requete};
use crate::sauvegarde;

pub fn traiter(srv: &Arc<Serveur>, req: &Requete, flux: &mut TcpStream) -> std::io::Result<()> {
    // Le navigateur embarque envoie un OPTIONS avant tout POST porteur
    // d'un en-tete Authorization. Sans reponse, aucune commande ne part
    // — et rien n'apparait dans les journaux du serveur, ce qui rend le
    // diagnostic penible.
    if req.methode == "OPTIONS" {
        return repondre_texte(flux, 204, "");
    }

    match (req.methode.as_str(), req.chemin.as_str()) {
        // La console du serveur. Ce qu'elle montre sans mot de passe
        // est exactement ce que `/sante` expose deja : aucune donnee de
        // commerce.
        ("GET", "/") | ("GET", "/console") => {
            repondre_html(flux, &crate::console::page(srv.port))
        }
        ("GET", "/sante") => sante(srv, flux),
        ("POST", "/connexion") => connexion(srv, req, flux),
        ("POST", "/deconnexion") => deconnexion(srv, req, flux),
        ("POST", "/rpc") => rpc(srv, req, flux),
        ("GET", "/rpc/catalogue") => {
            repondre_json(flux, 200, &json!({ "commandes": srv.registre.noms() }))
        }
        ("GET", "/canal") => canal(srv, req, flux),
        ("POST", "/sauvegarde") => sauvegarde_manuelle(srv, req, flux),
        _ => repondre_json(
            flux,
            404,
            &json!({
                "etat": "erreur",
                "code": "technique",
                "message": format!("Route inconnue : {} {}", req.methode, req.chemin),
            }),
        ),
    }
}

// =====================================================================
//  Sante — la seule route ouverte sans jeton
// =====================================================================

/// Volontairement anonyme : c'est ce que le poste caisse interroge
/// AVANT d'avoir un jeton, pour dire « serveur joignable » plutot que
/// « identifiant incorrect » quand c'est le reseau qui manque. Elle ne
/// divulgue aucune donnee de commerce.
fn sante(srv: &Arc<Serveur>, flux: &mut TcpStream) -> std::io::Result<()> {
    // Le controle d'integrite est une PRAGMA SQLite : pas de pendant
    // PostgreSQL pour l'instant, donc pas de controle sur ce moteur.
    let base_saine = match &srv.conn {
        Some(m) => m
            .lock()
            .ok()
            .map(|conn| persistance::verifier_integrite(&conn).map(|o| o.is_none()).unwrap_or(false))
            .unwrap_or(false),
        None => true,
    };
    // Les sessions, elles, sont portees (D11) : le compte est exact sur
    // les deux moteurs.
    let connectes = srv
        .base
        .lock()
        .ok()
        .map(|mut b| sessions::lister_actives_sur(&mut b).map(|v| v.len()).unwrap_or(0))
        .unwrap_or(0);

    let s = Sante {
        version_protocole: VERSION_PROTOCOLE,
        version_serveur: env!("CARGO_PKG_VERSION").to_string(),
        demarre_le: srv.demarre_le.clone(),
        postes_connectes: connectes,
        base_saine,
        derniere_sauvegarde: srv.derniere_sauvegarde.lock().ok().and_then(|v| v.clone()),
    };
    repondre_json(flux, 200, &serde_json::to_value(s).unwrap_or(Value::Null))
}

// =====================================================================
//  Connexion
// =====================================================================

fn connexion(srv: &Arc<Serveur>, req: &Requete, flux: &mut TcpStream) -> std::io::Result<()> {
    let demande: DemandeConnexion = match req
        .json()
        .and_then(|v| serde_json::from_value(v).map_err(|e| format!("Demande illisible : {e}")))
    {
        Ok(d) => d,
        Err(m) => return erreur(flux, 400, CodeErreur::Technique, &m),
    };

    // Le refus doit etre net et immediat. Laisser passer une version
    // differente ferait lire des champs absents comme des zeros : des
    // montants faux, sans message d'erreur.
    if demande.version_protocole != VERSION_PROTOCOLE {
        return erreur(
            flux,
            426,
            CodeErreur::Technique,
            &format!(
                "Ce poste parle la version {} du protocole, le serveur la version {}. \
                 Mettre les deux à la même version de Gescom.",
                demande.version_protocole, VERSION_PROTOCOLE
            ),
        );
    }

    // D11 : l'authentification passe par `Base`, sur les deux moteurs —
    // sessions, postes et permissions sont portes. Seules les 186
    // commandes du registre restent sur `Connection` pour l'instant.
    let mut base = match srv.base.lock() {
        Ok(b) => b,
        Err(_) => return erreur(flux, 500, CodeErreur::Technique, "Base indisponible."),
    };

    let ligne = base.lire_une(
        "SELECT ua.utilisateur_id, ua.mot_de_passe, ua.doit_changer_mdp,
                u.nom, r.nom
         FROM utilisateur_auth ua
         JOIN utilisateur u ON u.id = ua.utilisateur_id
         JOIN role r        ON r.id = u.role_id
         WHERE (ua.pseudo = ?1 OR ua.email = ?1) AND u.actif = 1",
        &gescom_noyau::parametres![demande.identifiant.clone()],
        |r| {
            Ok((
                r.get::<String>(0)?,
                r.get::<String>(1)?,
                r.get::<i64>(2)?,
                r.get::<String>(3)?,
                r.get::<String>(4)?,
            ))
        },
    );

    // Meme message pour « identifiant inconnu » et « mot de passe
    // faux » : distinguer les deux revient a confirmer qu'un compte
    // existe a qui essaie des noms au hasard.
    let Ok(Some((utilisateur_id, hash, doit_changer, nom, role))) = ligne else {
        return erreur(
            flux,
            401,
            CodeErreur::Authentification,
            "Identifiant ou mot de passe incorrect",
        );
    };
    if !bcrypt::verify(&demande.mot_de_passe, &hash).unwrap_or(false) {
        return erreur(
            flux,
            401,
            CodeErreur::Authentification,
            "Identifiant ou mot de passe incorrect",
        );
    }

    // La console du serveur n'est pas une caisse : elle n'encaisse
    // pas, et surtout elle ne doit pas pouvoir etre desactivee depuis
    // elle-meme.
    let genre = if demande.poste_empreinte == crate::console::EMPREINTE {
        "console"
    } else {
        "caisse"
    };
    let poste = match postes::inscrire_ou_retrouver_sur(
        &mut base,
        &demande.poste_nom,
        &demande.poste_empreinte,
        genre,
        Some(&req.ip),
    ) {
        Ok(p) => p,
        Err(e) => return erreur(flux, 500, CodeErreur::Technique, &e),
    };
    if !poste.actif {
        return erreur(
            flux,
            403,
            CodeErreur::Permission,
            "Ce poste a été désactivé depuis le serveur.",
        );
    }

    let (session_id, jeton, expire_le) =
        match sessions::ouvrir_sur(&mut base, &poste.id, &utilisateur_id) {
            Ok(t) => t,
            Err(e) => return erreur(flux, 500, CodeErreur::Technique, &e),
        };

    let _ = base.executer(
        "UPDATE utilisateur_auth SET derniere_connexion = ?1 WHERE utilisateur_id = ?2",
        &gescom_noyau::parametres![gescom_noyau::utils::maintenant_iso(), utilisateur_id.clone()],
    );

    let mut permissions: Vec<String> =
        gescom_noyau::portes::permissions_de_sur(&mut base, &utilisateur_id, &role)
            .into_iter()
            .collect();
    permissions.sort();

    let identite = Identite {
        jeton: jeton.clone(),
        utilisateur_id,
        utilisateur_nom: nom,
        role,
        permissions,
        doit_changer_mdp: doit_changer != 0,
        poste_id: poste.id.clone(),
        expire_le,
        caisse_par_utilisateur: caisses::par_utilisateur_sur(&mut base),
    };
    drop(base);

    srv.enregistrer_jeton(jeton, session_id);
    srv.canal
        .publier("poste_connecte", "poste", Some(poste.id.clone()), &poste.id);

    repondre_json(
        flux,
        200,
        &serde_json::to_value(identite).unwrap_or(Value::Null),
    )
}

fn deconnexion(srv: &Arc<Serveur>, req: &Requete, flux: &mut TcpStream) -> std::io::Result<()> {
    if let Some(jeton) = req.jeton() {
        if let Some(session_id) = srv.session_du_jeton(jeton) {
            if let Ok(mut base) = srv.base.lock() {
                sessions::revoquer_sur(&mut base, &session_id, "deconnexion").ok();
            }
        }
        srv.oublier_jeton(jeton);
    }
    repondre_json(flux, 200, &json!({ "etat": "ok" }))
}

// =====================================================================
//  Appel de commande
// =====================================================================

fn rpc(srv: &Arc<Serveur>, req: &Requete, flux: &mut TcpStream) -> std::io::Result<()> {
    let appelant = match authentifier(srv, req) {
        Ok(a) => a,
        Err((code, message)) => return erreur(flux, 401, code, &message),
    };

    let corps = match req.json() {
        Ok(v) => v,
        Err(m) => return erreur(flux, 400, CodeErreur::Technique, &m),
    };
    let nom = corps.get("commande").and_then(Value::as_str).unwrap_or("");
    let params = corps.get("params").cloned().unwrap_or(Value::Null);

    let Some(entree) = srv.registre.trouver(nom) else {
        return erreur(
            flux,
            404,
            CodeErreur::CommandeInconnue,
            &format!(
                "Le serveur ne connaît pas la commande « {nom} ». \
                 Ce poste est probablement plus récent que le serveur.",
            ),
        );
    };

    // Les permissions se lisent en base a CHAQUE appel — via `Base`,
    // portee (D11) — et sur les DEUX moteurs : c'est ce qui rend un
    // retrait de droit immediat, comme la revocation d'une session. Les
    // garder en memoire ferait attendre le redemarrage du serveur.
    // Fait AVANT de savoir si la commande elle-meme peut s'executer :
    // une caisse sans le droit doit lire un refus de permission, pas
    // un refus generique « pas encore sur PostgreSQL ».
    if let Some(permission) = entree.permission {
        let ctx = ContexteUtilisateur {
            id: appelant.utilisateur_id.clone(),
            role: appelant.role.clone(),
        };
        let mut base = match srv.base.lock() {
            Ok(b) => b,
            Err(_) => return erreur(flux, 500, CodeErreur::Technique, "Base indisponible."),
        };
        if let Err(e) = verifier_permission_sur(&mut base, &ctx, permission) {
            return erreur(flux, 403, CodeErreur::Permission, &e.to_string());
        }
    }

    // D11 : sur une cible fichier, `conn` existe toujours — le chemin
    // ne change pas d'un octet. Sur PostgreSQL, `conn` est absent ; la
    // commande passe par `poignee_base` quand elle est portee, sinon
    // elle refuse clairement plutot que de retomber en silence sur un
    // fichier SQLite vide.
    let resultat = if let Some(conn_mutex) = srv.conn.as_ref() {
        let mut conn = match conn_mutex.lock() {
            Ok(c) => c,
            Err(_) => return erreur(flux, 500, CodeErreur::Technique, "Base indisponible."),
        };
        (entree.poignee)(
            &mut Contexte {
                conn: &mut conn,
                appelant: &appelant,
            },
            params,
        )
    } else if let Some(poignee_base) = entree.poignee_base {
        let mut base = match srv.base.lock() {
            Ok(b) => b,
            Err(_) => return erreur(flux, 500, CodeErreur::Technique, "Base indisponible."),
        };
        poignee_base(
            &mut ContexteBase {
                base: &mut base,
                appelant: &appelant,
            },
            params,
        )
    } else {
        return erreur(
            flux,
            409,
            CodeErreur::Technique,
            "Cette opération n'est pas encore disponible sur PostgreSQL.",
        );
    };

    // L'evenement n'est publie que si la commande a REUSSI. Prevenir
    // les autres postes d'une vente refusee les ferait recharger pour
    // rien, et pire, laisserait croire que quelque chose a bouge.
    if entree.ecrit && resultat.is_ok() {
        srv.canal.publier(nom, "commande", None, &appelant.poste_id);
    }

    let reponse = Reponse::depuis(resultat);
    let code = match &reponse {
        Reponse::Ok { .. } => 200,
        Reponse::Erreur { .. } => 409,
    };
    repondre_json(
        flux,
        code,
        &serde_json::to_value(reponse).unwrap_or(Value::Null),
    )
}

fn authentifier(srv: &Arc<Serveur>, req: &Requete) -> Result<Appelant, (CodeErreur, String)> {
    let jeton = req.jeton().ok_or((
        CodeErreur::Authentification,
        "Jeton absent — se connecter au serveur.".to_string(),
    ))?;
    let session_id = srv.session_du_jeton(jeton).ok_or((
        CodeErreur::Authentification,
        "Session inconnue du serveur — se reconnecter.".to_string(),
    ))?;

    let mut base = srv
        .base
        .lock()
        .map_err(|_| (CodeErreur::Technique, "Base indisponible.".to_string()))?;

    match sessions::etat_sur(&mut base, &session_id) {
        sessions::Etat::Valide {
            utilisateur_id,
            role,
            poste_id,
        } => {
            sessions::toucher_sur(&mut base, &session_id);
            Ok(Appelant {
                utilisateur_id,
                role,
                poste_id,
            })
        }
        sessions::Etat::Expiree => Err((
            CodeErreur::Authentification,
            "Session expirée — se reconnecter.".to_string(),
        )),
        sessions::Etat::Revoquee => Err((
            CodeErreur::Authentification,
            "Session fermée depuis le serveur.".to_string(),
        )),
        sessions::Etat::PosteFerme => Err((
            CodeErreur::Authentification,
            "Ce poste a été désactivé depuis le serveur.".to_string(),
        )),
        sessions::Etat::Inconnue => Err((
            CodeErreur::Authentification,
            "Session inconnue — se reconnecter.".to_string(),
        )),
    }
}

// =====================================================================
//  Canal
// =====================================================================

fn canal(srv: &Arc<Serveur>, req: &Requete, flux: &mut TcpStream) -> std::io::Result<()> {
    let appelant = match authentifier(srv, req) {
        Ok(a) => a,
        Err((code, message)) => return erreur(flux, 401, code, &message),
    };
    let depuis: u64 = req
        .parametres
        .get("depuis")
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| srv.canal.seq_courant());

    // La longue attente se fait SANS le verrou de la base : le tenir
    // trente secondes bloquerait toutes les caisses.
    let (evenements, seq_max) = srv.canal.depuis(depuis, &appelant.poste_id);
    let lot = LotEvenements {
        evenements,
        seq_max,
    };
    repondre_json(flux, 200, &serde_json::to_value(lot).unwrap_or(Value::Null))
}

// =====================================================================
//  Sauvegarde
// =====================================================================

fn sauvegarde_manuelle(
    srv: &Arc<Serveur>,
    req: &Requete,
    flux: &mut TcpStream,
) -> std::io::Result<()> {
    let appelant = match authentifier(srv, req) {
        Ok(a) => a,
        Err((code, message)) => return erreur(flux, 401, code, &message),
    };
    let ctx = ContexteUtilisateur {
        id: appelant.utilisateur_id,
        role: appelant.role,
    };
    // Le verrou se prend et se relache tout de suite : `sauvegarde` le
    // reprendra pour son propre compte, et le garder ici bloquerait les
    // caisses pendant toute la copie. Le controle passe par `Base`
    // (portee), sur les deux moteurs — un refus de permission doit
    // rester un refus de permission, meme la ou la sauvegarde
    // elle-meme n'est pas encore possible (D4).
    {
        let mut base = match srv.base.lock() {
            Ok(b) => b,
            Err(_) => return erreur(flux, 500, CodeErreur::Technique, "Base indisponible."),
        };
        if let Err(e) = verifier_permission_sur(&mut base, &ctx, "sauvegarde:lancer") {
            return erreur(flux, 403, CodeErreur::Permission, &e.to_string());
        }
    }

    match sauvegarde::maintenant(srv) {
        Ok(chemin) => repondre_json(flux, 200, &json!({ "etat": "ok", "fichier": chemin })),
        Err(e) => erreur(flux, 500, CodeErreur::Technique, &e),
    }
}

fn erreur(
    flux: &mut TcpStream,
    code_http: u16,
    code: CodeErreur,
    message: &str,
) -> std::io::Result<()> {
    let r = Reponse::erreur(code, message);
    repondre_json(
        flux,
        code_http,
        &serde_json::to_value(r).unwrap_or(Value::Null),
    )
}
