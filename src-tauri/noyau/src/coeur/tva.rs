//! La TVA des marchandises rendues (D53).
//!
//! Un article rendu sort de la TVA collectee A LA DATE DU RETOUR, au
//! prorata de la quantite rendue, au taux de la ligne vendue ; la
//! periode de la vente ne bouge pas (regle d'un avoir). Rien n'est
//! stocke : la TVA rendue se calcule a la lecture, depuis la ligne de
//! vente (`montant_tva`, `quantite`) et les retours de cette ligne.
//!
//! L'echange ne sort pas de la TVA, comme il ne sort pas du chiffre
//! d'affaires (`calcul::RETOUR_HORS_CA`) : son remplacement part sans
//! ligne de vente. Sa quantite compte pourtant dans le cumul de la
//! ligne — elle est bien revenue.

/// La TVA que rend chaque retour d'une ligne, dans l'ordre des retours.
///
/// Cumulee : apres chaque retour, la TVA rendue depuis le debut vaut
/// `arrondi(tva_ligne x quantite rendue cumulee / quantite vendue)`, et
/// ce retour rend la difference. Trois retours d'un tiers rendent donc
/// exactement la TVA de la ligne (100 F, pas 3 x 33). Le cumul ne
/// depasse jamais la quantite vendue.
pub fn tva_rendue_par_retour(tva_ligne: i64, qte_vendue: f64, quantites: &[f64]) -> Vec<i64> {
    if qte_vendue <= 0.0 {
        return vec![0; quantites.len()];
    }
    let mut cumul = 0.0_f64;
    let mut deja = 0_i64;
    quantites
        .iter()
        .map(|q| {
            cumul = (cumul + q.max(0.0)).min(qte_vendue);
            let jusqu_ici = (tva_ligne as f64 * cumul / qte_vendue).round() as i64;
            let rendue = jusqu_ici - deja;
            deja = jusqu_ici;
            rendue
        })
        .collect()
}

/// Un retour vu par la TVA : sa ligne de vente, et s'il se deduit.
#[derive(Debug, Clone)]
pub struct RetourTaxe {
    /// La ligne de vente rendue (un retour d'avant v1.3 : la premiere
    /// ligne du meme article dans sa vente).
    pub ligne: String,
    pub taux: f64,
    /// `ligne_vente.montant_tva` et `ligne_vente.quantite`.
    pub tva_ligne: i64,
    pub qte_ligne: f64,
    /// `retour.quantite` (dans l'unite de la ligne) et `montant_credit` (TTC).
    pub quantite: f64,
    pub credit: i64,
    /// Date du retour dans la periode lue.
    pub dans_periode: bool,
    /// Faux pour l'echange : compte dans le cumul, ne se deduit pas.
    pub deduit: bool,
}

/// Ce que les retours d'une periode retirent, a un taux.
#[derive(Debug, Clone, PartialEq)]
pub struct TvaRendue {
    pub taux: f64,
    pub tva: i64,
    /// Le montant rendu moins sa TVA.
    pub ht: i64,
    pub nb: i64,
}

/// Les retours deduits d'une periode, par taux, dans l'ordre des taux.
///
/// `retours` porte TOUS les retours des lignes touchees jusqu'a la fin
/// de la periode, dans l'ordre ou ils ont eu lieu : ceux d'avant la
/// periode ne se deduisent pas, mais ils font le cumul.
pub fn retours_par_taux(retours: &[RetourTaxe]) -> Vec<TvaRendue> {
    let mut lignes: Vec<&str> = Vec::new();
    for r in retours {
        if !lignes.contains(&r.ligne.as_str()) {
            lignes.push(&r.ligne);
        }
    }
    let mut par_taux: Vec<TvaRendue> = Vec::new();
    for ligne in lignes {
        let de_la_ligne: Vec<&RetourTaxe> = retours.iter().filter(|r| r.ligne == ligne).collect();
        let quantites: Vec<f64> = de_la_ligne.iter().map(|r| r.quantite).collect();
        let premier = de_la_ligne[0];
        let tva = tva_rendue_par_retour(premier.tva_ligne, premier.qte_ligne, &quantites);
        for (r, t) in de_la_ligne.iter().zip(tva) {
            if !(r.dans_periode && r.deduit) {
                continue;
            }
            let i = match par_taux.iter().position(|x| meme_taux(x.taux, r.taux)) {
                Some(i) => i,
                None => {
                    par_taux.push(TvaRendue { taux: r.taux, tva: 0, ht: 0, nb: 0 });
                    par_taux.len() - 1
                }
            };
            par_taux[i].tva += t;
            par_taux[i].ht += r.credit - t;
            par_taux[i].nb += 1;
        }
    }
    par_taux.sort_by(|a, b| a.taux.partial_cmp(&b.taux).unwrap_or(std::cmp::Ordering::Equal));
    par_taux
}

