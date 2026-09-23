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
| Écran | [components/OngletDossiers.tsx](../../src/components/OngletDossiers.tsx) | Paramètres → Dossiers (`dossiers:gerer`) : la liste (« ouvert ici »), la création avec ses dates de travail |
| Connexion | `pages/PageLogin.tsx` | plusieurs dossiers, aucun mémorisé : « Quel dossier ouvrir ? » (mémoriser coché par défaut) |

**D22** : plusieurs dossiers demandent **le serveur**, pas PostgreSQL.
Le serveur sert tout par `Base` (D-2) — SQLite ou PostgreSQL. La
fenêtre monoposte n'a pas de commande pour créer un dossier : elle
reste au dossier d'origine.

**Le refus de date** (D21), mot pour mot : « Le 5 avril 2027 est hors
des dates de travail (jusqu'au 28 février 2027). Prolonger l'exercice
ou en ouvrir un nouveau. » — y compris quand aucun exercice ne couvre
la date mais qu'un exercice ouvert finit avant ; avant le premier :
« … est avant les dates de travail (à partir du 1er mars 2026). »

Preuves : `dossiers_base.rs` (7 scénarios, deux moteurs, dont
`un_dossier_nait_avec_ses_dates_de_travail_et_refuse_ce_qui_en_sort`),
`cloisonnement.rs` (20), banc `d3-dossiers.mjs`.
