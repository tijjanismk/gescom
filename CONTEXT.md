# Gescom — Contexte de développement

**Dernière mise à jour** : 2026-09-06
**Version** : v1.9.0
**Repo** : https://github.com/tijjanismk/gescom.git

> Ce fichier vit désormais **dans le dépôt**. Les copies `CONTEXT (n).md`
> de `~/Downloads` sont caduques : une seule version, suivie par git,
> qui avance avec le code au lieu de le suivre de loin.

---

## Stack
- **Backend** : Tauri 2 + Rust + SQLite (rusqlite bundled)
- **Frontend** : React 18 + Vite + TypeScript + shadcn/ui + Tailwind
- **Auth** : bcrypt 0.15
- **Cible** : PME/commerçants Bamako, Mali — local-first, Windows

---

## ⚠️ Conventions à connaître avant de coder

### La TVA est AJOUTÉE au HT (D8)

Les prix saisis sont **hors taxe**. Le POS convertit en TTC avant
l'appel, si bien que `ligne_vente.prix_pratique` contient **ce que le
client paie**. `SUM(prix_pratique × quantite)` est donc le montant dû,
partout, sans correction.

La base HT vaut `SUM(prix_pratique × qte) − SUM(montant_tva)`.

### Une seule caisse

`mouvement_caisse` n'a **pas** de `depot_id`, et ce n'est pas un oubli.
Il y a un tiroir, physique, commun à tous les dépôts. C'est ce qui
distingue le multi-dépôt du multi-magasin.

Le rapprochement porte sur `moyen = 'especes'` uniquement (D29), en
excluant `motif = 'ouverture'` — le fond est déjà porté par
`session_caisse.fond_ouverture`.

### Le montant dû se dérive des prix stockés (D31)

Jamais de la somme arithmétique des TTC de lignes. Les requêtes
recalculent `CAST(SUM(prix_pratique × quantite) AS INTEGER)` à partir du
prix unitaire **arrondi**, et `CAST` tronque. Une divergence d'un franc
laisse une créance résiduelle qui ne se solde jamais.

### Le stock et l'argent sortent à UN seul endroit

`valider_facture` (`pieces.rs`). Tout le reste — bon de livraison, suivi
de livraison, aperçu — est du document ou de l'information. Avant
d'ajouter un effet stock ailleurs, relire D49.

---

## Architecture src-tauri/src/

```
main.rs · lib.rs · utils.rs · seed.rs

coeur/                  calculs purs, TESTÉS — 43 tests
  calcul.rs             montant_ligne, reste_du, statut_vente
  caisse.rs             solde_theorique, ecart_caisse
  pieces.rs             peut_modifier, peut_annuler (règle OHADA)
  codebarre.rs          EAN-13 : clé, génération, validation
  stock.rs              types de mouvement — source unique (D42)

persistance/
  mod.rs                migrations — un execute_batch par CREATE TABLE
  schema.sql            29 tables, régénéré depuis une base réelle
  journal.rs

commandes/
  ventes.rs             creer_vente ATOMIQUE, modifier_client
  achats.rs             enregistrer_achat, retour fournisseur
  pieces.rs             prochain_numero, valider_facture, annulation
                        par avoir, commande → BL + facture
  pieces_pos.rs         creer_facture_depuis_vente
  livraisons.rs         suivi de livraison — informatif (D49)
  transferts.rs         bons BTR entre dépôts
  depots.rs             CRUD dépôts, stock multi-dépôts, mouvements
  codebarre.rs          attribution EAN-13
  cheques.rs            suivi des chèques reçus
  caisse.rs             sessions, dépenses, historique, écarts
  journal.rs            journal quotidien
  fournisseurs.rs       ⚠️ contient TOUT le fournisseur (D12)
  chantiers.rs          TVA, dettes, irrécouvrable ⚠️ ne pas écraser
  creances.rs           règlements, état de créance client
  relances.rs · retours.rs · avoirs.rs
  rapports.rs · dashboard.rs · pagination.rs
  parametres.rs         réglages d'écran (scanner, bon de sortie,
                        suivi de livraison) — regroupés ICI
  societe.rs · auth.rs · logo.rs
  impression.rs · sauvegarde.rs
```

---

## Décisions

