//! L'Historique : le journal metier, enfin lu (v3, B-1).
//!
//! « Qui a annule le reglement de Coulibaly mardi ? » — la reponse est
//! dans `journal` depuis la v1, et nulle part a l'ecran. Ici : une
//! lecture paginee, sur les deux moteurs, qui dit pour chaque ligne
//! QUAND, QUI, QUOI, SUR QUOI, et ce qui a change.
//!
//! « Sur quoi » se RESOUT a la lecture : un evenement porte
//! `(entite_type, entite_id)` — une vente, un paiement, une ligne. On en
//! tire le tiers (client ou fournisseur), la piece et l'article, par
//! sous-requetes. Rien n'est recopie a l'ecriture : le journal reste ce
//! qu'il est, et une fiche renommee se lit sous son nom d'aujourd'hui.
//!
//! Rien ne s'efface : un journal qu'on purge n'est plus un journal.
//! Index sur `date_evenement` et `(entite_type, entite_id)`.

use crate::base::Acces;
use crate::coeur::historique as regles;
use crate::parametres;

/// Ce qu'on cherche. Tout est facultatif.
#[derive(Debug, Default, Clone, serde::Deserialize)]
pub struct Filtre {
    pub du: Option<String>,
    pub au: Option<String>,
    pub auteur_id: Option<String>,
    pub type_evenement: Option<String>,
    /// Le client ou le fournisseur, directement ou par sa vente, son
    /// paiement, sa piece.
    pub tiers_id: Option<String>,
    pub piece_id: Option<String>,
    pub article_id: Option<String>,
    /// Un morceau de nom de client, de numero de piece, d'article, ou du
    /// detail enregistre.
    pub recherche: Option<String>,
    /// Seulement les anomalies que personne n'a encore marquees vues
    /// (le compteur du tableau de bord ouvre l'Historique ainsi).
    #[serde(default)]
    pub a_verifier: bool,
    #[serde(default)]
    pub page: i64,
    #[serde(default)]
    pub par_page: i64,
}

