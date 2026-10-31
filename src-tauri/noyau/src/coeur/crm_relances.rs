//! D50 — les clients a relancer (PLAN-EQUIPE, EQ-4). La regle pure :
//! elle recoit des AGES en jours, deja comptes par le module en base
//! (`utils::jours_depuis`), et rend qui relancer et pourquoi. Pas
//! d'horloge ici : elle se teste avec des nombres.
//!
//! Trois motifs, une ligne par client :
//! - **creance** : une vente non soldee de plus de 30 jours (la regle de
//!   l'ecran Relances de Gescom) ;
//! - **ralentit** : un des meilleurs clients achete moins de la moitie
//!   de son habitude sur les 90 derniers jours ;
//! - **dormant** : pas d'achat depuis N jours (reglage du dossier).
//!
//! Un client contacte depuis moins de 14 jours, ou qui a un rappel non
//! fait, est « mis de cote » : on ne le relance pas deux fois.

/// N d'office : pas d'achat depuis 60 jours.
pub const JOURS_DORMANT_DEFAUT: i64 = 60;
pub const JOURS_DORMANT_MIN: i64 = 15;
pub const JOURS_DORMANT_MAX: i64 = 365;

/// Une creance « en retard » : vente non soldee de PLUS de 30 jours
/// (`relances::lire_creances_relances_sur_base`, `> 30`).
pub const JOURS_CREANCE_EN_RETARD: i64 = 30;

/// La periode recente : les 90 derniers jours (0 a 90).
pub const JOURS_RECENTS: i64 = 90;
/// La reference : du 91e au 360e jour, trois periodes de 90 jours.
pub const JOURS_REFERENCE_FIN: i64 = 360;
const PERIODES_DE_REFERENCE: i64 = 3;

/// « Meilleur client » : parmi les 10 premiers du dossier par chiffre
/// de reference, avec au moins 3 achats dans la reference — un seul
/// gros achat ne fait pas un fidele.
pub const MEILLEURS_CLIENTS: usize = 10;
pub const ACHATS_MIN_FIDELE: i64 = 3;

/// Apres un echange ou une relance, le client sort de la liste 14 jours.
pub const JOURS_REPOS_APRES_CONTACT: i64 = 14;

pub const MOTIF_CREANCE: &str = "creance";
pub const MOTIF_RALENTIT: &str = "ralentit";
pub const MOTIF_DORMANT: &str = "dormant";

/// Juge le reglage N du dossier. Pure.
pub fn valider_jours_dormant(jours: i64) -> Result<i64, String> {
    if !(JOURS_DORMANT_MIN..=JOURS_DORMANT_MAX).contains(&jours) {
        return Err(format!(
            "Le nombre de jours sans achat va de {JOURS_DORMANT_MIN} à {JOURS_DORMANT_MAX}."
        ));
    }
    Ok(jours)
}

/// Ce que la base sait d'un client, en ages (jours depuis aujourd'hui).
#[derive(Debug, Clone, Default)]
pub struct Releve {
    pub id: String,
    pub nom: String,
    /// Age du dernier achat (vente non annulee) ; `None` : jamais achete.
    pub jours_dernier_achat: Option<i64>,
    /// Les achats des 360 derniers jours : (age, montant).
    pub achats: Vec<(i64, i64)>,
    /// Les ventes non soldees : (age, reste du).
    pub creances: Vec<(i64, i64)>,
    /// Age du dernier echange ou de la derniere relance de creance.
    pub jours_dernier_contact: Option<i64>,
    pub rappel_actif: bool,
    /// Marque « parti » (D50, point 13) : il ne reste que pour sa
    /// creance (point 15).
    pub parti: bool,
}

/// Le verdict d'un client qui a au moins un motif.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verdict {
    pub id: String,
    pub motifs: Vec<&'static str>,
    pub ca_recent: i64,
    pub ca_reference: i64,
    pub nb_achats_reference: i64,
    pub reste_en_retard: i64,
    pub nb_creances_en_retard: i64,
    pub jours_plus_vieille_creance: Option<i64>,
    /// Contacte depuis moins de 14 jours, ou un rappel l'attend.
    pub mis_de_cote: bool,
}

