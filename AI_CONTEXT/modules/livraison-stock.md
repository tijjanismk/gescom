# Module : livraison, réception, et le moment où le stock bouge

Rôle : faire dire au stock informatique la même chose que les sacs
empilés dans le magasin.

## La règle, en une phrase

**Le stock bouge une fois, au premier document qui constate le
mouvement physique.**

C'est la seule règle. Tout le reste en découle :

| Ce qui se passe | Qui sort le stock |
|---|---|
| Vente au comptoir (POS) | la vente, tout de suite — le client repart avec |
| Pièce sans bon de livraison | la validation de la facture, comme avant |
| Pièce avec bon de livraison | **le bon**, au fur et à mesure des livraisons |
| Achat direct (`enregistrer_achat`) | l'achat, tout de suite |
| Achat avec bon de réception | **le bon**, à la réception |

## Pourquoi ce n'est pas un réglage

`suivi_livraison_actif` ne pilote que **l'écran**. Le comportement, lui,
dépend d'un fait : cette pièce descend-elle d'un bon, oui ou non
([`pieces::stock_confie_a_un_bon`](../../src-tauri/noyau/src/pieces.rs)).

C'est délibéré. Un réglage global aurait fait coexister deux régimes de
stock dans la même base : une pièce créée avant la bascule ne suivrait
pas la même règle que celle d'après, et un stock faux deviendrait
impossible à auditer. En s'appuyant sur la présence d'un bon, la
question ne se pose jamais — et **les pièces déjà en base n'ont pas de
bon, donc rien ne change pour elles**. La bascule ne peut pas fausser un
stock existant.

## Le POS et les pièces parlent bien la même langue

Ce sont deux chemins, une seule règle : *la marchandise sort quand elle
sort*.

- Au comptoir, elle sort à l'instant de la vente. Il n'y a pas de bon,
  il n'y en aura jamais : le client est là, il emporte ses sacs.
- Sur une pièce sans bon, elle sort à la validation de la facture —
  c'est le même geste, la remise au comptoir, avec un papier.
- Avec un bon, elle sort quand le camion part.

**La réalité malienne décide de la porte, pas de la règle.** La plupart
des commerçants visés remettent la marchandise au comptoir : ils
n'ouvriront jamais l'écran de livraison, et pour eux rien ne change.
Les quincailleries — une part importante — livrent : pour elles le bon
devient le document qui compte. Les deux cohabitent dans la même base
sans se contredire.

## Conséquence assumée

Un bon facturé mais pas encore livré **laisse la marchandise en
magasin**. La facture peut être émise et même payée sans qu'un sac ait
bougé.

C'est vrai, et c'est le but. L'écran Pièces montre déjà les deux axes
séparément : `impayé / partiel / payé` d'un côté, `non livré / partiel /
livré` de l'autre.

## Le mouvement doit être vérifiable

Un mouvement qu'on ne peut pas rattacher à son document ne sert à rien :
« il manque 12 sacs » sans savoir d'où ils sont partis, c'est pire que
pas de trace du tout.

L'historique de stock retrouve le numéro du document en joignant sur le
**type** et `operation_id`. Chaque type pointe vers une table
différente :

| type | `operation_id` pointe vers |
|---|---|
| `vente` | `vente` → puis `piece_commerciale` |
| `retour`, `echange` | `retour` |
| `achat`, `retour_fournisseur` | `piece_commerciale` |
| **`livraison`, `reception`** | **`piece_commerciale`** (le bon) |

D'où deux types **distincts** de `vente` et `entree`. Écrire une
livraison en `vente` ferait chercher un identifiant de pièce dans la
table des ventes : la jointure ne trouverait rien, et le mouvement
s'afficherait sans numéro. Le piège est silencieux — le stock serait
juste, la traçabilité perdue.

Les trois lecteurs sont branchés :

- [depots.rs](../../src-tauri/noyau/src/depots.rs) — l'historique
  affiche le numéro du bon ;
- [journal.rs](../../src-tauri/noyau/src/journal.rs) — la journée du
  commerçant les liste, comme les entrées et les ajustements ;
- [fournisseurs.rs](../../src-tauri/noyau/src/fournisseurs.rs) — la
  fiche fournisseur compte la marchandise reçue.

`reception` n'est **pas** un achat facturé
([`est_achat_facture`](../../src-tauri/noyau/src/coeur/stock.rs)), pour
la même raison qu'`entree` (D42) : à la réception, rien n'est encore
facturé. C'est la facture fournisseur qui suit qui porte la dette.

## Le raccourci « bon né entièrement livré »

Émettre un bon, c'est constater que la marchandise part : un bon créé
par conversion naît donc entièrement livré. Ce raccourci passe par
[`livraisons::marquer_entierement_livre`](../../src-tauri/noyau/src/livraisons.rs),
**le même chemin** que la saisie ligne à ligne.

Ce n'est pas de la coquetterie : la première version écrivait
`UPDATE ligne_piece SET quantite_livree = quantite` en bloc, à deux
endroits. Le bon se disait livré sans que rien ne sorte du magasin.

## Règles métier

- [CONFIRMÉ] Le mouvement porte sur l'**écart**, jamais sur le total :
  corriger « 6 livrés » en « 7 livrés » sort une unité, pas sept
  (test `le_stock_bouge_de_l_ecart_pas_du_total`).
- [CONFIRMÉ] Un écart négatif fait rentrer la marchandise — le livreur
  revient avec deux sacs refusés
  (test `corriger_une_livraison_a_la_baisse_rend_la_marchandise`).
- [CONFIRMÉ] On ne livre pas plus que la quantité du bon.
- [CONFIRMÉ] Une commande ou une facture ne constatent aucun mouvement
  physique : y enregistrer une livraison ne bouge rien
  (test `une_commande_ne_bouge_aucun_stock`).
- [CONFIRMÉ] Une facture issue d'un bon ne sort rien
  (test `une_facture_issue_d_un_bon_ne_sort_rien`), une facture sans bon
  sort le stock comme avant
  (test `une_facture_sans_bon_sort_le_stock_comme_avant`).
- [CONFIRMÉ] L'action combinée « commande → BL + facture » ne sort la
  marchandise qu'une fois
  (test `livrer_et_facturer_en_un_geste_ne_sort_qu_une_fois`).
- [CONFIRMÉ] Sans dépôt actif, la livraison est **refusée** plutôt
  qu'enregistrée sans mouvement.
- [CONFIRMÉ] Le stock se compte en unité de **base** : un carton de
  douze sort douze.

## Tests

[noyau/tests/livraison_stock.rs](../../src-tauri/noyau/tests/livraison_stock.rs)
— 14 scénarios. Les deux erreurs qu'ils cherchent sont symétriques :
que la marchandise sorte **deux fois** (une au bon, une à la facture),
ou qu'elle ne sorte **jamais**, parce que chacun croit que l'autre s'en
charge.

## Ce qui reste

- L'écran : `suivi_livraison_actif` gouverne l'affichage, mais l'écran
  de saisie des livraisons n'expose pas encore la réception fournisseur.
- Le miroir fournisseur complet (BRF → FAF, FAF → AVF) — voir la
  discussion en cours, il n'est pas fait.
