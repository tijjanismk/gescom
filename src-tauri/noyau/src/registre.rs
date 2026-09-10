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

use std::collections::BTreeMap;
use rusqlite::Connection;
use serde_json::Value;

/// Qui appelle, depuis quelle machine.
///
/// En monoposte, `poste_id` et `utilisateur_id` sont ceux du poste
/// unique : le meme code s'execute, avec ou sans reseau. C'est la
/// condition pour que le monoposte ne devienne pas un chemin de code
/// a part, qu'on teste moins et qui casse en silence.
pub struct Appelant {
    pub utilisateur_id: String,
    pub role: String,
    pub poste_id: String,
}

pub struct Contexte<'a> {
    /// Mutable, parce que les ecritures d'argent ouvrent une
    /// transaction : `Connection::transaction` exige `&mut`. Les
    /// lectures s'en accommodent — un `&mut` se reprete en `&`.
    ///
    /// Le verrou reste celui du serveur : une seule commande s'execute
    /// a la fois, et c'est ce qui rend le multiposte sur sans avoir
    /// touche aux compteurs de stock.
    pub conn: &'a mut Connection,
    pub appelant: &'a Appelant,
}

pub type Poignee = fn(&mut Contexte, Value) -> Result<Value, String>;

pub struct Entree {
    pub poignee: Poignee,
    /// Permission requise (cf. `portes`), ou `None` si ouverte a tous
    /// les connectes.
    pub permission: Option<&'static str>,
    /// Une commande qui ECRIT est refusee au role lecture et emet un
    /// evenement sur le canal. Une lecture, non.
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

    pub fn lecture(&mut self, nom: &'static str, p: Poignee) -> &mut Self {
        self.entrees.insert(nom, Entree { poignee: p, permission: None, ecrit: false });
        self
    }

    pub fn ecriture(
        &mut self,
        nom: &'static str,
        permission: &'static str,
        p: Poignee,
    ) -> &mut Self {
        self.entrees
            .insert(nom, Entree { poignee: p, permission: Some(permission), ecrit: true });
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
