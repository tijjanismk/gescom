//! Porte d'écriture unique — point de contrôle central.
//!
//! Toute opération sensible passe par ce module AVANT d'appeler la persistance.
//! Il vérifie les droits, puis délègue à la couche persistance.
//!
//! Structure :
//!   1. ContexteUtilisateur — qui fait l'action
//!   2. verifier_permission  — le garde d'entrée (permissif au démarrage)
//!   3. Opérations métier    — une fonction par type d'acte

use rusqlite::Connection;

use crate::persistance::factures::{creer_facture_brouillon, valider_facture};
use crate::persistance::mouvements_stock::enregistrer_ajustement;
use crate::persistance::paiements::enregistrer_paiement;
use crate::persistance::transferts::inserer_transfert;
use crate::persistance::ventes::{inserer_vente_complete, ParamsLigne};

// =====================================================================
//  CONTEXTE UTILISATEUR
// =====================================================================

/// Qui fait l'action — passé à chaque opération.
#[derive(Debug, Clone)]
pub struct ContexteUtilisateur {
    pub id: String,
    pub role: String,    // "patron" / "employe" / "lecture"
    pub origine: String, // identifiant de la machine
}

// =====================================================================
//  ERREURS
// =====================================================================

#[derive(Debug)]
pub enum ErreurPorte {
    /// L'utilisateur n'a pas le droit de faire cette opération.
    PermissionRefusee { role: String, permission: String },
    /// Erreur de base de données.
    BaseDeDonnees(rusqlite::Error),
}

impl From<rusqlite::Error> for ErreurPorte {
    fn from(e: rusqlite::Error) -> Self {
        ErreurPorte::BaseDeDonnees(e)
    }
}

impl std::fmt::Display for ErreurPorte {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErreurPorte::PermissionRefusee { role, permission } => {
                write!(
                    f,
                    "Permission refusée : rôle '{}' ne peut pas '{}'",
                    role, permission
                )
            }
            ErreurPorte::BaseDeDonnees(e) => write!(f, "Erreur base : {}", e),
        }
    }
}

// =====================================================================
//  VÉRIFICATION DES PERMISSIONS
// =====================================================================

/// Vérifie qu'un utilisateur a le droit de faire une opération.
///
/// Permissif au démarrage — on resserre ici quand un client le demande,
/// sans toucher au reste du code. C'est le point de contrôle unique.
///
/// Permissions définies :
///   "ventes:creer"         — créer une vente
///   "paiements:creer"      — enregistrer un paiement
///   "factures:creer"       — créer une facture brouillon
///   "factures:valider"     — valider une facture (la figer)
///   "stock:ajuster"        — ajustement/régularisation de stock (patron)
///   "stock:transferer"     — transfert entre dépôts
///   "articles:read_cout"   — voir le prix d'achat (champ protégé)
pub fn verifier_permission(ctx: &ContexteUtilisateur, permission: &str) -> Result<(), ErreurPorte> {
    let autorise = match (ctx.role.as_str(), permission) {
        // Le patron peut tout.
        ("patron", _) => true,

        // L'employé peut vendre et encaisser.
        ("employe", "ventes:creer") => true,
        ("employe", "paiements:creer") => true,
        ("employe", "factures:creer") => true,
        ("employe", "stock:transferer") => true,

        // L'employé ne peut PAS ajuster le stock ni valider les factures.
        ("employe", "stock:ajuster") => false,
        ("employe", "factures:valider") => false,

        // L'employé ne voit pas le prix d'achat.
        ("employe", "articles:read_cout") => false,

        // Lecture seule : rien d'autre.
        ("lecture", _) => false,

        // Tout le reste : refusé.
        _ => false,
    };

    if autorise {
        Ok(())
    } else {
        Err(ErreurPorte::PermissionRefusee {
            role: ctx.role.clone(),
            permission: permission.to_string(),
        })
    }
}

// =====================================================================
//  OPÉRATIONS MÉTIER
// =====================================================================

