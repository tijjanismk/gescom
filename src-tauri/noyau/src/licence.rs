//! La licence — ce qui attache une installation à un poste.
//!
//! ## Le problème
//!
//! `Gescom.exe` se suffit à lui-même : SQLite est compilé dedans, le
//! front est embarqué. Copié sur une clé USB, il démarre ailleurs. Une
//! vente valait donc un nombre illimité d'installations.
//!
//! ## Ce que ce module fait, et ce qu'il ne fera jamais
//!
//! Il **lie** une licence à l'empreinte du poste et refuse de
//! travailler sans. Il ne prétend pas résister à quelqu'un qui
//! démonte l'exécutable : aucune protection logicielle ne le fait, et
//! prétendre le contraire ferait perdre du temps à tout le monde. Ce
//! qu'il empêche, c'est la copie ORDINAIRE — celle d'un client qui
//! installe le même logiciel dans sa deuxième boutique, ou d'un
//! revendeur qui duplique sans le dire. C'est là qu'est l'argent perdu,
//! pas chez le casseur déterminé.
//!
//! ## Pourquoi une signature asymétrique
//!
//! L'exécutable ne contient que la clef PUBLIQUE. Le désosser ne donne
//! rien pour fabriquer une licence. Avec un secret partagé, le premier
//! qui l'extrait publie un générateur de clefs, et la protection tombe
//! partout d'un coup — c'est la différence qui compte, pas la solidité
//! du chiffrement.
//!
//! ## Hors ligne, par construction
//!
//! Aucune activation par Internet. À Bamako, une boutique dont la
//! caisse exige le réseau pour ouvrir est une boutique qui n'ouvre pas.
//! Le commerçant lit son code de poste au téléphone, reçoit un fichier
//! par WhatsApp, l'importe. C'est le chemin que les gens empruntent
//! déjà.

use base64::Engine;
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};

/// Clef publique de vérification (Ed25519, 32 octets).
///
/// La clef PRIVÉE correspondante ne doit jamais entrer dans ce dépôt.
/// Elle vit chez l'éditeur, et elle seule permet d'émettre. La perdre
/// signifie ne plus pouvoir émettre de licence — la sauvegarder est
/// aussi sérieux que sauvegarder la base d'un client.
pub const CLE_PUBLIQUE: [u8; 32] = [
    0x3D, 0x2A, 0x08, 0xFF, 0xEC, 0x1A, 0xA0, 0x8E,
    0xC7, 0xAC, 0x52, 0x2E, 0xDD, 0xB2, 0x16, 0x8A,
    0xD0, 0xDC, 0x8E, 0xBA, 0x70, 0xA0, 0x1C, 0x39,
    0xA4, 0x9F, 0xA4, 0xA9, 0x13, 0xF1, 0x9F, 0xF9,
];

/// Préfixe du jeton. Permet de reconnaître un fichier de licence, et de
/// changer de format un jour sans faire lire un ancien jeton par un
/// nouveau vérificateur.
pub const PREFIXE: &str = "GESCOM1";

/// Empreinte qui accepte n'importe quel poste.
///
/// Réservée aux licences de démonstration et au support. Une licence
/// flottante distribuée par erreur annulerait toute la protection ;
/// elle doit rester une exception que l'éditeur décide, pas un défaut.
pub const TOUT_POSTE: &str = "*";

/// Durée de l'essai, en jours.
///
/// Trente jours : un cycle commercial complet, achats et clôture de
/// mois compris. En dessous, le commerçant n'a pas eu le temps de voir
/// ce que le logiciel fait de ses créances, et il juge sur l'écran de
/// vente seul.
pub const JOURS_ESSAI: i64 = 30;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Contenu {
    pub v: u32,
    pub boutique: String,
    #[serde(default)]
    pub nif: Option<String>,
    /// L'empreinte du poste, ou `*`.
    pub empreinte: String,
    /// Nombre de postes autorisés à se connecter au serveur.
    pub postes_max: u32,
    pub emis_le: String,
    #[serde(default)]
    pub expire_le: Option<String>,
}

