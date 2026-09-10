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
- [reseau_local.rs](../../src-tauri/serveur/src/reseau_local.rs) — l'adresse
  à saisir sur les caisses, et l'état du pare-feu, affichés au démarrage.
- [console.rs](../../src-tauri/serveur/src/console.rs) — la console, une
  page HTML entière dans une constante Rust.

## L'amorçage d'une base neuve

`seed.rs` vivait dans le crate applicatif : **seule la fenêtre
l'appelait**. Un `gescom-serveur.exe` lancé sur une base neuve
démarrait, écoutait, et refusait toutes les connexions avec
« Identifiant ou mot de passe incorrect » — sans qu'aucun écran ne
permette de créer le premier compte. La seule issue était de lancer la
fenêtre une fois sur le même fichier.

Il vit maintenant dans le noyau, comme le reste du code métier, et les
deux exécutables passent par le même point d'entrée :
`seed::amorcer_si_vide`. Le test de vacuité et l'amorçage étaient deux
appels séparés — c'est ainsi qu'un appelant a pu oublier l'un des deux.

Ce qu'il pose, toujours : rôles, comptes `admin` / `employe` (avec
changement de mot de passe obligatoire à la première connexion), dépôt
par défaut, client « Comptant », paramètres société. Les articles et
clients fictifs ne sont créés que si `GESCOM_DEMO=1` : chez un
commerçant ils seraient à supprimer un par un.

Au démarrage sur une base neuve, le serveur imprime les comptes créés —
sinon le patron a un serveur qui tourne et aucun moyen de savoir quoi
taper.

## Routes

| Méthode | Route | Jeton | Effet |
|---|---|---|---|
| GET | `/` ou `/console` | non | la console du serveur (HTML) |
| GET | `/sante` | non | version, postes connectés, intégrité |
| POST | `/connexion` | non | bcrypt → jeton 8 h |
| POST | `/deconnexion` | oui | révoque la session |
| POST | `/rpc` | oui | exécute une commande du registre |
| GET | `/rpc/catalogue` | non | les noms connus du serveur |
| GET | `/canal?depuis=N` | oui | longue attente, 30 s max |
| POST | `/sauvegarde` | oui | `VACUUM INTO` immédiat |

## La console du serveur

Le serveur n'a pas de fenêtre : c'est un service, il tourne sans écran.
Mais une console noire sur le poste principal est ingérable pour un
commerçant — il ne sait pas ce qu'elle dit, et **la fermer arrête la
boutique**. Une vraie fenêtre demanderait une boîte à outils graphique,
donc un moteur de rendu embarqué dans un service censé n'en avoir aucun.

Le serveur parle déjà HTTP. Il sert donc sa propre page : le patron
ouvre `http://localhost:7300` sur le poste principal — ou l'adresse du
serveur depuis n'importe quelle caisse, ou depuis le téléphone posé sur
le comptoir. **Zéro dépendance ajoutée, aucune ressource extérieure** :
pas de CDN, pas de police à télécharger. Une boutique de Bamako n'a pas
forcément Internet, et une console qui ne s'affiche pas le jour d'une
panne ne sert à rien.

Ce qu'elle montre **sans mot de passe** est exactement ce que `/sante`
expose déjà — version, intégrité de la base, nombre de postes connectés,
dernière sauvegarde. C'est ce qu'une caisse interroge AVANT d'avoir un
jeton ; aucune donnée de commerce n'y passe.

Le reste — nom des postes, sessions ouvertes, révocation, désactivation
d'une caisse, sauvegarde immédiate — exige de s'identifier, avec les
mêmes identifiants que Gescom et le même bcrypt.

### Ce que la mise au point a fait apparaître

Trois défauts que seule l'exécution réelle a montrés :

1. La console s'inscrivait comme un poste **`caisse`**. Elle se listait
   donc elle-même avec un bouton « Désactiver » : un clic, et le patron
   se fermait dehors du seul écran d'où il aurait pu se rouvrir. Elle
   s'inscrit désormais avec le genre **`console`**, et
   `postes::desactiver` refuse tout poste qui n'est pas une caisse
   (test `un_poste_qui_n_est_pas_une_caisse_ne_se_desactive_pas`).
2. Les boutons envoyaient `posteId` et `sessionId` là où le socle attend
   `poste_id` et `session_id` — le serveur répondait « Paramètre
   manquant », donc aucun bouton ne marchait.
3. La session de la console apparaissait dans la liste avec un bouton
   « Déconnecter » qui déconnectait celui qui cliquait. Sa propre ligne
   affiche maintenant « cette page ».

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
- [CONFIRMÉ] Un poste qui n'est pas de genre `caisse` — le poste
  `serveur`, le poste `console` — **ne se désactive pas** : il n'y
  aurait plus de chemin pour le rallumer
  ([postes.rs](../../src-tauri/noyau/src/postes.rs)).
- [CONFIRMÉ] La console annonce une empreinte fixe (`console::EMPREINTE`)
  pour ne pas ajouter une ligne de poste à chaque ouverture du
  navigateur, et pour que le serveur la reconnaisse
  ([console.rs](../../src-tauri/serveur/src/console.rs)).
- [CONFIRMÉ] `portes::verifier_permission` est une liste **blanche** : une
  commande ajoutée demain est refusée par défaut aux rôles restreints
  (test `une_permission_inconnue_est_refusee`).

## Tests

[noyau/tests/reseau.rs](../../src-tauri/noyau/tests/reseau.rs) — 12
scénarios sur base en mémoire. Plus 4 tests de permissions dans
`portes.rs`. Ce sont les seuls tests du multiposte ; les commandes
portées n'en ont toujours pas en propre.

La console, elle, n'est pas testée automatiquement — c'est une page. Ce
qui est vérifié, c'est **ce qu'elle appelle** : la route `/` rend bien
du HTML, `/connexion` avec l'empreinte de la console donne un jeton, et
`lire_postes`, `lire_sessions_reseau`, `desactiver_poste`,
`reactiver_poste`, `revoquer_session_reseau`, `POST /sauvegarde`
répondent chacun sur une instance réelle. C'est ainsi que les trois
défauts ci-dessus sont sortis.

## Ce qui reste à faire

1. **L'impression depuis une caisse.** Les dix commandes restées dans
   le crate applicatif ouvrent une fenêtre ; un poste caisse doit
   pouvoir imprimer sa facture. Le HTML se fabrique déjà dans l'écran :
   il suffit que la caisse l'imprime chez elle, sans passer par le
   serveur. À vérifier écran par écran.
   ⚠️ `gescom-serveur.exe` n'est toujours pas empaqueté par
   l'installeur, et n'a pas le contrôle d'installation du client.
2. Le pare-feu n'a été vérifié que **depuis la même machine**, ce qui ne
   prouve rien : seule une seconde machine sur le réseau le prouve.

Faits depuis : le stock dérive maintenant de ses mouvements (voir les
tests `stock_mouvements.rs`), la numérotation passe par un compteur
transactionnel ([numerotation.md](numerotation.md)), et l'écran de
réglage réseau existe
([OngletReseau.tsx](../../src/components/OngletReseau.tsx)).
