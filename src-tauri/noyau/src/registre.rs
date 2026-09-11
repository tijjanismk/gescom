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

/// Le pendant de `Contexte`, pour une commande portee sur `Base`.
///
/// Existe pour la meme raison que `Base` coexiste avec `Connection`
/// (D11, AI_CONTEXT/DECISIONS.md) : brancher une commande deja portee
/// au registre ne doit rien changer a celles qui ne le sont pas.
pub struct ContexteBase<'a> {
    pub base: &'a mut crate::base::Base,
    pub appelant: &'a Appelant,
}

pub type PoigneeBase = fn(&mut ContexteBase, Value) -> Result<Value, String>;

pub struct Entree {
    pub poignee: Poignee,
    /// La meme commande, sur `Base` — quand elle est portee. `None`
    /// pour les commandes qui ne le sont pas encore : le serveur
    /// refuse alors clairement sur une cible PostgreSQL, plutot que de
    /// deviner (D11).
    ///
    /// N'entre JAMAIS en jeu sur une cible fichier : `poignee` reste le
    /// seul chemin la ou il marche deja. C'est ce qui rend ce champ
    /// sans risque a remplir au fur et a mesure — aucune commande
    /// existante ne change de comportement le jour ou on lui en ajoute
    /// un.
    pub poignee_base: Option<PoigneeBase>,
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
        self.entrees
            .insert(nom, Entree { poignee: p, poignee_base: None, permission: None, ecrit: false });
        self
    }

    pub fn ecriture(
        &mut self,
        nom: &'static str,
        permission: &'static str,
        p: Poignee,
    ) -> &mut Self {
        self.entrees.insert(
            nom,
            Entree { poignee: p, poignee_base: None, permission: Some(permission), ecrit: true },
        );
        self
    }

    /// Branche la version `Base` d'une commande DEJA enregistree.
    ///
    /// A appeler apres `lecture`/`ecriture`/`ecriture_libre` pour le
    /// meme nom — jamais avant, sinon il n'y a rien a completer. Ne
    /// change rien sur une cible fichier : `poignee` reste le chemin
    /// emprunte tant que `Connection` existe (voir `ContexteBase`).
    pub fn aussi_sur_base(&mut self, nom: &'static str, p: PoigneeBase) -> &mut Self {
        if let Some(e) = self.entrees.get_mut(nom) {
            e.poignee_base = Some(p);
        }
        self
    }

    /// Une ecriture que TOUT UTILISATEUR CONNECTE peut faire.
    ///
    /// Il n'y en a presque pas, et c'est voulu. Le cas reel : changer
    /// SON PROPRE mot de passe. Il exigeait « utilisateurs:gerer », si
    /// bien qu'un caissier — a qui l'amorcage impose justement de
    /// changer son mot de passe a la premiere connexion — ne pouvait
    /// pas le faire. Il se retrouvait enferme dehors des le premier
    /// jour.
    ///
    /// Nommee explicitement pour qu'on ne l'utilise pas par paresse :
    /// une commande sans permission se voit dans cette liste.
    pub fn ecriture_libre(&mut self, nom: &'static str, p: Poignee) -> &mut Self {
        self.entrees
            .insert(nom, Entree { poignee: p, poignee_base: None, permission: None, ecrit: true });
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
