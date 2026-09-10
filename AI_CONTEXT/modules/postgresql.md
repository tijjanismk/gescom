# Module : portage PostgreSQL

Rôle : rendre le serveur capable de tenir la base dans PostgreSQL,
**sans jamais l'imposer au monoposte**.

## La contrainte qui décide de tout

`ARCHITECTURE.md` le pose : *« Le monoposte reste le défaut et ne
demande aucun service. »* Un commerçant avec une seule caisse ne doit
pas installer PostgreSQL pour vendre un sac de ciment.

PostgreSQL est donc une **option du serveur**, pas un remplacement de
SQLite. Les deux moteurs doivent coexister dans le même binaire, et
SQLite reste le chemin par défaut.

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

## Ce qui coûte vraiment, et qui reste à faire

Les 1448 placeholders **ne se réécrivent pas à la main** : ils se
traduisent au moment de l'appel, dans la couche qui parle au moteur.
`?1` → `$1` est une substitution sûre — c'est une des rares choses dont
un traducteur textuel peut se charger sans risque.

Le vrai poids est la **lecture des lignes**. Chacune des 578 requêtes
lit ses colonnes par une fermeture `|r| r.get::<_, String>(0)`, et
`rusqlite::Row` n'est pas `postgres::Row`. Il faut un type de ligne
commun, donc toucher les 578 fermetures.

Suite prévue, dans l'ordre :

1. Une façade `Base` qui enveloppe l'un ou l'autre moteur et expose
   `execute` / `query_row` / `query_map` / `transaction`, avec la
   traduction `?N` → `$N` faite au passage.
2. Les ~60 occurrences non mécaniques, réécrites en SQL portable quand
   c'est possible plutôt qu'en deux variantes.
3. La sauvegarde : `VACUUM INTO` n'existe pas côté PostgreSQL, il
   faudra `pg_dump` ou une copie logique.
4. Le choix du moteur dans `poste.json`, avec SQLite par défaut.

## Le raccordement

La chaîne de connexion ne va **pas dans le dépôt** : mot de passe. Elle
se lira dans `poste.json` ou dans une variable d'environnement, comme
le chemin de la base aujourd'hui.

Pour les essais en développement, PostgreSQL 18.4 tourne en local sur
le port 5432 et la base d'essai s'appelle `gescom_essai`.

## Règles métier

- [CONFIRMÉ] `schema.sql` est accepté tel quel par SQLite **et** par
  PostgreSQL 18.4 — 28 tables, 52 index, aucune erreur.
- [DÉDUIT] PostgreSQL ne concernera que le serveur ; le monoposte
  restera sur SQLite sans service à installer. Déduit de la contrainte
  posée dans `ARCHITECTURE.md`, pas encore inscrit dans le code.
