//! Quel tiroir, pour qui.
//!
//! Le v1 posait qu'il n'y a QU'UNE caisse ouverte a la fois (D46), et
//! c'est vrai de la plupart des boutiques de Bamako : un comptoir, un
//! tiroir, le patron qui compte le soir. Le v2 ne renverse pas ce
//! choix, il le rend explicite et lui donne une alternative.
//!
//! ## Pourquoi ce n'est pas un detail technique
//!
//! A deux caissiers sur un tiroir partage, l'ecart de cloture n'est
//! imputable a personne : c'est un chiffre que tout le monde conteste
//! et que personne ne corrige. A deux tiroirs, chacun repond du sien.
//! Le mode se choisit donc au niveau du COMMERCE, pas du poste.

use rusqlite::{Connection, params};

/// La caisse est-elle nominative ?
pub fn par_utilisateur(conn: &Connection) -> bool {
    conn.query_row(
        "SELECT valeur FROM config_app WHERE cle = 'caisse_par_utilisateur'",
        [],
        |r| r.get::<_, String>(0),
    )
    .map(|v| v == "1")
    .unwrap_or(false)
}

pub fn definir_par_utilisateur(conn: &Connection, actif: bool) -> Result<(), String> {
    // Basculer avec des caisses ouvertes laisserait des sessions sans
    // proprietaire dans un monde ou tout en a un — et la cloture
    // suivante ne saurait pas a qui reclamer l'ecart.
    let ouvertes: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM session_caisse WHERE statut = 'ouverte'",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    if ouvertes > 0 {
        return Err(
            "Fermer toutes les caisses avant de changer le mode de caisse."
                .to_string(),
        );
    }
    conn.execute(
        "INSERT INTO config_app (cle, valeur) VALUES ('caisse_par_utilisateur', ?1)
         ON CONFLICT(cle) DO UPDATE SET valeur = excluded.valeur",
        params![if actif { "1" } else { "0" }],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// La session de caisse a laquelle rattacher une operation.
///
/// Le refus vaut mieux que l'ecriture manquante : une operation
/// bloquee se voit, une ecriture absente ne se voit jamais.
pub fn exiger(
    conn: &Connection,
    utilisateur_id: Option<&str>,
) -> Result<String, String> {
    if par_utilisateur(conn) {
        let Some(uid) = utilisateur_id else {
            return Err(
                "CAISSE_SANS_UTILISATEUR — la caisse est nominative : \
                 cette operation doit indiquer qui l'enregistre."
                    .to_string(),
            );
        };
        return conn
            .query_row(
                "SELECT id FROM session_caisse
                 WHERE statut = 'ouverte' AND utilisateur_id = ?1
                 LIMIT 1",
                params![uid],
                |r| r.get(0),
            )
            .map_err(|_| {
                "CAISSE_FERMEE — votre caisse n'est pas ouverte. \
                 L'ouvrir pour enregistrer cette opération."
                    .to_string()
            });
    }

    conn.query_row(
        "SELECT id FROM session_caisse WHERE statut = 'ouverte' LIMIT 1",
        [],
        |r| r.get(0),
    )
    .map_err(|_| {
        "CAISSE_FERMEE — la caisse n'est pas ouverte. \
         L'ouvrir pour enregistrer cette opération."
            .to_string()
    })
}

// =====================================================================
//  SUR L'UN OU L'AUTRE MOTEUR
// =====================================================================
//
// Pour `creer_vente_sur_base` et `valider_facture_sur_base`. Deux
// variantes et non une seule generique : `Base` et `Transaction`
// n'ont pas de trait commun dans `base.rs`, et l'une des deux se
// verifie DANS une transaction deja ouverte — c'est le cas historique
// de `valider_facture`, garde tel quel plutot que deplace au passage.

use crate::base::Acces;
use crate::parametres;

/// La caisse est-elle nominative ? `config_app` n'est PAS cloisonnee :
/// un seul reglage vaut pour tous les dossiers, comme les autres
/// reglages generaux du poste.
pub fn par_utilisateur_sur(base: &mut impl Acces) -> bool {
    base.lire_une(
        "SELECT valeur FROM config_app WHERE cle = 'caisse_par_utilisateur'",
        &[],
        |r| r.get::<String>(0),
    )
    .ok()
    .flatten()
    .map(|v| v == "1")
    .unwrap_or(false)
}

/// La session de caisse a laquelle rattacher une operation, dans le
/// dossier courant. `session_caisse` est cloisonnee.
pub fn exiger_sur(base: &mut impl Acces, utilisateur_id: Option<&str>) -> Result<String, String> {
    let dossier = base.dossier().to_string();
    if par_utilisateur_sur(base) {
        let Some(uid) = utilisateur_id else {
            return Err(
                "CAISSE_SANS_UTILISATEUR — la caisse est nominative : cette \
                 opération doit indiquer qui l'enregistre."
                    .to_string(),
            );
        };
        return base
            .lire_une(
                "SELECT id FROM session_caisse
                 WHERE statut = 'ouverte' AND utilisateur_id = ?1 AND dossier_id = ?2
                 LIMIT 1",
                &parametres![uid, dossier],
                |r| r.get::<String>(0),
            )
            .map_err(|e| e.0)?
            .ok_or_else(|| {
                "CAISSE_FERMEE — votre caisse n'est pas ouverte. L'ouvrir pour \
                 enregistrer cette opération."
                    .to_string()
            });
    }

    base.lire_une(
        "SELECT id FROM session_caisse WHERE statut = 'ouverte' AND dossier_id = ?1 LIMIT 1",
        &parametres![dossier],
        |r| r.get::<String>(0),
    )
    .map_err(|e| e.0)?
    .ok_or_else(|| {
        "CAISSE_FERMEE — la caisse n'est pas ouverte. L'ouvrir pour \
         enregistrer cette opération."
            .to_string()
    })
}

/// Meme regle, depuis l'interieur d'une transaction deja ouverte.
///
/// Ne fait plus que rediriger : depuis que `Acces` couvre `Base` et
/// `Transaction`, une seule fonction porte la regle. Conservee pour
/// ses appelants ; `dossier` n'est plus lu, la transaction connait le
/// sien.
pub fn exiger_dans_tx(
    tx: &mut crate::base::Transaction,
    _dossier: &str,
    utilisateur_id: Option<&str>,
) -> Result<String, String> {
    exiger_sur(tx, utilisateur_id)
}
