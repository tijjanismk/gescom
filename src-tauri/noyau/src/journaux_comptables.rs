//! Les journaux comptables, fabriques a la lecture (v3, E-3 — D23).
//!
//! Quatre journaux, du dossier courant, sur une periode :
//! - VT ventes : chaque vente au debit du client (TTC), au credit des
//!   ventes (HT) et de la TVA collectee ; une creance irrecouvrable ;
//! - AC achats : chaque achat de marchandises (les entrees `achat` du
//!   stock, comme le cahier du jour) au credit du fournisseur ;
//! - RG reglements : ce que les clients paient (tresorerie du mode de
//!   paiement / client ; un avoir utilise solde le client sur les
//!   ventes) et ce qu'on paie aux fournisseurs ;
//! - CA caisse : les depenses par categorie, les ecarts de cloture ;
//! - PA paie (Gescom Equipe, G-4) : chaque fiche validee (salaires bruts
//!   au debit, remunerations dues, cotisations, retenues, avances
//!   retenues au credit ; charges patronales), la fiche remplacee par
//!   une rectificative contre-passee le jour de celle-ci, les
//!   versements, les avances donnees et annulees. Lu seulement par qui
//!   prepare ou valide la paie : il nomme ce que chacun gagne.
//!
//! Les comptes viennent de l'affectation du dossier (E-2). Rien ne
//! s'ecrit. Pas encore : les retours de marchandise (leur effet passe
//! par l'avoir utilise), les OD, la TVA sur achats (non saisie).

use std::collections::HashMap;

use crate::base::Base;
use crate::coeur::affectations::{cle_depense, cle_tresorerie};
use crate::coeur::journaux::{self as regles, credit, debit, equilibrer, simple, Ecriture};
use crate::parametres;

struct Periode {
    du: String,
    au: String,
}

fn periode(du: &str, au: &str) -> Result<Periode, String> {
    let du = crate::coeur::historique::jour_filtre(Some(du))?.ok_or("La période n'a pas de début.")?;
    let au = crate::coeur::historique::jour_filtre(Some(au))?.ok_or("La période n'a pas de fin.")?;
    if au < du {
        return Err("La fin de la période précède son début.".to_string());
    }
    Ok(Periode { du, au })
}

fn piece_courte(prefixe: &str, id: &str) -> String {
    format!("{prefixe}-{}", id.chars().take(8).collect::<String>().to_uppercase())
}

fn ventes(base: &mut Base, p: &Periode, a: &HashMap<String, String>) -> Result<Vec<Ecriture>, String> {
    let dossier = base.dossier().to_string();
    let lignes = base
        .lire_plusieurs(
            "SELECT v.id, v.date_vente, c.nom, p.numero,
                    CAST(SUM(CAST(lv.prix_pratique * lv.quantite AS BIGINT)) AS BIGINT),
                    CAST(SUM(COALESCE(lv.montant_tva, 0)) AS BIGINT)
             FROM vente v
             JOIN ligne_vente lv ON lv.vente_id = v.id
             JOIN client c ON c.id = v.client_id
             LEFT JOIN piece_commerciale p ON p.id = v.piece_id
             WHERE v.dossier_id = ?1 AND v.statut != 'annulee'
               AND SUBSTR(v.date_vente, 1, 10) >= ?2 AND SUBSTR(v.date_vente, 1, 10) <= ?3
             GROUP BY v.id, v.date_vente, c.nom, p.numero
             ORDER BY v.date_vente",
            &parametres![dossier.clone(), p.du.clone(), p.au.clone()],
            |r| {
                Ok((
                    r.get::<String>(0)?,
                    r.get::<String>(1)?,
                    r.get::<String>(2)?,
                    r.get::<Option<String>>(3)?,
                    r.get::<i64>(4)?,
                    r.get::<i64>(5)?,
                ))
            },
        )
        .map_err(|e| e.0)?;
    let mut v = Vec::new();
    for (id, date, client, numero, ttc, tva) in lignes {
        if ttc == 0 {
            continue;
        }
        v.push(equilibrer(Ecriture {
            journal: "VT",
            date: date.chars().take(10).collect(),
            piece: numero.unwrap_or_else(|| piece_courte("V", &id)),
            libelle: format!("Vente — {client}"),
            lignes: vec![
                debit(&a["ventes:clients"], ttc),
                credit(&a["ventes:marchandises"], ttc - tva),
                credit(&a["ventes:tva"], tva),
            ],
        })?);
    }

    // Les creances passees en perte : ce qui restait du, a la date du geste.
    let pertes = base
        .lire_plusieurs(
            "SELECT ci.vente_id, ci.date_marque, c.nom,
                    CAST(COALESCE((SELECT SUM(CAST(lv.prix_pratique * lv.quantite AS BIGINT))
                       FROM ligne_vente lv WHERE lv.vente_id = v.id), 0) AS BIGINT),
                    CAST(COALESCE((SELECT SUM(pa.montant) FROM paiement pa
                       WHERE pa.vente_id = v.id AND pa.dossier_id = ?1), 0) AS BIGINT)
             FROM creance_irrecouvrable ci
             JOIN vente v ON v.id = ci.vente_id
             JOIN client c ON c.id = v.client_id
             WHERE ci.dossier_id = ?1
               AND SUBSTR(ci.date_marque, 1, 10) >= ?2 AND SUBSTR(ci.date_marque, 1, 10) <= ?3
             ORDER BY ci.date_marque",
            &parametres![dossier, p.du.clone(), p.au.clone()],
            |r| Ok((r.get::<String>(0)?, r.get::<String>(1)?, r.get::<String>(2)?, r.get::<i64>(3)?, r.get::<i64>(4)?)),
        )
        .map_err(|e| e.0)?;
    for (id, date, client, total, paye) in pertes {
        if let Some(e) = simple(
            "VT",
            &date,
            piece_courte("IRR", &id),
            format!("Créance irrécouvrable — {client}"),
            &a["ventes:irrecouvrables"],
            &a["ventes:clients"],
            total - paye,
        ) {
            v.push(e);
        }
    }
    Ok(v)
}

