//! La fiche de paie, calculee (PLAN-EQUIPE, G-2 — D31). Pur.
//!
//! Une fiche part de la facon dont la personne est payee (D30) et de ce
//! que la base sait de la periode — jours d'emploi, jours marques,
//! ventes signees par son compte et leurs retours, avances en cours —
//! plus ce qu'on y saisit : lignes a la tache, primes, retenues (casse,
//! absence). Chaque ligne dit d'ou elle vient ; une ligne calculee se
//! refait a chaque calcul, une ligne saisie reste.
//!
//! Les avances se retiennent d'office, les plus anciennes d'abord, dans
//! la limite de ce que la personne a gagne : ce qui depasse se reporte
//! sur la fiche suivante (D32).
//!
//! **Tout l'argent se compte en francs entiers** (revue du 28/10) : un
//! taux devient des millioniemes, une quantite des milliemes, un jour
//! des demi-journees, et chaque ligne s'arrondit UNE fois, au franc le
//! plus proche, la moitie vers le haut (`arrondi`). Les sommes sont
//! verifiees (`coeur::saisie::somme`) : un depassement est un refus, pas
//! une panique du serveur.
//!
//! Les bornes d'emploi (revue du 28/10, point 1) : une fiche ne paie que
//! les jours ou la personne etait employee — entree, depart — quelle que
//! soit la periode demandee, avec ou sans prorata. Le prorata (point 2,
//! choix du proprietaire) : un jour non marque compte comme present ;
//! seuls une absence (un jour) et une demi-journee (la moitie) retirent.

use serde::{Deserialize, Serialize};

use crate::coeur::personnel::Remuneration;
use crate::coeur::plafonds::francs;
use crate::coeur::saisie::{borner_montant, borner_quantite, somme};

/// Les genres de ligne. Les trois premiers se calculent, les autres se
/// saisissent, sauf `avance` (calculee aussi).
pub const BASE: &str = "base";
pub const JOURS: &str = "jours";
pub const COMMISSION: &str = "commission";
pub const TACHE: &str = "tache";
pub const PRIME: &str = "prime";
pub const RETENUE: &str = "retenue";
pub const AVANCE: &str = "avance";
/// G-4 (D33) : une cotisation retenue sur le salaire (negative) ; une
/// charge de la societe, dite sur le bulletin mais hors du net.
pub const COTISATION: &str = "cotisation";
pub const CHARGE: &str = "charge_patronale";

/// Le « moyen » d'une avance nee d'un trop-verse (revue du 28/10,
/// point 6) : l'argent est deja sorti avec les versements de la fiche,
/// rien ne sort de la tresorerie. Le journal PA la passe 421 / 422.
pub const MOYEN_TROP_VERSE: &str = "trop_verse";

/// Ce qu'on peut saisir sur une fiche.
pub const SAISISSABLES: &[&str] = &[TACHE, PRIME, RETENUE];

/// Une ligne de fiche. `montant` est signe : un gain est positif, une
/// retenue negative.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ligne {
    pub genre: String,
    pub libelle: String,
    /// Des jours ou une quantite (pas de l'argent).
    pub quantite: Option<f64>,
    pub prix: Option<i64>,
    pub montant: i64,
    /// L'avance retenue, pour une ligne `avance` ; le compte de
    /// l'organisme, pour une cotisation ou une charge (vide : celui de
    /// l'affectation `paie:organismes`).
    pub source: Option<String>,
    pub saisie: bool,
}

/// Une avance a retenir : son identifiant, sa date, ce qui peut encore
/// en etre retenu, et ce qu'une paie en retient au plus (D42 : l'accord
/// passe avec l'employe ; vide = tout).
#[derive(Debug, Clone)]
pub struct AvanceARetenir {
    pub id: String,
    pub date: String,
    pub reste: i64,
    pub mensualite: Option<i64>,
}

/// Ce que la base sait de la periode, deja borne aux jours d'emploi.
#[derive(Debug, Clone, Default)]
pub struct Periode {
    /// Les jours du mois civil de la periode : le salaire au mois s'y
    /// mesure.
    pub jours_du_mois: i64,
    /// Les jours de la periode ou la personne etait employee (entree,
    /// depart).
    pub jours_emploi: i64,
    /// Les jours marques, dans les jours d'emploi.
    pub presents: i64,
    pub demis: i64,
    pub absents: i64,
    /// Les ventes signees par son compte pendant son emploi — ou, sur
    /// l'encaisse (D42), l'argent recu sur elles dans la periode ;
    /// `None` : pas de compte lie.
    pub ventes: Option<i64>,
    /// Les retours de la periode sur ces ventes, a la date du retour,
    /// l'echange mis a part (comme le chiffre d'affaires, lot B) — sur
    /// l'encaisse, ce qui a ete rendu (argent ou avoir), pas la dette
    /// eteinte.
    pub retours: i64,
    /// D42 : la commission se calcule sur l'argent recu, pas sur le vendu.
    pub sur_encaisse: bool,
}

/// Ce qui a servi au calcul d'une fiche, fige sur elle (revue du 28/10,
/// point 4 — D31) : une rectificative le reprend tel quel, sans relire
/// la fiche du personnel d'aujourd'hui. Les faits de la periode (jours,
/// ventes, avances, dates d'emploi) se relisent ; les conditions, non.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Conditions {
    #[serde(default)]
    pub remuneration: Remuneration,
    #[serde(default)]
    pub declare: bool,
    /// Le compte dont les ventes font la commission.
    #[serde(default)]
    pub utilisateur_id: Option<String>,
    /// Les cotisations appliquees (vide si non declaree).
    #[serde(default)]
    pub cotisations: Vec<Cotisation>,
    /// D42 : la commission sur l'encaisse. Absent d'une fiche figee
    /// avant D42 : elle a ete calculee sur le vendu, et sa rectificative
    /// aussi — une vente ne se commissionne pas deux fois.
    #[serde(default)]
    pub commission_sur_encaisse: bool,
}

/// `num / den` au franc le plus proche, la moitie s'eloignant de zero
/// (2 500,5 F -> 2 501 F ; -0,5 F -> -1 F). `den` > 0. Calcule en
/// 128 bits : aucun produit intermediaire ne deborde.
pub fn arrondi(num: i128, den: i128) -> i64 {
    debug_assert!(den > 0);
    let q = if num >= 0 { (num + den / 2) / den } else { -((-num + den / 2) / den) };
    q.clamp(i64::MIN as i128, i64::MAX as i128) as i64
}

/// Un pourcentage en millioniemes (2,5 % -> 25 000) : le taux se
/// convertit une fois, l'argent ne passe jamais par un flottant.
pub fn millioniemes(pct: f64) -> i64 {
    (pct * 10_000.0).round() as i64
}

fn pct_dit(p: f64) -> String {
    if p.fract() == 0.0 { format!("{p:.0}") } else { format!("{p}").replace('.', ",") }
}

/// « 21,5 jours », « 1 jour », depuis des demi-journees.
fn demi_jours_dits(demi: i64) -> String {
    let s = if demi % 2 == 0 { format!("{}", demi / 2) } else { format!("{},5", demi / 2) };
    if demi <= 2 { format!("{s} jour") } else { format!("{s} jours") }
}

fn pluriel(n: i64, mot: &str) -> String {
    if n <= 1 { format!("{n} {mot}") } else { format!("{n} {mot}s") }
}

