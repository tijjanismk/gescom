-- =====================================================================
--  Schéma complet de la base gescom
--  Toutes les tables créées si elles n'existent pas (idempotent).
--  Conventions :
--    - id : UUID (TEXT)
--    - montants : INTEGER (FCFA, jamais REAL)
--    - quantités : REAL (kg, mètre)
--    - booléens : INTEGER (0/1)
--    - dates : TEXT (ISO 8601 UTC)
-- =====================================================================

-- CATEGORIE
CREATE TABLE IF NOT EXISTS categorie (
    id               TEXT PRIMARY KEY,
    nom              TEXT NOT NULL,
    schema_attributs TEXT NOT NULL DEFAULT '[]',
    actif            INTEGER NOT NULL DEFAULT 1,
    cree_le          TEXT NOT NULL,
    modifie_le       TEXT NOT NULL,
    origine          TEXT NOT NULL
);

-- ARTICLE
CREATE TABLE IF NOT EXISTS article (
    id                    TEXT PRIMARY KEY,
    nom                   TEXT NOT NULL,
    reference             TEXT,
    categorie_id          TEXT NOT NULL REFERENCES categorie(id),
    prix_achat            INTEGER,
    dernier_prix_achat    INTEGER,
    unite_base            TEXT NOT NULL,
    gere_en_stock         INTEGER NOT NULL DEFAULT 1,
    fournisseur_voisin_id TEXT,
    attributs             TEXT NOT NULL DEFAULT '{}',
    actif                 INTEGER NOT NULL DEFAULT 1,
    cree_le               TEXT NOT NULL,
    modifie_le            TEXT NOT NULL,
    cree_par              TEXT NOT NULL,
    modifie_par           TEXT NOT NULL,
    origine               TEXT NOT NULL
);

-- UNITE_VENTE
CREATE TABLE IF NOT EXISTS unite_vente (
    id             TEXT PRIMARY KEY,
    article_id     TEXT NOT NULL REFERENCES article(id),
    libelle        TEXT NOT NULL,
    facteur        REAL NOT NULL,
    prix_reference INTEGER NOT NULL,
    actif          INTEGER NOT NULL DEFAULT 1,
    cree_le        TEXT NOT NULL,
    modifie_le     TEXT NOT NULL,
    cree_par       TEXT NOT NULL,
    modifie_par    TEXT NOT NULL,
    origine        TEXT NOT NULL
);

-- DEPOT
CREATE TABLE IF NOT EXISTS depot (
    id         TEXT PRIMARY KEY,
    nom        TEXT NOT NULL,
    est_defaut INTEGER NOT NULL DEFAULT 0,
    actif      INTEGER NOT NULL DEFAULT 1,
    cree_le    TEXT NOT NULL,
    modifie_le TEXT NOT NULL,
    origine    TEXT NOT NULL
);

-- STOCK_DEPOT
CREATE TABLE IF NOT EXISTS stock_depot (
    id         TEXT PRIMARY KEY,
    article_id TEXT NOT NULL REFERENCES article(id),
    depot_id   TEXT NOT NULL REFERENCES depot(id),
    quantite   REAL NOT NULL DEFAULT 0,
    UNIQUE(article_id, depot_id)
);

-- UTILISATEUR
CREATE TABLE IF NOT EXISTS utilisateur (
    id         TEXT PRIMARY KEY,
    nom        TEXT NOT NULL,
    role_id    TEXT NOT NULL,
    actif      INTEGER NOT NULL DEFAULT 1,
    cree_le    TEXT NOT NULL,
    modifie_le TEXT NOT NULL,
    origine    TEXT NOT NULL
);

-- ROLE
CREATE TABLE IF NOT EXISTS role (
    id          TEXT PRIMARY KEY,
    nom         TEXT NOT NULL,
    permissions TEXT NOT NULL DEFAULT '[]',
    cree_le     TEXT NOT NULL,
    modifie_le  TEXT NOT NULL,
    origine     TEXT NOT NULL
);