fn chiffres(r: &Releve) -> (i64, i64, i64) {
    let mut recent = 0;
    let mut reference = 0;
    let mut nb_ref = 0;
    for &(age, montant) in &r.achats {
        // Une vente datee d'apres aujourd'hui (horloge d'un poste) compte
        // dans le recent plutot que de disparaitre.
        if age <= JOURS_RECENTS {
            recent += montant;
        } else if age <= JOURS_REFERENCE_FIN {
            reference += montant;
            nb_ref += 1;
        }
    }
    (recent, reference, nb_ref)
}

/// Ralentit : le recent est sous la moitie de la moyenne de reference
/// ramenee a 90 jours — `recent < reference / 3 / 2`, en entiers.
pub fn ralentit(ca_recent: i64, ca_reference: i64) -> bool {
    ca_reference > 0 && ca_recent * PERIODES_DE_REFERENCE * 2 < ca_reference
}

/// Juge tous les clients du dossier d'un coup (le classement des
/// meilleurs se fait entre eux). `avec_creances` : faux pour qui ne lit
/// pas les soldes (D50, point 8) — le motif n'est alors pas calcule.
/// Rend les clients qui ont un motif, dans l'ordre de D50 (point 5),
/// mis de cote compris (a l'appelant de les compter). Pure.
pub fn juger(releves: &[Releve], jours_dormant: i64, avec_creances: bool) -> Vec<Verdict> {
    let calcules: Vec<(i64, i64, i64)> = releves.iter().map(chiffres).collect();

    // Les meilleurs : les 10 premiers par chiffre de reference, parmi
    // ceux qui ont assez d'achats. A egalite, le nom tranche (stable).
    // Un parti ne prend pas la place d'un fidele (D50, point 15).
    let mut candidats: Vec<usize> = (0..releves.len())
        .filter(|&i| !releves[i].parti && calcules[i].2 >= ACHATS_MIN_FIDELE && calcules[i].1 > 0)
        .collect();
    candidats.sort_by(|&a, &b| calcules[b].1.cmp(&calcules[a].1).then(releves[a].nom.cmp(&releves[b].nom)));
    candidats.truncate(MEILLEURS_CLIENTS);

    let mut verdicts: Vec<(usize, Verdict)> = Vec::new();
    for (i, r) in releves.iter().enumerate() {
        let (ca_recent, ca_reference, nb_ref) = calcules[i];
        let mut motifs = Vec::new();

        let en_retard: Vec<&(i64, i64)> = if avec_creances {
            r.creances.iter().filter(|(age, reste)| *age > JOURS_CREANCE_EN_RETARD && *reste > 0).collect()
        } else {
            Vec::new()
        };
        if !en_retard.is_empty() {
            motifs.push(MOTIF_CREANCE);
        }
        if candidats.contains(&i) && ralentit(ca_recent, ca_reference) {
            motifs.push(MOTIF_RALENTIT);
        }
        if !r.parti && r.jours_dernier_achat.is_some_and(|j| j >= jours_dormant) {
            motifs.push(MOTIF_DORMANT);
        }
        if motifs.is_empty() {
            continue;
        }
        let mis_de_cote = r.rappel_actif
            || r.jours_dernier_contact.is_some_and(|j| j < JOURS_REPOS_APRES_CONTACT);
        verdicts.push((
            i,
            Verdict {
                id: r.id.clone(),
                motifs,
                ca_recent,
                ca_reference,
                nb_achats_reference: nb_ref,
                reste_en_retard: en_retard.iter().map(|(_, reste)| reste).sum(),
                nb_creances_en_retard: en_retard.len() as i64,
                jours_plus_vieille_creance: en_retard.iter().map(|(age, _)| *age).max(),
                mis_de_cote,
            },
        ));
    }

    // D50, point 5 : creances (la plus vieille d'abord), ralentis (le
    // plus gros chiffre d'abord), dormants (les plus recents d'abord).
    let rang = |v: &Verdict| -> u8 {
        match v.motifs[0] {
            MOTIF_CREANCE => 0,
            MOTIF_RALENTIT => 1,
            _ => 2,
        }
    };
    verdicts.sort_by(|(ia, a), (ib, b)| {
        rang(a)
            .cmp(&rang(b))
            .then_with(|| match rang(a) {
                0 => b.jours_plus_vieille_creance.cmp(&a.jours_plus_vieille_creance),
                1 => b.ca_reference.cmp(&a.ca_reference),
                _ => releves[*ia].jours_dernier_achat.cmp(&releves[*ib].jours_dernier_achat),
            })
            .then_with(|| releves[*ia].nom.cmp(&releves[*ib].nom))
    });
    verdicts.into_iter().map(|(_, v)| v).collect()
}

