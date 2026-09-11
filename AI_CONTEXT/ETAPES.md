# Les étapes : ce qui est fait, ce qui reste

Ce fichier répond à une seule question : **où en est-on, et quoi
ensuite ?** Il se relit au début de chaque séance et se met à jour à la
fin. Les chiffres qu'il contient sont **mesurés**, jamais estimés — ce
projet a déjà payé une estimation à la louche (« 614 `rusqlite::`,
1556 placeholders » était faux d'un facteur deux).

Les décisions d'architecture ne vivent PAS ici : elles sont dans
[PLAN-MULTISOCIETE.md](PLAN-MULTISOCIETE.md) pour le multi-dossier, et
dans [DECISIONS.md](DECISIONS.md) pour tout le reste. Ce fichier-ci ne
dit que l'avancement — qui fait quoi, dans quel ordre.

Dernière mise à jour : **11 septembre 2026**.
État : **262 tests SQLite + 21 tests PostgreSQL**, tous au vert.

---

## Les trois produits

| | ce que c'est | état |
|---|---|---|
| **v1** | un poste, SQLite, pas de serveur | **livré**, ne bouge plus |
| **v2** | serveur + clients, le client ne parle **qu'**au serveur | en cours |
| **v3** | multi-société, multi-dossier, exercices | pas commencée |

**Une seule machine ne veut pas dire sans serveur.** La boutique à une
caisse installe les deux sur le même ordinateur ; la fenêtre se connecte
sur `127.0.0.1`. Ce poste est à la fois serveur et caisse.

⚠️ Ce document a d'abord décrit la v2 comme gardant un mode « monoposte »
où la fenêtre ouvrait une base SQLite locale. **C'était une lecture
fautive**, et elle a coûté deux pannes réelles. Il n'y a qu'un seul
chemin. Voir [ARCHITECTURE.md](ARCHITECTURE.md).

---

## v2 — ce qui est FAIT

### Le socle réseau
- Serveur HTTP/1.1 écrit à la main, **zéro dépendance réseau** ajoutée.
- 187 commandes servies, canal d'événements en longue attente (30 s),
  jamais d'écho au poste qui a provoqué l'événement.
- Sessions : bcrypt en base, jeton↔session en mémoire, validité relue
  **à chaque appel** pour qu'une révocation prenne effet tout de suite.
- Console du serveur, servie par le serveur lui-même dans un navigateur.
- Amorçage partagé : le serveur prépare sa base comme la fenêtre.
- → [reseau-v2.md](modules/reseau-v2.md)

### Le client ne parle qu'au serveur
- Plus de repli sur une base locale. Un repli silencieux, c'est une
  vente écrite dans une base vide pendant qu'on croit vendre.
- Écran de **branchement** avant la connexion, qui n'enregistre
  qu'**après un essai réussi**.
- Liste fermée des commandes qui restent locales : imprimer, ouvrir un
  fichier, régler le réseau.

### Les droits
- Catalogue de **23 permissions**, relevées sur les commandes réelles.
- Rôles en base, créables sans recompiler : `superadmin`, `patron`,
  `employe`, `caissier`, `magasinier`, `comptable`.
- Permissions **par personne**, en plus ou en moins de son rôle.
- Menu et onglets pilotés par les permissions, pas par le nom du rôle.
- → [permissions.md](modules/permissions.md)

### Les règles métier réparées
| quoi | pourquoi ça comptait |
|---|---|
| Stock = somme de ses mouvements | un compteur ne dit pas *pourquoi* il vaut ça |
| Numérotation par compteur transactionnel | **5 doublons sur 100** mesurés avec l'ancien calcul |
| Le stock bouge au document qui le constate | une facture ne sort plus ce qu'un bon a déjà sorti |
| Miroir fournisseur complet | le bouton « → Facture fourn. » était **mort** |
| Fond de caisse compté une fois | la clôture annonçait un manque **tous les soirs** |
| Vente à découvert recalculée en base | un écran périmé créait un découvert invisible |

### L'installeur
- `gescom-serveur.exe` **empaqueté**, raccourcis menu Démarrer et
  démarrage de session.
- Pas signé — décision assumée. → [installeur.md](modules/installeur.md)

### Les documents
- Atelier de modèles, glisser-déposer, import/export.
- Facture « classique » avec TVA détaillée ligne par ligne.
- Pied de page **dessiné**, placé au millimètre, répété sur chaque page.

---

## v2 — ce qui RESTE

### Bloquant avant de vendre
1. ~~Le pare-feu~~ — **vérifié le 11/09/2026** : un téléphone sur le
   même Wi-Fi joint le serveur. La règle posée par `parefeu.ps1` marche
   depuis un vrai second appareil, pas seulement depuis la boucle
   locale.
2. **L'installeur n'est pas signé.** Chaque installation dépend de
   l'humeur de SmartScreen. Auto-signé + racine posée à la main tient
   tant qu'on déploie soi-même.

### Fonctionnel
3. L'écran des permissions **par personne** : les commandes existent,
   l'interface non.
4. Aucun compte `superadmin` n'est créé. Le rôle existe, personne ne le
   porte. Un compte de secours à mot de passe connu est aussi un risque.
5. Écriture des images (logo, en-tête, pied) depuis une caisse : la
   commande reçoit un *chemin* local, qui ne désigne rien chez le
   serveur.
6. `entretenir_base` (REINDEX, VACUUM) reste locale alors que c'est un
   travail de serveur.

### Jamais vérifié à la main
7. Le glisser-déposer du pied de page, et une **vraie impression
   papier**.
8. La fenêtre en mode caisse, utilisée pour de bon. Tous les essais sont
   passés par le serveur en HTTP.

---

## Le portage PostgreSQL

### Fait
- **Schéma dialect-neutre** : `schema.sql` accepté par les deux moteurs.
- **Façade `Base`** : `?1` → `$1` traduit au passage, et un type `Ligne`
  commun pour que les 435 fermetures de lecture survivent sans être
  touchées. → [base.rs](../src-tauri/noyau/src/base.rs)
- **Amorçage portable** : schéma, tables v2, rôles, comptes, dépôt,
  client générique, et les données de démonstration.
- **Transactions** : `Base::transaction()` — tout ou rien, sur les deux
  moteurs. Indispensable dès qu'on touche à l'argent. La plomberie est
  partagée entre `Base` et `Transaction`, pour qu'une correction ne
  s'applique pas à une seule.
- Erreurs PostgreSQL lisibles : `db error` devenait trois mots inutiles,
  on remonte maintenant la contrainte, la colonne et la table.

### Les modules portés, dans l'ordre

| lot | modules | ce qu'il débloque |
|---|---|---|
| 1 | `auth`, `sessions`, `portes` | **entrer** dans la base |
| 2 | `catalogue` | l'écran de caisse s'affiche |
| 3 | `comptoir` | créer clients et articles — le premier lot qui **écrit** |
| 4 | fondation d'`argent` | la **numérotation**, dont tout dépend |

La règle des permissions vit dans une **fonction pure** appelée par les
deux lectures : deux copies d'un calcul de droits finissent toujours par
diverger, et personne ne s'en aperçoit avant qu'un caissier fasse ce
qu'il ne devait pas. Même principe pour le préfixe des séries.

Vérifié sur une base réelle : connexion du patron avec ses 23
permissions, catalogue de 8 articles avec leurs unités regroupées, prix
d'achat masqué pour l'employé, dépôt inconnu qui retombe sur le défaut,
client créé puis modifié (accent conservé, champ vidé redevenu NULL),
article refusé en doublon sans rien écrire.

Et le test qui compte le plus : **quatre connexions PostgreSQL
simultanées réservant 100 numéros — aucun doublon, suite continue.**
C'est le scénario qui sortait 5 doublons sur 100 avec l'ancien calcul.

### Reste — le vrai volume

| | mesuré |
|---|---|
| points d'appel (`execute`, `query_row`, `query_map`, `prepare`) | **739** |
| `params!` à convertir | 480 |
| fermetures de lecture | 435 |
| paramètres `conn:` dans les signatures | 242 |
| constructions SQL non portables (hors placeholders) | ~60 |

**Le serveur ne sait pas encore faire tourner une boutique sur
PostgreSQL.** Vendre, facturer, encaisser passent par SQLite — même
après ce qui suit, et c'est le point important de ce paragraphe.

### `creer_vente_sur_base` et `valider_facture_sur_base` — **portées le 11/09/2026**

Le morceau mis à part exprès (280 et 290 lignes, « une erreur ne se
corrige pas par un clic ») a eu sa propre séance et ses propres
scénarios : [argent_base.rs](../src-tauri/noyau/tests/argent_base.rs),
13 tests — vente comptant, crédit refusé au comptant (D40), crédit
accepté à un vrai client, encaissement sans caisse ouverte refusé,
vente à découvert signalée, avoir plus grand que la vente (soldé + un
reliquat rouvert), deux dossiers qui ne mélangent pas leurs ventes,
facture comptant/à crédit avec acompte, facture déjà validée ou pas une
facture refusées, facture dont le stock est déjà sorti par un bon.

⚠️ **Porter ces deux fonctions a fait remonter deux trous dans
l'amorçage `Base`, invisibles jusque-là parce que rien ne vérifiait
encore un vrai mouvement de stock à travers lui :**
- **Le déclencheur `stock_suit_les_mouvements` n'existait que côté
  fenêtre** (`persistance/v2.rs`), jamais rejoué par `amorcage.rs`. Une
  base amorcée uniquement par `Base` — tous les tests, et demain le
  serveur — semblait vendre pendant que `stock_depot` restait à zéro.
  Posé maintenant sur les **deux moteurs** dans `amorcage.rs`
  (`trigger_stock`), ce qui a aussi permis de retirer le contournement
  que `donnees_demo` portait pour PostgreSQL.
- **`avoir.piece_id` manquait au schéma.** La colonne n'existait que
  par une migration de la fenêtre (`persistance/mod.rs`), jamais dans
  `schema.sql` — pourtant `creer_vente_sur_base` (comme la version
  SQLite) la lit. Ajoutée à `schema.sql`, la source unique des deux
  moteurs ; l'ancienne migration continue de s'exécuter et échoue en
  silence, comme sur une colonne déjà là.

Ni l'un ni l'autre ne s'était vu avant : les scénarios existants ne
lisaient jamais `stock_depot` après un vrai mouvement passé par `Base`,
ni n'écrivaient un avoir avec `piece_id`. **C'est tout l'intérêt d'avoir
donné à ce morceau ses propres scénarios plutôt que de l'expédier à la
fin d'un autre.**

**Ce qui n'a toujours pas changé pour le commerçant :** ces deux
fonctions vivent dans `gescom-noyau`, testées et prêtes, mais **aucun
chemin de commande ne les appelle encore** — ni la fenêtre (qui tient
une `Connection`, pas une `Base`), ni le serveur (D11 : il doit d'abord
tenir une `Base` lui-même). Les brancher est un travail à part, qui
suit D11.

Ensuite : `pieces`, `achats`, `retours`, puis le reste. La façade permet
de porter module par module sans rien casser — ce qui n'est pas porté
continue de tourner sur SQLite.

### D11 (le serveur tient une `Base`) — **points 1 et 2 faits le 11/09/2026**

`gescom-serveur --base postgresql://…` s'ouvre, amorce le schéma, et
répond à `/sante`. Zéro changement sur une cible fichier — les 186
commandes non portées continuent de tourner exactement comme avant ;
sur PostgreSQL, elles refusent clairement au lieu de retomber sur un
fichier SQLite vide. Détail dans DECISIONS.md §D11.

⚠️ **Découvert en le faisant, puis corrigé le même jour : se connecter
ne marchait pas sur PostgreSQL**, même avec le bon mot de passe.
`sessions` et `portes::permissions_de` étaient déjà portés (trouvés en
regardant, pas refaits) ; il manquait `postes::inscrire_ou_retrouver`.
Écrit, et `api.rs` (`connexion`, `déconnexion`, `authentifier`, le
contrôle de permission de `/rpc`) bascule maintenant sur `Base`
**sur les deux moteurs** — plus de différence de chemin entre SQLite et
PostgreSQL pour l'authentification. 12 scénarios dédiés
([auth_base.rs](../src-tauri/noyau/tests/auth_base.rs)) : inscription
et retrouvaille d'un poste, session ouverte/révoquée/expirée, poste
désactivé qui ferme la session, permissions de rôle et personnelles, et
le login complet rejoué de bout en bout comme le fait le serveur.

**Ce qui reste vrai** : les 186 commandes de vente, stock, pièces
restent sur `Connection` et refusent sur PostgreSQL (D11). Une caisse
peut maintenant s'y connecter ; elle ne peut encore rien y faire.

Deux chantiers à part :
- la **sauvegarde** — `VACUUM INTO` n'existe pas côté PostgreSQL ;
- les **~60 constructions** — `julianday`, `strftime`, `INSERT OR
  IGNORE`, `substr(x, -5)`. À réécrire en SQL que les deux moteurs
  acceptent, pas en deux variantes.

---

## v3 — décidé, pas commencé

### Multi-société et multi-dossier — **tranché le 11/09/2026**

Modèle Ciel : un dossier = **une société × un exercice**, avec une date
de début, une date de fin, et une prolongation possible.

**Forme retenue : une colonne `dossier_id`.** L'autre option — une base
par dossier — n'aurait demandé aucun filtre, mais interdisait toute vue
consolidée.

Le prix est connu et tient en une phrase : *une requête qui oublie le
filtre mélange deux sociétés*. Et elle ne le dit pas — elle rend des
chiffres plausibles, calculés sur les ventes de quelqu'un d'autre.
C'est pourquoi le garde-fou a été construit **avant** les colonnes.

#### Fait : la fondation

| | |
|---|---|
| table `dossier` | société, début, fin, `prolonge_jusqu_au`, `clos` |
| colonne `dossier_id` | sur les **23 tables cloisonnées**, plus son index |
| `Base` porte son dossier | il ne vit plus dans un argument qu'on peut oublier |
| détecteur de requête non filtrée | nomme les tables en cause, refuse **avant** d'exécuter |
| règle des dates d'exercice | fonction pure, 9 scénarios |

**Ce qui n'est PAS cloisonné**, et c'est voulu : `utilisateur`, `role`,
`poste`, `session_reseau`, `modele_document`. Un caissier qui change de
société reste le même caissier.

**`compteur_piece` non plus** — sa clé porte le dossier
(`<dossier>:<série>`). La numérotation est la seule chose du projet où
un doublon se voit chez le commerçant et ne se répare pas : on ne
remanie pas sa table pour une raison de forme.

#### Ce qui change le coût du reste du portage

Sur PostgreSQL, `dossier_id` a pour valeur par défaut le dossier de la
**session**. Une insertion tombe donc dans le bon dossier *sans que
l'appelant le dise* — vérifié sur la vraie base.

Conséquence directe : **les 480 `params!` n'ont pas à être rouverts**
pour y glisser un argument de plus. Seules les **lectures** devront
porter le filtre. La dette annoncée avant l'arbitrage était surévaluée
de plus de moitié.

#### Fait aussi : les suites de numéros, et les modules déjà portés

**La numérotation est cloisonnée.** Chaque dossier a sa propre suite :
deux sociétés qui facturent le même jour ont chacune leur
`FAC-2026-00001`. Les quatre appelants — pièces, transferts,
code-barre, fournisseurs — passent par le même préfixe, posé dans
`suivant` et non chez eux : un seul qui l'oublierait suffirait à
mélanger deux suites.

⚠️ **Une migration accompagne ce changement, et elle n'est pas
facultative.** Les bases existantes portent la clé `FAC-2026` ; le code
cherche désormais `<dossier>:FAC-2026`. Sans migration il ne trouverait
rien, repartirait de 1, et refabriquerait un numéro déjà émis — la
contrainte UNIQUE bloquerait alors la première facture du matin de la
mise à jour, au comptoir, devant le client. La migration est rejouable
et couverte par ses scénarios.

**`catalogue`, `comptoir` et la fondation d'`argent` portent leur
filtre**, détecteur allumé sur leurs scénarios — sur les deux moteurs.
Un test appelle chaque fonction portée avec le détecteur actif : un
filtre oublié échoue là, pas chez un commerçant qui verrait le
catalogue d'une autre société sans s'en douter. Porter un module de
plus, c'est l'ajouter à cette liste et laisser le détecteur dire ce qui
manque.

#### Deux défauts trouvés en route

- **Deux tables de préfixes.** `reserver_numero` portait sa propre copie
  de la table que `prefixe_de` était censée centraliser — le
  commentaire de `prefixe_de` annonçait déjà le risque. Un préfixe
  changé d'un seul côté aurait donné deux séries pour le même type de
  pièce.
- **Un `NULL` sans type refusé par PostgreSQL.** Un champ laissé vide
  sur une colonne numérique — un article sans prix d'achat — échouait
  sur « error serializing parameter 3 », un message qui ne nomme ni la
  colonne, ni la table. Le cas ne s'était jamais vu parce que SQLite
  accepte tout et que les scénarios PostgreSQL renseignaient ce champ.

#### Corrigé le 11/09/2026 : les articles n'auraient jamais dû être cloisonnés

La fondation posée plus tôt le même jour cloisonnait `categorie`,
`article`, `unite_vente` — [PLAN-MULTISOCIETE.md](PLAN-MULTISOCIETE.md)
tranche l'inverse : un article est une chose, pas une relation, commune
à tous les dossiers. Corrigé dans `dossiers.rs`, `catalogue.rs`,
`comptoir.rs`. `stock_depot` reste cloisonné — c'est le stock qui
appartient au magasin. 226 tests SQLite toujours au vert ; les
scénarios PostgreSQL n'ont pas pu être rejoués faute d'instance
accessible dans cette séance. Détail dans PLAN-MULTISOCIETE.md §10.

#### Reste — et ce qui n'est pas encore vrai

⚠️ **Le multi-dossier n'est toujours pas utilisable.** Ce qui est porté
est cloisonné ; tout le reste ne l'est pas. Avec un seul dossier rien
ne change ; avec deux, les écrans non portés mélangeraient tout.

1. Poser le filtre sur les modules restants — `pieces`, `achats`,
   `retours`, le reste — en les ajoutant à la liste du détecteur.
2. **Un dossier neuf n'a ni magasin ni client de passage** : le
   catalogue refuse de s'ouvrir, en disant ce qui manque. C'est la
   preuve que le cloisonnement mord, et le cahier des charges de
   l'écran de création : poser le magasin par défaut et le client de
   passage, comme l'amorçage le fait pour le premier dossier.
3. Les écrans : créer, changer, prolonger, clore un dossier.
4. **Le garde-fou moteur manque.** PostgreSQL sait refuser lui-même une
   requête non cloisonnée (RLS), mais **le contourne pour un
   superutilisateur** — or l'application se connecte en `postgres`.
   Tant que ce sera le cas, le cloisonnement repose sur le code. Un
   rôle applicatif dédié est le vrai correctif.
5. Une connexion réutilisée (pool) doit reposer son dossier à chaque
   prise, sinon elle garde celui du précédent.

### Dépôt → magasin — **à l'écran : FAIT le 11/09/2026**
`depot` et `magasin` désignent la même chose.

Ce qui a changé : **les 67 textes que le commerçant lit** — écrans,
messages d'erreur, libellés de permissions. Plus l'étiquette du magasin
d'usine, rattrapée par une migration idempotente qui ne touche que le
nom exact posé à l'amorçage : un commerçant qui a renommé son magasin
garde son nom.

Ce qui n'a **pas** changé, et c'est voulu :
- les identifiants — `depot_id`, `lire_depots`, la colonne `depot` ;
- les **commentaires du code**, qui parlent de l'entité `depot`. Les
  aligner sur l'étiquette d'écran ferait croire que la base a été
  renommée.

La séparation était propre et vérifiable : **toute** occurrence
accentuée (« dépôt ») était du texte affiché, **toute** occurrence sans
accent (`depot`) un identifiant. C'est ce qui a rendu le renommage sûr.

Le renommage **jusque dans la base** touche ~200 requêtes et demande une
migration. Il reste prévu, **en même temps que le multi-dossier**, qui
rouvre de toute façon la forme de la base — séparément, ce serait
reprendre les mêmes fichiers deux fois.

---

## Comment travailler ici

```bash
# préparer une base, avant même de lancer le serveur
cargo run -p gescom-noyau --example amorcer -- <cible> [--demo]

# le serveur
./src-tauri/target/debug/gescom-serveur.exe --base <cible>

# les tests
.\outils\cargo-tenace.ps1 test --workspace          # SQLite
GESCOM_PG="postgresql://..." cargo test -p gescom-noyau \
    --test postgres_amorcage -- --test-threads=1     # PostgreSQL
```

`cargo-tenace` et non `cargo` : Smart App Control bloque les binaires
fraîchement liés, et l'erreur ne ressemble pas à ce qu'elle est. →
[environnement-windows.md](modules/environnement-windows.md)

Les tests PostgreSQL ne tournent que si `GESCOM_PG` est défini. Un test
qui exige un service tiers ne doit pas faire échouer la suite de
quelqu'un qui ne l'a pas installé.