-- CLIENT
CREATE TABLE IF NOT EXISTS client (
    id           TEXT PRIMARY KEY,
    code         TEXT NOT NULL UNIQUE,
    nom          TEXT NOT NULL,
    telephone    TEXT,
    nif          TEXT,
    adresse      TEXT,
    plafond_credit INTEGER,
    est_generique INTEGER NOT NULL DEFAULT 0,
    actif        INTEGER NOT NULL DEFAULT 1,
    cree_le      TEXT NOT NULL,
    modifie_le   TEXT NOT NULL,
    cree_par     TEXT NOT NULL,
    modifie_par  TEXT NOT NULL,
    origine      TEXT NOT NULL
);

-- FOURNISSEUR
CREATE TABLE IF NOT EXISTS fournisseur (
    id         TEXT PRIMARY KEY,
    nom        TEXT NOT NULL,
    telephone  TEXT,
    nif        TEXT,
    adresse    TEXT,
    est_voisin INTEGER NOT NULL DEFAULT 0,
    actif      INTEGER NOT NULL DEFAULT 1,
    cree_le    TEXT NOT NULL,
    modifie_le TEXT NOT NULL,
    origine    TEXT NOT NULL
);

-- VENTE
CREATE TABLE IF NOT EXISTS vente (
    id             TEXT PRIMARY KEY,
    client_id      TEXT NOT NULL REFERENCES client(id),
    depot_id       TEXT NOT NULL REFERENCES depot(id),
    mode_reglement TEXT NOT NULL,  -- 'comptant' / 'credit'
    auteur_id      TEXT NOT NULL REFERENCES utilisateur(id),
    statut         TEXT NOT NULL,  -- 'payee' / 'partiellement_payee' / 'creance_ouverte'
    date_vente     TEXT NOT NULL,
    cree_le        TEXT NOT NULL,
    modifie_le     TEXT NOT NULL,
    cree_par       TEXT NOT NULL,
    modifie_par    TEXT NOT NULL,
    origine        TEXT NOT NULL
);

-- LIGNE_VENTE
CREATE TABLE IF NOT EXISTS ligne_vente (
    id                      TEXT PRIMARY KEY,
    vente_id                TEXT NOT NULL REFERENCES vente(id),
    article_id              TEXT NOT NULL REFERENCES article(id),
    unite_vente_id          TEXT NOT NULL REFERENCES unite_vente(id),
    depot_source_id         TEXT NOT NULL REFERENCES depot(id),
    source_approvisionnement TEXT NOT NULL DEFAULT 'stock',  -- 'stock' / 'voisin'
    vente_a_decouvert       INTEGER NOT NULL DEFAULT 0,
    quantite                REAL NOT NULL,
    prix_reference          INTEGER NOT NULL,
    prix_pratique           INTEGER NOT NULL,
    cree_le                 TEXT NOT NULL,
    origine                 TEXT NOT NULL
);

-- FACTURE
CREATE TABLE IF NOT EXISTS facture (
    id               TEXT PRIMARY KEY,
    numero           TEXT NOT NULL UNIQUE,  -- '2026-000123'
    vente_id         TEXT NOT NULL REFERENCES vente(id),
    statut           TEXT NOT NULL DEFAULT 'brouillon',  -- 'brouillon' / 'validee'
    total            INTEGER NOT NULL DEFAULT 0,
    date_validation  TEXT,
    cree_le          TEXT NOT NULL,
    modifie_le       TEXT NOT NULL,
    cree_par         TEXT NOT NULL,
    modifie_par      TEXT NOT NULL,
    origine          TEXT NOT NULL
);

-- PAIEMENT
CREATE TABLE IF NOT EXISTS paiement (
    id             TEXT PRIMARY KEY,
    vente_id       TEXT NOT NULL REFERENCES vente(id),
    montant        INTEGER NOT NULL,
    mode           TEXT NOT NULL,  -- 'especes'/'orange_money'/'moov_money'/'cheque'/'avoir'
    avoir_id       TEXT REFERENCES avoir(id),  -- rempli si mode = 'avoir'
    auteur_id      TEXT NOT NULL REFERENCES utilisateur(id),
    date_paiement  TEXT NOT NULL,
    cree_le        TEXT NOT NULL,
    cree_par       TEXT NOT NULL,
    origine        TEXT NOT NULL
);

