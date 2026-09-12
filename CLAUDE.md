# Gescom — ce qu'il faut savoir avant d'écrire une ligne

Logiciel de gestion commerciale pour boutiques (Bamako). Rust
(`src-tauri/noyau` = la logique, `src-tauri/serveur` = HTTP,
`src-tauri/src` = façades Tauri) + React/TypeScript (`src/`). Montants
en `i64` FCFA, jamais de flottant sur l'argent. Commentaires et noms en
français, sans accents dans le code.

## Où lire, selon la tâche

Ne charge pas tout `AI_CONTEXT/`. Le routeur : [AI_CONTEXT/README.md](AI_CONTEXT/README.md).
Avant de corriger quoi que ce soit : [AI_CONTEXT/ALERTES.md](AI_CONTEXT/ALERTES.md)
(deux arbres du même nom, lequel ouvrir). État du projet :
[AI_CONTEXT/ETAPES.md](AI_CONTEXT/ETAPES.md), une page.

## Règles de code — non négociables

1. **La logique vit dans `noyau/src/<module>.rs`**, jamais dans
   `src-tauri/src/commandes/` (façades) ni dans le serveur.
2. **Toute commande existe en deux versions** : `fn(conn: &Connection)`
   et `fn_sur_base(base: &mut Base)`. Une correction se fait dans les
   deux. Une aide partagée prend `&mut impl Acces` (sert dedans et hors
   transaction), pas `&mut Base`.
3. **SQL portable, une seule fois** — jamais deux variantes par moteur :
   - `dossier_id` sur chaque table cloisonnée, en `WHERE` et en `INSERT` ;
   - paramètres liés, **jamais `format!` sur une valeur** ; filtre
     optionnel = `(CAST(?n AS TEXT) IS NULL OR col = ?n)` ;
   - `LOWER(x) LIKE LOWER(...)` (LIKE est sensible à la casse sur PostgreSQL) ;
   - `CAST(SUM(...) AS BIGINT)` — `SUM` rend `NUMERIC` sur PostgreSQL ;
   - dates ISO : comparer par `SUBSTR(x, 1, 10)` ; durées en Rust
     (`utils::jours_depuis`), pas `julianday`/`strftime`/`date('now')` ;
   - `ON CONFLICT (...) DO NOTHING|UPDATE`, pas `INSERT OR IGNORE` ;
   - pas d'alias du SELECT dans `HAVING`, pas de `GROUP BY` sans agrégat.
4. **Ce qui écrit plusieurs lignes tourne dans une transaction**
   (`base.transaction()` … `tx.valider()`), le numéro de pièce réservé
   DEDANS (`reserver_numero_sur`). L'argent sort du tiroir → caisse
   ouverte exigée AVANT d'écrire (`caisses::exiger_sur`, code
   `CAISSE_FERMEE`).
5. **Refuser plutôt que faire semblant** : une commande qui ne sait pas
   faire quelque chose sur un moteur rend une erreur qui le dit.
6. Une règle métier vit dans `coeur/` (pure, testée) et s'appelle des deux
   versions ; on ne la recopie pas en SQL sauf pour un agrégat, et alors
   la constante vient de `coeur`.
7. Les décisions numérotées (D1…D51) ne se rediscutent pas dans le code :
   [AI_CONTEXT/DECISIONS.md](AI_CONTEXT/DECISIONS.md).

## Tests

```bash
.\outils\cargo-tenace.ps1 test --workspace        # SQLite ; cargo nu est bloqué par Smart App Control (4551)
GESCOM_PG="postgresql://…/gescom_test" cargo test -p gescom-noyau --test <fichier> -- --test-threads=1
```

- **Jamais `GESCOM_PG` sur la base du serveur** (`gescom`) : les
  scénarios font `DROP SCHEMA`. La base jetable est `gescom_test`.
- Un module porté a son fichier `noyau/tests/<lot>_base.rs` : des
  scénarios qui vérifient **stock, caisse et créance après le geste**,
  et un dernier test « tout passe le détecteur » (`base.auditer(true)`).
- Toute colonne ajoutée va dans `persistance/schema.sql` **et** dans la
  liste idempotente d'`amorcage.rs` ; `tests/schema_commun.rs` compare
  les deux chemins et nomme ce qui manque.

## Après une séance

Une ligne dans le tableau d'`ETAPES.md`, une section datée dans
`JOURNAL.md`, la fiche `modules/*.md` touchée si une signature ou une
règle a bougé. Commits en français, un par lot, message = le pourquoi.
