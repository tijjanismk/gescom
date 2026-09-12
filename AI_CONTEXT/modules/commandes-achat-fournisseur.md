# Module : commandes — achat et fournisseur

Rôle : le côté sortant de l'argent. Achats facturés, entrées de stock,
dettes fournisseurs, TVA, créances irrécouvrables.

## Fichiers (la logique est dans `noyau/`, les `commandes/*.rs` sont des façades)

| | contient |
|---|---|
| [noyau/src/achats.rs](../../src-tauri/noyau/src/achats.rs) | achat, retour fournisseur, factures retournables, **valider / annuler une facture fournisseur** (le miroir de `valider_facture`) |
| [noyau/src/fournisseurs.rs](../../src-tauri/noyau/src/fournisseurs.rs) | ⚠️ **tout le fournisseur** (D12) : tiers, états de dette, entrée sans facture, retour sans facture, ajustement d'inventaire, annulation de paiement |
| [noyau/src/chantiers.rs](../../src-tauri/noyau/src/chantiers.rs) | ⚠️ nom trompeur, pas du BTP : TVA, dettes, irrécouvrable, expiration des avoirs |
| [noyau/src/argent.rs](../../src-tauri/noyau/src/argent.rs) | `regler_dette_fournisseur`, l'imputation et la réallocation des règlements globaux |

Chaque commande existe en `fn(conn)` et `fn_sur_base(base)`.

## Entrant

| écran | appelle |
|---|---|
| [Achats.tsx](../../src/pages/Achats.tsx) | `enregistrer_achat`, `creer_fournisseur` |
| [Fournisseurs.tsx](../../src/pages/Fournisseurs.tsx), [FicheFournisseur.tsx](../../src/pages/FicheFournisseur.tsx) | listes, dettes, fiche, `regler_dette_fournisseur`, `annuler_paiement_fournisseur` |
| [Stock.tsx](../../src/pages/Stock.tsx) | `enregistrer_entree_stock`, `enregistrer_ajustement_inventaire` |
| [RetourFournisseur.tsx](../../src/components/RetourFournisseur.tsx), [RetourSansFacture.tsx](../../src/components/RetourSansFacture.tsx) | retour sur facture (avec reliquat), retour sans facture |
| [OngletChantiers.tsx](../../src/components/OngletChantiers.tsx) | TVA, dettes, irrécouvrable, avoirs expirés |
| Pièces | `convertir_piece` BCF → BRF → FAF, `valider_facture_fournisseur`, `annuler_facture_fournisseur_par_avoir` |

Sortant : `coeur::calcul::repartir_reglement`, `coeur::stock`,
`caisses::exiger_sur` (`CAISSE_FERMEE`).

## Règles

- [CONFIRMÉ] **Dette = FAF − AVF non remboursés − paiements**, lue dans `paiement_fournisseur`, jamais dans un statut (D9, D36) ; par facture puis additionnée (D41, seuil 5 F) — `fournisseurs.rs`.
- [CONFIRMÉ] Un règlement global s'impute de la plus ancienne facture à la plus récente et la répartition est **écrite**, une ligne de paiement par facture — [calcul.rs:28](../../src-tauri/noyau/src/coeur/calcul.rs#L28), `argent::regler_dette_fournisseur`. Avant, calculée en mémoire, deux écrans donnaient deux vérités.
- [CONFIRMÉ] `achat` (facturé : dette + caisse) et `entree` (sans facture : ni l'un ni l'autre) sont deux types de mouvement distincts — [stock.rs:58](../../src-tauri/noyau/src/coeur/stock.rs#L58), D42.
- [CONFIRMÉ] Caisse ouverte exigée **avant d'écrire** dès qu'un franc sort (comptant, acompte) — D46.
- [CONFIRMÉ] Un bon de réception ne se facture qu'une fois ; la FAF issue d'un BRF ne fait entrer **que l'argent**, le stock est entré au bon (`stock_confie_a_un_bon`) — [livraison-stock.md](livraison-stock.md).
- [CONFIRMÉ] Retour fournisseur : sort du **dépôt de la facture d'achat** (paramètre → `piece_origine_id` → défaut, D43) ; **refusé au-delà du stock**, quantités cumulées par article (D32) ; reliquat **par facture**. `lire_factures_fournisseur_retournables` rend `retournable = min(reliquat, stock)`.
- [CONFIRMÉ] Rendre une marchandise entrée sans facture ne crée **aucun avoir** : simple sortie de stock, plafonnée au stock présent, tracée au journal — `enregistrer_retour_sans_facture`.
- [CONFIRMÉ] Un AVF « remboursement » naît `paye` : il ne réduit pas la dette en plus de l'entrée de caisse.
- [CONFIRMÉ] `annuler_paiement_fournisseur` : contre-passation (jamais de suppression), l'argent **rentre** si le fournisseur rembourse ; virement et chèque ne touchent pas le tiroir — symétrique de `annuler_reglement`.
- [CONFIRMÉ] Annuler une FAF validée = un retour intégral (`annuler_facture_fournisseur_par_avoir`) ; un brouillon n'a rien à annuler.

## Tests

`tests_multi_depot` (lib) — dépôt du retour, découvert, cumul ;
[miroir_fournisseur.rs](../../src-tauri/noyau/tests/miroir_fournisseur.rs) ;
sur `Base` : [achats_base.rs](../../src-tauri/noyau/tests/achats_base.rs),
[fournisseurs_base.rs](../../src-tauri/noyau/tests/fournisseurs_base.rs),
[gestion_base.rs](../../src-tauri/noyau/tests/gestion_base.rs) (chantiers).