| # | Décision |
|---|---|
| D1 | Session localStorage, 8h. À remplacer par JWT en v2. |
| D2 | bcrypt (pas argon2 — rand_core sur Windows) |
| D3 | Impression : HTML → fenêtre **Tauri**, jamais le navigateur (en-têtes) |
| D4 | Logo en base64 dans le HTML |
| D5 | Avoirs consommés du plus ancien au plus récent |
| D6 | Scanner : keydown global < 80ms + Enter |
| D7 | Irrécouvrable : jamais de suppression |
| **D8** | **TVA ajoutée au HT** ; `prix_pratique` stocké TTC |
| D9 | Dette fournisseur = `SUM(FAF)` − `SUM(AVF non payés)` − paiements |
| D10 | Montants `i64` FCFA, jamais de flottant |
| D11 | Pièces : transfert unique, sauf conversions fournisseur |
| D12 | `fournisseurs.rs` contient tout |
| D13 | Relances WhatsApp via `ouvrir_avec_systeme`, pas `window.open` |
| D16 | Facture POS auto après `creer_vente`, en try/catch |
| D19 | `taux_tva_defaut` fourni par `lire_articles_avec_unites` |
| D22 | shadcn ignore `max-w-*` sur DialogContent → style inline |
| D26 | `persistance/mod.rs` : un `execute_batch` par CREATE TABLE |
| D28 | Numérotation par `MAX(substr(numero,-5))`, jamais `COUNT` |
| D29 | Rapprochement caisse : espèces seules, hors `ouverture` |
| D30 | Facture fournisseur : comptant → `paye`, crédit → `emis` |
| **D31** | Montant dû dérivé des `prix_pratique` stockés |
| **D32** | Un transfert refuse de mettre le dépôt source à découvert |
| **D33** | Un retour éteint d'abord la dette, puis rend le solde |
| **D34** | Codes-barres internes : préfixe `20`, réservé usage privé |
| **D35** | Chèque rejeté → paiement annulé, créance rouverte |
| **D36** | **Dette/créance lue dans `paiement`/`paiement_fournisseur`, jamais dans le `statut` seul** |
| D37 | `BRF → FAF` interdit par copie, uniquement via `enregistrer_achat` |
| D38 | Acompte à l'achat, symétrique de `valider_facture` côté vente |
| **D39** | **`unite_base` = plus petite unité vendable ; `facteur` toujours en unités de base, jamais emboîté** |
| **D40** | **Client générique : ni vente à crédit, ni avoir. Remboursement ou échange** |
| **D41** | **Reste dû ≤ `SEUIL_SOLDE` (5 F) non exigible — ferme le statut, ne crée aucun paiement, s'écrit au journal** |
| **D42** | **`achat` = facturé (dette + caisse) ; `entree` = marchandise sans facture. Types de mouvement déclarés dans `coeur/stock.rs`, nulle part ailleurs** |
| **D43** | **Un échange sort le remplacement du dépôt de la vente, jamais du dépôt par défaut** |
| **D44** | **`avoir.piece_id` relie un avoir à son AVC ; le reste d'une AVC est le crédit encore ouvert, pas un impayé. L'AVC d'annulation n'est pas liée et naît `paye`** |
| **D45** | **Code-barres porté par `unite_vente` : le carton a son propre EAN. `article.code_barre` reste pour l'unité de base. Un code est unique sur les deux tables** |
| **D46** | **Caisse fermée = opération d'argent refusée, sur les 10 points d'argent (`utils::exiger_session_caisse`, code `CAISSE_FERMEE`)** |
| **D47** | **Un AVC portant du crédit ouvert ne s'annule pas ; un AVC créée à la main crée son crédit (geste commercial)** |
| **D48** | **Santé de la base : `quick_check` + anomalies métier, au démarrage et à la demande. Ne répare rien, dit de restaurer** |
| **D49** | **Suivi de livraison = axe d'information PARALLÈLE au paiement. Aucun effet stock ni caisse, aucune session exigée. L'état se DÉRIVE de `ligne_piece.quantite_livree`, jamais stocké. Désactivé par défaut** |
| **D50** | **Un aperçu affiche le document généré (`genererImpression` en iframe `sandbox` SANS `allow-scripts`), jamais une reconstitution en JSX** |
| **D51** | **`mouvement_stock.operation_id` porte l'id de la PIÈCE pour `achat` et `retour_fournisseur` — c'est ce qui permet de retrouver le numéro de facture d'un mouvement** |

---

## Statuts de pièce

```
brouillon → emis → paye
     ↘ transfere        ↘ annule
```

| Statut | Sens | Lignes modifiables |
|---|---|---|
| `brouillon` | En préparation | oui |
| `emis` | Sortie, reste dû | oui |
| `paye` | Soldée | **non** |
| `transfere` | Convertie | non |
| `annule` | Annulée | non |

