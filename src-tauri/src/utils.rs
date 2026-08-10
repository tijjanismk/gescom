//! Utilitaires partagés entre tous les modules.

use chrono::Utc;

/// Horodatage ISO 8601 UTC — remplace le stub "2024-01-01T00:00:00Z".
/// Utilisé par tous les modules de persistance pour cree_le / modifie_le.
pub fn maintenant_iso() -> String {
    Utc::now().to_rfc3339()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn horodatage_non_vide() {
        let ts = maintenant_iso();
        assert!(!ts.is_empty());
        // Format ISO 8601 : contient T et Z ou +
        assert!(ts.contains('T'));
    }
}