fn calculee(genre: &str, libelle: String, quantite: Option<f64>, prix: Option<i64>, montant: i64) -> Ligne {
    Ligne { genre: genre.to_string(), libelle, quantite, prix, montant, source: None, saisie: false }
}

// ---------------------------------------------------------------------
//  Dates : les bornes d'emploi, le mois civil
// ---------------------------------------------------------------------

fn jour_iso(d: &str) -> &str {
    &d[..d.len().min(10)]
}

fn date(d: &str) -> Result<chrono::NaiveDate, String> {
    chrono::NaiveDate::parse_from_str(jour_iso(d), "%Y-%m-%d").map_err(|_| format!("Date illisible : {d}"))
}

/// « 15/09/2026 ».
pub fn date_dite(iso: &str) -> String {
    let j = jour_iso(iso);
    let p: Vec<&str> = j.split('-').collect();
    if p.len() == 3 { format!("{}/{}/{}", p[2], p[1], p[0]) } else { j.to_string() }
}

/// Les jours d'emploi de la personne dans la periode `du..au` : de la
/// plus tardive de (debut, entree) a la plus tot de (fin, depart).
/// `None` : elle n'y a pas ete employee un seul jour.
pub fn bornes_d_emploi(du: &str, au: &str, entree: Option<&str>, depart: Option<&str>) -> Option<(String, String)> {
    let du = jour_iso(du);
    let au = jour_iso(au);
    let debut = match entree.map(jour_iso) {
        Some(e) if e > du => e,
        _ => du,
    };
    let fin = match depart.map(jour_iso) {
        Some(d) if d < au => d,
        _ => au,
    };
    (debut <= fin).then(|| (debut.to_string(), fin.to_string()))
}

/// Pourquoi une periode est hors de l'emploi de quelqu'un.
pub fn hors_emploi(nom: &str, du: &str, au: &str, entree: Option<&str>, depart: Option<&str>) -> String {
    let periode = format!("du {} au {}", date_dite(du), date_dite(au));
    match (entree, depart) {
        (Some(e), _) if jour_iso(e) > jour_iso(au) => {
            format!("{nom} est entré(e) le {} : la période {periode} est avant son entrée.", date_dite(e))
        }
        (_, Some(d)) if jour_iso(d) < jour_iso(du) => {
            format!("{nom} est parti(e) le {} : la période {periode} est après son départ.", date_dite(d))
        }
        _ => format!("{nom} n'était pas employé(e) {periode}."),
    }
}

/// Le nombre de jours de `du` a `au`, bornes comprises.
pub fn nombre_de_jours(du: &str, au: &str) -> Result<i64, String> {
    Ok((date(au)? - date(du)?).num_days() + 1)
}

/// Les jours du mois civil d'une date.
pub fn jours_du_mois_de(jour: &str) -> Result<i64, String> {
    use chrono::Datelike;
    let d = date(jour)?;
    let premier = d.with_day(1).ok_or("Date hors limites")?;
    let suivant = if d.month() == 12 {
        chrono::NaiveDate::from_ymd_opt(d.year() + 1, 1, 1)
    } else {
        chrono::NaiveDate::from_ymd_opt(d.year(), d.month() + 1, 1)
    }
    .ok_or("Date hors limites")?;
    Ok((suivant - premier).num_days())
}

/// Une personne payee au mois se paie sur un mois civil entier (revue
/// du 28/10, point 8) : deux quinzaines feraient deux salaires, une
/// fiche d'un jour un salaire entier. Une arrivee ou un depart en cours
/// de mois se regle a l'interieur du mois, par les bornes d'emploi.
pub fn verifier_mois_entier(du: &str, au: &str) -> Result<(), String> {
    use chrono::Datelike;
    let d = date(du)?;
    let a = date(au)?;
    let fin_du_mois = d.with_day(jours_du_mois_de(du)? as u32).ok_or("Date hors limites")?;
    if d.day() != 1 || a != fin_du_mois {
        let premier = d.with_day(1).ok_or("Date hors limites")?;
        return Err(format!(
            "Une personne payée au mois se paie sur un mois entier, du 1er au dernier jour (par exemple du {} au {}). \
             Pour une arrivée ou un départ en cours de mois, préparer le mois entier : le salaire se limite à ses jours d'emploi.",
            date_dite(&premier.format("%Y-%m-%d").to_string()),
            date_dite(&fin_du_mois.format("%Y-%m-%d").to_string())
        ));
    }
    Ok(())
}

/// La periode tient-elle dans un seul mois civil ? Une rectificative
/// d'une ancienne fiche (d'avant la regle du mois entier) le demande
/// pour recalculer un salaire au mois.
pub fn verifier_un_seul_mois(du: &str, au: &str) -> Result<(), String> {
    if jour_iso(du).get(..7) != jour_iso(au).get(..7) {
        return Err(format!(
            "Cette fiche s'étend sur deux mois (du {} au {}) : un salaire au mois ne se recalcule que sur un mois. \
             Corriger par une prime ou une retenue sur la fiche suivante.",
            date_dite(du),
            date_dite(au)
        ));
    }
    Ok(())
}

/// Le premier jour ou les ventes d'un compte comptent pour cette
/// personne : son entree, ou le lendemain du depart de qui avait le
/// compte avant elle — un compte de vente repris par quelqu'un d'autre
/// ne commissionne pas deux fois, meme si la date d'entree n'a pas ete
/// saisie (elle est facultative, D30). `departs_des_autres` : les
/// departs des autres personnes liees au meme compte ; seuls comptent
/// ceux d'avant son propre depart.
pub fn debut_du_compte(entree: Option<&str>, depart: Option<&str>, departs_des_autres: &[String]) -> Option<String> {
    let mut debut = entree.map(|e| jour_iso(e).to_string());
    for d in departs_des_autres {
        let d = jour_iso(d);
        if depart.is_some_and(|p| d >= jour_iso(p)) {
            continue;
        }
        let Ok(lendemain) = date(d).map(|x| x + chrono::Duration::days(1)) else { continue };
        let l = lendemain.format("%Y-%m-%d").to_string();
        if debut.as_deref().is_none_or(|x| l.as_str() > x) {
            debut = Some(l);
        }
    }
    debut
}

// ---------------------------------------------------------------------
//  Les gains
// ---------------------------------------------------------------------