fn achats(base: &mut Base, p: &Periode, a: &HashMap<String, String>) -> Result<Vec<Ecriture>, String> {
    let dossier = base.dossier().to_string();
    let lignes = base
        .lire_plusieurs(
            "SELECT COALESCE(ms.operation_id, ms.id), ms.date_mouvement, COALESCE(f.nom, 'Sans fournisseur'),
                    ms.quantite_delta, COALESCE(ms.prix_achat_unitaire, a.dernier_prix_achat, 0)
             FROM mouvement_stock ms
             JOIN article a ON a.id = ms.article_id
             LEFT JOIN fournisseur f ON f.id = ms.fournisseur_id
             WHERE ms.type_mouvement = 'achat' AND ms.dossier_id = ?1
               AND SUBSTR(ms.date_mouvement, 1, 10) >= ?2 AND SUBSTR(ms.date_mouvement, 1, 10) <= ?3
             ORDER BY ms.date_mouvement",
            &parametres![dossier, p.du.clone(), p.au.clone()],
            |r| Ok((r.get::<String>(0)?, r.get::<String>(1)?, r.get::<String>(2)?, r.get::<f64>(3)?, r.get::<i64>(4)?)),
        )
        .map_err(|e| e.0)?;
    // Une ecriture par achat (ses lignes regroupees), dans l'ordre.
    let mut ordre: Vec<String> = Vec::new();
    let mut par_achat: HashMap<String, (String, String, i64)> = HashMap::new();
    for (op, date, fournisseur, qte, pu) in lignes {
        // Meme arrondi que le cahier du jour, ligne par ligne.
        let montant = (pu as f64 * qte).round() as i64;
        let e = par_achat.entry(op.clone()).or_insert_with(|| {
            ordre.push(op.clone());
            (date, fournisseur, 0)
        });
        e.2 += montant;
    }
    Ok(ordre
        .into_iter()
        .filter_map(|op| {
            let (date, fournisseur, montant) = par_achat.remove(&op)?;
            simple(
                "AC",
                &date,
                piece_courte("A", &op),
                format!("Achat — {fournisseur}"),
                &a["achats:marchandises"],
                &a["achats:fournisseurs"],
                montant,
            )
        })
        .collect())
}

