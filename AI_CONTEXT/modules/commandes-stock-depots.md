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

## Articles sans suivi de stock (D36, 26/10/2026)

`article.gere_en_stock = 0` : l'article se vend normalement, jamais à
découvert (le drapeau `vente_a_decouvert` reste à 0 dans les deux
versions de `creer_vente` et `valider_facture`), sort de l'écran Stock
(`lire_stocks_pagines`), des alertes et de la valeur ; transfert et
inventaire le refusent (`suivi_stock::exiger_suivi[_sur]`). Ses
mouvements s'écrivent quand même. `suivi_stock::changer_suivi_stock_sur`
(`parametres:modifier`, journal `suivi_stock_change`) : cesser rend ce
qui sort du stock ; reprendre exige un comptage par magasin actif
(`coeur::stock::verifier_comptages`) et pose les ajustements dans la même
transaction. L'article est commun aux dossiers : la case vaut pour tous.

À l'import CSV (27/10/2026, lot D.11) : colonne facultative « Suivre le
stock » (oui/non), écrite EN DERNIER par l'export (`ENTETE`), lue par
son nom seulement (`suivrelestock`, `suivrestock`, `gereenstock`).
Règle `coeur::stock::lire_suivi_csv`, jugée avant toute écriture de la
ligne (catégorie comprise). Un article **créé** : absente ou vide → suivi,
« non » → `gere_en_stock = 0` et journal `suivi_stock_change`
(`cree_par_import`), et — version `Base` — seulement avec
`parametres:modifier` (le socle le juge et le passe en
`peut_ne_pas_suivre` ; sinon la ligne est refusée). Un article
**existant** : vide → inchangé ; une valeur égale passe ; une valeur qui
changerait le suivi **refuse la ligne entière** (cesser ou reprendre
passe par la case, avec sa permission, ses comptages et sa trace). Une
valeur illisible refuse la ligne avec le message. Deux versions. Scénario
`donnees_base::l_import_csv_pose_le_suivi_du_stock_sans_deviner`.

À l'achat (27/10/2026, lot C.8) : il reste achetable comme les autres ;
`Achats.tsx` et `ModalNouvellePiece.tsx` lisent `gere_en_stock` dans
`lire_articles_avec_unites` (déjà rendu par les deux versions) et le
marquent « stock non suivi » au lieu d'un stock à 0 qui ferait croire à
une rupture. Scénario : `garde_fous_base::un_article_non_suivi_reste_achetable_et_la_lecture_des_achats_le_dit`.