/// Les lignes que la remuneration donne pour la periode (sans les
/// saisies ni les avances). Le salaire au mois se borne toujours aux
/// jours d'emploi ; `prorata` retire en plus les absences et les
/// demi-journees marquees (un jour non marque compte comme present).
pub fn gains_calcules(r: &Remuneration, p: &Periode, prorata: bool) -> Vec<Ligne> {
    let mut l = Vec::new();
    if let Some(m) = r.salaire_mensuel {
        let n = p.jours_du_mois.max(1);
        let e = p.jours_emploi.clamp(0, n);
        // En demi-journees.
        let payes = if prorata { (2 * (e - p.absents) - p.demis).max(0) } else { 2 * e };
        let montant = arrondi(m as i128 * payes as i128, 2 * n as i128).min(m);
        let libelle = if prorata {
            let mut details = Vec::new();
            if e < n {
                details.push(format!("{} d'emploi", pluriel(e, "jour")));
            }
            if p.absents > 0 {
                details.push(pluriel(p.absents, "absence"));
            }
            if p.demis > 0 {
                details.push(pluriel(p.demis, "demi-journée"));
            }
            let details = if details.is_empty() { String::new() } else { format!(" ({})", details.join(", ")) };
            format!("Salaire du mois au prorata, {} sur {n}{details}", demi_jours_dits(payes))
        } else if e < n {
            format!("Salaire du mois, {} d'emploi sur {n}", pluriel(e, "jour"))
        } else {
            "Salaire du mois".to_string()
        };
        l.push(calculee(BASE, libelle, Some(payes as f64 / 2.0), Some(m), montant));
    }
    if let Some(t) = r.tarif_journalier {
        let demi = 2 * p.presents + p.demis;
        let montant = arrondi(t as i128 * demi as i128, 2);
        l.push(calculee(JOURS, format!("{} × {}", demi_jours_dits(demi), francs(t)), Some(demi as f64 / 2.0), Some(t), montant));
    }
    if let Some(c) = r.commission_pct {
        match p.ventes {
            Some(v) if p.sur_encaisse => {
                let net = v.saturating_sub(p.retours);
                let assiette = net.max(0);
                let montant = arrondi(assiette as i128 * millioniemes(c) as i128, 1_000_000);
                let libelle = if p.retours == 0 {
                    format!("{} % de {} encaissés sur ses ventes", pct_dit(c), francs(v))
                } else if net >= 0 {
                    format!(
                        "{} % de {} encaissés sur ses ventes ({} reçus moins {} rendus)",
                        pct_dit(c),
                        francs(net),
                        francs(v),
                        francs(p.retours)
                    )
                } else {
                    format!(
                        "{} % de l'encaissé : {} rendus pour {} reçus, pas de commission",
                        pct_dit(c),
                        francs(p.retours),
                        francs(v)
                    )
                };
                l.push(calculee(COMMISSION, libelle, None, Some(assiette), montant));
            }
            Some(v) => {
                let net = v - p.retours;
                let assiette = net.max(0);
                let montant = arrondi(assiette as i128 * millioniemes(c) as i128, 1_000_000);
                let libelle = if p.retours == 0 {
                    format!("{} % de {} de ventes signées", pct_dit(c), francs(v))
                } else if net >= 0 {
                    format!(
                        "{} % de {} de ventes signées ({} moins {} de retours)",
                        pct_dit(c),
                        francs(net),
                        francs(v),
                        francs(p.retours)
                    )
                } else {
                    format!(
                        "{} % des ventes : {} de retours pour {} de ventes signées, pas de commission",
                        pct_dit(c),
                        francs(p.retours),
                        francs(v)
                    )
                };
                l.push(calculee(COMMISSION, libelle, None, Some(assiette), montant));
            }
            None => l.push(calculee(COMMISSION, format!("{} % des ventes : pas de compte lié, aucune vente à compter", pct_dit(c)), None, None, 0)),
        }
    }
    l
}

/// Juge une ligne saisie et la rend prete. Tache : quantite × prix ;
/// prime : un montant ; retenue : un montant, rendu negatif. Montants
/// et quantites bornes (`coeur::saisie`).
pub fn ligne_saisie(genre: &str, libelle: &str, quantite: Option<f64>, prix: Option<i64>, montant: Option<i64>) -> Result<Ligne, String> {
    let libelle = libelle.trim().to_string();
    match genre {
        TACHE => {
            if libelle.is_empty() {
                return Err("Quelle tâche ? (livraisons, déchargement…)".to_string());
            }
            let q = quantite.ok_or("Combien de fois ?")?;
            borner_quantite(q)?;
            let p = prix.ok_or("Combien pour chacune ?")?;
            if p <= 0 {
                return Err("Le prix d'une tâche doit être plus grand que zéro.".to_string());
            }
            borner_montant(p, "le prix d'une tâche")?;
            // La quantite en milliemes : 2,5 livraisons × 1 000 F = 2 500 F.
            let milliemes = (q * 1000.0).round() as i64;
            if milliemes == 0 {
                return Err("La quantité doit être plus grande que zéro.".to_string());
            }
            let m = arrondi(p as i128 * milliemes as i128, 1000);
            borner_montant(m, "cette ligne à la tâche")?;
            Ok(Ligne { genre: TACHE.into(), libelle, quantite: Some(q), prix: Some(p), montant: m, source: None, saisie: true })
        }
        PRIME | RETENUE => {
            let m = montant.ok_or("Quel montant ?")?;
            if m <= 0 {
                return Err("Le montant doit être plus grand que zéro.".to_string());
            }
            borner_montant(m, if genre == PRIME { "la prime" } else { "la retenue" })?;
            let (libelle, montant) = if genre == PRIME {
                (if libelle.is_empty() { "Prime".to_string() } else { libelle }, m)
            } else {
                if libelle.is_empty() {
                    return Err("Une retenue dit pourquoi (casse, absence…).".to_string());
                }
                (libelle, -m)
            };
            Ok(Ligne { genre: genre.into(), libelle, quantite: None, prix: None, montant, source: None, saisie: true })
        }
        _ => Err(format!("« {genre} » ne se saisit pas : prime, tâche ou retenue.")),
    }
}

/// Retient les avances sur ce qui reste, les plus anciennes d'abord,
/// chacune au plus de sa mensualite (D42). Rend les lignes et ce qui se
/// reporte.
pub fn retenir_avances(disponible: i64, avances: &[AvanceARetenir]) -> (Vec<Ligne>, i64) {
    let mut avances: Vec<&AvanceARetenir> = avances.iter().filter(|a| a.reste > 0).collect();
    avances.sort_by(|a, b| a.date.cmp(&b.date).then(a.id.cmp(&b.id)));
    let mut reste_dispo = disponible.max(0);
    let mut lignes = Vec::new();
    let mut reporte: i64 = 0;
    for a in avances {
        let accord = a.mensualite.filter(|m| *m > 0).unwrap_or(a.reste);
        let pris = a.reste.min(reste_dispo).min(accord);
        reste_dispo -= pris;
        reporte = reporte.saturating_add(a.reste - pris);
        if pris == 0 {
            continue;
        }
        let libelle = if pris < a.reste && accord < a.reste {
            format!(
                "Avance du {} ({} ; {} par paie ; {} reportés)",
                date_dite(&a.date),
                francs(a.reste),
                francs(accord),
                francs(a.reste - pris)
            )
        } else if pris < a.reste {
            format!("Avance du {} ({} ; {} reportés)", date_dite(&a.date), francs(a.reste), francs(a.reste - pris))
        } else {
            format!("Avance du {}", date_dite(&a.date))
        };
        lignes.push(Ligne { genre: AVANCE.into(), libelle, quantite: None, prix: None, montant: -pris, source: Some(a.id.clone()), saisie: false });
    }
    (lignes, reporte)
}

/// Une cotisation reglee pour le dossier (D33) : jamais devinee, vide
/// par defaut, appliquee aux seules personnes declarees.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cotisation {
    #[serde(default)]
    pub id: String,
    pub libelle: String,
    /// `salarie` (retenue sur le salaire) ou `employeur` (charge).
    pub qui: String,
    /// Un pourcentage, entre 0 et 100.
    pub taux: f64,
    /// La base est le brut, plafonne si `plafond` est donne.
    #[serde(default)]
    pub plafond: Option<i64>,
    /// Le compte de l'organisme (431, 447…) ; vide : l'affectation.
    #[serde(default)]
    pub compte: Option<String>,
}