/// La table du journal, avec ce qu'elle designe resolu. `?1` est le
/// dossier, `?2`…`?9` les filtres (`FILTRES`). Chaque sous-requete lit
/// par cle primaire : une seule ligne, sinon PostgreSQL refuse. (Un
/// transfert est journalise par son BON, qui couvre plusieurs articles :
/// il ne se rattache donc a aucun.)
const RESOLU: &str = "
WITH h AS (
  SELECT j.id, j.date_evenement, j.type_evenement, j.entite_type, j.entite_id,
         j.ancien_valeur, j.nouveau_valeur, j.auteur_id,
         u.nom AS auteur_nom,
         av.vue_le, uv.nom AS vue_par_nom,
         CASE
           WHEN j.entite_type IN ('client', 'fournisseur') THEN j.entite_id
           WHEN j.entite_type = 'vente' THEN
             (SELECT v.client_id FROM vente v WHERE v.id = j.entite_id)
           WHEN j.entite_type = 'paiement' THEN
             (SELECT v.client_id FROM paiement p JOIN vente v ON v.id = p.vente_id WHERE p.id = j.entite_id)
           WHEN j.entite_type = 'ligne_vente' THEN
             (SELECT v.client_id FROM ligne_vente lv JOIN vente v ON v.id = lv.vente_id WHERE lv.id = j.entite_id)
           WHEN j.entite_type IN ('piece_commerciale', 'piece') THEN
             (SELECT pc.tiers_id FROM piece_commerciale pc WHERE pc.id = j.entite_id)
           WHEN j.entite_type = 'paiement_fournisseur' THEN
             (SELECT pf.fournisseur_id FROM paiement_fournisseur pf WHERE pf.id = j.entite_id)
           WHEN j.entite_type = 'retour' THEN
             (SELECT v.client_id FROM retour rt JOIN vente v ON v.id = rt.vente_id WHERE rt.id = j.entite_id)
           WHEN j.entite_type = 'avoir' THEN
             (SELECT av.client_id FROM avoir av WHERE av.id = j.entite_id)
           WHEN j.entite_type = 'cheque_recu' THEN
             (SELECT v.client_id FROM cheque_recu cr JOIN vente v ON v.id = cr.vente_id WHERE cr.id = j.entite_id)
         END AS tiers_id,
         CASE
           WHEN j.entite_type IN ('piece_commerciale', 'piece') THEN j.entite_id
           WHEN j.entite_type = 'vente' THEN
             (SELECT v.piece_id FROM vente v WHERE v.id = j.entite_id)
           WHEN j.entite_type = 'paiement' THEN
             (SELECT v.piece_id FROM paiement p JOIN vente v ON v.id = p.vente_id WHERE p.id = j.entite_id)
           WHEN j.entite_type = 'ligne_vente' THEN
             (SELECT v.piece_id FROM ligne_vente lv JOIN vente v ON v.id = lv.vente_id WHERE lv.id = j.entite_id)
           WHEN j.entite_type = 'retour' THEN
             (SELECT v.piece_id FROM retour rt JOIN vente v ON v.id = rt.vente_id WHERE rt.id = j.entite_id)
           WHEN j.entite_type = 'avoir' THEN
             (SELECT av.piece_id FROM avoir av WHERE av.id = j.entite_id)
           WHEN j.entite_type = 'cheque_recu' THEN
             (SELECT v.piece_id FROM cheque_recu cr JOIN vente v ON v.id = cr.vente_id WHERE cr.id = j.entite_id)
         END AS piece_id,
         CASE
           WHEN j.entite_type = 'article' THEN j.entite_id
           WHEN j.entite_type = 'ligne_vente' THEN
             (SELECT lv.article_id FROM ligne_vente lv WHERE lv.id = j.entite_id)
           WHEN j.entite_type = 'retour' THEN
             (SELECT rt.article_id FROM retour rt WHERE rt.id = j.entite_id)
         END AS article_id
  FROM journal j
  LEFT JOIN utilisateur u ON u.id = j.auteur_id
  LEFT JOIN anomalie_vue av ON av.journal_id = j.id AND av.dossier_id = ?1
  LEFT JOIN utilisateur uv ON uv.id = av.vue_par
  WHERE j.dossier_id = ?1
),
r AS (
  SELECT h.*,
         COALESCE((SELECT c.nom FROM client c WHERE c.id = h.tiers_id),
                  (SELECT f.nom FROM fournisseur f WHERE f.id = h.tiers_id)) AS tiers_nom,
         (SELECT pc.numero FROM piece_commerciale pc WHERE pc.id = h.piece_id) AS piece_numero,
         (SELECT a.nom FROM article a WHERE a.id = h.article_id) AS article_nom
  FROM h
)";

const FILTRES: &str = "
WHERE (CAST(?2 AS TEXT) IS NULL OR SUBSTR(r.date_evenement, 1, 10) >= ?2)
  AND (CAST(?3 AS TEXT) IS NULL OR SUBSTR(r.date_evenement, 1, 10) <= ?3)
  AND (CAST(?4 AS TEXT) IS NULL OR r.auteur_id = ?4)
  AND (CAST(?5 AS TEXT) IS NULL OR r.type_evenement = ?5)
  AND (CAST(?6 AS TEXT) IS NULL OR r.tiers_id = ?6)
  AND (CAST(?7 AS TEXT) IS NULL OR r.piece_id = ?7)
  AND (CAST(?8 AS TEXT) IS NULL OR r.article_id = ?8)
  AND (CAST(?9 AS TEXT) IS NULL OR LOWER(
         COALESCE(r.tiers_nom, '') || ' ' || COALESCE(r.piece_numero, '') || ' ' ||
         COALESCE(r.article_nom, '') || ' ' || COALESCE(r.auteur_nom, '') || ' ' ||
         COALESCE(r.nouveau_valeur, '') || ' ' || COALESCE(r.ancien_valeur, '')
       ) LIKE LOWER(?9))
  AND (CAST(?10 AS BIGINT) = 0 OR (r.type_evenement = 'anomalie' AND r.vue_le IS NULL))";

