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
        if self.clos {
            return Err("Cet exercice est clos : on n'y écrit plus.".to_string());
        }
        let jour = &date[..date.len().min(10)];
        if jour < self.date_debut.as_str() {
            return Err(format!(
                "Le {jour} précède l'exercice, qui commence le {}.",
                self.date_debut
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
        assert!(e.contains("2026-01-01"), "le refus dit jusqu'où reculer");
    }

    #[test]
    fn apres_l_exercice_le_refus_propose_la_suite() {
        let e = exercice_2026().accepte("2027-01-01").unwrap_err();
        assert!(e.contains("2026-12-31"));
        assert!(e.contains("prolonger"), "le refus dit quoi faire");
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
        assert!(err.contains("2027-03-31"));
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
            Err(format!(
                "Le {jour} ne tombe dans aucun exercice ouvert pour ce dossier. \
                 Ouvrir un exercice qui le couvre."
            ))
        }
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
    fn aucun_exercice_ne_couvrant_la_date_le_dit() {
        let err = dossier_accepte(&deux_exercices(), "2027-01-05").unwrap_err();
        assert!(err.contains("aucun exercice"), "{err}");
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
// Sur l'un ou l'autre moteur, comme `catalogue` et `comptoir`. Pas
// encore appele par un ecran — celui-ci attend que le serveur tienne
// une `Base` (D11) — mais deja teste et pret a l'etre : c'est le meme
// garde-fou qu'appellera `creer_vente_sur` avant d'ecrire.

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
    if date_fin < date_debut {
        return Err("La date de fin précède la date de début.".to_string());
    }
    let dossier = base.dossier().to_string();
    let existants = lire_les_exercices(base, &dossier)?;
    if let Some(chevauche) = existants
        .iter()
        .find(|e| {
            date_debut.as_str() <= e.fin_effective() && date_fin.as_str() >= e.date_debut.as_str()
        })
    {
        return Err(format!(
            "Chevauche l'exercice du {} au {}.",
            chevauche.date_debut,
            chevauche.fin_effective()
        ));
    }

    let id = uuid::Uuid::new_v4().to_string();
    let now = crate::utils::maintenant_iso();
    base.executer(
        "INSERT INTO exercice
           (id, dossier_id, date_debut, date_fin, clos, cree_le, modifie_le)
         VALUES (?1,?2,?3,?4,0,?5,?5)",
        &parametres![id.clone(), dossier, date_debut.clone(), date_fin.clone(), now],
    )
    .map_err(|e| e.0)?;

    Ok(serde_json::json!({
        "id": id, "date_debut": date_debut, "date_fin": date_fin, "clos": false
    }))
}

/// Repousse la fin d'un exercice DU DOSSIER COURANT.
///
/// Le filtre de dossier compte ici autant que l'identifiant : sans lui,
/// un identifiant devine ou vole permettrait de prolonger l'exercice
/// d'une autre societe.
pub fn prolonger_exercice_sur(
    base: &mut Base,
    exercice_id: String,
    prolonge_jusqu_au: String,
) -> Result<(), String> {
    let dossier = base.dossier().to_string();
    let now = crate::utils::maintenant_iso();
    let touche = base
        .executer(
            "UPDATE exercice SET prolonge_jusqu_au = ?1, modifie_le = ?2
             WHERE id = ?3 AND dossier_id = ?4",
            &parametres![prolonge_jusqu_au, now, exercice_id, dossier],
        )
        .map_err(|e| e.0)?;
    if touche == 0 {
        return Err("Exercice introuvable.".to_string());
    }
    Ok(())
}

/// Clot un exercice DU DOSSIER COURANT. On n'y ecrit plus ensuite.
pub fn clore_exercice_sur(base: &mut Base, exercice_id: String) -> Result<(), String> {
    let dossier = base.dossier().to_string();
    let now = crate::utils::maintenant_iso();
    let touche = base
        .executer(
            "UPDATE exercice SET clos = 1, modifie_le = ?1 WHERE id = ?2 AND dossier_id = ?3",
            &parametres![now, exercice_id, dossier],
        )
        .map_err(|e| e.0)?;
    if touche == 0 {
        return Err("Exercice introuvable.".to_string());
    }
    Ok(())
}
