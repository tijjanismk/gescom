# Module : réseau (v2 — multiposte)

Rôle : plusieurs postes autour d'un serveur qui détient la base.
Sessions, canal d'événements, sauvegardes, caisse par utilisateur.
**Un seul chemin** : le client parle au serveur, jamais à une base
locale — même sur la machine qui porte le serveur (`127.0.0.1:7300`).
Deux pannes réelles ont appris que le repli silencieux est pire que
l'arrêt (récit : [JOURNAL.md](../JOURNAL.md), [DECISIONS.md](../DECISIONS.md) D11).

## Fichiers

| | rôle |
|---|---|
| [noyau/src/protocole.rs](../../src-tauri/noyau/src/protocole.rs) | le contrat client/serveur, compilé des deux côtés — un champ renommé casse l'autre |
| [noyau/src/sessions.rs](../../src-tauri/noyau/src/sessions.rs), [postes.rs](../../src-tauri/noyau/src/postes.rs), [caisses.rs](../../src-tauri/noyau/src/caisses.rs) | jeton/expiration/révocation ; les machines ; quel tiroir pour qui |
| [noyau/src/registre.rs](../../src-tauri/noyau/src/registre.rs) | nom → poignée `Connection` **et** poignée `Base` (`aussi_sur_base`) ; `sur_base` pour une commande née sur `Base` seule (v3), servie par la `Base` sur les deux moteurs |
| [serveur/src/service.rs](../../src-tauri/serveur/src/service.rs) | le service Windows (D12) : installer, désinstaller, tourner sous le gestionnaire, journal dans `ProgramData` |
| [serveur/src/socle.rs](../../src-tauri/serveur/src/socle.rs) | les 210 commandes enregistrées ; le bloc généré vit entre marqueurs (`outils/generer_socle.py`) |
| [serveur/src/http.rs](../../src-tauri/serveur/src/http.rs), [api.rs](../../src-tauri/serveur/src/api.rs), [canal.rs](../../src-tauri/serveur/src/canal.rs) | HTTP/1.1 minimal sans dépendance (ni TLS ni keep-alive) ; les routes ; la longue attente |
| [serveur/src/sauvegarde.rs](../../src-tauri/serveur/src/sauvegarde.rs) | toutes les 24 h, 14 copies : `VACUUM INTO` (SQLite) ou `pg_dump` (PostgreSQL) |
| [serveur/src/console.rs](../../src-tauri/serveur/src/console.rs), [reseau_local.rs](../../src-tauri/serveur/src/reseau_local.rs) | la console (une page HTML dans une constante) ; adresse à saisir et état du pare-feu au démarrage |
| [src/lib/pont.ts](../../src/lib/pont.ts) | **le** pont côté écran : `invoke` → `POST /rpc` ; la liste `LOCALES` des commandes qui restent sur le poste (imprimer, ouvrir un fichier, régler le réseau) |

Trois crates : `noyau` (lib, sans Tauri), `serveur` (`gescom-serveur.exe`),
`src-tauri` (la fenêtre, façades `commandes/*.rs` qui délèguent au noyau).

## Routes

| | route | jeton | effet |
|---|---|---|---|
| GET | `/`, `/console` | non | la console |
| GET | `/sante` | non | version, postes connectés, intégrité |
| POST | `/connexion` | non | bcrypt → jeton 8 h ; **HTTP 426** si la version de protocole diffère |
| POST | `/deconnexion` | oui | révoque la session |
| POST | `/rpc` | oui | `{commande, params}` → `{etat: ok, donnee}` ou `{etat: erreur, code, message}` |
| GET | `/rpc/catalogue` | non | les noms connus |
| GET | `/canal?depuis=N` | oui | longue attente 30 s |
| POST | `/sauvegarde` | oui | une sauvegarde tout de suite |
| POST | `/entretien` | oui | l'entretien (D9) : réaffecte les règlements globaux, réindexe, compacte — une copie avant ; les caisses patientent |

## Règles

