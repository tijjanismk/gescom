//! La sauvegarde PostgreSQL, pour de vrai : `pg_dump` lancé par le
//! noyau sur la base jetable, puis `pg_restore --list` pour prouver
//! que le fichier produit est un vrai dump, pas un fichier vide.
//!
//! Ne tourne qu'avec `GESCOM_PG` — sans instance, rien à sauvegarder.

mod commun;

use commun::*;
use gescom_noyau::sauvegarde;

#[test]
fn pg_dump_produit_un_fichier_que_pg_restore_sait_lire() {
    if std::env::var("GESCOM_PG").is_err() {
        return;
    }
    let mut base = base_avec_demo();
    let dossier = std::env::temp_dir().join(format!("gescom-sauvegarde-{}", uuid::Uuid::new_v4()));

    let chemin = sauvegarde::sauvegarder_base_sur_base(&mut base, dossier.to_string_lossy().to_string())
        .expect("pg_dump");
    assert!(chemin.ends_with(".dump"), "{chemin}");
    let taille = std::fs::metadata(&chemin).unwrap().len();
    assert!(taille > 1_000, "un dump de la démo pèse plus que ça : {taille} octets");

    // Le journal en garde la trace, et l'écran la relit.
    let cfg = sauvegarde::lire_config_sauvegarde_sur_base(&mut base).unwrap();
    assert_eq!(cfg["derniere_sauvegarde"], chemin);
    assert_eq!(cfg["moteur"], "postgresql");

    // pg_restore, à côté de pg_dump, sait le lister : c'est un dump.
    let pg_restore = std::env::var("GESCOM_PG_RESTORE").ok().unwrap_or_else(|| {
        let mut c: Vec<_> = std::fs::read_dir("C:\\Program Files\\PostgreSQL")
            .map(|d| d.filter_map(|e| e.ok()).map(|e| e.path().join("bin").join("pg_restore.exe")).filter(|p| p.exists()).collect())
            .unwrap_or_default();
        c.sort();
        c.pop().map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|| "pg_restore".into())
    });
    let sortie = std::process::Command::new(pg_restore).arg("--list").arg(&chemin).output().expect("pg_restore");
    assert!(sortie.status.success(), "{}", String::from_utf8_lossy(&sortie.stderr));
    let liste = String::from_utf8_lossy(&sortie.stdout);
    assert!(liste.contains("TABLE public vente") || liste.contains("vente"), "{liste}");

    let _ = std::fs::remove_dir_all(&dossier);
}
