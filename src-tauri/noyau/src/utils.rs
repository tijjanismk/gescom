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

/// Nombre de jours ecoules depuis une date ISO (`AAAA-MM-JJ...`).
///
/// C'est le travail que `julianday('now') - julianday(x)` faisait en
/// SQL — une fonction qui n'existe pas sur PostgreSQL. En Rust, le
/// meme calcul vaut sur les deux moteurs, et il est testable sans base.
/// Une date illisible compte pour zero jour : mieux vaut un cheque qui
/// parait frais qu'un ecran qui casse.
pub fn jours_depuis(iso: &str) -> i64 {
    let Some(jour) = iso.get(..10) else { return 0 };
    let Ok(d) = chrono::NaiveDate::parse_from_str(jour, "%Y-%m-%d") else { return 0 };
    (chrono::Local::now().date_naive() - d).num_days()
}

#[cfg(test)]
mod tests_jours {
    use super::jours_depuis;

    #[test]
    fn aujourd_hui_vaut_zero() {
        let j = chrono::Local::now().format("%Y-%m-%dT10:00:00").to_string();
        assert_eq!(jours_depuis(&j), 0);
    }

    #[test]
    fn une_date_vieille_de_vingt_jours() {
        let d = chrono::Local::now().date_naive() - chrono::Duration::days(20);
        assert_eq!(jours_depuis(&d.format("%Y-%m-%d").to_string()), 20);
    }

    #[test]
    fn une_date_illisible_vaut_zero() {
        assert_eq!(jours_depuis("n'importe quoi"), 0);
        assert_eq!(jours_depuis(""), 0);
    }
}
