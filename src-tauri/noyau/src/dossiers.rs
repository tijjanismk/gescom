//! Le cloisonnement par dossier.
//!
//! Un dossier = **une societe**, comme chez Ciel. Il n'est PAS une
//! annee : il a UN stock continu et PLUSIEURS **exercices** qui se
//! suivent, chacun avec sa date de debut, sa date de fin, et une
//! prolongation possible. Voir `Dossier` (l'identite) et `Exercice` (la
//! periode) plus bas — melanger les deux dans une seule struct aurait
//! force un dossier a n'avoir qu'une seule periode a la fois.
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
//! `session_reseau`. Un caissier qui change de
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

/// Le declencheur qui fait suivre le stock aux mouvements, sur SQLite
/// (v3, D-1 — ETAPES item 9).
///
/// L'ancien ne posait pas `dossier_id` : la ligne de stock d'un
/// magasin de `dossier-b` naissait avec la valeur par defaut de la
/// colonne — le dossier d'origine — et `dossier-b` ne voyait jamais
/// son stock. Sur PostgreSQL le defaut suivait la session ; on ne s'y
/// fie plus non plus : le dossier est celui du MOUVEMENT.
///
/// `DROP` puis `CREATE` : un `CREATE ... IF NOT EXISTS` aurait garde
/// l'ancien corps sur toutes les bases deja installees.
pub const DECLENCHEUR_STOCK_SQLITE: &str = "
DROP TRIGGER IF EXISTS stock_suit_les_mouvements;
CREATE TRIGGER stock_suit_les_mouvements
AFTER INSERT ON mouvement_stock
BEGIN
  INSERT INTO stock_depot (id, article_id, depot_id, quantite, dossier_id)
  VALUES (lower(hex(randomblob(16))), NEW.article_id, NEW.depot_id,
          NEW.quantite_delta, NEW.dossier_id)
  ON CONFLICT(article_id, depot_id)
  DO UPDATE SET quantite = quantite + NEW.quantite_delta;
END;";

/// Les lignes de stock posees dans le mauvais dossier par l'ancien
/// declencheur reviennent au dossier de leur magasin. Rejouable : ne
/// touche que ce qui est faux.
pub const REPARER_STOCK_DOSSIER: &str = "
UPDATE stock_depot
SET dossier_id = (SELECT d.dossier_id FROM depot d WHERE d.id = stock_depot.depot_id)
WHERE EXISTS (SELECT 1 FROM depot d
              WHERE d.id = stock_depot.depot_id AND d.dossier_id <> stock_depot.dossier_id)";

