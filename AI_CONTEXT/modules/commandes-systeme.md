# Module : commandes — système et lecture

Rôle : tout ce qui n'écrit pas de métier. Authentification, réglages,
identité de société, images, sauvegarde, journal, tableaux de bord,
rapports, pagination.

## Fichiers

| Fichier | l. | Rôle |
|---|---|---|
| [auth.rs](../../src-tauri/src/commandes/auth.rs) | 207 | connexion bcrypt, utilisateurs |
| [parametres.rs](../../src-tauri/src/commandes/parametres.rs) | 132 | catégories, articles complets, unités de vente, réglages d'écran, diagnostic — **sans l'entretien** : il est au serveur (D9) |
| [societe.rs](../../src-tauri/src/commandes/societe.rs) | 80 | identité de l'entreprise |
| [logo.rs](../../src-tauri/src/commandes/logo.rs) | 132 | logo, bandeau d'en-tête, pied de page — mince : le travail est dans `noyau/src/images.rs` |
| [sauvegarde.rs](../../src-tauri/src/commandes/sauvegarde.rs) | 242 | sauvegarde manuelle et auto |
| [journal.rs](../../src-tauri/src/commandes/journal.rs) | 415 | journal du jour (lecture) |
| [dashboard.rs](../../src-tauri/src/commandes/dashboard.rs) | 428 | résumé, ventes période, tops |
| [rapports.rs](../../src-tauri/src/commandes/rapports.rs) | 281 | TVA, CA mensuel, tops, stock, créances |
| [pagination.rs](../../src-tauri/src/commandes/pagination.rs) | 663 | listes paginées |

## Commandes exposées

**auth** — `connexion`, `changer_mot_de_passe`, `creer_utilisateur`,
`lire_utilisateurs`.

**parametres** (15) — `lire_categories`, `creer_categorie`,
`lire_articles_complets`, `creer_article_complet`, `ajouter_unite_vente`,
`modifier_unite_vente`, `desactiver_unite_vente`, `diagnostiquer_base`,
`lire_config_bon_sortie`,
`sauvegarder_config_bon_sortie`, `lire_config_suivi_livraison`,
`sauvegarder_config_suivi_livraison`, `lire_config_signatures`,
`sauvegarder_config_signatures` ; `lire_stocks` *(jamais appelée)*.
`entretenir_base` n'est plus une commande (D9) : c'est la route
`POST /entretien` du serveur, à côté de la sauvegarde.

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

**Chiffre d'affaires net des retours (lot B.2, 27/10/2026)** — le
résumé du tableau de bord (jour, semaine, mois, mois précédent), la
courbe `lire_ventes_periode`, les tops du tableau de bord et des
rapports, `lire_rapport_ca_mensuel` et le `ca_total` de la fiche client
déduisent les retours **datés dans la période** (`coeur::calcul::ca_net`,
échange exclu par `RETOUR_HORS_CA`), filtrés comme la vente rendue
(magasin, vente non annulée, dossier). Le retour d'une vente du mois
dernier se déduit de ce mois-ci. Les tops se classent sur le net, la
limite après ; le top articles retire aussi la quantité rendue et son
coût d'achat de la marge. `lire_rapport_ca_mensuel` se calcule vente par
vente : chaque mois porte `ca` (net), `nb_ventes`, `encaisse` (sans la
dette éteinte par retour), `retours` et `creances` (reste dû des ventes
du mois via `reste_exigible`) — l'écran lit `creances`, plus
`ca - encaisse`. **Pas** le rapport TVA ni le cahier du jour
(`journal.rs`, qui liste les retours à part).

**pagination** — `lire_clients_pagines`, `lire_stocks_pagines`,
`lire_fournisseurs_pagines` ; `lire_ventes_recentes_paginee` (depuis le
lot D.12, 27/10/2026 : l'écran Retours et la fenêtre de retour de
Pièces, 50 par page ; rend `client_generique` et `deja_retourne`) et
`lire_ventes_paginees` *(toujours sans écran)*. Les deux : `v.dossier_id`
filtré, recherche sur nom, code du client et numéro de la facture
(`RECHERCHE_VENTE`), ordre `date_vente DESC, cree_le DESC, id`
(`ORDRE_VENTES`), numéro lu dans `piece_commerciale` dans les deux
versions. Scénario `donnees_base::les_ventes_se_lisent_par_pages_de_50`.

## Entrant

`Parametres.tsx` (1 089 l.) concentre auth, catégories, articles,
diagnostic et sauvegarde — l'entretien, lui, a quitté l'écran : le
bouton « Réparer et compacter » est remplacé par un renvoi vers la
console du serveur (D9). `ParametresSociete.tsx` porte l'identité et les
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
- [CONFIRMÉ] `diagnostiquer_base` est ici, mais le travail est fait par
  [persistance/mod.rs](../../src-tauri/noyau/src/persistance/mod.rs)
  (`verifier_integrite`, `anomalies_metier`).
- [CONFIRMÉ] L'entretien n'appartient plus à l'écran (D9) :
  `entretenir_base_sur_base` dans
  [noyau/src/parametres.rs](../../src-tauri/noyau/src/parametres.rs) fait
  le travail (intégrité, réaffectation des règlements globaux, copie
  avant, compactage) ; la route `POST /entretien` du serveur l'expose à
  la console, sous la même permission que la sauvegarde. La caisse n'en
  garde que le diagnostic.
- [CONFIRMÉ] Images en **base64 dans le HTML** d'impression (D4) : le
  document imprimé doit être autonome, il n'a pas accès au disque.
- [CONFIRMÉ] Les images voyagent **en contenu, jamais en chemin** (D8) :
  la caisse lit le fichier et envoie du base64, le serveur range les
  octets dans **son** dossier d'images et enregistre le chemin. Toute la
  validation (format, taille, base64) vit dans `noyau/src/images.rs`,
  partagée par la façade et le serveur. `supprimer_*` efface aussi le
  fichier, sinon le repli de lecture ferait revenir l'image.
- [CONFIRMÉ] Impression via une fenêtre **Tauri**, jamais le navigateur
  (D3) — le navigateur ajoute ses propres en-têtes et pieds de page.
- [DÉDUIT] Les réglages d'écran sont volontairement regroupés dans
  `parametres.rs` plutôt que près de la fonctionnalité qu'ils règlent
  (CONTEXT.md). Chercher `lire_config_suivi_livraison` dans
  `livraisons.rs` ne la trouve pas.
- [DÉDUIT] bcrypt et non argon2, à cause de `rand_core` sur Windows (D2).
- [DÉDUIT] Session localStorage 8 h, à remplacer par JWT en v2 (D1) — la
  logique vit dans [App.tsx](../../src/App.tsx), pas côté Rust.
