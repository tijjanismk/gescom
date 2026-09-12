# Module : portage PostgreSQL

Rôle : rendre le serveur capable de tenir la base dans PostgreSQL.

## Qui porte le moteur

Le moteur est une affaire du **serveur**, et de lui seul. Le client ne
voit jamais la base : il parle au serveur, qui décide ce qu'il y a
derrière. Changer de moteur ne demande donc rien au poste caisse — c'est
tout l'intérêt d'avoir un seul chemin.

SQLite reste le défaut : une boutique à une caisse installe le serveur
sur son unique ordinateur et n'a aucun service de base de données à
poser. PostgreSQL devient utile quand la boutique grandit — plusieurs
caisses actives, des sauvegardes à chaud, une vue consolidée.

Les deux moteurs coexistent dans le même binaire, choisis par l'adresse
passée au serveur.

## Ce que le portage coûte réellement

Estimation annoncée plus tôt dans le projet : « 614 `rusqlite::`, 1556
placeholders, 150 `CAST` ». Elle était obtenue par `grep` sur tout le
dépôt et **surestimait largement** : elle comptait les tests, les
commentaires, et des `CAST` qui n'ont rien d'incompatible.

Mesure refaite sur les seules requêtes SQL de `noyau/src`, construction
par construction — **578 requêtes** analysées :

| construction | occurrences | remède |
|---|---|---|
| placeholder `?N` | 1448 | `$N` — **mécanique**, voir plus bas |
| comparaison `= 0` / `= 1` | 106 | néant si les colonnes restent `INTEGER` |
| `julianday(` | 12 | `EXTRACT(EPOCH FROM …)` |
| `substr(` | 9 | existe, mais pas les indices négatifs |
| `PRAGMA` | 7 | n'existe pas — réglages de connexion |
| `INSERT OR IGNORE` | 6 | `ON CONFLICT DO NOTHING` |
| `randomblob` / `lower(hex(…))` | 6 | `gen_random_uuid()` |
| `date('now')` | 4 | `CURRENT_DATE` |
| `CAST(… AS INTEGER)` | 4 | `::bigint` |
| `VACUUM` | 4 | `VACUUM INTO` n'existe pas — la sauvegarde change |
| `INSERT OR REPLACE` | 3 | `ON CONFLICT DO UPDATE` |
| `substr(…, -N)` | 3 | PostgreSQL ne compte pas depuis la fin |
| `strftime(` | 1 | `to_char()` |

**Hors placeholders et comparaisons booléennes, il reste ~60
occurrences, sur 12 fichiers.** Ce n'est pas le chantier qu'on
croyait ; le vrai poids est ailleurs (voir « Ce qui coûte vraiment »).

Les fichiers concernés, par ordre de densité :
`persistance/v2.rs` (19), `persistance/mod.rs` (11), `rapports.rs` (6),
`cheques.rs` (4), `chantiers.rs` (3), `depots.rs` (3), `relances.rs` (3),
`sauvegarde.rs` (3), puis `caisse.rs`, `comptoir.rs`, `journal.rs`,
`pagination.rs` (1 chacun).

L'inventaire se refait à volonté :
`python outils/inventaire_pg.py src-tauri/noyau/src`.

## Étape 1 — faite : un schéma que les deux moteurs acceptent

`schema.sql` est désormais **dialect-neutre**. Deux corrections ont
suffi, et aucune n'est un compromis :

- `PRAGMA foreign_keys = ON;` retiré. C'était un réglage de
  **connexion**, pas de schéma : SQLite le remet à zéro à chaque
  ouverture, donc l'écrire là ne protégeait rien. Il vit dans
  `ouvrir_base`, où il agit vraiment.
- `DEFAULT (datetime('now'))` → `DEFAULT CURRENT_TIMESTAMP`. Même
  valeur pour SQLite, et PostgreSQL le comprend.

Vérifié sur PostgreSQL 18.4 : **28 tables, 52 index, zéro erreur**, et
les 139 tests SQLite passent sans changement.