/// Juge une cotisation reglee.
pub fn valider_cotisation(c: &Cotisation) -> Result<(), String> {
    if c.libelle.trim().is_empty() {
        return Err("Une cotisation a un nom (INPS part salariale, AMO…).".to_string());
    }
    if c.qui != "salarie" && c.qui != "employeur" {
        return Err("Qui paie la cotisation : le salarié ou l'employeur ?".to_string());
    }
    if !c.taux.is_finite() || c.taux <= 0.0 || c.taux > 100.0 {
        return Err("Le taux est un pourcentage entre 0 et 100.".to_string());
    }
    if c.plafond.is_some_and(|p| p <= 0) {
        return Err("Le plafond doit être plus grand que zéro — ou vide.".to_string());
    }
    if let Some(p) = c.plafond {
        borner_montant(p, "le plafond")?;
    }
    if let Some(n) = c.compte.as_deref().filter(|n| !n.trim().is_empty()) {
        if !(n.starts_with("43") || n.starts_with("44")) {
            return Err(format!("Une cotisation se doit à un organisme : un compte qui commence par 43 ou 44, pas {n}."));
        }
    }
    Ok(())
}

/// Les lignes des cotisations sur un brut : une retenue par cotisation
/// salariale, une charge par cotisation patronale.
pub fn cotisations(brut: i64, liste: &[Cotisation]) -> Vec<Ligne> {
    let mut l = Vec::new();
    for c in liste {
        let assiette = c.plafond.map_or(brut, |p| brut.min(p)).max(0);
        let m = arrondi(assiette as i128 * millioniemes(c.taux) as i128, 1_000_000);
        if m == 0 {
            continue;
        }
        let plafonne = if c.plafond.is_some_and(|p| brut > p) { " (plafonné)" } else { "" };
        let libelle = format!("{} — {} % de {}{plafonne}", c.libelle.trim(), pct_dit(c.taux), francs(assiette));
        let (genre, montant) = if c.qui == "salarie" { (COTISATION, -m) } else { (CHARGE, m) };
        let compte = c.compte.clone().filter(|n| !n.trim().is_empty());
        l.push(Ligne { genre: genre.into(), libelle, quantite: None, prix: Some(assiette), montant, source: compte, saisie: false });
    }
    l
}

/// La fiche entiere : gains calcules, saisies (gains puis retenues),
/// avances retenues. Rend les lignes dans l'ordre du bulletin et ce qui
/// se reporte. Refuse une fiche dont les sommes debordent.
///
/// `cotisations` : celles des conditions de la fiche, a ne passer que
/// pour une personne declaree (D33) — elles se calculent sur le brut,
/// avant les retenues saisies ; les charges patronales viennent en
/// dernier, hors du net.
pub fn calculer(
    r: &Remuneration,
    p: &Periode,
    prorata: bool,
    saisies: &[Ligne],
    avances: &[AvanceARetenir],
    cotisations_du_dossier: &[Cotisation],
) -> Result<(Vec<Ligne>, i64), String> {
    let mut lignes = gains_calcules(r, p, prorata);
    lignes.extend(saisies.iter().filter(|l| l.montant >= 0).cloned());
    let brut = somme(lignes.iter().map(|l| l.montant))?;
    let cot = cotisations(brut, cotisations_du_dossier);
    lignes.extend(cot.iter().filter(|l| l.genre == COTISATION).cloned());
    lignes.extend(saisies.iter().filter(|l| l.montant < 0).cloned());
    let avant_avances = somme(lignes.iter().map(|l| l.montant))?;
    let (retenues, reporte) = retenir_avances(avant_avances, avances);
    lignes.extend(retenues);
    lignes.extend(cot.into_iter().filter(|l| l.genre == CHARGE));
    totaux(&lignes)?;
    Ok((lignes, reporte))
}

/// Brut (les gains), retenues (en positif, avances et cotisations
/// comprises), net. Les charges patronales n'y entrent pas : la
/// societe les paie en plus, pas la personne. Sommes verifiees.
pub fn totaux(lignes: &[Ligne]) -> Result<(i64, i64, i64), String> {
    let paie = || lignes.iter().filter(|l| l.genre != CHARGE);
    let brut = somme(paie().filter(|l| l.montant > 0).map(|l| l.montant))?;
    let retenues = somme(paie().filter(|l| l.montant < 0).map(|l| l.montant.saturating_neg()))?;
    let net = brut.checked_sub(retenues).ok_or("Montants hors limites.")?;
    Ok((brut, retenues, net))
}

/// Les charges patronales d'une fiche (hors du net).
pub fn charges(lignes: &[Ligne]) -> i64 {
    lignes.iter().filter(|l| l.genre == CHARGE).map(|l| l.montant).fold(0i64, i64::saturating_add)
}

/// Peut-on valider ? Un net negatif veut dire que les retenues saisies
/// depassent ce que la personne a gagne.
pub fn validable(lignes: &[Ligne]) -> Result<(), String> {
    let (brut, retenues, net) = totaux(lignes)?;
    if net < 0 {
        return Err(format!(
            "Les retenues ({}) dépassent ce que la personne a gagné ({}) : corriger avant de valider.",
            francs(retenues),
            francs(brut)
        ));
    }
    Ok(())
}

/// Le CA mensuel et le cout des salaires, mois par mois (Rapports ->
/// CA mensuel) : chaque mois des ventes recoit `salaires` et
/// `apres_salaires` ; un mois de salaires sans vente apparait aussi.
/// Les mois restent du plus recent au plus ancien.
pub fn avec_salaires(mut mois: Vec<serde_json::Value>, salaires: &std::collections::HashMap<String, i64>) -> Vec<serde_json::Value> {
    for m in salaires.keys() {
        if !mois.iter().any(|x| x["mois"] == m.as_str()) {
            mois.push(serde_json::json!({ "mois": m, "ca": 0, "nb_ventes": 0, "encaisse": 0 }));
        }
    }
    for x in &mut mois {
        let s = x["mois"].as_str().and_then(|m| salaires.get(m)).copied().unwrap_or(0);
        let ca = x["ca"].as_i64().unwrap_or(0);
        x["salaires"] = s.into();
        x["apres_salaires"] = ca.saturating_sub(s).into();
    }
    mois.sort_by(|a, b| b["mois"].as_str().cmp(&a["mois"].as_str()));
    mois
}

/// Peut-on verser ce montant ? `reste` : ce qui reste du sur la fiche.
pub fn verifier_versement(montant: i64, reste: i64, nom: &str) -> Result<(), String> {
    if montant <= 0 {
        return Err("Le montant d'un versement doit être plus grand que zéro.".to_string());
    }
    borner_montant(montant, "le versement")?;
    if reste <= 0 {
        return Err(format!("{nom} a déjà tout reçu pour cette fiche."));
    }
    if montant > reste {
        return Err(format!("Il ne reste que {} à verser à {nom}.", francs(reste)));
    }
    Ok(())
}

/// Ce qui reste a verser sur une fiche : le net, moins ce qui a ete
/// verse, plus ce qui a ete verse en trop et repris en avance (cet
/// argent la, la personne le rend sur sa fiche suivante : il ne compte
/// plus comme paye ici). Negatif : un trop-verse pas (ou plus) repris.
pub fn reste_a_verser(net: i64, verse: i64, repris_en_avance: i64) -> i64 {
    net.saturating_sub(verse).saturating_add(repris_en_avance)
}

