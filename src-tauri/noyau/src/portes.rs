//! Les portes : qui a le droit de faire quoi.
//!
//! ## Ce que c'etait
//!
//! Un `match` code en dur sur le NOM du role : patron peut tout,
//! employe a une liste figee, lecture ne lit que. La colonne
//! `role.permissions` existait en base et n'etait jamais lue. Ajouter
//! un role — un caissier, un magasinier — demandait de recompiler, et
//! donner une permission de plus a une seule personne etait impossible.
//!
//! ## Ce que c'est
//!
//! Les permissions vivent en base :
//!
//! - `role.permissions` — la liste du role, en JSON ;
//! - `role.acces_total` — le role passe avant toute liste ;
//! - `utilisateur_permission` — ce qu'on ajoute ou retire A UNE
//!   PERSONNE, par-dessus son role.
//!
//! Le CATALOGUE, lui, reste dans le code : c'est la liste des
//! permissions que les commandes verifient reellement. Une permission
//! absente du catalogue est refusee, meme si quelqu'un l'a ecrite en
//! base — une faute de frappe dans un role ne doit pas ouvrir une porte
//! qui n'existe pas, ni en fermer une qu'on croyait ouverte.
//!
//! ## Liste blanche, toujours
//!
//! Une commande ajoutee demain est refusee par defaut aux roles
//! restreints. L'inverse ouvrirait chaque nouveaute a tout le monde
//! sans que personne ne le remarque.
//!
//! Les deux roles a `acces_total` sont l'exception assumee : sans eux,
//! chaque nouvelle commande serait invisible au patron jusqu'a ce que
//! quelqu'un pense a cocher une case.

use std::collections::HashSet;
use std::fmt;

// =====================================================================
//  LE CATALOGUE
// =====================================================================

/// Une permission, telle qu'un ecran doit la presenter.
pub struct Permission {
    /// Ce que le code verifie. Ne change jamais : il est ecrit en base.
    pub code: &'static str,
    /// Ce qu'un commercant lit.
    pub libelle: &'static str,
    /// Pour regrouper a l'ecran.
    pub groupe: &'static str,
}

/// Toutes les permissions que les commandes verifient reellement.
///
/// Etablie en relevant les `r.ecriture(nom, permission, …)` du socle —
/// pas en imaginant ce qui serait utile. Une permission qui ne figure
/// dans aucune commande serait une case a cocher sans effet, et c'est
/// pire que pas de case du tout.
///
/// ⚠️ Les LECTURES ne sont pas encore filtrees : `r.lecture(…)` ne
/// demande aucune permission. N'importe quel utilisateur connecte peut
/// donc tout lire. Le catalogue ne contient volontairement aucune
/// permission en `:lire` tant que c'est vrai.
pub const CATALOGUE: &[Permission] = &[
    // --- Vente ---
    Permission { code: "ventes:creer", libelle: "Enregistrer une vente", groupe: "Vente" },
    Permission { code: "paiements:creer", libelle: "Encaisser un paiement", groupe: "Vente" },
    Permission { code: "retours:creer", libelle: "Enregistrer un retour client", groupe: "Vente" },
    Permission { code: "clients:creer", libelle: "Créer un client", groupe: "Vente" },
    Permission { code: "clients:modifier", libelle: "Modifier une fiche client", groupe: "Vente" },
    Permission { code: "creances:gerer", libelle: "Gérer les créances et relances", groupe: "Vente" },
    // --- Pieces ---
    Permission { code: "pieces:creer", libelle: "Créer et convertir des pièces", groupe: "Pièces" },
    Permission { code: "avoirs:gerer", libelle: "Gérer les avoirs", groupe: "Pièces" },
    Permission { code: "livraisons:enregistrer", libelle: "Enregistrer livraisons et réceptions", groupe: "Pièces" },
    Permission { code: "modeles:gerer", libelle: "Modifier les modèles de documents", groupe: "Pièces" },
    // --- Caisse ---
    Permission { code: "caisse:mouvementer", libelle: "Ouvrir la caisse et saisir des mouvements", groupe: "Caisse" },
    Permission { code: "caisse:configurer", libelle: "Configurer le mode de caisse", groupe: "Caisse" },
    Permission { code: "cheques:gerer", libelle: "Gérer les chèques", groupe: "Caisse" },
    // --- Stock ---
    Permission { code: "articles:creer", libelle: "Créer et modifier des articles", groupe: "Stock" },
    Permission { code: "stock:transferer", libelle: "Transférer entre magasins", groupe: "Stock" },
    Permission { code: "depots:gerer", libelle: "Gérer les magasins", groupe: "Stock" },
    // --- Achat ---
    Permission { code: "achats:creer", libelle: "Enregistrer un achat", groupe: "Achat" },
    Permission { code: "fournisseurs:regler", libelle: "Régler un fournisseur", groupe: "Achat" },
    // --- Administration ---
    Permission { code: "utilisateurs:gerer", libelle: "Gérer les utilisateurs et les rôles", groupe: "Administration" },
    Permission { code: "parametres:modifier", libelle: "Modifier les paramètres", groupe: "Administration" },
    Permission { code: "postes:gerer", libelle: "Gérer les postes du réseau", groupe: "Administration" },
    Permission { code: "sauvegarde:lancer", libelle: "Lancer une sauvegarde", groupe: "Administration" },
    Permission { code: "chantiers:gerer", libelle: "TVA, irrécouvrables, expiration des avoirs", groupe: "Administration" },
];

