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
pub const CRM_SUIVRE: &str = "crm:suivre";

/// Une regle de lecture d'une commande.
pub enum Regle {
    /// Sans la permission, la commande est refusee.
    Refus(&'static str),
    /// Sans AUCUNE de ces permissions, la commande est refusee (la
    /// premiere est celle que le refus nomme).
    RefusSaufUne(&'static [&'static str]),
    /// Sans AUCUNE de ces permissions, ces cles valent `null`, a
    /// n'importe quelle profondeur de la reponse. `si` : seulement
    /// quand la reponse le dit (une piece fournisseur, pas une facture
    /// client).
    Masque {
        permissions: &'static [&'static str],
        cles: &'static [&'static str],
        si: Option<fn(&Value) -> bool>,
    },
    /// Sans AUCUNE de ces permissions, dans chaque element du tableau
    /// `tableau` de la reponse que `si` designe, ces cles valent `null`
    /// (a n'importe quelle profondeur de l'element). L'Historique : les
    /// lignes de la paie perdent leurs montants, les autres les gardent.
    MasqueElements {
        permissions: &'static [&'static str],
        tableau: &'static str,
        si: fn(&Value) -> bool,
        cles: &'static [&'static str],
    },
    /// Sans AUCUNE de ces permissions, ces parametres prennent leur
    /// valeur neutre : un filtre « avec dette seulement » ou un tri par
    /// dette trahirait la donnee masquee.
    Neutre(&'static [&'static str], &'static [(&'static str, ParamNeutre)]),
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
// `personnel:avancer` lit la liste : on ne donne pas une avance sans
// choisir a qui. Les montants restent masques (PAIE_LIRE).
const PERSONNEL_LIRE: &[&str] = &["personnel:gerer", "paie:preparer", "paie:valider", "personnel:avancer"];
const PAIE_LIRE: &[&str] = &["paie:preparer", "paie:valider"];
const REMUNERATION: &[&str] = &["salaire_mensuel", "tarif_journalier", "commission_pct", "remuneration_dite", "avance_max"];
const AVANCES_LIRE: &[&str] = &["personnel:avancer", "paie:preparer", "paie:valider"];
/// Les montants que le journal garde d'un geste de paie (fiche validee,
/// versement, avance, cotisation) : traçabilite au journal, masques a la
/// lecture pour qui ne voit pas la paie (revue du 28/10, point 9).
const MONTANTS_PAIE: &[&str] =
    &["montant", "brut", "retenues", "net", "taux", "plafond", "reste", "en_cours", "trop_verse", "mensualite"];

/// L'appelant voit-il ce que gagne une personne ? `paie:preparer` ou
/// `paie:valider` — la meme regle que pour lire les montants de la
/// fiche du personnel. Les commandes qui ECRIVENT la remuneration la
/// demandent aussi (`personnel::modifier_selon_droits_sur`).
pub fn voit_la_paie(a: impl Fn(&str) -> bool) -> bool {
    PAIE_LIRE.iter().any(|p| a(p))
}

/// D49 : ce que l'accueil du jour d'Equipe montre a l'appelant. Chaque
/// bloc suit la regle de la commande qui le lit en entier ; un bloc
/// sans droit n'est pas calcule (`equipe_jour::accueil_sur`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AccueilEquipe {
    /// Ses rappels dus — comme `lire_rappels`.
    pub rappels: bool,
    /// Brouillons et fiches a solder — comme `lire_fiches_paie`.
    pub fiches: bool,
    /// Les avances en cours — comme `lire_avances`.
    pub avances: bool,
    /// Les montants : la paie seulement (C-1), avances comprises.
    pub montants: bool,
}

pub fn accueil_equipe(a: impl Fn(&str) -> bool) -> AccueilEquipe {
    AccueilEquipe {
        rappels: a(CRM_SUIVRE),
        fiches: PAIE_LIRE.iter().any(|p| a(p)),
        avances: AVANCES_LIRE.iter().any(|p| a(p)),
        montants: voit_la_paie(&a),
    }
}

/// Une ligne de l'Historique qui porte la paie : une fiche de paie (ses
/// totaux, ses versements), une cotisation (son taux) ou un pret (D47).
fn ligne_de_paie(v: &Value) -> bool {
    matches!(v.get("entite_type").and_then(Value::as_str), Some("fiche_paie" | "cotisation" | "pret"))
}

/// Une ligne de l'Historique qui porte une avance (son montant).
fn ligne_d_avance(v: &Value) -> bool {
    v.get("entite_type").and_then(Value::as_str) == Some("avance")
}

