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

### Ce qui marche aujourd'hui sur PostgreSQL

[`noyau/src/amorcage.rs`](../../src-tauri/noyau/src/amorcage.rs) crée le
schéma, pose les six rôles avec leurs permissions, crée les comptes
`admin` et `employe`, le dépôt par défaut et le client « Comptant ».

Mesuré sur une base réelle (`postgresql://…/gescom`) : **30 tables, 6
rôles, 2 comptes, 1 dépôt**, et l'amorçage ne se rejoue pas.

⚠️ **Les 739 points d'appel métier parlent encore rusqlite.** Vendre,
facturer, encaisser passent par SQLite. PostgreSQL sait aujourd'hui
recevoir une boutique neuve — il ne sait pas encore la faire tourner.

## Ce qui coûte vraiment, et qui reste à faire

Les 1448 placeholders **ne se réécrivent pas à la main** : ils se
traduisent au moment de l'appel, dans la couche qui parle au moteur.
`?1` → `$1` est une substitution sûre — c'est une des rares choses dont
un traducteur textuel peut se charger sans risque.

Le vrai poids est la **lecture des lignes**. Chacune des 578 requêtes
lit ses colonnes par une fermeture `|r| r.get::<_, String>(0)`, et
`rusqlite::Row` n'est pas `postgres::Row`. Il faut un type de ligne
commun, donc toucher les 578 fermetures.

Mesure refaite sur le noyau, précisément :

| | |
|---|---|
| paramètres `conn:` à changer | 242 |
| `params!` à convertir | 480 |
| fermetures de lecture | 435 |
| points d'appel (`execute`, `query_row`, `query_map`, `prepare`) | **739** |

Suite, dans l'ordre :

1. Porter **module par module**. La façade permet de le faire sans tout
   casser : ce qui n'est pas porté continue de tourner sur SQLite.
   Commencer par `auth` et `sessions` — de quoi se connecter — puis
   `catalogue`, puis `argent`.
2. Les ~60 occurrences non mécaniques, réécrites en SQL portable quand
   c'est possible plutôt qu'en deux variantes.
3. La sauvegarde : `VACUUM INTO` n'existe pas côté PostgreSQL, il
   faudra `pg_dump` ou une copie logique.
4. Le multi-dossier, **avant** d'avoir tout porté : le découpage décide
   de la forme de la base, et l'ajouter après obligerait à reprendre les
   739 points d'appel une seconde fois.

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