/// Crée une vente après vérification des droits.
pub fn op_creer_vente<'a>(
    conn: &mut Connection,
    ctx: &ContexteUtilisateur,
    client_id: &str,
    depot_id: &str,
    mode_reglement: &str,
    lignes: &[ParamsLigne<'a>],
) -> Result<String, ErreurPorte> {
    verifier_permission(ctx, "ventes:creer")?;

    let vente_id = inserer_vente_complete(
        conn,
        client_id,
        depot_id,
        mode_reglement,
        lignes,
        &ctx.id,
        &ctx.origine,
    )?;

    Ok(vente_id)
}

/// Enregistre un paiement après vérification des droits.
pub fn op_enregistrer_paiement(
    conn: &mut Connection,
    ctx: &ContexteUtilisateur,
    vente_id: &str,
    montant: i64,
    mode: &str,
    avoir_id: Option<&str>,
) -> Result<String, ErreurPorte> {
    verifier_permission(ctx, "paiements:creer")?;

    let id = enregistrer_paiement(
        conn,
        vente_id,
        montant,
        mode,
        avoir_id,
        &ctx.id,
        &ctx.origine,
    )?;

    Ok(id)
}

/// Crée une facture brouillon après vérification des droits.
pub fn op_creer_facture(
    conn: &Connection,
    ctx: &ContexteUtilisateur,
    vente_id: &str,
    total: i64,
    annee: i32,
) -> Result<String, ErreurPorte> {
    verifier_permission(ctx, "factures:creer")?;

    let id = creer_facture_brouillon(conn, vente_id, total, annee, &ctx.id, &ctx.origine)?;

    Ok(id)
}

/// Valide une facture après vérification des droits.
/// Seul le patron peut valider — la facture devient immuable.
pub fn op_valider_facture(
    conn: &Connection,
    ctx: &ContexteUtilisateur,
    facture_id: &str,
) -> Result<(), ErreurPorte> {
    verifier_permission(ctx, "factures:valider")?;

    valider_facture(conn, facture_id, &ctx.id, &ctx.origine)?;

    Ok(())
}

/// Ajuste le stock après vérification des droits.
/// Réservé au patron — motif obligatoire.
pub fn op_ajuster_stock(
    conn: &mut Connection,
    ctx: &ContexteUtilisateur,
    article_id: &str,
    depot_id: &str,
    delta: f64,
    type_mouvement: &str,
    motif: &str,
) -> Result<(), ErreurPorte> {
    verifier_permission(ctx, "stock:ajuster")?;

    enregistrer_ajustement(
        conn,
        article_id,
        depot_id,
        delta,
        type_mouvement,
        motif,
        &ctx.id,
        &ctx.origine,
    )?;

    Ok(())
}

/// Transfère du stock entre dépôts après vérification des droits.
pub fn op_transferer_stock(
    conn: &mut Connection,
    ctx: &ContexteUtilisateur,
    depot_source_id: &str,
    depot_dest_id: &str,
    article_id: &str,
    quantite: f64,
) -> Result<String, ErreurPorte> {
    verifier_permission(ctx, "stock:transferer")?;

    let id = inserer_transfert(
        conn,
        depot_source_id,
        depot_dest_id,
        article_id,
        quantite,
        &ctx.id,
        &ctx.origine,
    )?;

    Ok(id)
}

