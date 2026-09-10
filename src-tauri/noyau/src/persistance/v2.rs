//! Migrations du multiposte (v2).
//!
//! Idempotentes, comme le reste : une base v1 ouverte par le serveur
//! v2 se met a jour sans script externe, et une base v2 ouverte par un
//! client v1 continue de fonctionner — les colonnes ajoutees sont
//! toutes nullables ou pourvues d'un defaut.

use rusqlite::{Connection, Result};

pub fn migrer(conn: &Connection) -> Result<()> {
    // -----------------------------------------------------------------
    //  Postes
    // -----------------------------------------------------------------
    // Un poste est une MACHINE, pas un utilisateur. Deux caissiers qui
    // se relaient sur le meme comptoir partagent le poste ; le meme
    // caissier qui passe au bureau change de poste. C'est cette
    // distinction qui permet plus tard de dire « le tiroir suit le
    // comptoir » ou « le tiroir suit la personne » sans tout revoir.
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS poste (
            id              TEXT PRIMARY KEY,
            nom             TEXT NOT NULL,
            empreinte       TEXT NOT NULL UNIQUE,
            genre           TEXT NOT NULL DEFAULT 'caisse',
            actif           INTEGER NOT NULL DEFAULT 1,
            dernier_contact TEXT,
            derniere_ip     TEXT,
            cree_le         TEXT NOT NULL,
            modifie_le      TEXT NOT NULL
         );
         CREATE INDEX IF NOT EXISTS idx_poste_empreinte ON poste(empreinte);",
    )?;

    // -----------------------------------------------------------------
    //  Sessions reseau
    // -----------------------------------------------------------------
    // Le jeton n'est JAMAIS stocke en clair. Une base volee ou une
    // sauvegarde egaree sur une cle USB donnerait sinon huit heures
    // d'acces a chaque poste connecte.
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS session_reseau (
            id              TEXT PRIMARY KEY,
            jeton_hash      TEXT NOT NULL UNIQUE,
            poste_id        TEXT NOT NULL REFERENCES poste(id),
            utilisateur_id  TEXT NOT NULL REFERENCES utilisateur(id),
            ouvert_le       TEXT NOT NULL,
            expire_le       TEXT NOT NULL,
            revoque_le      TEXT,
            revoque_par     TEXT,
            derniere_vue    TEXT
         );
         CREATE INDEX IF NOT EXISTS idx_session_reseau_jeton
             ON session_reseau(jeton_hash);
         CREATE INDEX IF NOT EXISTS idx_session_reseau_actives
             ON session_reseau(revoque_le, expire_le);",
    )?;

    // -----------------------------------------------------------------
    //  La caisse apprend d'ou elle vient
    // -----------------------------------------------------------------
    // `mouvement_caisse` n'avait pas de `depot_id`, deliberement (D29) :
    // un seul tiroir. On ne casse pas ce choix, on l'ANNOTE. Tant que
    // `caisse_par_utilisateur` vaut 0, ces colonnes ne servent qu'a
    // savoir qui a ouvert quoi et depuis quelle machine — ce qui manque
    // deja aujourd'hui quand un ecart de cloture apparait.
    for sql in [
        "ALTER TABLE session_caisse ADD COLUMN poste_id TEXT",
        "ALTER TABLE session_caisse ADD COLUMN utilisateur_id TEXT",
        "ALTER TABLE mouvement_caisse ADD COLUMN poste_id TEXT",
    ] {
        conn.execute(sql, []).ok();
    }

    // Une seule caisse ouverte par utilisateur, quand le mode nominatif
    // est actif. L'index partiel ne contraint QUE les lignes concernees :
    // les sessions historiques, sans `utilisateur_id`, restent valides.
    conn.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_caisse_ouverte_par_utilisateur
         ON session_caisse(utilisateur_id)
         WHERE statut = 'ouverte' AND utilisateur_id IS NOT NULL",
        [],
    )
    .ok();

    // -----------------------------------------------------------------
    //  Reglages
    // -----------------------------------------------------------------
    // Defaut 0 : la boutique type de Bamako a UN tiroir. Le mode
    // nominatif se choisit, il ne s'impose pas — l'imposer obligerait
    // chaque commercant a ouvrir deux caisses pour un seul comptoir.
    conn.execute(
        "INSERT OR IGNORE INTO config_app (cle, valeur)
         VALUES ('caisse_par_utilisateur', '0')",
        [],
    )
    .ok();

    Ok(())
}
