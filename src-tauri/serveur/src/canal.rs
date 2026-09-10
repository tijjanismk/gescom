//! Le canal : ce que le serveur pousse aux postes.
//!
//! ## Le probleme qu'il resout
//!
//! Deux caisses, un stock. La caisse 1 vend le dernier sac de ciment ;
//! la caisse 2 continue de l'afficher disponible jusqu'a ce que son
//! ecran soit rouvert. Le client repart, ou l'on vend a decouvert. Le
//! canal existe pour que la caisse 2 SACHE, sans avoir a demander
//! toutes les deux secondes.
//!
//! ## Longue attente, pas WebSocket
//!
//! Le poste demande les evenements depuis le numero N ; s'il n'y en a
//! pas, le serveur garde la requete ouverte jusqu'a trente secondes.
//! Un WebSocket ferait la meme chose avec une bibliotheque de plus, un
//! protocole de plus a deboguer, et le meme comportement au travers du
//! routeur de la boutique.
//!
//! Le numero d'ordre est ce qui rend la reconnexion sure : un poste
//! qui a perdu le reseau pendant deux minutes redemande depuis son
//! dernier numero et rattrape le retard, au lieu de repartir aveugle.

use std::collections::VecDeque;
use std::sync::{Condvar, Mutex};
use std::time::Duration;

use gescom_noyau::protocole::Evenement;
use gescom_noyau::utils::maintenant_iso;

/// Nombre d'evenements conserves.
///
/// Un poste absent plus longtemps que ces 500 evenements ne peut plus
/// rattraper par le canal : il est renvoye vers une relecture
/// complete, ce que `seq_max` lui signale. Garder tout l'historique en
/// memoire serait une fuite lente sur une machine qui ne redemarre
/// jamais.
const CAPACITE: usize = 500;

/// Duree maximale d'une longue attente.
///
/// Trente secondes : sous la minute des coupures TCP des routeurs
/// domestiques, et assez long pour que le poste ne rouvre pas une
/// connexion en boucle.
const ATTENTE_MAX: Duration = Duration::from_secs(30);

pub struct Canal {
    interne: Mutex<Interne>,
    signal: Condvar,
}

struct Interne {
    file: VecDeque<Evenement>,
    prochain_seq: u64,
}

impl Canal {
    pub fn nouveau() -> Self {
        Canal {
            interne: Mutex::new(Interne { file: VecDeque::new(), prochain_seq: 1 }),
            signal: Condvar::new(),
        }
    }

    /// Publie un evenement et reveille tous les postes en attente.
    pub fn publier(&self, genre: &str, entite: &str, entite_id: Option<String>, poste_id: &str) {
        let mut interne = match self.interne.lock() {
            Ok(g) => g,
            // Un verrou empoisonne signifie qu'un fil a panique en le
            // tenant. Le canal est un confort, pas une ecriture
            // comptable : on ne fait pas tomber le serveur pour lui.
            Err(e) => e.into_inner(),
        };
        let seq = interne.prochain_seq;
        interne.prochain_seq += 1;
        interne.file.push_back(Evenement {
            seq,
            genre: genre.to_string(),
            entite: entite.to_string(),
            entite_id,
            poste_id: poste_id.to_string(),
            horodatage: maintenant_iso(),
        });
        while interne.file.len() > CAPACITE {
            interne.file.pop_front();
        }
        drop(interne);
        self.signal.notify_all();
    }

    /// Les evenements posterieurs a `depuis`, en attendant qu'il y en
    /// ait. Retourne une liste vide si l'attente expire — le poste
    /// rappellera.
    pub fn depuis(&self, depuis: u64, exclure_poste: &str) -> (Vec<Evenement>, u64) {
        let interne = match self.interne.lock() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        };
        let (mut interne, _) = self
            .signal
            .wait_timeout_while(interne, ATTENTE_MAX, |i| {
                i.prochain_seq.saturating_sub(1) <= depuis
            })
            .unwrap_or_else(|e| e.into_inner());

        let seq_max = interne.prochain_seq.saturating_sub(1);
        let lot = interne
            .file
            .iter()
            // Le poste qui a provoque l'evenement a deja rafraichi son
            // ecran ; le lui renvoyer le ferait recharger deux fois,
            // dont une par-dessus une saisie en cours.
            .filter(|e| e.seq > depuis && e.poste_id != exclure_poste)
            .cloned()
            .collect();
        interne.file.make_contiguous();
        (lot, seq_max)
    }

    pub fn seq_courant(&self) -> u64 {
        match self.interne.lock() {
            Ok(g) => g.prochain_seq.saturating_sub(1),
            Err(e) => e.into_inner().prochain_seq.saturating_sub(1),
        }
    }
}
