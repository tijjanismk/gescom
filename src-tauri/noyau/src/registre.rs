//! Le registre des commandes.
//!
//! Le v1 adressait ses 174 commandes par une chaine, via
//! `invoke("creer_vente")` — un nom que le pont Tauri resolvait sans
//! qu'aucun graphe d'import ne le voie. Le v2 garde ces memes noms
//! (le front n'a pas a savoir ou tourne le code) mais les rassemble
//! dans une table explicite, que le serveur peut interroger.
//!
//! ## Ce que ca permet, et qui manquait
//!
//! Enumerer ce qui existe (`/rpc/catalogue`), refuser proprement ce
//! qui n'existe pas — un poste caisse plus recent que le serveur
//! recoit `commande_inconnue` au lieu d'un silence —, et attacher une
//! permission a chaque nom en un seul endroit.
//!
//! ## Une seule facon de servir (v3, D-2 — D22)
//!
//! Chaque commande avait deux poignees : une sur `Connection` (le
//! serveur sur fichier SQLite), une sur `Base` (PostgreSQL), choisies
//! selon le moteur (D11). Le serveur sert desormais TOUT par `Base`,
//! sur les deux moteurs : une seule poignee par commande, celle que
//! les scenarios `*_base.rs` eprouvent. Les fonctions `fn(conn)` du
//! noyau restent pour la fenetre monoposte, qui ne passe pas par ici.

use std::collections::BTreeMap;
use serde_json::Value;

/// Qui appelle, depuis quelle machine.
pub struct Appelant {
    pub utilisateur_id: String,
    pub role: String,
    pub poste_id: String,
    /// Le dossier de la session (v3). Le serveur le pose sur la `Base`
    /// avant chaque commande.
    pub dossier_id: String,
    /// La session elle-meme : ce que `choisir_dossier` doit modifier.
    pub session_id: String,
}

/// Ce qu'une commande recoit : la base (placee sur le dossier de la
/// session) et qui appelle.
pub struct ContexteBase<'a> {
    pub base: &'a mut crate::base::Base,
    pub appelant: &'a Appelant,
}

pub type PoigneeBase = fn(&mut ContexteBase, Value) -> Result<Value, String>;

pub struct Entree {
    pub poignee_base: PoigneeBase,
    /// Permission requise (cf. `portes`), ou `None` si ouverte a tous
    /// les connectes.
    pub permission: Option<&'static str>,
    /// Une commande qui ECRIT emet un evenement sur le canal. Une
    /// lecture, non.
    pub ecrit: bool,
}

#[derive(Default)]
pub struct Registre {
    entrees: BTreeMap<&'static str, Entree>,
}

impl Registre {
    pub fn nouveau() -> Self {
        Self::default()
    }

    /// Une commande. `permission` absente = ouverte a tout connecte —
    /// rare et voulu : une ecriture sans permission (changer SON mot
    /// de passe) se voit ici ; `ecrit` dit si elle emet un evenement.
    pub fn sur_base(
        &mut self,
        nom: &'static str,
        permission: Option<&'static str>,
        ecrit: bool,
        p: PoigneeBase,
    ) -> &mut Self {
        self.entrees.insert(nom, Entree { poignee_base: p, permission, ecrit });
        self
    }

    pub fn trouver(&self, nom: &str) -> Option<&Entree> {
        self.entrees.get(nom)
    }

    pub fn noms(&self) -> Vec<&'static str> {
        self.entrees.keys().copied().collect()
    }

    pub fn len(&self) -> usize {
        self.entrees.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entrees.is_empty()
    }
}