`validee` est un **ancien** statut, encore présent en base sur des
pièces historiques. Traité comme clos. Ne plus l'écrire.

### Axe livraison (D49) — indépendant du statut ci-dessus

```
non_livre → partiel → livre        (sans_objet si la pièce n'a aucune ligne)
```

Croiser les deux axes rend représentable le « payé non livré ». Le badge
`non_livre` n'est jamais affiché : tant que rien n'est parti, la pièce
est dans son état normal.

---

## Numérotation

```
DEV · PRO · CMD · BL · FAC · ACP · AVC      (client)
BCF · BRF · FAF · AVF                        (fournisseur)
BTR                                          (transfert)
```

Un préfixe = un type = un compteur. `prochain_numero` (`pieces.rs`,
`pub(crate)`) est le **seul** point de génération.

⚠️ La table `facture` legacy (`GESCOM-…`) n'est **plus alimentée**.
Conservée pour l'historique des numéros déjà remis à des clients.

---

## Flux des pièces

**Suivi de livraison désactivé** — le défaut, la majorité des commerçants :

```
devis/proforma → commande → facture → (valider) vente
```

**Suivi activé** — le BL s'intercale, il constate le départ :

```
devis/proforma → commande → BL → facture → (valider) vente
                     └──── BL + facture d'un coup ────┘
```

Côté fournisseur, symétrique, avec le vocabulaire inversé (« reçu », pas
« livré ») :

```
BCF → BRF → facture fournisseur (via enregistrer_achat SEULEMENT, D37)
```

Dans tous les cas, stock et caisse ne bougent qu'à `valider_facture`.

---

## Multi-dépôt

Plusieurs lieux de stockage pour un **même** commerce, un seul
ordinateur, une seule caisse.

- Sélecteur dans la sidebar, masqué s'il n'y a qu'un dépôt
- `DEPOT_ACTIF` exporté par `App.tsx`, persisté en localStorage
- Filtre le dashboard et le journal, **pas** la caisse
- L'écran Ventes s'aligne dessus et le met à jour en retour

**Vente répartie** : quand le dépôt courant ne suffit pas mais qu'un
autre a le complément, un modal demande d'où sort chaque unité. Une
ligne de panier par dépôt — `ligne_vente.depot_source_id` le permettait
depuis l'origine.

Pas de transfert automatique : la marchandise part directement du dépôt
vers le client, aucun mouvement vers la boutique n'a eu lieu.

⚠️ Toute requête de tableau de bord doit accepter `depot_id`. Un
graphique non filtré à côté d'un total filtré affiche deux vérités
différentes — c'était le cas de `lire_ventes_du_jour`.

---

## Invariants

```
1.  Montants i64 FCFA — jamais f64
2.  TVA ajoutée au HT ; prix_pratique = TTC = montant dû
3.  Le montant dû se dérive des prix_pratique stockés (D31)
4.  Pièce payée ou validée = immuable — corriger par un avoir
5.  Numérotation par MAX, jamais COUNT
6.  Rapprochement caisse = espèces, hors ouverture
7.  Un retour ne rend jamais plus que ce que le client a versé
8.  creer_vente et enregistrer_achat = UNE transaction
9.  fournisseurs.rs contient tout ; chantiers.rs à lire avant d'écraser
10. persistance/mod.rs = un execute_batch par CREATE TABLE
11. Le journal est append-only
12. Un reste dû se calcule par coeur::calcul::reste_exigible, jamais
    par une soustraction locale (D41)
13. Client générique = ni crédit ni avoir (D40)
14. type_mouvement s'écrit depuis coeur::stock, jamais en littéral
15. Toute impression passe par genererImpression (D3)
16. Une commande Tauri non déclarée dans lib.rs est invisible au front
17. Le stock et l'argent ne sortent qu'à valider_facture (D49)
18. Les réglages d'écran (bascules on/off) vivent dans parametres.rs
```

---

## Pièges rencontrés — à ne pas rejouer

**Écarts de clé front/back.** Six fois dans le projet : `taux` vs
`taux_tva`, `id` vs `vente_id`, `unite_id` vs `unite_vente_id`,
`creer_fournisseur` renvoyant un objet annoté `<string>`,
`nouvelle_quantite` vs `quantiteReelle`, et `VenteJour.heure` déclaré
`string` alors que le backend envoie un entier. Les commandes renvoient
du `serde_json::Value` : TypeScript ne vérifie rien. `src/lib/types-api.ts`
déclare les formes réelles — l'étendre plutôt que d'annoter en ligne.

