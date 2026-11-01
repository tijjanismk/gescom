//! Rapports — CA mensuel, top clients, top articles, export données.
//!
//! Le chiffre d'affaires est NET des retours (`coeur::calcul::ca_net`) :
//! les ventes de la periode moins les retours DATES dans la periode,
//! l'echange mis a part (`RETOUR_HORS_CA`). Un retour d'octobre sur une
//! vente de septembre se deduit d'octobre ; septembre ne bouge plus.
//! L'« encaisse » ne compte pas la dette eteinte par un retour
//! (`coeur::calcul::encaisse`) : aucun argent n'est entre.

/// Une vente du rapport mensuel : (mois `AAAA-MM`, total, tout le paye,
/// la part de ce paye eteinte par un retour).
type VenteDuMois = (String, i64, i64, i64);

/// Assemble le rapport mensuel a partir des ventes (par mois de VENTE)
/// et des retours (par mois de RETOUR) — le meme pour les deux moteurs.
///
/// Par mois : `ca` net des retours du mois, `nb_ventes`, `encaisse`
/// (l'argent reellement recu sur les ventes du mois), `retours` (ce qui
/// a ete deduit) et `creances` (ce qui reste du sur les ventes du mois,
/// vente par vente via `reste_exigible`, D41). `ca - encaisse` ne dit
/// plus la creance des qu'un retour a eu lieu : l'ecran lit `creances`.
/// Un mois de retours sans vente a sa ligne, son chiffre negatif.
fn mois_du_rapport(ventes: Vec<VenteDuMois>, retours: Vec<(String, i64)>) -> Vec<serde_json::Value> {
    use crate::coeur::calcul::{ca_net, encaisse, reste_exigible};
    // (brut, nb, encaisse, creances, retours) par mois, dans l'ordre.
    let mut par_mois: std::collections::BTreeMap<String, (i64, i64, i64, i64, i64)> =
        std::collections::BTreeMap::new();
    for (mois, total, paye, eteint) in ventes {
        let m = par_mois.entry(mois).or_default();
        m.0 += total;
        m.1 += 1;
        m.2 += encaisse(paye, eteint);
        m.3 += reste_exigible(total, paye);
    }
    for (mois, montant) in retours {
        par_mois.entry(mois).or_default().4 += montant;
    }
    par_mois
        .into_iter()
        .rev()
        .map(|(mois, (brut, nb, enc, creances, ret))| {
            serde_json::json!({
                "mois":      mois,
                "ca":        ca_net(brut, ret),
                "nb_ventes": nb,
                "encaisse":  enc,
                "retours":   ret,
                "creances":  creances,
            })
        })
        .collect()
}

// =====================================================================
//  CA mensuel sur N mois
// =====================================================================

pub fn lire_rapport_ca_mensuel(
    conn: &rusqlite::Connection,
    nb_mois: Option<i64>,
) -> Result<Vec<serde_json::Value>, String> {
    let mois = nb_mois.unwrap_or(12).min(24).max(1);
    let depuis_jour = chrono::Local::now()
        .date_naive()
        .checked_sub_months(chrono::Months::new(mois as u32))
        .unwrap_or_else(|| chrono::Local::now().date_naive())
        .format("%Y-%m-%d")
        .to_string();

    // Vente par vente : le reste du se juge par vente (D41), pas sur
    // une somme.
    let ventes: Vec<VenteDuMois> = {
        let mut stmt = conn.prepare(
            "SELECT SUBSTR(v.date_vente, 1, 7),
                    CAST(COALESCE((SELECT SUM(prix_pratique * quantite)
                                   FROM ligne_vente WHERE vente_id = v.id), 0) AS INTEGER),
                    CAST(COALESCE((SELECT SUM(montant) FROM paiement WHERE vente_id = v.id), 0) AS INTEGER),
                    CAST(COALESCE((SELECT SUM(montant) FROM paiement
                                   WHERE vente_id = v.id AND mode = 'retour'), 0) AS INTEGER)
             FROM vente v
             WHERE v.statut != 'annulee'
               AND SUBSTR(v.date_vente, 1, 10) >= ?1"
        ).map_err(|e| e.to_string())?;
        let v = stmt.query_map(rusqlite::params![depuis_jour], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?))
        }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
        v
    };
    let retours: Vec<(String, i64)> = {
        let mut stmt = conn.prepare(
            "SELECT SUBSTR(rt.date_retour, 1, 7),
                    CAST(COALESCE(SUM(rt.montant_credit), 0) AS INTEGER)
             FROM retour rt
             JOIN vente v ON v.id = rt.vente_id
             WHERE v.statut != 'annulee'
               AND rt.mode_resolution <> ?2
               AND SUBSTR(rt.date_retour, 1, 10) >= ?1
             GROUP BY SUBSTR(rt.date_retour, 1, 7)"
        ).map_err(|e| e.to_string())?;
        let v = stmt.query_map(rusqlite::params![depuis_jour, crate::coeur::calcul::RETOUR_HORS_CA], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
        }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
        v
    };
    let x = mois_du_rapport(ventes, retours);
    // Gescom Equipe : le cout des salaires du mois, a cote du CA.
    let depuis = depuis_jour[..7].to_string();
    let salaires: std::collections::HashMap<String, i64> = conn
        .prepare(
            "SELECT SUBSTR(f.au, 1, 7),
                    CAST(SUM(f.brut + COALESCE((SELECT SUM(l.montant) FROM ligne_paie l
                        WHERE l.fiche_id = f.id AND l.genre = ?2), 0)) AS INTEGER)
             FROM fiche_paie f WHERE f.statut = 'validee' AND SUBSTR(f.au, 1, 7) >= ?1
             GROUP BY SUBSTR(f.au, 1, 7)",
        )
        .and_then(|mut s| {
            s.query_map(rusqlite::params![depuis, crate::coeur::paie::CHARGE], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
                .map(|it| it.filter_map(|r| r.ok()).collect())
        })
        .unwrap_or_default();
    Ok(crate::coeur::paie::avec_salaires(x, &salaires))
}

