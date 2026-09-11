# Module : numérotation des pièces

Rôle : donner à chaque pièce, bon de transfert et code-barre interne un
numéro qui ne sort **jamais deux fois**.

## Le bug qu'on vient de fermer

Le numéro se calculait ainsi :

```sql
SELECT COALESCE(MAX(CAST(substr(numero, -5) AS INTEGER)), 0)
FROM piece_commerciale WHERE numero LIKE 'FAC-2026-%'
```

C'est une **lecture**, et elle était faite *avant* la transaction qui
écrit la pièce. Entre les deux, une autre vente lit le même maximum.
Les deux fabriquent `FAC-2026-00042`, et la seconde se heurte à la
contrainte `UNIQUE` sur `numero`.

Au comptoir, cela ne se lit pas comme un problème de concurrence : cela
se lit comme **une facture qui refuse de s'enregistrer** pendant qu'un
client attend, sans que rien n'explique pourquoi.

### Ce n'était pas théorique

Contre-épreuve mesurée, pas supposée : le scénario du test
`deux_connexions_simultanees_ne_se_marchent_pas_dessus` (4 connexions,
25 réservations chacune, départ simultané) rejoué avec l'ancien
algorithme a produit **5 doublons sur 100 numéros**.

## Ce qui le remplace

Une table, un compteur par série :

```sql
CREATE TABLE compteur_piece (
  cle     TEXT PRIMARY KEY,   -- « FAC-2026 », « BTR-2026 », « codebarre »
  dernier INTEGER NOT NULL
);
```

Et **un seul ordre SQL** pour réserver, dans
[argent.rs](../../src-tauri/noyau/src/argent.rs) :

```sql
INSERT INTO compteur_piece (cle, dernier) VALUES (?1, 1)
ON CONFLICT(cle) DO UPDATE SET dernier = dernier + 1
RETURNING dernier
```

Un seul ordre, parce que l'incrément et sa lecture ne doivent pas
pouvoir être séparés par une autre connexion — les faire en deux temps
(`UPDATE` puis `SELECT`) rouvrirait exactement la fenêtre qu'on ferme.
C'est une **écriture** : SQLite sérialise les deux demandes, et chacune
repart avec son rang.

La clé porte la série **et l'année**, parce que la numérotation repart à
1 chaque janvier. Le code-barre interne fait exception : une seule
suite, sans année — un code collé sur un sac ne se réinitialise pas au
1er janvier.

## Fonctions exposées

- `argent::reserver_numero(conn, type_piece) -> Result<String, String>`
  — remplace `prochain_numero`. Rend `FAC-2026-00056`.
- `argent::suivant(conn, cle) -> Result<i64, String>` — le rang brut,
  pour les séries qui ne sont pas des pièces.
- `transferts::reserver_bon(conn) -> Result<String, String>` — remplace
  `prochain_bon`.
- `codebarre::reserver_sequence(conn) -> Result<u64, String>` —
  remplace `prochaine_sequence`.

⚠️ **Les quatre consomment un numéro à chaque appel.** Ce ne sont pas
des aperçus. Le changement de nom est délibéré : `prochain_numero`
laissait croire à une lecture. Aucun écran ne les appelle pour afficher
« le prochain numéro sera… » — et il ne faut pas commencer.

## Réserver dans la transaction

Un appel hors transaction consomme le numéro même si l'enregistrement
échoue ensuite : la série a alors un trou. Un trou vaut mieux qu'un
doublon, mais il est évitable — **réserver depuis l'intérieur de la
transaction** fait annuler l'incrément par le retour arrière.

C'est fait partout où une transaction existait déjà :
[achats.rs](../../src-tauri/noyau/src/achats.rs) (facture et avoir
fournisseur), [pieces.rs](../../src-tauri/noyau/src/pieces.rs) (avoir
client, et le couple BL + facture),
[transferts.rs](../../src-tauri/noyau/src/transferts.rs).

Quatre appels de `pieces.rs` restent hors transaction — ces
fonctions-là écrivent directement, sans en ouvrir une. L'insertion suit
immédiatement la réservation.

## La reprise d'une base existante

Un compteur reparti de zéro refabriquerait `FAC-2026-00001` alors que la
pièce existe : la contrainte `UNIQUE` bloquerait **la première vente du
matin de la mise à jour** — la panne la plus visible qu'on puisse
livrer.

[v2.rs](../../src-tauri/noyau/src/persistance/v2.rs) amorce donc chaque
compteur au plus grand numéro déjà émis, série par série et année par
année, en relisant les pièces, les transferts et les codes-barres. Une
seule fois, gardée par `config_app['compteurs_repris']` : rejouée, la
reprise ferait **reculer** le compteur derrière ce qui a été réservé
depuis.

Vérifié sur la base de démonstration, qui a un historique réel :

| série | plus haut numéro émis | compteur après migration |
|---|---|---|
| `FAC-2026` | 55 | 55 |
| `FAF-2026` | 13 | 13 |
| `AVF-2026` | 1 | 1 |

et la facture suivante créée par le serveur a bien reçu
`FAC-2026-00056`, la suivante `FAC-2026-00057`, tandis qu'un devis — série
neuve — partait sur `DEV-2026-00001`.

## `busy_timeout`

[persistance/mod.rs](../../src-tauri/noyau/src/persistance/mod.rs) pose
désormais `PRAGMA busy_timeout=5000`. Sans lui, une écriture qui tombe
pendant une autre rend « database is locked » **tout de suite**, au lieu
d'attendre son tour. En WAL, deux lecteurs coexistent mais deux
écrivains non — et la réservation d'un numéro est précisément une
écriture très courte que l'autre n'a qu'à laisser finir.

## Règles métier

- [CONFIRMÉ] Réserver un numéro **écrit en base**
  ([argent.rs, `suivant`](../../src-tauri/noyau/src/argent.rs)).
- [CONFIRMÉ] Une pièce supprimée ne rend pas son numéro : le compteur ne
  recule jamais (test `une_piece_supprimee_ne_rend_pas_son_numero`).
- [CONFIRMÉ] Chaque série a son compteur : un bon de livraison ne fait
  pas avancer les factures (test `chaque_serie_a_son_compteur`).
- [CONFIRMÉ] Un retour arrière rend le numéro, quand la réservation
  était dans la transaction (test `un_retour_arriere_rend_le_numero`).
- [CONFIRMÉ] La numérotation repart à 1 chaque année, sauf le
  code-barre interne.
- [CONFIRMÉ] Un compteur en retard sur les pièces émises est signalé par
  `persistance::anomalies_metier`, au démarrage — pas au comptoir
  (test `un_compteur_en_retard_est_signale`).

## Tests

[noyau/tests/numerotation.rs](../../src-tauri/noyau/tests/numerotation.rs)
— 9 scénarios. Le seul qui prouve vraiment quelque chose est
`deux_connexions_simultanees_ne_se_marchent_pas_dessus` : sur un **vrai
fichier**, pas en mémoire, où chaque connexion aurait sa propre base.
Quatre fils partent d'une `Barrier` commune et réservent 100 numéros :
tous distincts, et la suite est continue de 00001 à 00100.

## Ce que ça ne règle pas

Le serveur ne détient qu'une connexion derrière un `Mutex` : en
pratique, aujourd'hui, les ventes font la queue. Ce travail sert le jour
où il y aura un pool de connexions ou PostgreSQL.

Il sert **déjà** contre un autre cas, moins théorique : le poste qui
porte le serveur peut aussi porter une caisse, et rien n'empêche
quelqu'un d'ouvrir la base avec un autre outil pendant que la boutique
vend.
