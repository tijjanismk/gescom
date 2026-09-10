//! Logique de caisse — calculs purs.
//!
//! ⚠️ Le rapprochement ne porte que sur les ESPÈCES : Orange Money et Moov
//! Money sont tracés en mouvement de caisse mais ne sont pas dans le tiroir
//! physique. L'appelant doit donc filtrer `moyen = 'especes'` avant de
//! sommer `entrees` et `sorties`. (cf. audit §10)

/// Solde théorique espèces = fond d'ouverture + entrées - sorties.
pub fn solde_theorique(fond: i64, entrees: i64, sorties: i64) -> i64 {
    fond + entrees - sorties
}

/// Écart de caisse = espèces comptées - solde théorique.
/// Négatif = manque dans le tiroir. Positif = excédent.
pub fn ecart_caisse(compte: i64, theorique: i64) -> i64 {
    compte - theorique
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solde_theorique_additionne_le_fond() {
        assert_eq!(solde_theorique(10_000, 8_000, 0), 18_000);
        assert_eq!(solde_theorique(10_000, 8_000, 3_000), 15_000);
        assert_eq!(solde_theorique(0, 0, 0), 0);
    }

    #[test]
    fn ecart_negatif_signale_un_manque() {
        assert_eq!(ecart_caisse(17_500, 18_000), -500);
        assert!(ecart_caisse(17_500, 18_000) < 0);
    }

    #[test]
    fn ecart_nul_si_caisse_juste() {
        let theorique = solde_theorique(10_000, 8_000, 0);
        assert_eq!(ecart_caisse(18_000, theorique), 0);
    }

    #[test]
    fn mobile_money_hors_tiroir() {
        // 10 000 de fond, 8 000 en espèces, 10 000 en Orange Money.
        // Le théorique espèces ne doit compter QUE les 8 000.
        let theorique = solde_theorique(10_000, 8_000, 0);
        assert_eq!(theorique, 18_000);
        // Si on avait inclus l'Orange Money (bug §10), on obtiendrait 28 000
        // et un écart fantôme de -10 000 à la fermeture.
        assert_ne!(theorique, 28_000);
    }
}
