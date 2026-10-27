# Module : front — composants

Rôle : ce qui est monté **dans** une page — modales, onglets, cadre —
et les primitives shadcn sans métier.

Deux étages à ne pas confondre :

- `src/components/*.tsx` — porte du **métier** et appelle `invoke`.
- `src/components/ui/*.tsx` — primitives shadcn, aucun `invoke`, aucune
  règle. Les modifier touche 10 à 38 fichiers.

## Composants métier

| Fichier | l. | Monté dans | Ce qu'il appelle |
|---|---|---|---|
| [ModalsRetour.tsx](../../src/components/ModalsRetour.tsx) | 491 | Pieces, Retours | `enregistrer_retour_groupe` — `ModalRetour` : les articles de la vente à cocher (déjà revenu dit, grisé s'il ne reste rien), rembourser / avoir / échange (articles pris), bilan rendu par le serveur, bon d'échange |
| [OngletChantiers.tsx](../../src/components/OngletChantiers.tsx) | 795 | Parametres | TVA, dettes, irrécouvrable, avoirs expirés — exporte **4** onglets |
| [ModalNouvellePiece.tsx](../../src/components/ModalNouvellePiece.tsx) | 652 | Pieces | création de pièce ; un article non suivi (D36) dit « stock non suivi » dans la recherche et « non suivi » sur sa ligne (lot C.8) |
| [OngletHorloge.tsx](../../src/components/OngletHorloge.tsx) | 140 | Parametres | `lire_horloge`, `confirmer_horloge` (lot C.5) : heure de l'ordinateur qui garde la base, dernière écriture ; quand l'écriture est bloquée, le bouton du patron « L'heure de Windows est juste » après confirmation |
| [ParametresSociete.tsx](../../src/components/ParametresSociete.tsx) | 464 | Parametres | identité, logo, en-tête, pied, signatures |
| [OngletDepots.tsx](../../src/components/OngletDepots.tsx) | 418 | Parametres | CRUD dépôt complet |
| [RetourFournisseur.tsx](../../src/components/RetourFournisseur.tsx) | 352 | Retours | `enregistrer_retour_fournisseur` |
| [OngletCodesBarres.tsx](../../src/components/OngletCodesBarres.tsx) | 328 | Parametres | les 4 commandes de `codebarre.rs` |
| [ParametresVentes.tsx](../../src/components/ParametresVentes.tsx) | 314 | Parametres | ⚠️ exporte `OngletVentes`, **pas** `ParametresVentes` |
| [OngletHistoriqueCaisse.tsx](../../src/components/OngletHistoriqueCaisse.tsx) | 309 | Parametres | sessions, mouvements, écarts |
| [FiltresAvances.tsx](../../src/components/FiltresAvances.tsx) | 299 | Pieces | exporte aussi `FILTRES_VIDES`, `FiltresState` |
| [ModalLivraison.tsx](../../src/components/ModalLivraison.tsx) | 244 | Pieces | suivi de livraison (D49) |
| [ApercuPiece.tsx](../../src/components/ApercuPiece.tsx) | 234 | Pieces, FicheClient, FicheFournisseur | aperçu en `iframe` sandbox |
| [Layout.tsx](../../src/components/Layout.tsx) | 202 | App.tsx | cadre, navigation, sélecteur de dépôt |
| [OngletImportExport.tsx](../../src/components/OngletImportExport.tsx) | 197 | Parametres | CSV |
| [ModalImpression.tsx](../../src/components/ModalImpression.tsx) | 178 | Ventes | choix de format |
| [ModalModifierTiers.tsx](../../src/components/ModalModifierTiers.tsx) | 173 | FicheClient, FicheFournisseur | `modifier_client` **et** `modifier_fournisseur` |
| [ApercuRecu.tsx](../../src/components/ApercuRecu.tsx) | 150 | FicheClient, FicheFournisseur | reçu de règlement |
| [ModalChangerMdp.tsx](../../src/components/ModalChangerMdp.tsx) | 136 | App.tsx | `changer_mot_de_passe` |
| [ModalOuvrirCaisse.tsx](../../src/components/ModalOuvrirCaisse.tsx) | 105 | Caisse | exporte `estCaisseFermee` — **le détecteur de `CAISSE_FERMEE`** |
| [SelectUnite.tsx](../../src/components/SelectUnite.tsx) | 83 | Ventes, Achats, Parametres | choix d'unité de vente |
| [MoneyInput.tsx](../../src/components/MoneyInput.tsx) | 69 | 10 fichiers | saisie de montant ; exporte `parseMontant` |
| [Pagination.tsx](../../src/components/Pagination.tsx) | 74 | Clients, Fournisseurs, Stock | contrôles de page |
| [SelecteurProfil.tsx](../../src/components/SelecteurProfil.tsx) | 110 | **aucun** | mort, cf. [ALERTES.md](../ALERTES.md) |

## Primitives shadcn (`components/ui/`)

| Fichier | importé par |
|---|---|
| [button.tsx](../../src/components/ui/button.tsx) | 38 |
| [input.tsx](../../src/components/ui/input.tsx) | 28 |
| [label.tsx](../../src/components/ui/label.tsx) | 24 |
| [dialog.tsx](../../src/components/ui/dialog.tsx) | 22 |
| [select.tsx](../../src/components/ui/select.tsx) | 15 |
| [badge.tsx](../../src/components/ui/badge.tsx) | 12 |
| [card.tsx](../../src/components/ui/card.tsx) | 11 |
| [GlassIcon.tsx](../../src/components/ui/GlassIcon.tsx) | 5 |
| [KpiVerre.tsx](../../src/components/ui/KpiVerre.tsx) | 4 |
| [dropdown-menu.tsx](../../src/components/ui/dropdown-menu.tsx) | 1 |
| [table.tsx](../../src/components/ui/table.tsx) | **0 — mort** |

`GlassIcon.tsx` et `KpiVerre.tsx` ne viennent pas de shadcn : ce sont des
composants maison (effet verre, tuiles de KPI).

## Règles

- [CONFIRMÉ] shadcn ignore `max-w-*` sur `DialogContent` : la largeur se
  pose en **style inline** (D22). Un `className` sera silencieusement
  sans effet.
- [CONFIRMÉ] `estCaisseFermee` reconnaît l'erreur Rust par sa **chaîne**
  `CAISSE_FERMEE` ([utils.rs:40](../../src-tauri/noyau/src/utils.rs#L40)).
  Reformuler ce message côté Rust casse la modale d'ouverture de caisse,
  sans erreur de compilation d'aucun côté.
- [CONFIRMÉ] Les images partent **en contenu** (D8) :
  `ParametresSociete.tsx` lit le fichier choisi
  (`@tauri-apps/plugin-fs`), l'encode en base64 et envoie
  `{nom, contenu}` — jamais un chemin local.
- [CONFIRMÉ] `ParametresVentes.tsx` exporte `OngletVentes` : le nom du
  fichier et celui du symbole divergent. Chercher par nom de fichier ne
  donne rien.
- [CONFIRMÉ] `ModalsRetour.tsx` redéclare `LigneVente` et `UniteVente`,
  également définis dans `lib/remise.ts` et `lib/tauri.ts` — tous deux
  morts. Les copies vivantes sont celles de `ModalsRetour.tsx`.
- [DÉDUIT] `table.tsx` est mort parce que 13 fichiers écrivent leur
  `<table>` à la main. Adopter la primitive serait un chantier, pas une
  correction.

## Ajouts du 25-26/10/2026

- `PageRolePoste.tsx` (page) — rôle du poste au premier démarrage, et
  `PageServeurLocal` qui joint ou lance le serveur de la machine (D22/D24
  révisés).
- `ListePassages.tsx` — Clients → « Passages » : fiches de passage par
  code ou numéro de facture, « Passer en client ordinaire » (D35) et
  « Rattacher à un client » (lot D.10) : une fenêtre cherche un client
  ordinaire (`lire_clients_pagines`, nom ou téléphone, 8 résultats), dit
  ce qui va passer et que le geste ne se défait pas, puis
  `rattacher_passage` ; le message rend le compte des lignes déplacées.
  Les deux boutons suivent `clients:modifier`.
- `OngletImportExport.tsx` — le format décrit la colonne facultative
  « Suivre le stock » (lot D.11).
- `OngletMenu.tsx` + `lib/menu.ts` — Paramètres → Menu : ce que le menu
  du dossier cache ; `Layout` (menu et Ctrl K) suit `estCache` (D37).
- `SuiviStockArticle.tsx` — Paramètres → Articles : suivre ou non le
  stock d'un article, reprise par comptage de chaque magasin (D36) ;
  Stock → « Non suivis » ; la caisse (Ventes) ne signale ni rupture ni
  découvert pour un article non suivi.

## Ajouts du 27/10/2026

- `RechercheServeur.tsx` — le bouton « Rechercher le serveur » : commande
  LOCALE `rechercher_serveurs` (`pont.ts::LOCALES`, diffusion UDP 40000,
  `src-tauri/src/reseau.rs`), liste des serveurs trouvés (nom du poste,
  IP, version ; hors plage ou autre protocole désactivé ou signalé) ; un
  clic rend l'adresse conseillée (le nom s'il mène à ce serveur, sinon
  l'IP) au parent, qui la **teste** (`tester_serveur`) avant tout
  enregistrement. Rien trouvé : les trois causes probables (pare-feu UDP,
  autre réseau, serveur arrêté) et la saisie à la main. Absent hors de
  Tauri (`estDansTauri`, exportée de `pont.ts`). Dans `PageServeur`
  (branchement d'une caisse) et `OngletReseau` (poste caisse). Fiche :
  [serveur-fenetre.md](serveur-fenetre.md).
