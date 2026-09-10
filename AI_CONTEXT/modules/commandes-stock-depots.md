# Module : commandes — stock et dépôts

Rôle : où se trouve la marchandise. Dépôts, stock par dépôt, transferts
inter-dépôts, catalogue, codes-barres.

## Fichiers

- [commandes/depots.rs](../../src-tauri/src/commandes/depots.rs) (573 l.)
- [commandes/transferts.rs](../../src-tauri/src/commandes/transferts.rs) (319 l.)
- [commandes/catalogue.rs](../../src-tauri/src/commandes/catalogue.rs) (466 l.)
  — import/export CSV, état du stock.
- [commandes/codebarre.rs](../../src-tauri/src/commandes/codebarre.rs) (176 l.)
  — attribution EAN-13. ⚠️ La **lecture** au scan est dans
  `avoirs.rs::chercher_article_par_code_barre`, pas ici.

## Commandes exposées

**depots.rs** — `lire_depots_detail`, `creer_depot`, `renommer_depot`,
`definir_depot_defaut`, `desactiver_depot`, `reactiver_depot`,
`lire_resume_par_depot`, `lire_stock_multi_depots`,
`lire_mouvements_stock`, `lire_ventes_a_decouvert` ; `lire_stock_depot`
et `lire_stock_article_depots` *(jamais appelées)*.

**transferts.rs** — `enregistrer_transfert`
([transferts.rs:50](../../src-tauri/src/commandes/transferts.rs#L50)),
`lire_transferts`, `lire_bon_transfert`.

**catalogue.rs** — `exporter_articles_csv`, `importer_articles_csv`,
`lire_etat_stock`.

**codebarre.rs** — `generer_code_barre`,
`generer_codes_barres_manquants`, `definir_code_barre`,
`lire_articles_codes_barres`.

## Entrant

| Écran | Ce qu'il appelle |
|---|---|
| [Stock.tsx](../../src/pages/Stock.tsx) | `lire_etat_stock`, `lire_stocks_pagines`, `lire_mouvements_stock` |
| [Transferts.tsx](../../src/pages/Transferts.tsx) | les 3 de transferts, `lire_depots` |
| [OngletDepots.tsx](../../src/components/OngletDepots.tsx) | tout le CRUD dépôt |
| [OngletCodesBarres.tsx](../../src/components/OngletCodesBarres.tsx) | les 4 de codebarre |
| [OngletImportExport.tsx](../../src/components/OngletImportExport.tsx) | CSV |
| [Ventes.tsx](../../src/pages/Ventes.tsx) | `lire_stock_multi_depots` |
| [Dashboard.tsx](../../src/pages/Dashboard.tsx) | `lire_ventes_a_decouvert` |

Sortant : `coeur::stock` (types de mouvement, `est_a_decouvert`),
`coeur::codebarre`, `commandes::ventes` (`EtatApp`),
`persistance::journal`.

## Règles métier

- [CONFIRMÉ] Les 8 types de mouvement ne se déclarent qu'en un endroit,
  [coeur/stock.rs:17-39](../../src-tauri/noyau/src/coeur/stock.rs#L17). La
  colonne `mouvement_stock.type_mouvement` est du **TEXT libre** : rien
  en base n'empêche d'y écrire une valeur inventée, et c'est exactement
  ce qui a désynchronisé le journal (échange, ajustement et transfert
  n'apparaissaient nulle part).
- [CONFIRMÉ] `transfert` écrit **deux** lignes opposées, une par dépôt
  ([coeur/stock.rs:32](../../src-tauri/noyau/src/coeur/stock.rs#L32)) ; son
  sens n'est pas dans le type mais dans le signe de `quantite_delta`.
- [CONFIRMÉ] Un bon `BTR-AAAA-NNNNN` regroupe les lignes d'un même
  transfert via `transfert.bon` (colonne de migration, v1.2).
- [CONFIRMÉ] Un transfert refuse de mettre le dépôt source à découvert
  (D32) : le refus est levé dans `enregistrer_transfert_sur`, avec le
  message « Stock insuffisant pour « … » : … disponible(s) » —
  [transferts.rs:97](../../src-tauri/src/commandes/transferts.rs#L97)
  pour le commentaire,
  [transferts.rs:126](../../src-tauri/src/commandes/transferts.rs#L126)
  pour le message.
- [DÉDUIT] Un échange sort le remplacement du dépôt **de la vente**,
  jamais du dépôt par défaut (D43). Logé dans `retours.rs`, pas ici.
- [CONFIRMÉ] `unite_vente.code_barre` existe **en plus** de
  `article.code_barre` : en boutique le carton porte son propre EAN,
  différent de celui de la pièce. Une seule colonne rendait le scan d'un
  carton impossible par construction (D45, migration dans
  [persistance/mod.rs](../../src-tauri/noyau/src/persistance/mod.rs)).
- [CONFIRMÉ] Codes internes : préfixe `20`, jamais attribué à un pays,
  donc sans collision possible
  ([coeur/codebarre.rs:18](../../src-tauri/noyau/src/coeur/codebarre.rs#L18)).
