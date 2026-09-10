//! L'etat partage entre tous les fils du serveur.

use std::collections::HashMap;
use std::sync::Mutex;

use rusqlite::Connection;
use gescom_noyau::registre::Registre;

use crate::canal::Canal;

pub struct Serveur {
    /// La base, derriere un verrou.
    ///
    /// SQLite serialise de toute facon les ecritures ; un pool de
    /// connexions ne ferait qu'avancer le point de contention sans le
    /// supprimer. Le jour ou la base passe a PostgreSQL, c'est ce
    /// champ — et lui seul — qui devient un pool.
    pub conn: Mutex<Connection>,
    pub chemin_base: String,
    pub canal: Canal,
    pub registre: Registre,
    /// jeton en clair -> identifiant de session.
    ///
    /// En memoire uniquement : la base ne garde que le hash. Un
    /// redemarrage vide cette table, donc reconnecte les postes — c'est
    /// le prix a payer pour ne pas verifier un bcrypt a chaque appel.
    pub jetons: Mutex<HashMap<String, String>>,
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