// =====================================================================
//  TESTS
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistance::initialiser_tables;
    use rusqlite::Connection;

    fn base_test() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        initialiser_tables(&conn).unwrap();
        conn
    }

    fn patron() -> ContexteUtilisateur {
        ContexteUtilisateur {
            id: "user-patron".to_string(),
            role: "patron".to_string(),
            origine: "machine-1".to_string(),
        }
    }

    fn employe() -> ContexteUtilisateur {
        ContexteUtilisateur {
            id: "user-employe".to_string(),
            role: "employe".to_string(),
            origine: "machine-1".to_string(),
        }
    }

    fn lecture() -> ContexteUtilisateur {
        ContexteUtilisateur {
            id: "user-lecture".to_string(),
            role: "lecture".to_string(),
            origine: "machine-1".to_string(),
        }
    }

    // ---- Tests de permissions ----

    #[test]
    fn patron_peut_tout() {
        assert!(verifier_permission(&patron(), "ventes:creer").is_ok());
        assert!(verifier_permission(&patron(), "stock:ajuster").is_ok());
        assert!(verifier_permission(&patron(), "factures:valider").is_ok());
        assert!(verifier_permission(&patron(), "articles:read_cout").is_ok());
    }

    #[test]
    fn employe_peut_vendre_pas_ajuster() {
        assert!(verifier_permission(&employe(), "ventes:creer").is_ok());
        assert!(verifier_permission(&employe(), "paiements:creer").is_ok());
        assert!(verifier_permission(&employe(), "factures:creer").is_ok());

        // L'employé ne peut pas ajuster le stock ni valider les factures.
        assert!(verifier_permission(&employe(), "stock:ajuster").is_err());
        assert!(verifier_permission(&employe(), "factures:valider").is_err());
        assert!(verifier_permission(&employe(), "articles:read_cout").is_err());
    }

    #[test]
    fn lecture_ne_peut_rien_ecrire() {
        assert!(verifier_permission(&lecture(), "ventes:creer").is_err());
        assert!(verifier_permission(&lecture(), "paiements:creer").is_err());
        assert!(verifier_permission(&lecture(), "stock:ajuster").is_err());
    }

    #[test]
    fn message_erreur_permission_lisible() {
        let err = verifier_permission(&employe(), "stock:ajuster").unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("employe"));
        assert!(msg.contains("stock:ajuster"));
    }

    // ---- Test d'intégration : vente complète via la porte ----

    #[test]
    fn vente_complete_via_porte() {
        let mut conn = base_test();

        // Préparer les données minimales.
        conn.execute(
            "INSERT INTO categorie (id,nom,schema_attributs,actif,cree_le,modifie_le,origine)
             VALUES ('cat1','Alim','[]',1,'2024-01-01','2024-01-01','m1')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO article
             (id,nom,categorie_id,unite_base,gere_en_stock,attributs,actif,
              cree_le,modifie_le,cree_par,modifie_par,origine)
             VALUES ('art1','Sucre','cat1','kg',1,'{}',1,
                     '2024-01-01','2024-01-01','u1','u1','m1')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO unite_vente
             (id,article_id,libelle,facteur,prix_reference,actif,
              cree_le,modifie_le,cree_par,modifie_par,origine)
             VALUES ('uv1','art1','kg',1.0,800,1,
                     '2024-01-01','2024-01-01','u1','u1','m1')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO depot (id,nom,est_defaut,actif,cree_le,modifie_le,origine)
             VALUES ('dep1','Principal',1,1,'2024-01-01','2024-01-01','m1')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO stock_depot (id,article_id,depot_id,quantite)
             VALUES ('sd1','art1','dep1',100.0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO role (id,nom,permissions,cree_le,modifie_le,origine)
             VALUES ('r1','patron','[]','2024-01-01','2024-01-01','m1')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO utilisateur
             (id,nom,role_id,actif,cree_le,modifie_le,origine)
             VALUES ('user-patron','Amadou','r1',1,'2024-01-01','2024-01-01','m1')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO client
             (id,code,nom,est_generique,actif,cree_le,modifie_le,
              cree_par,modifie_par,origine)
             VALUES ('cli1','CLIENT00000','Comptant',1,1,
                     '2024-01-01','2024-01-01','system','system','m1')",
            [],
        )
        .unwrap();

        let ctx = patron();

        let ligne = ParamsLigne {
            article_id: "art1",
            unite_vente_id: "uv1",
            depot_source_id: "dep1",
            source_approvisionnement: "stock",
            quantite: 5.0,
            facteur: 1.0,
            prix_reference: 800,
            prix_pratique: 800,
        };

        // Étape 1 : créer la vente via la porte.
        let vente_id =
            op_creer_vente(&mut conn, &ctx, "cli1", "dep1", "comptant", &[ligne]).unwrap();

        // Étape 2 : encaisser via la porte.
        op_enregistrer_paiement(&mut conn, &ctx, &vente_id, 4000, "especes", None).unwrap();

        let vente = lire_vente(&conn, &vente_id).unwrap().unwrap();
        assert_eq!(vente.statut, "payee");
    }

    #[test]
    fn employe_bloque_sur_ajustement_stock() {
        let mut conn = base_test();
        let ctx = employe();

        let result = op_ajuster_stock(&mut conn, &ctx, "art1", "dep1", -2.0, "ajustement", "test");

        // L'employé doit être bloqué avant même d'atteindre la base.
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ErreurPorte::PermissionRefusee { .. }
        ));
    }
}