fn reglements(base: &mut Base, p: &Periode, a: &HashMap<String, String>) -> Result<Vec<Ecriture>, String> {
    let dossier = base.dossier().to_string();
    let clients = base
        .lire_plusieurs(
            "SELECT pa.id, pa.date_paiement, pa.montant, pa.mode, c.nom
             FROM paiement pa
             JOIN vente v ON v.id = pa.vente_id
             JOIN client c ON c.id = v.client_id
             WHERE pa.dossier_id = ?1
               AND SUBSTR(pa.date_paiement, 1, 10) >= ?2 AND SUBSTR(pa.date_paiement, 1, 10) <= ?3
             ORDER BY pa.date_paiement",
            &parametres![dossier.clone(), p.du.clone(), p.au.clone()],
            |r| Ok((r.get::<String>(0)?, r.get::<String>(1)?, r.get::<i64>(2)?, r.get::<String>(3)?, r.get::<String>(4)?)),
        )
        .map_err(|e| e.0)?;
    let mut v = Vec::new();
    for (id, date, montant, mode, client) in clients {
        let (compte, libelle) = if mode == "avoir" {
            (a["ventes:retours"].clone(), format!("Avoir utilisé — {client}"))
        } else {
            (a[cle_tresorerie(&mode)].clone(), format!("Règlement {} — {client}", regles::libelle_mode(&mode)))
        };
        if let Some(e) = simple("RG", &date, piece_courte("P", &id), libelle, &compte, &a["ventes:clients"], montant) {
            v.push(e);
        }
    }
    let fournisseurs = base
        .lire_plusieurs(
            "SELECT pf.id, pf.date_paiement, pf.montant, pf.mode, COALESCE(f.nom, 'Fournisseur')
             FROM paiement_fournisseur pf
             LEFT JOIN fournisseur f ON f.id = pf.fournisseur_id
             WHERE pf.dossier_id = ?1
               AND SUBSTR(pf.date_paiement, 1, 10) >= ?2 AND SUBSTR(pf.date_paiement, 1, 10) <= ?3
             ORDER BY pf.date_paiement",
            &parametres![dossier, p.du.clone(), p.au.clone()],
            |r| Ok((r.get::<String>(0)?, r.get::<String>(1)?, r.get::<i64>(2)?, r.get::<String>(3)?, r.get::<String>(4)?)),
        )
        .map_err(|e| e.0)?;
    for (id, date, montant, mode, fournisseur) in fournisseurs {
        if let Some(e) = simple(
            "RG",
            &date,
            piece_courte("PF", &id),
            format!("Paiement {} — {fournisseur}", regles::libelle_mode(&mode)),
            &a["achats:fournisseurs"],
            &a[cle_tresorerie(&mode)],
            montant,
        ) {
            v.push(e);
        }
    }
    v.sort_by(|x, y| x.date.cmp(&y.date));
    Ok(v)
}

fn caisse(base: &mut Base, p: &Periode, a: &HashMap<String, String>) -> Result<Vec<Ecriture>, String> {
    let dossier = base.dossier().to_string();
    let depenses = base
        .lire_plusieurs(
            "SELECT id, date_mouvement, montant, moyen, categorie, COALESCE(libelle, '')
             FROM mouvement_caisse
             WHERE motif = 'depense' AND dossier_id = ?1
               AND SUBSTR(date_mouvement, 1, 10) >= ?2 AND SUBSTR(date_mouvement, 1, 10) <= ?3
             ORDER BY date_mouvement",
            &parametres![dossier.clone(), p.du.clone(), p.au.clone()],
            |r| {
                Ok((
                    r.get::<String>(0)?,
                    r.get::<String>(1)?,
                    r.get::<i64>(2)?,
                    r.get::<String>(3)?,
                    r.get::<Option<String>>(4)?,
                    r.get::<String>(5)?,
                ))
            },
        )
        .map_err(|e| e.0)?;
    let mut v = Vec::new();
    for (id, date, montant, moyen, categorie, libelle) in depenses {
        let texte = if libelle.is_empty() { "Dépense".to_string() } else { format!("Dépense — {libelle}") };
        if let Some(e) = simple(
            "CA",
            &date,
            piece_courte("D", &id),
            texte,
            &a[cle_depense(categorie.as_deref())],
            &a[cle_tresorerie(&moyen)],
            montant,
        ) {
            v.push(e);
        }
    }
    let ecarts = base
        .lire_plusieurs(
            "SELECT id, ferme_le, ecart FROM session_caisse
             WHERE statut = 'fermee' AND ecart <> 0 AND dossier_id = ?1
               AND SUBSTR(ferme_le, 1, 10) >= ?2 AND SUBSTR(ferme_le, 1, 10) <= ?3
             ORDER BY ferme_le",
            &parametres![dossier, p.du.clone(), p.au.clone()],
            |r| Ok((r.get::<String>(0)?, r.get::<String>(1)?, r.get::<i64>(2)?)),
        )
        .map_err(|e| e.0)?;
    let especes = &a["tresorerie:especes"];
    for (id, date, ecart) in ecarts {
        // ecart = compte - theorique (coeur::caisse) : negatif, il manque.
        let e = if ecart < 0 {
            simple("CA", &date, piece_courte("S", &id), "Manquant à la clôture".into(), &a["caisse:ecart_manquant"], especes, -ecart)
        } else {
            simple("CA", &date, piece_courte("S", &id), "Excédent à la clôture".into(), especes, &a["caisse:ecart_excedent"], ecart)
        };
        v.extend(e);
    }
    v.sort_by(|x, y| x.date.cmp(&y.date));
    Ok(v)
}

