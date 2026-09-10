# Module : réseau (v2 — multiposte)

Rôle : faire tourner Gescom sur plusieurs postes autour d'un serveur qui
détient la base. Sessions, canal d'événements, sauvegardes, caisse par
utilisateur.

⚠️ **Fondations posées, migration non faite.** Le serveur exécute
**7** commandes sur les 174. Les 167 autres vivent encore dans
`commandes/`, en `#[tauri::command]`, et ne fonctionnent qu'en
monoposte. Un poste caisse qui les appelle reçoit `commande_inconnue`.

## Les trois crates

`src-tauri/Cargo.toml` est devenu la racine d'un workspace.

| Crate | Produit | Connaît Tauri |
|---|---|---|
| [noyau/](../../src-tauri/noyau/) | `gescom-noyau` (lib) | non |
| [serveur/](../../src-tauri/serveur/) | `gescom-serveur.exe` | non |
| `src-tauri/` (`gescom`) | `Gescom.exe` (fenêtre) | oui |

`coeur/`, `persistance/`, `utils.rs` et `portes.rs` ont **déménagé** de
`src-tauri/src/` vers `src-tauri/noyau/src/`. `lib.rs` les ré-exporte
(`pub use gescom_noyau::{caisses, coeur, persistance, portes, utils}`),
donc `crate::coeur::…` résout toujours dans les 27 fichiers de
commandes : aucun d'eux n'a été touché.

## Fichiers du noyau

- [protocole.rs](../../src-tauri/noyau/src/protocole.rs) — le contrat
  client/serveur. Compilé dans les deux exécutables : un champ renommé
  d'un côté casse la compilation de l'autre.
- [sessions.rs](../../src-tauri/noyau/src/sessions.rs) — jeton, expiration,
  révocation.
- [postes.rs](../../src-tauri/noyau/src/postes.rs) — les machines.
- [caisses.rs](../../src-tauri/noyau/src/caisses.rs) — quel tiroir, pour qui.
- [registre.rs](../../src-tauri/noyau/src/registre.rs) — nom → poignée.
- [persistance/v2.rs](../../src-tauri/noyau/src/persistance/v2.rs) — migrations.

## Fichiers du serveur

- [main.rs](../../src-tauri/serveur/src/main.rs) — options, base, boucle
  d'acceptation, un fil par connexion.
- [http.rs](../../src-tauri/serveur/src/http.rs) — HTTP/1.1 minimal,
  **zéro dépendance**. Ni TLS, ni keep-alive, ni HTTP/2.
- [api.rs](../../src-tauri/serveur/src/api.rs) — les routes.
- [canal.rs](../../src-tauri/serveur/src/canal.rs) — longue attente.
- [sauvegarde.rs](../../src-tauri/serveur/src/sauvegarde.rs) — `VACUUM INTO`
  toutes les 24 h, 14 copies conservées.
- [socle.rs](../../src-tauri/serveur/src/socle.rs) — **le point d'entrée de
  la migration** : c'est ici qu'on enregistre les commandes portées.

## Routes

| Méthode | Route | Jeton | Effet |
|---|---|---|---|
| GET | `/sante` | non | version, postes connectés, intégrité |
| POST | `/connexion` | non | bcrypt → jeton 8 h |
| POST | `/deconnexion` | oui | révoque la session |
| POST | `/rpc` | oui | exécute une commande du registre |
| GET | `/rpc/catalogue` | non | les noms connus du serveur |
| GET | `/canal?depuis=N` | oui | longue attente, 30 s max |
| POST | `/sauvegarde` | oui | `VACUUM INTO` immédiat |

## Front

Un seul point de bascule : [pont.ts](../../src/lib/pont.ts). Les 39
fichiers qui appelaient `invoke` importent désormais
`import { appeler as invoke } from "@/lib/pont"` — **les 325 appels sont
inchangés**. `appeler` rejette avec une *chaîne*, comme `invoke`, pour ne
pas préfixer « Error: » les messages métier affichés au commerçant.

Plus aucun fichier de `src/` n'importe `@tauri-apps/api/core` hors
`pont.ts`.

## Règles métier

