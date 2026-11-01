//! D55 : trouver le serveur depuis l'ecran de connexion — l'adresse de
//! Gescom Equipe (lecture, ecriture, priorite sur la caisse, refus hors
//! de la plage) et l'essai d'une adresse tapee, partage par la caisse et
//! Equipe (`noyau::recherche_serveur`).

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::time::Duration;

use gescom_noyau::recherche_serveur::{
    adresse_equipe, ecrire_empreinte_equipe, enregistrer_adresse_equipe, lire_reglage_equipe, tester_serveur,
    SOURCE_CAISSE, SOURCE_DEFAUT, SOURCE_EQUIPE,
};

/// Un dossier a soi, vide, par test.
fn dossier(nom: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("gescom-d55-{nom}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

// ---------------------------------------------------------------------
//  La priorite
// ---------------------------------------------------------------------

#[test]
fn l_adresse_reglee_dans_equipe_passe_devant_la_caisse() {
    let a = adresse_equipe(Some("192.168.1.20:40003"), "poste", "192.168.1.10");
    assert_eq!((a.serveur.as_str(), a.source), ("192.168.1.20:40003", SOURCE_EQUIPE));
}

#[test]
fn sans_reglage_equipe_suit_la_caisse_du_poste() {
    let a = adresse_equipe(None, "poste", " 192.168.1.10 ");
    assert_eq!((a.serveur.as_str(), a.source), ("192.168.1.10", SOURCE_CAISSE));
    // Le poste qui garde la base : son serveur local, au port qu'il a pris.
    let a = adresse_equipe(None, "monoposte", "127.0.0.1:40002");
    assert_eq!((a.serveur.as_str(), a.source), ("127.0.0.1:40002", SOURCE_CAISSE));
}

#[test]
fn sans_caisse_ni_reglage_le_serveur_de_cette_machine() {
    for (mode, serveur) in [("", ""), ("a_choisir", "192.168.1.10"), ("poste", "  ")] {
        let a = adresse_equipe(None, mode, serveur);
        assert_eq!((a.serveur.as_str(), a.source), ("127.0.0.1:40000", SOURCE_DEFAUT), "{mode:?} {serveur:?}");
    }
}

#[test]
fn une_adresse_hors_de_la_plage_est_ignoree_a_la_lecture() {
    // Equipe retouchee a la main en 7300 : on retombe sur la caisse.
    let a = adresse_equipe(Some("192.168.1.20:7300"), "poste", "192.168.1.10:40001");
    assert_eq!((a.serveur.as_str(), a.source), ("192.168.1.10:40001", SOURCE_CAISSE));
    // La caisse aussi hors de la plage : le serveur de cette machine.
    let a = adresse_equipe(Some("x:abc"), "poste", "192.168.1.10:7300");
    assert_eq!(a.source, SOURCE_DEFAUT);
}

// ---------------------------------------------------------------------
//  Le fichier equipe.json
// ---------------------------------------------------------------------

#[test]
fn l_adresse_s_ecrit_se_relit_et_garde_l_empreinte() {
    let f = dossier("ecrire").join("equipe.json");
    ecrire_empreinte_equipe(&f, "empreinte-1").unwrap();
    enregistrer_adresse_equipe(&f, Some(" http://SERVEUR-BOUTIQUE:40007/ ")).unwrap();
    let r = lire_reglage_equipe(&f);
    assert_eq!(r.empreinte.as_deref(), Some("empreinte-1"), "l'empreinte survit a l'adresse");
    assert_eq!(r.serveur.as_deref(), Some("SERVEUR-BOUTIQUE:40007"));

    // L'empreinte tiree apres l'adresse ne l'efface pas non plus.
    ecrire_empreinte_equipe(&f, "empreinte-2").unwrap();
    assert_eq!(lire_reglage_equipe(&f).serveur.as_deref(), Some("SERVEUR-BOUTIQUE:40007"));
    assert!(!f.with_extension("json.tmp").exists(), "pas de fichier temporaire laisse");
}

#[test]
fn reprendre_celle_de_la_caisse_efface_l_adresse_seulement() {
    let f = dossier("effacer").join("equipe.json");
    ecrire_empreinte_equipe(&f, "e").unwrap();
    enregistrer_adresse_equipe(&f, Some("192.168.1.20")).unwrap();
    enregistrer_adresse_equipe(&f, None).unwrap();
    let r = lire_reglage_equipe(&f);
    assert_eq!((r.empreinte.as_deref(), r.serveur.as_deref()), (Some("e"), None));
    let a = adresse_equipe(r.serveur.as_deref(), "poste", "192.168.1.10");
    assert_eq!(a.source, SOURCE_CAISSE);
}

#[test]
fn une_adresse_hors_de_la_plage_ou_vide_est_refusee_sans_toucher_au_fichier() {
    let f = dossier("refus").join("equipe.json");
    enregistrer_adresse_equipe(&f, Some("192.168.1.20:40100")).unwrap();
    let avant = std::fs::read_to_string(&f).unwrap();
    for non in ["192.168.1.20:7300", "192.168.1.20:40101", "192.168.1.20:39999", "serveur:abc", "  "] {
        let e = enregistrer_adresse_equipe(&f, Some(non)).unwrap_err();
        assert!(!e.is_empty(), "{non}");
    }
    assert!(enregistrer_adresse_equipe(&f, Some("192.168.1.20:7300")).unwrap_err().contains("40000"));
    assert_eq!(std::fs::read_to_string(&f).unwrap(), avant, "le reglage precedent reste");
}

#[test]
fn un_fichier_absent_illisible_ou_avec_bom_se_lit_sans_panique() {
    let d = dossier("lire");
    assert_eq!(lire_reglage_equipe(&d.join("absent.json")), Default::default());
    let illisible = d.join("illisible.json");
    std::fs::write(&illisible, "{ pas du json").unwrap();
    assert_eq!(lire_reglage_equipe(&illisible), Default::default());
    // Le Bloc-notes enregistre « avec BOM » ; et l'ancien fichier
    // (avant D55) n'avait que l'empreinte.
    let bom = d.join("bom.json");
    std::fs::write(&bom, "\u{feff}{\"empreinte\":\"ancienne\"}").unwrap();
    let r = lire_reglage_equipe(&bom);
    assert_eq!((r.empreinte.as_deref(), r.serveur.as_deref()), (Some("ancienne"), None));
    // On peut ecrire par-dessus un fichier illisible.
    enregistrer_adresse_equipe(&illisible, Some("192.168.1.20")).unwrap();
    assert_eq!(lire_reglage_equipe(&illisible).serveur.as_deref(), Some("192.168.1.20"));
}

// ---------------------------------------------------------------------
//  L'essai d'une adresse (la caisse et Equipe)
// ---------------------------------------------------------------------

/// Les essais reseau tournent l'un apres l'autre : un port lache par l'un
/// (« rien ne repond ») ne doit pas etre repris par un autre entre-temps.
static UN_ESSAI_A_LA_FOIS: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Un port libre de la plage, pris dans 40060-40099 (reserve aux essais :
/// 40000 et 40050 tiennent des serveurs du proprietaire). Libre = rien
/// n'y repond ET on peut s'y lier.
fn port_libre_d_essai() -> (u16, TcpListener) {
    for p in (40060..=40099).rev() {
        let occupe = TcpStream::connect_timeout(&format!("127.0.0.1:{p}").parse().unwrap(), Duration::from_millis(300)).is_ok();
        if occupe {
            continue;
        }
        if let Ok(l) = TcpListener::bind(("127.0.0.1", p)) {
            return (p, l);
        }
    }
    panic!("aucun port libre de 40060 a 40099");
}

/// Repond une fois, avec ce texte, a la premiere connexion.
fn repondre_une_fois(l: TcpListener, reponse: &'static str) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        if let Ok((mut c, _)) = l.accept() {
            let mut tampon = [0u8; 1024];
            let _ = c.read(&mut tampon);
            let _ = c.write_all(reponse.as_bytes());
        }
    })
}