/// L'état de la licence, tel que l'écran doit le montrer.
///
/// Chaque variante porte de quoi écrire un message utile. « Licence
/// invalide » sans dire pourquoi produit un appel téléphonique et une
/// heure perdue de part et d'autre.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "etat", rename_all = "snake_case")]
pub enum EtatLicence {
    Valide {
        contenu: Contenu,
        /// Jours restants, `None` si la licence est perpétuelle.
        jours_restants: Option<i64>,
    },
    /// Pas de licence, mais l'essai court encore.
    Essai { jours_restants: i64, empreinte: String },
    /// Pas de licence et l'essai est fini.
    EssaiTermine { empreinte: String },
    /// Le fichier n'est pas un jeton Gescom.
    Illisible { raison: String, empreinte: String },
    /// Le jeton a été modifié, ou n'a pas été émis par l'éditeur.
    SignatureInvalide { empreinte: String },
    /// Licence authentique, mais émise pour une autre machine.
    AutrePoste { attendue: String, empreinte: String },
    Expiree { le: String, contenu: Contenu, empreinte: String },
}

impl EtatLicence {
    /// Le logiciel peut-il travailler ?
    pub fn autorise(&self) -> bool {
        matches!(self, EtatLicence::Valide { .. } | EtatLicence::Essai { .. })
    }

    /// Le plafond de postes que le serveur doit faire respecter.
    ///
    /// Un essai vaut un seul poste : sinon, laisser courir trente jours
    /// suffirait à équiper un magasin entier.
    pub fn postes_max(&self) -> u32 {
        match self {
            EtatLicence::Valide { contenu, .. } => contenu.postes_max.max(1),
            EtatLicence::Essai { .. } => 1,
            _ => 0,
        }
    }
}

// =====================================================================
//  Vérification
// =====================================================================

fn b64() -> base64::engine::general_purpose::GeneralPurpose {
    base64::engine::general_purpose::URL_SAFE_NO_PAD
}

/// Vérifie un jeton pour ce poste, à cette date.
///
/// `aujourdhui` au format `AAAA-MM-JJ`. Passé en paramètre et non lu de
/// l'horloge : c'est ce qui rend l'expiration testable, et une règle de
/// date non testée finit toujours par se tromper d'un jour.
pub fn verifier(texte: &str, empreinte: &str, aujourdhui: &str) -> EtatLicence {
    let clef = match VerifyingKey::from_bytes(&CLE_PUBLIQUE) {
        Ok(c) => c,
        Err(_) => {
            return EtatLicence::Illisible {
                raison: "Cette version de Gescom n'a pas de clef de vérification."
                    .to_string(),
                empreinte: empreinte.to_string(),
            }
        }
    };
    verifier_avec(&clef, texte, empreinte, aujourdhui)
}

/// La même vérification, avec une clef choisie.
///
/// Existe pour les tests : sans elle, on ne pourrait éprouver le chemin
/// nominal qu'en embarquant une vraie licence signée dans le dépôt,
/// donc en la laissant expirer un jour sans que personne ne comprenne
/// pourquoi la suite casse.
pub fn verifier_avec(
    clef: &VerifyingKey,
    texte: &str,
    empreinte: &str,
    aujourdhui: &str,
) -> EtatLicence {
    let illisible = |raison: &str| EtatLicence::Illisible {
        raison: raison.to_string(),
        empreinte: empreinte.to_string(),
    };

    // Les licences voyagent par WhatsApp et par copier-coller : espaces,
    // retours à la ligne et guillemets s'y invitent. On les retire
    // plutôt que de renvoyer « illisible » à un jeton parfaitement bon.
    let propre: String = texte
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '"' && *c != '\'')
        .collect();

    let mut parts = propre.split('.');
    let (Some(prefixe), Some(charge), Some(signature), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return illisible("Le jeton ne comporte pas ses trois parties.");
    };
    if prefixe != PREFIXE {
        return illisible("Ce n'est pas une licence Gescom.");
    }

    let Ok(octets_charge) = b64().decode(charge) else {
        return illisible("La partie centrale du jeton est abîmée.");
    };
    let Ok(octets_sig) = b64().decode(signature) else {
        return illisible("La signature du jeton est abîmée.");
    };
    let Ok(sig_fixe) = <[u8; 64]>::try_from(octets_sig.as_slice()) else {
        return illisible("La signature n'a pas la bonne longueur.");
    };

    // La signature porte sur les octets DÉCODÉS. Signer la chaîne
    // base64 laisserait passer deux encodages du même contenu, dont un
    // qu'on n'a pas relu.
    if clef
        .verify_strict(&octets_charge, &Signature::from_bytes(&sig_fixe))
        .is_err()
    {
        return EtatLicence::SignatureInvalide { empreinte: empreinte.to_string() };
    }

    let Ok(contenu) = serde_json::from_slice::<Contenu>(&octets_charge) else {
        return illisible("Le contenu de la licence n'est pas lisible.");
    };
    if contenu.v != 1 {
        return illisible(
            "Cette licence vient d'une version plus récente de Gescom.",
        );
    }

    // L'empreinte se compare sans tenir compte de la casse ni des
    // tirets : le commerçant la dicte au téléphone, on la retape.
    let normaliser = |s: &str| s.replace('-', "").to_uppercase();
    if contenu.empreinte != TOUT_POSTE
        && normaliser(&contenu.empreinte) != normaliser(empreinte)
    {
        return EtatLicence::AutrePoste {
            attendue: contenu.empreinte.clone(),
            empreinte: empreinte.to_string(),
        };
    }

    match &contenu.expire_le {
        Some(fin) if fin.as_str() < aujourdhui => EtatLicence::Expiree {
            le: fin.clone(),
            contenu: contenu.clone(),
            empreinte: empreinte.to_string(),
        },
        Some(fin) => {
            let jours = jours_entre(aujourdhui, fin);
            EtatLicence::Valide { contenu, jours_restants: Some(jours) }
        }
        None => EtatLicence::Valide { contenu, jours_restants: None },
    }
}