/// L'ecriture d'une fiche validee, depuis ses lignes. `sens` -1 : la
/// contre-passation d'une fiche remplacee.
fn ecriture_fiche(
    date: &str,
    piece: String,
    libelle: String,
    lignes: &[crate::coeur::paie::Ligne],
    a: &HashMap<String, String>,
    sens: i64,
) -> Result<Option<Ecriture>, String> {
    use crate::coeur::paie as p;
    let organisme = |l: &p::Ligne| l.source.clone().unwrap_or_else(|| a["paie:organismes"].clone());
    let brut: i64 = lignes.iter().filter(|l| l.genre != p::CHARGE && l.montant > 0).map(|l| l.montant).sum();
    let cot: i64 = lignes.iter().filter(|l| l.genre == p::COTISATION).map(|l| -l.montant).sum();
    let ret: i64 = lignes.iter().filter(|l| l.genre == p::RETENUE).map(|l| -l.montant).sum();
    let av: i64 = lignes.iter().filter(|l| l.genre == p::AVANCE).map(|l| -l.montant).sum();
    let dues = &a["paie:remunerations_dues"];
    let mut l = vec![debit(&a["paie:salaires"], brut), credit(dues, brut - cot - ret), credit(&a["paie:retenues"], ret)];
    for x in lignes.iter().filter(|l| l.genre == p::COTISATION) {
        l.push(credit(&organisme(x), -x.montant));
    }
    for x in lignes.iter().filter(|l| l.genre == p::CHARGE) {
        l.push(debit(&a["paie:charges_sociales"], x.montant));
        l.push(credit(&organisme(x), x.montant));
    }
    l.push(debit(dues, av));
    l.push(credit(&a["paie:avances"], av));
    if sens < 0 {
        for x in &mut l {
            std::mem::swap(&mut x.debit, &mut x.credit);
        }
    }
    let e = equilibrer(Ecriture { journal: "PA", date: date.chars().take(10).collect(), piece, libelle, lignes: l })?;
    Ok(if e.lignes.is_empty() { None } else { Some(e) })
}