// =====================================================================
//  Top clients sur une période
// =====================================================================

pub fn lire_rapport_top_clients(
    conn: &rusqlite::Connection,
    date_debut: String,
    date_fin: String,
    limite: Option<i64>,
) -> Result<Vec<serde_json::Value>, String> {
    let lim = limite.unwrap_or(20);

    // Les retours de la periode, par client de la vente rendue ; le
    // classement se fait sur le chiffre net, la limite apres.
    let mut stmt = conn.prepare(
        "SELECT c.id, c.code, c.nom, c.telephone,
                CAST(COALESCE(SUM(
                  (SELECT COALESCE(SUM(prix_pratique * quantite), 0)
                   FROM ligne_vente WHERE vente_id = v.id)
                ), 0) AS INTEGER) as ca,
                COUNT(v.id) as nb_ventes,
                CAST(COALESCE(SUM(
                  (SELECT COALESCE(SUM(prix_pratique * quantite), 0)
                   FROM ligne_vente WHERE vente_id = v.id) -
                  (SELECT COALESCE(SUM(montant), 0)
                   FROM paiement WHERE vente_id = v.id)
                ), 0) AS INTEGER) as creances,
                CAST(COALESCE((SELECT SUM(rt.montant_credit) FROM retour rt
                               JOIN vente vr ON vr.id = rt.vente_id
                               WHERE vr.client_id = c.id
                                 AND rt.date_retour BETWEEN ?1 AND ?2
                                 AND vr.statut != 'annulee'
                                 AND rt.mode_resolution <> ?3), 0) AS INTEGER)
         FROM vente v
         JOIN client c ON c.id = v.client_id
         WHERE v.date_vente BETWEEN ?1 AND ?2
           AND v.statut != 'annulee'
           AND c.est_generique = 0
         GROUP BY c.id"
    ).map_err(|e| e.to_string())?;

    let x = stmt.query_map(
        rusqlite::params![date_debut, date_fin, crate::coeur::calcul::RETOUR_HORS_CA],
        |row| Ok((serde_json::json!({
            "id":        row.get::<_,String>(0)?,
            "code":      row.get::<_,String>(1)?,
            "nom":       row.get::<_,String>(2)?,
            "telephone": row.get::<_,Option<String>>(3)?,
            "nb_ventes": row.get::<_,i64>(5)?,
            "creances":  row.get::<_,i64>(6)?,
        }), row.get::<_,i64>(4)?, row.get::<_,i64>(7)?))
    ).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
    Ok(crate::tableau_bord::classer_par_ca_net(x, lim.max(0) as usize))
}

// =====================================================================
//  Top articles sur une période
// =====================================================================

