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
| [Achats.tsx](../../src/pages/Achats.tsx) | `enregistrer_achat`, `creer_fournisseur` ; `lire_articles_avec_unites` porte `gere_en_stock` (D36) : un article non suivi reste achetable et l'écran le marque (lot C.8) |
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
- [CONFIRMÉ] **Sans fournisseur : comptant, chez « Fournisseur divers »** (25/09) : avant, un achat sans fournisseur n'avait ni pièce ni paiement (à crédit : rien du tout) — la somme n'était dite nulle part, et AC laissait au 401 une dette envers personne. Règle `coeur::tiers::verifier_achat_sans_fournisseur` (comptant seulement, aussi quand on choisit le « divers » dans la liste) jugée avant d'écrire, dans les deux versions ; `achats::fournisseur_divers[_sur]` le crée une fois par dossier (`fournisseur.est_generique = 1`, trois chemins de schéma) ; l'achat a sa FAF payée et son `paiement_fournisseur`. `lire_fournisseurs` rend `est_generique` ; l'écran ferme « À crédit » sans fournisseur. Les achats d'avant restent sans pièce ; RG les solde par leur sortie de caisse (`mouvement_caisse` motif `achat` du même lot). Scénarios `achats_base` (2), `journaux_comptables_base` (1) ; banc `i1-achat-sans-fournisseur.mjs`.
- [CONFIRMÉ] **Le « Fournisseur divers » n'a qu'une porte** (22/10, revue de `rh`) : la règle « comptant seulement » tenait dans Achats, mais Pièces (`creer_piece_fournisseur*`), la validation d'une FAF (`valider_facture_fournisseur*`), le retour fournisseur en avoir et le règlement de dette le laissaient recevoir une dette ou un avoir envers personne, et sa fiche se renommait. Règles pures `coeur::tiers::refuser_si_fournisseur_divers` / `verifier_retour_fournisseur_divers`, jugées dans les deux versions via `achats::est_fournisseur_divers[_sur]` (`&mut impl Acces`) : pas de pièce à la main, pas de règlement, retour **remboursé** seulement, fiche figée (comme le client générique). Dans un second dossier, sa FAF est maintenant numérotée (préfixe du dossier, voir [numerotation.md](numerotation.md)). Scénarios `achats_base` (+1, et l'achat du second dossier passe).
- [CONFIRMÉ] **La réception se date** (18/09) : `enregistrer_achat_date[_sur_base]` (`enregistrer_achat*` en enveloppe), règle `coeur::dates`, permission `pieces:antidater` vérifiée par le serveur (`dateReception`). La FAF et le `paiement_fournisseur` portent la date de l'affaire ; `mouvement_stock` et `mouvement_caisse` restent au jour, libellé « Réception du jj/mm ». Scénario dans `fournisseurs_base`.
- [CONFIRMÉ] Un bon de réception ne se facture qu'une fois ; la FAF issue d'un BRF ne fait entrer **que l'argent**, le stock est entré au bon (`stock_confie_a_un_bon`) — [livraison-stock.md](livraison-stock.md).
- [CONFIRMÉ] Retour fournisseur : sort du **dépôt de la facture d'achat** (paramètre → `piece_origine_id` → défaut, D43) ; **refusé au-delà du stock**, quantités cumulées par article (D32) ; reliquat **par facture**. `lire_factures_fournisseur_retournables` rend `retournable = min(reliquat, stock)`.
- [CONFIRMÉ] Rendre une marchandise entrée sans facture ne crée **aucun avoir** : simple sortie de stock, plafonnée au stock présent, tracée au journal — `enregistrer_retour_sans_facture`.
- [CONFIRMÉ] Un AVF « remboursement » naît `paye` : il ne réduit pas la dette en plus de l'entrée de caisse.
- [CONFIRMÉ] `annuler_paiement_fournisseur` : contre-passation (jamais de suppression), l'argent **rentre** si le fournisseur rembourse ; virement et chèque ne touchent pas le tiroir — symétrique de `annuler_reglement`.
- [CONFIRMÉ] Annuler une FAF validée = un retour intégral (`annuler_facture_fournisseur_par_avoir`) ; un brouillon n'a rien à annuler.
- [CONFIRMÉ] **Le nom figé du fournisseur** (lot D.9, 27/10/2026, comme D35 côté client) : `modifier_fournisseur[_sur_base]` appelle d'abord `fournisseurs::figer_noms[_sur]` (`&mut impl Acces`), qui écrit le nom EN COURS dans `piece_commerciale.tiers_nom` des pièces émises qui n'en portent pas (les brouillons suivent la fiche) — fige puis renomme dans **une** transaction (avant : pas de transaction du tout). Lecture `COALESCE(pc.tiers_nom, f.nom)` dans `lire_donnees_piece` (impression), `lire_toutes_pieces_fournisseur` (liste ; la recherche trouve l'ancien nom comme le nouveau) et le reçu `lire_donnees_recu` côté fournisseur — les deux versions chaque fois. Les états de dette (par fournisseur, aujourd'hui) gardent le nom de la fiche. Scénario `donnees_base::une_facture_fournisseur_emise_garde_le_nom_ecrit_a_son_emission`.
- [DÉDUIT] `lire_etat_dette_fournisseur` compte aussi une FAF **brouillon** (filtre `statut <> 'annule'` seulement) : vu en écrivant le scénario D.9, non corrigé (hors lot).

## Tests

`tests_multi_depot` (lib) — dépôt du retour, découvert, cumul ;
[miroir_fournisseur.rs](../../src-tauri/noyau/tests/miroir_fournisseur.rs) ;
sur `Base` : [achats_base.rs](../../src-tauri/noyau/tests/achats_base.rs),
[fournisseurs_base.rs](../../src-tauri/noyau/tests/fournisseurs_base.rs),
[gestion_base.rs](../../src-tauri/noyau/tests/gestion_base.rs) (chantiers),
[donnees_base.rs](../../src-tauri/noyau/tests/donnees_base.rs) (nom figé, lot D.9).

## Doublons, import et export (D44, 28/10/2026)

- `fournisseurs_csv::exporter_fournisseurs_csv_sur_base` — `Nom;Téléphone;Adresse;NIF;Email;Voisin`, UTF-8 avec BOM, sans le « Fournisseur divers ».
- `fournisseurs_csv::importer_fournisseurs_csv_sur_base(base, contenu, completer, simuler)` — crée, complète un doublon (champs vides seulement, jamais le nom) ou le laisse ; refuse nom vide, nom réservé, « Voisin » illisible, doublon dans le fichier. `simuler` : l'aperçu, transaction qui retombe. Journal `import_fournisseurs`.
- `fournisseurs_csv::juger_saisie_sur` — appelée par `creer_fournisseur_sur_base` et `modifier_fournisseur_sur_base` (nouveau paramètre `forcer`) : même NIF refusé, même nom → `FOURNISSEUR_HOMONYME` sauf `forcer`.
- Règle pure : `coeur/doublons.rs` (`normaliser_nom`, `normaliser_nif`, `ressemblance_fournisseur` — deux NIF différents ne se ressemblent jamais).
- Les articles, même règle de nom : `catalogue_csv::article_du_meme_nom_sur`, `refuser_doublon_article_sur` (comptoir et Paramètres), import avec `simuler`.

Preuves : [doublons_base.rs](../../src-tauri/noyau/tests/doublons_base.rs) (9, deux moteurs).
