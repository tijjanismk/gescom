# Module : numérotation

Rôle : donner à chaque pièce, bon de transfert et code-barre un numéro
**unique et continu**, même quand plusieurs connexions en demandent en
même temps. L'ancien calcul (`MAX(numero) + 1`) sortait 5 doublons sur
100 sous quatre connexions — mesuré, pas théorique
([JOURNAL.md](../JOURNAL.md)).

## Comment

Une table `compteur_piece(cle, dernier)` et **un seul ordre SQL** que
les deux moteurs acceptent :

```sql
INSERT INTO compteur_piece (cle, dernier) VALUES (?1, 1)
ON CONFLICT (cle) DO UPDATE SET dernier = compteur_piece.dernier + 1
RETURNING dernier
```

La clé porte le **dossier** (`dossiers::cle_compteur`) : deux sociétés
ont chacune leur suite. Le préfixe vient d'un seul endroit,
`argent::prefixe_de` (`FAC`, `DEV`, `CMD`, `BL`, `AVC`, `BCF`, `BRF`,
`FAF`, `AVF`…) ; la série repart à 1 chaque année, sauf le code-barre
interne.

## Fonctions

| | rend | note |
|---|---|---|
| `argent::reserver_numero(conn, type)` / `reserver_numero_sur(acces, type)` | `FAC-2026-00056` | |
| `argent::suivant(conn, cle)` / `suivant_sur(acces, cle)` | le rang brut | pour les séries qui ne sont pas des pièces |
| `transferts::reserver_bon` / `reserver_bon_sur` | `BTR-2026-00007` | |
| `codebarre::reserver_sequence` / `reserver_sequence_sur` | `u64` | une seule suite, sans année |

⚠️ **Chacune consomme un numéro à chaque appel.** Ce ne sont pas des
aperçus ; aucun écran ne doit afficher « le prochain sera… ». Les
versions `_sur` prennent `&mut impl Acces` : **réserver DANS la
transaction**, pour qu'un échec plus bas rende le numéro par le retour
arrière — un trou vaut mieux qu'un doublon, mais il est évitable.

## Règles

- [CONFIRMÉ] Réserver **écrit en base** — [argent.rs, `suivant_sur`](../../src-tauri/noyau/src/argent.rs).
- [CONFIRMÉ] Le compteur ne recule jamais : une pièce supprimée ne rend pas son numéro (test `une_piece_supprimee_ne_rend_pas_son_numero`).
- [CONFIRMÉ] Chaque série a son compteur ; un BL ne fait pas avancer les factures (`chaque_serie_a_son_compteur`).
- [CONFIRMÉ] Un retour arrière rend le numéro si la réservation était dans la transaction (`un_retour_arriere_rend_le_numero`).
- [CONFIRMÉ] Un compteur en retard sur les pièces émises est signalé au démarrage par `persistance::anomalies_metier`, pas au comptoir (`un_compteur_en_retard_est_signale`). Sur PostgreSQL, ce contrôle n'est pas porté (`substr(x, -5)`) — [postgresql.md](postgresql.md).
- [CONFIRMÉ] SQLite : `PRAGMA busy_timeout=5000` — une écriture qui tombe pendant une autre attend son tour au lieu de rendre « database is locked » ([persistance/mod.rs](../../src-tauri/noyau/src/persistance/mod.rs)).

## Tests

[tests/numerotation.rs](../../src-tauri/noyau/tests/numerotation.rs) —
9 scénarios ; celui qui prouve quelque chose est
`deux_connexions_simultanees_ne_se_marchent_pas_dessus` : quatre fils,
un vrai fichier, 100 numéros, tous distincts et continus. Le même
scénario tourne sur PostgreSQL ([postgres_amorcage.rs](../../src-tauri/noyau/tests/postgres_amorcage.rs),
`deux_caisses_qui_facturent_en_meme_temps`).