- [CONFIRMÉ] Un seul processus ouvre le fichier SQLite ; un partage Windows corrompt en silence — [main.rs:5](../../src-tauri/serveur/src/main.rs#L5).
- [CONFIRMÉ] Jeton jamais en clair : bcrypt en base, jeton → session en mémoire ; un redémarrage reconnecte tous les postes — [sessions.rs:53](../../src-tauri/noyau/src/sessions.rs#L53).
- [CONFIRMÉ] La validité est relue **en base à chaque appel** : « déconnecter ce poste » agit tout de suite — `api.rs::authentifier`.
- [CONFIRMÉ] Désactiver un poste ou un utilisateur coupe ses sessions ; un poste `serveur` ou `console` **ne se désactive pas** (plus de chemin pour le rallumer) — [postes.rs](../../src-tauri/noyau/src/postes.rs).
- [CONFIRMÉ] `caisse_par_utilisateur` vaut 0 par défaut (un tiroir, D46) ; en nominatif, un index partiel interdit deux caisses au même nom, et une opération sans utilisateur est refusée (`CAISSE_SANS_UTILISATEUR`) ; on ne change pas de mode caisse ouverte — [caisses.rs](../../src-tauri/noyau/src/caisses.rs).
- [CONFIRMÉ] Le canal ne renvoie jamais un événement au poste qui l'a provoqué, et seulement si la commande a **réussi** — [canal.rs](../../src-tauri/serveur/src/canal.rs), `api.rs::rpc`.
- [CONFIRMÉ] Sur PostgreSQL, une commande sans poignée `Base` **refuse** (« pas encore disponible ») au lieu de retomber sur SQLite — D11. Aujourd'hui : 193/193 en ont une.
- [CONFIRMÉ] Le mot de passe de l'URL ne s'affiche jamais : masqué au démarrage (`main.rs::sans_mot_de_passe`), passé à `pg_dump` par `PGPASSWORD` — D10.
- [CONFIRMÉ] `portes::verifier_permission` est une liste **blanche** : une commande nouvelle est refusée aux rôles restreints par défaut.

## Ce qui reste local sur le poste

`imprimer_facture`, `imprimer_piece`, `ouvrir_avec_systeme`,
les réglages réseau et l'import/export de modèles —
la liste exacte est `pont.ts::LOCALES`. Une caisse qui les appelle
reçoit `commande_inconnue` : voulu, l'impression se fait sur la machine
qui a l'écran. Les images, elles, ne sont plus locales : **écriture et
lecture** passent par le serveur (D8). L'entretien non plus : ni
commande ni local, c'est la route `POST /entretien` du serveur, sous la
même permission que la sauvegarde (D9).

## Tests et essais

- [tests/reseau.rs](../../src-tauri/noyau/tests/reseau.rs) — sessions, postes, révocations, sur base en mémoire ; [registre_base.rs](../../src-tauri/noyau/tests/registre_base.rs) — le double enregistrement.
- **`outils/caisse_pg.py <url>`** — rejoue une caisse écran par écran par `/rpc` (141 clics) contre un serveur qui tourne. Sur une base **jetable**.
- Le pare-feu est vérifié depuis un second appareil (11/09/2026) ; la console n'est pas testée automatiquement — ce qu'elle appelle l'est. `POST /entretien` a été joué par HTTP le 13/09 (401 sans jeton, ok avec, dump lisible) : il a attrapé un interblocage (`sauvegarde::dossier` reprenait le verrou que le gestionnaire tenait), corrigé — récit dans [JOURNAL.md](../JOURNAL.md).

## La v3 dans le protocole (19/09/2026, D13)

`DemandeConnexion.dossier_id` / `memoriser_dossier` ; `Identite.dossier_id`
(`null` = à choisir), `dossier_societe`, `dossiers` (les ouverts).
`CodeErreur::DossierAChoisir` sur toute commande d'une session sans
dossier, sauf `lire_dossiers` et `choisir_dossier`. `Appelant` porte
`dossier_id` et `session_id` ; `api::rpc` fait `base.choisir_dossier`
avant chaque poignée `Base`. Commandes : `lire_dossiers`,
`creer_dossier` (`dossiers:gerer`), `choisir_dossier`,
`oublier_dossier_memorise`, `lire_exercices`, `ouvrir_exercice`,
`prolonger_exercice`, `clore_exercice`. L'écran : `PageLogin.tsx`
demande « Quel dossier ouvrir ? » quand il y en a plusieurs ;
`Layout` affiche le dossier ouvert sous le nom.

## Ce qui reste

- Une vraie impression papier depuis une caisse, jamais essayée.
- Le service Windows n'a pas été déroulé en élevé depuis la session
  d'écriture (TESTS-MANUELS, section A).
