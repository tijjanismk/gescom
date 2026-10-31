//! Le contrat entre le client et le serveur.
//!
//! Ces types sont compiles dans les DEUX executables. C'est leur seule
//! raison d'etre : un champ renomme d'un cote casse la compilation de
//! l'autre, au lieu d'arriver silencieusement en `null` sur un poste
//! caisse un samedi de marche.

use serde::{Deserialize, Serialize};

/// Port par defaut du serveur Gescom, et le premier de sa plage (D39).
///
/// 40000 : loin des ports systeme et des plages courantes (3000 dev,
/// 5432 Postgres, 7000-9000 des outils), pour eviter la collision sur un
/// poste ou tourne deja autre chose. 7300 est abandonne (27/10/2026).
pub const PORT_DEFAUT: u16 = 40000;

/// Le dernier port de la plage du serveur (D39). La fenetre livree ne
/// joint que 40000..=40100 : sa CSP (`tauri.conf.json`) les enumere un
/// par un, et `tests/plage_de_ports.rs` verifie qu'ils y sont tous. Sur
/// le poste qui garde la base, si 40000 est pris, le serveur local prend
/// le suivant libre de la plage.
pub const PORT_MAX: u16 = 40100;

/// Ce port est-il joignable par la fenetre livree ?
pub fn port_dans_la_plage(port: u16) -> bool {
    (PORT_DEFAUT..=PORT_MAX).contains(&port)
}

/// Les ports de la plage, dans l'ordre ou on les essaie.
pub fn ports_de_la_plage() -> impl Iterator<Item = u16> {
    PORT_DEFAUT..=PORT_MAX
}

/// Le port d'une adresse « hote:port » (sans schema), s'il est ecrit.
/// « [::1]:40000 » ; « 192.168.1.10:40000 » ; une IPv6 nue a plusieurs
/// « : » et pas de port. `Some(Err)` : un port ecrit mais illisible.
pub fn port_ecrit(adresse: &str) -> Option<Result<u16, String>> {
    let hote = adresse.trim().trim_start_matches("http://").split('/').next().unwrap_or("");
    let port = match hote.strip_prefix('[') {
        Some(reste) => reste.split_once("]:").map(|(_, p)| p),
        None if hote.matches(':').count() == 1 => hote.split_once(':').map(|(_, p)| p),
        None => None,
    }?;
    Some(port.trim().parse::<u16>().map_err(|_| format!("Port illisible : « {} ».", port.trim())))
}

/// La fenetre peut-elle joindre cette adresse ? Sans port, c'est
/// `PORT_DEFAUT`. Refuse un port hors de la plage, plutot que d'echouer
/// plus tard sur un « Failed to fetch » que personne ne comprend.
pub fn adresse_joignable(adresse: &str) -> Result<(), String> {
    match port_ecrit(adresse) {
        None => Ok(()),
        Some(Err(e)) => Err(e),
        Some(Ok(p)) if port_dans_la_plage(p) => Ok(()),
        Some(Ok(_)) => Err(format!(
            "Gescom ne joint le serveur que sur un port de {PORT_DEFAUT} à {PORT_MAX} : « {} » ne sera pas \
             joignable. Taper l'adresse du serveur sans port ({PORT_DEFAUT}), ou avec le port qu'il affiche.",
            adresse.trim()
        )),
    }
}

#[cfg(test)]
mod tests_ports {
    use super::*;

    #[test]
    fn la_plage_va_de_40000_a_40100() {
        assert!(port_dans_la_plage(40000) && port_dans_la_plage(40100));
        assert!(!port_dans_la_plage(39999) && !port_dans_la_plage(40101) && !port_dans_la_plage(7300));
        assert_eq!(ports_de_la_plage().count(), 101);
    }

    #[test]
    fn une_adresse_se_juge_sur_son_port() {
        for ok in ["192.168.1.10", "192.168.1.10:40000", "http://192.168.1.10:40057/", "caisse-principale", "[::1]:40100", "fe80::1"] {
            assert!(adresse_joignable(ok).is_ok(), "{ok}");
        }
        for non in ["192.168.1.10:7300", "localhost:80", "[::1]:40101", "192.168.1.10:abc"] {
            assert!(adresse_joignable(non).is_err(), "{non}");
        }
        assert_eq!(port_ecrit("127.0.0.1:40003"), Some(Ok(40003)));
        assert_eq!(port_ecrit("127.0.0.1"), None);
    }
}

/// Duree de vie d'un jeton de connexion.
///
/// Huit heures : la journee de travail. Le poste caisse ne redemande
/// pas le mot de passe en pleine vente, mais un poste laisse allume la
/// nuit ne reste pas ouvert au matin.
pub const DUREE_JETON_HEURES: i64 = 8;

// =====================================================================
//  Appel de commande
// =====================================================================

/// Une commande metier appelee a distance.
///
/// `commande` est le meme nom que cote Tauri (`creer_vente`,
/// `lire_clients`...). C'est voulu : le client appelle le meme nom
/// qu'il soit en monoposte ou en reseau, et seul le transport change.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Requete {
    pub commande: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "etat", rename_all = "lowercase")]
pub enum Reponse {
    Ok { donnee: serde_json::Value },
    Erreur { message: String, code: CodeErreur },
}