pub fn lire_rapport_top_articles(
    conn: &rusqlite::Connection,
    date_debut: String,
    date_fin: String,
    limite: Option<i64>,
) -> Result<Vec<serde_json::Value>, String> {
    let lim = limite.unwrap_or(20);

    // Les retours de la periode, par article rendu : montant, quantite,
    // et le cout d'achat de ce qui revient — la marge est nette aussi.
    let mut stmt = conn.prepare(
        "SELECT a.id, a.nom, a.unite_base,
                CAST(COALESCE(SUM(lv.prix_pratique * lv.quantite), 0) AS INTEGER) as ca,
                COALESCE(SUM(lv.quantite), 0) as qte_vendue,
                COUNT(DISTINCT lv.vente_id) as nb_ventes,
                CAST(COALESCE(SUM(
                  (COALESCE(a.dernier_prix_achat, 0) * lv.quantite)
                ), 0) AS INTEGER) as cout_achat,
                CAST(COALESCE((SELECT SUM(rt.montant_credit) FROM retour rt
                               JOIN vente vr ON vr.id = rt.vente_id
                               WHERE rt.article_id = a.id
                                 AND rt.date_retour BETWEEN ?1 AND ?2
                                 AND vr.statut != 'annulee'
                                 AND rt.mode_resolution <> ?3), 0) AS INTEGER),
                CAST(COALESCE((SELECT SUM(rt.quantite) FROM retour rt
                               JOIN vente vr ON vr.id = rt.vente_id
                               WHERE rt.article_id = a.id
                                 AND rt.date_retour BETWEEN ?1 AND ?2
                                 AND vr.statut != 'annulee'
                                 AND rt.mode_resolution <> ?3), 0) AS REAL),
                COALESCE(a.dernier_prix_achat, 0)
         FROM ligne_vente lv
         JOIN article a ON a.id = lv.article_id
         JOIN vente v ON v.id = lv.vente_id
         WHERE v.date_vente BETWEEN ?1 AND ?2
           AND v.statut != 'annulee'
         GROUP BY a.id"
    ).map_err(|e| e.to_string())?;

    let x = stmt.query_map(
        rusqlite::params![date_debut, date_fin, crate::coeur::calcul::RETOUR_HORS_CA],
        |row| {
            Ok(article_du_rapport(
                row.get::<_,String>(0)?, row.get::<_,String>(1)?, row.get::<_,String>(2)?,
                row.get::<_,i64>(3)?, row.get::<_,f64>(4)?, row.get::<_,i64>(5)?,
                row.get::<_,i64>(6)?, row.get::<_,i64>(7)?, row.get::<_,f64>(8)?,
                row.get::<_,i64>(9)?,
            ))
        }
    ).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
    Ok(crate::tableau_bord::classer_par_ca_net(x, lim.max(0) as usize))
}

/// Une ligne du top articles, nette des retours de la periode : la
/// quantite rendue se retire de la vendue, et la marge retire aussi le
/// cout d'achat de ce qui est revenu en stock. Rend (ligne, brut,
/// retours) pour `classer_par_ca_net`, qui pose le `ca` net.
#[allow(clippy::too_many_arguments)]
fn article_du_rapport(
    id: String,
    nom: String,
    unite: String,
    brut: i64,
    qte: f64,
    nb_ventes: i64,
    cout: i64,
    retours: i64,
    qte_rendue: f64,
    prix_achat: i64,
) -> (serde_json::Value, i64, i64) {
    let ca = crate::coeur::calcul::ca_net(brut, retours);
    // Tronque comme le `CAST(SUM(prix_achat * quantite))` du cout vendu.
    let cout_rendu = (prix_achat as f64 * qte_rendue) as i64;
    (
        serde_json::json!({
            "id":         id,
            "nom":        nom,
            "unite":      unite,
            "qte_vendue": qte - qte_rendue,
            "nb_ventes":  nb_ventes,
            "marge":      ca - (cout - cout_rendu),
        }),
        brut,
        retours,
    )
}

// =====================================================================
//  Rapport créances par ancienneté
// =====================================================================

pub fn lire_rapport_creances(
    conn: &rusqlite::Connection,
) -> Result<serde_json::Value, String> {

    // Tranches : <30j, 30-60j, 60-90j, >90j
    let tranches = vec![
        ("moins_30j", 0i64, 30i64),
        ("tranche_30_60", 30, 60),
        ("tranche_60_90", 60, 90),
        ("plus_90j", 90, 99999),
    ];

    let mut result = serde_json::json!({});

    for (cle, min_j, max_j) in &tranches {
        let (montant, nb): (i64, i64) = conn.query_row(
            &format!(
                "SELECT
                    CAST(COALESCE(SUM(
                      (SELECT COALESCE(SUM(prix_pratique * quantite), 0)
                       FROM ligne_vente WHERE vente_id = v.id) -
                      (SELECT COALESCE(SUM(montant), 0) FROM paiement WHERE vente_id = v.id)
                    ), 0) AS INTEGER),
                    COUNT(*)
                 FROM vente v
                 WHERE v.statut IN ('creance_ouverte','partiellement_payee')
                   AND CAST(julianday('now') - julianday(v.date_vente) AS INTEGER) >= {}
                   AND CAST(julianday('now') - julianday(v.date_vente) AS INTEGER) < {}",
                min_j, max_j
            ),
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        ).unwrap_or((0, 0));

        result[cle] = serde_json::json!({"montant": montant, "nb": nb});
    }

    Ok(result)
}

