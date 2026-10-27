# Module : commandes — vente et client

Rôle : le comptoir. Vente au POS, encaissement, créances, retours,
avoirs, relances, chèques.

## Fichiers

- [commandes/ventes.rs](../../src-tauri/src/commandes/ventes.rs) (809 l.)
  — **porte `EtatApp`**, que toute autre commande importe. D'où sa place
  en tête du graphe inverse (47 entrants) : ce n'est pas un couplage
  métier. **Depuis le 27/10/2026 (lot E.13), plus de base** : `EtatApp.conn`
  est `SansBase`, dont `lock()` rend toujours `Err(REFUS_SANS_BASE)` —
  les 182 façades gardent leur appel et refusent toutes au même endroit,
  sans panique ni écriture. L'écran n'en appelle aucune (tout passe au
  serveur sauf `pont.ts::LOCALES`).
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
`lire_donnees_recu`, `solder_residus_creances` *(jamais appelée — depuis
le 27/10 un simple rattrapage des ventes anciennes, voir `solder_residu`)*.
`lire_creances_ouvertes` rend aussi `encaisse` et `eteint_par_retour`
(lot B.3, 27/10) ; `total_paye` et `reste` inchangés.

**Le résidu D41 au règlement (lot B.4, 27/10/2026)** —
`creances::solder_residu(tx: &Connection, vente, auteur, maintenant)` et
`solder_residu_sur(&mut impl Acces, …)` : appelées **dans la transaction**
du geste, juste après l'écriture du paiement. Si `coeur::calcul::residu_a_solder`
trouve 1 à 5 F : vente `payee`, pièce `paye`, journal `residu_absorbe`
— aucun paiement, aucune caisse. Rendent le résidu (0 sinon). Appelants,
dans les deux versions : `regler_creance_datee` (son `residu_absorbe`
en vient), `regler_creance_exceptionnel`, `argent::creer_vente_datee_sur`,
`valider_facture_sur` (si acompte), `retours::enregistrer_retour_sur` /
`enregistrer_retour_sur_base` / `enregistrer_retour_groupe_sur_base`
(une fois le geste entier écrit, si une dette a été éteinte),
`avoirs::appliquer_avoir_vente`. Ne pas l'appeler sans paiement écrit
(un échange) : le journal compterait deux fois. Scénarios :
`retours_chiffres_base.rs`.

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

**Les ventes par pages (lot D.12, 27/10/2026)** — l'écran Retours et la
fenêtre de retour de Pièces ne lisent plus `lire_ventes_recentes` (les
50 dernières, rien au-delà : une vente plus ancienne ne se retournait
nulle part) mais `pagination::lire_ventes_recentes_paginee` (50 par
page, `Pagination`), qui rend maintenant ce que lit `ModalRetour` :
`client_generique` et `deja_retourne` par ligne. La recherche (serveur)
porte sur le nom, le **code** du client et le **numéro de facture**
(`RECHERCHE_VENTE`) ; Pièces retrouve la vente d'une facture par son
numéro. Ordre `date_vente DESC, cree_le DESC, id` (pas de vente sur deux
pages). `lire_ventes_paginees` suit les mêmes règles ; ses deux versions
lisent le numéro dans `piece_commerciale` (la version `Connection`
lisait l'ancienne table `facture`). `lire_ventes_recentes` reste
enregistrée, plus appelée. Scénario `donnees_base::les_ventes_se_lisent_par_pages_de_50`.

**avoirs.rs** — `lire_avoirs_client`, `total_avoirs_client`,
`rembourser_avoir`, `chercher_article_par_code_barre`,
`lire_config_scanner`, `sauvegarder_config_scanner`,
`sauvegarder_code_barre_article`, `lire_articles_avec_codes_barres`,
`appliquer_avoir_vente` *(jamais appelée)*.

**relances.rs** — `lire_creances_relances`, `enregistrer_relance`,
`lire_historique_relances`, `lire_stats_relances`. Une créance relancée
porte `total_paye` (dette éteinte par retour comprise, il fait `reste`),
`encaisse` (l'argent versé, `coeur::calcul::encaisse`) et
`eteint_par_retour` (lot B.3, 27/10) ; l'écran affiche « payé » =
`encaisse` et le retour à part.

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
| [Retours.tsx](../../src/pages/Retours.tsx), [Pieces.tsx](../../src/pages/Pieces.tsx) + [ModalsRetour.tsx](../../src/components/ModalsRetour.tsx) (`ModalRetour`) | `enregistrer_retour_groupe`, `lire_ventes_recentes_paginee` (lot D.12) |
| [ListePassages.tsx](../../src/components/ListePassages.tsx) (Clients → Passages) | `lire_passages`, `promouvoir_passage`, `rattacher_passage` (lot D.10), `lire_clients_pagines` pour choisir la cible |

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
- [CONFIRMÉ] **Rattacher un passant à un client (lot D.10, 27/10/2026)** —
  `passage::rattacher_sur(base, passage_id, client_id)` (version `Base`
  seule, commande `rattacher_passage`, `clients:modifier`). Geste
  manuel : le patron choisit la fiche ; jamais de fusion automatique.
  Règle pure `coeur::tiers::juger_rattachement` (source : fiche de
  passage active du dossier ; cible : client ordinaire actif du dossier,
  ni « Comptant », ni passage, ni elle-même), fiches lues **dans** la
  transaction. Une transaction : la fiche de passage d'abord désactivée
  (`actif = 0` seulement si elle l'était encore — un second rattachement
  simultané ne touche rien et s'arrête), puis le nom ET le code figés sur
  ses pièces émises (`tiers_nom`, `tiers_code` — nouvelle colonne), puis
  `vente`, `piece_commerciale` (tiers), `avoir`, `echange`, `rappel`
  passent au client (paiements, retours, chèques, relances,
  irrécouvrables, mouvements suivent la vente). Un prospect qui reçoit
  des ventes devient client (D34). Journal `passage_rattache` sur le
  client (ancien : id, code, nom du passage ; nouveau : le client et le
  compte des lignes déplacées). **La fiche n'est pas supprimée** : vide,
  désactivée, hors de `lire_passages_sur` (filtre `actif = 1`) ; son code
  reste pris (il est sur les tickets) et le journal qui la nomme se lit.
  Ni stock, ni caisse ne bougent. À l'impression d'une pièce ou d'un
  reçu, `COALESCE(tiers_code, code)` et, pour un nom figé « Client de
  passage », ni téléphone, ni adresse, ni NIF
  (`passage::coordonnee_a_imprimer`) ; la recherche de Pièces trouve
  aussi le code figé. Le geste ne se défait pas. Scénario
  `donnees_base::rattacher_un_passant_deplace_toute_son_histoire_et_garde_ses_factures`.

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
- [CONFIRMÉ] D41 s'applique **en base au moment du règlement** depuis
  le 27/10 (`solder_residu[_sur]`, ci-dessus). `solder_residus_creances`
  (exposée, jamais invoquée) ne rattrape plus que les ventes réglées
  avant cette date ; `reste_exigible` reste la règle de lecture.
- [CONFIRMÉ] Un paiement `mode = 'retour'` est une **dette éteinte**,
  pas de l'argent : il compte dans `total_paye` et le reste dû, jamais
  dans un « payé » ou « encaissé » affiché (Pièces, Relances, Clients,
  fiche client — onglet Règlements —, facture imprimée, rapport mensuel).
- [DÉDUIT] `avoirs.rs` mélange deux sujets sans rapport — les avoirs et
  le scanner de codes-barres. Chercher une commande de scan dans
  `codebarre.rs` ne la trouve pas.
