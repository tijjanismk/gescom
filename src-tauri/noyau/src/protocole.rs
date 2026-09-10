//! Le contrat entre le client et le serveur.
//!
//! Ces types sont compiles dans les DEUX executables. C'est leur seule
//! raison d'etre : un champ renomme d'un cote casse la compilation de
//! l'autre, au lieu d'arriver silencieusement en `null` sur un poste
//! caisse un samedi de marche.

use serde::{Deserialize, Serialize};

/// Port par defaut du serveur Gescom.
///
/// 7300 : au-dessus des ports systeme, hors des plages courantes
/// (3000 dev, 5432 Postgres, 8080 proxy) pour eviter la collision sur
/// un poste ou tourne deja autre chose.
pub const PORT_DEFAUT: u16 = 7300;

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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Identite {
    pub jeton: String,
    pub utilisateur_id: String,
    pub utilisateur_nom: String,
    pub role: String,
    pub doit_changer_mdp: bool,
    pub poste_id: String,
    pub expire_le: String,
    /// Vrai si la caisse est nominative (un tiroir par utilisateur).
    pub caisse_par_utilisateur: bool,
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
    /// Etat de la licence, en clair. `default` pour qu'un poste d'une
    /// version anterieure lise encore la reponse.
    #[serde(default)]
    pub licence: String,
    #[serde(default)]
    pub postes_max: u32,
}
