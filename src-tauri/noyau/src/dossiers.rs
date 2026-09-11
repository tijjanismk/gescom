//! Le cloisonnement par dossier.
//!
//! Un dossier = **une societe x un exercice**, comme chez Ciel : une
//! date de debut, une date de fin, et une prolongation possible.
//!
//! ## La forme retenue, et ce qu'elle coute
//!
//! Chaque table cloisonnee porte une colonne `dossier_id`. L'autre
//! forme possible — une base par dossier — n'aurait demande aucun
//! filtre, mais interdisait toute vue consolidee.
//!
//! Le prix de ce choix est connu et tient en une phrase : **une requete
//! qui oublie le filtre melange deux societes.** Et elle ne le dit pas.
//! Elle rend des chiffres plausibles, calcules sur les ventes de
//! quelqu'un d'autre.
//!
//! C'est pour cela que ce module existe : il ne se contente pas de
//! declarer la colonne, il donne de quoi ATTRAPER l'oubli.
//!
//! ## Ce qui n'est PAS cloisonne
//!
//! Les personnes et les machines : `utilisateur`, `role`, `poste`,
//! `session_reseau`, `modele_document`. Un caissier qui change de
//! societe reste le meme caissier ; l'obliger a un compte par dossier
//! multiplierait les mots de passe sans rien proteger.
//!
//! **`categorie`, `article`, `unite_vente` non plus.** Decision du
//! chapitre 3 du plan multi-societe : un article est une CHOSE, pas une
//! relation — le sac de ciment CIMAF est le meme objet pour tous les
//! dossiers, et le saisir deux fois obligerait a le corriger deux fois
//! quand son nom change. `stock_depot`, lui, reste cloisonne : le stock
//! appartient au magasin, donc au dossier qui le possede.
//!
//! ## Le cas du compteur de pieces
//!
//! `compteur_piece` n'a PAS de colonne. Sa cle porte le dossier
//! (`<dossier>:<serie>`), ce qui cloisonne la numerotation sans toucher
//! a sa cle primaire ni a sa clause `ON CONFLICT`.
//!
//! C'est delibere : la numerotation est la seule chose du projet ou un
//! doublon se voit chez le commercant et ne se repare pas. On ne
//! remanie pas sa table pour une raison de forme.

/// Le dossier d'origine, celui des bases qui n'en connaissaient qu'un.
///
/// Un identifiant FIXE et non un uuid tire au hasard : il sert de
/// valeur par defaut dans le DDL SQLite, qui n'accepte qu'une
/// constante.
pub const DOSSIER_DEFAUT: &str = "00000000-0000-0000-0000-000000000001";

/// Les tables dont chaque ligne appartient a un dossier.
///
/// Cette liste est la reference : le DDL la parcourt pour poser les
/// colonnes, et le detecteur la parcourt pour reperer les requetes non
/// filtrees. Une table ajoutee ici est cloisonnee partout a la fois.
pub const TABLES_CLOISONNEES: &[&str] = &[
    "depot",
    "stock_depot",
    "client",
    "fournisseur",
    "vente",
    "ligne_vente",
    "facture",
    "paiement",
    "piece_commerciale",
    "ligne_piece",
    "session_caisse",
    "mouvement_caisse",
    "mouvement_stock",
    "transfert",
    "retour",
    "avoir",
    "paiement_fournisseur",
    "creance_irrecouvrable",
    "relance_creance",
    "journal",
];

/// La cle d'un compteur, cloisonnee par dossier.
pub fn cle_compteur(dossier: &str, serie: &str) -> String {
    format!("{dossier}:{serie}")
}

// =====================================================================
//  LE DETECTEUR
// =====================================================================

/// Nomme les tables cloisonnees qu'une requete touche sans filtrer.
///
/// Rend `None` quand la requete est saine. Rend le reproche quand elle
/// ne l'est pas, avec le nom des tables en cause — parce qu'un message
/// qui dit seulement « requete non cloisonnee » oblige a relire
/// cinquante lignes de SQL pour trouver laquelle.
///
/// ## Ce qu'il ne sait pas faire
///
/// Ce n'est pas un analyseur SQL. Il repere un nom de table et
/// l'absence du mot `dossier_id` — rien de plus. Il peut donc se taire
/// a tort sur une requete qui mentionne `dossier_id` dans une
/// sous-requete mais pas dans la principale.
///
/// C'est assume : un detecteur grossier qui tourne sur les 739 points
/// d'appel attrape plus d'oublis reels qu'un analyseur exact qu'on
/// n'ecrira jamais. Il ne remplace pas la relecture, il la dirige.
pub fn requete_non_cloisonnee(sql: &str) -> Option<String> {
    let nu = sans_litteraux(sql);
    if nu.contains("dossier_id") {
        return None;
    }
    // Le DDL et les reglages de session ne portent pas de filtre, et
    // n'ont pas a en porter.
    let debut = nu.trim_start();
    for prefixe in ["create", "alter", "drop", "pragma", "set ", "select set_config"] {
        if debut.starts_with(prefixe) {
            return None;
        }
    }

    let touchees: Vec<&str> = TABLES_CLOISONNEES
        .iter()
        .copied()
        .filter(|t| mot_present(&nu, t))
        .collect();

    if touchees.is_empty() {
        return None;
    }
    Some(format!(
        "requête non cloisonnée — elle touche {} sans filtrer sur \
         dossier_id, et mélangerait deux sociétés : {}",
        touchees.join(", "),
        sql.split_whitespace().collect::<Vec<_>>().join(" ")
    ))
}

