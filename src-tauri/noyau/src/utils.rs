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
/// Conserve pour les 16 appelants du v1. La regle elle-meme a demenage
/// dans `caisses`, qui sait en plus servir une caisse nominative ;
/// cette signature suppose la caisse unique, c'est-a-dire le reglage
/// par defaut. Les appelants qui connaissent leur utilisateur doivent
/// appeler `caisses::exiger` directement.
pub fn exiger_session_caisse(conn: &rusqlite::Connection) -> Result<String, String> {
    crate::caisses::exiger(conn, None)
}