/// Une ligne du rapport TVA : les ventes de la periode a ce taux, moins
/// les retours dates dans la periode a ce taux.
#[derive(Debug, Clone, PartialEq)]
pub struct LigneTva {
    pub taux: f64,
    /// Nets des retours.
    pub total_tva: i64,
    pub total_ht: i64,
    pub nb_ventes: i64,
    pub tva_retours: i64,
    pub ht_retours: i64,
    pub nb_retours: i64,
}

/// Les ventes `(taux, tva, ht, nb_ventes)` et les retours, fondus par
/// taux. Un taux qui n'a que des retours dans la periode a sa ligne, en
/// negatif : c'est la TVA a declarer en moins.
pub fn fusionner(ventes: &[(f64, i64, i64, i64)], retours: &[TvaRendue]) -> Vec<LigneTva> {
    let mut lignes: Vec<LigneTva> = ventes
        .iter()
        .map(|&(taux, tva, ht, nb)| LigneTva {
            taux,
            total_tva: tva,
            total_ht: ht,
            nb_ventes: nb,
            tva_retours: 0,
            ht_retours: 0,
            nb_retours: 0,
        })
        .collect();
    for r in retours {
        let i = match lignes.iter().position(|l| meme_taux(l.taux, r.taux)) {
            Some(i) => i,
            None => {
                lignes.push(LigneTva {
                    taux: r.taux,
                    total_tva: 0,
                    total_ht: 0,
                    nb_ventes: 0,
                    tva_retours: 0,
                    ht_retours: 0,
                    nb_retours: 0,
                });
                lignes.len() - 1
            }
        };
        let l = &mut lignes[i];
        l.total_tva -= r.tva;
        l.total_ht -= r.ht;
        l.tva_retours += r.tva;
        l.ht_retours += r.ht;
        l.nb_retours += r.nb;
    }
    lignes.sort_by(|a, b| a.taux.partial_cmp(&b.taux).unwrap_or(std::cmp::Ordering::Equal));
    lignes
}

/// Deux taux lus de la base (REAL / DOUBLE PRECISION) sont le meme taux.
fn meme_taux(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

#[cfg(test)]
mod tests {
    use super::*;

    fn retour(ligne: &str, quantite: f64, credit: i64, dans_periode: bool, deduit: bool) -> RetourTaxe {
        RetourTaxe {
            ligne: ligne.into(),
            taux: 0.18,
            tva_ligne: 100,
            qte_ligne: 3.0,
            quantite,
            credit,
            dans_periode,
            deduit,
        }
    }

    #[test]
    fn trois_retours_d_un_tiers_rendent_toute_la_tva_de_la_ligne() {
        assert_eq!(tva_rendue_par_retour(100, 3.0, &[1.0, 1.0, 1.0]), vec![33, 34, 33]);
        assert_eq!(tva_rendue_par_retour(100, 3.0, &[3.0]), vec![100]);
        assert_eq!(tva_rendue_par_retour(366, 2.0, &[0.5]), vec![92]);
    }

    #[test]
    fn le_cumul_ne_depasse_pas_la_quantite_vendue() {
        assert_eq!(tva_rendue_par_retour(100, 2.0, &[2.0, 1.0]), vec![100, 0]);
        assert_eq!(tva_rendue_par_retour(100, 0.0, &[1.0]), vec![0]);
    }

    #[test]
    fn un_retour_d_avant_la_periode_fait_le_cumul_sans_se_deduire() {
        // Septembre (deja declare) : 1 sur 3 ; octobre : les 2 autres.
        let r = retours_par_taux(&[
            retour("L", 1.0, 1_000, false, true),
            retour("L", 2.0, 2_000, true, true),
        ]);
        assert_eq!(r, vec![TvaRendue { taux: 0.18, tva: 67, ht: 2_000 - 67, nb: 1 }]);
    }

    #[test]
    fn l_echange_compte_dans_le_cumul_mais_ne_se_deduit_pas() {
        let r = retours_par_taux(&[
            retour("L", 1.0, 1_000, true, false),
            retour("L", 1.0, 1_000, true, true),
        ]);
        assert_eq!(r, vec![TvaRendue { taux: 0.18, tva: 34, ht: 1_000 - 34, nb: 1 }]);
    }

    #[test]
    fn la_tva_nette_deduit_les_retours_et_un_taux_sans_vente_finit_negatif() {
        let ventes = [(0.18, 900, 5_000, 2)];
        let retours = [
            TvaRendue { taux: 0.18, tva: 100, ht: 556, nb: 1 },
            TvaRendue { taux: 0.05, tva: 10, ht: 200, nb: 1 },
        ];
        let l = fusionner(&ventes, &retours);
        assert_eq!(l.len(), 2);
        assert_eq!((l[0].taux, l[0].total_tva, l[0].total_ht, l[0].nb_ventes), (0.05, -10, -200, 0));
        assert_eq!((l[1].total_tva, l[1].total_ht, l[1].tva_retours, l[1].nb_retours), (800, 4_444, 100, 1));
    }
}