/// Différence en jours entre deux dates `AAAA-MM-JJ`.
///
/// Zéro si l'une des deux est illisible : mieux vaut annoncer « expire
/// aujourd'hui » qu'une durée inventée.
pub fn jours_entre(depuis: &str, jusqua: &str) -> i64 {
    let lire = |s: &str| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok();
    match (lire(depuis), lire(jusqua)) {
        (Some(a), Some(b)) => (b - a).num_days(),
        _ => 0,
    }
}

// =====================================================================
//  L'essai
// =====================================================================

/// L'état à afficher quand aucune licence n'est installée.
pub fn etat_essai(debut: &str, aujourdhui: &str, empreinte: &str) -> EtatLicence {
    let ecoules = jours_entre(debut, aujourdhui);
    let restants = JOURS_ESSAI - ecoules;
    if restants > 0 {
        EtatLicence::Essai {
            jours_restants: restants,
            empreinte: empreinte.to_string(),
        }
    } else {
        EtatLicence::EssaiTermine { empreinte: empreinte.to_string() }
    }
}

/// La date de première ouverture, posée si elle manque.
///
/// Écrite à DEUX endroits : la base et le registre de l'utilisateur. On
/// retient la plus ancienne. Effacer la base pour repartir sur trente
/// jours neufs est le premier réflexe de qui veut prolonger l'essai ;
/// le témoin du registre le rend inopérant, et l'inverse aussi.
///
/// Ce n'est pas inviolable — les deux se nettoient. C'est un ralentisseur
/// honnête, pas un coffre-fort.
pub fn debut_essai(conn: &rusqlite::Connection, aujourdhui: &str) -> String {
    let en_base: Option<String> = conn
        .query_row(
            "SELECT valeur FROM config_app WHERE cle = 'premiere_ouverture'",
            [],
            |r| r.get(0),
        )
        .ok()
        .filter(|v: &String| !v.trim().is_empty());

    let au_registre = lire_temoin();

    let plus_ancienne = match (&en_base, &au_registre) {
        (Some(a), Some(b)) => Some(if a <= b { a.clone() } else { b.clone() }),
        (Some(a), None) => Some(a.clone()),
        (None, Some(b)) => Some(b.clone()),
        (None, None) => None,
    };
    let retenue = plus_ancienne.unwrap_or_else(|| aujourdhui.to_string());

    if en_base.as_deref() != Some(retenue.as_str()) {
        conn.execute(
            "INSERT INTO config_app (cle, valeur) VALUES ('premiere_ouverture', ?1)
             ON CONFLICT(cle) DO UPDATE SET valeur = excluded.valeur",
            rusqlite::params![retenue],
        )
        .ok();
    }
    if au_registre.as_deref() != Some(retenue.as_str()) {
        ecrire_temoin(&retenue);
    }
    retenue
}

#[cfg(windows)]
fn lire_temoin() -> Option<String> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(r"Software\Gescom")
        .ok()?
        .get_value::<String, _>("PremiereOuverture")
        .ok()
        .filter(|v| !v.trim().is_empty())
}

