//! Les avances sur salaire en base (PLAN-EQUIPE, G-1 — D32). Version
//! `Base` seule (D29).
//!
//! Une avance est rattachee a la personne. Elle est **independante de
//! la caisse** (decision du proprietaire, 24/09) : l'argent de la paie
//! ne passe pas par le tiroir du jour, le moyen dit d'ou il vient. Ce
//! qui n'est pas retenu est « en cours » ; la fiche de paie le retient
//! (G-2, colonne `retenu`). Annuler une avance (une erreur de saisie) se
//! fait seulement si rien n'en a ete retenu.
//!
//! D42 : quand l'argent est **pris dans le tiroir**, la sortie de caisse
//! s'ecrit avec l'avance (caisse ouverte exigee) ; l'avance peut etre
//! datee d'un jour passe et s'etaler (« X F par paie »).

use crate::base::Base;
use crate::coeur::avances as regles;
use crate::parametres;

/// Le motif des mouvements de caisse d'une avance prise dans le tiroir.
pub const MOTIF_CAISSE: &str = "avance_salaire";
/// Le motif de l'argent qui revient au tiroir quand on l'annule.
pub const MOTIF_CAISSE_ANNULEE: &str = "avance_salaire_annulee";

struct Personne {
    nom: String,
    statut: String,
    plafond: Option<i64>,
    entree: Option<String>,
}

fn personne(base: &mut Base, employe_id: &str) -> Result<Personne, String> {
    let dossier = base.dossier().to_string();
    base.lire_une(
        "SELECT nom, statut, avance_max, date_entree FROM employe WHERE id = ?1 AND dossier_id = ?2",
        &parametres![employe_id, dossier],
        |r| Ok(Personne { nom: r.get(0)?, statut: r.get(1)?, plafond: r.get(2)?, entree: r.get(3)? }),
    )
    .map_err(|e| e.0)?
    .ok_or_else(|| "Fiche introuvable.".to_string())
}

/// Ce que la personne doit encore en avances non retenues (les prets a
/// part, D47 : ils n'entrent pas dans le plafond).
pub fn en_cours_sur(base: &mut Base, employe_id: &str) -> Result<i64, String> {
    en_cours_de_nature_sur(base, employe_id, "avance")
}

/// Ce que la personne doit encore, en avances ou en prets (`nature`).
pub fn en_cours_de_nature_sur(base: &mut Base, employe_id: &str, nature: &str) -> Result<i64, String> {
    let dossier = base.dossier().to_string();
    base.lire_une(
        "SELECT CAST(COALESCE(SUM(montant - retenu), 0) AS BIGINT) FROM avance
         WHERE employe_id = ?1 AND dossier_id = ?2 AND statut = 'ouverte' AND nature = ?3",
        &parametres![employe_id, dossier, nature],
        |r| r.get::<i64>(0),
    )
    .map_err(|e| e.0)
    .map(|v| v.unwrap_or(0))
}

fn noter(
    acces: &mut impl crate::base::Acces,
    type_evenement: &str,
    avance_id: &str,
    auteur: &str,
    valeur: serde_json::Value,
) -> Result<(), String> {
    noter_entite(acces, type_evenement, "avance", avance_id, auteur, valeur)
}

/// Note un geste au journal ; un pret s'y range sous `pret` (D47), que
/// la lecture masque comme la paie.
pub(crate) fn noter_entite(
    acces: &mut impl crate::base::Acces,
    type_evenement: &str,
    entite: &str,
    avance_id: &str,
    auteur: &str,
    valeur: serde_json::Value,
) -> Result<(), String> {
    let dossier = acces.dossier().to_string();
    acces
        .executer(
            "INSERT INTO journal
               (id, type_evenement, entite_type, entite_id, auteur_id, nouveau_valeur, origine, date_evenement, dossier_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'app', ?7, ?8)",
            &parametres![
                uuid::Uuid::new_v4().to_string(),
                type_evenement,
                entite,
                avance_id,
                auteur,
                valeur.to_string(),
                crate::utils::maintenant_iso(),
                dossier
            ],
        )
        .map(|_| ())
        .map_err(|e| e.0)
}