/// Le SQL en minuscules, les textes entre apostrophes remplaces par un
/// vide.
///
/// Sans cela, une requete qui insere le mot « client » dans un libelle
/// serait prise pour une lecture de la table `client`.
fn sans_litteraux(sql: &str) -> String {
    let mut sortie = String::with_capacity(sql.len());
    let mut dans_texte = false;
    for c in sql.chars() {
        match c {
            '\'' => dans_texte = !dans_texte,
            _ if dans_texte => {}
            _ => sortie.push(c.to_ascii_lowercase()),
        }
    }
    sortie
}

/// `mot` present en tant que MOT, pas en morceau d'un autre.
///
/// `client` ne doit pas se reconnaitre dans `client_id`, ni `vente`
/// dans `ligne_vente` — sinon le detecteur crie sur tout, et on
/// l'eteint au bout d'une heure.
fn mot_present(foin: &str, mot: &str) -> bool {
    let borne = |c: Option<char>| !matches!(c, Some(c) if c.is_alphanumeric() || c == '_');
    let mut depuis = 0;
    while let Some(i) = foin[depuis..].find(mot) {
        let deb = depuis + i;
        let fin = deb + mot.len();
        if borne(foin[..deb].chars().next_back()) && borne(foin[fin..].chars().next()) {
            return true;
        }
        depuis = fin;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn une_lecture_non_filtree_est_signalee() {
        let r = requete_non_cloisonnee("SELECT id, nom FROM client WHERE actif = 1");
        assert!(r.is_some());
        assert!(r.unwrap().contains("client"), "le reproche nomme la table");
    }

    #[test]
    fn une_lecture_filtree_passe() {
        assert!(requete_non_cloisonnee(
            "SELECT id FROM client WHERE actif = 1 AND dossier_id = ?1"
        )
        .is_none());
    }

    #[test]
    fn une_table_non_cloisonnee_ne_concerne_personne() {
        assert!(requete_non_cloisonnee("SELECT nom FROM utilisateur").is_none());
        assert!(requete_non_cloisonnee("SELECT * FROM poste WHERE actif = 1").is_none());
    }

    #[test]
    fn un_nom_de_colonne_ne_compte_pas_pour_une_table() {
        // `client_id` n'est pas `client`.
        assert!(requete_non_cloisonnee(
            "SELECT client_id FROM session_reseau WHERE id = ?1"
        )
        .is_none());
    }

    #[test]
    fn un_mot_dans_un_texte_ne_compte_pas() {
        assert!(requete_non_cloisonnee(
            "INSERT INTO config_app (cle, valeur) VALUES ('client', 'avoir')"
        )
        .is_none());
    }

    #[test]
    fn le_ddl_echappe_au_detecteur() {
        assert!(requete_non_cloisonnee("CREATE TABLE client (id TEXT)").is_none());
        assert!(requete_non_cloisonnee("ALTER TABLE vente ADD COLUMN x TEXT").is_none());
    }

    #[test]
    fn le_reproche_nomme_toutes_les_tables_en_cause() {
        let r = requete_non_cloisonnee(
            "SELECT v.id FROM vente v JOIN ligne_vente l ON l.vente_id = v.id",
        )
        .expect("doit être signalée");
        assert!(r.contains("vente") && r.contains("ligne_vente"));
    }

    #[test]
    fn la_cle_du_compteur_porte_le_dossier() {
        let a = cle_compteur("dossier-a", "FAC-2026");
        let b = cle_compteur("dossier-b", "FAC-2026");
        assert_ne!(a, b, "deux sociétés ne partagent pas une suite de numéros");
    }
}

// =====================================================================
//  LA DATE DE TRAVAIL
// =====================================================================

/// Un dossier : une societe, un exercice, et la faculte de le
/// prolonger.
#[derive(Debug, Clone)]
pub struct Dossier {
    pub id: String,
    pub code: String,
    pub societe: String,
    pub exercice_debut: String,
    pub exercice_fin: String,
    /// Une fin repoussee, sans toucher a la fin d'origine.
    ///
    /// Deux champs et non un seul : ecraser `exercice_fin` ferait
    /// perdre la duree reelle de l'exercice, celle qui figure sur les
    /// etats. On veut savoir qu'il a ete prolonge, pas l'oublier.
    pub prolonge_jusqu_au: Option<String>,
    pub clos: bool,
}

impl Dossier {
    /// La derniere date acceptee, prolongation comprise.
    pub fn fin_effective(&self) -> &str {
        match &self.prolonge_jusqu_au {
            Some(p) if p.as_str() > self.exercice_fin.as_str() => p,
            _ => &self.exercice_fin,
        }
    }

    /// Ce dossier accepte-t-il une ecriture a cette date ?
    ///
    /// Le refus porte la raison ET la borne. « Date hors exercice » tout
    /// seul oblige le commercant a deviner de quel cote il deborde, et
    /// il corrigera au hasard.
    pub fn accepte(&self, date: &str) -> Result<(), String> {
        if self.clos {
            return Err(format!(
                "Le dossier « {} » est clos : on n'y écrit plus.",
                self.code
            ));
        }
        let jour = &date[..date.len().min(10)];
        if jour < self.exercice_debut.as_str() {
            return Err(format!(
                "Le {jour} précède l'exercice, qui commence le {}.",
                self.exercice_debut
            ));
        }
        if jour > self.fin_effective() {
            let fin = self.fin_effective();
            return Err(if self.prolonge_jusqu_au.is_some() {
                format!("Le {jour} dépasse l'exercice, prolongé jusqu'au {fin}.")
            } else {
                format!(
                    "Le {jour} dépasse l'exercice, qui finit le {fin}. \
                     Le prolonger, ou ouvrir l'exercice suivant."
                )
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests_exercice {
    use super::*;

    fn dossier_2026() -> Dossier {
        Dossier {
            id: DOSSIER_DEFAUT.to_string(),
            code: "PRINCIPAL".into(),
            societe: "Ma boutique".into(),
            exercice_debut: "2026-01-01".into(),
            exercice_fin: "2026-12-31".into(),
            prolonge_jusqu_au: None,
            clos: false,
        }
    }

    #[test]
    fn une_date_dans_l_exercice_passe() {
        assert!(dossier_2026().accepte("2026-09-11").is_ok());
    }

    #[test]
    fn les_deux_bornes_sont_incluses() {
        let d = dossier_2026();
        assert!(d.accepte("2026-01-01").is_ok(), "le premier jour compte");
        assert!(d.accepte("2026-12-31").is_ok(), "le dernier jour aussi");
    }

    #[test]
    fn une_date_horodatee_se_compare_au_jour() {
        assert!(dossier_2026().accepte("2026-12-31T23:59:59.999").is_ok());
    }

    #[test]
    fn avant_l_exercice_le_refus_nomme_le_debut() {
        let e = dossier_2026().accepte("2025-12-31").unwrap_err();
        assert!(e.contains("2026-01-01"), "le refus dit jusqu'où reculer");
    }

    #[test]
    fn apres_l_exercice_le_refus_propose_la_suite() {
        let e = dossier_2026().accepte("2027-01-01").unwrap_err();
        assert!(e.contains("2026-12-31"));
        assert!(e.contains("prolonger"), "le refus dit quoi faire");
    }

    #[test]
    fn la_prolongation_repousse_la_fin_sans_l_effacer() {
        let mut d = dossier_2026();
        d.prolonge_jusqu_au = Some("2027-03-31".into());
        assert!(d.accepte("2027-02-15").is_ok());
        assert_eq!(d.exercice_fin, "2026-12-31", "la fin d'origine est gardée");
    }

    #[test]
    fn une_prolongation_plus_courte_ne_raccourcit_rien() {
        // Saisie a l'envers : elle ne doit pas fermer l'exercice plus
        // tot que prevu.
        let mut d = dossier_2026();
        d.prolonge_jusqu_au = Some("2026-06-30".into());
        assert!(d.accepte("2026-11-02").is_ok());
    }

    #[test]
    fn au_dela_de_la_prolongation_c_est_refuse() {
        let mut d = dossier_2026();
        d.prolonge_jusqu_au = Some("2027-03-31".into());
        let e = d.accepte("2027-04-01").unwrap_err();
        assert!(e.contains("2027-03-31"));
    }

    #[test]
    fn un_dossier_clos_refuse_meme_une_date_valide() {
        let mut d = dossier_2026();
        d.clos = true;
        assert!(d.accepte("2026-09-11").is_err());
    }
}