/// Ce qui doit devenir une avance a la validation d'une rectificative
/// (revue du 28/10, point 6 — D32) : ce qui a ete verse au-dela du net,
/// moins ce qui est deja repris. Zero s'il n'y a rien a reprendre.
pub fn trop_verse_a_reprendre(net: i64, verse: i64, deja_repris: i64) -> i64 {
    reste_a_verser(net, verse, deja_repris).min(0).saturating_neg()
}

/// Deux periodes se chevauchent-elles ? Dates ISO `AAAA-MM-JJ`.
pub fn se_chevauchent(du1: &str, au1: &str, du2: &str, au2: &str) -> bool {
    du1 <= au2 && du2 <= au1
}

/// Juge une periode : deux dates ISO, dans l'ordre, pas plus d'un an.
pub fn verifier_periode(du: &str, au: &str) -> Result<(), String> {
    let d = chrono::NaiveDate::parse_from_str(du, "%Y-%m-%d").map_err(|_| format!("Date de début illisible : {du}"))?;
    let a = chrono::NaiveDate::parse_from_str(au, "%Y-%m-%d").map_err(|_| format!("Date de fin illisible : {au}"))?;
    if a < d {
        return Err("La période finit avant de commencer.".to_string());
    }
    if (a - d).num_days() > 366 {
        return Err("Une fiche de paie couvre au plus un an.".to_string());
    }
    Ok(())
}

// ---------------------------------------------------------------------
//  Les conditions d'une ancienne fiche (d'avant le 28/10)
// ---------------------------------------------------------------------

/// Le pourcentage en tete d'un libelle (« 2,5 % de … » -> 2,5).
fn pct_en_tete(texte: &str) -> Option<f64> {
    let (avant, _) = texte.split_once(" % ")?;
    avant.trim().replace(',', ".").parse::<f64>().ok().filter(|p| p.is_finite() && *p > 0.0 && *p <= 100.0)
}

