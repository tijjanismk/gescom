//! Le duplicata (26/10/2026, PLAN-DUPLICATA) : le serveur compte les
//! impressions.
//!
//! L'ecran ne decide pas si une feuille est un original : deux postes
//! qui impriment la meme facture la verraient chacun « premiere ». Le
//! serveur garde chaque impression (`impression`) ; la premiere est
//! l'original, les suivantes des duplicatas, quel que soit le poste. La
//! regle est dans `coeur::impression`.
//!
//! - `lire_etat_impression_sur` : ce que l'APERCU doit montrer avant
//!   d'imprimer (l'aperçu ne compte pas) ;
//! - `noter_impression_sur` : appele au moment d'imprimer, rend le
//!   verdict qui fait foi ;
//! - `reimprimer_original_sur` : l'original du patron
//!   (`pieces:reimprimer_original`, verifie par le serveur), trace au
//!   journal. Apres la boite d'impression Windows on ne sait pas si l'on
//!   a imprime ou annule : une premiere impression annulee fait sortir la
//!   suivante en duplicata, et c'est ce bouton qui rattrape.
//!
//! Le duplicata est optionnel, par genre (Paramètres → Documents,
//! `ReglageGenre::duplicata`) : le serveur lit la case. Case decochee,
//! l'impression se compte QUAND MEME — cochee plus tard, la feuille deja
//! sortie reste l'original — mais la reponse dit `duplicata: false`.
//!
//! Nees sur `Base` (D22) : la fenetre ne parle plus qu'au serveur. Le
//! serveur sert une requete a la fois sur sa `Base` : compter puis
//! ecrire, dans la transaction, ne se croise pas avec un autre poste.

use crate::base::{Acces, Base};
use crate::coeur::impression::{self as regles, Objet};
use crate::parametres;
use crate::utils::maintenant_iso;

/// Ce qu'on sait deja des impressions d'un objet.
struct Etat {
    nombre: i64,
    dernier_rang: i64,
    /// La date de la toute premiere — l'original, meme si le patron l'a
    /// reimprime depuis.
    original_le: Option<String>,
}

/// L'objet a imprimer, tel que la base le connait.
struct Imprime {
    /// Pourquoi l'impression ne se compte pas (`coeur::hors_compte`), ou
    /// `None` si elle se compte.
    hors_compte: Option<&'static str>,
    /// Le genre dont le reglage decide du duplicata.
    genre: &'static str,
}

/// Introuvable dans ce dossier : refuse.
fn objet_sur(acces: &mut impl Acces, objet: Objet, id: &str, format: Option<&str>) -> Result<Imprime, String> {
    let dossier = acces.dossier().to_string();
    let sql = match objet {
        Objet::Piece => "SELECT statut, type_piece FROM piece_commerciale WHERE id = ?1 AND dossier_id = ?2",
        Objet::Paiement => "SELECT id, id FROM paiement WHERE id = ?1 AND dossier_id = ?2",
        Objet::PaiementFournisseur => "SELECT id, id FROM paiement_fournisseur WHERE id = ?1 AND dossier_id = ?2",
    };
    let (statut, type_piece) = acces
        .lire_une(sql, &parametres![id.to_string(), dossier], |r| Ok((r.get::<String>(0)?, r.get::<String>(1)?)))
        .map_err(|e| e.0)?
        .ok_or_else(|| match objet {
            Objet::Piece => "Pièce introuvable dans ce dossier.".to_string(),
            _ => "Reçu introuvable dans ce dossier.".to_string(),
        })?;
    Ok(match objet {
        Objet::Piece => Imprime {
            hors_compte: regles::hors_compte(Some(&statut), Some(&type_piece)),
            genre: crate::coeur::documents::genre_imprime(Some(&type_piece), format),
        },
        _ => Imprime { hors_compte: None, genre: crate::coeur::documents::genre_imprime(None, format) },
    })
}

fn etat_sur(acces: &mut impl Acces, objet: Objet, id: &str) -> Result<Etat, String> {
    let dossier = acces.dossier().to_string();
    let p = parametres![objet.nom().to_string(), id.to_string(), dossier];
    let (nombre, dernier_rang) = acces
        .lire_une(
            "SELECT COUNT(*), CAST(COALESCE(MAX(rang), 0) AS BIGINT) FROM impression
             WHERE objet_type = ?1 AND objet_id = ?2 AND dossier_id = ?3",
            &p,
            |r| Ok((r.get::<i64>(0)?, r.get::<i64>(1)?)),
        )
        .map_err(|e| e.0)?
        .unwrap_or((0, 0));
    let original_le = acces
        .lire_une(
            "SELECT imprime_le FROM impression
             WHERE objet_type = ?1 AND objet_id = ?2 AND dossier_id = ?3
             ORDER BY rang LIMIT 1",
            &p,
            |r| r.get::<String>(0),
        )
        .map_err(|e| e.0)?;
    Ok(Etat { nombre, dernier_rang, original_le })
}