/// Cette permission existe-t-elle ?
///
/// Tout ce qui vient de la base passe par ici. Une faute de frappe dans
/// un role ne doit ni ouvrir une porte qui n'existe pas, ni laisser
/// croire qu'une porte est ouverte.
pub fn existe(code: &str) -> bool {
    CATALOGUE.iter().any(|p| p.code == code)
}

/// Le nom du role qui ne peut jamais etre enferme dehors.
///
/// Il passe avant toute lecture de base : meme si les permissions
/// etaient corrompues, meme si quelqu'un lui retirait tout, il entre.
/// Sans cette garantie, une mauvaise manipulation sur les roles rendrait
/// l'application definitivement inadministrable.
pub const SUPERADMIN: &str = "superadmin";

// =====================================================================
//  L'ERREUR
// =====================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErreurPermission {
    /// Le role existe mais ne couvre pas cette action.
    Refuse { role: String, permission: String },
}

impl fmt::Display for ErreurPermission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErreurPermission::Refuse { role, permission } => write!(
                f,
                "PERMISSION_REFUSEE — le rôle « {role} » ne permet pas « {permission} »."
            ),
        }
    }
}

pub struct ContexteUtilisateur {
    pub id: String,
    pub role: String,
}

// =====================================================================
//  LA VERIFICATION
// =====================================================================

/// Tout ce que cette personne a le droit de faire.
///
/// = les permissions de son role
///   + celles qu'on lui a ajoutees personnellement
///   − celles qu'on lui a retirees personnellement.
///
/// Le retrait l'emporte sur l'ajout : entre deux lectures possibles
/// d'un reglage contradictoire, on choisit la plus fermee.
///
/// Un role a `acces_total` rend tout le catalogue, et les retraits
/// personnels ne s'y appliquent pas — sinon on pourrait enfermer dehors
/// le seul compte capable de reparer.
/// La REGLE, isolee de toute lecture.
///
/// Les permissions se lisent sur deux moteurs pendant le portage. La
/// regle, elle, ne doit exister qu'UNE fois : deux copies d'un calcul
/// de droits finissent toujours par diverger, et personne ne s'en
/// apercoit avant qu'un caissier fasse ce qu'il ne devait pas.
///
/// Ici, seule la regle. Les deux fonctions ci-dessous ne font que lui
/// apporter ce qu'elles ont lu.
pub fn calculer_permissions(
    role: &str,
    liste_json: &str,
    acces_total: bool,
    reglages: &[(String, bool)],
) -> HashSet<String> {
    if role == SUPERADMIN || acces_total {
        return CATALOGUE.iter().map(|p| p.code.to_string()).collect();
    }

    let mut acquises: HashSet<String> = serde_json::from_str::<Vec<String>>(liste_json)
        .unwrap_or_default()
        .into_iter()
        // Filtre par le catalogue : une permission ecrite en base mais
        // inconnue du code ne donne aucun droit.
        .filter(|c| existe(c))
        .collect();

    // Le retrait l'emporte sur l'ajout : entre deux lectures possibles
    // d'un reglage contradictoire, on choisit la plus fermee.
    for (code, accorde) in reglages {
        if !existe(code) {
            continue;
        }
        if *accorde {
            acquises.insert(code.clone());
        }
    }
    for (code, accorde) in reglages {
        if !*accorde {
            acquises.remove(code);
        }
    }

    acquises
}

