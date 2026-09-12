# Module : livraison, réception, et le moment où le stock bouge

Rôle : faire dire au stock informatique la même chose que les sacs
empilés dans le magasin.

## La règle, en une phrase

**Le stock bouge une fois, au premier document qui constate le
mouvement physique.** Le stock lui-même est une conséquence : la somme
des `mouvement_stock`, tenue par le déclencheur `stock_suit_les_mouvements`
sur les deux moteurs — on n'écrit jamais `stock_depot` en direct.

| ce qui se passe | qui sort (ou entre) le stock |
|---|---|
| vente au comptoir | la vente, tout de suite |
| pièce sans bon | la validation de la facture |
| pièce avec bon de livraison / réception | **le bon**, au fur et à mesure |
| achat direct | l'achat, tout de suite |
| entrée sans facture, ajustement, transfert | le mouvement lui-même (pas d'argent) |

Ce n'est **pas un réglage** : `suivi_livraison_actif` ne pilote que
l'écran. Le comportement dépend d'un fait — cette pièce descend-elle
d'un bon (`pieces::stock_confie_a_un_bon` / `_sur`) — donc les pièces
déjà en base, qui n'ont pas de bon, ne changent pas. Conséquence
assumée : un bon facturé et payé mais pas livré laisse la marchandise
en magasin ; l'écran Pièces montre les deux axes (paiement, livraison).

## Fichiers

| | |
|---|---|
| [noyau/src/livraisons.rs](../../src-tauri/noyau/src/livraisons.rs) | `etat(livree, commandee)`, `enregistrer_livraison` (ligne à ligne), `marquer_entierement_livre` — chacune en `_sur` aussi |
| [noyau/src/coeur/stock.rs](../../src-tauri/noyau/src/coeur/stock.rs) | les types de mouvement, `libelle`, `est_achat_facture` |
| [noyau/src/amorcage.rs](../../src-tauri/noyau/src/amorcage.rs) `trigger_stock` | le déclencheur, SQLite et PostgreSQL |

## Le mouvement doit être vérifiable

`operation_id` pointe vers une table différente **selon le type** —
c'est ainsi que l'historique retrouve le numéro du document :

| type | `operation_id` → |
|---|---|
| `vente` | `vente` → `piece_commerciale` |
| `retour`, `echange` | `retour` |
| `achat`, `retour_fournisseur` | `piece_commerciale` |
| `livraison`, `reception` | `piece_commerciale` (le bon) |
| `transfert` | le bon `BTR-…` dans `motif` |

D'où des types **distincts** (`vente` ≠ `livraison`, `achat` ≠ `entree` ≠
`reception`) : le mauvais type garde un stock juste mais une
traçabilité perdue, sans erreur. Lecteurs branchés : `depots.rs`
(historique), `journal.rs` (la journée), `fournisseurs.rs` (marchandise
reçue).

## Règles

- [CONFIRMÉ] Le mouvement porte sur l'**écart**, jamais le total : « 6 → 7 livrés » sort une unité ; un écart négatif fait rentrer (`le_stock_bouge_de_l_ecart_pas_du_total`, `corriger_une_livraison_a_la_baisse_rend_la_marchandise`).
- [CONFIRMÉ] On ne livre pas plus que commandé (plafonné) ; en unité de **base** — un carton de douze sort douze.
- [CONFIRMÉ] Une commande ou une facture ne constatent aucun mouvement ; une facture issue d'un bon ne sort rien, une facture sans bon sort comme avant (`une_facture_issue_d_un_bon_ne_sort_rien`, `une_facture_sans_bon_sort_le_stock_comme_avant`).
- [CONFIRMÉ] « Commande → BL + facture » ne sort la marchandise qu'une fois ; un bon créé par conversion naît **entièrement livré par le même chemin** que la saisie (`marquer_entierement_livre`) — la première version le disait livré sans rien sortir.
- [CONFIRMÉ] Sans dépôt actif, la livraison est **refusée**, pas enregistrée sans mouvement.
- [CONFIRMÉ] `reception` n'est pas un achat facturé (`est_achat_facture`) : la dette vient de la FAF qui suit (D42).

## Tests

[tests/livraison_stock.rs](../../src-tauri/noyau/tests/livraison_stock.rs)
(14), [stock_mouvements.rs](../../src-tauri/noyau/tests/stock_mouvements.rs) ;
sur `Base` : [pieces_base.rs](../../src-tauri/noyau/tests/pieces_base.rs),
[listes_base.rs](../../src-tauri/noyau/tests/listes_base.rs). Les deux
erreurs cherchées : sortir **deux fois**, ou **jamais** parce que chacun
croit que l'autre s'en charge.

## L'écran

Avec `suivi_livraison_actif`, « Livraison » / « Réception » apparaît
**sur les bons seulement** — un bouton qui ment est pire que pas de
bouton.
