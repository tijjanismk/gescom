# Module : coeur

Rôle : les règles métier **pures** — aucune I/O, aucun état, aucune
connexion. Tout ce qui est ici est testable seul, et l'est.

C'est la seule partie du projet qu'on peut modifier avec un filet.
Une règle qui n'est pas ici est une règle qu'aucun test ne protège.

## Fichiers

- [coeur/mod.rs](../../src-tauri/noyau/src/coeur/mod.rs) (6 l.) — déclare les
  six modules. `#![allow(dead_code)]` : certaines fonctions n'ont pas
  encore d'appelant.
- [coeur/calcul.rs](../../src-tauri/noyau/src/coeur/calcul.rs) (362 l.) —
  montants, statuts de vente, imputation d'un règlement, effet caisse
  d'une annulation.
- [coeur/pieces.rs](../../src-tauri/noyau/src/coeur/pieces.rs) (274 l.) —
  immuabilité OHADA : modifier / transférer / annuler.
- [coeur/stock.rs](../../src-tauri/noyau/src/coeur/stock.rs) (145 l.) — les 8
  types de mouvement, source unique. Marges, découvert.
- [coeur/caisse.rs](../../src-tauri/noyau/src/coeur/caisse.rs) (52 l.) — solde
  théorique et écart, espèces seulement.
- [coeur/saisie.rs](../../src-tauri/noyau/src/coeur/saisie.rs) (130 l.) —
  la saisie jugée au serveur (D27) : modes de paiement, montant,
  quantité, prix, remise.
- [coeur/codebarre.rs](../../src-tauri/noyau/src/coeur/codebarre.rs) (127 l.) —
  EAN-13 : clé de contrôle, génération interne préfixe `20`.
- [coeur/dates.rs](../../src-tauri/noyau/src/coeur/dates.rs) (124 l.) — la
  date d'une affaire saisie après coup : jamais dans le futur, pas plus
  loin que `RECUL_MAX_JOURS` (31). `est_antidatee` dit si la permission
  `pieces:antidater` est requise. `aujourd_hui` est passé en paramètre :
  la règle se teste sans attendre minuit. **Ce module ne touche pas au
  mouvement de caisse** — deux dates, deux faits (ALERTES.md).
- [coeur/horloge.rs](../../src-tauri/noyau/src/coeur/horloge.rs) (27/10/2026,
  lot C.5) — une horloge qui recule : comparer l'heure de la machine à
  la dernière écriture réussie, tolérance 5 min. Voir plus bas.
- [coeur/lecture.rs](../../src-tauri/noyau/src/coeur/lecture.rs) — les
  permissions de lecture (C-1), commande par commande ; détail dans
  [permissions.md](permissions.md). Depuis le 27/10 (lot C.7),
  `lire_lignes_piece` masque les prix d'une pièce **fournisseur**
  (`lignes_fournisseur` lit le `tiers_type` de chaque ligne).

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
- `residu_a_solder(total, payé) -> i64` (27/10, lot B.4) — le résidu
  de 1 à `SEUIL_SOLDE` F à solder au moment du règlement, 0 sinon (ni
  soldé, ni trop-perçu, ni vente sans encaissement). Appelée par
  `creances::solder_residu[_sur]` et `solder_residus_creances`.
- `encaisse(total_payé, éteint_par_retour) -> i64` (lot B.3) — l'argent
  vraiment versé : le payé sans la dette éteinte par un retour
  (paiement `mode = 'retour'`). `total_paye` garde cette part (il fait
  le reste dû) ; tout « payé », « acompte reçu », « encaissé » affiché
  passe par ici.
