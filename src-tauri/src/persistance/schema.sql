-- =====================================================================
--  Gescom — schéma complet de la base SQLite
--
--  Régénéré depuis une base réelle le 29/08/2026, après les lots de
--  correction v1. Ce fichier décrit l'état CIBLE d'une base neuve.
--
--  ⚠️ Les migrations de persistance/mod.rs restent nécessaires : elles
--     mettent à niveau les bases DÉJÀ créées. Les colonnes qu'elles
--     ajoutent sont désormais présentes ici, si bien qu'une base neuve
--     est complète dès le premier démarrage et que les ALTER ne font
--     alors rien (ils échouent silencieusement sur "duplicate column").
--
--  Invariants (voir CONTEXT.md) :
--    1. Montants toujours INTEGER — FCFA, jamais de flottant
--    2. TVA ajoutée au HT ; prix_pratique stocké TTC
--    3. Numérotation par MAX(), jamais COUNT() — numero est UNIQUE
--    4. Le journal est append-only
-- =====================================================================

PRAGMA foreign_keys = ON;


-- =====================================================================
--  RÉFÉRENTIELS — utilisateurs, rôles, configuration
-- =====================================================================

CREATE TABLE IF NOT EXISTS role (
    id          TEXT PRIMARY KEY,
    nom         TEXT NOT NULL UNIQUE,
    permissions TEXT NOT NULL DEFAULT '[]',
    cree_le     TEXT NOT NULL,
    modifie_le  TEXT NOT NULL,
    origine     TEXT NOT NULL DEFAULT 'app'
);

CREATE TABLE IF NOT EXISTS utilisateur (
    id          TEXT PRIMARY KEY,
    nom         TEXT NOT NULL,
    role_id     TEXT NOT NULL REFERENCES role(id),
    actif       INTEGER NOT NULL DEFAULT 1,
    cree_le     TEXT NOT NULL,
    modifie_le  TEXT NOT NULL,
    origine     TEXT NOT NULL DEFAULT 'app'
);

CREATE TABLE IF NOT EXISTS utilisateur_auth (
    utilisateur_id      TEXT NOT NULL PRIMARY KEY REFERENCES utilisateur(id),
    pseudo              TEXT UNIQUE,
    email               TEXT UNIQUE,
    mot_de_passe        TEXT NOT NULL,
    doit_changer_mdp    INTEGER NOT NULL DEFAULT 0,
    derniere_connexion  TEXT
);

