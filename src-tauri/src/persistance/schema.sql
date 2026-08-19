-- Gescom — Schéma SQLite complet
-- Toutes les tables avec IF NOT EXISTS pour idempotence

-- =====================================================================
-- RÔLES ET UTILISATEURS
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

-- =====================================================================
-- DÉPÔTS
-- =====================================================================

CREATE TABLE IF NOT EXISTS depot (
    id          TEXT PRIMARY KEY,
    nom         TEXT NOT NULL,
    est_defaut  INTEGER NOT NULL DEFAULT 0,
    actif       INTEGER NOT NULL DEFAULT 1,
    cree_le     TEXT NOT NULL,
    modifie_le  TEXT NOT NULL,
    origine     TEXT NOT NULL DEFAULT 'app'
);

-- =====================================================================
-- CLIENTS
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

-- =====================================================================
-- FOURNISSEURS
-- =====================================================================

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
-- ARTICLES ET UNITÉS
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
    attributs           TEXT NOT NULL DEFAULT '{}',
    actif               INTEGER NOT NULL DEFAULT 1,
    cree_le             TEXT NOT NULL,
    modifie_le          TEXT NOT NULL,
    cree_par            TEXT NOT NULL DEFAULT 'system',
    modifie_par         TEXT NOT NULL DEFAULT 'system',
    origine             TEXT NOT NULL DEFAULT 'app'
);

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

-- =====================================================================
-- STOCK
-- =====================================================================

CREATE TABLE IF NOT EXISTS stock_depot (
    id          TEXT PRIMARY KEY,
    article_id  TEXT NOT NULL REFERENCES article(id),
    depot_id    TEXT NOT NULL REFERENCES depot(id),
    quantite    REAL NOT NULL DEFAULT 0,
    UNIQUE(article_id, depot_id)
);

CREATE TABLE IF NOT EXISTS mouvement_stock (
    id              TEXT PRIMARY KEY,
    article_id      TEXT NOT NULL REFERENCES article(id),
    depot_id        TEXT NOT NULL REFERENCES depot(id),
    type_mouvement  TEXT NOT NULL, -- vente / achat / retour / ajustement / transfert / echange
    quantite_delta  REAL NOT NULL,
    motif           TEXT,
    operation_id    TEXT,
    auteur_id       TEXT,
    date_mouvement  TEXT NOT NULL,
    cree_le         TEXT NOT NULL,
    cree_par        TEXT,
    origine         TEXT NOT NULL DEFAULT 'app'
);

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
-- VENTES ET FACTURES
-- =====================================================================

CREATE TABLE IF NOT EXISTS vente (
    id              TEXT PRIMARY KEY,
    client_id       TEXT NOT NULL REFERENCES client(id),
    depot_id        TEXT NOT NULL REFERENCES depot(id),
    mode_reglement  TEXT NOT NULL DEFAULT 'credit', -- comptant / credit
    auteur_id       TEXT,
    statut          TEXT NOT NULL DEFAULT 'creance_ouverte',
    -- creance_ouverte / partiellement_payee / payee / annulee
    date_vente      TEXT NOT NULL,
    cree_le         TEXT NOT NULL,
    modifie_le      TEXT NOT NULL,
    cree_par        TEXT NOT NULL DEFAULT 'system',
    modifie_par     TEXT NOT NULL DEFAULT 'system',
    origine         TEXT NOT NULL DEFAULT 'app'
);

CREATE TABLE IF NOT EXISTS ligne_vente (
    id                          TEXT PRIMARY KEY,
    vente_id                    TEXT NOT NULL REFERENCES vente(id),
    article_id                  TEXT NOT NULL REFERENCES article(id),
    unite_vente_id              TEXT NOT NULL REFERENCES unite_vente(id),
    depot_source_id             TEXT NOT NULL REFERENCES depot(id),
    source_approvisionnement    TEXT NOT NULL DEFAULT 'stock',
    -- stock / fournisseur_secondaire
    vente_a_decouvert           INTEGER NOT NULL DEFAULT 0,
    quantite                    REAL NOT NULL,
    prix_reference              INTEGER NOT NULL,
    prix_pratique               INTEGER NOT NULL,
    taux_tva                    REAL NOT NULL DEFAULT 0.0,
    montant_tva                 INTEGER NOT NULL DEFAULT 0,
    cree_le                     TEXT NOT NULL,
    origine                     TEXT NOT NULL DEFAULT 'app'
);

