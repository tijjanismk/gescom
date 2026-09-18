# Module : commandes — pièces commerciales

Rôle : le cycle documentaire. Devis → commande → BL → facture → vente.
C'est ici que le stock sort et que l'argent entre.

⚠️ [pieces.rs](../../src-tauri/src/commandes/pieces.rs) est le plus gros
fichier du projet (2 143 l.) et le plus chargé en règles. Ne pas le
modifier depuis un résumé — ouvrir le fichier.

## Fichiers

- [commandes/pieces.rs](../../src-tauri/src/commandes/pieces.rs) (2 143 l.)
- [commandes/pieces_pos.rs](../../src-tauri/src/commandes/pieces_pos.rs) (221 l.)
  — facture automatique derrière une vente au comptoir.
- [commandes/livraisons.rs](../../src-tauri/src/commandes/livraisons.rs) (232 l.)
  — suivi de livraison, **informatif seulement** (D49).
- [commandes/impression.rs](../../src-tauri/src/commandes/impression.rs) (103 l.)

## Fonctions clés

- `reserver_numero(conn, type_piece) -> Result<String, String>`
  ([argent.rs](../../src-tauri/noyau/src/argent.rs)), réexporté
  `pub(crate)` par `pieces.rs` — **seul** point de génération de numéro
  de pièce du projet. ⚠️ **Il écrit** : chaque appel consomme un numéro,
  ce n'est pas un aperçu. Voir [numerotation.md](numerotation.md).
