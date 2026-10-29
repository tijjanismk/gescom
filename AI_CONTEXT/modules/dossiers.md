# Module : dossiers et exercices (v3, chantier D)

Un **dossier** est une société : ses clients, fournisseurs, stock,
caisse, pièces, numéros, journal (les tables de
`dossiers::TABLES_CLOISONNEES` portent `dossier_id`). Articles, prix,
catégories, utilisateurs et rôles sont **communs**
([PLAN-MULTISOCIETE.md](../PLAN-MULTISOCIETE.md)). Un **exercice** est
une période de travail d'un dossier.

| Couche | Fichier | Rôle |
|---|---|---|
| Règles pures | [dossiers.rs](../../src-tauri/noyau/src/dossiers.rs) | `Exercice::accepte` (borne + quoi faire), `dossier_accepte`, `dates_de_travail` (D21 : données, ou l'année civile), le détecteur `requete_non_cloisonnee` |
| Base | idem | `creer_dossier_sur(base, code, societe, date_debut, date_fin)` (exercice + magasin principal + client de passage + journal `dossier_cree`, une transaction), `lire_dossiers_sur`, `lire_exercices_sur`, `verifier_date_sur`, `ouvrir/prolonger/clore_exercice_sur`, `dossier_memorise_sur` |
| Dates lisibles | [coeur/dates.rs](../../src-tauri/noyau/src/coeur/dates.rs) | `en_lettres` : « 5 avril 2027 », « 1er mars 2026 » |
| Stock | `dossiers::DECLENCHEUR_STOCK_SQLITE`, `REPARER_STOCK_DOSSIER` | D-1 : la ligne de stock suit le dossier du **mouvement** |
| Garde-fou | [serveur/src/api.rs](../../src-tauri/serveur/src/api.rs) `rpc` + `dossiers::date_d_ecriture` | D-4 : toute écriture datée (date donnée, ou aujourd'hui) passe `verifier_date_sur` avant la poignée ; refus = 409 `Metier`. Une commande qui écrit de l'argent ou du stock s'ajoute à `date_d_ecriture` |
| Écran | [components/OngletDossiers.tsx](../../src/components/OngletDossiers.tsx), [ExercicesDossier.tsx](../../src/components/ExercicesDossier.tsx) | Paramètres → Dossiers (`dossiers:gerer`) : la liste (« ouvert ici »), la création avec ses dates de travail ; les exercices du dossier ouvert — prolonger, clore, ouvrir le suivant |
| Migration | `dossiers::migrer_dossier_d_origine_sur` | D-5 : une fois par base, le dossier d'origine prend le nom de la société et un exercice qui couvre la plus ancienne écriture ; appelée par `amorcer` et `serveur::main` (fichier) |
| Connexion | `pages/PageLogin.tsx` | plusieurs dossiers, aucun mémorisé : « Quel dossier ouvrir ? » (mémoriser coché par défaut) |
| Avis de fin | [coeur/fin_exercice.rs](../../src-tauri/noyau/src/coeur/fin_exercice.rs), `dossiers::derniere_date_ecrivable` (pure), `dossiers::avis_fin_exercice_sur(base, aujourd_hui)`, commande `lire_avis_fin_exercice` (ouverte à tous), [components/AvisFinExercice.tsx](../../src/components/AvisFinExercice.tsx) au tableau de bord | 27/10/2026 : prévenir avant que D21 refuse tout. Voir plus bas |

**D22** : plusieurs dossiers demandent **le serveur**, pas PostgreSQL.
Le serveur sert tout par `Base` (D-2) — SQLite ou PostgreSQL. La
fenêtre monoposte n'a pas de commande pour créer un dossier : elle
reste au dossier d'origine.

**Le refus de date** (D21), mot pour mot : « Le 5 avril 2027 est hors
des dates de travail (jusqu'au 28 février 2027). Prolonger l'exercice
ou en ouvrir un nouveau. » — y compris quand aucun exercice ne couvre
la date mais qu'un exercice ouvert finit avant ; avant le premier :
« … est avant les dates de travail (à partir du 1er mars 2026). »

**Les exercices** (D-4) : pas de trou — le suivant commence le
lendemain du dernier, prolongation comprise (`debut_suivant`) ; pas de
chevauchement ; une prolongation va plus loin et s'arrête avant le
suivant ; un exercice clos ne se rouvre ni ne se prolonge. Chaque geste
au journal : `exercice_ouvert`, `exercice_prolonge`, `exercice_clos`.
Plusieurs années, oui, **cinq au plus** (`coeur::fin_exercice::DUREE_MAX_ANS`,
27/10/2026) : une prolongation va au plus 5 ans après la fin prévue, un
exercice ouvert ou créé dure au plus 5 ans — une prolongation ne se
raccourcit pas, une faute d'année (2062 pour 2026) bloquerait le dossier.
L'écran propose +1/+2/+3 ans et dit l'écart en clair (« soit 3 ans de plus »).
**Couper** (D38, `couper_exercice_sur`, `juger_coupe` pure) : le dernier
exercice ouvert s'arrête au jour choisi, le suivant s'ouvre le lendemain
dans la même transaction, au moins jusqu'à l'ancienne fin (défaut : un
an) ; la prolongation s'efface de la ligne, le journal `exercice_coupe`
garde l'ancienne fin. Ni un clos, ni un exercice qui a déjà un suivant.
**Clore** (D38, `juger_cloture` pure) : seulement un exercice terminé
(fin passée, horloge du serveur ; `clore_exercice_au_sur` prend la date
pour les tests) et dans l'ordre des dates. Clore l'exercice en cours
arrêtait toutes les ventes sans remède ; l'écran dit la raison avant la
confirmation.
Les exercices ne suivent pas l'année civile (juillet → juin se donne à
la création), mais la **numérotation**, elle, la suit (`FAC-2026-…`,
`argent::reserver_numero_sur`).

**L'avis de fin** (27/10/2026) : D21 ne bouge pas, on l'annonce. La
fin est le dernier jour écrivable **sans interruption** depuis
aujourd'hui (un suivant déjà ouvert la repousse), jugée par
`dossier_accepte` : l'avis prédit exactement le refus. Horloge du
serveur. Rien au-delà de 30 jours (`PREAVIS_JOURS`), orange dans le
mois, rouge les 7 derniers (`URGENCE_JOURS`) et le jour du refus, qui
est alors rendu mot pour mot (`refus`). Montré dans les deux branches
du tableau de bord (le caissier sans `rapports:lire` aussi) : avec
`dossiers:gerer`, « prolonger ou ouvrir le suivant » et un bouton vers
Paramètres → Dossiers ; sans, « prévenir le patron ». Seulement le
dossier ouvert : un patron à plusieurs dossiers ne voit pas l'avis
des autres.

**Le dossier d'origine** (D-5) : une base d'avant la v3 est le dossier
`PRINCIPAL`. Il s'appelait « Ma boutique » et n'avait que l'année en
cours : la migration le nomme comme la société et recule son début au
1er janvier de la plus ancienne écriture. `renommer_dossier` et
l'invitation de l'écran font le reste.

Preuves : `exercice_coupe_base.rs` (5) et `fin_exercice_base.rs` (7), deux moteurs et compte limité, `dossier_d_origine.rs` (4), `dossiers_base.rs` (9 scénarios, deux moteurs, dont
`un_dossier_nait_avec_ses_dates_de_travail_et_refuse_ce_qui_en_sort`),
`cloisonnement.rs` (20), route `une_ecriture_hors_des_dates_de_travail_est_refusee_avant_d_ecrire`, banc `d3-dossiers.mjs`, `d4-exercices.mjs`, `d5-dossier-d-origine.mjs`.

## L'assistant du premier démarrage (D43, 28/10/2026)

Une base NEUVE porte `config_app.assistant_dossier = a_faire`, posée par
`amorcage::amorcer` (Base) et `seed::seeder` (fichier) ; une base sans
la marque ne voit jamais l'assistant. `amorcage::mode_essai` (appelée
par `donnees_demo`, et par le serveur sous `GESCOM_DEMO=1` si la marque
est là) la retire et crée `employe` / `employe123` : **ce compte
n'existe plus qu'en démonstration** (`COMPTES_USINE` = le patron seul).

| fonction | rôle |
|---|---|
| `assistant::a_faire_sur(acces)` | la marque est-elle là |
| `assistant::refus_si_a_faire(acces, commande)` | le verrou du serveur : tout sauf `COMMANDES_PERMISES` |
| `assistant::lire_sur(base, utilisateur)` | ce que l'écran propose (société, dates, magasin, patron) |
| `assistant::terminer_sur(base, utilisateur, Saisie)` / `terminer_au_sur(…, maintenant)` | tout en une transaction : patron (nom, identifiant, mot de passe), `parametres_societe`, `dossier.societe`, l'unique exercice ouvert, TVA et fond (`config_app`), premier numéro de facture (`compteur_piece`, refusé si une facture est sortie), nom du dépôt par défaut, vendeurs (rôle `employe`, `doit_changer_mdp`), journal `assistant_dossier_fait`, marque « fait » |
| `assistant::{lire,definir}_reglages_dossier_sur` | TVA des nouveaux articles (`tva_par_defaut:<dossier>`, 0 ou 18) et fond de caisse habituel (`fond_caisse_habituel:<dossier>`), après l'assistant ; journal `reglages_dossier_modifies` |
| `assistant::taux_nouvel_article_sur` | le taux que prennent les articles neufs (comptoir, Paramètres, import) |
| `assistant::seuil_defaut_sur` | D45 : le seuil d'alerte des articles qui n'ont pas le leur (`seuil_alerte_defaut:<dossier>`, 5 d'office) ; `definir_reglages_dossier_sur` prend `seuil_defaut` |
| `assistant::definir_tva_boutique_sur(base, tva_pct, appliquer_aux_articles)` | l'état de la TVA (Paramètres → TVA) ; avec `appliquer_aux_articles`, tous les articles actifs au même taux dans la même transaction (ils sont communs à tous les dossiers : `lire_reglages_dossier` rend `nb_articles`, `nb_dossiers` pour le dire avant) |

Règles pures : `coeur/assistant.rs` (identifiant, mot de passe — celui
d'usine refusé —, TVA 0/18, premier numéro 1…99 999, fond 0…10 M, des
dates qui comprennent aujourd'hui). Les dates passent aussi par
`dates_de_travail` (5 ans au plus).

Preuves : `assistant_base.rs` (9, deux moteurs), route
`une_base_neuve_ne_sert_que_l_assistant_jusqu_a_ce_qu_il_soit_fait`,
banc `p1-premier-demarrage.mjs` (`lancer.ps1 monoposte -Neuf -SansDemo`).