// =====================================================================
//  Rapport stock
// =====================================================================

pub fn lire_rapport_stock(
    conn: &rusqlite::Connection,
) -> Result<Vec<serde_json::Value>, String> {

    let mut stmt = conn.prepare(
        "SELECT a.nom, a.unite_base,
                COALESCE(sd.quantite, 0) as quantite,
                COALESCE(a.dernier_prix_achat, 0) as prix_achat,
                CAST(COALESCE(sd.quantite, 0) *
                     COALESCE(a.dernier_prix_achat, 0) AS INTEGER) as valeur_stock,
                d.nom as depot_nom,
                a.seuil_alerte
         FROM article a
         JOIN stock_depot sd ON sd.article_id = a.id
         JOIN depot d ON d.id = sd.depot_id
         WHERE a.actif = 1 AND a.gere_en_stock = 1
         ORDER BY valeur_stock DESC, a.nom"
    ).map_err(|e| e.to_string())?;

    let x = stmt.query_map([], |row| {
        let qte: f64 = row.get(2)?;
        Ok(serde_json::json!({
            "nom":          row.get::<_,String>(0)?,
            "unite":        row.get::<_,String>(1)?,
            "quantite":     qte,
            "prix_achat":   row.get::<_,i64>(3)?,
            "valeur_stock": row.get::<_,i64>(4)?,
            "depot":        row.get::<_,String>(5)?,
            // D45 : le seuil de l'article, sinon le defaut.
            "statut": crate::coeur::stock::statut_stock(
                qte,
                row.get::<_, Option<f64>>(6)?.unwrap_or(crate::coeur::stock::SEUIL_ALERTE_DEFAUT),
            ),
        }))
    }).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();
    Ok(x)
}

// =====================================================================
//  Rapport TVA collectée
// =====================================================================

pub fn lire_rapport_tva(
    conn: &rusqlite::Connection,
    date_debut: String,
    date_fin: String,
) -> Result<serde_json::Value, String> {

    let mut stmt = conn.prepare(
        "SELECT lv.taux_tva,
                CAST(SUM(lv.montant_tva) AS INTEGER) as total_tva,
                CAST(SUM(lv.prix_pratique * lv.quantite)
                     - SUM(lv.montant_tva) AS INTEGER) as total_ht,
                COUNT(DISTINCT v.id) as nb_ventes
         FROM ligne_vente lv
         JOIN vente v ON v.id = lv.vente_id
         WHERE v.date_vente BETWEEN ?1 AND ?2
           AND v.statut != 'annulee'
           AND lv.taux_tva > 0
         GROUP BY lv.taux_tva
         ORDER BY lv.taux_tva"
    ).map_err(|e| e.to_string())?;

    let ventes: Vec<(f64, i64, i64, i64)> = stmt.query_map(
        rusqlite::params![date_debut, date_fin],
        |row| Ok((row.get::<_,f64>(0)?, row.get::<_,i64>(1)?, row.get::<_,i64>(2)?, row.get::<_,i64>(3)?))
    ).map_err(|e| e.to_string())?.filter_map(|r| r.ok()).collect();

    // D53 : nette des retours dates dans la periode.
    let retours = tva_des_retours_conn(conn, &date_fin, None, |d| entre(d, &date_debut, &date_fin))?;
    Ok(rapport_tva(&ventes, &retours))
}

// =====================================================================
//  TVA des retours (D53) — commune au rapport TVA, au resume de
//  l'onglet Chantiers et au cahier du jour
// =====================================================================

/// `date` dans `[du, au]`, comparee comme le `BETWEEN` des ventes.
pub(crate) fn entre(date: &str, du: &str, au: &str) -> bool {
    date >= du && date <= au
}

