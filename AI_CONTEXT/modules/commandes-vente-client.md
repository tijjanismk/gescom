# Module : commandes — vente et client

Rôle : le comptoir. Vente au POS, encaissement, créances, retours,
avoirs, relances, chèques.

## Fichiers

- [commandes/ventes.rs](../../src-tauri/src/commandes/ventes.rs) (809 l.)
  — **porte `EtatApp`**, le `Mutex<Connection>` que toute autre commande
  importe. D'où sa place en tête du graphe inverse (47 entrants) : ce
  n'est pas un couplage métier.
- [commandes/creances.rs](../../src-tauri/src/commandes/creances.rs) (824 l.)
- [commandes/retours.rs](../../src-tauri/src/commandes/retours.rs) (686 l.)
- [commandes/avoirs.rs](../../src-tauri/src/commandes/avoirs.rs) (524 l.)
  — avoirs **et** lecture des codes-barres au scan.
- [commandes/relances.rs](../../src-tauri/src/commandes/relances.rs) (181 l.)
- [commandes/cheques.rs](../../src-tauri/src/commandes/cheques.rs) (246 l.)

## Commandes exposées

**ventes.rs** — `lire_clients`, `lire_client_generique`,
`creer_client_rapide`, `modifier_client`, `lire_articles_avec_unites`,
`lire_depots`, `lire_depot_defaut`, `creer_article_rapide`,
`creer_vente` ([ventes.rs:407](../../src-tauri/src/commandes/ventes.rs#L407)),
`enregistrer_paiement` *(plus appelée par l'écran ; depuis le 23/09
une enveloppe de `regler_creance_datee`, D27)*,
`lire_clients_avec_creances` *(jamais appelée)*.

**creances.rs** — `lire_creances_ouvertes`, `regler_creance` (mode dans
la liste de `coeur::saisie`, jamais `avoir` ; tout ou rien sur les deux
moteurs — D27),
`lire_etat_creances_client`, `lire_etat_creances_global`,
`lire_reglements_client`,
`annuler_reglement` ([creances.rs:233](../../src-tauri/src/commandes/creances.rs#L233)),
`lire_donnees_recu`, `solder_residus_creances` *(jamais appelée)*.

**retours.rs** — `lire_ventes_recentes`,
`enregistrer_retour` ([retours.rs:110](../../src-tauri/src/commandes/retours.rs#L110)),
`lire_avoirs_ouverts_tous`. Le retour crédite `prix_pratique × qté`
(prix net, remise déduite), éteint d'abord la dette (D33), puis rend le
solde selon `coeur::calcul::repartir_retour` (D15) : la part payée en
argent en espèces ou en avoir selon le mode, la part payée avec un
avoir **toujours en avoir**. La réponse porte `part_especes`,
`part_avoir`, `numero_avoir`.

**Le retour groupé (26/10/2026)** — `enregistrer_retour_groupe`
(`retours:creer`, **né sur `Base`**, D22) : plusieurs lignes d'une vente
en un geste, `articles: [{ligne_vente_id, quantite}]`, mode
`remboursement` | `avoir_conserve` | `echange`, et en échange
`pris: [{article_id, unite_vente_id, quantite}]`. Tout ou rien, une
transaction. Le geste se juge d'abord (`coeur::pieces::verifier_retour_groupe` :
au moins un article, chacun une fois, mode connu, échange avec au moins
un article pris). Chaque article passe par `retourner_une_ligne` (la même
que `enregistrer_retour`, qui n'en est plus qu'un cas) : les gardes
« pas plus que vendu » et « jamais plus que versé » s'additionnent d'un
article à l'autre parce que chacun lit les écritures des précédents.
**Une seule pièce AVC** par geste (`AvoirDuRetour` : une ligne `avoir`
par retour, même `piece_id`, une ligne de pièce par article, la note au
total). L'échange se règle **une fois** (`remplacer_et_regler`) : les
articles pris sortent du magasin de la vente (D43), le reliquat va en
argent dans la limite du versé ou en avoir réparti au prorata
(`coeur::calcul::repartir_au_prorata`), le complément entre en caisse.
Le plafond de remboursement (C-3) se juge sur le total du geste. Une
ligne d'une autre vente est refusée (« introuvable »).
`lire_ventes_recentes` rend `client_generique` et, par ligne,
`deja_retourne`. Scénarios : `retours_groupe_base.rs` (6).

**avoirs.rs** — `lire_avoirs_client`, `total_avoirs_client`,
`rembourser_avoir`, `chercher_article_par_code_barre`,
`lire_config_scanner`, `sauvegarder_config_scanner`,
`sauvegarder_code_barre_article`, `lire_articles_avec_codes_barres`,
`appliquer_avoir_vente` *(jamais appelée)*.

**relances.rs** — `lire_creances_relances`, `enregistrer_relance`,
`lire_historique_relances`, `lire_stats_relances`.

**cheques.rs** — `enregistrer_cheque`, `lire_cheques`,
`changer_statut_cheque`. Un passage à `rejete` ne supprime plus rien :
il écrit un `paiement` négatif (`origine = 'rejet_cheque'`,
`annule_paiement_id`) et une sortie de caisse `cheque_rejete` en moyen
`cheque`, sans exiger une caisse ouverte — la sortie va sur la session
ouverte, sinon sur celle qui avait reçu le chèque (D14, 20/09/2026).
Scénario : `journal_rapports_base::un_cheque_suit_son_cycle_et_un_rejet_rouvre_la_creance`.

## Entrant

| Écran | Ce qu'il appelle ici |
|---|---|
| [Ventes.tsx](../../src/pages/Ventes.tsx) | tout le POS, `creer_vente`, scan, `total_avoirs_client`, `enregistrer_cheque` |
| [Clients.tsx](../../src/pages/Clients.tsx) | listes, `regler_creance`, états de créance |
| [FicheClient.tsx](../../src/pages/FicheClient.tsx) | créances, règlements, `annuler_reglement`, avoirs |
| [Relances.tsx](../../src/pages/Relances.tsx) | les 4 commandes de relances |
| [Cheques.tsx](../../src/pages/Cheques.tsx) | les 3 de chèques |
| [Retours.tsx](../../src/pages/Retours.tsx), [Pieces.tsx](../../src/pages/Pieces.tsx) + [ModalsRetour.tsx](../../src/components/ModalsRetour.tsx) (`ModalRetour`) | `enregistrer_retour_groupe`, `lire_ventes_recentes` |

Sortant : `coeur::calcul` (montants, statuts, répartition, effet caisse),
`utils::exiger_session_caisse`, `commandes::pieces_pos`
(facture POS automatique après `creer_vente`), `persistance::journal`.

## Règles métier

- [CONFIRMÉ] **Clients de passage (D35, 25/10/2026)** — module
  [passage.rs](../../src-tauri/noyau/src/passage.rs) : `creer_fiche_sur`
  / `creer_fiche` (dans la transaction de la vente, compteur `PAS`),
  `est_le_comptant_sur`, `lire_passages_sur` (code ou numéro de facture),
  `promouvoir_sur` (`clients:modifier`, journal `passage_promu`),
  `figer_noms_sur` / `figer_noms` (appelés par la promotion et
  `modifier_client`). `creer_facture_depuis_vente` prend le client de la
  **vente**, plus celui que l'écran passe. Commandes serveur
  `lire_passages`, `promouvoir_passage` (version `Base` seule).

- [CONFIRMÉ] Le découvert est **constaté en base**, jamais déclaré par
  l'écran. `creer_vente_sur` et `valider_facture_sur` relisent
  `stock_depot` dans leur propre transaction et recalculent
  `vente_a_decouvert` ; le drapeau envoyé par le POS n'est qu'un OU
  (il porte le cas de la vente répartie). Le POS le calcule sur le
  stock lu au chargement de l'écran : le croire laissait passer un
  découvert **invisible**, donc jamais régularisé. Tests
  `un_ecran_perime_ne_cache_pas_le_decouvert` et
  `une_vente_couverte_n_est_pas_signalee`.
- [CONFIRMÉ] Une vente n'est **jamais refusée** pour stock insuffisant —
  seul le transfert l'est (D32). Un client attend au comptoir ; un
  transfert est un mouvement interne que rien ne presse.

- [CONFIRMÉ] `creer_vente` exige une session de caisse ouverte
  ([ventes.rs:455](../../src-tauri/src/commandes/ventes.rs#L455)) — de
  même `enregistrer_paiement`
  ([ventes.rs:712](../../src-tauri/src/commandes/ventes.rs#L712)).
- [CONFIRMÉ] `annuler_reglement` n'exige la session que **conditionnellement**
  ([creances.rs:289](../../src-tauri/src/commandes/creances.rs#L289), un
  `Some(...)`) : c'est l'application directe de `effet_caisse_annulation`
  — une erreur de saisie sur session close ne doit toucher à rien.
- [CONFIRMÉ] Le prédicat « client générique » existe en deux exemplaires
  — `lire_client_generique` ici et `utils::est_client_generique`
  ([utils.rs:13](../../src-tauri/noyau/src/utils.rs#L13)). Le commentaire dit
  la duplication volontaire ; modifier l'un impose de vérifier l'autre.
- [CONFIRMÉ] `prix_pratique` est stocké **TTC** : la TVA est ajoutée au
  HT côté POS avant l'appel (D8). `SUM(prix_pratique × quantite)` est
  donc le montant dû, partout, sans correction.
- [DÉDUIT] `solder_residus_creances` implémenterait D41 en base (fermer
  les restes ≤ 5 F). Elle est exposée mais **jamais invoquée** : le seuil
  n'est appliqué qu'à la lecture, via `reste_exigible`. Vérifier avant de
  supposer qu'une créance résiduelle a été soldée en base.
- [DÉDUIT] `avoirs.rs` mélange deux sujets sans rapport — les avoirs et
  le scanner de codes-barres. Chercher une commande de scan dans
  `codebarre.rs` ne la trouve pas.