/// Ce qu'on dit d'une avance en la donnant, outre la personne et le
/// montant. Vide : especes, aujourd'hui, hors caisse, tout retenu sur la
/// prochaine paie — la regle d'avant D42.
#[derive(Debug, Clone, Default)]
pub struct Saisie {
    pub moyen: Option<String>,
    pub motif: Option<String>,
    /// Le jour ou l'avance a ete donnee (`AAAA-MM-JJ`) ; vide : aujourd'hui.
    pub date: Option<String>,
    /// Ce qu'une paie en retient au plus ; vide : tout.
    pub mensualite: Option<i64>,
    /// L'argent sort du tiroir : la sortie de caisse s'ecrit avec elle.
    pub du_tiroir: bool,
}

/// Donne une avance, rattachee a la personne, hors caisse — comme qui
/// voit la paie (les tests).
pub fn donner_sur(
    base: &mut Base,
    employe_id: String,
    montant: i64,
    moyen: Option<String>,
    motif: Option<String>,
) -> Result<serde_json::Value, String> {
    donner_selon_droits_sur(base, employe_id, montant, moyen, motif, true)
}

/// Donne une avance hors caisse, aujourd'hui, retenue en entier.
/// `voit_la_paie` : l'appelant lit-il le plafond
/// (`coeur::lecture::voit_la_paie`) ? Sans, le refus ne le chiffre pas
/// (revue du 28/10, point 3).
pub fn donner_selon_droits_sur(
    base: &mut Base,
    employe_id: String,
    montant: i64,
    moyen: Option<String>,
    motif: Option<String>,
    voit_la_paie: bool,
) -> Result<serde_json::Value, String> {
    donner_avance_sur(base, employe_id, montant, Saisie { moyen, motif, ..Saisie::default() }, voit_la_paie)
}

/// Donne une avance (D32, D42). Prise dans le tiroir, la sortie de
/// caisse s'ecrit dans la meme transaction, sur la caisse ouverte du
/// dossier — fermee, rien n'est ecrit (`CAISSE_FERMEE`).
pub fn donner_avance_sur(
    base: &mut Base,
    employe_id: String,
    montant: i64,
    s: Saisie,
    voit_la_paie: bool,
) -> Result<serde_json::Value, String> {
    let p = personne(base, &employe_id)?;
    if p.statut != "actif" {
        return Err(format!("{} est partie : pas d'avance.", p.nom));
    }
    let moyen = s.moyen.map(|m| m.trim().to_string()).filter(|m| !m.is_empty()).unwrap_or_else(|| "especes".into());
    crate::coeur::saisie::verifier_mode_encaissement(&moyen)?;
    regles::verifier_tiroir(&moyen, s.du_tiroir)?;
    let en_cours = en_cours_sur(base, &employe_id)?;
    regles::verifier(montant, en_cours, p.plafond, &p.nom, voit_la_paie)?;
    let mensualite = regles::verifier_mensualite(s.mensualite, montant)?;
    let jour = regles::verifier_date(s.date.as_deref(), chrono::Local::now().date_naive(), p.entree.as_deref(), s.du_tiroir)?;
    crate::dossiers::verifier_date_sur(base, &jour)?;
    let auteur = crate::argent::id_utilisateur_courant_sur(base);
    let dossier = base.dossier().to_string();
    let now = crate::utils::maintenant_iso();
    // Aujourd'hui : l'instant, comme avant ; un jour passe : ce jour.
    let date_avance = if now.starts_with(&jour) { now.clone() } else { jour.clone() };
    let motif = s.motif.map(|m| m.trim().to_string()).filter(|m| !m.is_empty());
    let id = uuid::Uuid::new_v4().to_string();
    let mut tx = base.transaction().map_err(|e| e.0)?;
    // La caisse d'abord : fermee, rien ne s'ecrit (regle 4).
    let mouvement = if s.du_tiroir {
        Some(crate::caisses::mouvement_especes_sur(
            &mut tx,
            "sortie",
            montant,
            MOTIF_CAISSE,
            &format!("Avance sur salaire — {}", p.nom),
            &id,
            &auteur,
            &now,
        )?)
    } else {
        None
    };
    tx.executer(
        "INSERT INTO avance
           (id, dossier_id, employe_id, montant, retenu, moyen, motif, date_avance, statut,
            mouvement_caisse_id, cree_par, cree_le, mensualite)
         VALUES (?1, ?2, ?3, ?4, 0, ?5, ?6, ?7, 'ouverte', ?8, ?9, ?10, ?11)",
        &parametres![
            id.clone(),
            dossier,
            employe_id.clone(),
            montant,
            moyen.clone(),
            motif.clone(),
            date_avance.clone(),
            mouvement.clone(),
            auteur.clone(),
            now.clone(),
            mensualite
        ],
    )
    .map_err(|e| e.0)?;
    noter(
        &mut tx,
        "avance_donnee",
        &id,
        &auteur,
        serde_json::json!({
            "nom": p.nom, "montant": montant, "moyen": moyen, "date_avance": jour,
            "mensualite": mensualite, "du_tiroir": s.du_tiroir,
        }),
    )?;
    tx.valider().map_err(|e| e.0)?;
    Ok(serde_json::json!({
        "id": id, "employe_id": employe_id, "nom": p.nom, "montant": montant,
        "en_cours": en_cours.saturating_add(montant), "date_avance": date_avance,
        "mensualite": mensualite, "du_tiroir": s.du_tiroir,
        "avertissement": deja_payee(base, &employe_id, &jour)?,
    }))
}