/// Le rapport TVA par taux, net des retours (`coeur::tva::fusionner`).
/// `total_tva` est la TVA collectee nette ; `tva_retours` ce qui en a
/// ete deduit.
pub(crate) fn rapport_tva(ventes: &[(f64, i64, i64, i64)], retours: &[crate::coeur::tva::TvaRendue]) -> serde_json::Value {
    let lignes = crate::coeur::tva::fusionner(ventes, retours);
    let total_tva: i64 = lignes.iter().map(|l| l.total_tva).sum();
    let tva_retours: i64 = lignes.iter().map(|l| l.tva_retours).sum();
    let par_taux: Vec<serde_json::Value> = lignes
        .into_iter()
        .map(|l| {
            serde_json::json!({
                "taux":        l.taux,
                "total_tva":   l.total_tva,
                "total_ht":    l.total_ht,
                "nb_ventes":   l.nb_ventes,
                "tva_retours": l.tva_retours,
                "ht_retours":  l.ht_retours,
                "nb_retours":  l.nb_retours,
            })
        })
        .collect();
    serde_json::json!({
        "par_taux":    par_taux,
        "total_tva":   total_tva,
        "tva_retours": tva_retours,
    })
}

/// Les retours vus par la TVA : tous ceux des lignes taxees dont le jour
/// ne depasse pas celui de `au` (ceux d'avant la periode font le cumul
/// de leur ligne), dans l'ordre ou ils ont eu lieu. Un retour d'avant
/// v1.3 (sans ligne) prend la premiere ligne du meme article. Le magasin
/// est celui de la vente, comme pour le chiffre d'affaires.
const SQL_RETOURS_TAXES: &str =
    "SELECT lv.id, lv.taux_tva, CAST(lv.montant_tva AS BIGINT), lv.quantite,
            rt.quantite, CAST(rt.montant_credit AS BIGINT), rt.date_retour, rt.mode_resolution
     FROM retour rt
     JOIN vente v ON v.id = rt.vente_id
     JOIN ligne_vente lv ON lv.id = COALESCE(rt.ligne_vente_id,
          (SELECT MIN(l2.id) FROM ligne_vente l2
           WHERE l2.vente_id = rt.vente_id AND l2.article_id = rt.article_id))
     WHERE v.statut != 'annulee'
       AND lv.taux_tva > 0
       AND SUBSTR(rt.date_retour, 1, 10) <= ?1
       AND (CAST(?2 AS TEXT) IS NULL OR v.depot_id = ?2)
       AND rt.dossier_id = ?3
     ORDER BY rt.date_retour, rt.cree_le, rt.id";

/// (ligne, taux, tva de la ligne, qte de la ligne, qte rendue, credit, date, mode).
type LigneRetourTaxe = (String, f64, i64, f64, f64, i64, String, String);

fn retours_taxes(lignes: Vec<LigneRetourTaxe>, dans_periode: impl Fn(&str) -> bool) -> Vec<crate::coeur::tva::TvaRendue> {
    let retours: Vec<crate::coeur::tva::RetourTaxe> = lignes
        .into_iter()
        .map(|(ligne, taux, tva_ligne, qte_ligne, quantite, credit, date, mode)| crate::coeur::tva::RetourTaxe {
            ligne,
            taux,
            tva_ligne,
            qte_ligne,
            quantite,
            credit,
            dans_periode: dans_periode(&date),
            deduit: mode != crate::coeur::calcul::RETOUR_HORS_CA,
        })
        .collect();
    crate::coeur::tva::retours_par_taux(&retours)
}

/// La TVA rendue par les retours dont la date passe `dans_periode`,
/// par taux — sur l'un ou l'autre moteur, dedans ou hors transaction.
pub(crate) fn tva_des_retours_sur(
    acces: &mut impl crate::base::Acces,
    au: &str,
    depot: Option<String>,
    dans_periode: impl Fn(&str) -> bool,
) -> Result<Vec<crate::coeur::tva::TvaRendue>, String> {
    let dossier = acces.dossier().to_string();
    let jour: String = au.chars().take(10).collect();
    let lignes = acces
        .lire_plusieurs(SQL_RETOURS_TAXES, &crate::parametres![jour, depot, dossier], |r| {
            Ok((
                r.get::<String>(0)?,
                r.get::<f64>(1)?,
                r.get::<i64>(2)?,
                r.get::<f64>(3)?,
                r.get::<f64>(4)?,
                r.get::<i64>(5)?,
                r.get::<String>(6)?,
                r.get::<String>(7)?,
            ))
        })
        .map_err(|e| e.0)?;
    Ok(retours_taxes(lignes, dans_periode))
}

