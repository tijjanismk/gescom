# Alertes

Généré depuis [_genere/carte.json](_genere/carte.json) le 12/09/2026
(214 fichiers), puis vérifié à la main. **À lire avant de corriger quoi
que ce soit** : la moitié des pièges ci-dessous sont des copies qui
semblent vivantes.

> Le script ne voit que les imports statiques. Le pont `invoke("nom")`
> entre React et Rust est une chaîne : les listes croisent donc les deux
> sources (`grep` sur le nom de la commande dans `src/`).

---

## Deux fichiers du même nom : lequel ouvrir

`src-tauri/src/commandes/<module>.rs` et `src-tauri/noyau/src/<module>.rs`
existent en parallèle pour 29 modules. **Ce n'est pas un doublon** :

- `noyau/src/<module>.rs` — **la logique**. C'est là qu'on lit et qu'on
  modifie. Chaque commande y a deux versions : `fn(conn: &Connection)`
  (SQLite, chemin de la fenêtre) et `fn_sur_base(base: &mut Base)` (les
  deux moteurs, chemin du serveur). **Une correction se fait dans les
  deux**, ou la fenêtre et le serveur divergent.
- `src-tauri/src/commandes/<module>.rs` — une façade `#[tauri::command]`
  de 200 lignes qui délègue au noyau. Ne contient aucune règle.

Le script signale 180 « symboles dupliqués » entre les deux arbres :
c'est ce motif, voulu, et rien d'autre.

## Fichiers orphelins — vérifiés

| Fichier | Verdict |
|---|---|
| [src/components/SelecteurProfil.tsx](../src/components/SelecteurProfil.tsx) | **Mort.** Remplacé par `PageLogin.tsx`. |
| [src/components/ui/table.tsx](../src/components/ui/table.tsx) | **Mort.** Primitive jamais adoptée, 13 fichiers écrivent leur `<table>`. |
| [src/lib/session.ts](../src/lib/session.ts) | **Mort et dupliqué.** La règle des 8 h vit dans `App.tsx`. |
| [src/lib/tauri.ts](../src/lib/tauri.ts) | **Mort.** Ancienne façade typée sur `invoke`. |
| [src/lib/remise.ts](../src/lib/remise.ts) | **Mort.** Le calcul de remise est en Rust. |
| `outils/generer_socle.py` | **Vivant, hors application.** Régénère le bloc de `serveur/src/socle.rs` entre ses marqueurs. |
| `creer_tiers_test.py`, `generer_catalogue.py`, `nettoyer_imports.py`, `preparer_demo_video.py`, `t_regles.py` | **Vivants, hors application.** Outillage lancé à la main ; `t_regles.py` rejoue les règles du cœur en Python, ce n'est pas un doublon. |

`portes.rs` n'est **plus** orphelin : déclaré dans `lib.rs`, il porte le
catalogue des permissions et `permissions_de_sur` (les deux moteurs).

## Commandes enregistrées, jamais appelées par l'écran

15 sur 187. Déclarées côté Rust, servies par le serveur, aucun `invoke`
dans `src/`. Ne rien supprimer : plusieurs sont des demi-chantiers
volontairement laissés branchés.

```
pieces::annuler_facture_par_avoir     pieces::changer_statut_piece
pieces_pos::modifier_facture_pos      pieces_pos::valider_facture_credit
avoirs::appliquer_avoir_vente         creances::solder_residus_creances
comptoir::lire_clients_avec_creances  chantiers::lire_factures_fournisseur_ouvertes
caisse::lire_depenses_du_jour         caisse::modifier_depense
depots::lire_stock_depot              depots::lire_stock_article_depots
parametres::lire_stocks
pagination::lire_ventes_paginees      pagination::lire_ventes_recentes_paginee
```

Deux qui devraient l'être :
- **les deux `pagination::lire_ventes_*`** — les listes de ventes se
  chargent **sans** pagination alors que la commande existe ;
- **`creances::solder_residus_creances`** — l'application de D41 (seuil
  de 5 F) : le reste exigible est calculé à la lecture, mais rien ne
  solde jamais en base.

## Fichiers les plus sollicités

Modifier une signature ici casse loin.

| fichier | importé par |
|---|---|
| [noyau/src/coeur/pieces.rs](../src-tauri/noyau/src/coeur/pieces.rs) | 77 |
| [noyau/src/pieces.rs](../src-tauri/noyau/src/pieces.rs) | 74 |
| [src/lib/pont.ts](../src/lib/pont.ts) | 48 — **le** pont vers le serveur |
| [noyau/src/coeur/caisse.rs](../src-tauri/noyau/src/coeur/caisse.rs) | 39 |
| [noyau/src/fournisseurs.rs](../src-tauri/noyau/src/fournisseurs.rs), [chantiers.rs](../src-tauri/noyau/src/chantiers.rs) | 39 |

## Fichiers volumineux

| fichier | lignes | pourquoi |
|---|---|---|
| [noyau/src/pieces.rs](../src-tauri/noyau/src/pieces.rs) | 3 579 | le cycle documentaire, en deux versions |
| [serveur/src/socle.rs](../src-tauri/serveur/src/socle.rs) | 2 661 | 187 commandes × 2 poignées ; le bloc généré est entre marqueurs |
| [noyau/src/argent.rs](../src-tauri/noyau/src/argent.rs) | 2 420 | vente, facture, règlement — **ouvrir avant de toucher à l'argent** |
| [noyau/src/achats.rs](../src-tauri/noyau/src/achats.rs) | 2 030 | |
| [src/pages/Pieces.tsx](../src/pages/Pieces.tsx), [Ventes.tsx](../src/pages/Ventes.tsx) | 1 700 | |

Les modules du noyau ont doublé de taille avec le portage : la moitié
« `Connection` » deviendra du code mort le jour où la fenêtre passera
elle aussi par le serveur (D11). C'est le prochain grand nettoyage, pas
un bug.

## Régénérer

```bash
python <chemin>/carte.py . --json AI_CONTEXT/_genere/carte.json
```

Sous Git Bash, ne pas passer `--alias '@/=src/'` : l'alias par défaut est
le bon, et le shell mange les guillemets. Puis relire ce fichier :
c'est lui qu'on charge, pas le JSON.
