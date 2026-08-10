// Un utilisateur avec son rôle
pub struct ContexteUtilisateur {
    pub id: String,
    pub role: String,  // "patron" / "employe" / "lecture"
}

// La vérification — permissive pour l'instant
pub fn verifier_permission(
    contexte: &ContexteUtilisateur,
    permission: &str,
) -> Result<(), ErreurPermission> {
    // Pour l'instant : le patron peut tout,
    // l'employé peut vendre/encaisser,
    // lecture peut juste lire.
    match (contexte.role.as_str(), permission) {
        ("patron", _)                    => Ok(()),
        ("employe", "ventes:creer")      => Ok(()),
        ("employe", "paiements:creer")   => Ok(()),
        ("employe", "stock:lire")        => Ok(()),
        ("lecture", p) if p.starts_with("lire") => Ok(()),
        _ => Err(ErreurPermission::Refuse),
    }
}