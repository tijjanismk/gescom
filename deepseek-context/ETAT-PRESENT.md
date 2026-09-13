# État présent — 13/09/2026

## Le projet en une phrase

Gescom : logiciel de gestion commerciale pour boutiques (Bamako).
Rust (`src-tauri/noyau` = la logique, `src-tauri/serveur` = HTTP,
`src-tauri/src` = façades Tauri) + React/TypeScript (`src/`).
Montants en `i64` FCFA, **jamais de flottant sur l'argent**.
Commentaires et noms en français, sans accents dans le code.

## Les trois produits

| | état |
|---|---|
| **v1** — un poste, SQLite, pas de serveur | **livré**, ne bouge plus |
| **v2** — serveur + clients, le client ne parle **qu'**au serveur | **fonctionnelle**, sur SQLite **et** PostgreSQL |
| **v3** — multi-société, multi-dossier, exercices | fondation posée (colonne `dossier_id`, tables `dossier`/`exercice`, détecteur de requêtes non cloisonnées), écrans pas commencés |

## Le portage PostgreSQL — où on en est

- **Fini** (depuis le commit `8dd0d7a` « docs: 187/187 partout ») : les
  **193 commandes** du serveur ont leur poignée `Base` (`*_sur_base`) —
  les six dernières sont celles des images (D8, le 13/09). Vendre,
  facturer, encaisser, acheter, rendre, transférer, clôturer, relancer :
  tout est servi sur PostgreSQL.
- Le moteur se choisit par l'adresse passée au serveur :
  `gescom-serveur --base postgresql://…` → PostgreSQL ; un chemin de
  fichier → SQLite. Rien ne change côté caisse.
- Un seul fichier de schéma (`persistance/schema.sql`), dialect-neutre.
- Sauvegardes planifiées toutes les 24 h, 14 copies gardées :
  `VACUUM INTO` (SQLite) / `pg_dump --format=custom` (PG, mot de passe
  en `PGPASSWORD`, jamais en argument ni dans le dépôt — D10).
- Filet `noyau/tests/schema_commun.rs` : compare les deux chemins de
  création de base (fenêtre vs serveur) colonne par colonne.
- **Reste à faire sur PG** : (1) la fenêtre Tauri elle-même en mode
  caisse — l'essai écran par écran a été rejoué par HTTP le 13/09 sur
  base neuve (141 clics, 141 ok) mais la fenêtre n'a jamais servi ; (2) le
  déclencheur de stock multi-dossier sur SQLite — à régler avec la v3.
  (Le `pg_restore` exigé par D4 a été joué le 13/09 : dump →
  restauration → redémarrage, sans ré-amorçage.)

## Tests

- **394 tests noyau SQLite** (`cargo test -p gescom-noyau`) et
  **414 tests workspace** (`--workspace`), tous au vert (mesure du
  13/09/2026, après D9 — les chiffres précédents 389/1 073 ne se
  reproduisent pas).
- **394 tests noyau sur PostgreSQL** : suite complète du paquet sur
  `gescom_test` (103 lib + 291 dans les 31 fichiers de `tests/`),
  0 échec (13/09/2026).
- **140 scénarios** en seize fichiers `noyau/tests/*_base.rs` : chacun
  vérifie stock, caisse et créance après le geste, plus un dernier
  « tout passe le détecteur » (`base.auditer(true)`).
- SQLite : `.\outils\cargo-tenace.ps1 test --workspace`
- PostgreSQL, depuis **Bash** uniquement :
  `GESCOM_PG="postgresql://postgres:user@127.0.0.1:5432/gescom_test" cargo test -p gescom-noyau --test <fichier> -- --test-threads=1`

## Environnement de cette machine (Windows 11)

- PostgreSQL 18.4 local : `127.0.0.1:5432`, user `postgres`, mot de
  passe `user` (binaire `psql` dans `C:\Program Files\PostgreSQL\18\bin`).
