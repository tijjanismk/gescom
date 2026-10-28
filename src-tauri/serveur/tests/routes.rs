//! Les routes HTTP, par le vrai binaire.
//!
//! Les scénarios du noyau couvrent ce que les commandes font à la base ;
//! ils ne passent jamais par `api.rs`, ni par le JSON tel que l'écran
//! l'envoie. C'est ce trou qui a laissé passer « missing field
//! ligne_id » (un champ imbriqué en camelCase) et l'interblocage du
//! 13/09. Ici on lance `gescom-serveur.exe` sur une base jetable, port
//! libre, et on lui parle en HTTP brut, comme une caisse — sans client
//! HTTP, pour ne rien devoir à une dépendance.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

struct Serveur {
    enfant: Child,
    port: u16,
    base: std::path::PathBuf,
}

impl Drop for Serveur {
    fn drop(&mut self) {
        let _ = self.enfant.kill();
        let _ = self.enfant.wait();
        for suffixe in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffixe}", self.base.display()));
        }
        let _ = std::fs::remove_file(self.base.with_extension("log"));
    }
}

fn port_libre() -> u16 {
    TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

/// La recherche UDP du serveur (27/10/2026) : coupee par defaut dans
/// les essais (« 0 »), pour qu'aucun serveur de test ne prenne l'UDP 40000
/// a celui de la boutique ; les tests de la recherche lui donnent un port.
const DECOUVERTE: &str = "GESCOM_DECOUVERTE_PORT";

fn lancer() -> Serveur {
    lancer_avec(|_| {})
}

/// `lancer`, avec une retouche de la commande (variables, arguments).
fn lancer_avec(retouche: impl FnOnce(&mut Command)) -> Serveur {
    let port = port_libre();
    lancer_sur(port, true, retouche)
}

/// `avec_port` : `--hote 127.0.0.1 --port N` sur la ligne de commande
/// (comme la fenetre de Gescom, D39). Sans, le serveur lit son port dans
/// le `serveur.json` que la retouche lui designe (`GESCOM_SERVEUR_JSON`).
fn lancer_sur(port: u16, avec_port: bool, retouche: impl FnOnce(&mut Command)) -> Serveur {
    let base = std::env::temp_dir().join(format!("gescom_routes_{}_{port}.db", std::process::id()));
    let sauvegardes = std::env::temp_dir().join(format!("gescom_routes_sauv_{port}"));
    let mut commande = Command::new(env!("CARGO_BIN_EXE_gescom-serveur"));
    if avec_port {
        commande.args(["--hote", "127.0.0.1", "--port", &port.to_string()]);
    }
    commande
        .args(["--base", &base.to_string_lossy()])
        .args(["--sauvegardes", &sauvegardes.to_string_lossy()])
        // Des articles pour vendre : la demo.
        .env("GESCOM_DEMO", "1")
        .env(DECOUVERTE, "0")
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    retouche(&mut commande);
    let enfant = commande.spawn().expect("lancer gescom-serveur");
    let srv = Serveur { enfant, port, base };
    // L'amorçage d'une base neuve prend quelques secondes (bcrypt).
    let debut = Instant::now();
    loop {
        if let Ok((200, _)) = requete(port, "GET", "/sante", None, None) {
            break;
        }
        assert!(debut.elapsed() < Duration::from_secs(60), "le serveur ne répond pas sur /sante");
        std::thread::sleep(Duration::from_millis(300));
    }
    srv
}

/// Une requête HTTP/1.1 écrite à la main, réponse lue jusqu'à la
/// fermeture. Rend (code, corps JSON ou Null).
fn requete(port: u16, methode: &str, chemin: &str, corps: Option<&Value>, jeton: Option<&str>) -> std::io::Result<(u16, Value)> {
    let mut flux = TcpStream::connect(("127.0.0.1", port))?;
    flux.set_read_timeout(Some(Duration::from_secs(30)))?;
    let corps = corps.map(|c| c.to_string()).unwrap_or_default();
    let mut tete = format!(
        "{methode} {chemin} HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n",
        corps.len()
    );
    if let Some(j) = jeton {
        tete.push_str(&format!("Authorization: Bearer {j}\r\n"));
    }
    tete.push_str("\r\n");
    flux.write_all(tete.as_bytes())?;
    flux.write_all(corps.as_bytes())?;
    let mut brut = Vec::new();
    flux.read_to_end(&mut brut)?;
    let texte = String::from_utf8_lossy(&brut);
    let code: u16 = texte.split_whitespace().nth(1).and_then(|c| c.parse().ok()).unwrap_or(0);
    let corps = texte.split_once("\r\n\r\n").map(|(_, c)| c).unwrap_or("");
    Ok((code, serde_json::from_str(corps).unwrap_or(Value::Null)))
}

fn connexion(port: u16) -> (String, Value) {
    let (code, v) = requete(
        port, "POST", "/connexion",
        Some(&json!({
            "identifiant": "admin", "mot_de_passe": "admin123",
            "poste_nom": "Caisse test", "poste_empreinte": "routes-1", "version_protocole": 1
        })),
        None,
    ).unwrap();
    assert_eq!(code, 200, "{v}");
    (v["jeton"].as_str().unwrap().to_string(), v)
}

fn rpc(port: u16, jeton: &str, commande: &str, params: Value) -> (u16, Value) {
    requete(port, "POST", "/rpc", Some(&json!({ "commande": commande, "params": params })), Some(jeton)).unwrap()
}

#[test]
fn le_serveur_repond_connecte_refuse_sans_jeton_et_sert_une_commande() {
    let srv = lancer();
    let port = srv.port;

    let (jeton, identite) = connexion(port);
    assert_eq!(identite["role"], "patron");
    assert_eq!(identite["doit_changer_mdp"], true, "les comptes livrés exigent un nouveau mot de passe");
    // Un seul dossier : ouvert d'office, aucun choix à faire.
    assert!(identite["dossier_id"].is_string(), "{identite}");
    assert_eq!(identite["dossiers"].as_array().map(|d| d.len()), Some(1));

    // Sans jeton : 401 et un code que la caisse sait lire.
    let (code, v) = requete(port, "POST", "/rpc", Some(&json!({ "commande": "lire_dossiers", "params": {} })), None).unwrap();
    assert_eq!(code, 401, "{v}");
    assert_eq!(v["code"], "authentification");

    // Un mauvais mot de passe : même message qu'un identifiant inconnu.
    let (code, v) = requete(
        port, "POST", "/connexion",
        Some(&json!({ "identifiant": "admin", "mot_de_passe": "faux", "poste_nom": "x", "poste_empreinte": "x", "version_protocole": 1 })),
        None,
    ).unwrap();
    assert_eq!(code, 401);
    assert_eq!(v["message"], "Identifiant ou mot de passe incorrect");

    // Une commande inconnue le dit, sans deviner.
    let (code, v) = rpc(port, &jeton, "commande_qui_n_existe_pas", json!({}));
    assert_eq!(code, 404, "{v}");
    assert_eq!(v["code"], "commande_inconnue");

    // Une lecture ordinaire, avec le jeton.
    let (code, v) = rpc(port, &jeton, "lire_dossiers", json!({}));
    assert_eq!(code, 200, "{v}");
    assert_eq!(v["etat"], "ok");
    assert_eq!(v["donnee"][0]["code"], "PRINCIPAL");

    // La déconnexion révoque : le jeton ne vaut plus rien.
    let (code, _) = requete(port, "POST", "/deconnexion", Some(&json!({})), Some(&jeton)).unwrap();
    assert_eq!(code, 200);
    let (code, v) = rpc(port, &jeton, "lire_dossiers", json!({}));
    assert_eq!(code, 401, "{v}");

    // Tout ce qui a été refusé a laissé une ligne dans le journal
    // technique, à côté de la base, avec l'heure, le niveau et la
    // commande — c'est ce qui manquait pour comprendre un « erreur
    // technique » signalé par une caisse à 11 h.
    let journal = std::fs::read_to_string(srv.base.with_extension("log")).expect("journal technique");
    assert!(journal.contains("[INFO  ] Serveur démarré"), "{journal}");
    assert!(journal.contains("[REFUS ]") && journal.contains("401 · Identifiant ou mot de passe incorrect"), "{journal}");
    assert!(journal.contains("commande_qui_n_existe_pas") && journal.contains("404 ·"), "{journal}");
    // Une commande qui réussit ne laisse rien : le journal reste lisible.
    assert!(!journal.contains("lire_dossiers"), "{journal}");
    // Le contexte d'une commande : route, commande, utilisateur@poste.
    assert!(journal.contains("POST /rpc · commande_qui_n_existe_pas · "), "{journal}");
    // Jamais de mot de passe dedans (D10).
    assert!(!journal.contains("admin123"), "{journal}");
}

#[test]
fn une_livraison_passe_avec_le_json_de_l_ecran_et_le_stock_suit() {
    let srv = lancer();
    let port = srv.port;
    let (jeton, _) = connexion(port);

    // Un article de la démo, avec son unité.
    let (code, v) = rpc(port, &jeton, "lire_articles_avec_unites", json!({ "role": "patron" }));
    assert_eq!(code, 200, "{v}");
    let article = &v["donnee"][0];
    let article_id = article["id"].as_str().unwrap();
    let unite_id = article["unites"][0]["id"].as_str().unwrap();
    let stock_avant = article["stock"].as_f64().unwrap();

    // Un client réel, sinon pas de pièce.
    let (code, c) = rpc(port, &jeton, "creer_client_rapide", json!({ "nom": "Client des routes" }));
    assert_eq!(code, 200, "{c}");
    let client_id = c["donnee"]["id"].as_str().unwrap();

    // Un bon de livraison créé à la main : brouillon, rien ne bouge.
    let (code, bl) = rpc(port, &jeton, "creer_piece", json!({
        "clientId": client_id, "typePiece": "bon_livraison",
        "lignes": [{ "article_id": article_id, "unite_vente_id": unite_id, "quantite": 2.0, "prix_unitaire": 1000, "remise_pct": 0.0, "taux_tva": 0.0 }],
        "remiseGlobale": 0, "dateEcheance": null, "note": null, "pieceOrigineId": null, "depotId": null
    }));
    assert_eq!(code, 200, "{bl}");
    assert_eq!(bl["donnee"]["statut"], "brouillon");
    let bl_id = bl["donnee"]["id"].as_str().unwrap().to_string();

    // Livrer sur un brouillon : refus clair, et la réponse porte le code métier.
    let (code, lig) = rpc(port, &jeton, "lire_lignes_piece", json!({ "pieceId": bl_id }));
    assert_eq!(code, 200, "{lig}");
    let ligne_id = lig["donnee"][0]["id"].as_str().unwrap().to_string();
    let (code, v) = rpc(port, &jeton, "enregistrer_livraison", json!({
        "pieceId": bl_id, "lignes": [{ "ligne_id": ligne_id, "quantite_livree": 1.0 }]
    }));
    assert_eq!(code, 409, "{v}");
    assert!(v["message"].as_str().unwrap_or("").contains("émettre"), "{v}");

    // Émettre : tout sort.
    let (code, v) = rpc(port, &jeton, "changer_statut_piece", json!({ "pieceId": bl_id, "nouveauStatut": "emis" }));
    assert_eq!(code, 200, "{v}");
    let stock = |jeton: &str| -> f64 {
        let (_, v) = rpc(port, jeton, "lire_articles_avec_unites", json!({ "role": "patron" }));
        v["donnee"].as_array().unwrap().iter().find(|a| a["id"] == article_id).unwrap()["stock"].as_f64().unwrap()
    };
    assert_eq!(stock(&jeton), stock_avant - 2.0, "l'émission sort deux unités");

    // Le JSON de l'écran, en snake_case : un sac revient.
    let (code, v) = rpc(port, &jeton, "enregistrer_livraison", json!({
        "pieceId": bl_id, "lignes": [{ "ligne_id": ligne_id, "quantite_livree": 1.0 }]
    }));
    assert_eq!(code, 200, "{v}");
    assert_eq!(v["donnee"]["etat"], "partiel");
    assert_eq!(stock(&jeton), stock_avant - 1.0);

    // L'ancienne graphie camelCase passe aussi (« missing field ligne_id »).
    let (code, v) = rpc(port, &jeton, "enregistrer_livraison", json!({
        "pieceId": bl_id, "lignes": [{ "ligneId": ligne_id, "quantiteLivree": 2.0 }]
    }));
    assert_eq!(code, 200, "{v}");
    assert_eq!(v["donnee"]["etat"], "livre");
    assert_eq!(stock(&jeton), stock_avant - 2.0);

    // Une permission manquante se lit comme telle : l'employé ne crée pas de dossier.
    let (code, v) = requete(
        port, "POST", "/connexion",
        Some(&json!({ "identifiant": "employe", "mot_de_passe": "employe123", "poste_nom": "Caisse 2", "poste_empreinte": "routes-2", "version_protocole": 1 })),
        None,
    ).unwrap();
    assert_eq!(code, 200, "{v}");
    let employe = v["jeton"].as_str().unwrap().to_string();
    let (code, v) = rpc(port, &employe, "creer_dossier", json!({ "code": "X", "societe": "X" }));
    assert_eq!(code, 403, "{v}");
    assert_eq!(v["code"], "permission");
}

/// D26 et D27, par le vrai chemin : deux comptes du même rôle vendent,
/// chacun signe sa vente ; une quantité négative est refusée par le
/// serveur même si l'écran ne l'enverrait jamais.
#[test]
fn deux_comptes_du_meme_role_signent_chacun_leur_vente() {
    let srv = lancer();
    let port = srv.port;
    let (admin, _) = connexion(port);

    // Awa, un second compte « employe » à côté de celui de l'amorçage.
    let (code, v) = rpc(port, &admin, "creer_utilisateur", json!({
        "nom": "Awa", "pseudo": "awa", "email": null, "motDePasse": "awa-secret", "roleNom": "employe"
    }));
    assert_eq!(code, 200, "{v}");
    let se_connecter = |identifiant: &str, mdp: &str, poste: &str| -> (String, String) {
        let (code, v) = requete(
            port, "POST", "/connexion",
            Some(&json!({ "identifiant": identifiant, "mot_de_passe": mdp, "poste_nom": poste, "poste_empreinte": poste, "version_protocole": 1 })),
            None,
        ).unwrap();
        assert_eq!(code, 200, "{v}");
        (v["jeton"].as_str().unwrap().to_string(), v["utilisateur_id"].as_str().unwrap().to_string())
    };
    let (jeton_employe, id_employe) = se_connecter("employe", "employe123", "routes-e");
    let (jeton_awa, id_awa) = se_connecter("awa", "awa-secret", "routes-a");
    assert_ne!(id_employe, id_awa);

    let (_, a) = rpc(port, &admin, "lire_articles_avec_unites", json!({}));
    let article = &a["donnee"][0];
    let unite = &article["unites"][0];
    let (_, d) = rpc(port, &admin, "lire_depot_defaut", json!({}));
    let depot = d["donnee"]["id"].as_str().unwrap().to_string();
    let (_, c) = rpc(port, &admin, "lire_client_generique", json!({}));
    let client = c["donnee"]["id"].as_str().unwrap().to_string();
    // D35 : au « Comptant », le passant paie en entier — caisse ouverte,
    // le total encaisse.
    let (code, v) = rpc(port, &admin, "ouvrir_session_caisse", json!({ "fondOuverture": 0 }));
    assert_eq!(code, 200, "{v}");
    let prix = unite["prix_reference"].as_i64().unwrap();
    let vente = |quantite: f64| json!({
        "clientId": client, "depotId": depot, "modeReglement": "comptant",
        "montantPaye": (prix as f64 * quantite).round() as i64, "modePaiement": "especes",
        "lignes": [{
            "article_id": article["id"], "unite_vente_id": unite["id"], "depot_source_id": depot,
            "source_approvisionnement": "stock", "quantite": quantite, "facteur": unite["facteur"],
            "prix_reference": unite["prix_reference"], "prix_pratique": unite["prix_reference"]
        }]
    });

    // Awa vend en premier, puis le compte de l'amorçage.
    let (code, v) = rpc(port, &jeton_awa, "creer_vente", vente(1.0));
    assert_eq!(code, 200, "{v}");
    let vente_awa = v["donnee"]["vente_id"].as_str().unwrap().to_string();
    let (code, v) = rpc(port, &jeton_employe, "creer_vente", vente(1.0));
    assert_eq!(code, 200, "{v}");
    let vente_employe = v["donnee"]["vente_id"].as_str().unwrap().to_string();

    // D27 : une quantité négative, envoyée à la main, est refusée.
    let (code, v) = rpc(port, &jeton_awa, "creer_vente", vente(-3.0));
    assert_eq!(code, 409, "{v}");
    assert!(v["message"].as_str().unwrap_or("").contains("Quantité"), "{v}");

    let base = rusqlite::Connection::open(&srv.base).unwrap();
    let auteur = |vente: &str| -> String {
        base.query_row("SELECT auteur_id FROM vente WHERE id = ?1", [vente], |r| r.get(0)).unwrap()
    };
    assert_eq!(auteur(&vente_awa), id_awa, "la vente d'Awa est signée par Awa");
    assert_eq!(auteur(&vente_employe), id_employe);
}

#[test]
fn l_historique_se_lit_avec_journal_lire_et_nomme_qui_a_vendu() {
    let srv = lancer();
    let port = srv.port;
    let (admin, _) = connexion(port);

    let (code, _) = rpc(port, &admin, "creer_utilisateur", json!({
        "nom": "Awa Traoré", "pseudo": "awa", "email": null, "motDePasse": "awa-secret", "roleNom": "employe"
    }));
    assert_eq!(code, 200);
    let (code, v) = requete(
        port, "POST", "/connexion",
        Some(&json!({ "identifiant": "awa", "mot_de_passe": "awa-secret", "poste_nom": "h", "poste_empreinte": "routes-h", "version_protocole": 1 })),
        None,
    ).unwrap();
    assert_eq!(code, 200, "{v}");
    let awa = v["jeton"].as_str().unwrap().to_string();

    let (_, a) = rpc(port, &admin, "lire_articles_avec_unites", json!({}));
    let article = &a["donnee"][0];
    let unite = &article["unites"][0];
    let (_, d) = rpc(port, &admin, "lire_depot_defaut", json!({}));
    let depot = d["donnee"]["id"].as_str().unwrap().to_string();
    let (_, c) = rpc(port, &admin, "lire_client_generique", json!({}));
    let client = c["donnee"]["id"].as_str().unwrap().to_string();
    // D35 : au « Comptant », le passant paie en entier.
    let (code, v) = rpc(port, &admin, "ouvrir_session_caisse", json!({ "fondOuverture": 0 }));
    assert_eq!(code, 200, "{v}");
    let (code, v) = rpc(port, &awa, "creer_vente", json!({
        "clientId": client, "depotId": depot, "modeReglement": "comptant",
        "montantPaye": unite["prix_reference"], "modePaiement": "especes",
        "lignes": [{
            "article_id": article["id"], "unite_vente_id": unite["id"], "depot_source_id": depot,
            "source_approvisionnement": "stock", "quantite": 1.0, "facteur": unite["facteur"],
            "prix_reference": unite["prix_reference"], "prix_pratique": unite["prix_reference"]
        }]
    }));
    assert_eq!(code, 200, "{v}");

    // Le JSON tel que l'écran l'envoie : `filtre`, champs vides à null.
    let filtre = |recherche: &str| json!({ "filtre": {
        "du": null, "au": null, "auteur_id": null, "type_evenement": "vente_creee",
        "tiers_id": null, "piece_id": null, "article_id": null,
        "recherche": recherche, "page": 0, "par_page": 50
    }});
    let (code, v) = rpc(port, &admin, "lire_historique", filtre("awa"));
    assert_eq!(code, 200, "{v}");
    let lignes = v["donnee"]["lignes"].as_array().unwrap();
    assert_eq!(lignes.len(), 1, "la vente d'Awa, trouvée par son nom : {v}");
    assert_eq!(lignes[0]["auteur_nom"], "Awa Traoré");
    assert_eq!(lignes[0]["libelle_type"], "Vente");

    let (code, v) = rpc(port, &admin, "lire_filtres_historique", json!({}));
    assert_eq!(code, 200, "{v}");
    assert!(v["donnee"]["auteurs"].as_array().unwrap().iter().any(|a| a["nom"] == "Awa Traoré"));

    // Un employé n'a pas `journal:lire` : le serveur refuse, quel que
    // soit l'écran.
    let (code, v) = rpc(port, &awa, "lire_historique", filtre(""));
    assert_ne!(code, 200, "{v}");
    assert!(v.to_string().contains("journal:lire"), "le refus nomme la permission : {v}");
    let (code, _) = rpc(port, &awa, "lire_filtres_historique", json!({}));
    assert_ne!(code, 200);
}

#[test]
fn les_erreurs_d_une_caisse_arrivent_au_journal_et_la_onzieme_est_jetee() {
    let srv = lancer();
    let port = srv.port;
    let (jeton, _) = connexion(port);

    // Sans jeton : refusé, et rien d'écrit sous [POSTE ].
    let (code, _) = requete(port, "POST", "/journal-poste", Some(&json!({ "message": "anonyme" })), None).unwrap();
    assert_eq!(code, 401);

    // Plus de 4 Ko : refusé sans être lu.
    let gros = json!({ "page": "ventes", "message": "x".repeat(5000) });
    let (code, _) = requete(port, "POST", "/journal-poste", Some(&gros), Some(&jeton)).unwrap();
    assert_eq!(code, 413);

    // Dix passent — dont une qui essaie d'écrire une fausse ligne.
    for i in 0..10 {
        let message = if i == 0 {
            "TypeError: boum\n2026-01-01T00:00:00 [ERREUR] ligne fabriquée".to_string()
        } else {
            format!("erreur n°{i}")
        };
        let (code, v) = requete(port, "POST", "/journal-poste",
            Some(&json!({ "page": "ventes", "message": message, "pile": "at f (Ventes.tsx:12)", "genre": "erreur" })),
            Some(&jeton)).unwrap();
        assert_eq!(code, 200, "envoi {i} : {v}");
    }
    // La onzième de la minute est jetée ; la douzième aussi, sans ligne.
    let (code, v) = requete(port, "POST", "/journal-poste", Some(&json!({ "message": "onze" })), Some(&jeton)).unwrap();
    assert_eq!(code, 429, "{v}");
    let (code, _) = requete(port, "POST", "/journal-poste", Some(&json!({ "message": "douze" })), Some(&jeton)).unwrap();
    assert_eq!(code, 429);

    let journal = std::fs::read_to_string(srv.base.with_extension("log")).expect("journal technique");
    let postes: Vec<&str> = journal.lines().filter(|l| l.contains("[POSTE ]")).collect();
    assert_eq!(postes.len(), 10, "{journal}");
    assert!(postes[0].contains("POST /journal-poste · Caisse test · ventes · TypeError: boum"), "{}", postes[0]);
    assert!(postes[0].contains("pile : at f (Ventes.tsx:12)"), "{}", postes[0]);
    assert!(!journal.lines().any(|l| l.starts_with("2026-01-01")), "aucune fausse ligne : {journal}");
    assert_eq!(journal.matches("les suivantes sont jetées").count(), 1, "le trop-plein est dit une fois : {journal}");
    assert!(!journal.contains("onze") && !journal.contains("douze"), "{journal}");
}

#[test]
fn le_journal_technique_se_lit_depuis_la_console_avec_la_permission_de_sauvegarde() {
    let srv = lancer();
    let port = srv.port;
    let (admin, _) = connexion(port);

    // Un refus, pour avoir une ligne REFUS.
    let (code, _) = rpc(port, &admin, "commande_qui_n_existe_pas", json!({}));
    assert_eq!(code, 404);

    let (code, v) = requete(port, "GET", "/journal", None, None).unwrap();
    assert_eq!(code, 401, "{v}");

    let (code, v) = requete(port, "GET", "/journal?n=200", None, Some(&admin)).unwrap();
    assert_eq!(code, 200, "{v}");
    let lignes = v["lignes"].as_array().unwrap();
    assert!(lignes.iter().any(|l| l.as_str().unwrap().contains("Serveur démarré")), "{v}");
    assert!(lignes.len() <= 200);

    let (code, v) = requete(port, "GET", "/journal?niveau=REFUS", None, Some(&admin)).unwrap();
    assert_eq!(code, 200, "{v}");
    let refus = v["lignes"].as_array().unwrap();
    assert!(!refus.is_empty());
    assert!(refus.iter().all(|l| l.as_str().unwrap().contains("[REFUS ]")), "{v}");
    assert!(refus.iter().any(|l| l.as_str().unwrap().contains("commande_qui_n_existe_pas")), "{v}");

    let (code, _) = requete(port, "GET", "/journal?niveau=BAVARD", None, Some(&admin)).unwrap();
    assert_eq!(code, 400);

    // L'employé n'a pas `sauvegarde:lancer` : pas de journal.
    let (code, v) = requete(
        port, "POST", "/connexion",
        Some(&json!({ "identifiant": "employe", "mot_de_passe": "employe123", "poste_nom": "j", "poste_empreinte": "routes-j", "version_protocole": 1 })),
        None,
    ).unwrap();
    assert_eq!(code, 200, "{v}");
    let employe = v["jeton"].as_str().unwrap().to_string();
    let (code, v) = requete(port, "GET", "/journal", None, Some(&employe)).unwrap();
    assert_eq!(code, 403, "{v}");

    // La console porte l'onglet, et écrit les lignes en texte.
    let mut flux = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    flux.write_all(b"GET / HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n").unwrap();
    let mut page = String::new();
    flux.read_to_string(&mut page).unwrap();
    assert!(page.contains("id=\"carte-journal\"") && page.contains("d.textContent = l"), "la console a son journal");
}

#[test]
fn le_caissier_recoit_prix_achat_null_et_pas_le_tableau_de_bord() {
    let srv = lancer();
    let port = srv.port;
    let (admin, _) = connexion(port);
    let (code, v) = rpc(port, &admin, "creer_utilisateur", json!({
        "nom": "Fanta", "pseudo": "fanta", "email": null, "motDePasse": "fanta-secret", "roleNom": "caissier"
    }));
    assert_eq!(code, 200, "{v}");
    let (code, v) = requete(
        port, "POST", "/connexion",
        Some(&json!({ "identifiant": "fanta", "mot_de_passe": "fanta-secret", "poste_nom": "c", "poste_empreinte": "routes-c1", "version_protocole": 1 })),
        None,
    ).unwrap();
    assert_eq!(code, 200, "{v}");
    let caissier = v["jeton"].as_str().unwrap().to_string();

    // Le patron lit le coût ; le caissier reçoit null — pas un zéro.
    let (code, v) = rpc(port, &admin, "lire_etat_stock", json!({}));
    assert_eq!(code, 200, "{v}");
    assert!(v["donnee"]["lignes"][0]["prix_achat"].is_i64(), "{v}");
    let (code, v) = rpc(port, &caissier, "lire_etat_stock", json!({}));
    assert_eq!(code, 200, "{v}");
    assert!(v["donnee"]["lignes"][0]["prix_achat"].is_null(), "{v}");
    assert!(v["donnee"]["valeur_totale"].is_null(), "{v}");
    assert!(v["donnee"]["lignes"][0]["quantite"].is_number(), "le stock se lit : {v}");
    let (_, v) = rpc(port, &caissier, "lire_articles_avec_unites", json!({}));
    assert!(v["donnee"].as_array().unwrap().iter().all(|a| a["dernier_prix_achat"].is_null()), "{v}");

    // Le tableau de bord chiffré : refusé, et le refus nomme la permission.
    let (code, v) = rpc(port, &caissier, "lire_resume_dashboard", json!({}));
    assert_eq!(code, 403, "{v}");
    assert!(v.to_string().contains("rapports:lire"), "{v}");
    let (code, _) = rpc(port, &admin, "lire_resume_dashboard", json!({}));
    assert_eq!(code, 200);

    // Ce que doivent les clients : masqué dans la liste, refusé en relevé.
    let (code, v) = rpc(port, &caissier, "lire_clients_pagines", json!({ "page": 0, "limite": 20, "avecCreancesSeulement": true, "tri": "creance" }));
    assert_eq!(code, 200, "{v}");
    let donnees = v["donnee"]["donnees"].as_array().unwrap();
    assert!(!donnees.is_empty(), "le filtre « avec dette seulement » est neutralisé : {v}");
    assert!(donnees.iter().all(|c| c["total_creances"].is_null()), "{v}");
    let (code, v) = rpc(port, &caissier, "lire_etat_creances_global", json!({}));
    assert_eq!(code, 403, "{v}");
}

#[test]
fn le_plafond_du_caissier_se_juge_sur_l_argument_de_la_vente() {
    let srv = lancer();
    let port = srv.port;
    let (admin, _) = connexion(port);
    let (code, v) = rpc(port, &admin, "creer_utilisateur", json!({
        "nom": "Fanta", "pseudo": "fanta", "email": null, "motDePasse": "fanta-secret", "roleNom": "caissier"
    }));
    assert_eq!(code, 200, "{v}");
    let (code, v) = rpc(port, &admin, "definir_plafonds_role", json!({
        "role": "caissier", "plafonds": { "remise_max_pct": 15.0, "remboursement_max": 50000, "credit_max": 1 }
    }));
    assert_eq!(code, 200, "{v}");
    let (code, v) = requete(
        port, "POST", "/connexion",
        Some(&json!({ "identifiant": "fanta", "mot_de_passe": "fanta-secret", "poste_nom": "p", "poste_empreinte": "routes-c3", "version_protocole": 1 })),
        None,
    ).unwrap();
    assert_eq!(code, 200, "{v}");
    let caissier = v["jeton"].as_str().unwrap().to_string();

    let (_, a) = rpc(port, &admin, "lire_articles_avec_unites", json!({}));
    let article = &a["donnee"][0];
    let unite = &article["unites"][0];
    let prix = unite["prix_reference"].as_i64().unwrap();
    let (_, d) = rpc(port, &admin, "lire_depot_defaut", json!({}));
    let depot = d["donnee"]["id"].as_str().unwrap().to_string();
    let (_, c) = rpc(port, &admin, "lire_clients", json!({}));
    let client = c["donnee"].as_array().unwrap().iter().find(|x| x["code"] != "CLIENT00000")
        .map(|x| x["id"].as_str().unwrap().to_string()).unwrap();
    let vente = |pratique: i64, mode: &str| json!({
        "clientId": client, "depotId": depot, "modeReglement": mode,
        "lignes": [{
            "article_id": article["id"], "unite_vente_id": unite["id"], "depot_source_id": depot,
            "source_approvisionnement": "stock", "quantite": 1.0, "facteur": unite["facteur"],
            "prix_reference": prix, "prix_pratique": pratique
        }]
    });

    // 40 % : refusé, le message dit le plafond et quoi faire.
    let (code, v) = rpc(port, &caissier, "creer_vente", vente(prix * 60 / 100, "credit"));
    assert_eq!(code, 409, "{v}");
    assert!(v["message"].as_str().unwrap().contains("votre plafond est 15 %. Demander au patron."), "{v}");
    // Crédit au-delà de 1 F : refusé aussi.
    let (code, v) = rpc(port, &caissier, "creer_vente", vente(prix, "credit"));
    assert_eq!(code, 409, "{v}");
    assert!(v["message"].as_str().unwrap().starts_with("Crédit de"), "{v}");
    // Le patron n'a pas de plafond.
    let (code, v) = rpc(port, &admin, "creer_vente", vente(prix * 60 / 100, "credit"));
    assert_eq!(code, 200, "{v}");
    // Rien n'a été écrit pour la caissière.
    let base = rusqlite::Connection::open(&srv.base).unwrap();
    let n: i64 = base.query_row("SELECT COUNT(*) FROM vente", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 1, "seule la vente du patron");
}

#[test]
fn une_sauvegarde_se_restaure_hors_ligne_et_le_serveur_repart_dessus() {
    // Le diagnostic disait « restaurer la dernière sauvegarde » et rien ne
    // le faisait. Ici : on sauvegarde, on écrit encore, on restaure
    // serveur arrêté, on relance — l'écriture d'après a disparu, la base
    // d'avant est gardée à côté.
    let mut srv = lancer();
    let port = srv.port;
    let (jeton, _) = connexion(port);

    let (code, v) = requete(port, "POST", "/sauvegarde", Some(&json!({})), Some(&jeton)).unwrap();
    assert_eq!(code, 200, "{v}");
    let fichier = v["fichier"].as_str().unwrap().to_string();
    assert!(std::path::Path::new(&fichier).exists(), "la sauvegarde est un fichier : {fichier}");

    let (code, c) = rpc(port, &jeton, "creer_client_rapide", json!({ "nom": "Ecrit apres la sauvegarde" }));
    assert_eq!(code, 200, "{c}");

    // Serveur arrêté, restauration par le binaire.
    let _ = srv.enfant.kill();
    let _ = srv.enfant.wait();
    let sortie = Command::new(env!("CARGO_BIN_EXE_gescom-serveur"))
        .args(["--restaurer", &fichier, "--base", &srv.base.to_string_lossy()])
        .output()
        .expect("lancer --restaurer");
    let texte = String::from_utf8_lossy(&sortie.stdout);
    assert!(sortie.status.success(), "{texte}{}", String::from_utf8_lossy(&sortie.stderr));
    assert!(texte.contains("restaurée"), "{texte}");
    assert!(texte.contains("gardée"), "la base d'avant est mise de côté : {texte}");

    // Une sauvegarde qui n'en est pas une est refusée, et rien ne bouge.
    let faux = std::env::temp_dir().join(format!("pas_une_base_{port}.db"));
    std::fs::write(&faux, b"bonjour").unwrap();
    let sortie = Command::new(env!("CARGO_BIN_EXE_gescom-serveur"))
        .args(["--restaurer", &faux.to_string_lossy(), "--base", &srv.base.to_string_lossy()])
        .output()
        .unwrap();
    assert!(!sortie.status.success());
    assert!(String::from_utf8_lossy(&sortie.stderr).contains("Restauration impossible"));
    let _ = std::fs::remove_file(&faux);

    // On relance sur la base restaurée : le client d'après n'y est pas.
    srv.enfant = Command::new(env!("CARGO_BIN_EXE_gescom-serveur"))
        .args(["--hote", "127.0.0.1", "--port", &port.to_string()])
        .args(["--base", &srv.base.to_string_lossy()])
        .env(DECOUVERTE, "0")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let debut = Instant::now();
    while requete(port, "GET", "/sante", None, None).map(|(c, _)| c).unwrap_or(0) != 200 {
        assert!(debut.elapsed() < Duration::from_secs(60));
        std::thread::sleep(Duration::from_millis(300));
    }
    let (jeton, _) = connexion(port);
    let (code, v) = rpc(port, &jeton, "lire_clients", json!({}));
    assert_eq!(code, 200, "{v}");
    let noms: Vec<&str> = v["donnee"].as_array().unwrap().iter().filter_map(|c| c["nom"].as_str()).collect();
    assert!(!noms.contains(&"Ecrit apres la sauvegarde"), "{noms:?}");

    // Ménage : la copie « avant restauration » et la sauvegarde.
    if let Some(parent) = srv.base.parent() {
        for e in std::fs::read_dir(parent).unwrap().flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if n.contains("avant-restauration") && n.starts_with(&format!("gescom_routes_{}", std::process::id())) {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    let _ = std::fs::remove_dir_all(std::env::temp_dir().join(format!("gescom_routes_sauv_{port}")));
}

#[test]
fn une_ecriture_hors_des_dates_de_travail_est_refusee_avant_d_ecrire() {
    // v3, D-4 (D21) : le garde-fou juge la date de l'écriture — donnée,
    // ou aujourd'hui — contre les exercices du dossier de la session.
    let srv = lancer();
    let port = srv.port;
    let (admin, _) = connexion(port);

    let (_, a) = rpc(port, &admin, "lire_articles_avec_unites", json!({}));
    let article = &a["donnee"][0];
    let unite = &article["unites"][0];
    let (_, d) = rpc(port, &admin, "lire_depot_defaut", json!({}));
    let depot = d["donnee"]["id"].as_str().unwrap().to_string();
    let (_, c) = rpc(port, &admin, "lire_clients", json!({}));
    let client = c["donnee"][0]["id"].as_str().unwrap().to_string();
    let vente = |date: Option<&str>| json!({
        "clientId": client, "depotId": depot, "modeReglement": "credit", "dateVente": date,
        "lignes": [{
            "article_id": article["id"], "unite_vente_id": unite["id"], "depot_source_id": depot,
            "source_approvisionnement": "stock", "quantite": 1.0, "facteur": unite["facteur"],
            "prix_reference": unite["prix_reference"], "prix_pratique": unite["prix_reference"]
        }]
    });

    // Une date donnée, avant l'exercice : le refus nomme le début.
    let (code, v) = rpc(port, &admin, "creer_vente", vente(Some("2020-05-01")));
    assert_eq!(code, 409, "{v}");
    assert!(v["message"].as_str().unwrap().starts_with("Le 1er mai 2020 est avant les dates de travail"), "{v}");

    // L'exercice en cours ne se clôt pas (D38) : il arrêterait toutes
    // les ventes sur-le-champ, sans remède.
    let (_, ex) = rpc(port, &admin, "lire_exercices", json!({}));
    let id = ex["donnee"][0]["id"].as_str().unwrap().to_string();
    let (code, v) = rpc(port, &admin, "clore_exercice", json!({ "exerciceId": id }));
    assert_eq!(code, 409, "{v}");
    assert!(v["message"].as_str().unwrap().contains("qu'un exercice terminé"), "{v}");

    // Le temps passe : l'exercice se range dans le passé, comme au
    // lendemain de sa fin. Aujourd'hui ne s'écrit plus.
    rusqlite::Connection::open(&srv.base)
        .unwrap()
        .execute("UPDATE exercice SET date_debut = '2020-01-01', date_fin = '2020-12-31' WHERE id = ?1", [&id])
        .unwrap();
    let (code, v) = rpc(port, &admin, "creer_vente", vente(None));
    assert_eq!(code, 409, "{v}");
    assert!(v["message"].as_str().unwrap().contains("hors des dates de travail (jusqu'au 31 décembre 2020)"), "{v}");
    // Terminé, il se clôt ; plus aucun exercice ouvert.
    let (code, v) = rpc(port, &admin, "clore_exercice", json!({ "exerciceId": id }));
    assert_eq!(code, 200, "{v}");
    let (code, v) = rpc(port, &admin, "creer_vente", vente(None));
    assert_eq!(code, 409, "{v}");
    assert!(v["message"].as_str().unwrap().contains("aucun exercice ouvert"), "{v}");
    let (code, v) = rpc(port, &admin, "enregistrer_depense", json!({ "montant": 1000, "motif": "x", "categorie": "divers" }));
    assert_eq!(code, 409, "même sans date donnée : {v}");

    // Ce qui n'est pas une écriture datée passe toujours.
    let (code, v) = rpc(port, &admin, "creer_client_rapide", json!({ "nom": "Toujours possible" }));
    assert_eq!(code, 200, "{v}");

    let base = rusqlite::Connection::open(&srv.base).unwrap();
    let n: i64 = base.query_row("SELECT COUNT(*) FROM vente", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 0, "rien n'a été écrit");
    let n: i64 = base
        .query_row("SELECT COUNT(*) FROM journal WHERE type_evenement = 'exercice_clos'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 1, "la clôture est au journal");
}

/// Lot C.5 : l'ordinateur qui garde la base repart en arrière (pile,
/// coupure) — ou avait été avancé par erreur puis remis à l'heure. Les
/// écritures sont refusées (409), les lectures passent, et seul le
/// patron lève la garde. Retirer la garde d'`api.rs` fait échouer ce test.
#[test]
fn une_horloge_qui_recule_refuse_les_ecritures_et_le_patron_la_leve() {
    let srv = lancer();
    let port = srv.port;
    let (admin, _) = connexion(port);
    let (code, v) = rpc(port, &admin, "creer_utilisateur", json!({
        "nom": "Fanta", "pseudo": "fanta", "email": null, "motDePasse": "fanta-secret", "roleNom": "caissier"
    }));
    assert_eq!(code, 200, "{v}");
    let (code, v) = requete(
        port, "POST", "/connexion",
        Some(&json!({ "identifiant": "fanta", "mot_de_passe": "fanta-secret", "poste_nom": "c", "poste_empreinte": "routes-h1", "version_protocole": 1 })),
        None,
    ).unwrap();
    assert_eq!(code, 200, "{v}");
    let caissier = v["jeton"].as_str().unwrap().to_string();

    // Une écriture réussie pose la référence, en base.
    let (code, v) = rpc(port, &admin, "creer_client_rapide", json!({ "nom": "Avant la panne" }));
    assert_eq!(code, 200, "{v}");
    let lire_reference = || -> String {
        rusqlite::Connection::open(&srv.base)
            .unwrap()
            .query_row("SELECT valeur FROM config_app WHERE cle = 'horloge_derniere_ecriture'", [], |r| r.get(0))
            .unwrap()
    };
    let aujourd_hui = chrono::Local::now().format("%Y-%m-%d").to_string();
    assert!(lire_reference().starts_with(&aujourd_hui), "{}", lire_reference());

    // La dernière écriture est restée dans le futur (horloge avancée
    // puis corrigée) : pour la machine, l'heure a reculé.
    rusqlite::Connection::open(&srv.base)
        .unwrap()
        .execute("UPDATE config_app SET valeur = '2099-01-01T08:00:00.000' WHERE cle = 'horloge_derniere_ecriture'", [])
        .unwrap();
    let clients = || -> i64 {
        rusqlite::Connection::open(&srv.base).unwrap().query_row("SELECT COUNT(*) FROM client", [], |r| r.get(0)).unwrap()
    };
    let avant = clients();
    let (code, v) = rpc(port, &admin, "creer_client_rapide", json!({ "nom": "Pendant la panne" }));
    assert_eq!(code, 409, "{v}");
    let m = v["message"].as_str().unwrap();
    assert!(m.starts_with("HORLOGE_RECULE — "), "{m}");
    assert!(m.contains("dernière écriture le 01/01/2099 à 08:00"), "{m}");
    assert!(m.contains("Paramètres → Horloge"), "{m}");
    assert_eq!(clients(), avant, "rien n'a été écrit");
    // Toute écriture, pas seulement les datées ; et pour tout le monde.
    let (code, _) = rpc(port, &caissier, "creer_client_rapide", json!({ "nom": "Pendant la panne" }));
    assert_eq!(code, 409);

    // Les lectures continuent.
    let (code, v) = rpc(port, &caissier, "lire_clients", json!({}));
    assert_eq!(code, 200, "{v}");
    let (code, v) = rpc(port, &admin, "lire_horloge", json!({}));
    assert_eq!(code, 200, "{v}");
    assert_eq!(v["donnee"]["bloque"], true, "{v}");

    // Le geste est au patron : la caissière est refusée sur la permission.
    let (code, v) = rpc(port, &caissier, "confirmer_horloge", json!({}));
    assert_eq!(code, 403, "{v}");
    assert!(v.to_string().contains("parametres:modifier"), "{v}");
    let (code, v) = rpc(port, &admin, "confirmer_horloge", json!({}));
    assert_eq!(code, 200, "{v}");
    assert_eq!(v["donnee"]["ancienne"], "2099-01-01T08:00:00.000");
    assert!(lire_reference().starts_with(&aujourd_hui), "la référence revient à maintenant");

    // Les écritures reprennent.
    let (code, v) = rpc(port, &caissier, "creer_client_rapide", json!({ "nom": "Après la confirmation" }));
    assert_eq!(code, 200, "{v}");
    let n: i64 = rusqlite::Connection::open(&srv.base)
        .unwrap()
        .query_row(
            "SELECT COUNT(*) FROM journal WHERE type_evenement = 'horloge_confirmee' AND ancien_valeur = '2099-01-01T08:00:00.000'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(n, 1, "le geste est au journal, avec l'ancienne valeur");
}

/// Lot C.7 : les lignes d'une pièce fournisseur, lues pour la modifier,
/// rendaient le prix d'achat à tous. Le caissier reçoit null ; le
/// patron et les pièces client gardent leurs prix.
#[test]
fn les_lignes_d_une_piece_fournisseur_cachent_le_prix_d_achat_au_caissier() {
    let srv = lancer();
    let port = srv.port;
    let (admin, _) = connexion(port);
    let (code, v) = rpc(port, &admin, "creer_utilisateur", json!({
        "nom": "Fanta", "pseudo": "fanta", "email": null, "motDePasse": "fanta-secret", "roleNom": "caissier"
    }));
    assert_eq!(code, 200, "{v}");
    let (code, v) = requete(
        port, "POST", "/connexion",
        Some(&json!({ "identifiant": "fanta", "mot_de_passe": "fanta-secret", "poste_nom": "c", "poste_empreinte": "routes-p1", "version_protocole": 1 })),
        None,
    ).unwrap();
    assert_eq!(code, 200, "{v}");
    let caissier = v["jeton"].as_str().unwrap().to_string();

    let (_, a) = rpc(port, &admin, "lire_articles_avec_unites", json!({}));
    let article = &a["donnee"][0];
    let unite = &article["unites"][0];
    let ligne = json!([{
        "article_id": article["id"], "unite_vente_id": unite["id"], "quantite": 4.0,
        "prix_unitaire": 750, "remise_pct": 0.0, "taux_tva": 0.0
    }]);
    let (code, f) = rpc(port, &admin, "creer_fournisseur", json!({ "nom": "Grossiste du marché" }));
    assert_eq!(code, 200, "{f}");
    let (code, bcf) = rpc(port, &admin, "creer_piece_fournisseur", json!({
        "fournisseurId": f["donnee"]["id"], "typePiece": "bon_commande_fournisseur", "lignes": ligne
    }));
    assert_eq!(code, 200, "{bcf}");
    let bcf_id = bcf["donnee"]["id"].as_str().unwrap().to_string();
    let (_, c) = rpc(port, &admin, "lire_clients", json!({}));
    let client = c["donnee"].as_array().unwrap().iter().find(|x| x["code"] != "CLIENT00000")
        .map(|x| x["id"].as_str().unwrap().to_string()).unwrap();
    let (code, devis) = rpc(port, &admin, "creer_piece", json!({ "clientId": client, "typePiece": "devis", "lignes": ligne }));
    assert_eq!(code, 200, "{devis}");
    let devis_id = devis["donnee"]["id"].as_str().unwrap().to_string();

    // Le patron lit le prix d'achat.
    let (code, v) = rpc(port, &admin, "lire_lignes_piece", json!({ "pieceId": bcf_id }));
    assert_eq!(code, 200, "{v}");
    assert_eq!(v["donnee"][0]["prix_unitaire"], 750, "{v}");
    // Le caissier : la quantité, pas le prix.
    let (code, v) = rpc(port, &caissier, "lire_lignes_piece", json!({ "pieceId": bcf_id }));
    assert_eq!(code, 200, "{v}");
    let l = &v["donnee"][0];
    assert!(l["prix_unitaire"].is_null() && l["montant_ht"].is_null(), "{v}");
    assert_eq!(l["quantite"], 4.0, "{v}");
    // Un devis client garde ses prix : c'est lui qui vend.
    let (code, v) = rpc(port, &caissier, "lire_lignes_piece", json!({ "pieceId": devis_id }));
    assert_eq!(code, 200, "{v}");
    assert_eq!(v["donnee"][0]["prix_unitaire"], 750, "{v}");
}

const ORIGINE: &str = "00000000-0000-0000-0000-000000000001";

#[test]
fn le_frere_n_ouvre_que_sa_quincaillerie_et_la_comptable_choisit() {
    // v3, C-2 (décision C2) : un rôle par dossier, relu à chaque requête.
    let srv = lancer();
    let port = srv.port;
    let (admin, _) = connexion(port);
    let (code, d) = rpc(port, &admin, "creer_dossier", json!({ "code": "QUINC", "societe": "Quincaillerie du frère" }));
    assert_eq!(code, 200, "{d}");
    let quinc = d["donnee"]["id"].as_str().unwrap().to_string();
    let creer = |pseudo: &str, role: &str| {
        let (code, v) = rpc(port, &admin, "creer_utilisateur", json!({
            "nom": pseudo, "pseudo": pseudo, "email": null, "motDePasse": "secret-123", "roleNom": role
        }));
        assert_eq!(code, 200, "{v}");
    };
    creer("frere", "patron");
    creer("awa", "comptable");
    creer("moussa", "caissier");
    let (_, u) = rpc(port, &admin, "lire_utilisateurs", json!({}));
    let id = |pseudo: &str| u["donnee"].as_array().unwrap().iter()
        .find(|x| x["pseudo"] == pseudo || x["nom"] == pseudo)
        .map(|x| x["id"].as_str().unwrap().to_string()).expect(pseudo);
    let (frere, awa) = (id("frere"), id("awa"));
    let (code, v) = rpc(port, &admin, "definir_dossiers_utilisateur", json!({
        "utilisateurId": frere, "dossiers": [{ "dossier_id": quinc, "role": "patron" }]
    }));
    assert_eq!(code, 200, "{v}");
    let (code, v) = rpc(port, &admin, "definir_dossiers_utilisateur", json!({
        "utilisateurId": awa, "dossiers": [{ "dossier_id": ORIGINE, "role": "comptable" }, { "dossier_id": quinc, "role": "caissier" }]
    }));
    assert_eq!(code, 200, "{v}");

    let entrer = |pseudo: &str, dossier: Option<&str>| requete(
        port, "POST", "/connexion",
        Some(&json!({ "identifiant": pseudo, "mot_de_passe": "secret-123", "poste_nom": pseudo,
                      "poste_empreinte": format!("routes-c2-{pseudo}"), "version_protocole": 1, "dossier_id": dossier })),
        None,
    ).unwrap();

    // Le frère : un seul dossier ouvert, le sien, ouvert d'office, patron.
    let (code, v) = entrer("frere", None);
    assert_eq!(code, 200, "{v}");
    assert_eq!(v["dossier_id"], quinc.as_str(), "{v}");
    assert_eq!(v["dossiers"].as_array().unwrap().len(), 1, "il ne voit que le sien : {v}");
    assert_eq!(v["role"], "patron");
    let perms: Vec<&str> = v["permissions"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
    assert!(perms.contains(&"ventes:creer") && !perms.contains(&"utilisateurs:gerer"), "{perms:?}");
    let jeton_frere = v["jeton"].as_str().unwrap().to_string();
    let (_, c) = rpc(port, &jeton_frere, "lire_clients", json!({}));
    assert_eq!(c["donnee"].as_array().unwrap().len(), 1, "ses clients : son client de passage, pas les tiens");
    let (code, v) = rpc(port, &jeton_frere, "creer_utilisateur", json!({
        "nom": "x", "pseudo": "x", "email": null, "motDePasse": "secret-123", "roleNom": "patron"
    }));
    assert_eq!(code, 403, "il ne se crée pas un compte qui verrait ta boutique : {v}");
    // Ta boutique, demandée : refusée.
    let (code, v) = entrer("frere", Some(ORIGINE));
    assert_eq!(code, 403, "{v}");

    // La comptable : deux dossiers, elle choisit ; le rôle suit le dossier.
    let (code, v) = entrer("awa", None);
    assert_eq!(code, 200, "{v}");
    assert!(v["dossier_id"].is_null() && v["dossiers"].as_array().unwrap().len() == 2, "{v}");
    let jeton_awa = v["jeton"].as_str().unwrap().to_string();
    let (code, v) = rpc(port, &jeton_awa, "choisir_dossier", json!({ "dossierId": quinc }));
    assert_eq!(code, 200, "{v}");
    assert_eq!(v["donnee"]["role"], "caissier", "{v}");
    assert!(!v["donnee"]["permissions"].as_array().unwrap().iter().any(|p| p == "journal:lire"), "caissière ici : {v}");

    // Le caissier sans ligne : le dossier d'origine seulement.
    let (code, v) = entrer("moussa", None);
    assert_eq!(code, 200, "{v}");
    assert_eq!(v["dossier_id"], ORIGINE);
    assert_eq!(v["dossiers"].as_array().unwrap().len(), 1);

    // Le frère perd sa quincaillerie : sa session tombe à la requête suivante.
    let (code, v) = rpc(port, &admin, "definir_dossiers_utilisateur", json!({
        "utilisateurId": frere, "dossiers": [{ "dossier_id": ORIGINE, "role": "caissier" }]
    }));
    assert_eq!(code, 200, "{v}");
    let (code, v) = rpc(port, &jeton_frere, "lire_clients", json!({}));
    assert_eq!(code, 401, "{v}");
}

/// Revue du 25/09 : la copie automatique attendait 24 h de marche
/// continue — un poste éteint chaque soir n'était JAMAIS sauvegardé.
/// Ici le serveur démarre sur un dossier vide : il copie peu après, sans
/// attendre un jour ; `/sante` le dit ; relancé le même jour, il ne
/// recopie pas, et dit encore la date de la copie du matin.
#[test]
fn un_serveur_rallume_chaque_matin_est_sauvegarde_sans_attendre_un_jour() {
    let port = port_libre();
    let base = std::env::temp_dir().join(format!("gescom_matin_{}_{port}.db", std::process::id()));
    let dossier = std::env::temp_dir().join(format!("gescom_matin_sauv_{port}"));
    let _ = std::fs::remove_dir_all(&dossier);
    let demarrer = || {
        Command::new(env!("CARGO_BIN_EXE_gescom-serveur"))
            .args(["--hote", "127.0.0.1", "--port", &port.to_string()])
            .args(["--base", &base.to_string_lossy()])
            .args(["--sauvegardes", &dossier.to_string_lossy()])
            .env("GESCOM_SAUVEGARDE_DELAI_S", "1")
            .env(DECOUVERTE, "0")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("lancer gescom-serveur")
    };
    let attendre_sante = || {
        let debut = Instant::now();
        loop {
            if let Ok((200, v)) = requete(port, "GET", "/sante", None, None) {
                return v;
            }
            assert!(debut.elapsed() < Duration::from_secs(60), "le serveur ne répond pas");
            std::thread::sleep(Duration::from_millis(300));
        }
    };
    let copies = || -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(&dossier)
            .map(|d| d.flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect())
            .unwrap_or_default();
        v.retain(|n| n.starts_with("gescom_backup_"));
        v
    };

    let mut srv = Serveur { enfant: demarrer(), port, base: base.clone() };
    attendre_sante();
    let debut = Instant::now();
    while copies().is_empty() {
        assert!(debut.elapsed() < Duration::from_secs(30), "aucune copie automatique après le démarrage");
        std::thread::sleep(Duration::from_millis(300));
    }
    let premiere = copies();
    assert_eq!(premiere.len(), 1, "{premiere:?}");
    let chemin = dossier.join(&premiere[0]);
    gescom_noyau::sauvegarde::verifier_copie_sqlite(&chemin).expect("la copie se relit");

    // Relancé le même jour : pas de seconde copie, et la date est connue.
    let _ = srv.enfant.kill();
    let _ = srv.enfant.wait();
    srv.enfant = demarrer();
    let sante = attendre_sante();
    assert!(sante["derniere_sauvegarde"].as_str().is_some(), "après redémarrage : {sante}");
    std::thread::sleep(Duration::from_secs(4));
    assert_eq!(copies(), premiere, "une copie de moins d'un jour : pas de recopie");

    drop(srv);
    let _ = std::fs::remove_dir_all(&dossier);
}

// =====================================================================
//  Le reseau du serveur et la recherche automatique (27/10/2026)
// =====================================================================

fn port_udp_libre() -> u16 {
    UdpSocket::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

fn se_connecter(port: u16, identifiant: &str, mot_de_passe: &str, poste: &str) -> String {
    let (code, v) = requete(
        port, "POST", "/connexion",
        Some(&json!({ "identifiant": identifiant, "mot_de_passe": mot_de_passe, "poste_nom": poste, "poste_empreinte": poste, "version_protocole": 1 })),
        None,
    ).unwrap();
    assert_eq!(code, 200, "{v}");
    v["jeton"].as_str().unwrap().to_string()
}

/// Un port libre de la plage D39, cherche du haut vers le bas : ni 40000
/// (le serveur de la boutique ou celui d'un essai en cours) ni 40050
/// (l'essai a la main, `outils/banc/essai-main.ps1`).
fn port_libre_de_la_plage() -> u16 {
    (40001..=40100)
        .rev()
        .filter(|p| *p != 40050)
        .find(|p| TcpListener::bind(("127.0.0.1", *p)).is_ok())
        .expect("aucun port libre de 40001 à 40100")
}

/// Le serveur relance par lui-meme n'est pas un enfant du test : on le
/// retrouve par le pid que son journal ecrit, et on l'arrete — meme si le
/// test echoue en route. A declarer APRES le `Serveur` : il tombe avant
/// lui, et la base n'est plus ouverte quand on l'efface.
struct Relance {
    journal: std::path::PathBuf,
}

fn pids_relances(journal: &str) -> Vec<u32> {
    let marque = "nouveau processus (pid ";
    journal
        .match_indices(marque)
        .filter_map(|(i, _)| journal[i + marque.len()..].split(')').next()?.trim().parse().ok())
        .collect()
}

impl Drop for Relance {
    fn drop(&mut self) {
        let texte = std::fs::read_to_string(&self.journal).unwrap_or_default();
        for pid in pids_relances(&texte) {
            #[cfg(windows)]
            let _ = Command::new("taskkill")
                .args(["/F", "/PID", &pid.to_string()])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            #[cfg(not(windows))]
            let _ = Command::new("kill").args(["-9", &pid.to_string()]).status();
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

#[test]
fn la_recherche_udp_repond_par_le_nom_du_poste_et_rien_de_la_boutique() {
    let udp = port_udp_libre();
    let srv = lancer_avec(|c| {
        c.env(DECOUVERTE, udp.to_string());
    });
    let caisse = UdpSocket::bind("127.0.0.1:0").unwrap();
    let mut tampon = [0u8; 4096];

    // Un paquet étranger ne reçoit rien.
    caisse.set_read_timeout(Some(Duration::from_millis(800))).unwrap();
    caisse.send_to(b"bonjour", ("127.0.0.1", udp)).unwrap();
    caisse.send_to(br#"{"q":"autre","v":1}"#, ("127.0.0.1", udp)).unwrap();
    assert!(caisse.recv_from(&mut tampon).is_err(), "un paquet étranger ne reçoit rien");

    // La question de la caisse : une réponse, du port de la recherche.
    caisse.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    caisse.send_to(&gescom_noyau::decouverte::question(), ("127.0.0.1", udp)).unwrap();
    let (n, de) = caisse.recv_from(&mut tampon).expect("le serveur répond à la question");
    assert_eq!(de.port(), udp);
    let a = gescom_noyau::decouverte::lire_reponse(&tampon[..n]).expect("une réponse Gescom lisible");
    assert_eq!(a.produit, "Gescom Serveur");
    assert_eq!(a.port, srv.port, "le port TCP où il sert les caisses");
    assert_eq!(a.version_protocole, gescom_noyau::VERSION_PROTOCOLE);
    assert!(!a.version_serveur.is_empty());
    assert!(!a.poste.is_empty(), "le nom du poste");
    // Lancé sur 127.0.0.1 : il n'annonce que cette adresse.
    assert_eq!(a.adresses, vec!["127.0.0.1".to_string()]);
    // Exactement ces champs : rien de la base, rien des ventes.
    let brut: Value = serde_json::from_slice(&tampon[..n]).unwrap();
    let mut cles: Vec<&str> = brut.as_object().unwrap().keys().map(String::as_str).collect();
    cles.sort();
    assert_eq!(cles, ["adresses", "port", "poste", "produit", "r", "v", "version_protocole", "version_serveur"]);

    // Ce que la caisse en retient (la règle partagée).
    let trouves = gescom_noyau::decouverte::regrouper(&[(a, de.ip())], gescom_noyau::VERSION_PROTOCOLE, |_| vec![]);
    assert_eq!(trouves.len(), 1);
    assert_eq!(trouves[0].adresse_ip, format!("127.0.0.1:{}", srv.port));
}

#[test]
fn un_port_udp_deja_pris_n_empeche_pas_le_serveur_de_servir() {
    // Un autre « serveur » tient déjà le port de la recherche.
    let occupe = UdpSocket::bind("127.0.0.1:0").unwrap();
    let udp = occupe.local_addr().unwrap().port();
    let srv = lancer_avec(|c| {
        c.env(DECOUVERTE, udp.to_string());
    });
    let (admin, _) = connexion(srv.port);
    let journal = std::fs::read_to_string(srv.base.with_extension("log")).expect("journal technique");
    assert!(
        journal.contains("Recherche automatique du serveur indisponible") && journal.contains(&format!("UDP {udp} déjà pris")),
        "{journal}"
    );
    let (code, v) = requete(srv.port, "GET", "/reseau", None, Some(&admin)).unwrap();
    assert_eq!(code, 200, "{v}");
    assert_eq!(v["decouverte"]["active"], false, "{v}");
    assert!(v["decouverte"]["raison"].as_str().unwrap_or("").contains("déjà pris"), "{v}");
    drop(occupe);
}

#[test]
fn le_panneau_reseau_se_lit_mais_ne_change_rien_quand_la_ligne_de_commande_decide() {
    // `lancer` passe --hote et --port : comme la fenêtre de Gescom (D39).
    let srv = lancer();
    let port = srv.port;
    let (admin, _) = connexion(port);

    let (code, v) = requete(port, "GET", "/reseau", None, None).unwrap();
    assert_eq!(code, 401, "{v}");

    let (code, v) = requete(port, "GET", "/reseau", None, Some(&admin)).unwrap();
    assert_eq!(code, 200, "{v}");
    assert_eq!(v["port"], port);
    assert_eq!(v["ecoute"], "127.0.0.1");
    assert!(v["ecoute_libelle"].as_str().unwrap().contains("ce poste seulement"), "{v}");
    assert!(v["poste"].is_string() && v["cartes"].is_array(), "{v}");
    // N'écoute que ce poste : rien à taper sur une caisse.
    assert_eq!(v["a_saisir"], json!([]), "{v}");
    assert_eq!(v["depuis_ce_poste"], true);
    assert_eq!(v["en_service"], false);
    assert_eq!(v["plage"], json!([40000, 40100]));
    assert_eq!(v["modifiable"], false, "{v}");
    assert!(v["pourquoi_pas"].as_str().unwrap().contains("géré par Gescom"), "{v}");

    // L'employé lit, sans pouvoir changer — et le serveur le refuse.
    let employe = se_connecter(port, "employe", "employe123", "routes-reseau-e");
    let (code, v) = requete(port, "GET", "/reseau", None, Some(&employe)).unwrap();
    assert_eq!(code, 200, "{v}");
    assert_eq!(v["modifiable"], false);
    assert!(v["pourquoi_pas"].as_str().unwrap().contains("Modifier les paramètres"), "{v}");
    let (code, v) = requete(port, "POST", "/reseau", Some(&json!({ "port": 40001 })), Some(&employe)).unwrap();
    assert_eq!(code, 403, "{v}");
    assert_eq!(v["code"], "permission");

    // Le patron non plus : la ligne de commande décide ici.
    let (code, v) = requete(port, "POST", "/reseau", Some(&json!({ "port": 40001 })), Some(&admin)).unwrap();
    assert_eq!(code, 409, "{v}");
    assert!(v["message"].as_str().unwrap().contains("géré par Gescom"), "{v}");

    // La console porte le panneau, écrit en texte.
    let mut flux = TcpStream::connect(("127.0.0.1", port)).unwrap();
    flux.write_all(b"GET /console HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n").unwrap();
    let mut page = String::new();
    flux.read_to_string(&mut page).unwrap();
    assert!(page.contains("id=\"carte-reseau\"") && page.contains("Rechercher le serveur"), "la console a son panneau Réseau");
    assert!(page.contains("dd.textContent = valeur"), "les valeurs du système s'écrivent en texte");
}

#[test]
fn un_port_change_reecrit_serveur_json_passe_a_l_historique_et_le_serveur_y_redemarre() {
    // Un serveur lancé « à la main » sans --port ni --hote : son port et
    // son écoute viennent de serveur.json — ici un fichier d'essai, jamais
    // le vrai ProgramData. Deux ports de la plage : celui de départ et
    // le nouveau.
    let depart = port_libre_de_la_plage();
    let dossier = std::env::temp_dir().join(format!("gescom_reseau_{}_{depart}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dossier);
    std::fs::create_dir_all(&dossier).unwrap();
    let conf = dossier.join("serveur.json");
    let original = format!(
        "{{\n  \"base\": \"C:\\\\ne\\\\pas\\\\toucher.db\",\n  \"port\": {depart},\n  \"hote\": \"127.0.0.1\",\n  \
         \"sauvegardes\": \"D:\\\\Sauvegardes\",\n  \"journal\": \"D:\\\\gescom.log\",\n  \"cle_inconnue\": [1, 2]\n}}\n"
    );
    std::fs::write(&conf, &original).unwrap();
    let srv = lancer_sur(depart, false, |c| {
        c.env("GESCOM_SERVEUR_JSON", &conf);
    });
    let _relance = Relance { journal: srv.base.with_extension("log") };
    let (admin, identite) = connexion(depart);

    let (code, v) = requete(depart, "GET", "/reseau", None, Some(&admin)).unwrap();
    assert_eq!(code, 200, "{v}");
    assert_eq!(v["port"], depart, "le port vient de serveur.json : {v}");
    assert_eq!(v["ecoute"], "127.0.0.1");
    assert_eq!(v["modifiable"], true, "{v}");
    assert_eq!(v["configuration"], conf.display().to_string());

    // Les refus : sans la permission, hors de la plage, une écoute
    // inconnue, rien qui change. Le fichier n'a pas bougé.
    let employe = se_connecter(depart, "employe", "employe123", "routes-reseau-e2");
    let (code, v) = requete(depart, "POST", "/reseau", Some(&json!({ "port": 40001 })), Some(&employe)).unwrap();
    assert_eq!(code, 403, "{v}");
    for (demande, attendu) in [
        (json!({ "port": 7300 }), "40000-40100"),
        (json!({ "port": 70000 }), "70000"),
        (json!({ "port": 40001, "ecoute": "192.168.1.20" }), "Écoute"),
        (json!({ "port": depart, "ecoute": "127.0.0.1" }), "Rien ne change"),
    ] {
        let (code, v) = requete(depart, "POST", "/reseau", Some(&demande), Some(&admin)).unwrap();
        assert_eq!(code, 409, "{demande} : {v}");
        assert!(v["message"].as_str().unwrap().contains(attendu), "{demande} : {v}");
    }
    let (code, v) = requete(depart, "POST", "/reseau", Some(&json!({ "ecoute": "0.0.0.0" })), Some(&admin)).unwrap();
    assert_eq!(code, 400, "sans port : {v}");
    assert_eq!(std::fs::read_to_string(&conf).unwrap(), original, "un refus n'écrit rien");

    // Le changement accepté.
    let nouveau = port_libre_de_la_plage();
    let (code, v) = requete(depart, "POST", "/reseau", Some(&json!({ "port": nouveau, "ecoute": "127.0.0.1" })), Some(&admin)).unwrap();
    assert_eq!(code, 200, "{v}");
    assert_eq!(v["port"], nouveau);
    assert_eq!(v["redemarrage"], "relance", "lancé à la main : il se relance lui-même");
    assert_eq!(v["console"], format!("http://127.0.0.1:{nouveau}/console"));

    // serveur.json : le port change, tout le reste est gardé.
    let c: Value = serde_json::from_str(&std::fs::read_to_string(&conf).unwrap()).unwrap();
    assert_eq!(c["port"], nouveau, "{c}");
    assert_eq!(c["hote"], "127.0.0.1");
    assert_eq!(c["base"], "C:\\ne\\pas\\toucher.db");
    assert_eq!(c["sauvegardes"], "D:\\Sauvegardes");
    assert_eq!(c["journal"], "D:\\gescom.log");
    assert_eq!(c["cle_inconnue"], json!([1, 2]));
    let restes: Vec<String> = std::fs::read_dir(&dossier).unwrap().flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect();
    assert_eq!(restes, vec!["serveur.json".to_string()], "aucun fichier temporaire laissé");

    // Le serveur revient sur le nouveau port, avec la même base.
    let debut = Instant::now();
    loop {
        if let Ok((200, _)) = requete(nouveau, "GET", "/sante", None, None) {
            break;
        }
        assert!(debut.elapsed() < Duration::from_secs(60), "le serveur ne revient pas sur le port {nouveau}");
        std::thread::sleep(Duration::from_millis(300));
    }
    assert!(requete(depart, "GET", "/sante", None, None).is_err(), "l'ancien port ne répond plus");

    // L'Historique dit qui, quand, et de quoi à quoi.
    let (admin2, _) = connexion(nouveau);
    let (code, v) = rpc(nouveau, &admin2, "lire_historique", json!({ "filtre": {
        "type_evenement": "reseau_serveur_modifie", "page": 0, "par_page": 50
    }}));
    assert_eq!(code, 200, "{v}");
    let lignes = v["donnee"]["lignes"].as_array().unwrap();
    assert_eq!(lignes.len(), 1, "{v}");
    assert_eq!(lignes[0]["libelle_type"], "Port ou écoute du serveur changé");
    assert_eq!(lignes[0]["ancien"], format!("127.0.0.1:{depart}"));
    assert_eq!(lignes[0]["nouveau"], format!("127.0.0.1:{nouveau}"));
    assert_eq!(lignes[0]["auteur_id"], identite["utilisateur_id"], "signé par la session (D26)");

    // Et le journal technique : le changement, puis la relance.
    let journal = std::fs::read_to_string(srv.base.with_extension("log")).unwrap();
    assert!(journal.contains(&format!("Réseau du serveur changé : 127.0.0.1:{depart} → 127.0.0.1:{nouveau}")), "{journal}");
    assert_eq!(pids_relances(&journal).len(), 1, "{journal}");

    drop(_relance);
    drop(srv);
    let _ = std::fs::remove_dir_all(&dossier);
}

/// Revue de la paie du 28/10 (points 3 et 11), de bout en bout par le
/// serveur : qui gere le personnel sans voir la paie n'efface pas le
/// salaire en enregistrant la fiche, recoit les reponses masquees, et un
/// refus d'avance qui ne chiffre pas le plafond ; un montant enorme est
/// refuse sans faire tomber la base (avant : panique, « Base
/// indisponible »).
#[test]
fn un_gestionnaire_sans_la_paie_n_efface_pas_le_salaire_et_un_montant_enorme_ne_tue_pas_la_base() {
    let srv = lancer();
    let port = srv.port;
    let (admin, _) = connexion(port);
    let (code, v) = rpc(port, &admin, "creer_utilisateur", json!({
        "nom": "Issa", "pseudo": "issa", "email": null, "motDePasse": "issa-secret", "roleNom": "magasinier"
    }));
    assert_eq!(code, 200, "{v}");
    let issa_id = v["donnee"].as_str().unwrap().to_string();
    for p in ["personnel:gerer", "personnel:avancer"] {
        let (code, v) = rpc(port, &admin, "definir_permission_utilisateur", json!({ "utilisateurId": issa_id, "permission": p, "accorde": true }));
        assert_eq!(code, 200, "{v}");
    }
    let (code, v) = rpc(port, &admin, "creer_employe", json!({ "fiche": {
        "nom": "Awa", "fonction": "vendeuse", "salaire_mensuel": 60000, "avance_max": 15000
    }}));
    assert_eq!(code, 200, "{v}");
    let awa = v["donnee"]["id"].as_str().unwrap().to_string();
    assert_eq!(v["donnee"]["salaire_mensuel"], 60000, "le patron lit ce qu'il écrit");
    let (code, v) = requete(
        port, "POST", "/connexion",
        Some(&json!({ "identifiant": "issa", "mot_de_passe": "issa-secret", "poste_nom": "p", "poste_empreinte": "routes-paie", "version_protocole": 1 })),
        None,
    ).unwrap();
    assert_eq!(code, 200, "{v}");
    let issa = v["jeton"].as_str().unwrap().to_string();

    // Son écran renvoie la fiche sans montants : le salaire reste.
    let (code, v) = rpc(port, &issa, "modifier_employe", json!({ "employeId": awa, "fiche": { "nom": "Awa", "fonction": "caissière" } }));
    assert_eq!(code, 200, "{v}");
    assert!(v["donnee"]["salaire_mensuel"].is_null() && v["donnee"]["avance_max"].is_null(), "réponse masquée : {v}");
    assert_eq!(v["donnee"]["fonction"], "caissière");
    let (_, v) = rpc(port, &admin, "lire_employe", json!({ "employeId": awa }));
    assert_eq!((v["donnee"]["salaire_mensuel"].as_i64(), v["donnee"]["avance_max"].as_i64()), (Some(60000), Some(15000)), "{v}");
    let (code, v) = rpc(port, &issa, "creer_employe", json!({ "fiche": { "nom": "Ali", "fonction": "livreur", "tarif_journalier": 2000 } }));
    assert_eq!(code, 200, "{v}");
    assert!(v["donnee"]["tarif_journalier"].is_null(), "{v}");

    // Le refus d'une avance ne lui dit pas le plafond ; au patron, si.
    let (code, v) = rpc(port, &issa, "donner_avance", json!({ "employeId": awa, "montant": 20000 }));
    assert_eq!(code, 409, "{v}");
    let m = v["message"].as_str().unwrap_or("");
    assert!(m.contains("dépasserait son plafond") && !m.contains("15 000"), "{v}");
    let (_, v) = rpc(port, &admin, "donner_avance", json!({ "employeId": awa, "montant": 20000 }));
    assert!(v["message"].as_str().unwrap_or("").contains("15 000"), "{v}");

    // Un montant énorme : un refus, et la base répond encore.
    let (code, v) = rpc(port, &admin, "preparer_fiche_paie", json!({ "employeId": awa, "du": "2026-09-01", "au": "2026-09-30", "prorata": false }));
    assert_eq!(code, 200, "{v}");
    let fiche = v["donnee"]["id"].as_str().unwrap().to_string();
    let (code, v) = rpc(port, &admin, "ajouter_ligne_paie", json!({ "ficheId": fiche, "genre": "prime", "libelle": "x", "montant": i64::MAX }));
    assert_eq!(code, 409, "{v}");
    assert!(v["message"].as_str().unwrap_or("").contains("trop grand"), "{v}");
    let (code, v) = rpc(port, &admin, "donner_avance", json!({ "employeId": awa, "montant": i64::MAX }));
    assert_eq!(code, 409, "{v}");
    let (code, v) = rpc(port, &admin, "lire_fiche_paie", json!({ "ficheId": fiche }));
    assert_eq!(code, 200, "la base répond toujours : {v}");
    assert_eq!(v["donnee"]["net"], 60000);
}