/// La meme lecture sur la connexion de la fenetre (un seul dossier).
pub(crate) fn tva_des_retours_conn(
    conn: &rusqlite::Connection,
    au: &str,
    depot: Option<String>,
    dans_periode: impl Fn(&str) -> bool,
) -> Result<Vec<crate::coeur::tva::TvaRendue>, String> {
    let jour: String = au.chars().take(10).collect();
    let sql = SQL_RETOURS_TAXES.replace("AND rt.dossier_id = ?3", "");
    let mut st = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let lignes: Vec<LigneRetourTaxe> = st
        .query_map(rusqlite::params![jour, depot], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?))
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    Ok(retours_taxes(lignes, dans_periode))
}
// =====================================================================
//  SUR L'UN OU L'AUTRE MOTEUR
// =====================================================================
//
// `strftime('%Y-%m', x)` devient `SUBSTR(x, 1, 7)` ; `date('now', '-N
// months')` se calcule ici ; les tranches d'anciennete des creances se
// rangent en Rust apres lecture (`utils::jours_depuis`).

use crate::base::Base;
use crate::parametres;

pub fn lire_rapport_ca_mensuel_sur_base(
    base: &mut Base,
    nb_mois: Option<i64>,
) -> Result<Vec<serde_json::Value>, String> {
    let dossier = base.dossier().to_string();
    let mois = nb_mois.unwrap_or(12).clamp(1, 24);
    let depuis = chrono::Local::now()
        .date_naive()
        .checked_sub_months(chrono::Months::new(mois as u32))
        .unwrap_or_else(|| chrono::Local::now().date_naive())
        .format("%Y-%m-%d")
        .to_string();

    // Vente par vente : le reste du se juge par vente (D41).
    let ventes: Vec<VenteDuMois> = base
        .lire_plusieurs(
            "SELECT SUBSTR(v.date_vente, 1, 7),
                    CAST(COALESCE((SELECT SUM(prix_pratique * quantite)
                                   FROM ligne_vente WHERE vente_id = v.id), 0) AS BIGINT),
                    CAST(COALESCE((SELECT SUM(montant) FROM paiement WHERE vente_id = v.id), 0) AS BIGINT),
                    CAST(COALESCE((SELECT SUM(montant) FROM paiement
                                   WHERE vente_id = v.id AND mode = 'retour'), 0) AS BIGINT)
             FROM vente v
             WHERE v.statut != 'annulee'
               AND SUBSTR(v.date_vente, 1, 10) >= ?1
               AND v.dossier_id = ?2",
            &parametres![depuis.clone(), dossier.clone()],
            |r| Ok((r.get::<String>(0)?, r.get::<i64>(1)?, r.get::<i64>(2)?, r.get::<i64>(3)?)),
        )
        .map_err(|e| e.0)?;
    // Les retours, par mois du RETOUR.
    let retours: Vec<(String, i64)> = base
        .lire_plusieurs(
            "SELECT SUBSTR(rt.date_retour, 1, 7),
                    CAST(COALESCE(SUM(rt.montant_credit), 0) AS BIGINT)
             FROM retour rt
             JOIN vente v ON v.id = rt.vente_id
             WHERE v.statut != 'annulee'
               AND rt.mode_resolution <> ?3
               AND SUBSTR(rt.date_retour, 1, 10) >= ?1
               AND rt.dossier_id = ?2
             GROUP BY SUBSTR(rt.date_retour, 1, 7)",
            &parametres![depuis.clone(), dossier, crate::coeur::calcul::RETOUR_HORS_CA],
            |r| Ok((r.get::<String>(0)?, r.get::<i64>(1)?)),
        )
        .map_err(|e| e.0)?;
    let mois = mois_du_rapport(ventes, retours);
    // Gescom Equipe : le cout des salaires du mois, a cote du CA.
    let salaires = crate::paie::salaires_par_mois_sur(base, &depuis[..7])?;
    Ok(crate::coeur::paie::avec_salaires(mois, &salaires))
}

