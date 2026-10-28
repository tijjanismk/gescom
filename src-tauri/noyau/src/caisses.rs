//! Quel tiroir, pour qui.
//!
//! Le v1 posait qu'il n'y a QU'UNE caisse ouverte a la fois (D46), et
//! c'est vrai de la plupart des boutiques de Bamako : un comptoir, un
//! tiroir, le patron qui compte le soir. Le v2 avait voulu lui donner
//! une alternative : la caisse « par utilisateur » (un tiroir par
//! personne, `config_app.caisse_par_utilisateur = '1'`).
//!
//! ## Retiree (lot C.6, 27/10/2026)
//!
//! Elle n'a jamais marche : les dix-sept appelants de `exiger_sur`
//! passent `None` comme utilisateur — le mode actif aurait refuse tout
//! encaissement (`CAISSE_SANS_UTILISATEUR`) —, et l'ouverture refuse une
//! seconde session dans le dossier sans jamais ecrire
//! `session_caisse.utilisateur_id`. Aucun ecran ne la proposait.
//!
//! Plutot que de la laisser dormir, prete a casser la boutique au
//! premier clic, elle est refusee net jusqu'au jour ou le proprietaire
//! en aura besoin :
//! - le reglage qui la choisirait (`definir_par_utilisateur*`, commande
//!   `definir_mode_caisse`) rend `MODE_CAISSE_INDISPONIBLE` ;
//! - une base qui l'aurait deja (`'1'` en base) se comporte comme le
//!   mode par defaut : **un seul tiroir par dossier**, la caisse ouverte
//!   du dossier, quel que soit qui encaisse. `par_utilisateur*` rend
//!   toujours `false` ;
//! - `exiger*` ne lit plus le mode : l'utilisateur qu'on lui passe ne
//!   choisit aucun tiroir (il n'y en a qu'un).
//!
//! Pour la reprendre un jour : passer `auteur::courant()` la ou `None`
//! est ecrit, ecrire `utilisateur_id` a l'ouverture, et ne permettre une
//! seconde session que pour une autre personne (l'index partiel
//! `idx_caisse_ouverte_par_utilisateur` est deja en base).

use rusqlite::{Connection, params};

/// Le refus du reglage retire.
pub const MODE_INDISPONIBLE: &str = "MODE_CAISSE_INDISPONIBLE — la caisse par utilisateur \
     (un tiroir par personne) n'est pas disponible dans cette version de Gescom : \
     la boutique garde un seul tiroir par dossier.";

/// La caisse est-elle nominative ? Jamais depuis le lot C.6 — meme une
/// base qui porte encore `'1'` se sert d'un tiroir par dossier.
pub fn par_utilisateur(_conn: &Connection) -> bool {
    false
}

/// Choisir le mode de caisse. `true` est refuse (mode retire) ; `false`
/// remet le reglage a `'0'` — sans condition de caisse fermee : le
/// comportement ne change pas, c'est deja celui-la.
pub fn definir_par_utilisateur(conn: &Connection, actif: bool) -> Result<(), String> {
    if actif {
        return Err(MODE_INDISPONIBLE.to_string());
    }
    conn.execute(
        "INSERT INTO config_app (cle, valeur) VALUES ('caisse_par_utilisateur', ?1)
         ON CONFLICT(cle) DO UPDATE SET valeur = excluded.valeur",
        params!["0"],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// La session de caisse a laquelle rattacher une operation : LA caisse
/// ouverte. `_utilisateur_id` ne choisit rien (mode nominatif retire,
/// C.6) ; garde dans la signature pour le jour ou il reviendrait.
///
/// Le refus vaut mieux que l'ecriture manquante : une operation
/// bloquee se voit, une ecriture absente ne se voit jamais.
pub fn exiger(
    conn: &Connection,
    _utilisateur_id: Option<&str>,
) -> Result<String, String> {
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

/// La caisse est-elle nominative ? Jamais depuis le lot C.6 (voir
/// `par_utilisateur`). `config_app` n'est PAS cloisonnee : le reglage
/// valait pour tous les dossiers.
pub fn par_utilisateur_sur(_base: &mut impl Acces) -> bool {
    false
}

/// La session de caisse a laquelle rattacher une operation, dans le
/// dossier courant. `session_caisse` est cloisonnee. `_utilisateur_id`
/// ne choisit rien : un tiroir par dossier (C.6).
pub fn exiger_sur(base: &mut impl Acces, _utilisateur_id: Option<&str>) -> Result<String, String> {
    let dossier = base.dossier().to_string();
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

/// Ecrit un mouvement d'especes dans la caisse ouverte du dossier, et
/// rend son identifiant. Refuse `CAISSE_FERMEE` si aucune n'est ouverte
/// — a appeler DANS la transaction du geste, avant ses autres ecritures
/// (D42 : l'avance et le salaire pris dans le tiroir).
#[allow(clippy::too_many_arguments)]
pub fn mouvement_especes_sur(
    acces: &mut impl Acces,
    sens: &str,
    montant: i64,
    motif: &str,
    libelle: &str,
    operation_id: &str,
    auteur: &str,
    maintenant: &str,
) -> Result<String, String> {
    let session = exiger_sur(acces, None)?;
    let dossier = acces.dossier().to_string();
    let id = uuid::Uuid::new_v4().to_string();
    acces
        .executer(
            "INSERT INTO mouvement_caisse
             (id, session_id, sens, moyen, montant, motif, libelle,
              operation_id, date_mouvement, cree_le, cree_par, origine, dossier_id)
             VALUES (?1, ?2, ?3, 'especes', ?4, ?5, ?6, ?7, ?8, ?8, ?9, 'app', ?10)",
            &parametres![id.clone(), session, sens, montant, motif, libelle, operation_id, maintenant, auteur, dossier],
        )
        .map_err(|e| e.0)?;
    Ok(id)
}

/// Choisir le mode de caisse, sur l'un ou l'autre moteur. Meme regle
/// que `definir_par_utilisateur` : `true` refuse, `false` remet `'0'`.
pub fn definir_par_utilisateur_sur(base: &mut impl Acces, actif: bool) -> Result<(), String> {
    if actif {
        return Err(MODE_INDISPONIBLE.to_string());
    }
    base.executer(
        "INSERT INTO config_app (cle, valeur) VALUES ('caisse_par_utilisateur', ?1)
         ON CONFLICT (cle) DO UPDATE SET valeur = excluded.valeur",
        &parametres!["0"],
    )
    .map_err(|e| e.0)?;
    Ok(())
}
