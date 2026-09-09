# Règles métier

Les décisions numérotées (D1…D51) vivent dans [CONTEXT.md](../CONTEXT.md)
et ne sont pas recopiées ici. Ce fichier n'indexe que les règles
**adossées à un fichier:ligne** qu'on peut rouvrir, plus les déductions
qu'il reste à vérifier.

Convention : `[CONFIRMÉ]` = lu dans le code, référence cliquable.
`[DÉDUIT]` = probable, non vérifié — à confirmer avant de s'en servir.

---

## Argent — jamais de flottant

- [CONFIRMÉ] Tous les montants sont des `i64` en FCFA.
  `montant_ligne = (prix × qté).round()` —
  [calcul.rs:5](../src-tauri/src/coeur/calcul.rs#L5).
- [CONFIRMÉ] **Seuil de solde = 5 F.** Un reste dû ≤ 5 F n'est plus
  exigible, mais **seulement si un encaissement a eu lieu** : sans
  paiement, une vente de 3 F reste une créance —
  [calcul.rs:127-142](../src-tauri/src/coeur/calcul.rs#L127).
  Le seuil ferme un statut ; il ne crée aucun paiement et ne touche pas
  la caisse.
- [CONFIRMÉ] `statut_vente(total, montant_payé)` prend le **payé**, pas
  le reste. La variante `statut_vente_depuis_reste` existe pour éviter
  l'inversion d'argument —
  [calcul.rs:155-168](../src-tauri/src/coeur/calcul.rs#L155).
- [CONFIRMÉ] Un règlement global s'impute **de la plus ancienne facture à
  la plus récente**, les factures soldées sont sautées, et le surplus
  revient en `(None, montant)` — une avance, pas une perte —
  [calcul.rs:43-70](../src-tauri/src/coeur/calcul.rs#L43).
  La répartition est **écrite** en base, jamais recalculée à la lecture :
  un paiement sans `piece_id` n'appartient à aucune facture, et deux
  écrans donnaient alors deux vérités.

## Annulation d'un règlement

- [CONFIRMÉ] Quatre cas, tranchés par
  `effet_caisse_annulation(remboursement, dans_session_ouverte, touche_la_caisse)` —
  [calcul.rs:107-122](../src-tauri/src/coeur/calcul.rs#L107) :
  - réglé par avoir → **aucun** mouvement (aucun franc n'a bougé) ;
  - remboursement → **contre-passation**, quelle que soit la session ;
  - erreur de saisie, session ouverte → **contre-passation** ;
  - erreur de saisie, session déjà close → **aucun** (la clôture a déjà
    absorbé la ligne fantôme ; poster aujourd'hui créerait un faux écart).
- [CONFIRMÉ] Le sens (entrée/sortie) est décidé par l'appelant :
  l'énumération ne dit que « contre-passer ou non », pour que le côté
  fournisseur n'ait pas à lire `Sortie` en écrivant une entrée.
- [CONFIRMÉ] Un règlement contesté ne se supprime pas : on écrit une
  seconde ligne négative liée par `paiement.annule_paiement_id` (et son
  symétrique `paiement_fournisseur.annule_paiement_id`) —
  [persistance/mod.rs](../src-tauri/src/persistance/mod.rs).

## Immuabilité des pièces (OHADA / SYSCOHADA)

- [CONFIRMÉ] C'est l'**émission** qui fige une pièce, pas le paiement —
  [pieces.rs:1-13](../src-tauri/src/coeur/pieces.rs#L1).
- [CONFIRMÉ] Engageantes (immuables dès `emis`) : `facture`,
  `facture_acompte`, `avoir_client`, `facture_fournisseur`,
  `avoir_fournisseur`. Tout le reste (devis, proforma, commande, BL, BRF)
  reste modifiable — [pieces.rs:22-31](../src-tauri/src/coeur/pieces.rs#L22).
- [CONFIRMÉ] Statuts clos pour **tous** les types : `validee`,
  `transfere`, `annule`, `paye` —
  [pieces.rs:34-36](../src-tauri/src/coeur/pieces.rs#L34).
  `validee` est un statut historique : traité comme clos, ne plus l'écrire.
- [CONFIRMÉ] **Une pièce ne se transfère qu'une fois**, et c'est gardé par
  *deux* verrous indépendants — le statut `transfere` **et** l'existence
  d'un descendant non annulé —
  [pieces.rs:80-109](../src-tauri/src/coeur/pieces.rs#L80).
  Le second rattrape le cas commande → BL + facture d'un coup, où seule
  la commande passait en `transfere` : le BL restait refacturable.
- [CONFIRMÉ] Un descendant **annulé** libère la source — l'appelant ne
  passe que les descendants vivants
  ([pieces.rs:252](../src-tauri/src/coeur/pieces.rs#L252)).
- [CONFIRMÉ] Une pièce ayant produit des effets (vente, paiement,
  mouvement de stock) ne s'annule pas : il faut un avoir —
  [pieces.rs:142-148](../src-tauri/src/coeur/pieces.rs#L142). Une pièce
  émise **sans** effet s'annule, et le numéro reste consommé (pas de trou
  dans la série).
- [CONFIRMÉ] Tout message de refus nomme la sortie (« émettre un avoir »,
  « annuler d'abord FAC-… ») — c'est testé
  ([pieces.rs:266](../src-tauri/src/coeur/pieces.rs#L266)).

## Stock

- [CONFIRMÉ] Les 8 types de mouvement sont déclarés **uniquement** dans
  [stock.rs:17-39](../src-tauri/src/coeur/stock.rs#L17) : `achat`,
  `entree`, `vente`, `retour`, `retour_fournisseur`, `echange`,
  `ajustement`, `transfert`. La colonne est du TEXT libre, rien en base
  ne contraint la valeur — en ajouter ailleurs désynchronise le journal.
- [CONFIRMÉ] `achat` ≠ `entree` : seul `achat` est un achat **facturé**
  (dette + caisse). Les confondre gonflait les achats du jour d'un
  montant que personne ne doit —
  [stock.rs:58-60](../src-tauri/src/coeur/stock.rs#L58).
- [CONFIRMÉ] `ajustement` et `transfert` n'ont **pas** de sens fixe :
  `est_entrant` renvoie `None`, le signe de `quantite_delta` tranche —
  [stock.rs:46-52](../src-tauri/src/coeur/stock.rs#L46).

## Caisse

- [CONFIRMÉ] Le rapprochement ne porte que sur les **espèces**. Orange
  Money et Moov Money sont tracés en mouvement de caisse mais ne sont pas
  dans le tiroir : l'appelant doit filtrer `moyen = 'especes'` avant de
  sommer — [caisse.rs:1-11](../src-tauri/src/coeur/caisse.rs#L1).
- [CONFIRMÉ] `solde_theorique = fond + entrées − sorties` ;
  `ecart = compté − théorique`, négatif = manque —
  [caisse.rs:9-17](../src-tauri/src/coeur/caisse.rs#L9).
- [CONFIRMÉ] **Sans session ouverte, toute opération d'argent est
  refusée** (`CAISSE_FERMEE`) — le refus vaut mieux que l'écriture
  manquante : une opération bloquée se voit, une écriture absente
  jamais — [utils.rs:33-44](../src-tauri/src/utils.rs#L33).

## Client générique

- [CONFIRMÉ] Le client de passage n'a pas d'identité, donc ni crédit ni
  avoir. Le prédicat est dupliqué **volontairement** entre
  [utils.rs:13](../src-tauri/src/utils.rs#L13) et `lire_client_generique`
  (`commandes/ventes.rs`) — si l'un change, vérifier l'autre.

## Codes-barres

- [CONFIRMÉ] Préfixe interne `20`, jamais attribué à un pays, donc sans
  collision possible avec un code du commerce —
  [codebarre.rs:18](../src-tauri/src/coeur/codebarre.rs#L18).
- [CONFIRMÉ] Un article qui a déjà un code fabricant garde le sien ; on
  ne génère que pour ceux qui n'en ont pas —
  [codebarre.rs:14](../src-tauri/src/coeur/codebarre.rs#L14).

## Santé de la base

- [CONFIRMÉ] `quick_check` (pas `integrity_check` : dix fois plus rapide,
  suffit pour une base tronquée après coupure de courant) **plus** cinq
  contrôles de cohérence métier — ventes sans ligne, paiement sans vente,
  ligne sans pièce, mouvement de caisse hors session, stock négatif —
  [persistance/mod.rs:22-62](../src-tauri/src/persistance/mod.rs#L22).
  La vérification ne répare **rien** : elle dit de restaurer.
- [CONFIRMÉ] `entretenir()` = `VACUUM INTO` (copie cohérente, WAL inclus)
  puis `REINDEX; VACUUM;`. Le chemin est passé en **paramètre lié** — une
  apostrophe dans un nom d'utilisateur Windows cassait la requête.

---

## À vérifier

- [DÉDUIT] Les cinq modules de `coeur/` semblent purs (aucun `use
  rusqlite`, aucune I/O) et donc testables isolément. Non recompté.
- [DÉDUIT] `paiement_fournisseur.piece_id` à NULL identifie les
  règlements globaux **antérieurs** à la répartition écrite ; les écrans
  qui somment par `piece_id` les ignoreraient donc silencieusement.
- [DÉDUIT] `avoir.piece_id` à NULL sur les avoirs antérieurs à D44 fait
  afficher 0 en reste — dégradé mais juste, d'après le commentaire de
  migration. Non vérifié à l'écran.