fn vide(s: &Option<String>) -> Option<String> {
    s.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string)
}

/// La page demandee de l'historique, la plus recente d'abord.
pub fn lire_historique_sur(acces: &mut impl Acces, f: Filtre) -> Result<serde_json::Value, String> {
    let dossier = acces.dossier().to_string();
    let du = regles::jour_filtre(f.du.as_deref())?;
    let au = regles::jour_filtre(f.au.as_deref())?;
    let recherche = vide(&f.recherche).map(|r| format!("%{r}%"));
    let par_page = regles::borner_page(if f.par_page == 0 { 50 } else { f.par_page });
    let page = f.page.max(0);

    let filtres = parametres![
        dossier,
        du,
        au,
        vide(&f.auteur_id),
        vide(&f.type_evenement),
        vide(&f.tiers_id),
        vide(&f.piece_id),
        vide(&f.article_id),
        recherche,
        f.a_verifier as i64
    ];

    let total: i64 = acces
        .lire_une(
            &format!("{RESOLU} SELECT CAST(COUNT(*) AS BIGINT) FROM r {FILTRES}"),
            &filtres,
            |r| r.get::<i64>(0),
        )
        .map_err(|e| e.0)?
        .unwrap_or(0);

    let mut avec_page = filtres.clone();
    avec_page.push(crate::base::Valeur::from(par_page));
    avec_page.push(crate::base::Valeur::from(page * par_page));
    let lignes = acces
        .lire_plusieurs(
            &format!(
                "{RESOLU}
                 SELECT r.id, r.date_evenement, r.type_evenement, r.entite_type, r.entite_id,
                        r.auteur_id, r.auteur_nom, r.tiers_id, r.tiers_nom,
                        r.piece_id, r.piece_numero, r.article_id, r.article_nom,
                        r.ancien_valeur, r.nouveau_valeur, r.vue_le, r.vue_par_nom
                 FROM r {FILTRES}
                 ORDER BY r.date_evenement DESC, r.id DESC
                 LIMIT ?11 OFFSET ?12"
            ),
            &avec_page,
            |r| {
                let t: String = r.get::<String>(2)?;
                Ok(serde_json::json!({
                    "id":             r.get::<String>(0)?,
                    "date":           r.get::<String>(1)?,
                    "type":           t,
                    "libelle_type":   regles::libelle_type(&t),
                    "entite_type":    r.get::<String>(3)?,
                    "entite_id":      r.get::<String>(4)?,
                    "auteur_id":      r.get::<Option<String>>(5)?,
                    "auteur_nom":     r.get::<Option<String>>(6)?,
                    "tiers_id":       r.get::<Option<String>>(7)?,
                    "tiers_nom":      r.get::<Option<String>>(8)?,
                    "piece_id":       r.get::<Option<String>>(9)?,
                    "piece_numero":   r.get::<Option<String>>(10)?,
                    "article_id":     r.get::<Option<String>>(11)?,
                    "article_nom":    r.get::<Option<String>>(12)?,
                    "ancien":         detail(r.get::<Option<String>>(13)?),
                    "nouveau":        detail(r.get::<Option<String>>(14)?),
                    // Une anomalie vue dit par qui et quand ; les autres
                    // lignes n'ont rien a voir.
                    "vue": match (t == "anomalie", r.get::<Option<String>>(15)?) {
                        (true, Some(le)) => serde_json::json!({
                            "le": le,
                            "par_nom": r.get::<Option<String>>(16)?,
                        }),
                        _ => serde_json::Value::Null,
                    },
                }))
            },
        )
        .map_err(|e| e.0)?;

    Ok(serde_json::json!({
        "total": total,
        "page": page,
        "par_page": par_page,
        "lignes": lignes,
    }))
}

/// Le detail enregistre : du JSON le plus souvent, parfois un texte nu
/// (les plus anciens). On rend l'objet quand c'en est un.
fn detail(v: Option<String>) -> serde_json::Value {
    match v {
        None => serde_json::Value::Null,
        Some(t) => serde_json::from_str(&t).unwrap_or(serde_json::Value::String(t)),
    }
}

