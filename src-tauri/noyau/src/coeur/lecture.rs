//! Les permissions de LECTURE (v3, C-1) : ce qu'une commande refuse, ou
//! rend sans le champ, a qui n'a pas le droit de le lire.
//!
//! Cinq permissions, pas trente (PLAN-V3 § C1, D19) :
//!
//! | permission            | ce qu'elle decouvre                                    |
//! |-----------------------|--------------------------------------------------------|
//! | `achats:lire_prix`    | prix d'achat, marges, valeur du stock                  |
//! | `rapports:lire`       | tableau de bord chiffre, rapports, cahier du jour      |
//! | `tiers:lire_solde`    | ce que DOIT un client, ce qu'on doit a un fournisseur  |
//! | `journal:lire`        | l'Historique (B-1, portee par la commande elle-meme)   |
//! | `caisse:lire_autres`  | les sessions de caisse ouvertes par d'autres           |
//!
//! Sans la permission, la commande **rend la donnee sans le champ**
//! (`null`, jamais un zero qui ressemblerait a une vraie valeur) ou
//! **refuse** — quand la commande n'est faite que de ce champ (un
//! rapport, un releve de dette).
//!
//! La regle vit ICI, en une table, et le serveur l'applique a la
//! sortie de chaque commande (`api::rpc`) : pas de `if role == …`
//! disperse dans quarante fonctions, qu'on finirait par oublier dans
//! la quarante et unieme.
//!
//! Deux lignes de partage, choisies et ecrites :
//! - `tiers:lire_solde` cache ce que doit un TIERS (etat, releve,
//!   encours, listes de dettes, tri par dette). Le reste d'UNE piece ou
//!   d'UN recu reste lisible : c'est le document qu'on a en main, et le
//!   client au comptoir le lit sur son recu.
//! - `achats:lire_prix` cache le cout la ou il est incident (catalogue,
//!   stock, magasins, rapports). Sur les documents d'ACHAT eux-memes
//!   (fiche fournisseur, factures a retourner, pieces fournisseur),
//!   `achats:creer` suffit : celui qui saisit l'achat tape ces prix.

use serde_json::Value;

pub const ACHATS_LIRE_PRIX: &str = "achats:lire_prix";
pub const RAPPORTS_LIRE: &str = "rapports:lire";
pub const TIERS_LIRE_SOLDE: &str = "tiers:lire_solde";
pub const CAISSE_LIRE_AUTRES: &str = "caisse:lire_autres";