-- RETOUR
CREATE TABLE IF NOT EXISTS retour (
    id                      TEXT PRIMARY KEY,
    vente_id                TEXT NOT NULL REFERENCES vente(id),
    article_id              TEXT NOT NULL REFERENCES article(id),
    unite_vente_id          TEXT NOT NULL REFERENCES unite_vente(id),
    quantite                REAL NOT NULL,
    depot_reintegration_id  TEXT NOT NULL REFERENCES depot(id),
    mode_resolution         TEXT NOT NULL,  -- 'remboursement'/'echange'/'avoir_conserve'
    vente_remplacement_id   TEXT REFERENCES vente(id),
    montant_credit          INTEGER NOT NULL,
    reliquat                INTEGER NOT NULL DEFAULT 0,
    reliquat_resolution     TEXT NOT NULL DEFAULT 'aucun',
    auteur_id               TEXT NOT NULL REFERENCES utilisateur(id),
    date_retour             TEXT NOT NULL,
    cree_le                 TEXT NOT NULL,
    cree_par                TEXT NOT NULL,
    origine                 TEXT NOT NULL
);

-- AVOIR
CREATE TABLE IF NOT EXISTS avoir (
    id         TEXT PRIMARY KEY,
    client_id  TEXT NOT NULL REFERENCES client(id),
    retour_id  TEXT NOT NULL REFERENCES retour(id),
    montant    INTEGER NOT NULL,
    statut     TEXT NOT NULL DEFAULT 'ouvert',  -- 'ouvert' / 'consomme'
    cree_le    TEXT NOT NULL,
    origine    TEXT NOT NULL
);

-- MOUVEMENT_STOCK
CREATE TABLE IF NOT EXISTS mouvement_stock (
    id             TEXT PRIMARY KEY,
    article_id     TEXT NOT NULL REFERENCES article(id),
    depot_id       TEXT NOT NULL REFERENCES depot(id),
    type_mouvement TEXT NOT NULL,  -- 'vente'/'achat'/'transfert'/'ajustement'/'retour'/'regularisation'
    quantite_delta REAL NOT NULL,  -- signé : + entrée / - sortie, en unité de base
    motif          TEXT,           -- obligatoire pour ajustement et regularisation
    operation_id   TEXT,           -- lien vers la vente, le transfert...
    auteur_id      TEXT NOT NULL REFERENCES utilisateur(id),
    date_mouvement TEXT NOT NULL,
    cree_le        TEXT NOT NULL,
    cree_par       TEXT NOT NULL,
    origine        TEXT NOT NULL
);

-- TRANSFERT
CREATE TABLE IF NOT EXISTS transfert (
    id              TEXT PRIMARY KEY,
    depot_source_id TEXT NOT NULL REFERENCES depot(id),
    depot_dest_id   TEXT NOT NULL REFERENCES depot(id),
    article_id      TEXT NOT NULL REFERENCES article(id),
    quantite        REAL NOT NULL,
    auteur_id       TEXT NOT NULL REFERENCES utilisateur(id),
    date_transfert  TEXT NOT NULL,
    cree_le         TEXT NOT NULL,
    cree_par        TEXT NOT NULL,
    origine         TEXT NOT NULL
);

-- SESSION_CAISSE
CREATE TABLE IF NOT EXISTS session_caisse (
    id              TEXT PRIMARY KEY,
    fond_initial    INTEGER NOT NULL DEFAULT 0,
    date_ouverture  TEXT NOT NULL,
    date_fermeture  TEXT,
    montant_compte  INTEGER,
    ecart           INTEGER,
    statut          TEXT NOT NULL DEFAULT 'ouverte',  -- 'ouverte' / 'fermee'
    cree_le         TEXT NOT NULL,
    modifie_le      TEXT NOT NULL,
    cree_par        TEXT NOT NULL,
    modifie_par     TEXT NOT NULL,
    origine         TEXT NOT NULL
);