/// Les tables dont chaque ligne appartient a un dossier.
///
/// Cette liste est la reference : le DDL la parcourt pour poser les
/// colonnes, et le detecteur la parcourt pour reperer les requetes non
/// filtrees. Une table ajoutee ici est cloisonnee partout a la fois.
pub const TABLES_CLOISONNEES: &[&str] = &[
    "exercice",
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
    "anomalie_vue",
    "affectation_comptable",
    "employe",
    "presence",
    "avance",
    "fiche_paie",
    "ligne_paie",
    "versement_paie",
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
//  L'IDENTITE DU DOSSIER, ET SES EXERCICES
// =====================================================================
//
// Chapitre 4 du plan : un dossier n'est PAS une annee. Il a UN stock,
// continu, et PLUSIEURS exercices qui se suivent. Melanger les deux
// dans une seule struct aurait force un dossier a n'avoir qu'une seule
// periode — exactement le piege que le plan nomme.

/// Un dossier : une societe. Rien qui date — ca, c'est l'exercice.
#[derive(Debug, Clone)]
pub struct Dossier {
    pub id: String,
    pub code: String,
    pub societe: String,
    /// Le dossier entier a cesse toute activite. Plus grossier que la
    /// clôture d'un exercice : un dossier clos n'ouvre plus de nouvel
    /// exercice non plus.
    pub clos: bool,
}

/// Une periode comptable a l'interieur d'un dossier — en general une
/// annee.
#[derive(Debug, Clone)]
pub struct Exercice {
    pub id: String,
    pub dossier_id: String,
    pub date_debut: String,
    pub date_fin: String,
    /// Une fin repoussee, sans toucher a la fin d'origine.
    ///
    /// Deux champs et non un seul : ecraser `date_fin` ferait perdre la
    /// duree reelle de l'exercice, celle qui figure sur les etats. On
    /// veut savoir qu'il a ete prolonge, pas l'oublier.
    pub prolonge_jusqu_au: Option<String>,
    pub clos: bool,
}

impl Exercice {
    /// La derniere date acceptee, prolongation comprise.
    pub fn fin_effective(&self) -> &str {
        match &self.prolonge_jusqu_au {
            Some(p) if p.as_str() > self.date_fin.as_str() => p,
            _ => &self.date_fin,
        }
    }

    /// Le jour tombe-t-il dans cet exercice, prolongation comprise ?
    ///
    /// Ne dit rien de `clos` : un exercice clos couvre toujours les
    /// memes dates, il refuse juste d'y ecrire (voir `accepte`).
    pub fn couvre(&self, date: &str) -> bool {
        let jour = &date[..date.len().min(10)];
        jour >= self.date_debut.as_str() && jour <= self.fin_effective()
    }

    /// Cet exercice accepte-t-il une ecriture a cette date ?
    ///
    /// Le refus porte la raison ET la borne. « Date hors exercice » tout
    /// seul oblige le commercant a deviner de quel cote il deborde, et
    /// il corrigera au hasard.
    pub fn accepte(&self, date: &str) -> Result<(), String> {
        use crate::coeur::dates::en_lettres;
        let jour = &date[..date.len().min(10)];
        if self.clos {
            return Err(format!(
                "Les dates de travail du {} au {} sont closes : on n'y écrit plus.",
                en_lettres(&self.date_debut),
                en_lettres(self.fin_effective())
            ));
        }
        if jour < self.date_debut.as_str() {
            return Err(format!(
                "Le {} est avant les dates de travail (à partir du {}).",
                en_lettres(jour),
                en_lettres(&self.date_debut)
            ));
        }
        if jour > self.fin_effective() {
            // D21 : le refus dit la borne ET quoi faire.
            return Err(format!(
                "Le {} est hors des dates de travail (jusqu'au {}{}). \
                 Prolonger l'exercice ou en ouvrir un nouveau.",
                en_lettres(jour),
                en_lettres(self.fin_effective()),
                if self.prolonge_jusqu_au.is_some() { ", prolongation comprise" } else { "" }
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests_exercice {
    use super::*;

    fn exercice_2026() -> Exercice {
        Exercice {
            id: "ex-2026".into(),
            dossier_id: DOSSIER_DEFAUT.to_string(),
            date_debut: "2026-01-01".into(),
            date_fin: "2026-12-31".into(),
            prolonge_jusqu_au: None,
            clos: false,
        }
    }

    #[test]
    fn une_date_dans_l_exercice_passe() {
        assert!(exercice_2026().accepte("2026-09-11").is_ok());
    }

    #[test]
    fn les_deux_bornes_sont_incluses() {
        let e = exercice_2026();
        assert!(e.accepte("2026-01-01").is_ok(), "le premier jour compte");
        assert!(e.accepte("2026-12-31").is_ok(), "le dernier jour aussi");
    }

    #[test]
    fn une_date_horodatee_se_compare_au_jour() {
        assert!(exercice_2026().accepte("2026-12-31T23:59:59.999").is_ok());
    }

    #[test]
    fn avant_l_exercice_le_refus_nomme_le_debut() {
        let e = exercice_2026().accepte("2025-12-31").unwrap_err();
        assert!(e.contains("1er janvier 2026"), "le refus dit jusqu'où reculer : {e}");
    }

    #[test]
    fn apres_l_exercice_le_refus_propose_la_suite() {
        let e = exercice_2026().accepte("2027-01-01").unwrap_err();
        assert_eq!(
            e,
            "Le 1er janvier 2027 est hors des dates de travail (jusqu'au 31 décembre 2026). \
             Prolonger l'exercice ou en ouvrir un nouveau."
        );
    }

    #[test]
    fn la_prolongation_repousse_la_fin_sans_l_effacer() {
        let mut e = exercice_2026();
        e.prolonge_jusqu_au = Some("2027-03-31".into());
        assert!(e.accepte("2027-02-15").is_ok());
        assert_eq!(e.date_fin, "2026-12-31", "la fin d'origine est gardée");
    }

    #[test]
    fn une_prolongation_plus_courte_ne_raccourcit_rien() {
        // Saisie a l'envers : elle ne doit pas fermer l'exercice plus
        // tot que prevu.
        let mut e = exercice_2026();
        e.prolonge_jusqu_au = Some("2026-06-30".into());
        assert!(e.accepte("2026-11-02").is_ok());
    }

    #[test]
    fn au_dela_de_la_prolongation_c_est_refuse() {
        let mut e = exercice_2026();
        e.prolonge_jusqu_au = Some("2027-03-31".into());
        let err = e.accepte("2027-04-01").unwrap_err();
        assert!(err.contains("31 mars 2027, prolongation comprise"), "{err}");
    }

    #[test]
    fn un_exercice_clos_refuse_meme_une_date_valide() {
        let mut e = exercice_2026();
        e.clos = true;
        assert!(e.accepte("2026-09-11").is_err());
    }

    #[test]
    fn couvre_ne_regarde_pas_la_cloture() {
        // `couvre` sert a TROUVER l'exercice d'une date parmi plusieurs ;
        // `accepte` sert a savoir si on peut y ecrire. Les deux
        // questions sont distinctes : un exercice clos couvre toujours
        // ses dates, il refuse juste d'y ecrire.
        let mut e = exercice_2026();
        e.clos = true;
        assert!(e.couvre("2026-09-11"));
        assert!(e.accepte("2026-09-11").is_err());
    }
}

// =====================================================================
//  RETROUVER L'EXERCICE D'UNE DATE
// =====================================================================

/// Parmi plusieurs exercices d'un MEME dossier, celui qui couvre cette
/// date.
///
/// Pure : ne lit rien, ne decide que du choix parmi une liste deja lue.
/// C'est ce qui la rend testable sans base, et reutilisable telle
/// quelle le jour ou l'ecran des exercices affiche « exercice en cours
/// pour telle date ».
pub fn exercice_pour_date<'a>(exercices: &'a [Exercice], date: &str) -> Option<&'a Exercice> {
    exercices.iter().find(|e| e.couvre(date))
}

/// Ce dossier accepte-t-il une ecriture a cette date, vu ses exercices ?
///
/// Absent des deux cas d'`Exercice::accepte` : aucun exercice ne
/// couvre la date. Un dossier neuf, ou une date posterieure au dernier
/// exercice ouvert, tombe ici — le message doit dire qu'il faut ouvrir
/// un exercice, pas laisser croire a une faute de saisie.
pub fn dossier_accepte(exercices: &[Exercice], date: &str) -> Result<(), String> {
    match exercice_pour_date(exercices, date) {
        Some(e) => e.accepte(date),
        None => {
            let jour = &date[..date.len().min(10)];
            // D21 : le cas courant est une date APRES la fin des dates de
            // travail — le refus nomme cette fin et dit quoi faire, comme
            // `accepte`. Un dernier exercice clos : il faut en ouvrir un.
            let dernier = exercices
                .iter()
                .filter(|e| e.fin_effective() < jour)
                .max_by(|a, b| a.fin_effective().cmp(b.fin_effective()));
            // Avant le tout premier exercice : le refus nomme le debut.
            let premier = exercices.iter().min_by(|a, b| a.date_debut.cmp(&b.date_debut));
            if let Some(p) = premier.filter(|p| jour < p.date_debut.as_str()) {
                return p.accepte(date);
            }
            match dernier {
                Some(e) if !e.clos => e.accepte(date),
                _ => Err(format!(
                    "Le {} ne tombe dans aucun exercice ouvert pour ce dossier. \
                     Ouvrir un exercice qui le couvre.",
                    crate::coeur::dates::en_lettres(jour)
                )),
            }
        }
    }
}

/// Le jour ou commence l'exercice suivant : le lendemain de la fin
/// (prolongation comprise) du dernier. `None` sans exercice. D21 : un
/// dossier n'a jamais de trou.
pub fn debut_suivant(exercices: &[Exercice]) -> Option<String> {
    let fin = exercices.iter().map(|e| e.fin_effective()).max()?;
    let jour = crate::coeur::dates::jour(fin)?;
    Some((jour + chrono::Duration::days(1)).format("%Y-%m-%d").to_string())
}

/// Les ecritures DATEES, et le parametre qui porte leur date (v3, D-4).
/// Absent ou vide, c'est aujourd'hui. Le serveur les juge contre les
/// dates de travail du dossier (`verifier_date_sur`) avant de les
/// executer. Ni les reglages, ni les comptes, ni l'ouverture ou la
/// fermeture de la caisse : on peut toujours fermer son tiroir.
pub fn date_d_ecriture(commande: &str) -> Option<&'static [&'static str]> {
    const AUJOURDHUI: &[&str] = &[];
    match commande {
        "creer_vente" => Some(&["dateVente", "date_vente"]),
        "creer_piece" | "modifier_piece" | "creer_piece_fournisseur" => Some(&["datePiece", "date_piece"]),
        "regler_creance" | "regler_dette_fournisseur" => Some(&["datePaiement", "date_paiement"]),
        "enregistrer_achat" => Some(&["dateReception", "date_reception"]),
        "accorder_avoir_client" | "annuler_facture_fournisseur_par_avoir" | "annuler_facture_par_avoir"
        | "annuler_paiement_fournisseur" | "annuler_piece" | "annuler_reglement" | "appliquer_avoir_vente"
        | "changer_statut_cheque" | "convertir_commande_en_livraison_et_facture" | "convertir_piece"
        | "creer_facture_depuis_vente" | "dupliquer_piece" | "enregistrer_ajustement_inventaire" | "enregistrer_cheque"
        | "enregistrer_depense" | "enregistrer_entree_stock" | "enregistrer_livraison"
        | "enregistrer_paiement" | "enregistrer_retour" | "enregistrer_retour_fournisseur"
        | "enregistrer_retour_sans_facture" | "enregistrer_transfert" | "marquer_irrecouvrable"
        | "modifier_depense" | "modifier_facture_pos" | "regler_creance_exceptionnel"
        | "rembourser_avoir" | "solder_residus_creances"
        | "valider_facture" | "valider_facture_credit" | "valider_facture_fournisseur"
        | "donner_avance" | "annuler_avance" | "valider_fiche_paie" | "verser_paie" => Some(AUJOURDHUI),
        _ => None,
    }
}

#[cfg(test)]
mod tests_debut_suivant {
    use super::*;

    fn ex(debut: &str, fin: &str, prolonge: Option<&str>) -> Exercice {
        Exercice {
            id: debut.into(),
            dossier_id: DOSSIER_DEFAUT.into(),
            date_debut: debut.into(),
            date_fin: fin.into(),
            prolonge_jusqu_au: prolonge.map(str::to_string),
            clos: false,
        }
    }

    #[test]
    fn le_suivant_commence_le_lendemain_prolongation_comprise() {
        assert_eq!(debut_suivant(&[]), None);
        assert_eq!(debut_suivant(&[ex("2026-03-01", "2027-02-28", None)]).as_deref(), Some("2027-03-01"));
        assert_eq!(
            debut_suivant(&[ex("2026-01-01", "2026-12-31", Some("2027-03-31"))]).as_deref(),
            Some("2027-04-01")
        );
    }

    #[test]
    fn les_ecritures_datees_sont_nommees_et_les_reglages_non() {
        assert_eq!(date_d_ecriture("creer_vente"), Some(&["dateVente", "date_vente"][..]));
        assert_eq!(date_d_ecriture("enregistrer_depense"), Some(&[][..]));
        assert_eq!(date_d_ecriture("fermer_session_caisse"), None, "on ferme toujours son tiroir");
        assert_eq!(date_d_ecriture("creer_client_rapide"), None);
    }
}

#[cfg(test)]
mod tests_dossier_accepte {
    use super::*;

    fn deux_exercices() -> Vec<Exercice> {
        vec![
            Exercice {
                id: "ex-2025".into(),
                dossier_id: DOSSIER_DEFAUT.to_string(),
                date_debut: "2025-01-01".into(),
                date_fin: "2025-12-31".into(),
                prolonge_jusqu_au: None,
                clos: true,
            },
            Exercice {
                id: "ex-2026".into(),
                dossier_id: DOSSIER_DEFAUT.to_string(),
                date_debut: "2026-01-01".into(),
                date_fin: "2026-12-31".into(),
                prolonge_jusqu_au: None,
                clos: false,
            },
        ]
    }

    #[test]
    fn choisit_l_exercice_qui_couvre_la_date() {
        assert!(dossier_accepte(&deux_exercices(), "2026-09-11").is_ok());
    }

    #[test]
    fn un_exercice_clos_refuse_meme_choisi() {
        let err = dossier_accepte(&deux_exercices(), "2025-06-15").unwrap_err();
        assert!(err.contains("clos"));
    }

    #[test]
    fn apres_les_dates_de_travail_le_refus_nomme_la_fin_et_dit_quoi_faire() {
        let err = dossier_accepte(&deux_exercices(), "2027-01-05").unwrap_err();
        assert_eq!(
            err,
            "Le 5 janvier 2027 est hors des dates de travail (jusqu'au 31 décembre 2026). \
             Prolonger l'exercice ou en ouvrir un nouveau."
        );
    }

    #[test]
    fn apres_un_dernier_exercice_clos_il_faut_en_ouvrir_un() {
        let mut ex = deux_exercices();
        ex[1].clos = true;
        let err = dossier_accepte(&ex, "2027-01-05").unwrap_err();
        assert!(err.contains("aucun exercice ouvert"), "{err}");
    }

    #[test]
    fn un_dossier_sans_aucun_exercice_le_dit_aussi() {
        let err = dossier_accepte(&[], "2026-09-11").unwrap_err();
        assert!(err.contains("aucun exercice"));
    }
}

// =====================================================================
//  LES EXERCICES, EN BASE
// =====================================================================
//
// Sur l'un ou l'autre moteur. Le serveur appelle `verifier_date_sur`
// avant chaque ecriture datee (`date_d_ecriture`, v3 D-4) ; l'ecran
// Parametres -> Dossiers ouvre, prolonge et clot.

use crate::base::Base;
use crate::parametres;

fn lire_les_exercices(base: &mut Base, dossier_id: &str) -> Result<Vec<Exercice>, String> {
    base.lire_plusieurs(
        "SELECT id, dossier_id, date_debut, date_fin, prolonge_jusqu_au, clos
         FROM exercice WHERE dossier_id = ?1 ORDER BY date_debut ASC",
        &parametres![dossier_id],
        |r| {
            Ok(Exercice {
                id: r.get::<String>(0)?,
                dossier_id: r.get::<String>(1)?,
                date_debut: r.get::<String>(2)?,
                date_fin: r.get::<String>(3)?,
                prolonge_jusqu_au: r.get::<Option<String>>(4)?,
                clos: r.get::<i64>(5)? != 0,
            })
        },
    )
    .map_err(|e| e.0)
}

/// Les exercices du dossier courant, du plus ancien au plus recent.
pub fn lire_exercices_sur(base: &mut Base) -> Result<Vec<serde_json::Value>, String> {
    let dossier = base.dossier().to_string();
    let exercices = lire_les_exercices(base, &dossier)?;
    Ok(exercices
        .into_iter()
        .map(|e| {
            serde_json::json!({
                "id": e.id,
                "dossier_id": e.dossier_id,
                "date_debut": e.date_debut,
                "date_fin": e.date_fin,
                "fin_effective": e.fin_effective(),
                "prolonge_jusqu_au": e.prolonge_jusqu_au,
                "clos": e.clos,
            })
        })
        .collect())
}

/// Le dossier courant accepte-t-il une ecriture a cette date ?
///
/// C'est LE garde-fou a appeler avant d'ecrire une piece, une vente, un
/// mouvement — le pendant du detecteur de cloisonnement, mais pour le
/// TEMPS plutot que la SOCIETE : une ecriture tombee hors de tout
/// exercice ouvert est tout aussi mal rangee qu'une ecriture tombee
/// dans le mauvais dossier, et tout aussi silencieuse si personne ne la
/// refuse.
pub fn verifier_date_sur(base: &mut Base, date: &str) -> Result<(), String> {
    let dossier = base.dossier().to_string();
    let exercices = lire_les_exercices(base, &dossier)?;
    dossier_accepte(&exercices, date)
}

/// Ouvre un exercice pour le dossier courant.
///
/// Refuse un chevauchement avec un exercice existant : deux exercices
/// qui se recouvrent rendraient `exercice_pour_date` arbitraire — il
/// choisirait le premier trouve dans l'ordre de lecture, en silence.
pub fn ouvrir_exercice_sur(
    base: &mut Base,
    date_debut: String,
    date_fin: String,
) -> Result<serde_json::Value, String> {
    let (date_debut, date_fin) = dates_de_travail(Some(&date_debut), Some(&date_fin), &date_debut)?;
    let dossier = base.dossier().to_string();
    let existants = lire_les_exercices(base, &dossier)?;
    // D21 : pas de trou — le suivant commence le lendemain du dernier.
    if let Some(attendu) = debut_suivant(&existants) {
        if date_debut > attendu {
            return Err(format!(
                "Un exercice commence le lendemain du précédent : le {}. Pas de trou dans les dates de travail.",
                crate::coeur::dates::en_lettres(&attendu)
            ));
        }
    }
    if let Some(chevauche) = existants
        .iter()
        .find(|e| {
            date_debut.as_str() <= e.fin_effective() && date_fin.as_str() >= e.date_debut.as_str()
        })
    {
        return Err(format!(
            "Chevauche l'exercice du {} au {}.",
            crate::coeur::dates::en_lettres(&chevauche.date_debut),
            crate::coeur::dates::en_lettres(chevauche.fin_effective())
        ));
    }

    let id = uuid::Uuid::new_v4().to_string();
    let now = crate::utils::maintenant_iso();
    let mut tx = base.transaction().map_err(|e| e.0)?;
    tx.executer(
        "INSERT INTO exercice
           (id, dossier_id, date_debut, date_fin, clos, cree_le, modifie_le)
         VALUES (?1,?2,?3,?4,0,?5,?5)",
        &parametres![id.clone(), dossier, date_debut.clone(), date_fin.clone(), now],
    )
    .map_err(|e| e.0)?;
    noter_exercice(
        &mut tx,
        "exercice_ouvert",
        &id,
        None,
        serde_json::json!({ "du": date_debut, "au": date_fin }),
    )?;
    tx.valider().map_err(|e| e.0)?;

    Ok(serde_json::json!({
        "id": id, "date_debut": date_debut, "date_fin": date_fin, "clos": false
    }))
}

/// L'exercice `id` du dossier courant, ou un refus qui le dit.
///
/// Le filtre de dossier compte ici autant que l'identifiant : sans lui,
/// un identifiant devine ou vole permettrait de prolonger ou de clore
/// l'exercice d'une autre societe.
fn exercice_du_dossier(base: &mut Base, exercice_id: &str) -> Result<(Exercice, Vec<Exercice>), String> {
    let dossier = base.dossier().to_string();
    let tous = lire_les_exercices(base, &dossier)?;
    let ex = tous
        .iter()
        .find(|e| e.id == exercice_id)
        .cloned()
        .ok_or_else(|| "Exercice introuvable.".to_string())?;
    Ok((ex, tous))
}

/// Au journal : qui a ouvert, prolonge, clos quel exercice (v3, D-4).
fn noter_exercice(
    acces: &mut impl crate::base::Acces,
    type_evenement: &str,
    exercice_id: &str,
    ancien: Option<serde_json::Value>,
    nouveau: serde_json::Value,
) -> Result<(), String> {
    let auteur = crate::argent::id_utilisateur_courant_sur(acces);
    let dossier = acces.dossier().to_string();
    acces
        .executer(
            "INSERT INTO journal
               (id, type_evenement, entite_type, entite_id, auteur_id,
                ancien_valeur, nouveau_valeur, origine, date_evenement, dossier_id)
             VALUES (?1, ?2, 'exercice', ?3, ?4, ?5, ?6, 'app', ?7, ?8)",
            &parametres![
                uuid::Uuid::new_v4().to_string(),
                type_evenement,
                exercice_id,
                auteur,
                ancien.map(|v| v.to_string()),
                nouveau.to_string(),
                crate::utils::maintenant_iso(),
                dossier
            ],
        )
        .map(|_| ())
        .map_err(|e| e.0)
}

/// Repousse la fin d'un exercice DU DOSSIER COURANT.
///
/// On prolonge un exercice ouvert, vers l'avant, et jamais jusque dans
/// le suivant : deux exercices qui se recouvrent rendraient le choix de
/// `exercice_pour_date` arbitraire.
pub fn prolonger_exercice_sur(
    base: &mut Base,
    exercice_id: String,
    prolonge_jusqu_au: String,
) -> Result<(), String> {
    let (ex, tous) = exercice_du_dossier(base, &exercice_id)?;
    if ex.clos {
        return Err("Cet exercice est clos : on ne le prolonge plus.".to_string());
    }
    let jusqu_au = crate::coeur::dates::jour(prolonge_jusqu_au.trim())
        .map(|d| d.format("%Y-%m-%d").to_string())
        .ok_or_else(|| format!("Date illisible : « {} »", prolonge_jusqu_au.trim()))?;
    if jusqu_au.as_str() <= ex.fin_effective() {
        return Err(format!(
            "L'exercice va déjà jusqu'au {} : une prolongation va plus loin.",
            crate::coeur::dates::en_lettres(ex.fin_effective())
        ));
    }
    if let Some(suivant) = tous.iter().find(|e| e.date_debut > ex.date_debut) {
        if jusqu_au >= suivant.date_debut {
            return Err(format!(
                "L'exercice suivant commence le {} : la prolongation doit s'arrêter avant.",
                crate::coeur::dates::en_lettres(&suivant.date_debut)
            ));
        }
    }
    let now = crate::utils::maintenant_iso();
    let dossier = base.dossier().to_string();
    let mut tx = base.transaction().map_err(|e| e.0)?;
    tx.executer(
        "UPDATE exercice SET prolonge_jusqu_au = ?1, modifie_le = ?2
         WHERE id = ?3 AND dossier_id = ?4",
        &parametres![jusqu_au.clone(), now, exercice_id.clone(), dossier],
    )
    .map_err(|e| e.0)?;
    noter_exercice(
        &mut tx,
        "exercice_prolonge",
        &exercice_id,
        Some(serde_json::json!({ "jusqu_au": ex.fin_effective() })),
        serde_json::json!({ "jusqu_au": jusqu_au }),
    )?;
    tx.valider().map_err(|e| e.0)
}

/// Clot un exercice DU DOSSIER COURANT. On n'y ecrit plus ensuite, et
/// on ne le rouvre pas : c'est ce que le comptable a arrete.
pub fn clore_exercice_sur(base: &mut Base, exercice_id: String) -> Result<(), String> {
    let (ex, _) = exercice_du_dossier(base, &exercice_id)?;
    if ex.clos {
        return Err("Cet exercice est déjà clos.".to_string());
    }
    let now = crate::utils::maintenant_iso();
    let dossier = base.dossier().to_string();
    let mut tx = base.transaction().map_err(|e| e.0)?;
    tx.executer(
        "UPDATE exercice SET clos = 1, modifie_le = ?1 WHERE id = ?2 AND dossier_id = ?3",
        &parametres![now, exercice_id.clone(), dossier],
    )
    .map_err(|e| e.0)?;
    noter_exercice(
        &mut tx,
        "exercice_clos",
        &exercice_id,
        None,
        serde_json::json!({ "du": ex.date_debut, "au": ex.fin_effective() }),
    )?;
    tx.valider().map_err(|e| e.0)
}

// =====================================================================
//  LES DOSSIERS, EN BASE — la v3 commence ici
// =====================================================================
//
// Un dossier = une societe. Le plan (AI_CONTEXT/PLAN-MULTISOCIETE.md,
// decisions 3 et 4) : on choisit son dossier A LA CONNEXION, on en
// change en se deconnectant, et un dossier neuf nait avec ce qu'il lui
// faut pour vendre — un magasin par defaut et un client de passage —
// sinon la premiere vente echoue sur un « aucun depot » que personne
// ne comprend.

/// Les dates de travail d'un dossier neuf (D21) : donnees, ou l'annee
/// civile d'`aujourd_hui` par defaut. Lisibles, et la fin apres le
/// debut. Pure : la regle se teste sans horloge.
pub fn dates_de_travail(
    debut: Option<&str>,
    fin: Option<&str>,
    aujourd_hui: &str,
) -> Result<(String, String), String> {
    let annee = &aujourd_hui[..4.min(aujourd_hui.len())];
    let lire = |v: Option<&str>, defaut: String| -> Result<String, String> {
        match v.map(str::trim).filter(|v| !v.is_empty()) {
            None => Ok(defaut),
            Some(t) => crate::coeur::dates::jour(t)
                .map(|d| d.format("%Y-%m-%d").to_string())
                .ok_or_else(|| format!("Date illisible : « {t} »")),
        }
    };
    let d = lire(debut, format!("{annee}-01-01"))?;
    let f = lire(fin, format!("{annee}-12-31"))?;
    if f < d {
        return Err("La fin des dates de travail précède leur début.".to_string());
    }
    Ok((d, f))
}

#[cfg(test)]
mod tests_dates_de_travail {
    use super::dates_de_travail;

    #[test]
    fn par_defaut_l_annee_civile() {
        assert_eq!(
            dates_de_travail(None, Some(" "), "2026-03-15T10:00:00").unwrap(),
            ("2026-01-01".to_string(), "2026-12-31".to_string())
        );
    }

    #[test]
    fn des_dates_donnees_se_gardent_et_se_jugent() {
        assert_eq!(
            dates_de_travail(Some("2026-03-01"), Some("2027-02-28"), "2026-03-15").unwrap(),
            ("2026-03-01".to_string(), "2027-02-28".to_string())
        );
        assert!(dates_de_travail(Some("2026-03-01"), Some("2026-02-01"), "2026-03-15").is_err());
        assert!(dates_de_travail(Some("mars"), None, "2026-03-15").unwrap_err().contains("illisible"));
    }
}

/// Le code d'un dossier : ce qu'on tape, ce qui figure dans les
/// compteurs de numerotation. Majuscules, chiffres, tiret, souligne.
fn valider_code_dossier(code: &str) -> Result<String, String> {
    let code = code.trim().to_uppercase();
    if code.is_empty() || code.len() > 20 {
        return Err("Le code du dossier : 1 a 20 caracteres.".to_string());
    }
    if !code.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err("Le code du dossier : lettres, chiffres, tiret ou souligne, sans accent.".to_string());
    }
    Ok(code)
}

/// Tous les dossiers, les ouverts d'abord. Pas de filtre de dossier :
/// c'est la liste qu'on lit AVANT d'en avoir choisi un.
pub fn lire_dossiers_sur(base: &mut Base) -> Result<Vec<serde_json::Value>, String> {
    base.lire_plusieurs(
        "SELECT d.id, d.code, d.societe, d.clos,
                (SELECT COUNT(*) FROM exercice e WHERE e.dossier_id = d.id AND e.clos = 0)
         FROM dossier d
         ORDER BY d.clos ASC, d.code ASC",
        &[],
        |r| {
            Ok(serde_json::json!({
                "id": r.get::<String>(0)?,
                "code": r.get::<String>(1)?,
                "societe": r.get::<String>(2)?,
                "clos": r.get::<i64>(3)? != 0,
                "exercices_ouverts": r.get::<i64>(4)?,
            }))
        },
    )
    .map_err(|e| e.0)
}

/// Un dossier ouvert, ou le refus qui dit pourquoi.
pub fn dossier_ouvert_sur(base: &mut Base, dossier_id: &str) -> Result<Dossier, String> {
    let d = base
        .lire_une(
            "SELECT id, code, societe, clos FROM dossier WHERE id = ?1",
            &parametres![dossier_id],
            |r| {
                Ok(Dossier {
                    id: r.get::<String>(0)?,
                    code: r.get::<String>(1)?,
                    societe: r.get::<String>(2)?,
                    clos: r.get::<i64>(3)? != 0,
                })
            },
        )
        .map_err(|e| e.0)?
        .ok_or_else(|| "Dossier introuvable.".to_string())?;
    if d.clos {
        return Err(format!("Le dossier « {} » est clos : il ne s'ouvre plus.", d.societe));
    }
    Ok(d)
}

/// Cree un dossier, avec ce qu'il lui faut pour vendre des le premier
/// jour : son exercice de l'annee, son magasin par defaut, son client
/// de passage. Tout dans UNE transaction — un dossier a moitie ne, sans
/// magasin, ne vaut rien.
///
/// Ses dates de travail sont DONNEES (D21), proposees a l'annee
/// civile : c'est son premier exercice. Sur SQLite comme sur
/// PostgreSQL (D22) — le serveur sert tout par `Base` (D-2). La fenetre
/// monoposte, elle, n'a pas de commande pour creer un dossier : elle
/// reste a un seul.
pub fn creer_dossier_sur(
    base: &mut Base,
    code: String,
    societe: String,
    date_debut: Option<String>,
    date_fin: Option<String>,
) -> Result<serde_json::Value, String> {
    let code = valider_code_dossier(&code)?;
    let societe = societe.trim().to_string();
    if societe.is_empty() {
        return Err("Le nom de la societe est vide.".to_string());
    }
    let deja: Option<String> = base
        .lire_une("SELECT id FROM dossier WHERE code = ?1", &parametres![code.clone()], |r| r.get::<String>(0))
        .map_err(|e| e.0)?;
    if deja.is_some() {
        return Err(format!("Le code « {code} » est deja pris."));
    }

    let id = uuid::Uuid::new_v4().to_string();
    let now = crate::utils::maintenant_iso();
    let (debut, fin) = dates_de_travail(date_debut.as_deref(), date_fin.as_deref(), &now)?;
    let auteur = crate::argent::id_utilisateur_courant_sur(base);
    let dossier_courant = base.dossier().to_string();
    // C-2 : qui n'a que certains dossiers entre dans celui qu'il cree,
    // avec le role qu'il a ici.
    let role_ici = crate::acces_dossiers::role_dans_sur(base, &auteur, &dossier_courant)?;

    let mut tx = base.transaction().map_err(|e| e.0)?;
    tx.executer(
        "INSERT INTO dossier (id, code, societe, clos, cree_le, modifie_le)
         VALUES (?1, ?2, ?3, 0, ?4, ?4)",
        &parametres![id.clone(), code.clone(), societe.clone(), now.clone()],
    )
    .map_err(|e| e.0)?;
    // Ses affaires a lui s'ecrivent CHEZ LUI : sous le compte limite de
    // PostgreSQL (D-6), une ligne d'un autre dossier que la session est
    // refusee par le moteur.
    tx.ecrire_dans(&id).map_err(|e| e.0)?;
    tx.executer(
        "INSERT INTO exercice (id, dossier_id, date_debut, date_fin, clos, cree_le, modifie_le)
         VALUES (?1, ?2, ?3, ?4, 0, ?5, ?5)",
        &parametres![uuid::Uuid::new_v4().to_string(), id.clone(), debut.clone(), fin.clone(), now.clone()],
    )
    .map_err(|e| e.0)?;
    // Le magasin et le client de passage portent EXPLICITEMENT le
    // nouveau dossier : la transaction, elle, est encore dans l'ancien.
    tx.executer(
        "INSERT INTO depot (id, nom, est_defaut, actif, cree_le, modifie_le, origine, dossier_id)
         VALUES (?1, 'Magasin principal', 1, 1, ?2, ?2, 'dossier', ?3)",
        &parametres![uuid::Uuid::new_v4().to_string(), now.clone(), id.clone()],
    )
    .map_err(|e| e.0)?;
    // `client.code` est unique sur TOUTE la base, pas par dossier : les
    // codes de ce dossier portent son prefixe (`prefixe_de_code_sur`),
    // le client de passage comme les autres.
    tx.executer(
        "INSERT INTO client
           (id, code, nom, est_generique, actif, cree_le, modifie_le, origine, dossier_id)
         VALUES (?1, ?2, 'Comptant', 1, 1, ?3, ?3, 'dossier', ?4)",
        &parametres![
            uuid::Uuid::new_v4().to_string(),
            format!("{code}-CLIENT00000"),
            now.clone(),
            id.clone()
        ],
    )
    .map_err(|e| e.0)?;
    // Au journal du dossier ou l'on est : c'est la qu'on a agi.
    tx.ecrire_dans(&dossier_courant).map_err(|e| e.0)?;
    tx.executer(
        "INSERT INTO journal
           (id, type_evenement, entite_type, entite_id, auteur_id,
            nouveau_valeur, origine, date_evenement, dossier_id)
         VALUES (?1, 'dossier_cree', 'dossier', ?2, ?3, ?4, 'app', ?5, ?6)",
        &parametres![
            uuid::Uuid::new_v4().to_string(),
            id.clone(),
            auteur.clone(),
            serde_json::json!({ "code": code, "societe": societe, "du": debut, "au": fin }).to_string(),
            now,
            dossier_courant
        ],
    )
    .map_err(|e| e.0)?;
    crate::acces_dossiers::donner_au_createur_sur(&mut tx, &auteur, &id, role_ici.as_deref())?;
    tx.valider().map_err(|e| e.0)?;

    Ok(serde_json::json!({ "id": id, "code": code, "societe": societe, "date_debut": debut, "date_fin": fin }))
}

/// Le prefixe des codes de tiers du dossier courant : rien pour le
/// dossier d'origine (ses codes existent deja sans), « CODE- » pour les
/// autres — `client.code` est unique sur toute la base, pas par dossier.
pub fn prefixe_de_code_sur(base: &mut Base) -> Result<String, String> {
    let dossier = base.dossier().to_string();
    if dossier == DOSSIER_DEFAUT {
        return Ok(String::new());
    }
    let code: Option<String> = base
        .lire_une("SELECT code FROM dossier WHERE id = ?1", &parametres![dossier], |r| r.get::<String>(0))
        .map_err(|e| e.0)?;
    Ok(code.map(|c| format!("{c}-")).unwrap_or_default())
}

/// Le dossier que cet utilisateur ouvre d'office (plan, decision 3 :
/// par utilisateur). `None` : on lui demande.
pub fn dossier_memorise_sur(base: &mut Base, utilisateur_id: &str) -> Result<Option<String>, String> {
    base.lire_une(
        "SELECT valeur FROM config_app WHERE cle = ?1",
        &parametres![format!("dossier_defaut_utilisateur:{utilisateur_id}")],
        |r| r.get::<String>(0),
    )
    .map_err(|e| e.0)
    .map(|v| v.filter(|s| !s.trim().is_empty()))
}

/// Retient (ou oublie, avec `None`) le dossier a ouvrir d'office.
pub fn memoriser_dossier_sur(
    base: &mut Base,
    utilisateur_id: &str,
    dossier_id: Option<&str>,
) -> Result<(), String> {
    let cle = format!("dossier_defaut_utilisateur:{utilisateur_id}");
    match dossier_id {
        Some(d) => base
            .executer(
                "INSERT INTO config_app (cle, valeur) VALUES (?1, ?2)
                 ON CONFLICT (cle) DO UPDATE SET valeur = excluded.valeur",
                &parametres![cle, d],
            )
            .map(|_| ())
            .map_err(|e| e.0),
        None => base
            .executer("DELETE FROM config_app WHERE cle = ?1", &parametres![cle])
            .map(|_| ())
            .map_err(|e| e.0),
    }
}

#[cfg(test)]
mod tests_code_dossier {
    use super::*;

    #[test]
    fn le_code_se_normalise_et_se_refuse_quand_il_faut() {
        assert_eq!(valider_code_dossier(" quinc-2 ").unwrap(), "QUINC-2");
        assert!(valider_code_dossier("").is_err());
        assert!(valider_code_dossier("é").is_err());
        assert!(valider_code_dossier("a b").is_err());
        assert!(valider_code_dossier(&"X".repeat(21)).is_err());
    }
}

// =====================================================================
//  LE DOSSIER D'ORIGINE — la migration d'une base existante (v3, D-5)
// =====================================================================
//
// Tout ce qu'une base v2 contient porte `dossier_id = defaut` (la
// colonne est posee avec ce defaut) : c'est deja le premier dossier.
// Deux choses manquaient pour qu'il soit le SIEN : son nom — il
// s'appelait « Ma boutique » quel que soit le commerce — et ses dates —
// un exercice de l'annee en cours, alors que les ventes remontent
// parfois a deux ans : un reglement tardif d'une vente de 2024 aurait
// ete refuse par le garde-fou.

/// Le nom pose d'office au dossier d'origine.
pub const NOM_D_USINE: &str = "Ma boutique";

/// Le nom du dossier d'origine a la migration : celui que le patron a
/// deja donne a sa societe (Parametres -> Societe), si le dossier porte
/// encore le nom d'usine et que la societe en a un vrai. Pure.
pub fn nom_d_origine(societe_actuelle: &str, nom_societe: Option<&str>) -> Option<String> {
    if societe_actuelle != NOM_D_USINE {
        return None;
    }
    let nom = nom_societe.map(str::trim).filter(|n| !n.is_empty())?;
    if ["Ma Société", "Ma Societe", NOM_D_USINE].iter().any(|u| u.eq_ignore_ascii_case(nom)) {
        return None;
    }
    Some(nom.to_string())
}

/// Le debut du premier exercice a la migration : le 1er janvier de
/// l'annee de la plus ancienne ecriture, si elle precede le debut
/// actuel. Pure.
pub fn debut_d_origine(debut_actuel: &str, plus_ancienne: Option<&str>) -> Option<String> {
    let jour = crate::coeur::dates::jour(plus_ancienne?)?;
    let debut = format!("{}-01-01", jour.format("%Y"));
    (debut.as_str() < debut_actuel).then_some(debut)
}

/// Les ecritures datees dont la plus ancienne fixe le debut du dossier
/// d'origine. `SUBSTR(MIN(..))` : un seul aller-retour par table, et le
/// meme SQL sur les deux moteurs.
const DATES_ECRITES: &[(&str, &str)] = &[
    ("vente", "date_vente"),
    ("piece_commerciale", "date_piece"),
    ("paiement", "date_paiement"),
    ("mouvement_caisse", "date_mouvement"),
    ("mouvement_stock", "date_mouvement"),
    ("retour", "date_retour"),
    ("paiement_fournisseur", "date_paiement"),
];

/// La migration du dossier d'origine, une fois par base (marque
/// `migration_v3_dossier_origine`). Nomme le dossier comme la societe,
/// et recule le debut de son exercice unique jusqu'a couvrir la plus
/// ancienne ecriture. Rend ce qu'elle a fait (`None` : deja faite).
///
/// Appelee par `amorcage::amorcer` (PostgreSQL, base neuve) et par le
/// serveur au demarrage sur un fichier SQLite ; la fenetre monoposte
/// n'en a pas besoin (un dossier, pas de garde-fou de dates).
pub fn migrer_dossier_d_origine_sur(base: &mut Base) -> Result<Option<serde_json::Value>, String> {
    let deja = base
        .lire_une(
            "SELECT valeur FROM config_app WHERE cle = 'migration_v3_dossier_origine'",
            &[],
            |r| r.get::<String>(0),
        )
        .map_err(|e| e.0)?;
    if deja.is_some() {
        return Ok(None);
    }
    let precedent = base.dossier().to_string();
    base.choisir_dossier(DOSSIER_DEFAUT).map_err(|e| e.0)?;
    let r = migrer_dans_le_dossier_d_origine(base);
    let _ = base.choisir_dossier(&precedent);
    r.map(Some)
}

fn migrer_dans_le_dossier_d_origine(base: &mut Base) -> Result<serde_json::Value, String> {
    let now = crate::utils::maintenant_iso();
    let societe: Option<String> = base
        .lire_une("SELECT societe FROM dossier WHERE id = ?1", &parametres![DOSSIER_DEFAUT], |r| r.get::<String>(0))
        .map_err(|e| e.0)?;
    let nom_societe: Option<String> = base
        .lire_une("SELECT nom FROM parametres_societe WHERE id = 1", &[], |r| r.get::<String>(0))
        .ok()
        .flatten();
    let nouveau_nom = societe.as_deref().and_then(|s| nom_d_origine(s, nom_societe.as_deref()));

    let mut plus_ancienne: Option<String> = None;
    for (table, colonne) in DATES_ECRITES {
        let sql = format!("SELECT SUBSTR(MIN({colonne}), 1, 10) FROM {table} WHERE dossier_id = ?1");
        let d: Option<String> = base
            .lire_une(&sql, &parametres![DOSSIER_DEFAUT], |r| r.get::<Option<String>>(0))
            .map_err(|e| e.0)?
            .flatten();
        if let Some(d) = d.filter(|d| crate::coeur::dates::jour(d).is_some()) {
            if plus_ancienne.as_deref().is_none_or(|p| d.as_str() < p) {
                plus_ancienne = Some(d);
            }
        }
    }
    // Un exercice unique et ouvert : celui que la migration a pose. S'il
    // y en a plusieurs, le patron a deja decide ; on n'y touche pas.
    let exercices = lire_les_exercices(base, DOSSIER_DEFAUT)?;
    let recul = match exercices.as_slice() {
        [seul] if !seul.clos => debut_d_origine(&seul.date_debut, plus_ancienne.as_deref()).map(|d| (seul.id.clone(), d)),
        _ => None,
    };

    let mut tx = base.transaction().map_err(|e| e.0)?;
    if let Some(nom) = &nouveau_nom {
        tx.executer(
            "UPDATE dossier SET societe = ?1, modifie_le = ?2 WHERE id = ?3",
            &parametres![nom.clone(), now.clone(), DOSSIER_DEFAUT],
        )
        .map_err(|e| e.0)?;
    }
    if let Some((id, debut)) = &recul {
        tx.executer(
            "UPDATE exercice SET date_debut = ?1, modifie_le = ?2 WHERE id = ?3 AND dossier_id = ?4",
            &parametres![debut.clone(), now.clone(), id.clone(), DOSSIER_DEFAUT],
        )
        .map_err(|e| e.0)?;
    }
    let fait = serde_json::json!({
        "societe": nouveau_nom,
        "debut": recul.as_ref().map(|(_, d)| d.clone()),
        "plus_ancienne_ecriture": plus_ancienne,
    });
    if nouveau_nom.is_some() || recul.is_some() {
        tx.executer(
            "INSERT INTO journal
               (id, type_evenement, entite_type, entite_id, nouveau_valeur, origine, date_evenement, dossier_id)
             VALUES (?1, 'dossier_d_origine', 'dossier', ?2, ?3, 'migration', ?4, ?2)",
            &parametres![uuid::Uuid::new_v4().to_string(), DOSSIER_DEFAUT, fait.to_string(), now.clone()],
        )
        .map_err(|e| e.0)?;
    }
    tx.executer(
        "INSERT INTO config_app (cle, valeur) VALUES ('migration_v3_dossier_origine', ?1)
         ON CONFLICT (cle) DO NOTHING",
        &parametres![now],
    )
    .map_err(|e| e.0)?;
    tx.valider().map_err(|e| e.0)?;
    Ok(fait)
}

/// Renomme un dossier (sa societe). Le code ne change pas : il prefixe
/// deja les codes de ses tiers. Au journal du dossier renomme.
pub fn renommer_dossier_sur(base: &mut Base, dossier_id: String, societe: String) -> Result<serde_json::Value, String> {
    let societe = societe.trim().to_string();
    if societe.is_empty() {
        return Err("Le nom de la société est vide.".to_string());
    }
    let ancien: String = base
        .lire_une("SELECT societe FROM dossier WHERE id = ?1", &parametres![dossier_id.clone()], |r| r.get::<String>(0))
        .map_err(|e| e.0)?
        .ok_or_else(|| "Dossier introuvable.".to_string())?;
    let auteur = crate::argent::id_utilisateur_courant_sur(base);
    let now = crate::utils::maintenant_iso();
    let mut tx = base.transaction().map_err(|e| e.0)?;
    tx.executer(
        "UPDATE dossier SET societe = ?1, modifie_le = ?2 WHERE id = ?3",
        &parametres![societe.clone(), now.clone(), dossier_id.clone()],
    )
    .map_err(|e| e.0)?;
    tx.executer(
        "INSERT INTO journal
           (id, type_evenement, entite_type, entite_id, auteur_id,
            ancien_valeur, nouveau_valeur, origine, date_evenement, dossier_id)
         VALUES (?1, 'dossier_renomme', 'dossier', ?2, ?3, ?4, ?5, 'app', ?6, ?2)",
        &parametres![
            uuid::Uuid::new_v4().to_string(),
            dossier_id.clone(),
            auteur,
            serde_json::json!({ "societe": ancien }).to_string(),
            serde_json::json!({ "societe": societe }).to_string(),
            now
        ],
    )
    .map_err(|e| e.0)?;
    tx.valider().map_err(|e| e.0)?;
    Ok(serde_json::json!({ "id": dossier_id, "societe": societe }))
}

#[cfg(test)]
mod tests_dossier_d_origine {
    use super::*;

    #[test]
    fn le_dossier_prend_le_nom_de_la_societe_s_il_porte_encore_celui_d_usine() {
        assert_eq!(nom_d_origine("Ma boutique", Some(" Quincaillerie Traoré ")).as_deref(), Some("Quincaillerie Traoré"));
        assert_eq!(nom_d_origine("Ma boutique", Some("Ma Société")), None, "un nom d'usine n'en remplace pas un autre");
        assert_eq!(nom_d_origine("Ma boutique", Some("  ")), None);
        assert_eq!(nom_d_origine("Ma boutique", None), None);
        assert_eq!(nom_d_origine("Déjà nommé", Some("Autre")), None, "le patron a déjà décidé");
    }

    #[test]
    fn le_premier_exercice_recule_au_premier_janvier_de_la_plus_ancienne_ecriture() {
        assert_eq!(debut_d_origine("2026-01-01", Some("2024-06-15")).as_deref(), Some("2024-01-01"));
        assert_eq!(debut_d_origine("2026-01-01", Some("2026-03-02")), None, "déjà couverte");
        assert_eq!(debut_d_origine("2026-01-01", None), None, "une base vide");
        assert_eq!(debut_d_origine("2026-01-01", Some("n'importe quoi")), None);
    }
}
