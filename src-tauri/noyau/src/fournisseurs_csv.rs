//! D44 — les fournisseurs en CSV, et leurs doublons.
//!
//! Meme format que le catalogue (`catalogue_csv`) : point-virgule,
//! UTF-8 avec BOM, ce qu'Excel francophone ouvre d'un double-clic.
//! Colonnes : `Nom;Téléphone;Adresse;NIF;Email;Voisin`, lues par leur
//! nom (dans le desordre), ou dans cet ordre sans en-tete.
//!
//! Un fournisseur est cloisonne : l'import ecrit dans le dossier ou
//! l'on est, l'export n'en sort que les siens.
//!
//! Les doublons (`coeur::doublons`) : meme NIF = la meme entreprise ;
//! meme nom (sans la casse ni les espaces en trop) = le meme fournisseur
//! pour l'import, qui ne peut pas demander ligne par ligne. Un homonyme
//! se cree a la main (« créer quand même »). L'import ne renomme
//! jamais : un doublon se complete (ses champs vides) ou s'ignore.

use crate::base::{Acces, Base};
use crate::catalogue_csv::{champ, decouper, echapper, normaliser_entete};
use crate::coeur::doublons::{normaliser_nif, normaliser_nom, ressemblance_fournisseur, Ressemblance};
use crate::parametres;
use crate::utils::maintenant_iso;

const ENTETE: &str = "Nom;Téléphone;Adresse;NIF;Email;Voisin";

/// Le code qui precede le refus d'un homonyme : l'ecran le reconnait et
/// propose « créer quand même ».
pub const CODE_HOMONYME: &str = "FOURNISSEUR_HOMONYME";

/// Un fournisseur du dossier, tel qu'on le compare.
#[derive(Debug, Clone)]
struct Connu {
    id: String,
    nom: String,
    telephone: Option<String>,
    adresse: Option<String>,
    nif: Option<String>,
    email: Option<String>,
}

fn connus_du_dossier(acces: &mut impl Acces) -> Result<Vec<Connu>, String> {
    let dossier = acces.dossier().to_string();
    acces
        .lire_plusieurs(
            "SELECT id, nom, telephone, adresse, nif, email
             FROM fournisseur WHERE dossier_id = ?1 AND est_generique = 0
             ORDER BY cree_le",
            &parametres![dossier],
            |r| {
                Ok(Connu {
                    id: r.get::<String>(0)?,
                    nom: r.get::<String>(1)?,
                    telephone: r.get::<Option<String>>(2)?,
                    adresse: r.get::<Option<String>>(3)?,
                    nif: r.get::<Option<String>>(4)?,
                    email: r.get::<Option<String>>(5)?,
                })
            },
        )
        .map_err(|e| e.0)
}

/// Le premier fournisseur connu qui ressemble — le meme NIF passe avant
/// le meme nom.
fn ressemblant<'a>(
    connus: &'a [Connu],
    nom: &str,
    nif: Option<&str>,
    sauf: Option<&str>,
) -> Option<(Ressemblance, &'a Connu)> {
    let autres = || connus.iter().filter(move |c| Some(c.id.as_str()) != sauf);
    autres()
        .find_map(|c| {
            (ressemblance_fournisseur(nom, nif, &c.nom, c.nif.as_deref()) == Some(Ressemblance::MemeNif))
                .then_some((Ressemblance::MemeNif, c))
        })
        .or_else(|| {
            autres().find_map(|c| {
                ressemblance_fournisseur(nom, nif, &c.nom, c.nif.as_deref()).map(|r| (r, c))
            })
        })
}

