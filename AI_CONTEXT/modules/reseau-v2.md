# Module : réseau (v2 — multiposte)

Rôle : faire tourner Gescom sur plusieurs postes autour d'un serveur qui
détient la base. Sessions, canal d'événements, sauvegardes, caisse par
utilisateur.

**Migration terminée.** Le serveur exécute **172** commandes sur 182.

Les **10** restantes ne peuvent pas quitter le crate applicatif : elles
ouvrent une fenêtre Tauri ou lisent un chemin d'application —
`imprimer_facture`, `imprimer_piece`, `ouvrir_avec_systeme`,
`entretenir_base`, et les six du logo, de l'en-tête et du pied.
Un poste caisse qui les appelle reçoit `commande_inconnue` ; c'est
voulu, l'impression et les images se font sur la machine qui a l'écran.

## Comment les 141 dernières ont été portées

À la main, ç'aurait été sept mille lignes recopiées. Deux outils l'ont
fait, et c'est leur emploi qui rend la chose défendable :

- un **porteur** déplace un fichier de `commandes/` vers `noyau/` en
  coupant le texte, jamais en le récrivant : la signature perd son
  `State<EtatApp>` au profit d'une `&Connection`, le verrou disparaît,
  et une façade Tauri mince reste derrière ;
- [outils/generer_socle.py](../../outils/generer_socle.py) lit ces
  façades et écrit les poignées du serveur. Chaque façade dit son nom,
  son module et ses paramètres — retaper 141 signatures aurait été long
  et faux quelque part.

Le bloc généré vit entre deux marqueurs dans `socle.rs`. Les 31
poignées écrites à la main, au-dessus, ne sont pas touchées : ce sont
celles qui demandent un traitement particulier. Les autres vivent encore dans `commandes/`, en
`#[tauri::command]`, et ne fonctionnent qu'en monoposte. Un poste
caisse qui les appelle reçoit `commande_inconnue`.

Le comptoir porté : `lire_clients`, `lire_client_generique`,
`lire_depots`, `lire_depot_defaut`, `lire_articles_avec_unites` —
logique dans [noyau/src/catalogue.rs](../../src-tauri/noyau/src/catalogue.rs),
appelée par la façade Tauri comme par le serveur. Ce sont les cinq
commandes qu'une caisse appelle avant de pouvoir afficher quoi que ce
soit ; porter `creer_vente` d'abord n'aurait servi à rien, on ne vend
pas à un client qu'on ne voit pas.

[CONFIRMÉ] Le **rôle vient de la session**, jamais des paramètres
d'appel : un poste qui enverrait « patron » dans son JSON lirait les
prix d'achat. Le filtre §7 est donc appliqué côté serveur
([socle.rs](../../src-tauri/serveur/src/socle.rs), test
`le_prix_dachat_ne_sort_que_pour_le_patron`).

## Le domaine argent

`creer_vente`, `valider_facture` et `creer_facture_depuis_vente` vivent
maintenant dans [noyau/src/argent.rs](../../src-tauri/noyau/src/argent.rs),
avec `prochain_numero`, `lire_lignes_raw` et les deux résolveurs
d'utilisateur.

**Le code a déménagé, il n'a pas été récrit.** Ce sont les deux endroits
où le stock sort et où l'argent entre ; une réécriture fidèle en
apparence est exactement ce qui coûte ses livres à un commerçant. Le
filet : les 26 scénarios de `tests_multi_depot` continuent de les jouer
à travers les façades restées dans `commandes/`, et vérifient ce que le
SQL fait réellement à la base.

- [CONFIRMÉ] `Contexte.conn` est **mutable** :
  `Connection::transaction` l'exige, et toute écriture d'argent ouvre
  une transaction. Les lectures s'en accommodent — un `&mut` se reprête
  en `&` ([registre.rs](../../src-tauri/noyau/src/registre.rs)).
- [CONFIRMÉ] Le verrou reste celui du serveur : une seule commande
  s'exécute à la fois. C'est ce qui rend le multiposte sûr **sans**
  avoir touché aux compteurs de stock.
- [CONFIRMÉ] `enregistrer_paiement` et `regler_dette_fournisseur`
  n'ouvrent **pas** de transaction — c'est le code d'origine, conservé
  tel quel. Ils exigent la caisse ouverte avant le premier `INSERT`,
  précisément parce qu'un refus plus bas laisserait un paiement sans
  mouvement de caisse. Sous le verrou du serveur, une seule commande
  s'exécute à la fois : la propriété tient. Elle tomberait le jour d'un
  pool de connexions — **à reprendre avant PostgreSQL**.
