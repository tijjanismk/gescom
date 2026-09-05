//! Initialisation et ouverture de la base de données SQLite.

use rusqlite::{Connection, Result};

pub fn ouvrir_base(chemin: &str) -> Result<Connection> {
    let conn = Connection::open(chemin)?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
    Ok(conn)
}

/// Etat de sante de la base, lu au demarrage.
///
/// `quick_check` et non `integrity_check` : le premier saute la
/// verification des index, dix fois plus rapide, et suffit a detecter
/// une base tronquee ou corrompue — le cas reel apres une coupure de
/// courant en pleine ecriture, frequent a Bamako.
///
/// La verification ne repare RIEN. Elle dit au commercant qu'il doit
/// restaurer sa sauvegarde AVANT de saisir la journee par-dessus une
/// base abimee, ce qui rendrait la restauration inutile.
pub fn verifier_integrite(conn: &Connection) -> Result<Option<String>> {
    let resultat: String = conn.query_row(
        "PRAGMA quick_check(1)", [], |r| r.get(0),
    )?;
    if resultat == "ok" {
        Ok(None)
    } else {
        Ok(Some(resultat))
    }
}

/// Ecritures orphelines — coherence METIER, pas structurelle.
///
/// `quick_check` valide le fichier SQLite ; il ne dit rien d'une vente
/// dont les lignes ont disparu, ni d'un paiement sans vente. Ces cas
/// naissent d'une transaction interrompue, pas d'une corruption.
///
/// Retourne un compte par anomalie. Zero partout = base saine.
pub fn anomalies_metier(conn: &Connection) -> Vec<(String, i64)> {
    let controles: [(&str, &str); 5] = [
        ("Ventes sans aucune ligne",
         "SELECT COUNT(*) FROM vente v WHERE v.statut <> 'annulee'
            AND NOT EXISTS (SELECT 1 FROM ligne_vente WHERE vente_id = v.id)"),
        ("Paiements rattaches a une vente inexistante",
         "SELECT COUNT(*) FROM paiement p
            WHERE NOT EXISTS (SELECT 1 FROM vente WHERE id = p.vente_id)"),
        ("Lignes de piece sans piece",
         "SELECT COUNT(*) FROM ligne_piece lp
            WHERE NOT EXISTS (SELECT 1 FROM piece_commerciale WHERE id = lp.piece_id)"),
        ("Mouvements de caisse hors session",
         "SELECT COUNT(*) FROM mouvement_caisse mc
            WHERE NOT EXISTS (SELECT 1 FROM session_caisse WHERE id = mc.session_id)"),
        ("Stock negatif",
         "SELECT COUNT(*) FROM stock_depot WHERE quantite < 0"),
    ];

    controles.iter().filter_map(|(libelle, sql)| {
        let n: i64 = conn.query_row(sql, [], |r| r.get(0)).unwrap_or(0);
        if n > 0 { Some((libelle.to_string(), n)) } else { None }
    }).collect()
}

pub fn initialiser_tables(conn: &Connection) -> Result<()> {
    let schema = include_str!("schema.sql");
    conn.execute_batch(schema)?;

    // ---- Migrations colonnes (idempotentes) ----
    conn.execute(
        "ALTER TABLE ligne_vente ADD COLUMN taux_tva REAL NOT NULL DEFAULT 0.0", []
    ).ok();
    conn.execute(
        "ALTER TABLE ligne_vente ADD COLUMN montant_tva INTEGER NOT NULL DEFAULT 0", []
    ).ok();
    conn.execute(
        "ALTER TABLE session_caisse ADD COLUMN solde_theorique INTEGER", []
    ).ok();
    conn.execute(
        "ALTER TABLE session_caisse ADD COLUMN especes_comptees INTEGER", []
    ).ok();
    conn.execute(
        "ALTER TABLE session_caisse ADD COLUMN ecart INTEGER", []
    ).ok();
    conn.execute(
        "ALTER TABLE session_caisse ADD COLUMN ferme_le TEXT", []
    ).ok();
    conn.execute(
        "ALTER TABLE parametres_societe ADD COLUMN logo_chemin TEXT", []
    ).ok();
    // Bandeau d'en-tete : remplace logo + coordonnees a l'impression.
    // Beaucoup de commercants ont deja leur papier a en-tete chez
    // l'imprimeur et veulent le retrouver a l'ecran.
    conn.execute(
        "ALTER TABLE parametres_societe ADD COLUMN entete_chemin TEXT", []
    ).ok();
    // Pendant du bandeau d'en-tete, en bas de page : mentions legales,
    // coordonnees bancaires. Remplace la ligne `pied_facture`.
    conn.execute(
        "ALTER TABLE parametres_societe ADD COLUMN pied_chemin TEXT", []
    ).ok();
    conn.execute(
        "ALTER TABLE article ADD COLUMN taux_tva_defaut REAL NOT NULL DEFAULT 0.0", []
    ).ok();
    // Lien vente -> piece_commerciale (facture POS automatique, D16).
    conn.execute(
        "ALTER TABLE vente ADD COLUMN piece_id TEXT", []
    ).ok();
    // L3 — rattachement des achats a leur fournisseur.
    // Sans cette colonne, toutes les dettes fournisseurs sont identiques.
    conn.execute(
        "ALTER TABLE mouvement_stock ADD COLUMN fournisseur_id TEXT", []
    ).ok();
    // Prix d'achat AU MOMENT du mouvement : sinon tout l'historique est
    // recalcule au dernier prix connu (article.dernier_prix_achat).
    conn.execute(
        "ALTER TABLE mouvement_stock ADD COLUMN prix_achat_unitaire INTEGER", []
    ).ok();
    // Imputation : a quelle facture fournisseur ce paiement se rapporte.
    // NULL = paiement global non impute (anciens reglements).
    conn.execute(
        "ALTER TABLE paiement_fournisseur ADD COLUMN piece_id TEXT", []
    ).ok();
    // Bug #8 — lien avoir -> AVC. Sans lui, un avoir entierement
    // consomme continuait d'afficher son montant plein en « reste »
    // dans l'ecran Pieces : `total_paye` ne trouve jamais de paiement
    // pour un avoir, ni via `paiement`, ni via `paiement_fournisseur`.
    // Les avoirs anterieurs restent NULL — leur AVC affichera 0, ce qui
    // est degrade mais juste : un avoir deja consomme ne doit rien
    // montrer.
    conn.execute(
        "ALTER TABLE avoir ADD COLUMN piece_id TEXT", []
    ).ok();
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_avoir_piece ON avoir(piece_id)"
    ).ok();

    // D45 — code-barres PAR UNITE de vente. En boutique le carton porte
    // son propre EAN, different de celui de la piece. La colonne sur
    // `article` ne permettait qu'un seul code : scanner un carton etait
    // impossible par construction. `article.code_barre` reste en place
    // pour l'unite de base et l'historique.
    conn.execute(
        "ALTER TABLE unite_vente ADD COLUMN code_barre TEXT", []
    ).ok();
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_unite_code_barre
            ON unite_vente(code_barre)"
    ).ok();

    // Depenses : un mouvement de caisse libre a besoin d'un libelle.
    // motif reste la categorie technique ('vente', 'achat', 'depense'),
    // libelle porte le texte saisi par le commercant.
    conn.execute(
        "ALTER TABLE mouvement_caisse ADD COLUMN libelle TEXT", []
    ).ok();
    // Poste de depense, pour ventiler le journal (transport, loyer...).
    conn.execute(
        "ALTER TABLE mouvement_caisse ADD COLUMN categorie TEXT", []
    ).ok();

    // ---- v1.2 : transferts inter-depots ----
    // `bon` regroupe les lignes d'un meme bon numerote BTR-AAAA-NNNNN.
    // La table transfert existait depuis l'origine mais n'etait pas
    // utilisee ; elle n'avait ni bon, ni unite, ni motif.
    conn.execute(
        "ALTER TABLE transfert ADD COLUMN bon TEXT", []
    ).ok();
    conn.execute(
        "ALTER TABLE transfert ADD COLUMN unite_vente_id TEXT", []
    ).ok();
    conn.execute(
        "ALTER TABLE transfert ADD COLUMN motif TEXT", []
    ).ok();
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_transfert_bon ON transfert(bon)"
    ).ok();

    // ---- v1.2 : suivi des cheques recus ----
    // Un cheque est une promesse, pas de l'argent. Il n'entre PAS dans
    // le rapprochement de caisse (D29) — comme le mobile money.
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS cheque_recu (
            id            TEXT PRIMARY KEY,
            paiement_id   TEXT,
            vente_id      TEXT,
            numero        TEXT NOT NULL,
            banque        TEXT NOT NULL,
            tireur        TEXT,
            montant       INTEGER NOT NULL,
            date_emission TEXT,
            date_echeance TEXT,
            statut        TEXT NOT NULL DEFAULT 'recu',
            motif_rejet   TEXT,
            cree_le       TEXT NOT NULL,
            modifie_le    TEXT NOT NULL,
            origine       TEXT NOT NULL DEFAULT 'app'
        )"
    ).ok();
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_cheque_statut ON cheque_recu(statut)"
    ).ok();
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_cheque_vente ON cheque_recu(vente_id)"
    ).ok();

    // ---- Nouvelles tables (une par une pour éviter stack overflow) ----

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS piece_commerciale (
            id               TEXT PRIMARY KEY,
            type_piece       TEXT NOT NULL,
            numero           TEXT NOT NULL UNIQUE,
            statut           TEXT NOT NULL DEFAULT 'brouillon',
            tiers_type       TEXT NOT NULL DEFAULT 'client',
            tiers_id         TEXT NOT NULL,
            depot_id         TEXT,
            piece_origine_id TEXT,
            auteur_id        TEXT,
            date_piece       TEXT NOT NULL,
            date_echeance    TEXT,
            remise_globale   REAL NOT NULL DEFAULT 0,
            note             TEXT,
            cree_le          TEXT NOT NULL,
            modifie_le       TEXT NOT NULL,
            origine          TEXT NOT NULL DEFAULT 'app'
        );"
    ).ok();

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS ligne_piece (
            id               TEXT PRIMARY KEY,
            piece_id         TEXT NOT NULL,
            article_id       TEXT NOT NULL,
            unite_vente_id   TEXT NOT NULL,
            quantite         REAL NOT NULL,
            prix_unitaire    INTEGER NOT NULL,
            remise_pct       REAL NOT NULL DEFAULT 0,
            remise_montant   INTEGER NOT NULL DEFAULT 0,
            taux_tva         REAL NOT NULL DEFAULT 0,
            montant_tva      INTEGER NOT NULL DEFAULT 0,
            montant_ht       INTEGER NOT NULL,
            cree_le          TEXT NOT NULL
        );"
    ).ok();

    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_piece_tiers
            ON piece_commerciale(tiers_id, tiers_type);"
    ).ok();

    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_ligne_piece ON ligne_piece(piece_id);"
    ).ok();

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS paiement_fournisseur (
            id              TEXT PRIMARY KEY,
            fournisseur_id  TEXT NOT NULL,
            montant         INTEGER NOT NULL,
            mode            TEXT NOT NULL DEFAULT 'especes',
            note            TEXT,
            auteur_id       TEXT,
            date_paiement   TEXT NOT NULL,
            cree_le         TEXT NOT NULL,
            origine         TEXT NOT NULL DEFAULT 'app'
        );"
    ).ok();

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS creance_irrecouvrable (
            id          TEXT PRIMARY KEY,
            vente_id    TEXT NOT NULL,
            motif       TEXT NOT NULL,
            auteur_id   TEXT,
            date_marque TEXT NOT NULL,
            cree_le     TEXT NOT NULL,
            origine     TEXT NOT NULL DEFAULT 'app'
        );"
    ).ok();

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS relance_creance (
            id           TEXT PRIMARY KEY,
            vente_id     TEXT NOT NULL,
            canal        TEXT NOT NULL DEFAULT 'whatsapp',
            note         TEXT,
            auteur_id    TEXT,
            date_relance TEXT NOT NULL,
            cree_le      TEXT NOT NULL,
            origine      TEXT NOT NULL DEFAULT 'app'
        );"
    ).ok();

    Ok(())
}
/// Entretien de la base : reconstruction des index et compactage.
///
/// C'est l'equivalent de la « reparation » des vieux logiciels de
/// gestion — et ce que ce mot recouvrait vraiment : une REINDEXATION.
/// Les bases Paradox de l'epoque perdaient leurs index bien plus
/// souvent que leurs donnees.
///
/// Ce que ca corrige reellement :
///   - un index desynchronise (recherches qui ne trouvent plus)
///   - un fichier qui a grossi apres beaucoup de suppressions
///
/// Ce que ca NE corrige PAS : une page de donnees corrompue. Aucune
/// commande SQL ne recree une donnee perdue. C'est pour ca qu'on exige
/// une copie AVANT — VACUUM reecrit tout le fichier, et sur une base
/// deja abimee cette reecriture peut aggraver les degats.
pub fn entretenir(conn: &Connection, copie_avant: &str) -> Result<u64> {
    // `VACUUM INTO` produit une copie COHERENTE, contenu du WAL inclus.
    // Une copie de fichier a la main donnerait une base amputee des
    // ecritures recentes — le piege documente dans MANUEL.md §13.
    //
    // Chemin en PARAMETRE LIE, jamais interpole : une apostrophe dans
    // un nom d'utilisateur Windows cassait la requete (cf. le meme
    // commentaire dans sauvegarde.rs).
    conn.execute("VACUUM INTO ?1", rusqlite::params![copie_avant])?;

    conn.execute_batch("REINDEX; VACUUM;")?;

    let pages: i64 = conn.query_row("PRAGMA page_count", [], |r| r.get(0))?;
    let taille: i64 = conn.query_row("PRAGMA page_size", [], |r| r.get(0))?;
    Ok((pages * taille) as u64)
}