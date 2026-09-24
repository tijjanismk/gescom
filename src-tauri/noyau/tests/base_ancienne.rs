//! Une base amorcée par une version plus ancienne, sur les deux moteurs.
//!
//! Trouvé chez le propriétaire le 24/09/2026 : sa base PostgreSQL datait
//! d'avant l'entrée de `avoir.piece_id` dans schema.sql. Seule la fenêtre
//! SQLite rejouait la colonne ; le serveur, jamais — et l'Historique
//! refusait « la colonne av.piece_id n'existe pas ». On retire ici les
//! colonnes que seule la fenêtre ajoutait, on relance l'amorçage comme au
//! démarrage du serveur, et tout revient.

mod commun;

use commun::*;
use gescom_noyau::amorcage;
use gescom_noyau::historique::{self, Filtre};

const RETIREES: &[(&str, &str)] = &[
    ("avoir", "piece_id"),
    ("vente", "piece_id"),
    ("paiement_fournisseur", "piece_id"),
    ("mouvement_caisse", "categorie"),
    ("transfert", "motif"),
    ("retour", "ligne_vente_id"),
];

#[test]
fn le_demarrage_rend_les_colonnes_qu_une_ancienne_base_n_a_pas() {
    let mut base = base_avec_demo();
    // Sous le compte limite (D-6), rien ne migre : c'est le proprietaire
    // qui demarre le serveur pour les migrations. Rien a prouver ici.
    if !base.peut_migrer() {
        return;
    }
    for (t, c) in RETIREES {
        base.executer(&format!("ALTER TABLE {t} DROP COLUMN {c}"), &[]).unwrap_or_else(|e| panic!("{t}.{c} : {}", e.0));
    }
    assert!(historique::lire_historique_sur(&mut base, Filtre::default()).is_err(), "sans avoir.piece_id, l'Historique échoue");
    amorcage::amorcer(&mut base).unwrap();
    for (t, c) in RETIREES {
        assert!(base.executer(&format!("SELECT {c} FROM {t} WHERE 1 = 0"), &[]).is_ok(), "{t}.{c} revenue");
    }
    historique::lire_historique_sur(&mut base, Filtre::default()).expect("l'Historique se lit de nouveau");
}
