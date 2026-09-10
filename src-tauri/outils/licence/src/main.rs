//! `gescom-licence` — l'outil de l'éditeur.
//!
//! ⚠️ **Ne se livre jamais au client.** Il contient de quoi émettre des
//! licences ; le distribuer reviendrait à publier le générateur de
//! clefs qu'on cherche précisément à empêcher.
//!
//! ## Deux gestes
//!
//! ```text
//! gescom-licence clef                       (une fois, au tout début)
//! gescom-licence signer --empreinte ...     (à chaque vente)
//! ```
//!
//! La clef privée n'entre pas dans le dépôt. Elle est écrite hors du
//! projet, et il faut la sauvegarder : la perdre, c'est ne plus pouvoir
//! émettre une seule licence — ni pour un nouveau client, ni pour
//! réémettre à celui qui a réinstallé Windows.

use base64::Engine;
use ed25519_dalek::{Signer, SigningKey};
use gescom_noyau::licence::{Contenu, PREFIXE, TOUT_POSTE};

fn b64() -> base64::engine::general_purpose::GeneralPurpose {
    base64::engine::general_purpose::URL_SAFE_NO_PAD
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("clef") => engendrer_clef(&args),
        Some("signer") => signer(&args),
        Some("verifier") => verifier(&args),
        Some("empreinte") => println!("{}", gescom_noyau::empreinte::empreinte_poste()),
        _ => {
            aide();
            std::process::exit(1);
        }
    }
}

fn aide() {
    eprintln!(
        r#"gescom-licence — outil de l'éditeur (ne pas livrer au client)

  clef   [--sortie CHEMIN]
         Engendre une paire de clefs. Écrit la clef privée dans CHEMIN
         (défaut : %USERPROFILE%\.gescom\licence_privee.key) et affiche
         la clef publique à recopier dans noyau/src/licence.rs.

  signer --boutique NOM --empreinte CODE [options]
         --postes N          postes autorisés (défaut 1)
         --expire AAAA-MM-JJ licence à durée limitée (défaut : perpétuelle)
         --nif NUMERO        NIF du commerçant, imprimé sur ses pièces
         --clef CHEMIN       clef privée
         --sortie CHEMIN     fichier .licence à envoyer (défaut : stdout)

         --empreinte {TOUT_POSTE}  émet une licence FLOTTANTE, valable sur
         n'importe quel poste. À réserver aux démonstrations : distribuée
         par erreur, elle annule toute la protection.

  verifier --fichier CHEMIN [--empreinte CODE]
         Relit une licence comme le ferait Gescom. Pour le support :
         le client dit « ça ne marche pas », on voit pourquoi en dix
         secondes au lieu de le faire décrire un message d'erreur.

  empreinte
         Affiche le code de CE poste."#
    );
}