**`hidden` dans un tableau = toute la table saute au survol.**
`hidden` est `display:none`, donc largeur nulle ; le contenu qui
réapparaît au survol élargit la colonne, et en largeur automatique le
navigateur recalcule **toutes** les colonnes. Utiliser
`invisible group-hover:visible` — `visibility` garde la place réservée.

**Un pourcentage appliqué en pixels.** Le graphe du dashboard calculait
`(montant / max) × 100` puis l'écrivait en `px` dans un conteneur de
96px : la barre du pic débordait de la carte. Les hauteurs
proportionnelles s'expriment en `%` d'un conteneur de hauteur fixe.

**Un paramètre Tauri nommé `etat` masque la fonction `etat()` du
module.** Erreur `E0618 : expected function, found State<EtatApp>`.
Renommer le paramètre (`etat_app`), pas la fonction métier.

**Collision de noms avec les globaux du navigateur.** `History` de
lucide-react entre en conflit avec `window.History` : React tente de
l'instancier, « Illegal constructor ». Importer `History as HistoryIcon`.
Même risque avec `Image`, `Text`, `Option`, `Audio`, `Navigator`.

**`CAST` tronque en SQLite**, il n'arrondit pas. C'est ce qui laissait
des créances résiduelles d'un franc, jamais soldables : d'où le seuil
de D41.

**Règle dupliquée à la main.** `reste = total - paye` et `paye >= total`
étaient réécrits à **neuf** endroits (`pieces.rs` ×6, `chantiers.rs` ×2,
`pieces_pos.rs`). Une correction sur deux d'entre eux laisse l'écran
Pièces en désaccord avec la vente. Passer par `coeur/`, toujours.
Corollaire côté impression : un document ne recalcule jamais un montant,
il affiche celui que le backend lui donne.

**Jointure sans condition.** `JOIN depot d ON d.est_defaut = 1` n'est
pas une jointure : zéro ligne si aucun dépôt n'est marqué par défaut,
donc `QueryReturnedNoRows` en pleine transaction. Lire séparément, avec
un repli explicite.

**Un `operation_id` qui ne mène à rien.** `enregistrer_achat` y écrivait
un UUID de lot sans rapport avec la facture créée : impossible de
remonter d'un mouvement de stock à sa FAF. Y écrire l'id de la pièce
(D51). Les mouvements antérieurs restent orphelins.

**`as const` sur une ternaire** est un `TS1355`. Écrire
`cond ? ("a" as const) : ("b" as const)`, pas `(cond ? "a" : "b") as const`.

---

## Tests

```bash
cd src-tauri && cargo test     # 43 tests — coeur/ + livraisons
python t_regles.py             # 12 suites, ~500 000 combinaisons
```

`t_regles.py` (racine du dépôt) reproduit les formules Rust et les
éprouve sur les cas limites : arrondis, TVA, retours, caisse,
numérotation, dette fournisseur, immuabilité. Il a trouvé le
sur-remboursement.

Recette manuelle : `RECETTE.md`, `APPLIQUER_V12_COMPLET.md`.

⚠️ Les commandes Tauri n'ont **aucun** test. C'est le premier endroit où
en ajouter.

---

## Dette technique

**Bon de livraison partiel.** Le suivi gère le partiel (30 sur 100), le
**document** non : la conversion copie toutes les lignes à quantité
pleine. On ne peut pas remettre au client un BL qui dit 30. Il faudrait
choisir les quantités à la création, plus un `ligne_origine_id` sur
`ligne_piece` pour cumuler les BL successifs contre la commande.

**Héritage de livraison BL → facture en bloc.** Reporté seulement si le
BL source est entièrement livré. Un appariement ligne à ligne
demanderait un `ORDER BY` stable sur `lire_lignes_raw`, qui n'en a pas.

**`ModalImpression` fait doublon avec `ApercuPiece`** depuis que
l'aperçu porte le choix du format. À fusionner.

**Paiements fournisseur globaux** répartis du plus ancien au plus
récent — convention usuelle, mais approximation.

**Pièces historiques en `validee`.** Migration possible :
`UPDATE piece_commerciale SET statut='paye' WHERE statut='validee'` —
seulement si toutes sont soldées.

**Codes-barres non dessinés.** On imprime le numéro : des barres sur une
imprimante de bureau ne se scannent pas de façon fiable.

**`lire_fournisseurs_pagines`** construit son WHERE par `format!()`.
Sans risque aujourd'hui (valeurs internes), à passer en paramètres liés.

---

## npm
```bash
npm install xlsx    # SheetJS — export Excel des rapports
```
