//! Les erreurs des caisses, dans le journal du serveur (v3, B-2).
//!
//! Une caisse affiche « erreur technique » a 11 h 04 ; le soir, rien
//! nulle part : la fenetre ecrivait ses erreurs dans une console que
//! personne n'ouvre. Desormais `window.onerror` et
//! `unhandledrejection` envoient `POST /journal-poste`, et le serveur
//! ecrit une ligne `[POSTE ]` avec le nom du poste :
//!
//! ```text
//! 2026-09-23T11:04:12.031 [POSTE ] 192.168.1.12 POST /journal-poste · CAISSE-1 · Ventes · TypeError: …
//! ```
//!
//! Trois bornes, parce qu'un poste en boucle d'erreur ne doit pas
//! remplir le disque du serveur : jeton exige, 4 Ko par envoi, 10 par
//! minute et par poste — au-dela on jette, et on le dit UNE fois par
//! minute. Rien de plus : pas de telemetrie, pas de « clics ».

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

/// Un envoi plus gros est refuse sans etre lu plus loin.
pub const TAILLE_MAX: usize = 4 * 1024;
/// Par poste, sur une minute glissante.
pub const PAR_MINUTE: usize = 10;
const FENETRE: Duration = Duration::from_secs(60);
/// Ce qu'on garde du message, de la page, de la pile.
const MESSAGE_MAX: usize = 600;
const PILE_MAX: usize = 1200;

/// Ce que la fenetre envoie.
#[derive(Debug, Default, serde::Deserialize)]
pub struct Signalement {
    /// L'ecran ouvert (« ventes », « historique »…).
    #[serde(default)]
    pub page: String,
    #[serde(default)]
    pub message: String,
    /// Les premieres lignes de la pile, si le navigateur la donne.
    #[serde(default)]
    pub pile: String,
    /// « erreur » ou « promesse ».
    #[serde(default)]
    pub genre: String,
}

/// La verdict du limiteur pour un envoi.
#[derive(Debug, PartialEq, Eq)]
pub enum Verdict {
    Accepte,
    /// Le premier refus de la fenetre : on ecrit une ligne qui le dit.
    PremierRefus,
    /// Les suivants : on jette sans rien ecrire.
    Refuse,
}

/// Dix par minute et par poste.
#[derive(Default)]
pub struct Limiteur {
    envois: HashMap<String, VecDeque<Instant>>,
    signale: HashMap<String, Instant>,
}

impl Limiteur {
    pub fn juger(&mut self, poste: &str, maintenant: Instant) -> Verdict {
        let file = self.envois.entry(poste.to_string()).or_default();
        while file.front().is_some_and(|t| maintenant.duration_since(*t) >= FENETRE) {
            file.pop_front();
        }
        if file.len() < PAR_MINUTE {
            file.push_back(maintenant);
            return Verdict::Accepte;
        }
        match self.signale.get(poste) {
            Some(t) if maintenant.duration_since(*t) < FENETRE => Verdict::Refuse,
            _ => {
                self.signale.insert(poste.to_string(), maintenant);
                Verdict::PremierRefus
            }
        }
    }
}

/// Une ligne, et une seule : un retour a la ligne venu de la fenetre
/// fabriquerait une fausse ligne du journal (« [ERREUR] … » ecrit par
/// n'importe quelle caisse). Les caracteres de controle disparaissent,
/// la longueur est bornee.
pub fn nettoyer(texte: &str, max: usize) -> String {
    let une_ligne: String = texte
        .chars()
        .map(|c| if c == '\n' || c == '\r' || c == '\t' { ' ' } else { c })
        .filter(|c| !c.is_control())
        .collect();
    let compacte = une_ligne.split_whitespace().collect::<Vec<_>>().join(" ");
    if compacte.chars().count() <= max {
        compacte
    } else {
        let mut s: String = compacte.chars().take(max).collect();
        s.push('…');
        s
    }
}

/// Le texte de la ligne `[POSTE ]`, sans l'horodatage ni le contexte.
pub fn ligne(nom_poste: &str, s: &Signalement) -> String {
    let page = nettoyer(&s.page, 40);
    let message = nettoyer(&s.message, MESSAGE_MAX);
    let genre = if s.genre == "promesse" { "promesse rejetée · " } else { "" };
    let mut l = format!(
        "{} · {} · {genre}{}",
        nettoyer(nom_poste, 60),
        if page.is_empty() { "?" } else { &page },
        if message.is_empty() { "(sans message)" } else { &message },
    );
    let pile = nettoyer(&s.pile, PILE_MAX);
    if !pile.is_empty() {
        l.push_str(" · pile : ");
        l.push_str(&pile);
    }
    l
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dix_par_minute_puis_un_seul_refus_ecrit() {
        let mut l = Limiteur::default();
        let t0 = Instant::now();
        for i in 0..PAR_MINUTE {
            assert_eq!(l.juger("caisse-1", t0 + Duration::from_millis(i as u64)), Verdict::Accepte, "envoi {i}");
        }
        assert_eq!(l.juger("caisse-1", t0 + Duration::from_secs(1)), Verdict::PremierRefus);
        assert_eq!(l.juger("caisse-1", t0 + Duration::from_secs(2)), Verdict::Refuse);
        // Un autre poste a son propre compte.
        assert_eq!(l.juger("caisse-2", t0 + Duration::from_secs(2)), Verdict::Accepte);
        // Une minute apres le premier envoi, la place revient.
        assert_eq!(l.juger("caisse-1", t0 + Duration::from_secs(61)), Verdict::Accepte);
    }

    #[test]
    fn un_message_ne_fabrique_pas_de_fausse_ligne() {
        let s = Signalement {
            page: "ventes".into(),
            message: "TypeError: x\n2026-09-23T11:00:00 [ERREUR] faux\r\u{0}".into(),
            pile: "at a (Ventes.tsx:1)\n    at b (App.tsx:2)".into(),
            genre: "erreur".into(),
        };
        let l = ligne("CAISSE-1", &s);
        assert!(!l.contains('\n') && !l.contains('\r') && !l.contains('\u{0}'), "{l}");
        assert!(l.starts_with("CAISSE-1 · ventes · TypeError: x"), "{l}");
        assert!(l.contains("pile : at a (Ventes.tsx:1) at b (App.tsx:2)"), "{l}");
    }

    #[test]
    fn un_message_trop_long_est_coupe() {
        let long = "é".repeat(5000);
        let n = nettoyer(&long, MESSAGE_MAX);
        assert_eq!(n.chars().count(), MESSAGE_MAX + 1);
        assert!(n.ends_with('…'));
        assert_eq!(ligne("P", &Signalement::default()), "P · ? · (sans message)");
    }
}
