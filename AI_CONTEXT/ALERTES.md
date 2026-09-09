# Alertes

Généré depuis [carte.json](carte.json), puis vérifié à la main.

> ⚠️ Le script résout les imports statiques uniquement. Le pont
> `invoke("nom")` entre React et Rust est une **chaîne de caractères** :
> il n'apparaît dans aucun graphe d'import. Les listes ci-dessous
> croisent donc les deux sources.

---

## Fichiers orphelins — vérifiés

Aucun import ne les atteint. Vérifiés un par un par `grep` sur le nom du
symbole, y compris les imports dynamiques.

| Fichier | l. | Verdict |
|---|---|---|
| [src-tauri/src/portes.rs](../src-tauri/src/portes.rs) | 23 | **Mort, et ne compile pas.** `ErreurPermission` n'est défini nulle part, et `mod portes;` n'est pas déclaré dans `lib.rs` — le fichier n'est jamais compilé. Esquisse d'un modèle de permissions (patron / employé / lecture) jamais branchée. |
| [src/components/SelecteurProfil.tsx](../src/components/SelecteurProfil.tsx) | 110 | **Mort.** Remplacé par `PageLogin.tsx` (authentification réelle). |
| [src/components/ui/table.tsx](../src/components/ui/table.tsx) | 115 | **Mort.** Primitive shadcn jamais adoptée : 13 fichiers écrivent leur `<table>` à la main. |
| [src/lib/session.ts](../src/lib/session.ts) | 48 | **Mort, et dupliqué.** La logique de session vit en réalité dans [App.tsx:28-60](../src/App.tsx#L28) (`CLE_SESSION`, `sauvegarderSession`). Deux copies de la même règle des 8 h ; celle-ci diverge en silence. |
| [src/lib/tauri.ts](../src/lib/tauri.ts) | 137 | **Mort.** Ancienne façade typée sur `invoke` (`creerVente`, `enregistrerPaiement`, `UTILISATEUR_ID`…). Les pages appellent `invoke` directement. Redéclare `UniteVente`. |
| [src/lib/remise.ts](../src/lib/remise.ts) | 79 | **Mort.** `calculerLigne` / `calculerTotaux` — le calcul de remise vit aujourd’hui côté Rust (`commandes/pieces.rs`, 16 occurrences de `remise_pct`). Redéclare `LigneVente` et `fmt`. |
| `creer_tiers_test.py`, `generer_catalogue.py`, `nettoyer_imports.py`, `preparer_demo_video.py`, `t_regles.py` | ~1 700 | **Vivants mais hors application.** Scripts d'outillage lancés à la main (jeux de test, démo vidéo). Leurs « doublons » de symboles avec le Rust sont des **réimplémentations de vérification** (`t_regles.py` rejoue `reste_exigible`, `statut_vente`, `peut_modifier`), pas du code mort. |

**Rien d'autre n'est orphelin** : 104 des 115 fichiers sont atteints.

## Commandes enregistrées, jamais appelées

Déclarées dans `generate_handler!` mais introuvables dans `src/`. Elles
compilent, elles sont exposées, et rien ne les invoque — coût zéro à la
compilation, donc rien ne les signalera jamais.

```
pieces::annuler_facture_par_avoir        pieces::changer_statut_piece
pieces::creer_piece_fournisseur          pieces_pos::modifier_facture_pos
pieces_pos::valider_facture_credit       avoirs::appliquer_avoir_vente
creances::solder_residus_creances        ventes::lire_clients_avec_creances
chantiers::lire_factures_fournisseur_ouvertes
caisse::lire_depenses_du_jour            caisse::modifier_depense
depots::lire_stock_depot                 depots::lire_stock_article_depots
parametres::lire_stocks
pagination::lire_ventes_paginees         pagination::lire_ventes_recentes_paginee
```

16 sur 174. Deux familles s'y distinguent :

- **`pagination::lire_ventes_paginees` et `lire_ventes_recentes_paginee`**
  — les quatre autres commandes de `pagination.rs` sont bien utilisées.
  Les listes de ventes sont donc chargées **sans** pagination, alors que
  la commande existe.
- **`creances::solder_residus_creances`** — c'est l'application de D41
  (seuil de 5 F). Le calcul `reste_exigible` est appliqué à la lecture,
  mais rien ne déclenche jamais le solde en base.

Ne rien supprimer sans vérifier : plusieurs sont des demi-chantiers
volontairement laissés branchés côté Rust.

## Symboles définis à deux endroits

Vrais doublons (deux copies qui peuvent diverger) :

| Symbole | Fichiers |
|---|---|
| `LigneVente` | `components/ModalsRetour.tsx`, `lib/remise.ts` *(mort)* |
| `UniteVente` | `components/ModalsRetour.tsx`, `lib/tauri.ts` *(mort)* |
| `fmt` | `lib/remise.ts` *(mort)*, `preparer_demo_video.py` |
| session 8 h | `App.tsx` *(vivant)*, `lib/session.ts` *(mort)* |

Faux positifs — même nom, rôles distincts, pas de duplication à corriger :

- `cle_ean13`, `peut_modifier`, `prochain_numero`, `reste_exigible`,
  `statut_vente`, `valider_facture` : le Rust d'un côté, un script Python
  de vérification (`t_regles.py`) de l'autre. **La copie Python est là
  pour attraper une divergence** — la supprimer perdrait le contrôle.
- `main`, `supprimer`, `telephone`, `base_par_defaut` : noms génériques
  dans des scripts indépendants.

## Fichiers les plus sollicités

Les modifier a le plus d'effets de bord.

| Fichier | importé par |
|---|---|
| [src-tauri/src/commandes/ventes.rs](../src-tauri/src/commandes/ventes.rs) | 47 |
| [src/components/ui/button.tsx](../src/components/ui/button.tsx) | 38 |
| [src/components/ui/input.tsx](../src/components/ui/input.tsx) | 28 |
| [src/components/ui/label.tsx](../src/components/ui/label.tsx) | 24 |
| [src/components/ui/dialog.tsx](../src/components/ui/dialog.tsx) | 22 |
| [src-tauri/src/coeur/calcul.rs](../src-tauri/src/coeur/calcul.rs) | 20 |
| [src/lib/utils.ts](../src/lib/utils.ts) | 18 |
| [src/App.tsx](../src/App.tsx) | 17 |
| [src/components/ui/select.tsx](../src/components/ui/select.tsx) | 15 |
| [src-tauri/src/coeur/pieces.rs](../src-tauri/src/coeur/pieces.rs) | 13 |

`commandes/ventes.rs` en tête n'est pas un hasard : il porte `EtatApp`
(le `Mutex<Connection>`), que **toute** commande doit importer.

## Bruit dans l'arborescence

Deux répertoires **vides** au nom littéral, laissés par un `mkdir` dont
les accolades n'ont pas été développées (PowerShell ne fait pas
l'expansion d'accolades de bash) :

```
src/{pages,components,lib}/
src-tauri/src/{coeur,persistance,commandes}/
```

Sans effet, mais ils polluent toute recherche par glob. Supprimables.

## Fichiers volumineux

Au-delà de ~800 lignes, un fichier ne tient plus en tête ni en contexte.

| Fichier | l. |
|---|---|
| [src-tauri/src/commandes/pieces.rs](../src-tauri/src/commandes/pieces.rs) | 2 143 |
| [src/pages/Ventes.tsx](../src/pages/Ventes.tsx) | 1 670 |
| [src/pages/Pieces.tsx](../src/pages/Pieces.tsx) | 1 661 |
| [src/pages/FicheClient.tsx](../src/pages/FicheClient.tsx) | 1 414 |
| [src/pages/Parametres.tsx](../src/pages/Parametres.tsx) | 1 040 |
| [src/lib/genererPDF.ts](../src/lib/genererPDF.ts) | 965 |
| [src/pages/FicheFournisseur.tsx](../src/pages/FicheFournisseur.tsx) | 912 |