/// Le jugement d'un fournisseur saisi a la main (creation ou
/// modification de `sauf`) : nom obligatoire, pas le nom reserve du
/// « Fournisseur divers », pas un NIF deja connu, et un homonyme
/// seulement si l'on confirme (`forcer`).
pub fn juger_saisie_sur(
    acces: &mut impl Acces,
    nom: &str,
    nif: Option<&str>,
    sauf: Option<&str>,
    forcer: bool,
) -> Result<String, String> {
    let nom = nom.split_whitespace().collect::<Vec<_>>().join(" ");
    if nom.is_empty() {
        return Err("Le nom est obligatoire".to_string());
    }
    if normaliser_nom(&nom) == normaliser_nom(crate::coeur::tiers::FOURNISSEUR_DIVERS) {
        return Err(format!(
            "« {} » est réservé aux achats sans fournisseur nommé.",
            crate::coeur::tiers::FOURNISSEUR_DIVERS
        ));
    }
    let connus = connus_du_dossier(acces)?;
    match ressemblant(&connus, &nom, nif, sauf) {
        Some((Ressemblance::MemeNif, c)) => Err(format!(
            "Le NIF {} est déjà celui de « {} ».",
            nif.unwrap_or("").trim(),
            c.nom
        )),
        Some((Ressemblance::MemeNom, c)) if !forcer => Err(format!(
            "{CODE_HOMONYME} : « {} » existe déjà{}. Le même fournisseur, ou un autre du même nom ?",
            c.nom,
            c.telephone.as_deref().map(|t| format!(" (tél. {t})")).unwrap_or_default()
        )),
        _ => Ok(nom),
    }
}

// =====================================================================
//  EXPORT
// =====================================================================

pub fn exporter_fournisseurs_csv_sur_base(base: &mut Base) -> Result<String, String> {
    let dossier = base.dossier().to_string();
    let lignes: Vec<String> = base
        .lire_plusieurs(
            "SELECT nom, COALESCE(telephone, ''), COALESCE(adresse, ''), COALESCE(nif, ''),
                    COALESCE(email, ''), est_voisin
             FROM fournisseur
             WHERE dossier_id = ?1 AND est_generique = 0 AND actif = 1
             ORDER BY nom",
            &parametres![dossier],
            |r| {
                Ok(format!(
                    "{};{};{};{};{};{}",
                    echapper(&r.get::<String>(0)?),
                    echapper(&r.get::<String>(1)?),
                    echapper(&r.get::<String>(2)?),
                    echapper(&r.get::<String>(3)?),
                    echapper(&r.get::<String>(4)?),
                    if r.get::<i64>(5)? != 0 { "oui" } else { "non" },
                ))
            },
        )
        .map_err(|e| e.0)?;
    Ok(format!("\u{FEFF}{}\n{}", ENTETE, lignes.join("\n")))
}

// =====================================================================
//  IMPORT
// =====================================================================

/// « oui », « non », vide. Autre chose refuse la ligne : on ne devine pas.
fn lire_voisin(v: Option<&str>) -> Result<Option<bool>, String> {
    match v.map(|s| s.trim().to_lowercase()).as_deref() {
        None | Some("") => Ok(None),
        Some("oui" | "o" | "1" | "vrai" | "x") => Ok(Some(true)),
        Some("non" | "n" | "0" | "faux") => Ok(Some(false)),
        Some(autre) => Err(format!("« Voisin » vaut « {autre} » : oui ou non.")),
    }
}

