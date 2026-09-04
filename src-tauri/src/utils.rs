//! Utilitaires partagés.

/// Retourne l'horodatage ISO 8601 actuel en heure locale.
pub fn maintenant_iso() -> String {
    chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%.3f").to_string()
}

/// Client de passage : pas d'identité, donc ni crédit ni avoir (D40).
///
/// Même prédicat que `lire_client_generique` (commandes/ventes.rs).
/// Un `client_id` inconnu n'est pas générique : la contrainte de clé
/// étrangère le rejettera plus loin, ce garde-fou n'a pas à le faire.
pub fn est_client_generique(conn: &rusqlite::Connection, client_id: &str) -> bool {
    conn.query_row(
        "SELECT est_generique FROM client WHERE id = ?1",
        rusqlite::params![client_id],
        |r| r.get::<_, i64>(0),
    )
    .map(|v| v != 0)
    .unwrap_or(false)
}

/// Session de caisse ouverte, ou une erreur explicite.
///
/// Sans session, `mouvement_caisse` n'est pas alimente : l'argent entre
/// ou sort du tiroir sans laisser de trace. Le journal diverge alors de
/// lui-meme — `encaisse_jour` lit la table `paiement`, `caisse_par_moyen`
/// lit `mouvement_caisse` — et la cloture suivante affiche un excedent
/// inexplicable.
///
/// Le refus vaut mieux que l'ecriture manquante : une operation
/// bloquee se voit, une ecriture absente ne se voit jamais.
pub fn exiger_session_caisse(conn: &rusqlite::Connection) -> Result<String, String> {
    conn.query_row(
        "SELECT id FROM session_caisse WHERE statut = 'ouverte' LIMIT 1",
        [],
        |r| r.get(0),
    )
    .map_err(|_| {
        "CAISSE_FERMEE — la caisse n'est pas ouverte. \
         L'ouvrir pour enregistrer cette opération."
            .to_string()
    })
}