Aucune copie `schema_pg.sql` n'est maintenue — deux fichiers auraient
divergé au premier ajout de colonne.

## Étape 2 — faite : la façade, et une base PostgreSQL amorçable

[`noyau/src/base.rs`](../../src-tauri/noyau/src/base.rs) —
`Base::ouvrir(cible)` : une URL `postgresql://` ouvre PostgreSQL, tout
le reste ouvre SQLite. Un seul réglage, qui tient dans la ligne de
commande du serveur.

Elle règle les **deux écarts qui bloquent** :

- **les placeholders** — `?1` devient `$1`, traduit au passage. Réécrire
  1448 placeholders à la main, c'est 1448 occasions de se tromper. La
  traduction ne touche pas à ce qui est entre apostrophes : un message
  affiché au commerçant peut contenir un point d'interrogation ;
- **les lignes** — `rusqlite::Row` et `postgres::Row` n'ont pas la même
  API. `Ligne` leur donne la même, pour que les 435 fermetures
  `|r| r.get(0)?` survivent au portage **sans être touchées**.

Ce qu'elle ne fait **pas** : traduire le SQL au-delà des placeholders.
`julianday`, `strftime`, `INSERT OR IGNORE`, `substr(x, -5)` se
réécrivent à la main, une fois, en SQL que les deux moteurs acceptent.
Une traduction automatique de SQL marche sur les cas qu'on a essayés et
ment sur les autres.

### Deux écarts de TYPE, découverts en essayant

`schema.sql` est accepté par les deux moteurs — mais les **largeurs**
diffèrent, et PostgreSQL est strict là où SQLite est indifférent :

| SQLite | PostgreSQL | ce que le noyau envoie |
|---|---|---|
| `INTEGER` (64 bits) | `integer` = **4 octets** | `i64` |
| `REAL` (64 bits) | `real` = **4 octets** | `f64` |

PostgreSQL refuse net, avec `error serializing parameter 3` — un
message qui ne nomme pas la colonne fautive. `creer_schema` élargit donc
en `BIGINT` et `DOUBLE PRECISION` pour PostgreSQL seulement. SQLite
ignore ces largeurs : **un seul fichier de schéma**.

Le piège se répète pour tout DDL écrit en Rust et non dans
`schema.sql` — les `ALTER TABLE` des migrations. Ils passent par la même
traduction.

### Ce qui marche aujourd'hui sur PostgreSQL — **tout, depuis le 12/09/2026**

[`noyau/src/amorcage.rs`](../../src-tauri/noyau/src/amorcage.rs) crée le
schéma, les rôles, les comptes, le dépôt, le client « Comptant », les
tables du réseau v2, le cloisonnement par dossier et le déclencheur de
stock — sur les deux moteurs.

**186 des 187 commandes du serveur ont leur version `Base`**
(`*_sur_base`, branchée par `Registre::aussi_sur_base`). La 187e,
`lire_catalogue_permissions`, ne touche pas la base. Vendre, facturer,
encaisser, acheter, rendre, transférer, clôturer, relancer, imprimer :
tout passe. Voir [ETAPES.md](../ETAPES.md) pour le détail des lots et
[DECISIONS.md](../DECISIONS.md) D11.

Ce qui distingue une version `Base` de sa jumelle `Connection`, partout
de la même façon :

- chaque table cloisonnée porte `dossier_id` dans ses `WHERE` et ses
  `INSERT` ;
- les filtres passent par des paramètres liés, jamais par `format!` sur
  une valeur ;
- `LOWER(x) LIKE LOWER(...)` : LIKE ignore la casse sur SQLite, pas sur
  PostgreSQL ;
- `CAST(?n AS TEXT) IS NULL OR col = ?n` pour un filtre optionnel —
  PostgreSQL ne sait pas typer un paramètre NULL nu ;
- `CAST(... AS BIGINT)` autour des sommes — `SUM` rend `NUMERIC` sur
  PostgreSQL, que `i64` refuse ;