/// Une avance datee d'une periode dont la paie est deja validee : elle
/// sera retenue sur la suivante (D42) — l'ecran le dit.
fn deja_payee(base: &mut Base, employe_id: &str, jour: &str) -> Result<Option<String>, String> {
    let dossier = base.dossier().to_string();
    let numero: Option<String> = base
        .lire_une(
            "SELECT numero FROM fiche_paie
             WHERE dossier_id = ?1 AND employe_id = ?2 AND statut = 'validee'
               AND du <= ?3 AND au >= ?3
             ORDER BY valide_le DESC",
            &parametres![dossier, employe_id, jour],
            |r| r.get::<Option<String>>(0),
        )
        .map_err(|e| e.0)?
        .flatten();
    Ok(numero.map(|n| format!("La paie de cette période ({n}) est déjà validée : l'avance sera retenue sur la suivante.")))
}

/// Annule une avance donnee par erreur : elle ne se retiendra pas.
/// Refuse si une fiche de paie en a deja retenu une partie. Prise dans le
/// tiroir, l'argent y revient : une entree dans la caisse ouverte
/// (D42 ; fermee, rien n'est annule). Un pret s'annule par
/// `prets::annuler_sur` (D47 : un autre droit).
pub fn annuler_sur(base: &mut Base, avance_id: String, motif: Option<String>) -> Result<serde_json::Value, String> {
    annuler_de_nature_sur(base, avance_id, motif, "avance")
}

