//! Le duplicata (26/10/2026, PLAN-DUPLICATA) : la regle, pure.
//!
//! Une piece numerotee n'a qu'un original (usage comptable, OHADA) : une
//! seconde feuille « originale » sert a se faire rembourser deux fois, a
//! prouver un paiement deja compte. La premiere impression est
//! l'original, toutes les suivantes des duplicatas — sauf l'original
//! reimprime par le patron (`pieces:reimprimer_original`), trace au
//! journal. Le stockage est dans `impression.rs`.

/// Ce qui s'imprime et se compte. Les noms sont ceux du journal
/// (`entite_type`) : l'Historique retrouve ainsi le client et la piece
/// d'un original reimprime sans rien de plus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Objet {
    /// Facture, devis, BL, avoir… — et son ticket, qui en est un format.
    Piece,
    /// Le recu d'un reglement client.
    Paiement,
    /// Le recu d'un reglement fournisseur.
    PaiementFournisseur,
}

impl Objet {
    /// Refuse un nom inconnu plutot que de compter dans le vide.
    pub fn lire(nom: &str) -> Result<Self, String> {
        match nom {
            "piece_commerciale" => Ok(Self::Piece),
            "paiement" => Ok(Self::Paiement),
            "paiement_fournisseur" => Ok(Self::PaiementFournisseur),
            autre => Err(format!(
                "On ne compte pas les impressions de « {autre} » : pièce (piece_commerciale) ou reçu (paiement, paiement_fournisseur)."
            )),
        }
    }

    pub fn nom(self) -> &'static str {
        match self {
            Self::Piece => "piece_commerciale",
            Self::Paiement => "paiement",
            Self::PaiementFournisseur => "paiement_fournisseur",
        }
    }
}

/// Pourquoi une impression ne se compte pas, ou `None` si elle se
/// compte. Un brouillon change encore : il n'a pas d'original. Une
/// facture ou un avoir FOURNISSEUR est sa piece a lui, qu'on ne fait que
/// saisir : son original est chez lui, notre feuille n'en est jamais un.
/// `None`/`None` : un recu (ni statut, ni type).
pub fn hors_compte(statut: Option<&str>, type_piece: Option<&str>) -> Option<&'static str> {
    if statut == Some("brouillon") {
        return Some("Un brouillon n'a pas d'original : il s'imprime tel quel.");
    }
    if matches!(type_piece, Some("facture_fournisseur" | "avoir_fournisseur")) {
        return Some("Cette pièce est celle du fournisseur : son original est chez lui.");
    }
    None
}

/// Ce que sera la prochaine impression.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Verdict {
    pub duplicata: bool,
    /// Ecrit `original = 1` : la premiere, ou l'original du patron.
    pub original: bool,
}

/// `deja` : les impressions deja comptees de cet objet.
pub fn verdict(deja: i64, reimprimer_original: bool) -> Verdict {
    if deja == 0 || reimprimer_original {
        Verdict { duplicata: false, original: true }
    } else {
        Verdict { duplicata: true, original: false }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_premiere_est_l_original_les_suivantes_des_duplicatas() {
        assert_eq!(verdict(0, false), Verdict { duplicata: false, original: true });
        assert_eq!(verdict(1, false), Verdict { duplicata: true, original: false });
        assert_eq!(verdict(7, false), Verdict { duplicata: true, original: false });
    }

    #[test]
    fn l_original_du_patron_reste_un_original() {
        assert_eq!(verdict(3, true), Verdict { duplicata: false, original: true });
    }

    #[test]
    fn un_brouillon_ou_une_piece_du_fournisseur_ne_compte_pas() {
        assert!(hors_compte(Some("brouillon"), Some("facture")).is_some());
        assert!(hors_compte(Some("validee"), Some("facture_fournisseur")).is_some());
        assert!(hors_compte(Some("validee"), Some("avoir_fournisseur")).is_some());
        assert!(hors_compte(Some("validee"), Some("facture")).is_none());
        // Ceux-la, c'est nous qui les emettons.
        assert!(hors_compte(Some("validee"), Some("bon_commande_fournisseur")).is_none());
        assert!(hors_compte(Some("validee"), Some("avoir_client")).is_none());
        assert!(hors_compte(None, None).is_none(), "un reçu se compte");
    }

    #[test]
    fn un_objet_inconnu_est_refuse() {
        for o in [Objet::Piece, Objet::Paiement, Objet::PaiementFournisseur] {
            assert_eq!(Objet::lire(o.nom()), Ok(o));
        }
        assert!(Objet::lire("vente").is_err());
        assert!(Objet::lire("").is_err());
    }
}