- `ca_net(brut, retours) -> i64` et `RETOUR_HORS_CA = "echange"` (lot
  B.2) — le chiffre d'affaires d'une période = ventes de la période −
  retours **datés** dans la période (usage d'un avoir : un mois clos ne
  bouge plus) ; peut être négatif. L'échange ne se déduit pas (il ne
  rend pas la vente, le remplacement n'est pas une `ligne_vente`) ;
  limite : sa différence (reliquat/complément) ne bouge pas le chiffre.
  Le SQL lie la constante en paramètre, ne la recopie pas.
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
- `verifier_comptages(magasins, comptages)` (D36) — reprendre le suivi.
- `lire_suivi_csv(valeur) -> Result<Option<bool>, String>` (lot D.11,
  27/10/2026) — la colonne « Suivre le stock » de l'import CSV : vide →
  `None` (l'import décide), `oui/o/1/vrai` et `non/n/0/faux` sans casse
  (VRAI/FAUX : ce qu'Excel français écrit pour une case), toute autre
  valeur refusée avec un message — jamais devinée. Appelée par les deux
  versions de `catalogue_csv::importer_articles_csv`.

### tiers.rs
- `code_client_suivant`, `FOURNISSEUR_DIVERS`,
  `verifier_achat_sans_fournisseur`, `refuser_si_fournisseur_divers`,
  `verifier_retour_fournisseur_divers`.
- `FicheARattacher { id, est_generique, passage, actif }` et
  `juger_rattachement(passage: Option<&…>, cible: Option<&…>)` (lot D.10,
  27/10/2026) — « Rattacher à un client » : source = fiche de passage
  active du dossier (`None` : pas dans ce dossier) ; cible = client
  ordinaire actif du dossier, ni « Comptant », ni fiche de passage, ni
  elle-même. Appelée par `passage::rattacher_sur`, fiches lues dans la
  transaction.

### caisse.rs
- `solde_theorique(fond, entrées, sorties) -> i64`
- `ecart_caisse(compté, théorique) -> i64`

### saisie.rs (23/09/2026, D27)
Ce que le serveur refuse d'une saisie, quel que soit l'écran.
- `MODES_ENCAISSEMENT` = `especes`, `orange_money`, `moov_money`,
  `cheque`, `virement` ; `verifier_mode_encaissement(mode)` — **`avoir`
  refusé** : un avoir se consomme, il ne se déclare pas.
- `verifier_montant(i64)` — strictement positif.
- `verifier_ligne(quantité, facteur, prix)` — quantité et facteur > 0
  et finis, prix ≥ 0 (0 = cadeau, accepté). Le sens du stock est porté
  par le geste, jamais par le signe.
- `verifier_remise_pct(f64)` — 0 à 100.

Appelé par `creer_vente*`, `regler_creance*` (donc `enregistrer_paiement*`),
`regler_dette_fournisseur*`, `enregistrer_achat*`, `creer_piece*`,
`creer_piece_fournisseur*`, `modifier_piece*`.

### codebarre.rs
- `PREFIXE_INTERNE = "20"`
- `cle_ean13(douze: &str) -> Option<u8>`
- `generer_ean13_interne(sequence: u64) -> Option<String>` — séquence
  jusqu'à 9 999 999 999.

### horloge.rs (27/10/2026, lot C.5)
Les heures sont **passées**, jamais lues sur l'horloge.
- `TOLERANCE_RECUL_SECONDES = 300`, `CODE_REFUS = "HORLOGE_RECULE"`,
  `EXEMPTEES = ["confirmer_horloge"]` (le seul geste qui lève la garde).
- `lire(iso)` (avec ou sans millisecondes, `T` ou espace), `en_clair`
  (« le 27/10/2026 à 14:05 »), `recul_secondes(reference, maintenant)`.
- `juger(reference, maintenant)` — sans référence : oui ; référence
  illisible : oui (remplacée à la prochaine écriture) ; recul > 5 min :
  non, `message_refus` (quoi faire + « le patron le confirme dans
  Paramètres → Horloge ») ; 5 min pile : oui ; heure de la machine
  illisible : non.
- `nouvelle_reference(reference, maintenant)` — la plus récente des
  deux, `None` si rien ne change : la référence **ne recule jamais**
  (sinon 4 min de recul par écriture feraient une heure).
- `confirmation_permise` — le geste du patron n'est permis **que**
  quand l'écriture est bloquée.

Appelée par `noyau::horloge` (référence dans `config_app`, clé
`horloge_derniere_ecriture`), elle-même appelée par `api::rpc`.

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
