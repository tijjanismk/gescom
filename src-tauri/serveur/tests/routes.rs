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
use std::net::{TcpListener, TcpStream};
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

fn lancer() -> Serveur {
    let port = port_libre();
    let base = std::env::temp_dir().join(format!("gescom_routes_{}_{port}.db", std::process::id()));
    let sauvegardes = std::env::temp_dir().join(format!("gescom_routes_sauv_{port}"));
    let enfant = Command::new(env!("CARGO_BIN_EXE_gescom-serveur"))
        .args(["--hote", "127.0.0.1", "--port", &port.to_string()])
        .args(["--base", &base.to_string_lossy()])
        .args(["--sauvegardes", &sauvegardes.to_string_lossy()])
        // Des articles pour vendre : la demo.
        .env("GESCOM_DEMO", "1")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("lancer gescom-serveur");
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
    let vente = |quantite: f64| json!({
        "clientId": client, "depotId": depot, "modeReglement": "comptant",
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
    let (code, v) = rpc(port, &awa, "creer_vente", json!({
        "clientId": client, "depotId": depot, "modeReglement": "comptant",
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