fn valeur(args: &[String], nom: &str) -> Option<String> {
    args.iter()
        .position(|a| a == nom)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn chemin_clef_defaut() -> std::path::PathBuf {
    let base = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".to_string());
    // Hors du dépôt, volontairement : un `git add -A` distrait ne doit
    // pas pouvoir publier la clef privée.
    std::path::Path::new(&base).join(".gescom").join("licence_privee.key")
}

// =====================================================================
//  clef
// =====================================================================

fn engendrer_clef(args: &[String]) {
    let chemin = valeur(args, "--sortie")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(chemin_clef_defaut);

    if chemin.exists() {
        eprintln!(
            "Une clef existe déjà : {}\n\
             Refus d'écraser : toutes les licences déjà émises deviendraient \
             invalides.\n\
             La déplacer à la main si le remplacement est voulu.",
            chemin.display()
        );
        std::process::exit(2);
    }

    let mut graine = [0u8; 32];
    if let Err(e) = getrandom::getrandom(&mut graine) {
        eprintln!("Impossible de tirer une clef sûre : {e}");
        std::process::exit(3);
    }
    let signataire = SigningKey::from_bytes(&graine);
    let publique = signataire.verifying_key();

    if let Some(parent) = chemin.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    if let Err(e) = std::fs::write(&chemin, b64().encode(signataire.to_bytes())) {
        eprintln!("Écriture de la clef privée impossible : {e}");
        std::process::exit(3);
    }

    println!("Clef privée écrite : {}", chemin.display());
    println!("  À SAUVEGARDER. Sans elle, plus aucune licence ne peut être émise.");
    println!("  À NE JAMAIS mettre dans le dépôt ni livrer à un client.\n");
    println!("Recopier ceci dans src-tauri/noyau/src/licence.rs :\n");
    println!("pub const CLE_PUBLIQUE: [u8; 32] = [");
    for tranche in publique.to_bytes().chunks(8) {
        let ligne: Vec<String> = tranche.iter().map(|o| format!("0x{o:02X}")).collect();
        println!("    {},", ligne.join(", "));
    }
    println!("];");
}

// =====================================================================
//  signer
// =====================================================================

fn signer(args: &[String]) {
    let chemin_clef = valeur(args, "--clef")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(chemin_clef_defaut);

    let brut = match std::fs::read_to_string(&chemin_clef) {
        Ok(v) => v,
        Err(e) => {
            eprintln!(
                "Clef privée illisible ({}) : {e}\n\
                 Lancer d'abord : gescom-licence clef",
                chemin_clef.display()
            );
            std::process::exit(2);
        }
    };
    let octets = match b64().decode(brut.trim()).ok().and_then(|v| <[u8; 32]>::try_from(v).ok()) {
        Some(v) => v,
        None => {
            eprintln!("Le fichier de clef est abîmé.");
            std::process::exit(2);
        }
    };
    let signataire = SigningKey::from_bytes(&octets);

    let Some(boutique) = valeur(args, "--boutique") else {
        eprintln!("--boutique est obligatoire : c'est ce que le client verra.");
        std::process::exit(1);
    };
    let Some(empreinte) = valeur(args, "--empreinte") else {
        eprintln!("--empreinte est obligatoire (le code lu dans Gescom).");
        std::process::exit(1);
    };
    let postes_max: u32 = valeur(args, "--postes")
        .and_then(|v| v.parse().ok())
        .unwrap_or(1);
    let expire_le = valeur(args, "--expire");

    if let Some(fin) = &expire_le {
        if chrono::NaiveDate::parse_from_str(fin, "%Y-%m-%d").is_err() {
            eprintln!("--expire attend une date AAAA-MM-JJ, reçu « {fin} ».");
            std::process::exit(1);
        }
    }
    if empreinte == TOUT_POSTE {
        eprintln!("⚠️  Licence FLOTTANTE : elle marchera sur n'importe quel poste.");
    }

    let contenu = Contenu {
        v: 1,
        boutique,
        nif: valeur(args, "--nif"),
        empreinte,
        postes_max,
        emis_le: chrono::Local::now().format("%Y-%m-%d").to_string(),
        expire_le,
    };

    // On signe les octets JSON, ceux-là mêmes que le vérificateur
    // relira. Signer une chaîne re-sérialisée plus tard laisserait
    // passer un écart d'encodage.
    let charge = serde_json::to_vec(&contenu).expect("sérialisation");
    let signature = signataire.sign(&charge);

    let jeton = format!(
        "{PREFIXE}.{}.{}",
        b64().encode(&charge),
        b64().encode(signature.to_bytes())
    );

    match valeur(args, "--sortie") {
        Some(chemin) => match std::fs::write(&chemin, &jeton) {
            Ok(()) => {
                println!("Licence écrite : {chemin}");
                println!("Boutique  : {}", contenu.boutique);
                println!("Poste     : {}", contenu.empreinte);
                println!("Postes    : {}", contenu.postes_max);
                println!(
                    "Expire    : {}",
                    contenu.expire_le.as_deref().unwrap_or("jamais")
                );
            }
            Err(e) => {
                eprintln!("Écriture impossible : {e}");
                std::process::exit(3);
            }
        },
        None => println!("{jeton}"),
    }
}

// =====================================================================
//  verifier — l'outil de support
// =====================================================================

fn verifier(args: &[String]) {
    let Some(chemin) = valeur(args, "--fichier") else {
        eprintln!("--fichier est obligatoire.");
        std::process::exit(1);
    };
    let texte = match std::fs::read_to_string(&chemin) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Lecture impossible : {e}");
            std::process::exit(2);
        }
    };
    let empreinte = valeur(args, "--empreinte")
        .unwrap_or_else(gescom_noyau::empreinte::empreinte_poste);
    let jour = chrono::Local::now().format("%Y-%m-%d").to_string();

    let etat = gescom_noyau::licence::verifier(&texte, &empreinte, &jour);
    println!("Poste testé : {empreinte}");
    println!("Date        : {jour}");
    println!("Résultat    : {etat:#?}");
    println!(
        "Autorise    : {}   Postes max : {}",
        etat.autorise(),
        etat.postes_max()
    );
    if !etat.autorise() {
        std::process::exit(1);
    }
}
