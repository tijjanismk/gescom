//! L'etat partage entre tous les fils du serveur.

use std::collections::HashMap;
use std::sync::Mutex;

use rusqlite::Connection;
use gescom_noyau::base::Base;
use gescom_noyau::registre::Registre;

use crate::canal::Canal;

pub struct Serveur {
    /// La connexion SQLite brute, pour les 186 commandes pas encore
    /// portees sur `Base` (voir D11 dans AI_CONTEXT/DECISIONS.md).
    ///
    /// `None` quand la cible est PostgreSQL : il n'y a alors AUCUNE
    /// connexion SQLite a donner a ces commandes, et il ne faut surtout
    /// pas en ouvrir une quand meme sous un nom bizarre — c'est
    /// exactement le piege que D11 nomme : « il créerait un fichier
    /// SQLite portant ce nom, l'amorcerait, et tout marcherait — sur
    /// une base vide ». Chaque appelant de ce champ doit donc refuser
    /// clairement au lieu de suivre ce chemin.
    pub conn: Option<Mutex<Connection>>,
    /// La base, sur l'un ou l'autre moteur. C'est elle que les
    /// commandes PORTEES appellent — `catalogue::*_sur`,
    /// `comptoir::*_sur`, `argent::*_sur_base`, `dossiers::*_sur`.
    ///
    /// Sur une cible fichier, c'est une SECONDE connexion vers le meme
    /// fichier que `conn` (SQLite en WAL le permet) : la transition
    /// prevue par D11, une `Base` pour ce qui est porte, une
    /// `Connection` pour le reste, jusqu'a ce que tout le soit.
    ///
    /// Encore lu par personne : aucune commande portee n'est branchee
    /// au registre. C'est la suite de D11, pas cette etape — qui pose
    /// seulement le fait que le serveur SAIT tenir une `Base`, sur les
    /// deux moteurs, sans rien changer pour les 186 qui ne le sont pas.
    #[allow(dead_code)]
    pub base: Mutex<Base>,
    pub chemin_base: String,
    pub canal: Canal,
    pub registre: Registre,
    /// jeton en clair -> identifiant de session.
    ///
    /// En memoire uniquement : la base ne garde que le hash. Un
    /// redemarrage vide cette table, donc reconnecte les postes — c'est
    /// le prix a payer pour ne pas verifier un bcrypt a chaque appel.
    pub jetons: Mutex<HashMap<String, String>>,
    /// Le port ecoute, pour que la console sache s'annoncer.
    pub port: u16,
    pub demarre_le: String,
    pub derniere_sauvegarde: Mutex<Option<String>>,
}

impl Serveur {
    pub fn session_du_jeton(&self, jeton: &str) -> Option<String> {
        self.jetons.lock().ok()?.get(jeton).cloned()
    }

    pub fn enregistrer_jeton(&self, jeton: String, session_id: String) {
        if let Ok(mut m) = self.jetons.lock() {
            m.insert(jeton, session_id);
        }
    }

    pub fn oublier_jeton(&self, jeton: &str) {
        if let Ok(mut m) = self.jetons.lock() {
            m.remove(jeton);
        }
    }
}
