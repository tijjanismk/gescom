//! Le changement de port ou d'ecoute du serveur, au journal (27/10/2026).
//!
//! La regle est dans `coeur::reseau_serveur` ; le serveur l'applique
//! (route `POST /reseau`) et ecrit ici la ligne de l'Historique, DANS la
//! transaction qu'il valide apres avoir ecrit `serveur.json` : un fichier
//! qui n'a pas pu s'ecrire ne laisse pas de ligne, et une ligne qui n'a
//! pas pu s'ecrire defait le geste.
//!
//! Pas de version `Connection` : seul le serveur se reconfigure, et il
//! sert tout par `Base` (D22).

use crate::base::Acces;
use crate::coeur::reseau_serveur::Reglage;
use crate::parametres;

/// Le type de la ligne au journal (libelle : `coeur::historique`).
pub const TYPE_JOURNAL: &str = "reseau_serveur_modifie";

/// Ecrit « le serveur passe de `ancien` a `nouveau` » au journal du
/// dossier ouvert, signe par `auteur` (la session, D26).
pub fn noter_changement_sur(
    acces: &mut impl Acces,
    auteur: &str,
    ancien: &Reglage,
    nouveau: &Reglage,
    maintenant: &str,
) -> Result<(), String> {
    let dossier = acces.dossier().to_string();
    acces
        .executer(
            "INSERT INTO journal
               (id, type_evenement, entite_type, entite_id, auteur_id,
                ancien_valeur, nouveau_valeur, origine, date_evenement, dossier_id)
             VALUES (?1, ?2, 'serveur', 'reseau', ?3, ?4, ?5, 'serveur', ?6, ?7)",
            &parametres![
                uuid::Uuid::new_v4().to_string(),
                TYPE_JOURNAL,
                auteur,
                ancien.en_texte(),
                nouveau.en_texte(),
                maintenant,
                dossier
            ],
        )
        .map(|_| ())
        .map_err(|e| e.0)
}