/// Les conditions d'une fiche validee avant qu'elles ne soient figees
/// (revue du 28/10, point 4) : on les **relit sur ses lignes**, le choix
/// le plus prudent — ce sont les lignes qu'on a remises a la personne.
/// Le salaire et le tarif sont le `prix` des lignes `base` et `jours` ;
/// le taux de commission et ceux des cotisations, le pourcentage en
/// tete de leur libelle (« 3 % de … », « INPS — 3,6 % de … ») ; une
/// cotisation « (plafonné) » avait pour plafond son assiette. Ce qui ne
/// se relit pas est refuse plutot que devine. `utilisateur_id` : le
/// compte lie aujourd'hui (l'ancienne fiche ne le disait pas).
pub fn conditions_depuis_lignes(lignes: &[Ligne], utilisateur_id: Option<String>) -> Result<Conditions, String> {
    let mut c = Conditions { utilisateur_id, ..Default::default() };
    for l in lignes {
        match l.genre.as_str() {
            BASE => c.remuneration.salaire_mensuel = l.prix,
            JOURS => c.remuneration.tarif_journalier = l.prix,
            COMMISSION => {
                c.remuneration.commission_pct = Some(pct_en_tete(&l.libelle).ok_or_else(|| {
                    format!("Le taux de commission de l'ancienne fiche ne se relit pas (« {} ») : corriger par une prime ou une retenue.", l.libelle)
                })?)
            }
            TACHE => c.remuneration.a_la_tache = true,
            COTISATION | CHARGE => {
                c.declare = true;
                let (nom, reste) = l.libelle.rsplit_once(" — ").ok_or_else(|| format!("La cotisation « {} » ne se relit pas.", l.libelle))?;
                let taux = pct_en_tete(reste).ok_or_else(|| format!("Le taux de « {} » ne se relit pas.", l.libelle))?;
                c.cotisations.push(Cotisation {
                    id: String::new(),
                    libelle: nom.trim().to_string(),
                    qui: if l.genre == COTISATION { "salarie" } else { "employeur" }.to_string(),
                    taux,
                    plafond: if l.libelle.ends_with("(plafonné)") { l.prix } else { None },
                    compte: l.source.clone(),
                });
            }
            _ => {}
        }
    }
    Ok(c)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coeur::saisie::MONTANT_MAX;

    fn r(m: Option<i64>, j: Option<i64>, c: Option<f64>) -> Remuneration {
        Remuneration { salaire_mensuel: m, tarif_journalier: j, commission_pct: c, a_la_tache: false }
    }

    /// Un mois de `n` jours, `e` d'emploi, et des marques.
    fn mois(n: i64, e: i64, presents: i64, demis: i64, absents: i64) -> Periode {
        Periode { jours_du_mois: n, jours_emploi: e, presents, demis, absents, ventes: None, retours: 0, sur_encaisse: false }
    }

    #[test]
    fn l_arrondi_au_franc_est_defini() {
        assert_eq!(arrondi(5, 2), 3, "2,5 -> 3 : la moitié vers le haut");
        assert_eq!(arrondi(4, 2), 2);
        assert_eq!(arrondi(7, 3), 2, "2,33 -> 2");
        assert_eq!(arrondi(-5, 2), -3, "symétrique");
        assert_eq!(arrondi(0, 7), 0);
        assert_eq!(millioniemes(2.5), 25_000);
        assert_eq!(millioniemes(3.6), 36_000);
    }

    #[test]
    fn chaque_mode_donne_sa_ligne() {
        let p = Periode { ventes: Some(250_000), ..mois(30, 30, 22, 0, 0) };
        let l = gains_calcules(&r(Some(60_000), None, None), &p, false);
        assert_eq!((l[0].montant, l[0].libelle.as_str()), (60_000, "Salaire du mois"));
        let l = gains_calcules(&r(None, Some(2_500), None), &Periode { demis: 1, ..mois(30, 30, 21, 1, 0) }, false);
        assert_eq!((l[0].montant, l[0].libelle.as_str()), (53_750, "21,5 jours × 2 500 F"));
        let l = gains_calcules(&r(Some(25_000), None, Some(2.5)), &p, false);
        assert_eq!(l.len(), 2, "les modes s'ajoutent");
        assert_eq!((l[1].montant, l[1].libelle.as_str()), (6_250, "2,5 % de 250 000 F de ventes signées"));
        let l = gains_calcules(&r(None, None, Some(3.0)), &mois(30, 30, 0, 0, 0), false);
        assert_eq!(l[0].montant, 0);
        assert!(l[0].libelle.contains("pas de compte lié"));
        assert!(gains_calcules(&Remuneration::default(), &Periode::default(), false).is_empty(), "rien de fixe");
    }

    #[test]
    fn un_jour_a_la_journee_arrondi_au_franc() {
        // 2 501 F × 0,5 = 1 250,5 F -> 1 251 F (la moitié vers le haut).
        let l = gains_calcules(&r(None, Some(2_501), None), &mois(30, 30, 0, 1, 0), false);
        assert_eq!((l[0].montant, l[0].libelle.as_str()), (1_251, "0,5 jour × 2 501 F"));
    }

    #[test]
    fn le_salaire_au_mois_se_borne_aux_jours_d_emploi_meme_sans_prorata() {
        // Bakary, 60 000 F, parti le 15/09 : 15 jours sur 30.
        let l = gains_calcules(&r(Some(60_000), None, None), &mois(30, 15, 0, 0, 0), false);
        assert_eq!((l[0].montant, l[0].libelle.as_str()), (30_000, "Salaire du mois, 15 jours d'emploi sur 30"));
        // Issa, 40 000 F, entré le 15/10 : 17 jours sur 31 -> 21 935,48 -> 21 935.
        let l = gains_calcules(&r(Some(40_000), None, None), &mois(31, 17, 0, 0, 0), false);
        assert_eq!(l[0].montant, 21_935);
        // Sans prorata, les absences ne retirent rien.
        let l = gains_calcules(&r(Some(60_000), None, None), &mois(30, 30, 0, 0, 2), false);
        assert_eq!(l[0].montant, 60_000);
    }

    #[test]
    fn le_prorata_compte_un_jour_non_marque_comme_present() {
        // Deux absences marquées en septembre, rien d'autre : 28/30.
        let l = gains_calcules(&r(Some(60_000), None, None), &mois(30, 30, 0, 0, 2), true);
        assert_eq!((l[0].montant, l[0].libelle.as_str()), (56_000, "Salaire du mois au prorata, 28 jours sur 30 (2 absences)"));
        // Awa, 75 000 F, entrée le 15/02/2026 (28 jours), 14 présents : 14/28.
        let l = gains_calcules(&r(Some(75_000), None, None), &mois(28, 14, 14, 0, 0), true);
        assert_eq!((l[0].montant, l[0].libelle.as_str()), (37_500, "Salaire du mois au prorata, 14 jours sur 28 (14 jours d'emploi)"));
        // Rien de marqué : le mois entier.
        let l = gains_calcules(&r(Some(60_000), None, None), &mois(30, 30, 0, 0, 0), true);
        assert_eq!((l[0].montant, l[0].libelle.as_str()), (60_000, "Salaire du mois au prorata, 30 jours sur 30"));
        // Une demi-journée retire la moitié d'un jour : 29,5 / 30.
        let l = gains_calcules(&r(Some(60_000), None, None), &mois(30, 30, 0, 1, 0), true);
        assert_eq!((l[0].montant, l[0].libelle.as_str()), (59_000, "Salaire du mois au prorata, 29,5 jours sur 30 (1 demi-journée)"));
        // Jamais plus que le salaire, jamais moins que zéro.
        let l = gains_calcules(&r(Some(60_000), None, None), &mois(30, 30, 0, 0, 30), true);
        assert_eq!(l[0].montant, 0);
    }

    #[test]
    fn la_commission_sur_l_encaisse_dit_ce_qui_est_recu_et_rendu() {
        // D42 : 50 000 F recus, 5 000 F rendus.
        let p = Periode { ventes: Some(50_000), retours: 5_000, sur_encaisse: true, ..mois(30, 30, 0, 0, 0) };
        let l = gains_calcules(&r(None, None, Some(3.0)), &p, false);
        assert_eq!(l[0].montant, 1_350);
        assert_eq!(l[0].libelle, "3 % de 45 000 F encaissés sur ses ventes (50 000 F reçus moins 5 000 F rendus)");
        let p = Periode { ventes: Some(0), sur_encaisse: true, ..mois(30, 30, 0, 0, 0) };
        let l = gains_calcules(&r(None, None, Some(3.0)), &p, false);
        assert_eq!((l[0].montant, l[0].libelle.as_str()), (0, "3 % de 0 F encaissés sur ses ventes"));
        let p = Periode { ventes: Some(2_000), retours: 5_000, sur_encaisse: true, ..mois(30, 30, 0, 0, 0) };
        let l = gains_calcules(&r(None, None, Some(3.0)), &p, false);
        assert_eq!(l[0].montant, 0, "jamais négative");
        // Le taux se relit sur la ligne d'une ancienne fiche.
        assert_eq!(conditions_depuis_lignes(&l, None).unwrap().remuneration.commission_pct, Some(3.0));
    }

    #[test]
    fn la_commission_deduit_les_retours_de_la_periode() {
        let p = Periode { ventes: Some(250_000), retours: 50_000, ..mois(30, 30, 0, 0, 0) };
        let l = gains_calcules(&r(None, None, Some(3.0)), &p, false);
        assert_eq!(l[0].montant, 6_000, "3 % de 200 000");
        assert_eq!(l[0].libelle, "3 % de 200 000 F de ventes signées (250 000 F moins 50 000 F de retours)");
        // Plus de retours que de ventes : pas de commission, jamais négative.
        let p = Periode { ventes: Some(10_000), retours: 30_000, ..mois(30, 30, 0, 0, 0) };
        let l = gains_calcules(&r(None, None, Some(3.0)), &p, false);
        assert_eq!(l[0].montant, 0);
        assert!(l[0].libelle.contains("pas de commission"));
    }

    #[test]
    fn les_bornes_d_emploi() {
        let b = |e: Option<&str>, d: Option<&str>| bornes_d_emploi("2026-09-01", "2026-09-30", e, d);
        assert_eq!(b(None, None), Some(("2026-09-01".into(), "2026-09-30".into())));
        assert_eq!(b(None, Some("2026-09-15")), Some(("2026-09-01".into(), "2026-09-15".into())));
        assert_eq!(b(Some("2026-10-15"), None), None, "entré après la période");
        assert_eq!(b(None, Some("2026-08-31")), None, "parti avant la période");
        assert_eq!(b(Some("2026-09-30"), Some("2026-09-30")), Some(("2026-09-30".into(), "2026-09-30".into())));
        assert!(hors_emploi("Issa", "2026-09-01", "2026-09-30", Some("2026-10-15"), None).contains("avant son entrée"));
        assert!(hors_emploi("Bakary", "2026-10-01", "2026-10-31", None, Some("2026-09-15")).contains("après son départ"));
        assert_eq!(nombre_de_jours("2026-09-01", "2026-09-15"), Ok(15));
        assert_eq!(jours_du_mois_de("2026-02-15"), Ok(28));
        assert_eq!(jours_du_mois_de("2028-02-01"), Ok(29));
        assert_eq!(jours_du_mois_de("2026-12-31"), Ok(31));
    }

    #[test]
    fn au_mois_on_se_paie_sur_un_mois_civil_entier() {
        assert!(verifier_mois_entier("2026-09-01", "2026-09-30").is_ok());
        assert!(verifier_mois_entier("2026-02-01", "2026-02-28").is_ok());
        let e = verifier_mois_entier("2026-09-01", "2026-09-15").unwrap_err();
        assert!(e.contains("mois entier") && e.contains("du 01/09/2026 au 30/09/2026"), "{e}");
        assert!(verifier_mois_entier("2026-09-16", "2026-09-30").is_err(), "la seconde quinzaine");
        assert!(verifier_mois_entier("2026-09-10", "2026-09-10").is_err(), "un jour");
        assert!(verifier_mois_entier("2026-09-01", "2026-10-31").is_err(), "deux mois");
        assert!(verifier_un_seul_mois("2026-09-01", "2026-09-15").is_ok());
        assert!(verifier_un_seul_mois("2026-09-15", "2026-10-14").is_err());
    }

    #[test]
    fn un_compte_repris_ne_commissionne_pas_deux_fois() {
        // A avait le compte et est partie le 15/09 ; B l'a repris, sans date d'entrée.
        let departs = vec!["2026-09-15".to_string()];
        assert_eq!(debut_du_compte(None, None, &departs), Some("2026-09-16".into()));
        // B entrée le 20/09 : sa propre entrée l'emporte.
        assert_eq!(debut_du_compte(Some("2026-09-20"), None, &departs), Some("2026-09-20".into()));
        // A elle-même, partie le 15/09, ne se retire pas les ventes d'une
        // personne partie après elle.
        assert_eq!(debut_du_compte(None, Some("2026-09-15"), &["2026-12-31".to_string()]), None);
    }

    #[test]
    fn les_saisies_se_jugent() {
        assert_eq!(ligne_saisie(TACHE, "Livraisons", Some(3.0), Some(1_000), None).unwrap().montant, 3_000);
        assert_eq!(ligne_saisie(TACHE, "Heures", Some(2.5), Some(1_001), None).unwrap().montant, 2_503, "2 502,5 -> 2 503");
        assert!(ligne_saisie(TACHE, "", Some(3.0), Some(1_000), None).is_err());
        assert_eq!(ligne_saisie(PRIME, "", None, None, Some(5_000)).unwrap().libelle, "Prime");
        let casse = ligne_saisie(RETENUE, "Casse d'un carton", None, None, Some(2_000)).unwrap();
        assert_eq!(casse.montant, -2_000);
        assert!(ligne_saisie(RETENUE, "", None, None, Some(2_000)).unwrap_err().contains("pourquoi"));
        assert!(ligne_saisie(PRIME, "x", None, None, Some(0)).is_err());
        assert!(ligne_saisie(AVANCE, "x", None, None, Some(1)).is_err(), "une avance ne se saisit pas ici");
    }

    #[test]
    fn une_saisie_enorme_est_refusee() {
        assert!(ligne_saisie(PRIME, "x", None, None, Some(i64::MAX)).unwrap_err().contains("trop grand"));
        assert!(ligne_saisie(RETENUE, "Casse", None, None, Some(MONTANT_MAX + 1)).unwrap_err().contains("trop grand"));
        assert!(ligne_saisie(TACHE, "Livraisons", Some(1e300), Some(1_000), None).unwrap_err().contains("Quantité trop grande"));
        assert!(ligne_saisie(TACHE, "Livraisons", Some(100_000.0), Some(MONTANT_MAX), None).unwrap_err().contains("trop grand"));
        assert!(ligne_saisie(TACHE, "Livraisons", Some(0.0001), Some(1_000), None).is_err());
        assert!(verifier_versement(i64::MAX, i64::MAX, "Awa").unwrap_err().contains("trop grand"));
    }

    #[test]
    fn les_avances_se_retiennent_les_plus_anciennes_d_abord_et_le_reste_se_reporte() {
        let av = vec![
            AvanceARetenir { id: "b".into(), date: "2026-09-20".into(), reste: 30_000, mensualite: None },
            AvanceARetenir { id: "a".into(), date: "2026-09-05".into(), reste: 10_000, mensualite: None },
        ];
        let (l, reporte) = retenir_avances(25_000, &av);
        assert_eq!(l.len(), 2);
        assert_eq!((l[0].source.as_deref(), l[0].montant), (Some("a"), -10_000));
        assert_eq!((l[1].source.as_deref(), l[1].montant), (Some("b"), -15_000));
        assert_eq!(l[1].libelle, "Avance du 20/09/2026 (30 000 F ; 15 000 F reportés)");
        assert_eq!(reporte, 15_000);
        let (l, reporte) = retenir_avances(0, &av);
        assert!(l.is_empty());
        assert_eq!(reporte, 40_000, "rien gagné : tout se reporte");
    }

    #[test]
    fn une_avance_etalee_ne_retient_que_sa_mensualite() {
        // D42 : 30 000 F a rembourser 10 000 F par paie, 60 000 F de net.
        let av = vec![AvanceARetenir { id: "a".into(), date: "2026-09-05".into(), reste: 30_000, mensualite: Some(10_000) }];
        let (l, reporte) = retenir_avances(60_000, &av);
        assert_eq!(l[0].montant, -10_000);
        assert_eq!(l[0].libelle, "Avance du 05/09/2026 (30 000 F ; 10 000 F par paie ; 20 000 F reportés)");
        assert_eq!(reporte, 20_000);
        // Le net plus petit que la mensualite : le net borne toujours.
        let (l, reporte) = retenir_avances(4_000, &av);
        assert_eq!((l[0].montant, reporte), (-4_000, 26_000));
        // La derniere paie : il ne reste que 5 000 F, ils partent tous.
        let fin = vec![AvanceARetenir { id: "a".into(), date: "2026-09-05".into(), reste: 5_000, mensualite: Some(10_000) }];
        let (l, reporte) = retenir_avances(60_000, &fin);
        assert_eq!((l[0].montant, l[0].libelle.as_str(), reporte), (-5_000, "Avance du 05/09/2026", 0));
    }

    #[test]
    fn la_fiche_entiere() {
        let p = mois(30, 30, 0, 0, 0);
        let saisies = vec![
            ligne_saisie(RETENUE, "Casse", None, None, Some(2_000)).unwrap(),
            ligne_saisie(PRIME, "Fin d'année", None, None, Some(5_000)).unwrap(),
        ];
        let av = vec![AvanceARetenir { id: "a".into(), date: "2026-09-05".into(), reste: 10_000, mensualite: None }];
        let (l, reporte) = calculer(&r(Some(60_000), None, None), &p, false, &saisies, &av, &[]).unwrap();
        let genres: Vec<&str> = l.iter().map(|x| x.genre.as_str()).collect();
        assert_eq!(genres, vec![BASE, PRIME, RETENUE, AVANCE], "gains, puis retenues, puis avances");
        assert_eq!(totaux(&l), Ok((65_000, 12_000, 53_000)));
        assert_eq!(reporte, 0);
        assert!(validable(&l).is_ok());
        let trop = vec![ligne_saisie(RETENUE, "Casse", None, None, Some(70_000)).unwrap()];
        let (l, _) = calculer(&r(Some(60_000), None, None), &p, false, &trop, &av, &[]).unwrap();
        assert_eq!(l.iter().filter(|x| x.genre == AVANCE).count(), 0, "rien à retenir");
        assert!(validable(&l).unwrap_err().contains("dépassent"));
    }

    #[test]
    fn des_lignes_qui_debordent_sont_refusees_sans_paniquer() {
        let enorme = Ligne { genre: PRIME.into(), libelle: "x".into(), quantite: None, prix: None, montant: i64::MAX, source: None, saisie: true };
        let p = mois(30, 30, 0, 0, 0);
        assert!(calculer(&r(Some(60_000), None, None), &p, false, &[enorme.clone()], &[], &[]).unwrap_err().contains("hors limites"));
        assert!(totaux(&[enorme.clone(), enorme]).is_err());
    }

    #[test]
    fn un_versement_ne_depasse_pas_ce_qui_reste() {
        assert!(verifier_versement(20_000, 50_000, "Awa").is_ok());
        assert!(verifier_versement(50_000, 50_000, "Awa").is_ok());
        assert_eq!(verifier_versement(60_000, 50_000, "Awa").unwrap_err(), "Il ne reste que 50 000 F à verser à Awa.");
        assert!(verifier_versement(1, 0, "Awa").unwrap_err().contains("déjà tout reçu"));
        assert!(verifier_versement(0, 10, "Awa").is_err());
    }

    #[test]
    fn le_trop_verse_devient_une_avance_une_seule_fois() {
        // Septembre 60 000 versés, la rectificative dit 52 000 : 8 000 à reprendre.
        assert_eq!(trop_verse_a_reprendre(52_000, 60_000, 0), 8_000);
        assert_eq!(reste_a_verser(52_000, 60_000, 8_000), 0, "repris : la fiche est payée");
        // Une seconde rectificative au même net : rien de plus.
        assert_eq!(trop_verse_a_reprendre(52_000, 60_000, 8_000), 0);
        // Elle remonte à 60 000 : la personne a de nouveau 8 000 à recevoir
        // (l'avance, elle, reste à retenir).
        assert_eq!(reste_a_verser(60_000, 60_000, 8_000), 8_000);
        assert_eq!(trop_verse_a_reprendre(60_000, 60_000, 8_000), 0);
        assert_eq!(trop_verse_a_reprendre(60_000, 20_000, 0), 0, "pas de trop-versé");
    }

    fn cot(libelle: &str, qui: &str, taux: f64, plafond: Option<i64>) -> Cotisation {
        Cotisation { id: libelle.into(), libelle: libelle.into(), qui: qui.into(), taux, plafond, compte: None }
    }

    #[test]
    fn les_cotisations_d_un_declare() {
        let liste = vec![cot("INPS salarié", "salarie", 3.6, None), cot("INPS employeur", "employeur", 16.4, Some(50_000))];
        let p = mois(30, 30, 0, 0, 0);
        let (l, _) = calculer(&r(Some(60_000), None, None), &p, false, &[ligne_saisie(RETENUE, "Casse", None, None, Some(1_000)).unwrap()], &[], &liste).unwrap();
        let genres: Vec<&str> = l.iter().map(|x| x.genre.as_str()).collect();
        assert_eq!(genres, vec![BASE, COTISATION, RETENUE, CHARGE]);
        assert_eq!(l[1].montant, -2_160, "3,6 % de 60 000");
        assert_eq!(l[1].libelle, "INPS salarié — 3,6 % de 60 000 F");
        assert_eq!(l[3].montant, 8_200, "16,4 % de 50 000, plafonné");
        assert!(l[3].libelle.ends_with("(plafonné)"));
        assert_eq!(totaux(&l), Ok((60_000, 3_160, 56_840)), "la charge patronale n'entre pas dans le net");
        assert_eq!(charges(&l), 8_200);
    }

    #[test]
    fn une_cotisation_se_juge() {
        assert!(valider_cotisation(&cot("INPS", "salarie", 3.6, None)).is_ok());
        assert!(valider_cotisation(&cot("", "salarie", 3.6, None)).is_err());
        assert!(valider_cotisation(&cot("INPS", "patron", 3.6, None)).is_err());
        assert!(valider_cotisation(&cot("INPS", "salarie", 0.0, None)).is_err());
        assert!(valider_cotisation(&cot("INPS", "salarie", 3.6, Some(0))).is_err());
        assert!(valider_cotisation(&cot("INPS", "salarie", 3.6, Some(MONTANT_MAX + 1))).unwrap_err().contains("trop grand"));
        let mut c = cot("ITS", "salarie", 5.0, None);
        c.compte = Some("661".into());
        assert!(valider_cotisation(&c).unwrap_err().contains("43 ou 44"));
        c.compte = Some("447".into());
        assert!(valider_cotisation(&c).is_ok());
    }

    #[test]
    fn les_conditions_d_une_ancienne_fiche_se_relisent_sur_ses_lignes() {
        let mut c447 = cot("ITS", "salarie", 5.0, None);
        c447.compte = Some("447".into());
        let liste = vec![cot("INPS salarié", "salarie", 3.6, None), cot("INPS employeur", "employeur", 16.4, Some(50_000)), c447];
        let p = Periode { ventes: Some(100_000), ..mois(30, 30, 20, 1, 0) };
        let remu = Remuneration { salaire_mensuel: Some(60_000), tarif_journalier: Some(1_500), commission_pct: Some(2.5), a_la_tache: false };
        let (lignes, _) = calculer(&remu, &p, false, &[], &[], &liste).unwrap();
        let c = conditions_depuis_lignes(&lignes, Some("u1".into())).unwrap();
        assert_eq!(c.remuneration.salaire_mensuel, Some(60_000));
        assert_eq!(c.remuneration.tarif_journalier, Some(1_500));
        assert_eq!(c.remuneration.commission_pct, Some(2.5));
        assert!(c.declare);
        assert_eq!(c.utilisateur_id.as_deref(), Some("u1"));
        assert_eq!(c.cotisations.len(), 3);
        assert_eq!((c.cotisations[0].libelle.as_str(), c.cotisations[0].taux, c.cotisations[0].plafond), ("INPS salarié", 3.6, None));
        assert_eq!((c.cotisations[2].qui.as_str(), c.cotisations[2].plafond), ("employeur", Some(50_000)), "plafonné : l'assiette");
        assert_eq!(c.cotisations[1].compte.as_deref(), Some("447"));
        // Recalculer avec ce qui a été relu redonne les mêmes lignes.
        let (encore, _) = calculer(&c.remuneration, &p, false, &[], &[], &c.cotisations).unwrap();
        assert_eq!(encore, lignes);
        // Un libellé qu'on ne sait pas relire : refusé, pas deviné.
        let mut l = lignes.clone();
        l[2].libelle = "commission".into();
        assert!(conditions_depuis_lignes(&l, None).unwrap_err().contains("ne se relit pas"));
    }

    #[test]
    fn le_ca_mensuel_recoit_les_salaires() {
        let ventes = vec![
            serde_json::json!({ "mois": "2026-09", "ca": 500_000, "nb_ventes": 10, "encaisse": 400_000 }),
            serde_json::json!({ "mois": "2026-07", "ca": 300_000, "nb_ventes": 5, "encaisse": 300_000 }),
        ];
        let salaires: std::collections::HashMap<String, i64> = [("2026-09".to_string(), 180_000), ("2026-08".to_string(), 90_000)].into();
        let m = avec_salaires(ventes, &salaires);
        let mois: Vec<&str> = m.iter().map(|x| x["mois"].as_str().unwrap()).collect();
        assert_eq!(mois, vec!["2026-09", "2026-08", "2026-07"], "un mois de salaires sans vente apparaît, dans l'ordre");
        assert_eq!((m[0]["salaires"].as_i64(), m[0]["apres_salaires"].as_i64()), (Some(180_000), Some(320_000)));
        assert_eq!((m[1]["ca"].as_i64(), m[1]["apres_salaires"].as_i64()), (Some(0), Some(-90_000)));
        assert_eq!(m[2]["salaires"], 0);
    }

    #[test]
    fn les_periodes() {
        assert!(se_chevauchent("2026-09-01", "2026-09-30", "2026-09-15", "2026-10-15"));
        assert!(!se_chevauchent("2026-09-01", "2026-09-15", "2026-09-16", "2026-09-30"));
        assert!(verifier_periode("2026-09-01", "2026-09-30").is_ok());
        assert!(verifier_periode("2026-09-30", "2026-09-01").is_err());
        assert!(verifier_periode("2026-09", "2026-09-30").is_err());
    }
}