/// Le texte propose pour l'echange ou le rappel — sans montant : il
/// s'ecrit dans le fil du client, que lisent aussi ceux qui ne voient
/// ni les soldes ni le chiffre. Pure.
pub fn texte_propose(v: &Verdict, jours_dernier_achat: Option<i64>) -> String {
    let mut parties: Vec<String> = Vec::new();
    for m in &v.motifs {
        match *m {
            MOTIF_CREANCE => match v.jours_plus_vieille_creance {
                Some(j) => parties.push(format!("facture impayée depuis {j} jours")),
                None => parties.push("facture impayée".to_string()),
            },
            MOTIF_RALENTIT => parties.push("achète moins que d'habitude".to_string()),
            _ => {
                if let Some(j) = jours_dernier_achat {
                    parties.push(format!("pas d'achat depuis {j} jours"));
                }
            }
        }
    }
    format!("Relance : {}", parties.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client(nom: &str) -> Releve {
        Releve { id: nom.to_lowercase(), nom: nom.into(), ..Default::default() }
    }

    #[test]
    fn le_reglage_reste_entre_15_et_365() {
        assert_eq!(valider_jours_dormant(60).unwrap(), 60);
        assert!(valider_jours_dormant(14).is_err());
        assert!(valider_jours_dormant(366).is_err());
    }

    #[test]
    fn dormant_a_partir_de_n_jours_et_jamais_sans_achat() {
        let mut a = client("Awa");
        a.jours_dernier_achat = Some(60);
        let mut b = client("Bakary");
        b.jours_dernier_achat = Some(59);
        let jamais = client("Prospect");
        let v = juger(&[a, b, jamais], 60, true);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].id, "awa");
        assert_eq!(v[0].motifs, vec![MOTIF_DORMANT]);
    }

    #[test]
    fn creance_en_retard_au_dela_de_30_jours_et_seulement_si_on_lit_les_soldes() {
        let mut a = client("Awa");
        a.jours_dernier_achat = Some(31);
        a.creances = vec![(31, 5_000), (10, 2_000), (45, 0)];
        let v = juger(&[a.clone()], 60, true);
        assert_eq!(v[0].motifs, vec![MOTIF_CREANCE]);
        assert_eq!(v[0].reste_en_retard, 5_000, "la recente et la soldee n'y sont pas");
        assert_eq!(v[0].nb_creances_en_retard, 1);
        assert_eq!(v[0].jours_plus_vieille_creance, Some(31));

        let mut b = a.clone();
        b.creances = vec![(30, 5_000)];
        assert!(juger(&[b], 60, true).is_empty(), "30 jours tout juste : pas encore en retard");

        assert!(juger(&[a], 60, false).is_empty(), "sans tiers:lire_solde, pas de motif creance");
    }

    #[test]
    fn un_meilleur_client_qui_ralentit() {
        // Reference : 3 achats de 90 000 (moyenne 90 000 / 90 jours).
        let mut a = client("Awa");
        a.jours_dernier_achat = Some(20);
        a.achats = vec![(100, 90_000), (200, 90_000), (300, 90_000), (20, 44_999)];
        let v = juger(&[a.clone()], 60, true);
        assert_eq!(v[0].motifs, vec![MOTIF_RALENTIT]);
        assert_eq!(v[0].ca_reference, 270_000);
        assert_eq!(v[0].ca_recent, 44_999);

        a.achats[3].1 = 45_000;
        assert!(juger(&[a.clone()], 60, true).is_empty(), "la moitie tout juste : ne ralentit pas");

        // Deux achats seulement dans la reference : pas un fidele.
        a.achats = vec![(100, 900_000), (200, 900_000)];
        assert!(juger(&[a], 60, true).is_empty());
    }

    #[test]
    fn seuls_les_dix_meilleurs_comptent() {
        let releves: Vec<Releve> = (0..11)
            .map(|i| {
                let mut r = client(&format!("C{i:02}"));
                // Tous ralentis (rien de recent) ; C00 le plus petit.
                let m = 10_000 * (i + 1);
                r.achats = vec![(100, m), (200, m), (300, m)];
                r.jours_dernier_achat = Some(100);
                r
            })
            .collect();
        let v = juger(&releves, 365, true);
        assert_eq!(v.len(), 10);
        assert!(!v.iter().any(|x| x.id == "c00"), "le onzieme n'est pas un meilleur client");
        assert_eq!(v[0].id, "c10", "le plus gros chiffre d'abord");
    }

    #[test]
    fn un_contact_recent_ou_un_rappel_met_de_cote() {
        let mut a = client("Awa");
        a.jours_dernier_achat = Some(90);
        a.jours_dernier_contact = Some(13);
        assert!(juger(&[a.clone()], 60, true)[0].mis_de_cote);
        a.jours_dernier_contact = Some(14);
        assert!(!juger(&[a.clone()], 60, true)[0].mis_de_cote);
        a.rappel_actif = true;
        assert!(juger(&[a], 60, true)[0].mis_de_cote);
    }

    #[test]
    fn l_ordre_creances_ralentis_dormants() {
        let mut dormant_vieux = client("Dormant vieux");
        dormant_vieux.jours_dernier_achat = Some(200);
        let mut dormant_frais = client("Dormant frais");
        dormant_frais.jours_dernier_achat = Some(61);
        let mut ralenti = client("Ralenti");
        ralenti.jours_dernier_achat = Some(95);
        ralenti.achats = vec![(95, 10_000), (150, 10_000), (250, 10_000)];
        let mut creance = client("Creance");
        creance.jours_dernier_achat = Some(40);
        creance.creances = vec![(40, 1_000)];
        let v = juger(&[dormant_vieux, dormant_frais, ralenti, creance], 60, true);
        let ids: Vec<&str> = v.iter().map(|x| x.id.as_str()).collect();
        assert_eq!(ids, vec!["creance", "ralenti", "dormant frais", "dormant vieux"]);
        assert_eq!(v[1].motifs, vec![MOTIF_RALENTIT, MOTIF_DORMANT], "ses deux motifs sur une ligne");
    }

    #[test]
    fn un_parti_ne_reste_que_pour_sa_creance() {
        let mut doit = client("Awa");
        doit.parti = true;
        doit.jours_dernier_achat = Some(120);
        doit.achats = vec![(100, 90_000), (200, 90_000), (300, 90_000)];
        doit.creances = vec![(120, 5_000)];
        let mut quitte = client("Bakary");
        quitte.parti = true;
        quitte.jours_dernier_achat = Some(120);
        let v = juger(&[doit, quitte], 60, true);
        assert_eq!(v.len(), 1, "parti sans dette : hors de la liste");
        assert_eq!(v[0].id, "awa");
        assert_eq!(v[0].motifs, vec![MOTIF_CREANCE], "ni dormant, ni ralenti");
    }

    #[test]
    fn le_texte_propose_ne_dit_aucun_montant() {
        let mut a = client("Awa");
        a.jours_dernier_achat = Some(75);
        a.creances = vec![(75, 12_500)];
        let v = &juger(&[a], 60, true)[0];
        let t = texte_propose(v, Some(75));
        assert_eq!(t, "Relance : facture impayée depuis 75 jours, pas d'achat depuis 75 jours");
        assert!(!t.contains("12"));
    }
}