fn piece_fournisseur(v: &Value) -> bool {
    v.pointer("/piece/tiers_type").and_then(Value::as_str) == Some("fournisseur")
}

/// Les lignes d'une piece (`lire_lignes_piece`) : un tableau dont chaque
/// ligne dit le tiers de sa piece (lot C.7). Une seule ligne fournisseur
/// suffit — elles sont toutes de la meme piece.
fn lignes_fournisseur(v: &Value) -> bool {
    v.as_array()
        .is_some_and(|t| t.iter().any(|l| l.get("tiers_type").and_then(Value::as_str) == Some("fournisseur")))
}

/// Les regles d'une commande. Une commande absente n'en a aucune.
pub fn regles(commande: &str) -> &'static [Regle] {
    use Regle::*;
    match commande {
        // --- rapports:lire : le chiffre de la boutique ---
        "lire_resume_dashboard" | "lire_ventes_periode" | "lire_top_articles" | "lire_top_clients"
        | "lire_rapport_top_clients" | "lire_rapport_tva"
        | "lire_journal_du_jour" | "lire_resume_par_depot" | "lire_ventes_a_decouvert" => {
            &[Refus(RAPPORTS_LIRE)]
        }
        // Gescom Equipe : le cout des salaires, pour qui prepare ou
        // valide la paie seulement.
        "lire_rapport_ca_mensuel" => &[
            Refus(RAPPORTS_LIRE),
            Masque { permissions: PAIE_LIRE, cles: &["salaires", "apres_salaires"], si: None },
        ],
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
        // Lot C.7 : les lignes de la meme piece, lues pour la modifier.
        // Meme regle que `lire_donnees_piece` — sinon un prix cache sur
        // l'apercu se lirait dans la fenetre de modification.
        "lire_lignes_piece" => {
            &[Masque { permissions: ACHATS, cles: TOTAUX_PIECE, si: Some(lignes_fournisseur) }]
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
                &[TIERS_LIRE_SOLDE],
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

        // --- Gescom Equipe (PLAN-EQUIPE) : le personnel se lit par qui le
        // gere ou le paie ; ce qu'il gagne, par qui prepare ou valide la
        // paie seulement. ---
        "lire_presences_mois" => &[RefusSaufUne(PERSONNEL_LIRE)],
        // D52 : le motif d'une absence (maladie…) se lit comme la grille.
        "lire_absences_annee" => &[RefusSaufUne(PERSONNEL_LIRE)],
        "lire_avances" => &[RefusSaufUne(AVANCES_LIRE)],
        // G-4 : le journal de paie nomme ce que chacun gagne. Sans le
        // droit de lire la paie, il n'est ni fabrique ni rendu. Lire la
        // paie, c'est la preparer OU la valider (PLAN-EQUIPE, « Lecture » ;
        // D31 : qui valide relit ce qu'il fige) — `paie:valider` seul le
        // lit aussi depuis le 28/10, comme les fiches et le CA mensuel.
        "lire_journaux_comptables" | "exporter_journaux_csv" => &[Neutre(PAIE_LIRE, &[("avecPaie", ParamNeutre::Faux)])],
        "lire_fiches_paie" | "lire_fiche_paie" | "lire_donnees_bulletin" | "lire_cotisations" => &[RefusSaufUne(PAIE_LIRE)],
        // D47 : les prets se lisent comme la paie.
        "lire_prets" | "lire_pret" => &[RefusSaufUne(PAIE_LIRE)],
        // EQ-2 (D48) : l'etat de paie n'est fait que de montants — refus,
        // a l'ecran comme pour le fichier.
        "lire_etat_paie" | "exporter_etat_paie_csv" => &[RefusSaufUne(PAIE_LIRE)],
        "lire_personnel" | "lire_employe" => &[
            RefusSaufUne(PERSONNEL_LIRE),
            Masque { permissions: PAIE_LIRE, cles: REMUNERATION, si: None },
        ],
        // Revue du 28/10, point 3 : les ecritures de la fiche rendent la
        // fiche — masquee comme a la lecture. Un depart dit aussi l'avance
        // en cours, pour qui lit les avances.
        "creer_employe" | "modifier_employe" | "faire_revenir_employe" => {
            &[Masque { permissions: PAIE_LIRE, cles: REMUNERATION, si: None }]
        }
        "faire_partir_employe" => &[
            Masque { permissions: PAIE_LIRE, cles: REMUNERATION, si: None },
            Masque { permissions: AVANCES_LIRE, cles: &["avance_en_cours"], si: None },
        ],
        // Revue du 28/10, point 9 : l'Historique (`journal:lire`) garde
        // les montants de la paie au journal, mais ne les rend qu'a qui
        // voit la paie (une avance : a qui lit les avances). La recherche
        // ne fouille pas non plus leur detail : chercher « 60000 »
        // trouverait la fiche sans la montrer.
        "lire_historique" => &[
            MasqueElements { permissions: PAIE_LIRE, tableau: "lignes", si: ligne_de_paie, cles: MONTANTS_PAIE },
            MasqueElements { permissions: AVANCES_LIRE, tableau: "lignes", si: ligne_d_avance, cles: MONTANTS_PAIE },
            Neutre(PAIE_LIRE, &[("avecPaie", ParamNeutre::Faux)]),
        ],

        // --- H-1/H-2 (D34) : le suivi client, une seule permission ---
        "lire_echanges_client" | "lire_rappels" | "lire_prospects" => &[Refus(CRM_SUIVRE)],

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
        Regle::RefusSaufUne(ps) if !ps.iter().any(|p| a(p)) => ps.first().copied(),
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
        if let Regle::Neutre(ps, liste) = r {
            if ps.iter().any(|p| a(p)) {
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
            Regle::MasqueElements { permissions, tableau, si, cles } => {
                if permissions.iter().any(|p| a(p)) {
                    continue;
                }
                if let Some(t) = reponse.get_mut(*tableau).and_then(Value::as_array_mut) {
                    t.iter_mut().filter(|e| si(e)).for_each(|e| masquer(e, cles));
                }
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
    fn les_lignes_d_une_piece_fournisseur_perdent_leurs_prix_pas_celles_d_un_client() {
        // Lot C.7 : `lire_lignes_piece` rendait le prix d'achat a tous.
        let ligne = |tiers: &str| {
            json!({ "article_nom": "Sucre", "quantite": 6.0, "taux_tva": 0.0, "remise_pct": 0.0,
                    "prix_unitaire": 800, "montant_ht": 4800, "montant_tva": 0, "remise_montant": 0,
                    "tiers_type": tiers })
        };
        let mut f = json!([ligne("fournisseur"), ligne("fournisseur")]);
        filtrer("lire_lignes_piece", sans, "u", &mut f);
        for l in f.as_array().unwrap() {
            for cle in ["prix_unitaire", "montant_ht", "montant_tva", "remise_montant"] {
                assert_eq!(l[cle], Value::Null, "{cle}");
            }
            // Ce qui n'est pas un prix reste : on voit QUOI, pas combien.
            assert_eq!(l["quantite"], 6.0);
            assert_eq!(l["article_nom"], "Sucre");
        }
        // Le droit de lire les prix, ou de saisir les achats (comme
        // `lire_donnees_piece`) : rien n'est masque.
        for p in [ACHATS_LIRE_PRIX, "achats:creer"] {
            let mut g = json!([ligne("fournisseur")]);
            filtrer("lire_lignes_piece", avec(p), "u", &mut g);
            assert_eq!(g[0]["prix_unitaire"], 800, "{p}");
        }
        // Une piece client : le caissier lit ses prix, c'est lui qui vend.
        let client = json!([ligne("client")]);
        let mut c = client.clone();
        filtrer("lire_lignes_piece", sans, "u", &mut c);
        assert_eq!(c, client);
        // Une piece sans ligne : rien a masquer, rien ne casse.
        let mut vide = json!([]);
        filtrer("lire_lignes_piece", sans, "u", &mut vide);
        assert_eq!(vide, json!([]));
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
    fn les_ecritures_de_la_fiche_du_personnel_rendent_une_fiche_masquee() {
        // Revue du 28/10, point 3 : creer, modifier, faire partir, faire
        // revenir rendaient le salaire a qui gere sans la paie.
        let fiche = json!({ "nom": "Awa", "salaire_mensuel": 60000, "commission_pct": 2.0, "avance_max": 15000,
                            "remuneration_dite": "60 000 F par mois", "modes": ["au mois"], "avance_en_cours": 10000 });
        for cmd in ["creer_employe", "modifier_employe", "faire_partir_employe", "faire_revenir_employe"] {
            let mut v = fiche.clone();
            filtrer(cmd, |p| p == "personnel:gerer", "u", &mut v);
            for cle in REMUNERATION {
                assert_eq!(v[*cle], Value::Null, "{cmd} · {cle}");
            }
            assert_eq!(v["modes"], json!(["au mois"]), "{cmd} : la façon d'être payée reste");
            let mut w = fiche.clone();
            filtrer(cmd, |p| p == "paie:valider", "u", &mut w);
            assert_eq!(w["salaire_mensuel"], 60000, "{cmd}");
        }
        let mut v = fiche.clone();
        filtrer("faire_partir_employe", |p| p == "personnel:gerer", "u", &mut v);
        assert_eq!(v["avance_en_cours"], Value::Null, "l'avance en cours, pour qui lit les avances");
        let mut w = fiche;
        filtrer("faire_partir_employe", |p| p == "personnel:avancer", "u", &mut w);
        assert_eq!(w["avance_en_cours"], 10000);
        assert!(voit_la_paie(|p| p == "paie:preparer") && voit_la_paie(|p| p == "paie:valider"));
        assert!(!voit_la_paie(|p| p == "personnel:gerer"));
    }

    #[test]
    fn le_journal_de_paie_se_lit_avec_la_preparer_ou_la_valider() {
        for (p, attendu) in [("rapports:lire", Some(false)), ("paie:preparer", None), ("paie:valider", None)] {
            let mut params = json!({ "du": "2026-09-01" });
            neutraliser("lire_journaux_comptables", |x| x == p, &mut params);
            assert_eq!(params["avecPaie"].as_bool(), attendu, "{p}");
        }
    }

    #[test]
    fn l_historique_garde_les_montants_de_la_paie_pour_qui_la_voit() {
        // Revue du 28/10, point 9.
        let page = json!({ "total": 4, "lignes": [
            { "type": "paie_validee", "entite_type": "fiche_paie", "nouveau": { "nom": "Awa", "net": 60000, "brut": 60000, "retenues": 0 } },
            { "type": "paie_versee", "entite_type": "fiche_paie", "nouveau": { "nom": "Awa", "montant": 45000, "moyen": "especes" } },
            { "type": "avance_donnee", "entite_type": "avance", "nouveau": { "nom": "Awa", "montant": 10000 } },
            { "type": "cotisation_modifiee", "entite_type": "cotisation", "nouveau": { "libelle": "INPS", "taux": 3.6, "plafond": null } },
            { "type": "paiement", "entite_type": "paiement", "nouveau": { "montant": 5000 } },
        ]});
        let mut v = page.clone();
        filtrer("lire_historique", |p| p == "journal:lire", "u", &mut v);
        let l = &v["lignes"];
        assert_eq!((l[0]["nouveau"]["net"].clone(), l[0]["nouveau"]["nom"].clone()), (Value::Null, json!("Awa")), "le nom reste, le net part");
        assert_eq!(l[1]["nouveau"]["montant"], Value::Null);
        assert_eq!(l[1]["nouveau"]["moyen"], "especes");
        assert_eq!(l[2]["nouveau"]["montant"], Value::Null);
        assert_eq!(l[3]["nouveau"]["taux"], Value::Null);
        assert_eq!(l[4]["nouveau"]["montant"], 5000, "un paiement client garde son montant");
        assert_eq!(v["total"], 4);
        // Qui lit les avances voit l'avance, pas la fiche.
        let mut w = page.clone();
        filtrer("lire_historique", |p| p == "journal:lire" || p == "personnel:avancer", "u", &mut w);
        assert_eq!(w["lignes"][2]["nouveau"]["montant"], 10000);
        assert_eq!(w["lignes"][0]["nouveau"]["net"], Value::Null);
        // Qui voit la paie voit tout.
        let mut x = page.clone();
        filtrer("lire_historique", |p| p == "journal:lire" || p == "paie:valider", "u", &mut x);
        assert_eq!(x, page);
        // Et la recherche ne fouille pas le détail de la paie à sa place.
        let mut params = json!({ "filtre": { "recherche": "60000" } });
        neutraliser("lire_historique", |p| p == "journal:lire", &mut params);
        assert_eq!(params["avecPaie"], false);
        let mut params = json!({ "filtre": {} });
        neutraliser("lire_historique", |p| p == "paie:preparer", &mut params);
        assert!(params.get("avecPaie").is_none());
    }

    #[test]
    fn chaque_permission_nommee_existe_au_catalogue() {
        for p in [ACHATS_LIRE_PRIX, RAPPORTS_LIRE, TIERS_LIRE_SOLDE, CAISSE_LIRE_AUTRES, CRM_SUIVRE, "achats:creer"]
            .into_iter()
            .chain(PAIE_LIRE.iter().copied())
            .chain(AVANCES_LIRE.iter().copied())
            .chain(PERSONNEL_LIRE.iter().copied())
        {
            assert!(crate::portes::existe(p), "{p}");
        }
    }
}
