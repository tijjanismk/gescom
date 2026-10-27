//! L'etat partage entre tous les fils du serveur.

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;

use gescom_noyau::base::Base;
use gescom_noyau::registre::Registre;

use crate::canal::Canal;

pub struct Serveur {
    /// La cible est-elle PostgreSQL ? Decide ou poser les sauvegardes
    /// et comment les faire (VACUUM INTO ou pg_dump).
    pub est_postgres: bool,
    /// La base, sur l'un ou l'autre moteur. Depuis la v3 (D-2, D22),
    /// c'est la SEULE : chaque commande, `/connexion`, `/deconnexion`,
    /// les permissions et la sauvegarde passent par elle. La connexion
    /// SQLite brute d'avant (D11) ne sert plus qu'au demarrage, pour
    /// preparer le fichier, puis se ferme.
    pub base: Mutex<Base>,
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
    /// L'ecoute : « 0.0.0.0 » (toutes les cartes), « 127.0.0.1 » (ce
    /// poste seulement), ou une adresse reglee a la main.
    pub hote: String,
    /// Le port et l'ecoute viennent de `serveur.json` (ou du defaut), que
    /// le panneau Reseau de la console peut reecrire. Faux quand `--port`
    /// ou `--hote` les ont fixes : le fichier n'y changerait rien.
    pub reseau_modifiable: bool,
    /// Lance par le gestionnaire de services Windows (D12).
    pub en_service: bool,
    /// Pose par `POST /reseau` une fois la reponse partie : la boucle
    /// s'arrete et le serveur redemarre sur le nouveau reglage.
    pub redemarrage: AtomicBool,
    /// La recherche automatique (UDP) : `Ok(port)` en ecoute, sinon
    /// pourquoi non — pour le panneau Reseau.
    pub decouverte: Mutex<Result<u16, String>>,
    /// Un changement de reseau a la fois : deux clics ne reecrivent pas
    /// `serveur.json` deux fois de travers.
    pub changement_reseau: Mutex<()>,
    pub demarre_le: String,
    pub derniere_sauvegarde: Mutex<Option<String>>,
    /// Les erreurs des caisses : dix par minute et par poste (B-2).
    pub limiteur_postes: Mutex<crate::journal_poste::Limiteur>,
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
