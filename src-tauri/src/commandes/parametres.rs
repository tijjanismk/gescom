//! Facades Tauri de `parametres`.
//!
//! La logique vit dans `gescom_noyau::parametres` : le serveur v2
//! execute exactement le meme code que le comptoir.

use tauri::State;
use crate::commandes::ventes::EtatApp;

#[tauri::command]
pub fn lire_categories(
    etat: State<EtatApp>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::parametres::lire_categories(&conn, )
}
#[tauri::command]
pub fn creer_categorie(
    etat: State<EtatApp>,
    nom: String,
) -> Result<String, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::parametres::creer_categorie(&conn, nom)
}
#[tauri::command]
pub fn lire_articles_complets(
    etat: State<EtatApp>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::parametres::lire_articles_complets(&conn, )
}
#[tauri::command]
pub fn creer_article_complet(
    etat: State<EtatApp>,
    nom: String,
    categorie_id: Option<String>,
    unite_base: String,
    prix_reference: i64,
) -> Result<String, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::parametres::creer_article_complet(&conn, nom, categorie_id, unite_base, prix_reference)
}
#[tauri::command]
pub fn ajouter_unite_vente(
    etat: State<EtatApp>,
    article_id: String,
    libelle: String,
    facteur: f64,
    prix_reference: i64,
    code_barre: Option<String>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::parametres::ajouter_unite_vente(&conn, article_id, libelle, facteur, prix_reference, code_barre)
}
#[tauri::command]
pub fn modifier_unite_vente(
    etat: State<EtatApp>,
    unite_id: String,
    libelle: Option<String>,
    facteur: Option<f64>,
    prix_reference: Option<i64>,
    code_barre: Option<String>,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::parametres::modifier_unite_vente(&conn, unite_id, libelle, facteur, prix_reference, code_barre)
}
#[tauri::command]
pub fn desactiver_unite_vente(
    etat: State<EtatApp>,
    unite_id: String,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::parametres::desactiver_unite_vente(&conn, unite_id)
}
#[tauri::command]
pub fn lire_config_bon_sortie(
    etat: State<EtatApp>,
) -> Result<bool, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::parametres::lire_config_bon_sortie(&conn, )
}
#[tauri::command]
pub fn sauvegarder_config_bon_sortie(
    etat: State<EtatApp>,
    actif: bool,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::parametres::sauvegarder_config_bon_sortie(&conn, actif)
}
#[tauri::command]
pub fn lire_config_suivi_livraison(
    etat: State<EtatApp>,
) -> Result<bool, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::parametres::lire_config_suivi_livraison(&conn, )
}
#[tauri::command]
pub fn sauvegarder_config_suivi_livraison(
    etat: State<EtatApp>,
    actif: bool,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::parametres::sauvegarder_config_suivi_livraison(&conn, actif)
}
#[tauri::command]
pub fn lire_config_signatures(
    etat: State<EtatApp>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::parametres::lire_config_signatures(&conn, )
}
#[tauri::command]
pub fn sauvegarder_config_signatures(
    etat: State<EtatApp>,
    valeurs: std::collections::HashMap<String, String>,
) -> Result<(), String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::parametres::sauvegarder_config_signatures(&conn, valeurs)
}
#[tauri::command]
pub fn lire_stocks(
    etat: State<EtatApp>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::parametres::lire_stocks(&conn, )
}
#[tauri::command]
pub fn diagnostiquer_base(
    etat: State<EtatApp>,
) -> Result<serde_json::Value, String> {
    let conn = etat.conn.lock().map_err(|e| e.to_string())?;
    gescom_noyau::parametres::diagnostiquer_base(&conn, )
}

// =====================================================================
//  Restees ici : elles ont besoin de Tauri
// =====================================================================

/// Entretien de la base — reindexation et compactage.
///
/// L'equivalent de la « reparation » des anciens logiciels de gestion.
/// Ne recree AUCUNE donnee perdue : si `quick_check` signale une
/// corruption, la seule issue est la restauration d'une sauvegarde, et
/// cette commande refuse plutot que de donner un faux espoir.
///
/// Une copie horodatee est faite AVANT, systematiquement : VACUUM
/// reecrit tout le fichier, et sur une base fragile cette reecriture
/// peut aggraver les degats.
#[tauri::command]
pub fn entretenir_base(
    app: tauri::AppHandle,
    etat: State<EtatApp>,
    utilisateur_role: Option<String>,
) -> Result<serde_json::Value, String> {
    if utilisateur_role.as_deref() != Some("patron") {
        return Err("Réservé au patron".to_string());
    }

    let conn = etat.conn.lock().map_err(|e| e.to_string())?;

    // Refus si la base est deja corrompue : REINDEX et VACUUM
    // supposent un fichier sain. Les lancer dessus, c'est risquer de
    // perdre ce qui restait lisible.
    if let Some(detail) = crate::persistance::verifier_integrite(&conn)
        .map_err(|e| e.to_string())?
    {
        return Err(format!(
            "Base endommagée — l'entretien ne peut rien réparer. \
             Restaurer la dernière sauvegarde. Détail : {}",
            detail
        ));
    }

    use tauri::Manager;
    let dossier = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let copie = dossier.join(format!(
        "gescom_avant_entretien_{}.db",
        chrono::Local::now().format("%Y%m%d_%H%M%S")
    ));
    // Une copie de la veille ne sert a rien si l'entretien casse
    // quelque chose : on veut celle d'il y a trente secondes.
    let _ = std::fs::remove_file(&copie);

    let avant: i64 = conn.query_row(
        "SELECT page_count * page_size FROM pragma_page_count(), pragma_page_size()",
        [], |r| r.get(0),
    ).unwrap_or(0);

    // Reparation des reglements fournisseur globaux non imputes.
    // AVANT le VACUUM, tant que la copie de securite vient d'etre faite
    // et que la base est encore dans son etat d'origine.
    let reimputes =
        crate::commandes::chantiers::reimputer_paiements_globaux(&conn)?;

    let apres = crate::persistance::entretenir(&conn, &copie.to_string_lossy())
        .map_err(|e| format!("Entretien interrompu : {}", e))?;

    Ok(serde_json::json!({
        "copie":        copie.to_string_lossy(),
        "taille_avant": avant,
        "taille_apres": apres as i64,
        "gagne":        (avant - apres as i64).max(0),
        // Reglements globaux redistribues sur leurs factures.
        "reimputes":    reimputes,
    }))
}