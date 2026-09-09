# Module : front — lib

Rôle : la génération des **documents imprimables**, plus quelques
helpers. Un tiers du dossier est mort (cf. [ALERTES.md](../ALERTES.md)).

## Fichiers vivants

| Fichier | l. | Rôle | importé par |
|---|---|---|---|
| [genererPDF.ts](../../src/lib/genererPDF.ts) | 965 | le générateur central — factures, bons, tickets thermiques | 5 |
| [genererReleve.ts](../../src/lib/genererReleve.ts) | 539 | relevés client, relevé global, historique de règlements | 4 |
| [types-api.ts](../../src/lib/types-api.ts) | 364 | types de retour des commandes Rust, **redéclarés à la main** | 3 |
| [genererRecu.ts](../../src/lib/genererRecu.ts) | 290 | reçu de règlement | 1 |
| [genererManuel.ts](../../src/lib/genererManuel.ts) | 230 | le manuel utilisateur en HTML | 1 |
| [unites.ts](../../src/lib/unites.ts) | 89 | `TOUTES_UNITES`, `UNITES_COURANTES`, `normaliserUnite` | 1 |
| [useScanner.ts](../../src/lib/useScanner.ts) | 81 | hook de scan douchette | 1 |
| [utils.ts](../../src/lib/utils.ts) | 7 | `cn()` (clsx + tailwind-merge) | 18 |

## Fichiers morts

`session.ts` (48 l.), `tauri.ts` (137 l.), `remise.ts` (79 l.) — voir
[ALERTES.md](../ALERTES.md). Les trois **redéclarent** des symboles
vivants ailleurs : ne pas les lire pour comprendre le comportement
actuel.

## Fonctions exposées — genererPDF.ts

- `genererImpression(donnees, format) -> string` — l'entrée principale ;
  c'est ce que les aperçus injectent dans leur `iframe`.
- `genererPieceHTML`, `genererFactureEtBonHTML`, `genererBonSortieHTML`,
  `genererBonEchangeHTML`, `genererTicketThermique`.
- Types : `DonneesPiece`, `DonneesEchange`, `FormatImpression`,
  `Signatures`.

## genererReleve.ts

`genererReleveHTML`, `genererReleveGlobalHTML`,
`genererHistoriqueReglementsHTML` + les types `DonneesReleve`,
`DonneesReleveGlobal`, `LigneReleve`, `LigneReleveGlobal`,
`LigneHistorique`.

## Entrant

`pages/Pieces.tsx`, `FicheClient.tsx`, `FicheFournisseur.tsx`,
`Clients.tsx`, `Fournisseurs.tsx`, `components/ApercuPiece.tsx`,
`ApercuRecu.tsx`, `ModalImpression.tsx`, `ModalsRetour.tsx`.

`utils.ts` (`cn`) est importé par 18 fichiers — surtout `components/ui/`.

## Règles

- [CONFIRMÉ] Ces modules produisent une **chaîne HTML**, pas un PDF,
  malgré le nom `genererPDF.ts`. Le rendu final est fait par la fenêtre
  d'impression Tauri (D3).
- [CONFIRMÉ] Les images sont incluses en base64 dans le HTML (D4) : le
  document doit être autonome. C'est pourquoi 9 fichiers du front
  appellent `lire_logo_base64` / `lire_entete_base64` /
  `lire_pied_base64` chacun de leur côté.
- [CONFIRMÉ] L'aperçu injecte ce HTML dans une `iframe` `sandbox`
  **sans** `allow-scripts` (D50). Ajouter du JS au document généré le
  rendrait inerte à l'aperçu tout en marchant à l'impression.
- [DÉDUIT] `types-api.ts` ne couvre qu'une partie des 174 commandes ; le
  reste est typé en ligne dans les pages. Rien ne garantit que ces types
  correspondent encore aux `struct` Rust — **il n'y a aucune génération
  automatique**. Une divergence ne se voit qu'à l'exécution.
- [DÉDUIT] `xlsx` (SheetJS) n'est chargé qu'en `import()` dynamique,
  depuis [Rapports.tsx:75](../../src/pages/Rapports.tsx#L75) — il pèse à
  lui seul plus de 500 kB, d'où le `chunkSizeWarningLimit: 1200` dans
  [vite.config.ts](../../vite.config.ts).