- **Trois bases, à ne jamais confondre** :
  - `gescom` — la base du serveur. **Jamais `GESCOM_PG` dessus** : les
    scénarios font `DROP SCHEMA public CASCADE`.
  - `gescom_test` — jetable, pour les tests automatisés.
  - `gescom_essai` — jetable, pour les essais à la main (serveur lancé
    dessus + clics dans l'appli).
- **Smart App Control** bloque les binaires fraîchement liés (erreur
  4551) : `outils/cargo-tenace.ps1` supprime le binaire nommé et relie
  (4 essais). ⚠️ il avale `-p` et `--` → passer `--package`, et lancer
  les tests PG avec `cargo` direct depuis bash.
- PowerShell 5.1 : les `.ps1` du dépôt sont en UTF-8 **avec BOM** ; pas
  de `&&` ; `2>&1` sur un exécutable natif enveloppe la sortie en
  ErrorRecord (ne pas mettre `$ErrorActionPreference = "Stop"`).
- **Piège tauri-build (découvert le 13/09/2026)** : un
  `gescom-serveur.exe` qui tourne verrouille `target\debug\gescom-serveur.exe`
  (le sidecar copié) ; le build de l'appli (`npm run tauri dev`/`build`)
  échoue alors sur `Os { code: 5, PermissionDenied }` — ce n'est **pas**
  SAC. **Arrêter le serveur avant de builder l'appli.**
- Serveur de test PostgreSQL : **en cours** sur le port 7300, base
  `gescom_essai` **recréée à neuf le 13/09** + démo (8 articles, 4 clients),
  comptes `admin/admin123` et `employe/employe123` (les deux exigent un
  changement de mot de passe à la première connexion — `caisse_pg.py`
  l'a déjà rejoué pour `admin`). Binaire release reconstruit le 13/09
  **après D9 et la correction d'interblocage** : 193 commandes, plus
  de message ⚠ périmé, il porte `--promouvoir` et la route
  `POST /entretien` répond.
- Une instance `gescom.exe` (l'appli Tauri) peut tourner (PID 32208 au
  13/09) — vérifier avant d'ouvrir un serveur sur le port 7300.
- Pare-feu : `outils/parefeu.ps1 -Ouvrir` (admin) pour que les autres
  postes joignent le port 7300.

## Les lots du 13/09/2026 — **commités** (D6 `a006dd3`, D7 `4e3fb5b`, D8 `d707d51`, D9 `f2b29d8`)

1. **`--promouvoir`** (D6) : commande du serveur qui redonne
   `superadmin` à un compte. 3 scénarios d'`auth_base.rs`, vérifié en
   vrai sur PostgreSQL jetable.
2. **L'écran permissions par personne** (D7) :
   `ModalPermissionsUtilisateur.tsx`, trois états (rôle / autorisée /
   refusée), bouton dans Paramètres → Utilisateurs. Chemin HTTP rejoué
   contre le serveur PG. Le **rendu dans la fenêtre Tauri** n'a pas
   encore été vu (item 3 du plan, avec l'utilisateur).
3. **Les images depuis une caisse** (D8) : contenu en base64, jamais de
   chemin ; validation partagée dans `noyau/src/images.rs` ; 6 commandes
   de plus au serveur (**193**) ; `supprimer_*` efface aussi le fichier ;
   `CORPS_MAX` 8 → 16 Mio. 10 scénarios `images_base.rs` sur les deux
   moteurs ; rejeu HTTP **141/141** sur base neuve.
4. **L'entretien devient un travail du serveur** (D9) : route
   `POST /entretien` (permission `sauvegarde:lancer`, verrou gardé
   pendant l'opération), console → carte Administration avec le bouton
   Entretien, l'écran des caisses garde le diagnostic seulement.
   5 scénarios `entretien_base.rs` sur les deux moteurs. **Vérifié par
   HTTP** : 401 sans jeton, ok avec (copie `pg_dump` lisible, trace
   journal `origine = serveur`). La vérification a attrapé un
   **interblocage** (`sauvegarde::dossier` reprend le verrou que le
   gestionnaire tenait) — corrigé dans `serveur/src/api.rs` (le dossier
   se calcule avant le verrou), **non encore commité**.

## `--promouvoir` — le geste de secours (fait le 13/09/2026)

D6 réglé : `gescom-serveur --base <cible> --promouvoir IDENTIFIANT`
redonne le rôle `superadmin` à un compte existant, depuis la machine
du serveur, puis s'arrête. Refus « Aucun compte » (exit 1) ;
idempotent (« porte déjà ») ; écrit `role_change` au journal. Vérifié
sur les deux moteurs par 3 scénarios d'`auth_base.rs` et en vrai sur
une base PostgreSQL jetable.

Le binaire **release** qui tourne sur le port 7300 a été reconstruit
le 13/09 **avec** `--promouvoir` et D8 : le geste de secours est
utilisable tel quel (`gescom-serveur --base <cible> --promouvoir admin`).
Arrêter le serveur du port 7300 avant de rebuilder : il verrouille le
sidecar `target\debug\gescom-serveur.exe`.

## Dernier état git

Branche `main`. Commités le 13/09/2026, un par lot : D6 `a006dd3`
(`--promouvoir`), D7 `4e3fb5b` (écran permissions), D8 `d707d51`
(images par le serveur), D9 `f2b29d8` (entretien au serveur).
**Non commité** (règle de l'utilisateur : demander avant de
commiter) : la correction d'interblocage (`serveur/src/api.rs`,
attrapée par la vérification HTTP de D9), les docs de séance
(`AI_CONTEXT/*`, `deepseek-context/`), `caisse_pg.py`.