pub fn lire_rapport_top_clients_sur_base(
    base: &mut Base,
    date_debut: String,
    date_fin: String,
    limite: Option<i64>,
) -> Result<Vec<serde_json::Value>, String> {
    let dossier = base.dossier().to_string();
    // Les retours de la periode, par client de la vente rendue ; le
    // classement se fait sur le chiffre net, la limite apres.
    let lignes = base
        .lire_plusieurs(
            "SELECT c.id, c.code, c.nom, c.telephone,
                    CAST(COALESCE(SUM(
                      (SELECT COALESCE(SUM(prix_pratique * quantite), 0)
                       FROM ligne_vente WHERE vente_id = v.id)
                    ), 0) AS BIGINT) AS ca,
                    COUNT(v.id),
                    CAST(COALESCE(SUM(
                      (SELECT COALESCE(SUM(prix_pratique * quantite), 0)
                       FROM ligne_vente WHERE vente_id = v.id) -
                      (SELECT COALESCE(SUM(montant), 0)
                       FROM paiement WHERE vente_id = v.id)
                    ), 0) AS BIGINT),
                    CAST(COALESCE((SELECT SUM(rt.montant_credit) FROM retour rt
                                   JOIN vente vr ON vr.id = rt.vente_id
                                   WHERE vr.client_id = c.id
                                     AND rt.date_retour BETWEEN ?1 AND ?2
                                     AND vr.statut != 'annulee'
                                     AND rt.mode_resolution <> ?3
                                     AND rt.dossier_id = ?4), 0) AS BIGINT)
             FROM vente v
             JOIN client c ON c.id = v.client_id
             WHERE v.date_vente BETWEEN ?1 AND ?2
               AND v.statut != 'annulee'
               AND c.est_generique = 0
               AND v.dossier_id = ?4
             GROUP BY c.id, c.code, c.nom, c.telephone",
            &parametres![date_debut, date_fin, crate::coeur::calcul::RETOUR_HORS_CA, dossier],
            |r| {
                Ok((
                    serde_json::json!({
                        "id":        r.get::<String>(0)?,
                        "code":      r.get::<String>(1)?,
                        "nom":       r.get::<String>(2)?,
                        "telephone": r.get::<Option<String>>(3)?,
                        "nb_ventes": r.get::<i64>(5)?,
                        "creances":  r.get::<i64>(6)?,
                    }),
                    r.get::<i64>(4)?,
                    r.get::<i64>(7)?,
                ))
            },
        )
        .map_err(|e| e.0)?;
    Ok(crate::tableau_bord::classer_par_ca_net(lignes, limite.unwrap_or(20).max(0) as usize))
}

pub fn lire_rapport_top_articles_sur_base(
    base: &mut Base,
    date_debut: String,
    date_fin: String,
    limite: Option<i64>,
) -> Result<Vec<serde_json::Value>, String> {
    let dossier = base.dossier().to_string();
    // Les retours de la periode, par article rendu (voir la version
    // fenetre et `article_du_rapport`).
    let lignes = base
        .lire_plusieurs(
            "SELECT a.id, a.nom, a.unite_base,
                    CAST(COALESCE(SUM(lv.prix_pratique * lv.quantite), 0) AS BIGINT) AS ca,
                    CAST(COALESCE(SUM(lv.quantite), 0) AS DOUBLE PRECISION),
                    COUNT(DISTINCT lv.vente_id),
                    CAST(COALESCE(SUM(COALESCE(a.dernier_prix_achat, 0) * lv.quantite), 0) AS BIGINT),
                    CAST(COALESCE((SELECT SUM(rt.montant_credit) FROM retour rt
                                   JOIN vente vr ON vr.id = rt.vente_id
                                   WHERE rt.article_id = a.id
                                     AND rt.date_retour BETWEEN ?1 AND ?2
                                     AND vr.statut != 'annulee'
                                     AND rt.mode_resolution <> ?3
                                     AND rt.dossier_id = ?4), 0) AS BIGINT),
                    CAST(COALESCE((SELECT SUM(rt.quantite) FROM retour rt
                                   JOIN vente vr ON vr.id = rt.vente_id
                                   WHERE rt.article_id = a.id
                                     AND rt.date_retour BETWEEN ?1 AND ?2
                                     AND vr.statut != 'annulee'
                                     AND rt.mode_resolution <> ?3
                                     AND rt.dossier_id = ?4), 0) AS DOUBLE PRECISION),
                    CAST(COALESCE(a.dernier_prix_achat, 0) AS BIGINT)
             FROM ligne_vente lv
             JOIN article a ON a.id = lv.article_id
             JOIN vente v ON v.id = lv.vente_id
             WHERE v.date_vente BETWEEN ?1 AND ?2
               AND v.statut != 'annulee'
               AND v.dossier_id = ?4
             GROUP BY a.id, a.nom, a.unite_base, a.dernier_prix_achat",
            &parametres![date_debut, date_fin, crate::coeur::calcul::RETOUR_HORS_CA, dossier],
            |r| {
                Ok(article_du_rapport(
                    r.get::<String>(0)?, r.get::<String>(1)?, r.get::<String>(2)?,
                    r.get::<i64>(3)?, r.get::<f64>(4)?, r.get::<i64>(5)?,
                    r.get::<i64>(6)?, r.get::<i64>(7)?, r.get::<f64>(8)?,
                    r.get::<i64>(9)?,
                ))
            },
        )
        .map_err(|e| e.0)?;
    Ok(crate::tableau_bord::classer_par_ca_net(lignes, limite.unwrap_or(20).max(0) as usize))
}

