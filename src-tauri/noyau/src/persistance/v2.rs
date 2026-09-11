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
    //  Modeles de documents
    // -----------------------------------------------------------------
    // Le modele est une DONNEE, pas du code : le commercant qui veut
    // son logo a droite et sa colonne « reference » en plus ne doit pas
    // attendre une version de Gescom. Et parce que c'est une donnee,
    // le serveur la distribue a toutes les caisses.
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS modele_document (
            id          TEXT PRIMARY KEY,
            genre       TEXT NOT NULL,
            nom         TEXT NOT NULL,
            format      TEXT NOT NULL DEFAULT 'a4',
            contenu     TEXT NOT NULL,
            est_defaut  INTEGER NOT NULL DEFAULT 0,
            actif       INTEGER NOT NULL DEFAULT 0,
            cree_le     TEXT NOT NULL,
            modifie_le  TEXT NOT NULL,
            modifie_par TEXT
         );
         CREATE INDEX IF NOT EXISTS idx_modele_genre ON modele_document(genre);",
    )?;

    // Un seul modele actif par genre. En base et pas seulement dans
    // l'ecran : deux actifs, et le document imprime depend de l'ordre
    // de lecture — donc change sans raison visible.
    conn.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_modele_actif_par_genre
         ON modele_document(genre) WHERE actif = 1",
        [],
    )
    .ok();

    // -----------------------------------------------------------------
    //  Le stock devient une consequence de ses mouvements
    // -----------------------------------------------------------------
    //
    // `stock_depot.quantite` etait un compteur qu'on incrementait a onze
    // endroits. Un compteur ne sait pas dire POURQUOI il vaut ce qu'il
    // vaut : quand un stock est faux, il n'y a rien a auditer. Et rien
    // n'empechait un chemin d'ecrire le compteur sans son mouvement —
    // l'import CSV le faisait — apres quoi le stock et le journal
    // racontaient deux histoires differentes.
    //
    // Desormais le compteur est maintenu par la BASE, dans la meme
    // transaction que le mouvement qui le justifie. Il reste la pour la
    // vitesse — le point de vente lit le stock de tout le catalogue a
    // chaque ouverture d'ecran — mais il n'est plus qu'un cache : la
    // verite est la somme des mouvements, et `verifier_coherence` sait
    // comparer les deux.
    //
    // `mouvement_stock` n'est jamais modifie ni supprime : c'est ce qui
    // rend un simple AFTER INSERT suffisant.

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_mouvement_stock_article_depot
         ON mouvement_stock(article_id, depot_id)",
        [],
    )
    .ok();

    // Reprise, une seule fois. Les bases existantes ont derive : le
    // compteur ne vaut pas la somme, a cause de l'import CSV et de
    // l'histoire. Basculer sans rien faire changerait tous les stocks
    // du jour au lendemain, et le commercant y verrait une perte.
    //
    // On ecrit donc un mouvement d'ajustement de l'ECART, pour que la
    // somme rejoigne le compteur — pas l'inverse. Ce que le commercant
    // voit aujourd'hui reste ce qu'il verra demain, et l'ecart devient
    // une ligne visible dans l'historique au lieu d'un mystere.
    let reprise_faite: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM config_app WHERE cle = 'stock_reprise_mouvements'",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);

    if reprise_faite == 0 {
        let maintenant = crate::utils::maintenant_iso();
        // Le declencheur n'existe pas encore : ces ajustements ne
        // toucheront pas au compteur, ce qui est exactement voulu.
        conn.execute(
            "INSERT INTO mouvement_stock
               (id, article_id, depot_id, type_mouvement, quantite_delta,
                motif, auteur_id, date_mouvement, cree_le, cree_par, origine)
             SELECT lower(hex(randomblob(16))), sd.article_id, sd.depot_id,
                    'ajustement',
                    sd.quantite - COALESCE((
                        SELECT SUM(ms.quantite_delta) FROM mouvement_stock ms
                        WHERE ms.article_id = sd.article_id
                          AND ms.depot_id = sd.depot_id), 0),
                    'Reprise : mise en concordance du stock et de son historique',
                    'migration', ?1, ?1, 'migration', 'migration'
             FROM stock_depot sd
             WHERE sd.quantite <> COALESCE((
                    SELECT SUM(ms.quantite_delta) FROM mouvement_stock ms
                    WHERE ms.article_id = sd.article_id
                      AND ms.depot_id = sd.depot_id), 0)",
            rusqlite::params![maintenant],
        )
        .ok();

        conn.execute(
            "INSERT OR IGNORE INTO config_app (cle, valeur)
             VALUES ('stock_reprise_mouvements', ?1)",
            rusqlite::params![maintenant],
        )
        .ok();
    }

    // Le declencheur, pose APRES la reprise : l'inverse aurait fait
    // compter les ajustements deux fois.
    conn.execute_batch(
        "CREATE TRIGGER IF NOT EXISTS stock_suit_les_mouvements
         AFTER INSERT ON mouvement_stock
         BEGIN
           INSERT INTO stock_depot (id, article_id, depot_id, quantite)
           VALUES (lower(hex(randomblob(16))), NEW.article_id, NEW.depot_id,
                   NEW.quantite_delta)
           ON CONFLICT(article_id, depot_id)
           DO UPDATE SET quantite = quantite + NEW.quantite_delta;
         END;",
    )?;

    // -----------------------------------------------------------------
    //  Numerotation des pieces
    // -----------------------------------------------------------------
    // Le numero se calculait par MAX(substr(numero, -5)) sur les pieces
    // deja ecrites. Cette lecture est faite AVANT la transaction qui
    // ecrit la piece : entre les deux, une autre vente peut lire le
    // meme maximum. Les deux fabriquent alors FAC-2026-00042, et la
    // seconde se heurte a la contrainte UNIQUE — au comptoir, un client
    // qui attend et une facture qui refuse de s'enregistrer.
    //
    // Un compteur par serie remplace la lecture : il s'incremente en
    // ecriture, donc SQLite serialise les deux demandes et chacune
    // repart avec son numero.
    //
    // La cle porte la serie ET l'annee — « FAC-2026 » — parce que la
    // numerotation repart a 1 chaque janvier.
    conn.execute(
        "CREATE TABLE IF NOT EXISTS compteur_piece (
            cle     TEXT PRIMARY KEY,
            dernier INTEGER NOT NULL
         )",
        [],
    )?;

    // Reprise, une seule fois. Un compteur qui repartirait de zero
    // refabriquerait FAC-2026-00001 alors que la piece existe : la
    // contrainte UNIQUE bloquerait la premiere vente du matin de la
    // mise a jour. On part donc du plus grand numero deja emis, serie
    // par serie et annee par annee, telles qu'elles se lisent dans les
    // pieces existantes.
    let reprise_faite: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM config_app WHERE cle = 'compteurs_repris'",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);

    if reprise_faite == 0 {
        // « FAC-2026-00042 » : la cle est tout ce qui precede les cinq
        // derniers chiffres et leur tiret.
        conn.execute(
            "INSERT OR REPLACE INTO compteur_piece (cle, dernier)
             SELECT substr(numero, 1, length(numero) - 6),
                    MAX(CAST(substr(numero, -5) AS INTEGER))
             FROM piece_commerciale
             WHERE numero IS NOT NULL AND length(numero) > 6
             GROUP BY substr(numero, 1, length(numero) - 6)",
            [],
        )
        .ok();

        conn.execute(
            "INSERT OR REPLACE INTO compteur_piece (cle, dernier)
             SELECT substr(bon, 1, length(bon) - 6),
                    MAX(CAST(substr(bon, -5) AS INTEGER))
             FROM transfert
             WHERE bon IS NOT NULL AND length(bon) > 6
             GROUP BY substr(bon, 1, length(bon) - 6)",
            [],
        )
        .ok();

        // Le code-barre interne n'a ni serie ni annee : une seule suite,
        // les dix chiffres qui suivent le prefixe « 20 ».
        conn.execute(
            "INSERT OR REPLACE INTO compteur_piece (cle, dernier)
             SELECT 'codebarre',
                    COALESCE(MAX(CAST(substr(code_barre, 3, 10) AS INTEGER)), 0)
             FROM article
             WHERE code_barre LIKE '20%' AND length(code_barre) = 13",
            [],
        )
        .ok();

        conn.execute(
            "INSERT OR IGNORE INTO config_app (cle, valeur)
             VALUES ('compteurs_repris', ?1)",
            rusqlite::params![crate::utils::maintenant_iso()],
        )
        .ok();
    }

    // -----------------------------------------------------------------
    //  Roles et permissions
    // -----------------------------------------------------------------
    // Les droits etaient codes en dur sur le NOM du role. Ajouter un
    // caissier ou un magasinier demandait de recompiler, et donner une
    // permission de plus a UNE personne etait impossible.
    //
    // `role.permissions` existait deja en base, sans jamais etre lue.
    // On lui ajoute `acces_total` — le role passe avant toute liste —
    // et une table pour les reglages personnels.
    conn.execute(
        "ALTER TABLE role ADD COLUMN acces_total INTEGER NOT NULL DEFAULT 0",
        [],
    )
    .ok();
    conn.execute(
        "ALTER TABLE role ADD COLUMN protege INTEGER NOT NULL DEFAULT 0",
        [],
    )
    .ok();
    conn.execute(
        "ALTER TABLE role ADD COLUMN description TEXT",
        [],
    )
    .ok();

    // Ce qu'on ajoute ou retire A UNE PERSONNE, par-dessus son role.
    // `accorde` a deux valeurs : 1 ajoute, 0 retire. Le retrait existe
    // parce que « ce caissier-la ne touche pas au tiroir » est une
    // demande aussi courante que son inverse, et qu'y repondre en
    // fabriquant un role par personne rendrait la liste illisible.
    conn.execute(
        "CREATE TABLE IF NOT EXISTS utilisateur_permission (
            utilisateur_id TEXT NOT NULL REFERENCES utilisateur(id),
            permission     TEXT NOT NULL,
            accorde        INTEGER NOT NULL DEFAULT 1,
            cree_le        TEXT NOT NULL,
            cree_par       TEXT,
            PRIMARY KEY (utilisateur_id, permission)
         )",
        [],
    )?;

    // Reprise, une seule fois.
    //
    // C'est le point qui decide si la mise a jour se passe bien : les
    // roles existants ont `permissions = '[]'`. Basculer sur la lecture
    // en base sans les remplir retirerait TOUS leurs droits a tout le
    // monde, le matin de la mise a jour. On y ecrit donc exactement ce
    // que le code accordait jusqu'ici.
    let roles_repris: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM config_app WHERE cle = 'roles_repris'",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);

    if roles_repris == 0 {
        let maintenant = crate::utils::maintenant_iso();

        // `patron` pouvait tout : il garde l'acces total. Sans cela,
        // chaque commande ajoutee demain lui serait invisible jusqu'a
        // ce que quelqu'un pense a cocher une case.
        conn.execute(
            "UPDATE role SET acces_total = 1, modifie_le = ?1 WHERE nom = 'patron'",
            rusqlite::params![maintenant],
        )
        .ok();

        // `employe` avait une liste figee dans le code : on l'ecrit en
        // base, a l'identique. Les permissions en `:lire` qui y
        // figuraient sont omises — aucune lecture n'est filtree, elles
        // ne donnaient donc aucun droit.
        let employe = serde_json::json!([
            "ventes:creer",
            "paiements:creer",
            "clients:creer",
            "clients:modifier",
            "articles:creer",
            "caisse:mouvementer",
            "pieces:creer",
            "retours:creer"
        ])
        .to_string();
        conn.execute(
            "UPDATE role SET permissions = ?1, modifie_le = ?2 WHERE nom = 'employe'",
            rusqlite::params![employe, maintenant],
        )
        .ok();

        // Les roles qu'on ajoute. `INSERT OR IGNORE` : un commercant
        // qui aurait deja cree un role « caissier » garde le sien.
        let nouveaux: [(&str, &str, i64, i64, &str); 4] = [
            (
                "superadmin",
                "Accès complet, non modifiable. Le compte de secours.",
                1,
                1,
                "[]",
            ),
            (
                "caissier",
                "Encaisse, rend la monnaie, enregistre les retours.",
                0,
                0,
                r#"["ventes:creer","paiements:creer","retours:creer",
                    "clients:creer","caisse:mouvementer","pieces:creer"]"#,
            ),
            (
                "magasinier",
                "Tient le stock, reçoit et livre la marchandise.",
                0,
                0,
                r#"["articles:creer","stock:transferer","depots:gerer",
                    "livraisons:enregistrer","achats:creer"]"#,
            ),
            (
                "comptable",
                "Créances, chèques, TVA, règlements fournisseurs.",
                0,
                0,
                r#"["creances:gerer","cheques:gerer","fournisseurs:regler",
                    "avoirs:gerer","chantiers:gerer"]"#,
            ),
        ];
        for (nom, description, acces, protege, permissions) in nouveaux {
            // Le JSON est reecrit compact : les retours a la ligne du
            // litteral ci-dessus survivraient dans la base et rendraient
            // la colonne penible a relire.
            let compact = serde_json::from_str::<Vec<String>>(
                &permissions.replace('\n', " "),
            )
            .map(|v| serde_json::json!(v).to_string())
            .unwrap_or_else(|_| "[]".to_string());
            conn.execute(
                "INSERT OR IGNORE INTO role
                   (id, nom, permissions, acces_total, protege, description,
                    cree_le, modifie_le, origine)
                 VALUES (lower(hex(randomblob(16))), ?1, ?2, ?3, ?4, ?5, ?6, ?6, 'migration')",
                rusqlite::params![nom, compact, acces, protege, description, maintenant],
            )
            .ok();
        }

        conn.execute(
            "INSERT OR IGNORE INTO config_app (cle, valeur) VALUES ('roles_repris', ?1)",
            rusqlite::params![maintenant],
        )
        .ok();
    }

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

    // -----------------------------------------------------------------
    //  L'etiquette du magasin d'usine
    // -----------------------------------------------------------------
    // A l'ecran, « depot » s'appelle desormais « magasin ». Les bases
    // creees avant portent encore `Depot principal`. Le nom EXACT en
    // condition : un commercant qui a renomme son magasin garde son
    // nom — on ne corrige que l'etiquette qu'on avait posee soi-meme.
    conn.execute(
        "UPDATE depot SET nom = 'Magasin principal'
         WHERE nom = 'Dépôt principal' AND est_defaut = 1",
        [],
    )
    .ok();

    // -----------------------------------------------------------------
    //  Le cloisonnement par dossier
    // -----------------------------------------------------------------
    // Le meme que pose `amorcage::cloisonnement`, sur le chemin
    // rusqlite qu'emprunte l'application. Les deux doivent rester
    // identiques : une base preparee par la fenetre et une base
    // preparee par le serveur doivent avoir la meme forme, sinon un
    // dossier ouvert par l'un serait illisible par l'autre.
    //
    // La liste des tables est partagee, justement pour qu'elles ne
    // puissent pas diverger.
    conn.execute(
        "CREATE TABLE IF NOT EXISTS dossier (
            id                TEXT PRIMARY KEY,
            code              TEXT NOT NULL UNIQUE,
            societe           TEXT NOT NULL,
            exercice_debut    TEXT NOT NULL,
            exercice_fin      TEXT NOT NULL,
            prolonge_jusqu_au TEXT,
            clos              INTEGER NOT NULL DEFAULT 0,
            cree_le           TEXT NOT NULL,
            modifie_le        TEXT NOT NULL
         )",
        [],
    )
    .ok();

    let defaut = crate::dossiers::DOSSIER_DEFAUT;
    let maintenant = crate::utils::maintenant_iso();
    let annee: String = maintenant.chars().take(4).collect();
    conn.execute(
        "INSERT OR IGNORE INTO dossier
           (id, code, societe, exercice_debut, exercice_fin, clos, cree_le, modifie_le)
         VALUES (?1, 'PRINCIPAL', 'Ma boutique', ?2, ?3, 0, ?4, ?4)",
        rusqlite::params![
            defaut,
            format!("{annee}-01-01"),
            format!("{annee}-12-31"),
            maintenant
        ],
    )
    .ok();

    for table in crate::dossiers::TABLES_CLOISONNEES {
        // L'echec est le cas normal au deuxieme demarrage : la colonne
        // est deja la.
        conn.execute(
            &format!(
                "ALTER TABLE {table} ADD COLUMN dossier_id TEXT NOT NULL DEFAULT '{defaut}'"
            ),
            [],
        )
        .ok();
        conn.execute(
            &format!("CREATE INDEX IF NOT EXISTS idx_{table}_dossier ON {table}(dossier_id)"),
            [],
        )
        .ok();
    }

    // Les compteurs deja en place portent une cle sans dossier. Sans
    // cette migration, la numerotation repartirait a 1 et refabriquerait
    // un numero deja emis — la contrainte UNIQUE bloquerait alors la
    // premiere vente du matin de la mise a jour.
    //
    // Elle vient APRES la reprise des compteurs, qui ecrit des cles
    // sans prefixe. L'ordre n'est pas indifferent.
    conn.execute(
        "UPDATE compteur_piece SET cle = ?1 || cle WHERE cle NOT LIKE '%:%'",
        rusqlite::params![format!("{}:", crate::dossiers::DOSSIER_DEFAUT)],
    )
    .ok();

    Ok(())
}
