#![allow(dead_code)]
//! Logique de caisse — calculs purs.

/// Solde théorique = fond + entrées - sorties.
pub fn solde_theorique(fond: i64, entrees: i64, sorties: i64) -> i64 {
    fond + entrees - sorties
}

/// Écart de caisse = compté - théorique.
pub fn ecart_caisse(compte: i64, theorique: i64) -> i64 {
    compte - theorique
}
