# Module : coeur

Rôle : les règles métier **pures** — aucune I/O, aucun état, aucune
connexion. Tout ce qui est ici est testable seul, et l'est.

C'est la seule partie du projet qu'on peut modifier avec un filet.
Une règle qui n'est pas ici est une règle qu'aucun test ne protège.

## Fichiers

- [coeur/mod.rs](../../src-tauri/src/coeur/mod.rs) (6 l.) — déclare les
  cinq modules. `#![allow(dead_code)]` : certaines fonctions n'ont pas
  encore d'appelant.
- [coeur/calcul.rs](../../src-tauri/src/coeur/calcul.rs) (362 l.) —
  montants, statuts de vente, imputation d'un règlement, effet caisse
  d'une annulation.
- [coeur/pieces.rs](../../src-tauri/src/coeur/pieces.rs) (274 l.) —
  immuabilité OHADA : modifier / transférer / annuler.
- [coeur/stock.rs](../../src-tauri/src/coeur/stock.rs) (145 l.) — les 8
  types de mouvement, source unique. Marges, découvert.
- [coeur/caisse.rs](../../src-tauri/src/coeur/caisse.rs) (52 l.) — solde
  théorique et écart, espèces seulement.
- [coeur/codebarre.rs](../../src-tauri/src/coeur/codebarre.rs) (127 l.) —
  EAN-13 : clé de contrôle, génération interne préfixe `20`.

## Fonctions exposées

### calcul.rs
- `montant_ligne(prix: i64, qté: f64) -> i64` — arrondi au franc.
- `total_vente(&[i64]) -> i64`, `total_paye(&[i64]) -> i64`,
  `reste_du(total, &paiements) -> i64` — peut être négatif (surpaiement).
- `repartir_reglement<T>(montant, &[(T, dû)]) -> Vec<(Option<T>, i64)>` —
  ancien → récent, saute les factures soldées, surplus en `None`.
  **Invariant testé : la somme des parts égale toujours le montant.**
- `SEUIL_SOLDE: i64 = 5`
- `reste_exigible(total, payé) -> i64` — absorbe les résidus d'arrondi,
  mais seulement si `payé > 0`.
- `statut_vente(total, payé) -> StatutVente` — ⚠️ le 2ᵉ argument est le
  **payé**. `statut_vente_depuis_reste(total, reste)` pour l'autre sens.
- `effet_caisse_annulation(remboursement, session_ouverte, touche_caisse) -> EffetCaisse`
  — `ContrePassation` ou `Aucun`. Le **sens** du mouvement est laissé à
  l'appelant, exprès (symétrie client / fournisseur).
- `ecart_prix(référence, pratiqué) -> i64` — positif = remise.

### pieces.rs
- `est_engageante(type) -> bool` — facture, facture_acompte, avoir_client,
  facture_fournisseur, avoir_fournisseur.
- `peut_modifier(type, statut) -> Result<(), String>`
- `peut_transferer(statut_src, descendant: Option<&str>) -> Result<(), String>`
  — deux verrous, cf. [DOMAINE.md](../DOMAINE.md).
- `peut_annuler(type, statut, a_produit_effets) -> Result<(), String>`

Les `Err` portent un message **destiné à l'utilisateur**, qui nomme
toujours la sortie. Ne pas les remplacer par des codes.

### stock.rs
- Constantes `ACHAT`, `ENTREE`, `VENTE`, `RETOUR`, `RETOUR_FOURNISSEUR`,
  `ECHANGE`, `AJUSTEMENT`, `TRANSFERT` ; `TOUS: [&str; 8]`.
- `est_entrant(type) -> Option<bool>` — `None` pour ajustement/transfert.
- `est_achat_facture(type) -> bool` — vrai pour `achat` seul.
- `libelle(type) -> &'static str`
- `est_a_decouvert(stock, demandé)`, `marge(pv, pa, qté)`,
  `credit_retour(prix, qté)`.

### caisse.rs
- `solde_theorique(fond, entrées, sorties) -> i64`
- `ecart_caisse(compté, théorique) -> i64`

### codebarre.rs
- `PREFIXE_INTERNE = "20"`
- `cle_ean13(douze: &str) -> Option<u8>`
- `generer_ean13_interne(sequence: u64) -> Option<String>` — séquence
  jusqu'à 9 999 999 999.

## Entrant

Importé par `commandes/*` — surtout `pieces.rs` (immuabilité),
`ventes.rs` et `creances.rs` (montants), `caisse.rs`, `transferts.rs`.
`calcul.rs` est touché par ~20 fichiers, `pieces.rs` par ~13.

Rien dans `coeur/` n'importe quoi que ce soit du projet : c'est la
feuille du graphe, et ça doit le rester. Y introduire un
`rusqlite::Connection` rendrait tout le module non testable.

## Règles métier

Toutes détaillées avec leurs `fichier:ligne` dans
[DOMAINE.md](../DOMAINE.md). Résumé des pièges :

- [CONFIRMÉ] `statut_vente` prend le **payé**, pas le reste — l'inversion
  d'argument est le piège du module, d'où la variante nommée.
- [CONFIRMÉ] Le seuil de 5 F ne mord que si un encaissement a eu lieu.
- [CONFIRMÉ] `est_entrant` renvoie `None` sur ajustement et transfert :
  forcer une réponse ferait mentir l'appelant une fois sur deux.
- [CONFIRMÉ] `effet_caisse_annulation` ne dit **pas** le sens du
  mouvement. Un appelant qui lit `ContrePassation` doit décider
  lui-même s'il écrit une entrée ou une sortie.