- `valider_facture` ([pieces.rs:831](../../src-tauri/src/commandes/pieces.rs#L831))
  et `valider_facture_sur(tx, …)`
  ([pieces.rs:849](../../src-tauri/src/commandes/pieces.rs#L849)) —
  **le seul endroit où le stock sort et l'argent entre.**
- `creer_facture_depuis_vente` / `_sur`
  ([pieces_pos.rs:12](../../src-tauri/src/commandes/pieces_pos.rs#L12))
  — appelée après `creer_vente`, en try/catch (D16).

Le doublet `fn X` / `fn X_sur(tx, …)` est le motif du module : la
variante `_sur` prend une transaction déjà ouverte, pour composer
plusieurs écritures atomiquement. Ajouter une écriture composée impose
d'écrire la variante `_sur`, pas d'appeler la commande publique.

## Les bornes de date côté fournisseur (18/09/2026)

`lire_toutes_pieces_fournisseur[_sur_base]` prend `date_debut` /
`date_fin` (ISO, `>=` / `<=` sur `pc.date_piece`), comme la liste
client ; l'écran envoie `T00:00:00` / `T23:59:59`. Le filtre « Du → au »
est dans la barre de Pièces pour les deux côtés, et c'est la même paire
que les filtres avancés.

## La date se saisit aussi à la création (18/09/2026)

`creer_piece[_sur_base]` et `creer_piece_fournisseur[_sur_base]` prennent
maintenant `date_piece: Option<String>` — la date de l'AFFAIRE, pas
l'échéance de paiement. `None` → aujourd'hui. Validée par
`argent::date_de_la_piece` (même `date_saisie` que la réception et le
règlement, fenêtre de 31 jours) et gardée par `pieces:antidater` côté
serveur (`exiger_antidatage[_base]`, même geste que `modifier_piece`).
Rien d'autre ne bouge avec elle : `creer_piece*` ne touche ni le stock
ni la caisse (ça, c'est `valider_facture` / `enregistrer_achat`). La
façade Tauri (mono-poste) ne vérifie pas le droit — un seul poste, un
seul patron. Écran : `ModalNouvellePiece.tsx`, champ « Date de la
pièce », visible seulement à qui a `pieces:antidater`.

## Le reste d'un avoir fournisseur (18/09/2026)

`coeur::calcul::credit_avoir_fournisseur(statut, montant)` : remboursé
(`paye`) ou annulé → 0, sinon le crédit. Les deux listes fournisseur et
les états de dette (`fournisseurs.rs`) passent par elle — un AVF n'est
jamais un impayé. L'écran lit le reste d'un avoir en ambre (« crédit »).

## Commandes exposées

**pieces.rs** (19) — `lire_toutes_pieces_client`,
`lire_toutes_pieces_fournisseur`, `lire_pieces_client`,
`lire_lignes_piece`, `creer_piece`, `convertir_piece`,
`convertir_commande_en_livraison_et_facture`, `lire_donnees_piece`,
`lire_piece_de_vente`, `lire_vente_de_piece`, `lire_fiche_client`,
`imprimer_piece`, `valider_facture`, `modifier_piece`, `annuler_piece`,
`dupliquer_piece` — plus `creer_piece_fournisseur`,
`changer_statut_piece`, `annuler_facture_par_avoir` *(jamais appelées)*.

**pieces_pos.rs** — `creer_facture_depuis_vente` ; `modifier_facture_pos`
et `valider_facture_credit` *(jamais appelées)*.

**livraisons.rs** — `lire_livraison_piece`, `enregistrer_livraison`.

**impression.rs** — `imprimer_facture`, `ouvrir_avec_systeme`.

## Entrant

- [Pieces.tsx](../../src/pages/Pieces.tsx) (1 661 l.) — l'écran principal
  du module : liste, conversion, validation, annulation, duplication.
- [FicheClient.tsx](../../src/pages/FicheClient.tsx) — `creer_piece`,
  `convertir_piece`, `imprimer_piece`.
- [ModalNouvellePiece.tsx](../../src/components/ModalNouvellePiece.tsx),
  [ApercuPiece.tsx](../../src/components/ApercuPiece.tsx),
  [ModalImpression.tsx](../../src/components/ModalImpression.tsx),
  [ModalLivraison.tsx](../../src/components/ModalLivraison.tsx).
- `imprimer_facture` est le point d'impression **commun** : 13 fichiers
  du front l'appellent, pas seulement les pièces.

Sortant : `coeur::pieces` (les trois gardes d'immuabilité),
`coeur::calcul`, `coeur::stock`, `utils::exiger_session_caisse`
(3 sites), `commandes::ventes` (`EtatApp`), `persistance::journal`.

## Règles métier

- [CONFIRMÉ] **Stock et caisse ne bougent qu'à `valider_facture`.** Tout
  le reste — BL, aperçu, suivi de livraison — est du document ou de
  l'information (CONTEXT.md, D49).
- [CONFIRMÉ] Les trois gardes d'immuabilité (`peut_modifier`,
  `peut_transferer`, `peut_annuler`) vivent dans
  [coeur/pieces.rs](../../src-tauri/noyau/src/coeur/pieces.rs) et sont testées
  là-bas. Ce module les **appelle**, il ne les réimplémente pas — c'est
  à préserver.
- [CONFIRMÉ] Une pièce ne se transfère qu'une fois, gardée par le statut
  **et** par l'existence d'un descendant non annulé. Le second verrou
  existe précisément à cause de
  `convertir_commande_en_livraison_et_facture`, qui produit deux pièces
  d'un coup ([coeur/pieces.rs:68-75](../../src-tauri/noyau/src/coeur/pieces.rs#L68)).
- [CONFIRMÉ] Un préfixe = un type = un compteur : `DEV PRO CMD BL FAC
  ACP AVC` (client), `BCF BRF FAF AVF` (fournisseur), `BTR` (transfert).
  Numérotation par `MAX(substr(numero,-5))`, jamais `COUNT` (D28) — un
  `COUNT` réattribue un numéro après annulation.
- [CONFIRMÉ] `validee` est un statut **historique**. Traité comme clos
  par `est_close`, ne plus jamais l'écrire.
- [DÉDUIT] `annuler_facture_par_avoir` et `valider_facture_credit` sont
  exposées mais injoignables depuis l'interface : soit un chantier en
  cours, soit un chemin abandonné. Vérifier l'intention avant d'y
  toucher — ne pas supposer que le geste est disponible à l'écran.