- [CONFIRMÉ] Un seul processus ouvre le fichier SQLite. Deux machines
  l'ouvrant par un partage Windows corrompent silencieusement — le
  verrouillage SQLite ne traverse pas SMB de façon fiable
  ([main.rs:5](../../src-tauri/serveur/src/main.rs#L5)).
- [CONFIRMÉ] Le jeton n'est jamais stocké en clair : hash bcrypt en base,
  correspondance jeton → session en mémoire du serveur
  ([sessions.rs:53](../../src-tauri/noyau/src/sessions.rs#L53)). Conséquence
  assumée : un redémarrage du serveur reconnecte tous les postes
  ([main.rs:85](../../src-tauri/serveur/src/main.rs#L85)).
- [CONFIRMÉ] La validité est relue **en base** à chaque appel, jamais
  dans le cache mémoire : sinon « déconnecter ce poste » ne prendrait
  effet qu'au redémarrage
  ([api.rs, `authentifier`](../../src-tauri/serveur/src/api.rs)).
- [CONFIRMÉ] Désactiver un poste, ou désactiver un utilisateur, coupe
  ses sessions immédiatement (tests `desactiver_un_poste_coupe_ses_sessions`,
  `desactiver_un_utilisateur_coupe_sa_session`).
- [CONFIRMÉ] `caisse_par_utilisateur` vaut **0** par défaut : le
  comportement du v1 (D46, un seul tiroir) est conservé tel quel
  ([v2.rs](../../src-tauri/noyau/src/persistance/v2.rs)).
- [CONFIRMÉ] En mode nominatif, un index partiel interdit deux caisses
  ouvertes au même nom, et une opération sans utilisateur est refusée
  (`CAISSE_SANS_UTILISATEUR`) plutôt que rattachée au premier tiroir venu.
- [CONFIRMÉ] On ne change pas de mode de caisse tant qu'une caisse est
  ouverte ([caisses.rs](../../src-tauri/noyau/src/caisses.rs)).
- [CONFIRMÉ] Une version de protocole différente est **refusée** à la
  connexion (HTTP 426) : laisser passer ferait lire des champs absents
  comme des zéros, donc des montants faux sans message d'erreur.
- [CONFIRMÉ] Le canal ne renvoie jamais un événement au poste qui l'a
  provoqué — il rechargerait par-dessus une saisie en cours
  ([canal.rs](../../src-tauri/serveur/src/canal.rs)).
- [CONFIRMÉ] Un événement n'est publié que si la commande a **réussi**
  ([api.rs, `rpc`](../../src-tauri/serveur/src/api.rs)).
- [CONFIRMÉ] `portes::verifier_permission` est une liste **blanche** : une
  commande ajoutée demain est refusée par défaut aux rôles restreints
  (test `une_permission_inconnue_est_refusee`).

## Tests

[noyau/tests/reseau.rs](../../src-tauri/noyau/tests/reseau.rs) — 11
scénarios sur base en mémoire. Plus 4 tests de permissions dans
`portes.rs`. Ce sont les seuls tests du multiposte : le socle en a,
les 167 commandes à migrer n'en ont toujours pas.

## Ce qui reste à faire

1. Porter les commandes vers `socle.rs`, **par domaine et avec leur
   test** — en commençant par `creer_vente`, `valider_facture`,
   `regler_dette_fournisseur`.
2. Remplacer `stock_depot.quantite` (compteur muté) par une somme de
   `mouvement_stock`. ⚠️ **Pas encore urgent** : le serveur ne détient
   qu'une connexion derrière un `Mutex`, donc deux ventes ne s'exécutent
   jamais en même temps — elles font la queue. La course n'apparaîtra
   qu'avec un pool de connexions ou PostgreSQL. Le gain immédiat est
   ailleurs : un compteur ne dit pas *pourquoi* il vaut ça, une somme
   de mouvements si.
3. Remplacer la numérotation par `MAX(substr(numero,-5))` par un compteur
   transactionnel : sous concurrence elle produit deux FAC-00042.
4. Un écran de réglage réseau (mode, adresse, liste des postes) — les
   commandes Rust existent
   ([reseau.rs](../../src-tauri/src/reseau.rs)), l'écran non.