/// Une regle de lecture d'une commande.
pub enum Regle {
    /// Sans la permission, la commande est refusee.
    Refus(&'static str),
    /// Sans AUCUNE de ces permissions, ces cles valent `null`, a
    /// n'importe quelle profondeur de la reponse. `si` : seulement
    /// quand la reponse le dit (une piece fournisseur, pas une facture
    /// client).
    Masque {
        permissions: &'static [&'static str],
        cles: &'static [&'static str],
        si: Option<fn(&Value) -> bool>,
    },
    /// Sans la permission, ces parametres prennent leur valeur neutre :
    /// un filtre « avec dette seulement » ou un tri par dette trahirait
    /// la donnee masquee.
    Neutre(&'static str, &'static [(&'static str, ParamNeutre)]),
    /// Sans la permission, un tableau ne garde que les elements dont
    /// cette cle vaut l'utilisateur qui lit.
    AMoi(&'static str, &'static str),
    /// Sans la permission, le parametre (une session de caisse) doit
    /// designer une session ouverte par l'utilisateur. Se juge en base :
    /// le serveur le fait avant d'appeler la commande.
    SessionCaisseAMoi(&'static str),
}

/// La valeur neutre d'un parametre.
#[derive(Clone, Copy)]
pub enum ParamNeutre {
    Faux,
    Texte(&'static str),
}

const PRIX: &[&str] = &["dernier_prix_achat", "prix_achat", "valeur", "valeur_stock", "valeur_totale", "marge"];
const SOLDES_CLIENT: &[&str] = &["total_creances", "encours"];
// `reste_apres` (le reste d'UNE facture apres UN versement) n'y est pas :
// c'est le document en main, pas la dette du tiers.
const SOLDES_FOURNISSEUR: &[&str] = &["dette", "total_dettes", "total_paye", "total_achats"];
const TOTAUX_PIECE: &[&str] = &[
    "prix_unitaire", "montant_ht", "montant_tva", "remise_montant",
    "total_ht", "total_tva", "total_net", "total_ttc", "total_paye", "reste", "reste_du",
];
const ACHATS: &[&str] = &[ACHATS_LIRE_PRIX, "achats:creer"];

fn piece_fournisseur(v: &Value) -> bool {
    v.pointer("/piece/tiers_type").and_then(Value::as_str) == Some("fournisseur")
}

/// Les regles d'une commande. Une commande absente n'en a aucune.
pub fn regles(commande: &str) -> &'static [Regle] {
    use Regle::*;
    match commande {
        // --- rapports:lire : le chiffre de la boutique ---
        "lire_resume_dashboard" | "lire_ventes_periode" | "lire_top_articles" | "lire_top_clients"
        | "lire_rapport_ca_mensuel" | "lire_rapport_top_clients" | "lire_rapport_tva"
        | "lire_journal_du_jour" | "lire_resume_par_depot" | "lire_ventes_a_decouvert" => {
            &[Refus(RAPPORTS_LIRE)]
        }
        "lire_rapport_top_articles" | "lire_rapport_stock" => &[
            Refus(RAPPORTS_LIRE),
            Masque { permissions: &[ACHATS_LIRE_PRIX], cles: PRIX, si: None },
        ],
        "lire_rapport_creances" => &[Refus(RAPPORTS_LIRE), Refus(TIERS_LIRE_SOLDE)],
        "lire_rapport_ecarts" => &[Refus(RAPPORTS_LIRE), Refus(CAISSE_LIRE_AUTRES)],

        // --- achats:lire_prix, la ou le cout est incident ---
        "lire_articles_avec_unites" | "lire_articles_complets" | "lire_etat_stock"
        | "lire_depots_detail" | "lire_mouvements_stock" => {
            &[Masque { permissions: &[ACHATS_LIRE_PRIX], cles: PRIX, si: None }]
        }
        // Un fichier CSV : pas de cle a vider, on refuse.
        "exporter_articles_csv" => &[Refus(ACHATS_LIRE_PRIX)],

        // --- achats:lire_prix, sur les documents d'achat (achats:creer suffit) ---
        "lire_factures_fournisseur_retournables" => {
            &[Masque { permissions: ACHATS, cles: &["prix_achat", "total"], si: None }]
        }
        "lire_toutes_pieces_fournisseur" => &[Masque { permissions: ACHATS, cles: TOTAUX_PIECE, si: None }],
        "lire_donnees_piece" => {
            &[Masque { permissions: ACHATS, cles: TOTAUX_PIECE, si: Some(piece_fournisseur) }]
        }
        "lire_fiche_fournisseur" => &[
            Masque { permissions: ACHATS, cles: &["prix_achat"], si: None },
            Masque { permissions: &[TIERS_LIRE_SOLDE], cles: SOLDES_FOURNISSEUR, si: None },
        ],

        // --- tiers:lire_solde : les commandes faites de soldes ---
        "lire_etat_creances_client" | "lire_etat_creances_global" | "lire_creances_ouvertes"
        | "lire_clients_avec_creances" | "lire_creances_relances" | "lire_stats_relances"
        | "lire_irrecouvrable" | "lire_etat_dette_fournisseur" | "lire_etat_dettes_global"
        | "lire_dettes_fournisseurs" | "lire_fournisseurs_avec_dettes"
        | "lire_factures_fournisseur_ouvertes" => &[Refus(TIERS_LIRE_SOLDE)],

        // --- tiers:lire_solde : le solde incident d'une fiche ou d'une liste ---
        "lire_fiche_client" => &[Masque { permissions: &[TIERS_LIRE_SOLDE], cles: SOLDES_CLIENT, si: None }],
        "lire_clients_pagines" => &[
            Masque { permissions: &[TIERS_LIRE_SOLDE], cles: SOLDES_CLIENT, si: None },
            Neutre(
                TIERS_LIRE_SOLDE,
                &[
                    ("avecCreancesSeulement", ParamNeutre::Faux),
                    ("avec_creances_seulement", ParamNeutre::Faux),
                    ("tri", ParamNeutre::Texte("nom")),
                ],
            ),
        ],
        "lire_fournisseurs_pagines" => {
            &[Masque { permissions: &[TIERS_LIRE_SOLDE], cles: SOLDES_FOURNISSEUR, si: None }]
        }

        // --- caisse:lire_autres : les sessions des autres ---
        "lire_sessions_caisse" => &[AMoi(CAISSE_LIRE_AUTRES, "ouvert_par_id")],
        "lire_mouvements_session" => &[SessionCaisseAMoi(CAISSE_LIRE_AUTRES)],

        _ => &[],
    }
}

/// La premiere permission qui manque pour appeler la commande, s'il en
/// manque une.
pub fn refus(commande: &str, a: impl Fn(&str) -> bool) -> Option<&'static str> {
    regles(commande).iter().find_map(|r| match r {
        Regle::Refus(p) if !a(p) => Some(*p),
        _ => None,
    })
}

/// La permission qui oblige a verifier la session de caisse demandee,
/// quand l'appelant ne l'a pas.
pub fn session_caisse_a_verifier(commande: &str, a: impl Fn(&str) -> bool) -> bool {
    regles(commande).iter().any(|r| matches!(r, Regle::SessionCaisseAMoi(p) if !a(p)))
}

/// Remplace les parametres qui trahiraient une donnee masquee — y
/// compris quand ils sont absents.
pub fn neutraliser(commande: &str, a: impl Fn(&str) -> bool, params: &mut Value) {
    for r in regles(commande) {
        if let Regle::Neutre(p, liste) = r {
            if a(p) {
                continue;
            }
            // Pose MEME si absent : le tri par defaut de la liste des
            // clients est… par dette.
            if params.is_null() {
                *params = Value::Object(Default::default());
            }
            if let Some(o) = params.as_object_mut() {
                for (cle, neutre) in liste.iter() {
                    o.insert(
                        cle.to_string(),
                        match neutre {
                            ParamNeutre::Faux => Value::Bool(false),
                            ParamNeutre::Texte(t) => Value::String(t.to_string()),
                        },
                    );
                }
            }
        }
    }
}

/// Ce que l'appelant a le droit de lire de la reponse.
pub fn filtrer(commande: &str, a: impl Fn(&str) -> bool, utilisateur_id: &str, reponse: &mut Value) {
    for r in regles(commande) {
        match r {
            Regle::Masque { permissions, cles, si } => {
                if permissions.iter().any(|p| a(p)) {
                    continue;
                }
                if si.is_some_and(|f| !f(reponse)) {
                    continue;
                }
                masquer(reponse, cles);
            }
            Regle::AMoi(p, cle) if !a(p) => {
                if let Some(t) = reponse.as_array_mut() {
                    t.retain(|e| e.get(*cle).and_then(Value::as_str) == Some(utilisateur_id));
                }
            }
            _ => {}
        }
    }
}

/// Met a `null` chacune de ces cles, a n'importe quelle profondeur.
/// Une cle absente reste absente : on ne fabrique pas de champ.
pub fn masquer(v: &mut Value, cles: &[&str]) {
    match v {
        Value::Object(o) => {
            for (k, x) in o.iter_mut() {
                if cles.contains(&k.as_str()) {
                    *x = Value::Null;
                } else {
                    masquer(x, cles);
                }
            }
        }
        Value::Array(t) => t.iter_mut().for_each(|x| masquer(x, cles)),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sans(_: &str) -> bool {
        false
    }
    fn avec(p: &'static str) -> impl Fn(&str) -> bool {
        move |x| x == p
    }

    #[test]
    fn masquer_touche_toutes_les_profondeurs_et_ne_fabrique_rien() {
        let mut v = json!({ "prix_achat": 900, "lignes": [{ "prix_achat": 5, "nom": "Sucre" }], "prix": 1000 });
        masquer(&mut v, &["prix_achat", "marge"]);
        assert_eq!(v, json!({ "prix_achat": null, "lignes": [{ "prix_achat": null, "nom": "Sucre" }], "prix": 1000 }));
    }

    #[test]
    fn un_rapport_se_refuse_et_nomme_la_permission() {
        assert_eq!(refus("lire_resume_dashboard", sans), Some(RAPPORTS_LIRE));
        assert_eq!(refus("lire_resume_dashboard", avec(RAPPORTS_LIRE)), None);
        // Deux portes : la premiere qui manque est nommee.
        assert_eq!(refus("lire_rapport_creances", avec(RAPPORTS_LIRE)), Some(TIERS_LIRE_SOLDE));
        assert_eq!(refus("creer_vente", sans), None, "une ecriture n'est pas concernee");
    }

    #[test]
    fn le_cout_disparait_du_catalogue_mais_pas_le_prix_de_vente() {
        let mut v = json!([{ "nom": "Sucre", "dernier_prix_achat": 800, "unites": [{ "prix_reference": 1000 }] }]);
        filtrer("lire_articles_avec_unites", sans, "u", &mut v);
        assert_eq!(v[0]["dernier_prix_achat"], Value::Null);
        assert_eq!(v[0]["unites"][0]["prix_reference"], 1000);
        let mut w = json!([{ "dernier_prix_achat": 800 }]);
        filtrer("lire_articles_avec_unites", avec(ACHATS_LIRE_PRIX), "u", &mut w);
        assert_eq!(w[0]["dernier_prix_achat"], 800);
    }

    #[test]
    fn une_facture_client_garde_ses_totaux_une_piece_fournisseur_non() {
        let client = json!({ "piece": { "tiers_type": "client" }, "totaux": { "total_ttc": 5000 } });
        let mut c = client.clone();
        filtrer("lire_donnees_piece", sans, "u", &mut c);
        assert_eq!(c, client);
        let mut f = json!({ "piece": { "tiers_type": "fournisseur" }, "totaux": { "total_ttc": 5000 } });
        filtrer("lire_donnees_piece", sans, "u", &mut f);
        assert_eq!(f["totaux"]["total_ttc"], Value::Null);
        // Le magasinier qui saisit les achats les lit.
        let mut m = json!({ "piece": { "tiers_type": "fournisseur" }, "totaux": { "total_ttc": 5000 } });
        filtrer("lire_donnees_piece", avec("achats:creer"), "u", &mut m);
        assert_eq!(m["totaux"]["total_ttc"], 5000);
    }

    #[test]
    fn le_tri_par_dette_ne_trahit_pas_la_dette() {
        let mut p = json!({ "page": 0, "avecCreancesSeulement": true, "tri": "creance" });
        neutraliser("lire_clients_pagines", sans, &mut p);
        assert_eq!(p["avecCreancesSeulement"], false);
        assert_eq!(p["avec_creances_seulement"], false);
        assert_eq!(p["tri"], "nom");
        let mut q = json!({ "avecCreancesSeulement": true, "tri": "creance" });
        neutraliser("lire_clients_pagines", avec(TIERS_LIRE_SOLDE), &mut q);
        assert_eq!(q["tri"], "creance");
        // Absent, le tri est pose quand meme : le tri par defaut de la
        // liste est par dette.
        let mut r = json!({ "page": 0 });
        neutraliser("lire_clients_pagines", sans, &mut r);
        assert_eq!(r["tri"], "nom");
        assert_eq!(r["page"], 0);
    }

    #[test]
    fn sans_caisse_lire_autres_on_ne_voit_que_ses_sessions() {
        let mut v = json!([{ "id": "s1", "ouvert_par_id": "moi" }, { "id": "s2", "ouvert_par_id": "autre" }]);
        filtrer("lire_sessions_caisse", sans, "moi", &mut v);
        assert_eq!(v, json!([{ "id": "s1", "ouvert_par_id": "moi" }]));
        assert!(session_caisse_a_verifier("lire_mouvements_session", sans));
        assert!(!session_caisse_a_verifier("lire_mouvements_session", avec(CAISSE_LIRE_AUTRES)));
    }

    #[test]
    fn chaque_permission_nommee_existe_au_catalogue() {
        for p in [ACHATS_LIRE_PRIX, RAPPORTS_LIRE, TIERS_LIRE_SOLDE, CAISSE_LIRE_AUTRES, "achats:creer"] {
            assert!(crate::portes::existe(p), "{p}");
        }
    }
}