/// Annule une avance ou un pret (`nature`), aux memes conditions.
pub(crate) fn annuler_de_nature_sur(base: &mut Base, avance_id: String, motif: Option<String>, nature: &str) -> Result<serde_json::Value, String> {
    let dossier = base.dossier().to_string();
    let (employe_id, montant, retenu, statut, du_tiroir, sa_nature): (String, i64, i64, String, bool, String) = base
        .lire_une(
            "SELECT employe_id, montant, retenu, statut, mouvement_caisse_id, nature FROM avance WHERE id = ?1 AND dossier_id = ?2",
            &parametres![avance_id.clone(), dossier.clone()],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get::<Option<String>>(4)?.is_some(), r.get(5)?)),
        )
        .map_err(|e| e.0)?
        .ok_or_else(|| "Avance introuvable.".to_string())?;
    let pret = nature == crate::coeur::pret::NATURE;
    if sa_nature != nature {
        return Err(if pret { "Ce n'est pas un prêt.".to_string() } else { "C'est un prêt : il s'annule dans l'onglet Prêts.".to_string() });
    }
    if statut != "ouverte" {
        return Err("Cette avance n'est plus en cours.".to_string());
    }
    if retenu > 0 {
        return Err(if pret {
            "Une partie a déjà été retenue ou remboursée : le prêt ne s'annule plus.".to_string()
        } else {
            "Une fiche de paie en a déjà retenu une partie : elle ne s'annule plus.".to_string()
        });
    }
    let p = personne(base, &employe_id)?;
    let auteur = crate::argent::id_utilisateur_courant_sur(base);
    let now = crate::utils::maintenant_iso();
    let motif = motif.map(|m| m.trim().to_string()).filter(|m| !m.is_empty());
    let mut tx = base.transaction().map_err(|e| e.0)?;
    if du_tiroir {
        crate::caisses::mouvement_especes_sur(
            &mut tx,
            "entree",
            montant,
            MOTIF_CAISSE_ANNULEE,
            &format!("{}, l'argent revient — {}", if pret { "Prêt annulé" } else { "Avance annulée" }, p.nom),
            &avance_id,
            &auteur,
            &now,
        )?;
    }
    // `statut = 'ouverte'` : deux annulations en meme temps ne rendent
    // pas deux fois l'argent au tiroir.
    let n = tx
        .executer(
            "UPDATE avance SET statut = 'annulee', annule_le = ?1 WHERE id = ?2 AND dossier_id = ?3 AND statut = 'ouverte' AND retenu = 0",
            &parametres![now, avance_id.clone(), dossier],
        )
        .map_err(|e| e.0)?;
    if n == 0 {
        return Err("Cette avance vient de changer : relire avant d'annuler.".to_string());
    }
    noter_entite(
        &mut tx,
        if pret { "pret_annule" } else { "avance_annulee" },
        nature,
        &avance_id,
        &auteur,
        serde_json::json!({ "nom": p.nom, "montant": montant, "motif": motif, "du_tiroir": du_tiroir }),
    )?;
    tx.valider().map_err(|e| e.0)?;
    Ok(serde_json::json!({ "id": avance_id, "nom": p.nom, "montant": montant, "du_tiroir": du_tiroir }))
}

/// Les avances du dossier courant (d'une personne, ou de toutes), les
/// plus recentes d'abord. `en_cours_seulement` : ce qui reste a retenir.
pub fn lister_sur(base: &mut Base, employe_id: Option<String>, en_cours_seulement: bool) -> Result<Vec<serde_json::Value>, String> {
    let dossier = base.dossier().to_string();
    base.lire_plusieurs(
        "SELECT a.id, a.employe_id, e.nom, a.montant, a.retenu, a.moyen, a.motif, a.date_avance, a.statut, f.numero,
                a.mensualite, a.mouvement_caisse_id
         FROM avance a JOIN employe e ON e.id = a.employe_id AND e.dossier_id = a.dossier_id
         LEFT JOIN fiche_paie f ON f.id = a.trop_verse_fiche_id AND f.dossier_id = a.dossier_id
         WHERE a.dossier_id = ?1 AND a.nature = 'avance'
           AND (CAST(?2 AS TEXT) IS NULL OR a.employe_id = ?2)
           AND (CAST(?3 AS BIGINT) = 0 OR (a.statut = 'ouverte' AND a.montant > a.retenu))
         ORDER BY a.date_avance DESC",
        &parametres![dossier, employe_id, en_cours_seulement as i64],
        |r| {
            let montant: i64 = r.get(3)?;
            let retenu: i64 = r.get(4)?;
            Ok(serde_json::json!({
                "id": r.get::<String>(0)?,
                "employe_id": r.get::<String>(1)?,
                "nom": r.get::<String>(2)?,
                "montant": montant,
                "retenu": retenu,
                "reste": regles::reste(montant, retenu),
                "moyen": r.get::<String>(5)?,
                "motif": r.get::<Option<String>>(6)?,
                "date_avance": r.get::<String>(7)?,
                "statut": r.get::<String>(8)?,
                // Revue du 28/10, point 6 : une avance nee d'un trop-verse
                // dit sur quelle fiche (la rectificative qui l'a cree).
                "trop_verse_numero": r.get::<Option<String>>(9)?,
                // D42 : l'accord de retenue, et l'argent pris au tiroir.
                "mensualite": r.get::<Option<i64>>(10)?,
                "du_tiroir": r.get::<Option<String>>(11)?.is_some(),
            }))
        },
    )
    .map_err(|e| e.0)
}