fn paie(base: &mut Base, p: &Periode, a: &HashMap<String, String>) -> Result<Vec<Ecriture>, String> {
    let dossier = base.dossier().to_string();
    let mut v = Vec::new();
    // Les fiches validees dans la periode, et celles remplacees par une
    // rectificative validee dans la periode (contre-passees ce jour-la).
    let fiches = base
        .lire_plusieurs(
            "SELECT f.id, f.numero, f.nom, f.valide_le, 1 FROM fiche_paie f
             WHERE f.dossier_id = ?1 AND f.statut IN ('validee', 'remplacee')
               AND SUBSTR(f.valide_le, 1, 10) >= ?2 AND SUBSTR(f.valide_le, 1, 10) <= ?3
             UNION ALL
             SELECT f.id, f.numero, f.nom, n.valide_le, -1 FROM fiche_paie f
             JOIN fiche_paie n ON n.rectifie_id = f.id AND n.dossier_id = f.dossier_id AND n.statut IN ('validee', 'remplacee')
             WHERE f.dossier_id = ?1 AND f.statut = 'remplacee'
               AND SUBSTR(n.valide_le, 1, 10) >= ?2 AND SUBSTR(n.valide_le, 1, 10) <= ?3",
            &parametres![dossier.clone(), p.du.clone(), p.au.clone()],
            |r| Ok((r.get::<String>(0)?, r.get::<String>(1)?, r.get::<String>(2)?, r.get::<String>(3)?, r.get::<i64>(4)?)),
        )
        .map_err(|e| e.0)?;
    for (id, numero, nom, date, sens) in fiches {
        let lignes = base
            .lire_plusieurs(
                "SELECT genre, libelle, montant, source FROM ligne_paie WHERE fiche_id = ?1 AND dossier_id = ?2 ORDER BY rang",
                &parametres![id, dossier.clone()],
                |r| {
                    Ok(crate::coeur::paie::Ligne {
                        genre: r.get(0)?,
                        libelle: r.get(1)?,
                        quantite: None,
                        prix: None,
                        montant: r.get(2)?,
                        source: r.get(3)?,
                        saisie: false,
                    })
                },
            )
            .map_err(|e| e.0)?;
        let libelle = if sens > 0 { format!("Paie — {nom}") } else { format!("Paie remplacée par sa rectificative — {nom}") };
        v.extend(ecriture_fiche(&date, numero, libelle, &lignes, a, sens)?);
    }
    let versements = base
        .lire_plusieurs(
            "SELECT v.date_versement, v.montant, v.moyen, f.numero, f.nom FROM versement_paie v
             JOIN fiche_paie f ON f.id = v.fiche_id AND f.dossier_id = v.dossier_id
             WHERE v.dossier_id = ?1 AND SUBSTR(v.date_versement, 1, 10) >= ?2 AND SUBSTR(v.date_versement, 1, 10) <= ?3",
            &parametres![dossier.clone(), p.du.clone(), p.au.clone()],
            |r| Ok((r.get::<String>(0)?, r.get::<i64>(1)?, r.get::<String>(2)?, r.get::<Option<String>>(3)?, r.get::<String>(4)?)),
        )
        .map_err(|e| e.0)?;
    for (date, montant, moyen, numero, nom) in versements {
        v.extend(simple(
            "PA",
            &date,
            numero.unwrap_or_default(),
            format!("Salaire versé {} — {nom}", regles::libelle_mode(&moyen)),
            &a["paie:remunerations_dues"],
            &a[cle_tresorerie(&moyen)],
            montant,
        ));
    }
    let avances = base
        .lire_plusieurs(
            "SELECT a.id, a.date_avance, a.montant, a.moyen, e.nom FROM avance a
             JOIN employe e ON e.id = a.employe_id AND e.dossier_id = a.dossier_id
             WHERE a.dossier_id = ?1 AND SUBSTR(a.date_avance, 1, 10) >= ?2 AND SUBSTR(a.date_avance, 1, 10) <= ?3",
            &parametres![dossier.clone(), p.du.clone(), p.au.clone()],
            |r| Ok((r.get::<String>(0)?, r.get::<String>(1)?, r.get::<i64>(2)?, r.get::<String>(3)?, r.get::<String>(4)?)),
        )
        .map_err(|e| e.0)?;
    for (id, date, montant, moyen, nom) in avances {
        v.extend(simple(
            "PA",
            &date,
            piece_courte("AV", &id),
            format!("Avance {} — {nom}", regles::libelle_mode(&moyen)),
            &a["paie:avances"],
            &a[cle_tresorerie(&moyen)],
            montant,
        ));
    }
    // Une avance annulee : l'argent revient (l'entree de caisse le date).
    let annulees = base
        .lire_plusieurs(
            "SELECT m.operation_id, m.date_mouvement, m.montant, m.moyen, COALESCE(m.libelle, '') FROM mouvement_caisse m
             WHERE m.motif = 'avance_annulee' AND m.dossier_id = ?1
               AND SUBSTR(m.date_mouvement, 1, 10) >= ?2 AND SUBSTR(m.date_mouvement, 1, 10) <= ?3",
            &parametres![dossier, p.du.clone(), p.au.clone()],
            |r| Ok((r.get::<String>(0)?, r.get::<String>(1)?, r.get::<i64>(2)?, r.get::<String>(3)?, r.get::<String>(4)?)),
        )
        .map_err(|e| e.0)?;
    for (id, date, montant, moyen, libelle) in annulees {
        v.extend(simple("PA", &date, piece_courte("AV", &id), libelle, &a[cle_tresorerie(&moyen)], &a["paie:avances"], montant));
    }
    v.sort_by(|x, y| x.date.cmp(&y.date));
    Ok(v)
}