pub fn lire_rapport_creances_sur_base(base: &mut Base) -> Result<serde_json::Value, String> {
    let dossier = base.dossier().to_string();
    let creances: Vec<(String, i64)> = base
        .lire_plusieurs(
            "SELECT v.date_vente,
                    CAST(
                      (SELECT COALESCE(SUM(prix_pratique * quantite), 0)
                       FROM ligne_vente WHERE vente_id = v.id) -
                      (SELECT COALESCE(SUM(montant), 0) FROM paiement WHERE vente_id = v.id)
                    AS BIGINT)
             FROM vente v
             WHERE v.statut IN ('creance_ouverte','partiellement_payee')
               AND v.dossier_id = ?1",
            &parametres![dossier],
            |r| Ok((r.get::<String>(0)?, r.get::<i64>(1)?)),
        )
        .map_err(|e| e.0)?;

    // Tranches : <30j, 30-60j, 60-90j, >90j — rangees en Rust.
    let tranches = [("moins_30j", 0i64, 30i64), ("tranche_30_60", 30, 60), ("tranche_60_90", 60, 90), ("plus_90j", 90, i64::MAX)];
    let mut result = serde_json::json!({});
    for (cle, min_j, max_j) in tranches {
        let (montant, nb) = creances
            .iter()
            .filter(|(date, _)| {
                let j = crate::utils::jours_depuis(date);
                j >= min_j && j < max_j
            })
            .fold((0i64, 0i64), |(m, n), (_, reste)| (m + reste, n + 1));
        result[cle] = serde_json::json!({"montant": montant, "nb": nb});
    }
    Ok(result)
}

pub fn lire_rapport_stock_sur_base(base: &mut Base) -> Result<Vec<serde_json::Value>, String> {
    let dossier = base.dossier().to_string();
    let defaut = crate::assistant::seuil_defaut_sur(base);
    base.lire_plusieurs(
        "SELECT a.nom, a.unite_base,
                COALESCE(sd.quantite, 0),
                COALESCE(a.dernier_prix_achat, 0),
                CAST(COALESCE(sd.quantite, 0) * COALESCE(a.dernier_prix_achat, 0) AS BIGINT) AS valeur_stock,
                d.nom,
                a.seuil_alerte
         FROM article a
         JOIN stock_depot sd ON sd.article_id = a.id
         JOIN depot d ON d.id = sd.depot_id
         WHERE a.actif = 1 AND a.gere_en_stock = 1 AND sd.dossier_id = ?1
         ORDER BY valeur_stock DESC, a.nom",
        &parametres![dossier],
        |r| {
            let qte: f64 = r.get::<f64>(2)?;
            Ok(serde_json::json!({
                "nom":          r.get::<String>(0)?,
                "unite":        r.get::<String>(1)?,
                "quantite":     qte,
                "prix_achat":   r.get::<i64>(3)?,
                "valeur_stock": r.get::<i64>(4)?,
                "depot":        r.get::<String>(5)?,
                // D45 : le seuil de l'article, sinon celui du dossier.
                "statut": crate::coeur::stock::statut_stock(qte, r.get::<Option<f64>>(6)?.unwrap_or(defaut)),
            }))
        },
    )
    .map_err(|e| e.0)
}

pub fn lire_rapport_tva_sur_base(
    base: &mut Base,
    date_debut: String,
    date_fin: String,
) -> Result<serde_json::Value, String> {
    let dossier = base.dossier().to_string();
    let ventes: Vec<(f64, i64, i64, i64)> = base
        .lire_plusieurs(
            "SELECT lv.taux_tva,
                    CAST(SUM(lv.montant_tva) AS BIGINT),
                    CAST(SUM(lv.prix_pratique * lv.quantite) - SUM(lv.montant_tva) AS BIGINT),
                    COUNT(DISTINCT v.id)
             FROM ligne_vente lv
             JOIN vente v ON v.id = lv.vente_id
             WHERE v.date_vente BETWEEN ?1 AND ?2
               AND v.statut != 'annulee'
               AND lv.taux_tva > 0
               AND v.dossier_id = ?3
             GROUP BY lv.taux_tva
             ORDER BY lv.taux_tva",
            &parametres![date_debut.clone(), date_fin.clone(), dossier],
            |r| Ok((r.get::<f64>(0)?, r.get::<i64>(1)?, r.get::<i64>(2)?, r.get::<i64>(3)?)),
        )
        .map_err(|e| e.0)?;
    // D53 : nette des retours dates dans la periode.
    let retours = tva_des_retours_sur(base, &date_fin, None, |d| entre(d, &date_debut, &date_fin))?;
    Ok(rapport_tva(&ventes, &retours))
}