#[test]
fn l_essai_rend_la_sante_d_un_serveur_gescom() {
    let _tour = UN_ESSAI_A_LA_FOIS.lock().unwrap_or_else(|e| e.into_inner());
    let (port, l) = port_libre_d_essai();
    let fil = repondre_une_fois(
        l,
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"version_protocole\":1,\"base_saine\":true}",
    );
    let s = tester_serveur(&format!("127.0.0.1:{port}")).expect("serveur joignable");
    assert_eq!(s["base_saine"], true);
    fil.join().unwrap();
}

#[test]
fn l_essai_dit_quand_ce_n_est_pas_gescom() {
    let _tour = UN_ESSAI_A_LA_FOIS.lock().unwrap_or_else(|e| e.into_inner());
    let (port, l) = port_libre_d_essai();
    let fil = repondre_une_fois(l, "SSH-2.0-OpenSSH\r\n");
    let e = tester_serveur(&format!("127.0.0.1:{port}")).unwrap_err();
    assert!(e.contains("pas un serveur Gescom"), "{e}");
    fil.join().unwrap();
}

#[test]
fn l_essai_dit_quand_rien_ne_repond() {
    let _tour = UN_ESSAI_A_LA_FOIS.lock().unwrap_or_else(|e| e.into_inner());
    let (port, l) = port_libre_d_essai();
    drop(l);
    let e = tester_serveur(&format!("127.0.0.1:{port}")).unwrap_err();
    assert!(e.starts_with("Serveur injoignable"), "{e}");
}

#[test]
fn l_essai_refuse_hors_de_la_plage_sans_rien_essayer() {
    for non in ["127.0.0.1:7300", "192.168.1.10:40101", "192.168.1.10:abc"] {
        let e = tester_serveur(non).unwrap_err();
        assert!(!e.starts_with("Serveur injoignable"), "{non} : refuse avant d'essayer, pas apres ({e})");
    }
    assert!(tester_serveur("127.0.0.1:7300").unwrap_err().contains("40000"));
    assert_eq!(tester_serveur("  ").unwrap_err(), "Adresse vide.");
}