/// Les ecritures d'un journal (`VT`, `AC`, `RG`, `CA`, `PA`) ou de
/// tous. `avec_paie` : faux pour qui ne lit pas la paie (le serveur le
/// neutralise, `coeur::lecture`) — PA n'est alors ni fabrique ni rendu.
pub fn ecritures_sur(base: &mut Base, du: &str, au: &str, journal: Option<&str>, avec_paie: bool) -> Result<Vec<Ecriture>, String> {
    let p = periode(du, au)?;
    if let Some(j) = journal {
        if !regles::JOURNAUX.iter().any(|(c, _)| *c == j) {
            return Err(format!("Journal inconnu : « {j} ». Attendu : VT, AC, RG, CA ou PA."));
        }
        if j == "PA" && !avec_paie {
            return Err("Le journal de paie ne se lit qu'avec le droit de préparer ou valider la paie.".to_string());
        }
    }
    let a = crate::affectations::table_sur(base)?;
    let veut = |c: &str| journal.is_none_or(|j| j == c);
    let mut tout = Vec::new();
    if veut("VT") {
        tout.extend(ventes(base, &p, &a)?);
    }
    if veut("AC") {
        tout.extend(achats(base, &p, &a)?);
    }
    if veut("RG") {
        tout.extend(reglements(base, &p, &a)?);
    }
    if veut("CA") {
        tout.extend(caisse(base, &p, &a)?);
    }
    if avec_paie && veut("PA") {
        tout.extend(paie(base, &p, &a)?);
    }
    Ok(tout)
}

/// Pour l'ecran : chaque journal, ses ecritures et ses totaux.
pub fn lire_sur(base: &mut Base, du: String, au: String, journal: Option<String>, avec_paie: bool) -> Result<serde_json::Value, String> {
    let tout = ecritures_sur(base, &du, &au, journal.as_deref(), avec_paie)?;
    let mut libelles: HashMap<String, Option<String>> = HashMap::new();
    for e in &tout {
        for l in &e.lignes {
            if !libelles.contains_key(&l.compte) {
                let lib = crate::plan_comptable::libelle_sur(base, &l.compte)?;
                libelles.insert(l.compte.clone(), lib);
            }
        }
    }
    let journaux: Vec<serde_json::Value> = regles::JOURNAUX
        .iter()
        .filter(|(c, _)| journal.as_deref().is_none_or(|j| j == *c) && (avec_paie || *c != "PA"))
        .map(|(code, libelle)| {
            let e: Vec<Ecriture> = tout.iter().filter(|e| e.journal == *code).cloned().collect();
            let (d, c) = regles::totaux(&e);
            serde_json::json!({
                "code": code, "libelle": libelle, "ecritures": e,
                "total_debit": d, "total_credit": c,
            })
        })
        .collect();
    Ok(serde_json::json!({ "journaux": journaux, "comptes": libelles }))
}

/// L'export CSV, et un nom de fichier qui dit ce qu'il contient.
pub fn csv_sur(base: &mut Base, du: String, au: String, journal: Option<String>, avec_paie: bool) -> Result<serde_json::Value, String> {
    let tout = ecritures_sur(base, &du, &au, journal.as_deref(), avec_paie)?;
    let mut libelles: HashMap<String, String> = HashMap::new();
    for e in &tout {
        for l in &e.lignes {
            if !libelles.contains_key(&l.compte) {
                let lib = crate::plan_comptable::libelle_sur(base, &l.compte)?.unwrap_or_default();
                libelles.insert(l.compte.clone(), lib);
            }
        }
    }
    let contenu = regles::csv(&tout, &|n| libelles.get(n).cloned().unwrap_or_default());
    let nom = format!(
        "journaux_{}_{}_{}.csv",
        journal.as_deref().unwrap_or("tous"),
        du.chars().take(10).collect::<String>(),
        au.chars().take(10).collect::<String>()
    );
    Ok(serde_json::json!({ "nom_fichier": nom, "contenu": contenu, "lignes": tout.iter().map(|e| e.lignes.len()).sum::<usize>() }))
}