#[cfg(windows)]
fn ecrire_temoin(date: &str) {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    if let Ok((clef, _)) =
        RegKey::predef(HKEY_CURRENT_USER).create_subkey(r"Software\Gescom")
    {
        clef.set_value("PremiereOuverture", &date.to_string()).ok();
    }
}

#[cfg(not(windows))]
fn lire_temoin() -> Option<String> {
    None
}

#[cfg(not(windows))]
fn ecrire_temoin(_date: &str) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_texte_quelconque_est_illisible() {
        let e = verifier("bonjour", "AAAA-BBBB-CCCC", "2026-09-10");
        assert!(matches!(e, EtatLicence::Illisible { .. }));
        assert!(!e.autorise());
    }

    #[test]
    fn un_jeton_dun_autre_logiciel_est_refuse() {
        let e = verifier("AUTRE1.aaa.bbb", "AAAA-BBBB-CCCC", "2026-09-10");
        match e {
            EtatLicence::Illisible { raison, .. } => {
                assert!(raison.contains("licence Gescom"), "{raison}");
            }
            autre => panic!("attendu illisible, obtenu {autre:?}"),
        }
    }

    /// La clef publique livrée doit refuser une signature inventée.
    /// Sans ce test, une clef mal recopiée passerait inaperçue jusqu'au
    /// jour où toutes les licences seraient rejetées en clientèle.
    #[test]
    fn une_signature_inventee_ne_passe_pas() {
        let charge = b64().encode(
            br#"{"v":1,"boutique":"X","empreinte":"*","postes_max":9,"emis_le":"2026-01-01"}"#,
        );
        let fausse = b64().encode([7u8; 64]);
        let e = verifier(
            &format!("{PREFIXE}.{charge}.{fausse}"),
            "AAAA-BBBB-CCCC",
            "2026-09-10",
        );
        assert!(matches!(e, EtatLicence::SignatureInvalide { .. }), "{e:?}");
    }

    /// Une clef de test, tirée d'une graine fixe : reproductible, et
    /// sans rapport avec celle de l'éditeur.
    fn clef_test() -> ed25519_dalek::SigningKey {
        ed25519_dalek::SigningKey::from_bytes(&[42u8; 32])
    }

    fn jeton(contenu: &Contenu) -> String {
        use ed25519_dalek::Signer;
        let charge = serde_json::to_vec(contenu).unwrap();
        let sig = clef_test().sign(&charge);
        format!(
            "{PREFIXE}.{}.{}",
            b64().encode(&charge),
            b64().encode(sig.to_bytes())
        )
    }

    fn contenu_type(empreinte: &str, expire: Option<&str>) -> Contenu {
        Contenu {
            v: 1,
            boutique: "Quincaillerie du Fleuve".to_string(),
            nif: Some("084512345 X".to_string()),
            empreinte: empreinte.to_string(),
            postes_max: 3,
            emis_le: "2026-01-01".to_string(),
            expire_le: expire.map(str::to_string),
        }
    }

    #[test]
    fn une_licence_signee_pour_ce_poste_est_valide() {
        let c = contenu_type("A1B2-C3D4-E5F6", None);
        let e = verifier_avec(
            &clef_test().verifying_key(),
            &jeton(&c),
            "A1B2-C3D4-E5F6",
            "2026-09-10",
        );
        match e {
            EtatLicence::Valide { contenu, jours_restants } => {
                assert_eq!(contenu.postes_max, 3);
                assert_eq!(jours_restants, None, "perpétuelle");
            }
            autre => panic!("{autre:?}"),
        }
    }

    #[test]
    fn l_empreinte_se_compare_sans_tirets_ni_casse() {
        // Le commerçant dicte son code au téléphone ; on le retape.
        // Refuser « a1b2c3d4e5f6 » produirait un appel pour rien.
        let c = contenu_type("A1B2-C3D4-E5F6", None);
        let e = verifier_avec(
            &clef_test().verifying_key(),
            &jeton(&c),
            "a1b2c3d4e5f6",
            "2026-09-10",
        );
        assert!(e.autorise(), "{e:?}");
    }

    #[test]
    fn une_licence_dun_autre_poste_est_refusee() {
        let c = contenu_type("A1B2-C3D4-E5F6", None);
        let e = verifier_avec(
            &clef_test().verifying_key(),
            &jeton(&c),
            "9999-8888-7777",
            "2026-09-10",
        );
        match &e {
            EtatLicence::AutrePoste { attendue, .. } => {
                assert_eq!(attendue, "A1B2-C3D4-E5F6");
            }
            autre => panic!("{autre:?}"),
        }
        assert!(!e.autorise());
    }

    #[test]
    fn une_licence_flottante_passe_partout() {
        let c = contenu_type(TOUT_POSTE, None);
        let e = verifier_avec(
            &clef_test().verifying_key(),
            &jeton(&c),
            "PEU-IMPO-RTE0",
            "2026-09-10",
        );
        assert!(e.autorise(), "{e:?}");
    }

    #[test]
    fn une_licence_expiree_ne_travaille_plus() {
        let c = contenu_type("A1B2-C3D4-E5F6", Some("2026-06-30"));
        let e = verifier_avec(
            &clef_test().verifying_key(),
            &jeton(&c),
            "A1B2-C3D4-E5F6",
            "2026-09-10",
        );
        match &e {
            EtatLicence::Expiree { le, .. } => assert_eq!(le, "2026-06-30"),
            autre => panic!("{autre:?}"),
        }
        assert!(!e.autorise());
        assert_eq!(e.postes_max(), 0);
    }

    #[test]
    fn le_dernier_jour_est_encore_bon() {
        // Une expiration qui tombe un jour trop tôt bloque une caisse un
        // matin, et personne ne comprend pourquoi.
        let c = contenu_type("A1B2-C3D4-E5F6", Some("2026-09-10"));
        let e = verifier_avec(
            &clef_test().verifying_key(),
            &jeton(&c),
            "A1B2-C3D4-E5F6",
            "2026-09-10",
        );
        assert!(e.autorise(), "{e:?}");
    }

    #[test]
    fn un_contenu_modifie_casse_la_signature() {
        // Passer de 3 a 99 postes en editant le fichier : c'est le
        // premier essai de qui veut tricher.
        let c = contenu_type("A1B2-C3D4-E5F6", None);
        let bon = jeton(&c);
        let mut triche = contenu_type("A1B2-C3D4-E5F6", None);
        triche.postes_max = 99;
        let charge = b64().encode(serde_json::to_vec(&triche).unwrap());
        let signature = bon.split('.').nth(2).unwrap();

        let e = verifier_avec(
            &clef_test().verifying_key(),
            &format!("{PREFIXE}.{charge}.{signature}"),
            "A1B2-C3D4-E5F6",
            "2026-09-10",
        );
        assert!(matches!(e, EtatLicence::SignatureInvalide { .. }), "{e:?}");
    }

    #[test]
    fn une_licence_dun_autre_editeur_est_refusee() {
        // Signee par une clef differente : c'est ce que produirait un
        // faux generateur.
        let autre = ed25519_dalek::SigningKey::from_bytes(&[7u8; 32]);
        let c = contenu_type("A1B2-C3D4-E5F6", None);
        let e = verifier_avec(
            &autre.verifying_key(),
            &jeton(&c),
            "A1B2-C3D4-E5F6",
            "2026-09-10",
        );
        assert!(matches!(e, EtatLicence::SignatureInvalide { .. }), "{e:?}");
    }

    #[test]
    fn les_espaces_de_whatsapp_ne_genent_pas() {
        let c = contenu_type("A1B2-C3D4-E5F6", None);
        let sale = format!("  
{}
  ", jeton(&c).replace('.', ".
"));
        let e = verifier_avec(
            &clef_test().verifying_key(),
            &sale,
            "A1B2-C3D4-E5F6",
            "2026-09-10",
        );
        assert!(e.autorise(), "{e:?}");
    }

    #[test]
    fn l_essai_court_puis_s_arrete() {
        let e = etat_essai("2026-09-01", "2026-09-10", "AAAA");
        match e {
            EtatLicence::Essai { jours_restants, .. } => assert_eq!(jours_restants, 21),
            autre => panic!("{autre:?}"),
        }
        let fini = etat_essai("2026-08-01", "2026-09-10", "AAAA");
        assert!(matches!(fini, EtatLicence::EssaiTermine { .. }));
        assert!(!fini.autorise());
    }

    #[test]
    fn l_essai_ne_vaut_quun_poste() {
        // Trente jours suffiraient sinon à équiper un magasin entier.
        let e = etat_essai("2026-09-01", "2026-09-10", "AAAA");
        assert_eq!(e.postes_max(), 1);
    }
}
