//! Utilitaires partagés.

/// Retourne l'horodatage ISO 8601 actuel en heure locale.
pub fn maintenant_iso() -> String {
    chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%.3f").to_string()
}