/// Tout ce que cette personne a le droit de faire — lecture SQLite.
pub fn permissions_de(
    conn: &rusqlite::Connection,
    utilisateur_id: &str,
    role: &str,
) -> HashSet<String> {
    if role == SUPERADMIN {
        return CATALOGUE.iter().map(|p| p.code.to_string()).collect();
    }

    let (liste_json, acces_total): (String, i64) = conn
        .query_row(
            "SELECT COALESCE(permissions, '[]'), COALESCE(acces_total, 0)
             FROM role WHERE nom = ?1",
            rusqlite::params![role],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap_or_else(|_| ("[]".to_string(), 0));

    let mut reglages: Vec<(String, bool)> = Vec::new();
    if let Ok(mut st) = conn.prepare(
        "SELECT permission, accorde FROM utilisateur_permission
         WHERE utilisateur_id = ?1",
    ) {
        if let Ok(lignes) = st.query_map(rusqlite::params![utilisateur_id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? != 0))
        }) {
            reglages.extend(lignes.flatten());
        }
    }

    calculer_permissions(role, &liste_json, acces_total != 0, &reglages)
}

/// La meme chose, sur l'un ou l'autre moteur.
pub fn permissions_de_sur(
    base: &mut crate::base::Base,
    utilisateur_id: &str,
    role: &str,
) -> HashSet<String> {
    if role == SUPERADMIN {
        return CATALOGUE.iter().map(|p| p.code.to_string()).collect();
    }

    let ligne = base
        .lire_une(
            "SELECT COALESCE(permissions, '[]'), COALESCE(acces_total, 0)
             FROM role WHERE nom = ?1",
            &crate::parametres![role],
            |r| Ok((r.get::<String>(0)?, r.get::<i64>(1)?)),
        )
        .ok()
        .flatten()
        .unwrap_or_else(|| ("[]".to_string(), 0));

    let reglages = base
        .lire_plusieurs(
            "SELECT permission, accorde FROM utilisateur_permission
             WHERE utilisateur_id = ?1",
            &crate::parametres![utilisateur_id],
            |r| Ok((r.get::<String>(0)?, r.get::<i64>(1)? != 0)),
        )
        .unwrap_or_default();

    calculer_permissions(role, &ligne.0, ligne.1 != 0, &reglages)
}

/// Cette personne peut-elle faire cela ?
///
/// `conn` parce que les permissions vivent en base : les garder dans le
/// code obligeait a recompiler pour ajouter un role, et rendait
/// impossible d'accorder une permission a une seule personne.
/// Cette personne peut-elle faire cela ? — sur l'un ou l'autre moteur.
pub fn verifier_permission_sur(
    base: &mut crate::base::Base,
    contexte: &ContexteUtilisateur,
    permission: &str,
) -> Result<(), ErreurPermission> {
    let refus = || ErreurPermission::Refuse {
        role: contexte.role.clone(),
        permission: permission.to_string(),
    };
    if !existe(permission) {
        return Err(refus());
    }
    if contexte.role == SUPERADMIN {
        return Ok(());
    }
    if permissions_de_sur(base, &contexte.id, &contexte.role).contains(permission) {
        Ok(())
    } else {
        Err(refus())
    }
}

pub fn verifier_permission(
    conn: &rusqlite::Connection,
    contexte: &ContexteUtilisateur,
    permission: &str,
) -> Result<(), ErreurPermission> {
    let refus = || ErreurPermission::Refuse {
        role: contexte.role.clone(),
        permission: permission.to_string(),
    };

    // Une permission hors catalogue est refusee a tout le monde, y
    // compris au superadmin : elle ne correspond a aucune commande, la
    // laisser passer masquerait une faute de frappe cote code.
    if !existe(permission) {
        return Err(refus());
    }
    if contexte.role == SUPERADMIN {
        return Ok(());
    }
    if permissions_de(conn, &contexte.id, &contexte.role).contains(permission) {
        Ok(())
    } else {
        Err(refus())
    }
}
