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
| [Parametres.tsx](../../src/pages/Parametres.tsx) | 1 089 | conteneur des onglets de réglages — sans l'entretien, parti à la console du serveur (D9) ; onglet **Horloge** (`parametres:modifier`, lot C.5) |
| [FicheFournisseur.tsx](../../src/pages/FicheFournisseur.tsx) | 912 | dette, règlements, historique |
| [Achats.tsx](../../src/pages/Achats.tsx) | 845 | saisie d'un achat facturé ; un article non suivi (D36) y est marqué « stock non suivi » au lieu de son stock, sans « stock après » (lot C.8) |
| [Stock.tsx](../../src/pages/Stock.tsx) | 842 | état, entrées, ajustements, mouvements |
| [Journal.tsx](../../src/pages/Journal.tsx) | 838 | journal du jour |
| [Rapports.tsx](../../src/pages/Rapports.tsx) | 821 | 6 rapports ; **seul `import()` dynamique du projet** (`xlsx`, ligne 75) |
| [Clients.tsx](../../src/pages/Clients.tsx) | 720 | liste paginée, créances |
| [Relances.tsx](../../src/pages/Relances.tsx) | 674 | relances WhatsApp |
| [Caisse.tsx](../../src/pages/Caisse.tsx) | 624 | session, dépenses, clôture |
| [Dashboard.tsx](../../src/pages/Dashboard.tsx) | 591 | résumé, tops, ventes à découvert |
| [Fournisseurs.tsx](../../src/pages/Fournisseurs.tsx) | 551 | liste, dettes |
| [Transferts.tsx](../../src/pages/Transferts.tsx) | 452 | bons BTR |
| [Retours.tsx](../../src/pages/Retours.tsx) | 360 | retours et avoirs ouverts ; ventes par pages de 50 (`lire_ventes_recentes_paginee`, recherche serveur client/code/n° de facture, `Pagination`) depuis le lot D.12 — Pièces retrouve la vente d'une facture par la même lecture |
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

## AssistantDossier (D43, 28/10/2026)

`pages/AssistantDossier.tsx` — montré par `App.tsx` à la place de tout
(Layout compris) quand `lire_assistant` rend `a_faire`. Huit étapes :
bienvenue, vous, société (logo par `envoyerImage`, exporté de
`ParametresSociete`), dates, réglages, vendeurs, récapitulatif →
`terminer_assistant` (une `saisie` en snake_case), puis reprise des
données (`ImportCsvAvecApercu`, `ImportExportFournisseurs`). Sans
`dossiers:gerer` : « le patron n'a pas encore créé le dossier ».
`PageLogin` lit `sante().assistant_a_faire` pour rappeler le compte
d'usine. `ModalOuvrirCaisse` et `Caisse` proposent le fond habituel
(`lib/reglagesDossier.ts`) ; `ReglagesDuDossier.tsx` sous Paramètres →
Société (le fond de caisse, D45 le seuil d'alerte par défaut) ;
`ChampSeuil.tsx` (D45 : création dans Paramètres et Achats,
`SeuilArticle` sur la fiche dépliée ; le badge de `Stock.tsx` lit
`statut` du serveur) ; `EtatTvaBoutique.tsx` en tête de
Paramètres → TVA (`OngletTVA`, à la place de l'ancien « Taux TVA
global ») ; `lib/homonyme.ts` pose « créer quand même ? » aux quatre
endroits qui créent ou modifient un fournisseur. `OngletHistoriqueCaisse`
(28/10) : `relative z-[1]` — la page Caisse peint ses halos par-dessus ce
qui n'est pas positionné —, pas de pictogramme devant les chiffres,
couleurs de texte ≥ 4,5:1.