CREATE TABLE IF NOT EXISTS config_app (
    cle     TEXT PRIMARY KEY,
    valeur  TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS parametres_societe (
    id              INTEGER PRIMARY KEY DEFAULT 1,
    nom             TEXT NOT NULL DEFAULT 'Ma Société',
    adresse         TEXT,
    telephone       TEXT,
    telephone2      TEXT,
    email           TEXT,
    nif             TEXT,
    rccm            TEXT,
    site_web        TEXT,
    logo_chemin     TEXT,
    pied_facture    TEXT DEFAULT 'Merci de votre confiance',
    devise          TEXT NOT NULL DEFAULT 'FCFA',
    modifie_le      TEXT NOT NULL DEFAULT (datetime('now')),
    CHECK (id = 1)
);


-- =====================================================================
--  CATALOGUE — articles, unités de vente, dépôts, stock
-- =====================================================================

CREATE TABLE IF NOT EXISTS categorie (
    id                  TEXT PRIMARY KEY,
    nom                 TEXT NOT NULL UNIQUE,
    schema_attributs    TEXT NOT NULL DEFAULT '[]',
    actif               INTEGER NOT NULL DEFAULT 1,
    cree_le             TEXT NOT NULL,
    modifie_le          TEXT NOT NULL,
    origine             TEXT NOT NULL DEFAULT 'app'
);

CREATE TABLE IF NOT EXISTS article (
    id                  TEXT PRIMARY KEY,
    nom                 TEXT NOT NULL,
    categorie_id        TEXT REFERENCES categorie(id),
    unite_base          TEXT NOT NULL DEFAULT 'unite',
    gere_en_stock       INTEGER NOT NULL DEFAULT 1,
    dernier_prix_achat  INTEGER,
    code_barre          TEXT UNIQUE,
    attributs           TEXT NOT NULL DEFAULT '{}',
    actif               INTEGER NOT NULL DEFAULT 1,
    cree_le             TEXT NOT NULL,
    modifie_le          TEXT NOT NULL,
    cree_par            TEXT NOT NULL DEFAULT 'system',
    modifie_par         TEXT NOT NULL DEFAULT 'system',
    origine             TEXT NOT NULL DEFAULT 'app'
, taux_tva_defaut REAL NOT NULL DEFAULT 0.0);

CREATE TABLE IF NOT EXISTS unite_vente (
    id              TEXT PRIMARY KEY,
    article_id      TEXT NOT NULL REFERENCES article(id),
    libelle         TEXT NOT NULL,
    facteur         REAL NOT NULL DEFAULT 1.0,
    prix_reference  INTEGER NOT NULL DEFAULT 0,
    actif           INTEGER NOT NULL DEFAULT 1,
    cree_le         TEXT NOT NULL,
    modifie_le      TEXT NOT NULL,
    cree_par        TEXT NOT NULL DEFAULT 'system',
    modifie_par     TEXT NOT NULL DEFAULT 'system',
    origine         TEXT NOT NULL DEFAULT 'app'
);

CREATE TABLE IF NOT EXISTS depot (
    id          TEXT PRIMARY KEY,
    nom         TEXT NOT NULL,
    est_defaut  INTEGER NOT NULL DEFAULT 0,
    actif       INTEGER NOT NULL DEFAULT 1,
    cree_le     TEXT NOT NULL,
    modifie_le  TEXT NOT NULL,
    origine     TEXT NOT NULL DEFAULT 'app'
);

CREATE TABLE IF NOT EXISTS stock_depot (
    id          TEXT PRIMARY KEY,
    article_id  TEXT NOT NULL REFERENCES article(id),
    depot_id    TEXT NOT NULL REFERENCES depot(id),
    quantite    REAL NOT NULL DEFAULT 0,
    UNIQUE(article_id, depot_id)
);


-- =====================================================================
--  TIERS — clients et fournisseurs
-- =====================================================================

CREATE TABLE IF NOT EXISTS client (
    id              TEXT PRIMARY KEY,
    code            TEXT NOT NULL UNIQUE,
    nom             TEXT NOT NULL,
    telephone       TEXT,
    adresse         TEXT,
    nif             TEXT,
    email           TEXT,
    est_generique   INTEGER NOT NULL DEFAULT 0,
    actif           INTEGER NOT NULL DEFAULT 1,
    cree_le         TEXT NOT NULL,
    modifie_le      TEXT NOT NULL,
    cree_par        TEXT NOT NULL DEFAULT 'system',
    modifie_par     TEXT NOT NULL DEFAULT 'system',
    origine         TEXT NOT NULL DEFAULT 'app'
);

CREATE TABLE IF NOT EXISTS fournisseur (
    id          TEXT PRIMARY KEY,
    nom         TEXT NOT NULL,
    telephone   TEXT,
    nif         TEXT,
    adresse     TEXT,
    email       TEXT,
    est_voisin  INTEGER NOT NULL DEFAULT 0,
    actif       INTEGER NOT NULL DEFAULT 1,
    cree_le     TEXT NOT NULL,
    modifie_le  TEXT NOT NULL,
    origine     TEXT NOT NULL DEFAULT 'app'
);


-- =====================================================================
--  VENTES — le POS. prix_pratique est TTC (D8)
-- =====================================================================

CREATE TABLE IF NOT EXISTS vente (
    id              TEXT PRIMARY KEY,
    client_id       TEXT NOT NULL REFERENCES client(id),
    depot_id        TEXT NOT NULL REFERENCES depot(id),
    mode_reglement  TEXT NOT NULL DEFAULT 'credit',
    auteur_id       TEXT,
    statut          TEXT NOT NULL DEFAULT 'creance_ouverte',
    date_vente      TEXT NOT NULL,
    cree_le         TEXT NOT NULL,
    modifie_le      TEXT NOT NULL,
    cree_par        TEXT NOT NULL DEFAULT 'system',
    modifie_par     TEXT NOT NULL DEFAULT 'system',
    origine         TEXT NOT NULL DEFAULT 'app'
, piece_id TEXT);

-- prix_pratique = prix TTC réellement payé (D8).
--    montant_tva = TVA contenue dedans, jamais ajoutée par-dessus.
--    SUM(prix_pratique * quantite) = montant dû, partout.
CREATE TABLE IF NOT EXISTS ligne_vente (
    id                          TEXT PRIMARY KEY,
    vente_id                    TEXT NOT NULL REFERENCES vente(id),
    article_id                  TEXT NOT NULL REFERENCES article(id),
    unite_vente_id              TEXT NOT NULL REFERENCES unite_vente(id),
    depot_source_id             TEXT NOT NULL REFERENCES depot(id),
    source_approvisionnement    TEXT NOT NULL DEFAULT 'stock',
    vente_a_decouvert           INTEGER NOT NULL DEFAULT 0,
    quantite                    REAL NOT NULL,
    prix_reference              INTEGER NOT NULL,
    prix_pratique               INTEGER NOT NULL,
    taux_tva                    REAL NOT NULL DEFAULT 0.0,
    montant_tva                 INTEGER NOT NULL DEFAULT 0,
    cree_le                     TEXT NOT NULL,
    origine                     TEXT NOT NULL DEFAULT 'app'
);

-- ⚠️ LEGACY — plus alimentée depuis L31. Conservée pour
--    l'historique des numéros GESCOM- déjà remis à des clients.
--    Le référentiel est piece_commerciale.
CREATE TABLE IF NOT EXISTS facture (
    id              TEXT PRIMARY KEY,
    numero          TEXT NOT NULL UNIQUE,
    vente_id        TEXT NOT NULL REFERENCES vente(id),
    statut          TEXT NOT NULL DEFAULT 'validee',
    total           INTEGER NOT NULL DEFAULT 0,
    date_validation TEXT,
    cree_le         TEXT NOT NULL,
    modifie_le      TEXT NOT NULL,
    cree_par        TEXT NOT NULL DEFAULT 'system',
    modifie_par     TEXT NOT NULL DEFAULT 'system',
    origine         TEXT NOT NULL DEFAULT 'app'
);

CREATE TABLE IF NOT EXISTS paiement (
    id              TEXT PRIMARY KEY,
    vente_id        TEXT NOT NULL REFERENCES vente(id),
    montant         INTEGER NOT NULL,
    mode            TEXT NOT NULL DEFAULT 'especes',
    date_paiement   TEXT NOT NULL,
    auteur_id       TEXT,
    cree_le         TEXT NOT NULL,
    cree_par        TEXT NOT NULL DEFAULT 'system',
    origine         TEXT NOT NULL DEFAULT 'app'
);


-- =====================================================================
--  PIÈCES COMMERCIALES — référentiel unique des documents
-- =====================================================================

-- statut : brouillon | emis | paye | transfere | annule
--    'validee' est un ancien statut, encore présent en base.
--    numero est UNIQUE : la numérotation passe par MAX(), jamais COUNT().
CREATE TABLE IF NOT EXISTS piece_commerciale (
    id               TEXT PRIMARY KEY,
    type_piece       TEXT NOT NULL,
    numero           TEXT NOT NULL UNIQUE,
    statut           TEXT NOT NULL DEFAULT 'brouillon',
    tiers_type       TEXT NOT NULL DEFAULT 'client',
    tiers_id         TEXT NOT NULL,
    depot_id         TEXT REFERENCES depot(id),
    piece_origine_id TEXT REFERENCES piece_commerciale(id),
    auteur_id        TEXT,
    date_piece       TEXT NOT NULL,
    date_echeance    TEXT,
    remise_globale   REAL NOT NULL DEFAULT 0,
    note             TEXT,
    cree_le          TEXT NOT NULL,
    modifie_le       TEXT NOT NULL,
    origine          TEXT NOT NULL DEFAULT 'app'
);

CREATE TABLE IF NOT EXISTS ligne_piece (
    id               TEXT PRIMARY KEY,
    piece_id         TEXT NOT NULL REFERENCES piece_commerciale(id),
    article_id       TEXT NOT NULL REFERENCES article(id),
    unite_vente_id   TEXT NOT NULL REFERENCES unite_vente(id),
    quantite         REAL NOT NULL,
    prix_unitaire    INTEGER NOT NULL,
    remise_pct       REAL NOT NULL DEFAULT 0,
    remise_montant   INTEGER NOT NULL DEFAULT 0,
    taux_tva         REAL NOT NULL DEFAULT 0,
    montant_tva      INTEGER NOT NULL DEFAULT 0,
    montant_ht       INTEGER NOT NULL,
    cree_le          TEXT NOT NULL
);


-- =====================================================================
--  CAISSE ET STOCK — mouvements
-- =====================================================================

CREATE TABLE IF NOT EXISTS session_caisse (
    id                  TEXT PRIMARY KEY,
    statut              TEXT NOT NULL DEFAULT 'ouverte',
    fond_ouverture      INTEGER NOT NULL DEFAULT 0,
    solde_theorique     INTEGER,
    especes_comptees    INTEGER,
    ecart               INTEGER,
    ouvert_par          TEXT,
    ferme_le            TEXT,
    cree_le             TEXT NOT NULL,
    modifie_le          TEXT NOT NULL,
    origine             TEXT NOT NULL DEFAULT 'app'
);

-- motif : vente | achat | reglement_fournisseur |
--    retour_fournisseur | depense | remboursement | ouverture
--    Le rapprochement ne porte que sur moyen='especes' (D29).
CREATE TABLE IF NOT EXISTS mouvement_caisse (
    id              TEXT PRIMARY KEY,
    session_id      TEXT NOT NULL REFERENCES session_caisse(id),
    sens            TEXT NOT NULL,
    moyen           TEXT NOT NULL DEFAULT 'especes',
    montant         INTEGER NOT NULL,
    motif           TEXT NOT NULL DEFAULT 'autre',
    operation_id    TEXT,
    -- Dépenses : motif porte la catégorie technique ('depense'),
    -- libelle le texte saisi, categorie le poste comptable.
    libelle         TEXT,
    categorie       TEXT,
    date_mouvement  TEXT NOT NULL,
    cree_le         TEXT NOT NULL,
    cree_par        TEXT,
    origine         TEXT NOT NULL DEFAULT 'app'
);

CREATE TABLE IF NOT EXISTS mouvement_stock (
    id              TEXT PRIMARY KEY,
    article_id      TEXT NOT NULL REFERENCES article(id),
    depot_id        TEXT NOT NULL REFERENCES depot(id),
    type_mouvement  TEXT NOT NULL,
    quantite_delta  REAL NOT NULL,
    motif           TEXT,
    operation_id    TEXT,
    auteur_id       TEXT,
    date_mouvement  TEXT NOT NULL,
    cree_le         TEXT NOT NULL,
    cree_par        TEXT,
    origine         TEXT NOT NULL DEFAULT 'app'
, fournisseur_id TEXT, prix_achat_unitaire INTEGER);

-- Table prévue pour les transferts inter-dépôts. Non utilisée
--    en v1 — le multi-dépôt est reporté.
CREATE TABLE IF NOT EXISTS transfert (
    id              TEXT PRIMARY KEY,
    article_id      TEXT NOT NULL REFERENCES article(id),
    depot_source    TEXT NOT NULL REFERENCES depot(id),
    depot_dest      TEXT NOT NULL REFERENCES depot(id),
    quantite        REAL NOT NULL,
    auteur_id       TEXT,
    date_transfert  TEXT NOT NULL,
    cree_le         TEXT NOT NULL,
    origine         TEXT NOT NULL DEFAULT 'app'
);


-- =====================================================================
--  RETOURS, AVOIRS, DETTES
-- =====================================================================

CREATE TABLE IF NOT EXISTS retour (
    id                      TEXT PRIMARY KEY,
    vente_id                TEXT NOT NULL REFERENCES vente(id),
    article_id              TEXT NOT NULL REFERENCES article(id),
    unite_vente_id          TEXT NOT NULL REFERENCES unite_vente(id),
    quantite                REAL NOT NULL,
    depot_reintegration_id  TEXT NOT NULL REFERENCES depot(id),
    mode_resolution         TEXT NOT NULL,
    montant_credit          INTEGER NOT NULL,
    reliquat                INTEGER NOT NULL DEFAULT 0,
    reliquat_resolution     TEXT,
    auteur_id               TEXT,
    date_retour             TEXT NOT NULL,
    cree_le                 TEXT NOT NULL,
    cree_par                TEXT NOT NULL DEFAULT 'system',
    origine                 TEXT NOT NULL DEFAULT 'app'
);

CREATE TABLE IF NOT EXISTS avoir (
    id                      TEXT PRIMARY KEY,
    client_id               TEXT NOT NULL REFERENCES client(id),
    retour_id               TEXT REFERENCES retour(id),
    montant                 INTEGER NOT NULL,
    statut                  TEXT NOT NULL DEFAULT 'ouvert',
    vente_utilisation_id    TEXT REFERENCES vente(id),
    cree_le                 TEXT NOT NULL,
    origine                 TEXT NOT NULL DEFAULT 'app'
);

CREATE TABLE IF NOT EXISTS paiement_fournisseur (
            id              TEXT PRIMARY KEY,
            fournisseur_id  TEXT NOT NULL,
            montant         INTEGER NOT NULL,
            mode            TEXT NOT NULL DEFAULT 'especes',
            note            TEXT,
            auteur_id       TEXT,
            date_paiement   TEXT NOT NULL,
            cree_le         TEXT NOT NULL,
            origine         TEXT NOT NULL DEFAULT 'app'
        , piece_id TEXT);

CREATE TABLE IF NOT EXISTS creance_irrecouvrable (
            id          TEXT PRIMARY KEY,
            vente_id    TEXT NOT NULL,
            motif       TEXT NOT NULL,
            auteur_id   TEXT,
            date_marque TEXT NOT NULL,
            cree_le     TEXT NOT NULL,
            origine     TEXT NOT NULL DEFAULT 'app'
        );

CREATE TABLE IF NOT EXISTS relance_creance (
            id           TEXT PRIMARY KEY,
            vente_id     TEXT NOT NULL,
            canal        TEXT NOT NULL DEFAULT 'whatsapp',
            note         TEXT,
            auteur_id    TEXT,
            date_relance TEXT NOT NULL,
            cree_le      TEXT NOT NULL,
            origine      TEXT NOT NULL DEFAULT 'app'
        );


-- =====================================================================
--  JOURNAL — append-only
-- =====================================================================

CREATE TABLE IF NOT EXISTS journal (
    id              TEXT PRIMARY KEY,
    type_evenement  TEXT NOT NULL,
    entite_type     TEXT NOT NULL,
    entite_id       TEXT NOT NULL,
    auteur_id       TEXT,
    ancien_valeur   TEXT,
    nouveau_valeur  TEXT,
    origine         TEXT NOT NULL DEFAULT 'app',
    date_evenement  TEXT NOT NULL
);


-- =====================================================================
--  INDEX
-- =====================================================================

CREATE INDEX IF NOT EXISTS idx_facture_numero      ON facture(numero);
CREATE INDEX IF NOT EXISTS idx_facture_vente        ON facture(vente_id);
CREATE INDEX IF NOT EXISTS idx_journal_entite       ON journal(entite_type, entite_id);
CREATE INDEX IF NOT EXISTS idx_ligne_piece ON ligne_piece(piece_id);
CREATE INDEX IF NOT EXISTS idx_ligne_vente_vente    ON ligne_vente(vente_id);
CREATE INDEX IF NOT EXISTS idx_mouvement_caisse     ON mouvement_caisse(session_id);
CREATE INDEX IF NOT EXISTS idx_mouvement_stock      ON mouvement_stock(article_id);
CREATE INDEX IF NOT EXISTS idx_paiement_vente       ON paiement(vente_id);
CREATE INDEX IF NOT EXISTS idx_piece_tiers ON piece_commerciale(tiers_id, tiers_type);
CREATE INDEX IF NOT EXISTS idx_piece_type ON piece_commerciale(type_piece);
CREATE INDEX IF NOT EXISTS idx_stock_depot_article  ON stock_depot(article_id, depot_id);
CREATE INDEX IF NOT EXISTS idx_vente_client         ON vente(client_id);
CREATE INDEX IF NOT EXISTS idx_vente_date           ON vente(date_vente DESC);
CREATE INDEX IF NOT EXISTS idx_vente_statut         ON vente(statut);
