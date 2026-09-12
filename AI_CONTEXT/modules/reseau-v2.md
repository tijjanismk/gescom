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
| [noyau/src/registre.rs](../../src-tauri/noyau/src/registre.rs) | nom → poignée `Connection` **et** poignée `Base` (`aussi_sur_base`) |
| [serveur/src/socle.rs](../../src-tauri/serveur/src/socle.rs) | les 187 commandes enregistrées ; le bloc généré vit entre marqueurs (`outils/generer_socle.py`) |
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

## Règles

- [CONFIRMÉ] Un seul processus ouvre le fichier SQLite ; un partage Windows corrompt en silence — [main.rs:5](../../src-tauri/serveur/src/main.rs#L5).
- [CONFIRMÉ] Jeton jamais en clair : bcrypt en base, jeton → session en mémoire ; un redémarrage reconnecte tous les postes — [sessions.rs:53](../../src-tauri/noyau/src/sessions.rs#L53).
- [CONFIRMÉ] La validité est relue **en base à chaque appel** : « déconnecter ce poste » agit tout de suite — `api.rs::authentifier`.
- [CONFIRMÉ] Désactiver un poste ou un utilisateur coupe ses sessions ; un poste `serveur` ou `console` **ne se désactive pas** (plus de chemin pour le rallumer) — [postes.rs](../../src-tauri/noyau/src/postes.rs).
- [CONFIRMÉ] `caisse_par_utilisateur` vaut 0 par défaut (un tiroir, D46) ; en nominatif, un index partiel interdit deux caisses au même nom, et une opération sans utilisateur est refusée (`CAISSE_SANS_UTILISATEUR`) ; on ne change pas de mode caisse ouverte — [caisses.rs](../../src-tauri/noyau/src/caisses.rs).
- [CONFIRMÉ] Le canal ne renvoie jamais un événement au poste qui l'a provoqué, et seulement si la commande a **réussi** — [canal.rs](../../src-tauri/serveur/src/canal.rs), `api.rs::rpc`.
- [CONFIRMÉ] Sur PostgreSQL, une commande sans poignée `Base` **refuse** (« pas encore disponible ») au lieu de retomber sur SQLite — D11. Aujourd'hui : 187/187 en ont une.
- [CONFIRMÉ] Le mot de passe de l'URL ne s'affiche jamais : masqué au démarrage (`main.rs::sans_mot_de_passe`), passé à `pg_dump` par `PGPASSWORD` — D10.
- [CONFIRMÉ] `portes::verifier_permission` est une liste **blanche** : une commande nouvelle est refusée aux rôles restreints par défaut.

## Ce qui reste local sur le poste

`imprimer_facture`, `imprimer_piece`, `ouvrir_avec_systeme`,
`entretenir_base`, et l'**écriture** du logo/en-tête/pied (la lecture,
elle, est servie en base64 par le serveur — D8). Une caisse qui les
appelle reçoit `commande_inconnue` : voulu, l'impression se fait sur la
machine qui a l'écran.

## Tests et essais

- [tests/reseau.rs](../../src-tauri/noyau/tests/reseau.rs) — sessions, postes, révocations, sur base en mémoire ; [registre_base.rs](../../src-tauri/noyau/tests/registre_base.rs) — le double enregistrement.
- **`outils/caisse_pg.py <url>`** — rejoue une caisse écran par écran par `/rpc` (129 clics) contre un serveur qui tourne. Sur une base **jetable**.
- Le pare-feu est vérifié depuis un second appareil (11/09/2026) ; la console n'est pas testée automatiquement — ce qu'elle appelle l'est.

## Ce qui reste

- L'écran des permissions par personne (D7) ; l'écriture des images depuis une caisse (D8) ; `entretenir_base` côté serveur (D9).
- Une vraie impression papier depuis une caisse, jamais essayée.