/// Importe des fournisseurs dans le dossier courant.
///
/// `completer` : un doublon recoit les champs qu'il n'a pas (telephone,
/// adresse, NIF, email) ; sans, il est laisse tel quel. Jamais renomme.
/// `simuler` : l'apercu — le meme code, dans une transaction qui
/// retombe.
pub fn importer_fournisseurs_csv_sur_base(
    base: &mut Base,
    contenu: String,
    completer: bool,
    simuler: bool,
) -> Result<serde_json::Value, String> {
    let dossier = base.dossier().to_string();
    let now = maintenant_iso();
    let auteur = crate::argent::id_utilisateur_courant_sur(base);
    let mut connus = connus_du_dossier(base)?;

    let contenu = contenu.trim_start_matches('\u{FEFF}');
    let mut lignes = contenu.lines();
    let premiere = lignes.next().unwrap_or("");
    let a_entete = premiere.to_lowercase().starts_with("nom");
    let cols: std::collections::HashMap<String, usize> = if a_entete {
        decouper(premiere).iter().enumerate().map(|(i, c)| (normaliser_entete(c), i)).collect()
    } else {
        std::collections::HashMap::new()
    };
    let mut a_traiter: Vec<&str> = Vec::new();
    if !a_entete {
        a_traiter.push(premiere);
    }
    a_traiter.extend(lignes);

    let mut tx = base.transaction().map_err(|e| e.0)?;
    let mut crees = 0;
    let mut completes = 0;
    let mut ignores: Vec<String> = Vec::new();
    let mut erreurs: Vec<String> = Vec::new();
    // Les fournisseurs crees par CE fichier : une seconde ligne du meme
    // est un doublon dans le fichier, pas une mise a jour.
    let mut crees_ici: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

    for (i, ligne) in a_traiter.iter().enumerate() {
        let num = if a_entete { i + 2 } else { i + 1 };
        if ligne.trim().is_empty() {
            continue;
        }
        let ch = decouper(ligne);
        let lire = |nom: &str, pos: usize| {
            champ(&ch, &cols, nom, Some(pos)).map(str::to_string).filter(|s| !s.is_empty())
        };
        let nom = lire("nom", 0).unwrap_or_default().split_whitespace().collect::<Vec<_>>().join(" ");
        if nom.is_empty() {
            erreurs.push(format!("Ligne {num} : nom vide"));
            continue;
        }
        if normaliser_nom(&nom) == normaliser_nom(crate::coeur::tiers::FOURNISSEUR_DIVERS) {
            erreurs.push(format!("Ligne {num} « {nom} » : nom réservé aux achats sans fournisseur. Ligne ignorée."));
            continue;
        }
        let telephone = lire("telephone", 1);
        let adresse = lire("adresse", 2);
        let nif = lire("nif", 3);
        let email = lire("email", 4);
        let voisin = match lire_voisin(champ(&ch, &cols, "voisin", Some(5))) {
            Ok(v) => v,
            Err(e) => {
                erreurs.push(format!("Ligne {num} « {nom} » : {e}"));
                continue;
            }
        };

        match ressemblant(&connus, &nom, nif.as_deref(), None).map(|(r, c)| (r, c.clone())) {
            Some((r, c)) => {
                if let Some(ligne_creation) = crees_ici.get(&c.id) {
                    erreurs.push(format!(
                        "Ligne {num} « {nom} » : même fournisseur que la ligne {ligne_creation} — doublon dans le fichier, ligne ignorée."
                    ));
                    continue;
                }
                let pourquoi = match r {
                    Ressemblance::MemeNif => "même NIF",
                    Ressemblance::MemeNom => "même nom",
                };
                if !completer {
                    ignores.push(format!("Ligne {num} « {nom} » : existe déjà (« {} », {pourquoi}), laissé tel quel.", c.nom));
                    continue;
                }
                // Completer : seulement ce qui manque, jamais le nom.
                let manque = |connu: &Option<String>, neuf: &Option<String>| {
                    connu.as_deref().is_none_or(|s| s.trim().is_empty()) && neuf.is_some()
                };
                // Un NIF ajoute ne doit pas etre celui d'un autre.
                let nif_libre = nif.as_deref().and_then(normaliser_nif).is_none_or(|n| {
                    !connus.iter().any(|k| k.id != c.id && k.nif.as_deref().and_then(normaliser_nif) == Some(n.clone()))
                });
                let ajoute_nif = manque(&c.nif, &nif) && nif_libre;
                let rien = !manque(&c.telephone, &telephone) && !manque(&c.adresse, &adresse) && !ajoute_nif && !manque(&c.email, &email);
                if rien {
                    ignores.push(format!("Ligne {num} « {nom} » : existe déjà (« {} », {pourquoi}), rien à compléter.", c.nom));
                    continue;
                }
                let garder = |connu: &Option<String>, neuf: &Option<String>, oui: bool| {
                    if oui { neuf.clone() } else { connu.clone() }
                };
                let t = garder(&c.telephone, &telephone, manque(&c.telephone, &telephone));
                let a = garder(&c.adresse, &adresse, manque(&c.adresse, &adresse));
                let n = garder(&c.nif, &nif, ajoute_nif);
                let e = garder(&c.email, &email, manque(&c.email, &email));
                tx.executer(
                    "UPDATE fournisseur SET telephone = CAST(?1 AS TEXT), adresse = CAST(?2 AS TEXT),
                        nif = CAST(?3 AS TEXT), email = CAST(?4 AS TEXT), modifie_le = ?5
                     WHERE id = ?6 AND dossier_id = ?7",
                    &parametres![t.clone(), a.clone(), n.clone(), e.clone(), now.clone(), c.id.clone(), dossier.clone()],
                )
                .map_err(|e| e.0)?;
                if let Some(k) = connus.iter_mut().find(|k| k.id == c.id) {
                    k.telephone = t;
                    k.adresse = a;
                    k.nif = n;
                    k.email = e;
                }
                completes += 1;
            }
            None => {
                let id = uuid::Uuid::new_v4().to_string();
                tx.executer(
                    "INSERT INTO fournisseur
                     (id, nom, telephone, adresse, nif, email, est_voisin, cree_le, modifie_le, origine, dossier_id)
                     VALUES (?1,?2,CAST(?3 AS TEXT),CAST(?4 AS TEXT),CAST(?5 AS TEXT),CAST(?6 AS TEXT),?7,?8,?8,'import',?9)",
                    &parametres![
                        id.clone(), nom.clone(), telephone.clone(), adresse.clone(), nif.clone(), email.clone(),
                        voisin.unwrap_or(false) as i64, now.clone(), dossier.clone()
                    ],
                )
                .map_err(|e| e.0)?;
                connus.push(Connu { id: id.clone(), nom, telephone, adresse, nif, email });
                crees_ici.insert(id, num);
                crees += 1;
            }
        }
    }

    let bilan = serde_json::json!({
        "crees": crees,
        "completes": completes,
        "ignores": ignores,
        "nb_ignores": ignores.len(),
        "erreurs": erreurs,
        "nb_erreurs": erreurs.len(),
        "simule": simuler,
    });
    if simuler {
        drop(tx);
        return Ok(bilan);
    }
    if crees + completes > 0 {
        tx.executer(
            "INSERT INTO journal
               (id, type_evenement, entite_type, entite_id, auteur_id,
                nouveau_valeur, origine, date_evenement, dossier_id)
             VALUES (?1,'import_fournisseurs','fournisseur','import',?2,?3,'import',?4,?5)",
            &parametres![
                uuid::Uuid::new_v4().to_string(),
                auteur,
                serde_json::json!({ "crees": crees, "completes": completes, "ignores": ignores.len(), "refuses": erreurs.len() })
                    .to_string(),
                now,
                dossier
            ],
        )
        .map_err(|e| e.0)?;
    }
    tx.valider().map_err(|e| e.0)?;
    Ok(bilan)
}

#[cfg(test)]
mod tests {
    use super::lire_voisin;

    #[test]
    fn voisin_se_lit_oui_non_ou_vide() {
        assert_eq!(lire_voisin(Some(" Oui ")), Ok(Some(true)));
        assert_eq!(lire_voisin(Some("non")), Ok(Some(false)));
        assert_eq!(lire_voisin(None), Ok(None));
        assert!(lire_voisin(Some("peut-être")).is_err());
    }
}
