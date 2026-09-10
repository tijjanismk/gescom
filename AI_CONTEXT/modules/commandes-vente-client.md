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
`enregistrer_paiement`, `lire_clients_avec_creances` *(jamais appelée)*.

**creances.rs** — `lire_creances_ouvertes`, `regler_creance`,
`lire_etat_creances_client`, `lire_etat_creances_global`,
`lire_reglements_client`,
`annuler_reglement` ([creances.rs:233](../../src-tauri/src/commandes/creances.rs#L233)),
`lire_donnees_recu`, `solder_residus_creances` *(jamais appelée)*.

**retours.rs** — `lire_ventes_recentes`,
`enregistrer_retour` ([retours.rs:110](../../src-tauri/src/commandes/retours.rs#L110)),
`lire_avoirs_ouverts_tous`.

**avoirs.rs** — `lire_avoirs_client`, `total_avoirs_client`,
`rembourser_avoir`, `chercher_article_par_code_barre`,
`lire_config_scanner`, `sauvegarder_config_scanner`,
`sauvegarder_code_barre_article`, `lire_articles_avec_codes_barres`,
`appliquer_avoir_vente` *(jamais appelée)*.

**relances.rs** — `lire_creances_relances`, `enregistrer_relance`,
`lire_historique_relances`, `lire_stats_relances`.

**cheques.rs** — `enregistrer_cheque`, `lire_cheques`,
`changer_statut_cheque`.

## Entrant

| Écran | Ce qu'il appelle ici |
|---|---|
| [Ventes.tsx](../../src/pages/Ventes.tsx) | tout le POS, `creer_vente`, scan, `total_avoirs_client`, `enregistrer_cheque` |
| [Clients.tsx](../../src/pages/Clients.tsx) | listes, `regler_creance`, états de créance |
| [FicheClient.tsx](../../src/pages/FicheClient.tsx) | créances, règlements, `annuler_reglement`, avoirs |
| [Relances.tsx](../../src/pages/Relances.tsx) | les 4 commandes de relances |
| [Cheques.tsx](../../src/pages/Cheques.tsx) | les 3 de chèques |
| [Retours.tsx](../../src/pages/Retours.tsx) + [ModalsRetour.tsx](../../src/components/ModalsRetour.tsx) | `enregistrer_retour` |

Sortant : `coeur::calcul` (montants, statuts, répartition, effet caisse),
`utils::exiger_session_caisse`, `commandes::pieces_pos`
(facture POS automatique après `creer_vente`), `persistance::journal`.

## Règles métier

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