CREATE TABLE IF NOT EXISTS facture (
    id              TEXT PRIMARY KEY,
    numero          TEXT NOT NULL UNIQUE,
    vente_id        TEXT NOT NULL REFERENCES vente(id),
    statut          TEXT NOT NULL DEFAULT 'validee',
    -- brouillon / validee / annulee
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
    -- especes / orange_money / moov_money / cheque / avoir
    date_paiement   TEXT NOT NULL,
    auteur_id       TEXT,
    cree_le         TEXT NOT NULL,
    cree_par        TEXT NOT NULL DEFAULT 'system',
    origine         TEXT NOT NULL DEFAULT 'app'
);

-- =====================================================================
-- RETOURS ET AVOIRS
-- =====================================================================

CREATE TABLE IF NOT EXISTS retour (
    id                      TEXT PRIMARY KEY,
    vente_id                TEXT NOT NULL REFERENCES vente(id),
    article_id              TEXT NOT NULL REFERENCES article(id),
    unite_vente_id          TEXT NOT NULL REFERENCES unite_vente(id),
    quantite                REAL NOT NULL,
    depot_reintegration_id  TEXT NOT NULL REFERENCES depot(id),
    mode_resolution         TEXT NOT NULL,
    -- remboursement / echange / avoir_conserve
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
    id          TEXT PRIMARY KEY,
    client_id   TEXT NOT NULL REFERENCES client(id),
    retour_id   TEXT REFERENCES retour(id),
    montant     INTEGER NOT NULL,
    statut      TEXT NOT NULL DEFAULT 'ouvert', -- ouvert / utilise / expire
    cree_le     TEXT NOT NULL,
    origine     TEXT NOT NULL DEFAULT 'app'
);

-- =====================================================================
-- CAISSE
-- =====================================================================

CREATE TABLE IF NOT EXISTS session_caisse (
    id                  TEXT PRIMARY KEY,
    statut              TEXT NOT NULL DEFAULT 'ouverte', -- ouverte / fermee
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

CREATE TABLE IF NOT EXISTS mouvement_caisse (
    id              TEXT PRIMARY KEY,
    session_id      TEXT NOT NULL REFERENCES session_caisse(id),
    sens            TEXT NOT NULL, -- entree / sortie
    moyen           TEXT NOT NULL DEFAULT 'especes',
    montant         INTEGER NOT NULL,
    motif           TEXT NOT NULL DEFAULT 'autre',
    operation_id    TEXT,
    date_mouvement  TEXT NOT NULL,
    cree_le         TEXT NOT NULL,
    cree_par        TEXT,
    origine         TEXT NOT NULL DEFAULT 'app'
);

-- =====================================================================
-- PARAMÈTRES SOCIÉTÉ
-- =====================================================================

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
    pied_facture    TEXT DEFAULT 'Merci de votre confiance',
    devise          TEXT NOT NULL DEFAULT 'FCFA',
    modifie_le      TEXT NOT NULL DEFAULT (datetime('now')),
    CHECK (id = 1)
);

INSERT OR IGNORE INTO parametres_societe (id, nom) VALUES (1, 'Ma Société');

-- =====================================================================
-- CONFIGURATION APP
-- =====================================================================

CREATE TABLE IF NOT EXISTS config_app (
    cle     TEXT PRIMARY KEY,
    valeur  TEXT NOT NULL
);

-- =====================================================================
-- JOURNAL
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
-- INDEX
-- =====================================================================

CREATE INDEX IF NOT EXISTS idx_facture_numero ON facture(numero);
CREATE INDEX IF NOT EXISTS idx_facture_vente ON facture(vente_id);
CREATE INDEX IF NOT EXISTS idx_vente_client ON vente(client_id);
CREATE INDEX IF NOT EXISTS idx_vente_date ON vente(date_vente DESC);
CREATE INDEX IF NOT EXISTS idx_vente_statut ON vente(statut);
CREATE INDEX IF NOT EXISTS idx_ligne_vente_vente ON ligne_vente(vente_id);
CREATE INDEX IF NOT EXISTS idx_paiement_vente ON paiement(vente_id);
CREATE INDEX IF NOT EXISTS idx_stock_depot_article ON stock_depot(article_id, depot_id);
CREATE INDEX IF NOT EXISTS idx_mouvement_stock_article ON mouvement_stock(article_id);
CREATE INDEX IF NOT EXISTS idx_journal_entite ON journal(entite_type, entite_id);
CREATE INDEX IF NOT EXISTS idx_mouvement_caisse_session ON mouvement_caisse(session_id);
ALTER TABLE parametres_societe ADD COLUMN logo_chemin TEXT;