- [CONFIRMÉ] `paiements:creer` est dans la liste blanche de l'employé —
  encaisser une créance est son métier. `fournisseurs:regler` n'y est
  pas : payer un fournisseur reste au patron.

## L'écran de réglage réseau

[OngletReseau.tsx](../../src/components/OngletReseau.tsx), sous
Paramètres → Réseau (patron seulement).

- [CONFIRMÉ] Basculer en poste caisse **exige un test réussi**. Sans
  serveur joignable, le poste n'ouvre plus sa base et ne joint pas
  l'autre : il ne peut plus ni vendre, ni revenir en arrière autrement
  que par cet écran.
- [CONFIRMÉ] **Rust est la source, le navigateur la copie.** Le mode
  vit dans `poste.json`, à côté des données. `synchroniserConfig()` est
  appelé au démarrage, avant tout appel de commande : un cache vidé ou
  un profil WebView2 recréé ferait sinon croire à un poste caisse qu'il
  est monoposte — il ouvrirait sa base locale vide, et le commerçant
  conclurait que ses données ont disparu
  ([pont.ts](../../src/lib/pont.ts), [App.tsx](../../src/App.tsx)).
- [CONFIRMÉ] En mode poste, c'est le **serveur** qui authentifie
  ([PageLogin.tsx](../../src/pages/PageLogin.tsx)) : la base des
  utilisateurs est chez lui.
- [CONFIRMÉ] Le canal **signale**, il ne recharge pas. Un
  rafraîchissement automatique écraserait la saisie en cours du
  caissier ; un témoin discret s'allume dans la barre latérale et
  s'éteint au bout de quatre secondes
  ([Layout.tsx](../../src/components/Layout.tsx)).

## Essayer le multiposte à la main

Deux machines sur le même réseau local, ou une seule pour un premier
essai.

1. **Sur le poste principal**, lancer le service :
   `gescom-serveur.exe --base "%APPDATA%\ml.gescom.app\gescom.db"`.
   Il affiche son adresse d'écoute, le nombre de commandes servies et
   le dossier de sauvegarde.
2. Vérifier depuis un navigateur : `http://<ip-du-serveur>:7300/sante`.
3. **Sur le poste caisse**, ouvrir Gescom → Paramètres → Réseau,
   choisir « Poste caisse », saisir `<ip>:7300`, cliquer **Tester** —
   la version du serveur et l'état de la base doivent s'afficher —
   puis **Enregistrer**.
4. Fermer et rouvrir Gescom. L'écran de connexion annonce le serveur.
   S'identifier : c'est le serveur qui vérifie.
5. Vendre. Le catalogue, les clients, les dépôts et le stock viennent
   du serveur ; la vente et sa facture y sont écrites.
6. Sur le poste principal, Paramètres → Réseau ne sert à rien — mais
   `lire_postes` et `lire_sessions_reseau` (via le serveur) montrent la
   caisse connectée.

**Ce qui ne marchera pas depuis une caisse** : imprimer. Les quatre
commandes d'impression et les six d'images restent sur la machine qui a
l'écran. Un poste caisse peut donc tout faire sauf sortir le papier —
à traiter avant de livrer le multiposte.

**Les droits changent en réseau.** En monoposte, aucune permission
n'était vérifiée : l'écran seul décidait. Un poste caisse envoie du
JSON, donc chaque écriture porte désormais une permission et
`portes::verifier_permission` tranche. L'employé peut vendre,
encaisser, créer un client ou un article, ouvrir et mouvementer la
caisse, établir une pièce et enregistrer un retour. Il ne peut pas
toucher aux achats, aux paramètres, aux utilisateurs ni aux
transferts — c'est plus strict que le v1, et c'est délibéré.

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

1. **L'impression depuis une caisse.** Les dix commandes restées dans
   le crate applicatif ouvrent une fenêtre ; un poste caisse doit
   pouvoir imprimer sa facture. Le HTML se fabrique déjà dans l'écran :
   il suffit que la caisse l'imprime chez elle, sans passer par le
   serveur. À vérifier écran par écran.
   ⚠️ `gescom-serveur.exe` n'est toujours pas empaqueté par
   l'installeur, et n'a pas le contrôle d'installation du client.
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
