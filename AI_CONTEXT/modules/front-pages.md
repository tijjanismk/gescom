# Module : front — pages

Rôle : les 18 écrans plein cadre. Chacun exporte **un** composant du même
nom que le fichier, et n'est importé que par
[App.tsx](../../src/App.tsx).

Pas de react-router : `App.tsx` fait un `switch` sur `page_active`.
Ajouter un écran = un fichier ici + une branche dans `App.tsx` + une
entrée dans [Layout.tsx](../../src/components/Layout.tsx).

## L'état global, et son piège

`App.tsx` exporte **trois valeurs mutables**, lues par 17 fichiers :

```ts
export let UTILISATEUR_ACTIF: UtilisateurConnecte | null   // App.tsx:70
export let DEPOT_ACTIF: string | null                      // App.tsx:85
export function definirDepotActif(id: string | null)       // App.tsx:90
```

Ce sont des `let` de module, pas un contexte React. Elles ne déclenchent
**aucun rendu** : `App.tsx` maintient en parallèle un `useState`
(`depotActif`, ligne 117) dont le seul rôle est de forcer le rendu, la
valeur de référence restant `DEPOT_ACTIF`. Les deux doivent être
changées ensemble — voir le `onChangerDepot` ligne 267.

Le commentaire ligne 122 signale le piège déjà rencontré : un composant
lisait `UTILISATEUR_ACTIF` encore `null` au premier rendu.

## Fichiers

| Page | l. | Ce qu'elle pilote |
|---|---|---|
| [Ventes.tsx](../../src/pages/Ventes.tsx) | 1 670 | le POS : panier, scan, chèque, facture auto |
| [Pieces.tsx](../../src/pages/Pieces.tsx) | 1 661 | cycle documentaire complet, validation, annulation |
| [FicheClient.tsx](../../src/pages/FicheClient.tsx) | 1 414 | créances, règlements, avoirs, pièces d'un client |
| [Parametres.tsx](../../src/pages/Parametres.tsx) | 1 040 | conteneur des onglets de réglages |
| [FicheFournisseur.tsx](../../src/pages/FicheFournisseur.tsx) | 912 | dette, règlements, historique |
| [Achats.tsx](../../src/pages/Achats.tsx) | 845 | saisie d'un achat facturé |
| [Stock.tsx](../../src/pages/Stock.tsx) | 842 | état, entrées, ajustements, mouvements |
| [Journal.tsx](../../src/pages/Journal.tsx) | 838 | journal du jour |
| [Rapports.tsx](../../src/pages/Rapports.tsx) | 821 | 6 rapports ; **seul `import()` dynamique du projet** (`xlsx`, ligne 75) |
| [Clients.tsx](../../src/pages/Clients.tsx) | 720 | liste paginée, créances |
| [Relances.tsx](../../src/pages/Relances.tsx) | 674 | relances WhatsApp |
| [Caisse.tsx](../../src/pages/Caisse.tsx) | 624 | session, dépenses, clôture |
| [Dashboard.tsx](../../src/pages/Dashboard.tsx) | 591 | résumé, tops, ventes à découvert |
| [Fournisseurs.tsx](../../src/pages/Fournisseurs.tsx) | 551 | liste, dettes |
| [Transferts.tsx](../../src/pages/Transferts.tsx) | 452 | bons BTR |
| [Retours.tsx](../../src/pages/Retours.tsx) | 318 | retours et avoirs ouverts |
| [Cheques.tsx](../../src/pages/Cheques.tsx) | 248 | suivi des chèques reçus |
| [PageLogin.tsx](../../src/pages/PageLogin.tsx) | 128 | connexion ; exporte aussi `UtilisateurConnecte` |

## Entrant

Toutes : `App.tsx` seulement. `PageLogin.tsx` est la seule à être
importée deux fois (`App.tsx` pour l'écran, et pour son type
`UtilisateurConnecte`).

## Sortant

`@tauri-apps/api/core` (`invoke`), `@/components/ui/*`,
`@/lib/generer*` (documents HTML), `@/lib/types-api`.

## Règles

- [CONFIRMÉ] Un aperçu affiche le **document généré**, dans une `iframe`
  `sandbox` **sans** `allow-scripts` — jamais une reconstitution en JSX
  (D50). Voir [ApercuPiece.tsx](../../src/components/ApercuPiece.tsx).
- [CONFIRMÉ] Les relances WhatsApp passent par `ouvrir_avec_systeme`
  (commande Rust), pas `window.open` (D13) — Relances.tsx est le seul
  appelant.
- [DÉDUIT] Aucune page n'est paginée côté serveur pour les ventes :
  `lire_ventes_paginees` existe mais n'est appelée nulle part. Sur une
  base ancienne, la liste des ventes est chargée entière.
- [DÉDUIT] Cinq pages dépassent 800 lignes. Chercher un comportement
  précis dedans coûte plus cher que de repartir du nom de la commande
  `invoke` — la table commande → écran est dans les fiches `commandes-*`.