/// Ce que l'apercu montre avant d'imprimer : la prochaine feuille
/// sera-t-elle un duplicata ? N'ecrit rien. Le verdict qui fait foi est
/// celui de `noter_impression_sur` : entre l'apercu et l'impression, un
/// autre poste a pu imprimer. `format` : celui de l'apercu (un rouleau
/// est un ticket, qui a sa propre case).
pub fn lire_etat_impression_sur(
    base: &mut Base,
    objet_type: String,
    objet_id: String,
    format: Option<String>,
) -> Result<serde_json::Value, String> {
    let objet = Objet::lire(&objet_type)?;
    let o = objet_sur(base, objet, &objet_id, format.as_deref())?;
    let compte = o.hors_compte.is_none();
    let actif = crate::documents::duplicata_actif_sur(base, o.genre)?;
    let etat = etat_sur(base, objet, &objet_id)?;
    Ok(serde_json::json!({
        "genre":       o.genre,
        "actif":       actif,
        "compte":      compte,
        "impressions": etat.nombre,
        "duplicata":   actif && compte && regles::verdict(etat.nombre, false).duplicata,
        "original_le": etat.original_le,
    }))
}

/// Une impression, au moment d'imprimer. Rend `{ genre, actif, compte,
/// duplicata, original_le, rang }` ; un brouillon ne se compte pas.
pub fn noter_impression_sur(
    base: &mut Base,
    objet_type: String,
    objet_id: String,
    format: Option<String>,
    poste_id: Option<String>,
) -> Result<serde_json::Value, String> {
    noter_sur(base, objet_type, objet_id, format, poste_id, false)
}

/// L'original reimprime : la feuille sort sans « DUPLICATA ». Le droit
/// (`pieces:reimprimer_original`) se verifie dans la poignee du serveur,
/// comme `avoirs:accorder`. Un objet jamais imprime sort en original de
/// toute facon, sans trace : rien n'est REimprime. Case du genre
/// decochee : refuse, il n'y a pas de duplicata a eviter.
pub fn reimprimer_original_sur(
    base: &mut Base,
    objet_type: String,
    objet_id: String,
    format: Option<String>,
    poste_id: Option<String>,
) -> Result<serde_json::Value, String> {
    noter_sur(base, objet_type, objet_id, format, poste_id, true)
}

fn noter_sur(
    base: &mut Base,
    objet_type: String,
    objet_id: String,
    format: Option<String>,
    poste_id: Option<String>,
    reimprimer_original: bool,
) -> Result<serde_json::Value, String> {
    let objet = Objet::lire(&objet_type)?;
    let o = objet_sur(base, objet, &objet_id, format.as_deref())?;
    let actif = crate::documents::duplicata_actif_sur(base, o.genre)?;
    if let Some(pourquoi) = o.hors_compte {
        if reimprimer_original {
            return Err(pourquoi.to_string());
        }
        return Ok(serde_json::json!({
            "genre": o.genre, "actif": actif, "compte": false,
            "duplicata": false, "original_le": null, "rang": 0,
        }));
    }
    if reimprimer_original && !actif {
        return Err(
            "Le duplicata n'est pas activé pour ce document (Paramètres → Documents) : chaque impression sort déjà sans mention."
                .to_string(),
        );
    }
    let poste_id = poste_id.filter(|p| !p.trim().is_empty());
    let maintenant = maintenant_iso();
    let dossier = base.dossier().to_string();

    let mut tx = base.transaction().map_err(|e| e.0)?;
    let auteur = crate::argent::id_utilisateur_courant_sur(&mut tx);
    let etat = etat_sur(&mut tx, objet, &objet_id)?;
    let verdict = regles::verdict(etat.nombre, reimprimer_original);
    let rang = etat.dernier_rang + 1;
    tx.executer(
        "INSERT INTO impression
           (id, dossier_id, objet_type, objet_id, rang, original, imprime_le, auteur_id, poste_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        &parametres![
            uuid::Uuid::new_v4().to_string(),
            dossier.clone(),
            objet.nom().to_string(),
            objet_id.clone(),
            rang,
            verdict.original as i64,
            maintenant.clone(),
            auteur.clone(),
            poste_id.clone()
        ],
    )
    .map_err(|e| e.0)?;
    if reimprimer_original && etat.nombre > 0 {
        tx.executer(
            "INSERT INTO journal
               (id, type_evenement, entite_type, entite_id, auteur_id,
                ancien_valeur, nouveau_valeur, origine, date_evenement, dossier_id)
             VALUES (?1,'original_reimprime',?2,?3,?4,?5,?6,'app',?7,?8)",
            &parametres![
                uuid::Uuid::new_v4().to_string(),
                objet.nom().to_string(),
                objet_id.clone(),
                auteur,
                serde_json::json!({ "original_le": etat.original_le, "impressions": etat.nombre }).to_string(),
                serde_json::json!({ "rang": rang, "poste_id": poste_id, "genre": o.genre }).to_string(),
                maintenant.clone(),
                dossier
            ],
        )
        .map_err(|e| e.0)?;
    }
    tx.valider().map_err(|e| e.0)?;

    Ok(serde_json::json!({
        "genre":       o.genre,
        "actif":       actif,
        "compte":      true,
        "duplicata":   actif && verdict.duplicata,
        "original_le": etat.original_le.unwrap_or(maintenant),
        "rang":        rang,
    }))
}
