# Module : commandes — achat et fournisseur

Rôle : le côté sortant de l'argent. Achats facturés, entrées de stock,
dettes fournisseurs, TVA, créances irrécouvrables.

## Fichiers

- [commandes/achats.rs](../../src-tauri/src/commandes/achats.rs) (607 l.)
- [commandes/fournisseurs.rs](../../src-tauri/src/commandes/fournisseurs.rs)
  (774 l.) — ⚠️ **contient tout le fournisseur** (D12) : tiers, dettes,
  entrées de stock, ajustements d'inventaire. Ne pas chercher ailleurs.
- [commandes/chantiers.rs](../../src-tauri/src/commandes/chantiers.rs)
  (760 l.) — ⚠️ nom trompeur : ce n'est pas du BTP. Fourre-tout des
  « chantiers » de développement — TVA, dettes fournisseurs,
  irrécouvrable, expiration des avoirs. **Ne pas écraser** (CONTEXT.md).

## Commandes exposées

**achats.rs** —
`enregistrer_achat` ([achats.rs:39](../../src-tauri/src/commandes/achats.rs#L39)),
`enregistrer_retour_fournisseur`,
`lire_factures_fournisseur_retournables`.

**fournisseurs.rs** — `lire_fournisseurs`,
`lire_fournisseurs_avec_dettes`, `creer_fournisseur`,
`modifier_fournisseur`, `lire_etat_dette_fournisseur`,
`annuler_paiement_fournisseur`, `lire_etat_dettes_global`,
`enregistrer_entree_stock`, `enregistrer_ajustement_inventaire`,
`lire_fournisseur_detail`, `lire_fiche_fournisseur`.

**chantiers.rs** — `lire_taux_tva`, `sauvegarder_tva_article`,
`lire_resume_tva`, `lire_dettes_fournisseurs`,
`regler_dette_fournisseur` ([chantiers.rs:147](../../src-tauri/src/commandes/chantiers.rs#L147)),
`marquer_irrecouvrable`, `lire_irrecouvrable`, `lire_config_avoirs`,
`sauvegarder_config_avoirs`, `expirer_avoirs`, `reactiver_avoir`,
`lire_avoirs_expires`, `lire_factures_fournisseur_ouvertes` *(jamais appelée)*.

## Entrant

| Écran | Ce qu'il appelle |
|---|---|
| [Achats.tsx](../../src/pages/Achats.tsx) | `enregistrer_achat`, `creer_fournisseur` |
| [Fournisseurs.tsx](../../src/pages/Fournisseurs.tsx) | listes paginées, dettes, `regler_dette_fournisseur` |
| [FicheFournisseur.tsx](../../src/pages/FicheFournisseur.tsx) | fiche, dette, `annuler_paiement_fournisseur` |
| [Stock.tsx](../../src/pages/Stock.tsx) | `enregistrer_entree_stock`, `enregistrer_ajustement_inventaire` |
| [RetourFournisseur.tsx](../../src/components/RetourFournisseur.tsx) | retour, factures retournables |
| [OngletChantiers.tsx](../../src/components/OngletChantiers.tsx) | TVA, dettes, irrécouvrable, avoirs expirés |

Sortant : `coeur::calcul::repartir_reglement` (imputation d'un règlement
global), `coeur::stock` (types de mouvement),
`utils::exiger_session_caisse` (4 sites dans `achats.rs`),
`persistance::journal`.

## Règles métier

- [CONFIRMÉ] `enregistrer_achat` exige la caisse ouverte avant tout
  ([achats.rs:105](../../src-tauri/src/commandes/achats.rs#L105)), puis
  la ré-exige à l'intérieur de la transaction
  ([achats.rs:282](../../src-tauri/src/commandes/achats.rs#L282)) — le
  commentaire dit que le second appel ne peut plus échouer, il sert à
  récupérer l'id.
- [CONFIRMÉ] `achat` (facturé, crée dette + caisse) et `entree`
  (marchandise sans facture) sont deux types **distincts** de mouvement.
  Les confondre gonflait les achats du jour d'un montant que personne ne
  doit ([coeur/stock.rs:58](../../src-tauri/src/coeur/stock.rs#L58), D42).
- [CONFIRMÉ] Un règlement fournisseur global s'impute sur les factures de
  la plus ancienne à la plus récente, et la répartition est **écrite** —
  c'est la raison d'être de `repartir_reglement`
  ([coeur/calcul.rs:28-42](../../src-tauri/src/coeur/calcul.rs#L28)) :
  auparavant elle n'était calculée qu'en mémoire, et deux écrans
  donnaient deux vérités sur la même facture.
- [CONFIRMÉ] `annuler_paiement_fournisseur` est le symétrique exact de
  `annuler_reglement` côté client, et partage la même règle d'effet
  caisse ([fournisseurs.rs:698](../../src-tauri/src/commandes/fournisseurs.rs#L698)).
- [DÉDUIT] Dette fournisseur = `SUM(FAF) − SUM(AVF non payés) − paiements`
  (D9), lue dans `paiement_fournisseur`, jamais dans le `statut` seul
  (D36). Non revérifié requête par requête.
- [DÉDUIT] `BRF → FAF` est interdit par copie ; la facture fournisseur ne
  naît que de `enregistrer_achat` (D37). À vérifier dans
  `pieces.rs::convertir_piece` avant de s'y fier.