/// Combien d'anomalies personne n'a encore vues — le compteur rouge du
/// tableau de bord (B-4).
pub fn anomalies_a_verifier_sur(acces: &mut impl Acces) -> Result<i64, String> {
    let dossier = acces.dossier().to_string();
    acces
        .lire_une(
            "SELECT CAST(COUNT(*) AS BIGINT) FROM journal j
             WHERE j.dossier_id = ?1 AND j.type_evenement = 'anomalie'
               AND NOT EXISTS (SELECT 1 FROM anomalie_vue av
                               WHERE av.journal_id = j.id AND av.dossier_id = ?1)",
            &parametres![dossier],
            |r| r.get::<i64>(0),
        )
        .map_err(|e| e.0)
        .map(|n| n.unwrap_or(0))
}

/// Marque une anomalie vue, au nom de la personne de la session (D26).
/// Deja vue : on garde la PREMIERE — c'est elle qui a pris
/// l'anomalie en charge — et on la rend.
pub fn marquer_anomalie_vue_sur(acces: &mut impl Acces, journal_id: &str) -> Result<serde_json::Value, String> {
    let dossier = acces.dossier().to_string();
    let genre = acces
        .lire_une(
            "SELECT type_evenement FROM journal WHERE id = ?1 AND dossier_id = ?2",
            &parametres![journal_id, dossier.clone()],
            |r| r.get::<String>(0),
        )
        .map_err(|e| e.0)?;
    match genre.as_deref() {
        None => return Err("Événement introuvable.".to_string()),
        Some("anomalie") => {}
        Some(_) => return Err("Seule une anomalie se marque vue.".to_string()),
    }
    let par = crate::argent::id_utilisateur_courant_sur(acces);
    acces
        .executer(
            "INSERT INTO anomalie_vue (journal_id, vue_par, vue_le, dossier_id)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT (journal_id) DO NOTHING",
            &parametres![journal_id, par, crate::utils::maintenant_iso(), dossier.clone()],
        )
        .map_err(|e| e.0)?;
    acces
        .lire_une(
            "SELECT av.vue_le, u.nom FROM anomalie_vue av
             LEFT JOIN utilisateur u ON u.id = av.vue_par
             WHERE av.journal_id = ?1 AND av.dossier_id = ?2",
            &parametres![journal_id, dossier],
            |r| Ok(serde_json::json!({ "le": r.get::<String>(0)?, "par_nom": r.get::<Option<String>>(1)? })),
        )
        .map_err(|e| e.0)?
        .ok_or_else(|| "La marque ne s'est pas relue.".to_string())
}

/// Les types d'evenement, pour le filtre de l'ecran.
pub fn types_evenement() -> serde_json::Value {
    serde_json::Value::Array(
        regles::TYPES
            .iter()
            .map(|(t, l)| serde_json::json!({ "type": t, "libelle": l }))
            .collect(),
    )
}

/// Ce que proposent les listes de l'ecran : les types, et les personnes
/// qui ont reellement agi dans ce dossier (pas la liste des comptes :
/// elle demande `utilisateurs:gerer`, et un compte qui n'a rien fait
/// n'a rien a filtrer).
pub fn filtres_sur(acces: &mut impl Acces) -> Result<serde_json::Value, String> {
    let dossier = acces.dossier().to_string();
    let auteurs = acces
        .lire_plusieurs(
            "SELECT u.id, u.nom FROM utilisateur u
             WHERE u.id IN (SELECT j.auteur_id FROM journal j WHERE j.dossier_id = ?1)
             ORDER BY u.nom",
            &parametres![dossier],
            |r| Ok(serde_json::json!({ "id": r.get::<String>(0)?, "nom": r.get::<String>(1)? })),
        )
        .map_err(|e| e.0)?;
    Ok(serde_json::json!({ "types": types_evenement(), "auteurs": auteurs }))
}
