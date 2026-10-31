//! L'accueil du jour de Gescom Equipe (PLAN-EQUIPE, EQ-3 — D49).
//! Version `Base` seule (D29).
//!
//! Une lecture qui rassemble ce qui attend la personne connectee : ses
//! rappels dus, les fiches en brouillon, les fiches validees pas
//! soldees, les avances en cours — sur toutes les periodes du dossier
//! (un brouillon de septembre oublie est justement ce qu'il faut voir).
//!
//! Chaque bloc n'existe que pour qui a le droit de le lire
//! (`coeur::lecture::accueil_equipe`) : sans droit, il n'est ni calcule
//! ni rendu — pas un zero qui ressemblerait a « rien a faire ». Un
//! montant non lisible vaut `null` (C-1).

use crate::base::Base;
use crate::coeur::lecture::AccueilEquipe;
use crate::coeur::{avances as regles_avances, paie as regles_paie};
use crate::parametres;

/// Ce qui attend `moi` (l'utilisateur de la session, D26) le jour
/// `aujourd_hui` (l'horloge du serveur ; seuls les dix premiers
/// caracteres comptent).
pub fn accueil_sur(
    base: &mut Base,
    droits: AccueilEquipe,
    moi: &str,
    aujourd_hui: &str,
) -> Result<serde_json::Value, String> {
    let jour = aujourd_hui.get(..10).unwrap_or(aujourd_hui).to_string();
    let dossier = base.dossier().to_string();
    let montant = |v: i64| if droits.montants { serde_json::json!(v) } else { serde_json::Value::Null };
    let mut r = serde_json::Map::new();
    r.insert("aujourd_hui".into(), jour.clone().into());

    // Ses rappels dus : aujourd'hui et en retard, pas faits. Le client
    // doit etre du dossier, comme dans « Mes rappels ».
    if droits.rappels {
        let quand: Vec<String> = base
            .lire_plusieurs(
                "SELECT SUBSTR(r.quand, 1, 10) FROM rappel r
                 JOIN client c ON c.id = r.client_id AND c.dossier_id = r.dossier_id
                 WHERE r.dossier_id = ?1 AND r.pour_utilisateur_id = ?2 AND r.fait = 0
                   AND SUBSTR(r.quand, 1, 10) <= ?3",
                &parametres![dossier.clone(), moi, jour.clone()],
                |l| l.get::<String>(0),
            )
            .map_err(|e| e.0)?;
        r.insert(
            "rappels".into(),
            serde_json::json!({
                "nombre": quand.len(),
                "en_retard": quand.iter().filter(|q| q.as_str() < jour.as_str()).count(),
            }),
        );
    }

    if droits.fiches {
        // (net, debut de periode) des brouillons.
        let brouillons: Vec<(i64, String)> = base
            .lire_plusieurs(
                "SELECT net, du FROM fiche_paie WHERE dossier_id = ?1 AND statut = 'brouillon'",
                &parametres![dossier.clone()],
                |l| Ok((l.get::<i64>(0)?, l.get::<String>(1)?)),
            )
            .map_err(|e| e.0)?;
        r.insert(
            "brouillons".into(),
            serde_json::json!({
                "nombre": brouillons.len(),
                "net": montant(brouillons.iter().fold(0i64, |s, b| s.saturating_add(b.0))),
                "plus_ancien": brouillons.iter().map(|b| b.1.clone()).min(),
            }),
        );

        // Les validees (pas les remplacees) qui doivent encore quelque
        // chose : le meme reste que la fiche (`paie::lire_sur`).
        let validees: Vec<(i64, String)> = base
            .lire_plusieurs(
                "SELECT f.net, f.du,
                   (SELECT CAST(COALESCE(SUM(v.montant), 0) AS BIGINT) FROM versement_paie v
                     WHERE v.fiche_id = f.id AND v.dossier_id = f.dossier_id),
                   (SELECT CAST(COALESCE(SUM(a.montant), 0) AS BIGINT) FROM avance a
                     WHERE a.trop_verse_fiche_id = f.id AND a.dossier_id = f.dossier_id AND a.statut = 'ouverte')
                 FROM fiche_paie f WHERE f.dossier_id = ?1 AND f.statut = 'validee'",
                &parametres![dossier.clone()],
                |l| {
                    let reste = regles_paie::reste_a_verser(l.get::<i64>(0)?, l.get::<i64>(2)?, l.get::<i64>(3)?);
                    Ok((reste, l.get::<String>(1)?))
                },
            )
            .map_err(|e| e.0)?
            .into_iter()
            .filter(|(reste, _)| *reste > 0)
            .collect();
        r.insert(
            "a_solder".into(),
            serde_json::json!({
                "nombre": validees.len(),
                "reste": montant(validees.iter().fold(0i64, |s, v| s.saturating_add(v.0))),
                "plus_ancien": validees.iter().map(|v| v.1.clone()).min(),
            }),
        );
    }

    // Les avances qui restent a retenir (comme `avances::lister_sur`
    // « en cours seulement »).
    if droits.avances {
        let ouvertes: Vec<(String, i64)> = base
            .lire_plusieurs(
                "SELECT employe_id, montant, retenu FROM avance
                 WHERE dossier_id = ?1 AND statut = 'ouverte' AND montant > retenu",
                &parametres![dossier],
                |l| Ok((l.get::<String>(0)?, regles_avances::reste(l.get::<i64>(1)?, l.get::<i64>(2)?))),
            )
            .map_err(|e| e.0)?;
        let personnes: std::collections::HashSet<&str> = ouvertes.iter().map(|o| o.0.as_str()).collect();
        r.insert(
            "avances".into(),
            serde_json::json!({
                "nombre": ouvertes.len(),
                "personnes": personnes.len(),
                "reste": montant(ouvertes.iter().fold(0i64, |s, o| s.saturating_add(o.1))),
            }),
        );
    }

    Ok(serde_json::Value::Object(r))
}