- les dates se comparent par `SUBSTR(x, 1, 10)` ; les durées
  (`julianday`) se calculent en Rust (`utils::jours_depuis`) ;
- les écritures composées tournent dans une transaction, même là où la
  version SQLite n'en ouvrait pas.

### Le trait `Acces`

`Base` et `Transaction` implémentent [`base::Acces`](../../src-tauri/noyau/src/base.rs)
— `dossier`, `executer`, `lire_une`, `lire_plusieurs`. Une aide écrite
une fois (`reserver_numero_sur`, `inserer_lignes_sur`, `exiger_sur`,
`marquer_entierement_livre_sur`…) sert dedans comme dehors. Avant lui,
chacune existait en deux copies.

### Le filet : `tests/schema_commun.rs`

Deux chemins créent une base — la fenêtre (`persistance::initialiser_tables`,
SQLite, migrations accumulées) et le serveur (`amorcage::amorcer`,
`schema.sql`, les deux moteurs). **Sept fois** une colonne ou une table
n'existait que sur le premier, et aucune base PostgreSQL ne l'avait :
`avoir.piece_id`, `unite_vente.code_barre`, les deux
`annule_paiement_id`, `entete_chemin`/`pied_chemin`, `cheque_recu`,
`mouvement_caisse.poste_id`. Le test compare les deux structures colonne
par colonne et nomme ce qui manque.

## Ce qui reste

- **La sauvegarde.** `VACUUM INTO` copie un fichier SQLite ; une base
  PostgreSQL se sauvegarde avec `pg_dump` sur le serveur (D4).
  `sauvegarder_base_sur_base` **refuse clairement** sur PostgreSQL et
  `lire_config_sauvegarde` rend le moteur pour que l'écran le dise.
  Brancher `pg_dump` — chemin de l'exécutable, mot de passe hors dépôt
  (D10), rotation — est un chantier à part.
- **Le déclencheur de stock et le dossier.** `stock_suit_les_mouvements`
  pose la ligne `stock_depot` sans `dossier_id` explicite : le défaut de
  la colonne (SQLite : `defaut` ; PostgreSQL : le dossier de session)
  s'applique. Juste sur PostgreSQL ; sur SQLite, un mouvement écrit pour
  `dossier-b` crée sa ligne de stock dans `defaut`. Sans effet tant
  qu'une installation SQLite ne connaît qu'un dossier — à régler avec
  la v3.
- **Un essai à la main, écran par écran**, sur une caisse branchée à un
  serveur PostgreSQL. Les 83 scénarios disent ce que le SQL fait à la
  base ; ils ne disent pas ce que l'écran en montre.

## Le raccordement

La chaîne de connexion ne va **pas dans le dépôt** : mot de passe. Elle
se lira dans `poste.json` ou dans une variable d'environnement, comme
le chemin de la base aujourd'hui.

Pour les essais en développement, PostgreSQL 18.4 tourne en local sur
le port 5432 et la base d'essai s'appelle `gescom_essai`.

## Règles métier

- [CONFIRMÉ] `schema.sql` est accepté tel quel par SQLite **et** par
  PostgreSQL 18.4 — 28 tables, 52 index, aucune erreur.
- [CONFIRMÉ] Le moteur est une affaire du serveur : le client ne voit
  jamais la base. Changer de moteur ne demande rien au poste caisse.
- [CONFIRMÉ] `amorcage.rs` crée une boutique utilisable sur PostgreSQL —
  vérifié sur une base réelle.
- [CONFIRMÉ] Les 83 scénarios `*_base.rs` passent sur PostgreSQL
  (`GESCOM_PG=… cargo test --test <fichier> -- --test-threads=1`, sur
  une base jetable, jamais celle du serveur).
- [CONFIRMÉ] Sur PostgreSQL, une commande refuse plutôt que de faire
  semblant : la sauvegarde renvoie « pg_dump sur le serveur ».