-- MOUVEMENT_CAISSE
CREATE TABLE IF NOT EXISTS mouvement_caisse (
    id             TEXT PRIMARY KEY,
    session_id     TEXT NOT NULL REFERENCES session_caisse(id),
    sens           TEXT NOT NULL,   -- 'entree' / 'sortie'
    moyen          TEXT NOT NULL,   -- 'especes'/'orange_money'/'moov_money'/'cheque'
    montant        INTEGER NOT NULL,
    motif          TEXT NOT NULL,   -- 'vente'/'remboursement'/'depense'/'divers'
    operation_id   TEXT,
    date_mouvement TEXT NOT NULL,
    cree_le        TEXT NOT NULL,
    cree_par       TEXT NOT NULL,
    origine        TEXT NOT NULL
);

-- JOURNAL
CREATE TABLE IF NOT EXISTS journal (
    id             TEXT PRIMARY KEY,
    type_evenement TEXT NOT NULL,
    entite_type    TEXT NOT NULL,
    entite_id      TEXT NOT NULL,
    auteur_id      TEXT NOT NULL,
    ancien_valeur  TEXT,   -- JSON
    nouveau_valeur TEXT,   -- JSON
    origine        TEXT NOT NULL,
    date_evenement TEXT NOT NULL
);

-- =====================================================================
--  INDEX
--  Créés une seule fois, accélèrent les requêtes fréquentes.
--  Toujours sur les colonnes utilisées dans WHERE et ORDER BY.
-- =====================================================================

-- Articles
CREATE INDEX IF NOT EXISTS idx_article_categorie
    ON article(categorie_id);

-- Unités de vente
CREATE INDEX IF NOT EXISTS idx_unite_vente_article
    ON unite_vente(article_id);

-- Stock
CREATE INDEX IF NOT EXISTS idx_stock_depot_article
    ON stock_depot(article_id);
CREATE INDEX IF NOT EXISTS idx_stock_depot_depot
    ON stock_depot(depot_id);

-- Ventes
CREATE INDEX IF NOT EXISTS idx_vente_client
    ON vente(client_id);
CREATE INDEX IF NOT EXISTS idx_vente_depot
    ON vente(depot_id);
CREATE INDEX IF NOT EXISTS idx_vente_statut
    ON vente(statut);  -- 'creance_ouverte' souvent filtré
CREATE INDEX IF NOT EXISTS idx_vente_date
    ON vente(date_vente);

-- Lignes de vente
CREATE INDEX IF NOT EXISTS idx_ligne_vente_vente
    ON ligne_vente(vente_id);
CREATE INDEX IF NOT EXISTS idx_ligne_vente_article
    ON ligne_vente(article_id);

-- Factures
CREATE INDEX IF NOT EXISTS idx_facture_vente
    ON facture(vente_id);
CREATE INDEX IF NOT EXISTS idx_facture_statut
    ON facture(statut);

-- Paiements
CREATE INDEX IF NOT EXISTS idx_paiement_vente
    ON paiement(vente_id);

-- Retours
CREATE INDEX IF NOT EXISTS idx_retour_vente
    ON retour(vente_id);

-- Avoirs
CREATE INDEX IF NOT EXISTS idx_avoir_client
    ON avoir(client_id);
CREATE INDEX IF NOT EXISTS idx_avoir_statut
    ON avoir(statut);  -- 'ouvert' souvent filtré

-- Mouvements stock
CREATE INDEX IF NOT EXISTS idx_mouvement_stock_article
    ON mouvement_stock(article_id);
CREATE INDEX IF NOT EXISTS idx_mouvement_stock_depot
    ON mouvement_stock(depot_id);

-- Mouvements caisse
CREATE INDEX IF NOT EXISTS idx_mouvement_caisse_session
    ON mouvement_caisse(session_id);
CREATE INDEX IF NOT EXISTS idx_mouvement_caisse_moyen
    ON mouvement_caisse(moyen);  -- filtré pour le rapprochement espèces

-- Journal
CREATE INDEX IF NOT EXISTS idx_journal_entite
    ON journal(entite_id);
CREATE INDEX IF NOT EXISTS idx_journal_date
    ON journal(date_evenement);
CREATE INDEX IF NOT EXISTS idx_journal_auteur
    ON journal(auteur_id);