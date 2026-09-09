# Module : commandes — système et lecture

Rôle : tout ce qui n'écrit pas de métier. Authentification, réglages,
identité de société, images, sauvegarde, journal, tableaux de bord,
rapports, pagination.

## Fichiers

| Fichier | l. | Rôle |
|---|---|---|
| [auth.rs](../../src-tauri/src/commandes/auth.rs) | 207 | connexion bcrypt, utilisateurs |
| [parametres.rs](../../src-tauri/src/commandes/parametres.rs) | 590 | catégories, articles complets, unités de vente, **et tous les réglages d'écran** |
| [societe.rs](../../src-tauri/src/commandes/societe.rs) | 80 | identité de l'entreprise |
| [logo.rs](../../src-tauri/src/commandes/logo.rs) | 202 | logo, bandeau d'en-tête, pied de page |
| [sauvegarde.rs](../../src-tauri/src/commandes/sauvegarde.rs) | 242 | sauvegarde manuelle et auto |
| [journal.rs](../../src-tauri/src/commandes/journal.rs) | 415 | journal du jour (lecture) |
| [dashboard.rs](../../src-tauri/src/commandes/dashboard.rs) | 428 | résumé, ventes période, tops |
| [rapports.rs](../../src-tauri/src/commandes/rapports.rs) | 281 | TVA, CA mensuel, tops, stock, créances |
| [pagination.rs](../../src-tauri/src/commandes/pagination.rs) | 663 | listes paginées |

## Commandes exposées

**auth** — `connexion`, `changer_mot_de_passe`, `creer_utilisateur`,
`lire_utilisateurs`.

**parametres** (16) — `lire_categories`, `creer_categorie`,
`lire_articles_complets`, `creer_article_complet`, `ajouter_unite_vente`,
`modifier_unite_vente`, `desactiver_unite_vente`, `diagnostiquer_base`,
`entretenir_base`, `lire_config_bon_sortie`,
`sauvegarder_config_bon_sortie`, `lire_config_suivi_livraison`,
`sauvegarder_config_suivi_livraison`, `lire_config_signatures`,
`sauvegarder_config_signatures` ; `lire_stocks` *(jamais appelée)*.

**societe** — `lire_parametres_societe`, `sauvegarder_parametres_societe`.

**logo** (9) — `sauvegarder_*` / `lire_*_base64` / `supprimer_*` pour
`logo`, `entete`, `pied`.

**sauvegarde** — `sauvegarder_base`, `sauvegarde_auto_si_necessaire`
(appelée depuis [App.tsx](../../src/App.tsx) au démarrage),
`lire_config_sauvegarde`, `sauvegarder_config_sauvegarde`.

**journal** — `lire_journal_du_jour`.

**dashboard** — `lire_resume_dashboard`, `lire_ventes_periode`,
`lire_top_clients`, `lire_top_articles`.

**rapports** — `lire_rapport_tva`, `lire_rapport_ca_mensuel`,
`lire_rapport_top_clients`, `lire_rapport_top_articles`,
`lire_rapport_stock`, `lire_rapport_creances`.

**pagination** — `lire_clients_pagines`, `lire_stocks_pagines`,
`lire_fournisseurs_pagines` ; `lire_ventes_paginees` et
`lire_ventes_recentes_paginee` *(jamais appelées)*.

## Entrant

`Parametres.tsx` (1 040 l.) concentre auth, catégories, articles,
diagnostic et sauvegarde. `ParametresSociete.tsx` porte l'identité et les
images ; `ParametresVentes.tsx` les réglages d'écran (scanner, bon de
sortie, suivi de livraison). `Dashboard.tsx`, `Rapports.tsx`,
`Journal.tsx` ne font que lire.

Les neuf commandes de `logo.rs` sont appelées depuis **9 fichiers du
front** — chaque écran imprimable recharge le logo en base64 lui-même.

## Règles métier

- [CONFIRMÉ] `dashboard.rs` et `rapports.rs` exposent des `top_clients` /
  `top_articles` **distincts** (`lire_top_clients` vs
  `lire_rapport_top_clients`). Deux implémentations, deux écrans : une
  divergence de chiffres entre le tableau de bord et les rapports est
  possible et ne serait signalée par rien.
- [CONFIRMÉ] `diagnostiquer_base` et `entretenir_base` sont ici, mais le
  travail est fait par [persistance/mod.rs](../../src-tauri/src/persistance/mod.rs)
  (`verifier_integrite`, `anomalies_metier`, `entretenir`).
- [CONFIRMÉ] Images en **base64 dans le HTML** d'impression (D4) : le
  document imprimé doit être autonome, il n'a pas accès au disque.
- [CONFIRMÉ] Impression via une fenêtre **Tauri**, jamais le navigateur
  (D3) — le navigateur ajoute ses propres en-têtes et pieds de page.
- [DÉDUIT] Les réglages d'écran sont volontairement regroupés dans
  `parametres.rs` plutôt que près de la fonctionnalité qu'ils règlent
  (CONTEXT.md). Chercher `lire_config_suivi_livraison` dans
  `livraisons.rs` ne la trouve pas.
- [DÉDUIT] bcrypt et non argon2, à cause de `rand_core` sur Windows (D2).
- [DÉDUIT] Session localStorage 8 h, à remplacer par JWT en v2 (D1) — la
  logique vit dans [App.tsx](../../src/App.tsx), pas côté Rust.