impl Reponse {
    pub fn ok(donnee: serde_json::Value) -> Self {
        Reponse::Ok { donnee }
    }
    pub fn erreur(code: CodeErreur, message: impl Into<String>) -> Self {
        Reponse::Erreur { code, message: message.into() }
    }
    /// Traduit un `Result<Value, String>` — la signature de toutes les
    /// commandes existantes — en reponse reseau.
    pub fn depuis(r: Result<serde_json::Value, String>) -> Self {
        match r {
            Ok(v) => Reponse::ok(v),
            // Le message metier porte deja son sens (« CAISSE_FERMEE — ... »).
            // On ne le reclasse pas ici : le client l'affiche tel quel.
            Err(m) => Reponse::erreur(CodeErreur::Metier, m),
        }
    }
}

/// Ce que le client doit FAIRE de l'erreur, pas ce qui s'est passe.
///
/// Trois issues seulement : reessayer, se reconnecter, montrer le
/// message au commercant. Une taxonomie plus fine ne changerait rien
/// au comportement du poste.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodeErreur {
    /// Regle metier : stock insuffisant, caisse fermee, facture emise.
    /// A afficher tel quel. Reessayer ne sert a rien.
    Metier,
    /// Jeton absent, expire ou revoque : il faut se reconnecter.
    Authentification,
    /// Le role du poste n'autorise pas cette commande.
    Permission,
    /// Commande inconnue du serveur — versions differentes.
    CommandeInconnue,
    /// Serveur injoignable, base verrouillee : reessayer a du sens.
    Technique,
    /// v3 : la session n'a pas encore de dossier — en choisir un
    /// (`choisir_dossier`) avant toute autre commande.
    DossierAChoisir,
}

// =====================================================================
//  Connexion
// =====================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DemandeConnexion {
    pub identifiant: String,
    pub mot_de_passe: String,
    /// Nom lisible du poste (« Caisse 1 », « Bureau »).
    pub poste_nom: String,
    /// Identifiant stable du poste, genere une fois et conserve.
    pub poste_empreinte: String,
    pub version_protocole: u32,
    /// v3 : le dossier a ouvrir. Absent : le seul qui existe, ou celui
    /// memorise pour cette personne, ou — s'il faut choisir — la
    /// session s'ouvre SANS dossier et `Identite.dossiers` liste le choix.
    #[serde(default)]
    pub dossier_id: Option<String>,
    /// Retenir ce dossier pour les prochaines connexions de cette
    /// personne (decision 3 du plan : par utilisateur).
    #[serde(default)]
    pub memoriser_dossier: Option<bool>,
}

/// Un dossier tel que l'ecran de connexion le propose.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DossierOuvrable {
    pub id: String,
    pub code: String,
    pub societe: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Identite {
    pub jeton: String,
    pub utilisateur_id: String,
    pub utilisateur_nom: String,
    pub role: String,
    /// Ce que cette personne a le droit de faire, calcule a la
    /// connexion. L'ecran s'en sert pour ne montrer que ce qui marche :
    /// sans elles, il ne pourrait que deviner d'apres le NOM du role.
    pub permissions: Vec<String>,
    pub doit_changer_mdp: bool,
    pub poste_id: String,
    pub expire_le: String,
    /// Vrai si la caisse est nominative (un tiroir par utilisateur).
    /// Toujours faux depuis le lot C.6 (mode retire, un tiroir par
    /// dossier) ; le champ reste pour ne pas changer le protocole.
    pub caisse_par_utilisateur: bool,
    /// v3 : le dossier ouvert par la session. `None` : il reste a
    /// choisir parmi `dossiers` (commande `choisir_dossier`), et aucune
    /// autre commande ne passe avant.
    #[serde(default)]
    pub dossier_id: Option<String>,
    #[serde(default)]
    pub dossier_societe: Option<String>,
    /// Les dossiers ouverts, pour l'ecran de choix. Un seul : pas
    /// d'ecran, il est deja ouvert.
    #[serde(default)]
    pub dossiers: Vec<DossierOuvrable>,
}

// =====================================================================
//  Canal d'evenements
// =====================================================================

/// Ce que le serveur pousse aux postes.
///
/// Volontairement maigre : le type et l'entite, jamais la donnee. Le
/// poste qui s'y interesse relit. Envoyer la donnee obligerait a la
/// garder coherente dans deux chemins de code, et un poste en retard
/// afficherait une valeur perimee sans le savoir.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evenement {
    /// Numero d'ordre global, croissant. Le poste renvoie le dernier
    /// recu pour reprendre ou il en etait — c'est ce qui rend la
    /// reconnexion sans trou possible.
    pub seq: u64,
    pub genre: String,
    pub entite: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entite_id: Option<String>,
    pub poste_id: String,
    pub horodatage: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LotEvenements {
    pub evenements: Vec<Evenement>,
    pub seq_max: u64,
}

// =====================================================================
//  Sante du serveur
// =====================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sante {
    pub version_protocole: u32,
    pub version_serveur: String,
    pub demarre_le: String,
    pub postes_connectes: usize,
    pub base_saine: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub derniere_sauvegarde: Option<String>,
    /// D43 : la base est neuve, le patron n'a pas encore fait
    /// l'assistant de creation du dossier.
    #[serde(default)]
    pub assistant_a_faire: bool,
}
