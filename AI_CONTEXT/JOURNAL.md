# Journal — ce qui a été fait, dans l'ordre

Ce fichier est le **récit daté** du projet : chaque avancée, avec ce
qu'elle a coûté et ce qu'elle a trouvé en route. Il grossit et ne se
relit pas en entier — **il n'est pas chargé par défaut**. L'état
courant, lui, tient en une page : [ETAPES.md](ETAPES.md).

Quand une séance ajoute quelque chose : une section datée ici, une
ligne dans le tableau d'ETAPES.md, et c'est tout.

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
peut maintenant s'y connecter ; elle ne peut encore rien y faire —
sauf pour **12 commandes**, branchées le même jour.

### Douze commandes branchées à `Base`, dans le registre — **fait le 11/09/2026**

`Registre::aussi_sur_base` complète une commande déjà enregistrée avec
sa version `Base`, sans toucher à son chemin `Connection` — zéro
changement sur une cible fichier, c'était la condition pour y toucher
sans relire les 174 autres. `rpc()` : `conn` présent → chemin inchangé ;
absent → la version `Base` si elle existe, sinon le refus D11.

Branchées : `lire_clients`, `lire_client_generique`, `lire_depots`,
`lire_depot_defaut`, `lire_articles_avec_unites` (le comptoir — une
caisse PostgreSQL voit enfin un écran), `creer_client_rapide`,
`creer_article_rapide`, `modifier_client`, `lire_clients_avec_creances`,
`lire_config_scanner`, et les deux qui comptent le plus :
`creer_vente`, `valider_facture`.

**Une boutique peut désormais vendre sur PostgreSQL** — catalogue,
client, vente, facture. Ce qui manque encore pour que ce soit une vraie
boutique : la caisse (`ouvrir_session_caisse`, encaisser), les pièces
au sens large, le stock, les rapports — le reste des 186, module par
module, comme prévu.

4 scénarios dans
[registre_base.rs](../src-tauri/noyau/tests/registre_base.rs),
dont un qui appelle RÉELLEMENT les deux chemins d'une même commande
sur deux bases amorcées séparément et vérifie qu'ils s'accordent.

### La caisse, et le premier essai manuel — **fait le 11/09/2026**

En préparant l'essai manuel sur une vraie base PostgreSQL, deux trous
auraient bloqué le tout premier geste :

- **Ouvrir la caisse.** `creer_vente_sur_base` exige une session
  ouverte dès qu'un montant est encaissé — rien ne pouvait en ouvrir
  une. Écrit : `caisse::{lire_resume_caisse,ouvrir_session_caisse,
  fermer_session_caisse}_sur`, branchées au registre. 7 scénarios dans
  [caisse_base.rs](../src-tauri/noyau/tests/caisse_base.rs), dont un
  qui rejoue exactement le blocage (`CAISSE_FERMEE` sans caisse
  ouverte, la vente passe une fois la caisse ouverte).
- **Le changement de mot de passe obligatoire.** Les comptes `admin` /
  `employe` posés par l'amorçage exigent un changement à la première
  connexion (`doit_changer_mdp`) — la modale est obligatoire, rien
  d'autre ne s'affiche tant qu'elle n'a pas réussi.
  `auth::changer_mot_de_passe_sur` existait déjà (jamais branchée) ;
  branchée.

**Ce qui est maintenant essayable à la main sur une vraie base
PostgreSQL** : connexion, changement de mot de passe obligatoire,
catalogue, création client/article, ouverture de caisse, vente au
comptant ou à crédit, fermeture de caisse.

**Ce qui ne l'est pas encore** : le tableau de bord (aucune de ses
commandes n'est branchée — les widgets resteront vides), la facture
POS automatique après une vente (`creer_facture_depuis_vente` n'a
qu'un chemin SQLite — la vente s'enregistre quand même, un
avertissement non bloquant apparaît), l'impression, et tout le reste
des commandes non listées ci-dessus.

### Défaut trouvé pendant le premier essai manuel — **corrigé le 11/09/2026**

Le patron connecté (`admin`) n'avait **aucune** permission, alors que
`patron` a `acces_total = 1` dans le code de l'amorçage. Cause :
`INSERT ... ON CONFLICT (nom) DO NOTHING` ne pose cette valeur
**qu'une fois** — la toute première fois que la base a été amorcée.
La base de test avait été amorcée tôt dans la séance, avant une
version du code où cette valeur était encore fausse ; chaque
redémarrage du serveur depuis ne l'a jamais corrigée, puisque la base
n'était plus « vide » et que le bloc d'insertion des rôles ne se
rejouait plus.

Corrigé sur les deux chemins (`amorcage::acces_total_toujours_reaffirme`
et son pendant dans `persistance/v2.rs`) : `patron` et `superadmin`
sont réaffirmés à `acces_total = 1` à **chaque** démarrage, pas
seulement au premier. Un simple redémarrage du serveur suffit à
réparer une base déjà cassée — pas besoin de la réamorcer. Testé
(`un_patron_sans_acces_total_est_repare_au_prochain_amorcage`), qui
casse délibérément la ligne puis vérifie que le second amorçage la
répare.

### La facture POS automatique — **portée le 11/09/2026**

`creer_facture_depuis_vente_sur_base` : troisième pièce du trio
argent (avec `creer_vente_sur_base` et `valider_facture_sur_base`),
appelée juste après une vente pour produire la facture GESCOM/FAC- que
le client repart avec. Même absence de transaction que la version
SQLite — c'est un geste non bloquant côté écran (`try/catch`, la
vente reste acquise même si la facture échoue), donc le coût d'un
échec partiel est déjà assumé par l'appelant. Branchée au registre.

2 scénarios de plus dans `argent_base.rs` : facture soldée d'emblée au
comptant (lien `vente.piece_id` vérifié), facture qui reste "emis"
sans encaissement.

### Le tableau de bord — **porté le 12/09/2026**

Les cinq lectures de l'écran d'accueil (`lire_resume_dashboard`,
`lire_ventes_periode`, `lire_top_clients`, `lire_top_articles`,
`lire_ventes_a_decouvert`), en `_sur`, cloisonnées, branchées au
registre. **L'écran POS n'a plus rien qui manque sur PostgreSQL.**

Ce qui a changé par rapport à la version SQLite, et pourquoi :
- `julianday`, `strftime`, `date('now')` remplacés par `SUBSTR` sur les
  dates ISO et un rangement en cases **en Rust** (`case_de`, pure,
  5 tests) — le même calcul sur les deux moteurs, au lieu de deux SQL.
- Chaque somme est enveloppée dans `CAST(... AS BIGINT)` : PostgreSQL
  rend `NUMERIC` pour `SUM(bigint)`, que `i64` ne sait pas lire.
- Un paramètre optionnel s'écrit `CAST(?N AS TEXT) IS NULL OR col = ?N`.
  Le simple `?N IS NULL` échoue sur PostgreSQL (« n'a pas pu déterminer
  le type de données du paramètre ») dès que la valeur est NULL.
- **Le résumé fait remonter ses erreurs** au lieu de retomber à 0 comme
  la version SQLite. Sur deux moteurs, un 0 silencieux cache exactement
  ce qu'on cherche : un CA à zéro ressemble à « pas de vente », pas à
  « requête refusée ».

12 scénarios dans
[tableau_bord_base.rs](../src-tauri/noyau/tests/tableau_bord_base.rs),
qui vérifient les chiffres après de vraies ventes passées par
`creer_vente_sur_base` — et qui **tournent sur PostgreSQL quand
`GESCOM_PG` est défini** (`--test-threads=1`).

⚠️ **Rejouer les scénarios PostgreSQL a fait remonter deux défauts de
la façade, invisibles depuis SQLite :**
- **`INT4` illisible.** Les tables v2 créées par du DDL en ligne dans
  `amorcage.rs` (`exercice.clos`, `dossier.clos`, `role.acces_total`…)
  ne passent pas par `types_postgres` : elles sont en `INT4`, et
  `postgres` refuse de le lire dans un `i64` (« error deserializing
  column 5 »). `lire_exercices_sur` échouait sur PostgreSQL — le
  scénario existait, il n'avait jamais été rejoué. Corrigé dans
  `Ligne` : elle lit selon le type **réel** de la colonne (`INT2/4/8`,
  `FLOAT4/8`, `BOOL`) et rend ce que l'appelant demande. Un DDL qui
  oublie `BIGINT` ne casse plus une lecture.
- **Une apostrophe dans un commentaire SQL.** `traduire_parametres`
  prenait le `L'` de « L'oublier » (un commentaire `--` dans une
  requête) pour l'ouverture d'un texte : plus aucun `?N` n'était
  traduit ensuite, et PostgreSQL répondait « l'opérateur n'existe pas :
  ? integer ». Le traducteur saute maintenant les commentaires
  `-- … fin de ligne`. Deux tests.

### `pieces`, `achats`, `retours` — **portés le 12/09/2026**

Les trois modules annoncés « ensuite », d'un bloc : **30 commandes**
(21 pièces, 6 achats, 3 retours), en `_sur_base`, cloisonnées, branchées
au registre. Le serveur sert désormais **47 commandes sur `Base`**.

Ce qui a rendu ce lot possible sans dupliquer chaque aide : **le trait
`Acces`** (`base.rs`), implémenté par `Base` et par `Transaction`.
Réserver un numéro, insérer les lignes d'une pièce, trouver la caisse
ouverte, marquer un bon livré — ces gestes servent tantôt hors, tantôt
dans une transaction. Sans trait commun, chacun existait en deux copies
(`caisses.rs` en portait la trace : `exiger_sur` / `exiger_dans_tx`,
désormais une seule règle). `Transaction` connaît maintenant son
dossier, comme `Base`.

Ce qui change par rapport aux versions SQLite, au-delà du portage :
- **les listes filtrent par paramètres liés**, plus par
  `format!("pc.statut = '{}'")` avec un `replace('\'', "''")` — un
  paramètre lié n'a rien à échapper ;
- **la recherche ignore la casse sur les deux moteurs** :
  `LOWER(x) LIKE LOWER('%' || ?n || '%')`. `LIKE` seul est insensible
  sur SQLite, sensible sur PostgreSQL — un commerçant qui cherche
  « awa » n'aurait rien trouvé sur le moteur de production ;
- **`creer_piece`, `modifier_piece`, `dupliquer_piece` écrivent dans
  une transaction**, ce que la version SQLite ne faisait pas : une
  pièce sans ses lignes, un avoir sans sa pièce, sont exactement les
  demi-écritures qu'une transaction interdit ;
- la table des conversions permises et le statut de naissance d'une
  pièce vivent chacun dans UNE fonction (`conversion_permise`,
  `statut_initial`) : la version SQLite en avait deux copies qui
  pouvaient diverger.

`livraisons::marquer_entierement_livre_sur` a été porté avec, parce
que la conversion commande → bon de livraison en dépend : c'est elle
qui sort le stock au bon, et la facture qui suit ne le sort pas une
seconde fois — vérifié.

**33 scénarios** dans [pieces_base.rs](../src-tauri/noyau/tests/pieces_base.rs)
(15), [achats_base.rs](../src-tauri/noyau/tests/achats_base.rs) (9),
[retours_base.rs](../src-tauri/noyau/tests/retours_base.rs) (9), avec
un module partagé [commun/](../src-tauri/noyau/tests/commun/mod.rs).
Chacun vérifie stock, caisse et créance APRÈS le geste ; chacun tourne
sur PostgreSQL quand `GESCOM_PG` est défini ; chaque fichier finit par
un test qui appelle tout ce qui est porté avec le détecteur allumé.
**Les 33 passent sur PostgreSQL** — sans qu'un seul écart de moteur
n'ait eu à être corrigé après coup : les trois pièges déjà connus
(`SUM` → `NUMERIC`, paramètre NULL non typé, `LIKE`) étaient
appliqués d'emblée.

### Le reste — **porté le 12/09/2026, en six lots**

| lot | modules | commandes | scénarios |
|---|---|---|---|
| A | `societe`, `codebarre`, `roles`, `parametres` | 27 | [reglages_base.rs](../src-tauri/noyau/tests/reglages_base.rs) (8) |
| B | `journal`, `rapports`, `relances`, `cheques`, `transferts` | 17 | [journal_rapports_base.rs](../src-tauri/noyau/tests/journal_rapports_base.rs) (6) |
| C | `depots`, `avoirs`, `chantiers`, `creances` | 38 | [gestion_base.rs](../src-tauri/noyau/tests/gestion_base.rs) (7) |
| D | `fournisseurs` + le règlement fournisseur d'`argent` | 13 | [fournisseurs_base.rs](../src-tauri/noyau/tests/fournisseurs_base.rs) (5) |
| E | `pagination`, `livraisons`, `modeles`, `catalogue_csv` | 16 | [listes_base.rs](../src-tauri/noyau/tests/listes_base.rs) (6) |
| F | `caisse`, `pieces_pos`, `enregistrer_paiement`, utilisateurs, postes, mode de caisse, `images`, `sauvegarde` | 27 | [caisse_reste_base.rs](../src-tauri/noyau/tests/caisse_reste_base.rs) (6) |

Chaque lot : les fonctions en `_sur_base`, le registre dupliqué en
`aussi_sur_base` par un outil qui recopie le bloc `Connection` (mêmes
lignes `arg(...)`, `c.conn` → `c.base`), un fichier de scénarios qui
finit par un test « tout passe le détecteur », rejoué sur PostgreSQL.
**Chaque lot est passé sur PostgreSQL avant d'être commité.**

**Les ~60 constructions non portables** sont réglées, une fois, en SQL
commun ou en Rust :
- `julianday('now') - julianday(x)` → `utils::jours_depuis`, pure et
  testée (chèques dormants, retard de créance, tranches d'ancienneté) ;
- `strftime('%Y-%m', x)` et `DATE(x) = ...` → `SUBSTR(x, 1, 7|10)`
  sur les dates ISO ; `date('now', '-N months')` → calculé en Rust ;
- `INSERT OR IGNORE` → `ON CONFLICT (...) DO NOTHING` ; la ligne de
  stock d'un magasin neuf se pose article par article (le
  `randomblob` était du SQLite pur) ;
- `json_group_array` → les unités d'une ligne de stock se lisent en
  Rust, en une requête pour la page ;
- un alias du SELECT dans `HAVING` → une sous-requête, PostgreSQL le
  refuse ;
- les `GROUP BY` sans agrégat de l'import CSV sont retirés — PostgreSQL
  exige que chaque colonne lue soit groupée, et ils ne servaient à
  rien.

**La sauvegarde** est portée, pas contournée : `pg_dump` lancé par le
noyau avec l'URL que `Base` tient déjà, mot de passe en `PGPASSWORD`
(jamais en argument, jamais dans le dépôt — D10), exécutable trouvé par
réglage, `PATH` ou dossier d'installation. Le serveur planifie sur les
deux moteurs. Vérifié par un vrai dump de la démo que `pg_restore`
sait lire.

**Trouvé en route, et corrigé** : `unite_vente.code_barre`,
`paiement.annule_paiement_id`, `paiement_fournisseur.annule_paiement_id`,
`parametres_societe.entete_chemin` / `pied_chemin` et la table
`cheque_recu` n'existaient que par les migrations de la fenêtre
(`persistance/mod.rs`) — aucune base amorcée par `Base` ne les avait.
Ajoutés à `schema.sql`, rejoués par `amorcage.rs`. Même famille que
`avoir.piece_id` avant eux. **Le filet existe désormais** :
[schema_commun.rs](../src-tauri/noyau/tests/schema_commun.rs) compare
la base de la fenêtre à celle du serveur, colonne par colonne — et a
trouvé le septième trou (`mouvement_caisse.poste_id`) à son premier
passage.

**Ce que ça change pour le commerçant** : sur PostgreSQL, tout ce que
l'application sait faire, sauvegarde comprise.

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

---

## La caisse sur PostgreSQL, écran par écran — **fait le 12/09/2026**

`outils/caisse_pg.py <url>` rejoue ce que la fenêtre fait contre un
serveur qui tourne : connexion (et le changement de mot de passe
imposé), puis chaque écran dans l'ordre d'une journée — tableau de
bord, POS (comptant, crédit, chèque), clients, créances, pièces
(devis → commande → BL + facture → validation → avoir), achats et
retours fournisseur, retour client, stock, magasins, transferts,
dépense, clôture, rapports, relances, chantiers, paramètres,
sauvegarde `pg_dump`. Par le même pont HTTP et les mêmes noms de
paramètres que `src/pages`.

Sur une base vierge amorcée avec la démo : **129 clics, 129 ok**.

Un seul défaut, et pas du SQL : `lire_catalogue_permissions` — la
commande qui ne lit pas la base — n'avait pas de poignée `Base`, donc
le registre la refusait sur PostgreSQL et l'onglet Rôles s'ouvrait
vide. Les 84 scénarios ne pouvaient pas le voir : ils appellent le
noyau, pas le registre. C'est exactement ce qu'un essai « à la main »
attrape. Corrigé : 187/187.

Au passage : le serveur affichait l'URL de la base **avec le mot de
passe** au démarrage — masqué (D10).

## Les fiches ramenées à l'essentiel — **fait le 12/09/2026**

`reseau-v2`, `permissions`, `numerotation`, `commandes-achat-fournisseur`,
`livraison-stock`, `environnement-windows` : 61 k → 27 k octets. Les
règles avec leur `fichier:ligne` restent ; les récits (« le bug qu'on
vient de fermer », « le piège que le test a attrapé ») sont ici. Zéro
lien cassé après réécriture.

## La restauration `pg_restore` jouée pour de vrai — **fait le 13/09/2026**

D4 l'exigeait : une sauvegarde n'existe que si la restauration marche.
Essai du cycle complet, avec les mêmes arguments que
`noyau::sauvegarde::pg_dump` (`--format=custom --no-password`, mot de
passe en `PGPASSWORD`) :

1. dump de `gescom_essai` → `gescom_backup_2026-09-13_essai.dump` (77 Ko) ;
2. base jetable neuve `gescom_restaure`, puis
   `pg_restore --clean --if-exists --dbname gescom_restaure …` ;
3. contenu vérifié : 36 tables, les deux comptes d'amorçage (Patron,
   Employé), 6 rôles, 1 dépôt ;
4. `gescom-serveur --base postgresql://…/gescom_restaure` démarre **sans**
   « BASE NEUVE » — l'amorçage reconnaît la base peuplée —, sert les
   187 commandes, répond HTTP 200.

Au passage : retiré le message de démarrage périmé « aucune des 186
commandes ne répond encore (D11) » (187/187 est servi depuis le 12/09),
et créé `deepseek-context/` — instantané d'état, plan et reste, pour
qu'un agent externe reprenne sans relire la conversation. (Ce dossier a
été replié dans `AI_CONTEXT/` et supprimé le 16/09/2026 : deux cartes du
même projet divergent, et la carte qui ne se met pas à jour ment.)

## La caisse écran par écran rejouée — **fait le 13/09/2026**

`caisse_pg.py` repassé contre un serveur PostgreSQL : **129 clics,
129 ok, 0 en erreur** — connexion, POS, pièces (devis → commande →
BL + facture → validation → avoir), achats, retours, stock, magasins,
transferts, dépense, clôture, rapports, relances, chèques, chantiers,
paramètres, et la sauvegarde `pg_dump` déclenchée par HTTP.

En route, un piège de plus du portage : la démo refusait de se semer
sur `gescom_essai` — `error serializing parameter 3`, colonnes restées
en `integer`/`real` 4 octets. Pas le code : la base gardait un schéma
d'avant `types_postgres` (les 21 tests PG passent sur base fraîche,
dont `les_donnees_de_demonstration_passent_sur_postgresql`). Base
jetable recréée à neuf → démo semée (8 articles, 4 clients). Leçon :
une base PostgreSQL d'essai créée par un vieux binaire ne s'élargit
pas toute seule — la recréer.

Au passage : `caisse_pg.py` affichait « connecté : None » — il lisait
`nom` alors que `/connexion` répond `utilisateur_nom` (« Patron »).
Corrigé dans le script.

Reste pour fermer l'essai à la main : la fenêtre Tauri elle-même,
jamais utilisée en mode caisse contre un serveur PostgreSQL.

## `--promouvoir` : le geste de secours du serveur — **fait le 13/09/2026**

ETAPES #3 / D6 : le rôle `superadmin` existe, personne ne le porte, et
c'est voulu — un compte de secours **livré** serait utilisable depuis
n'importe quelle caisse. La solution retenue : une commande du serveur,
qui exige d'être devant la machine.

`gescom-serveur --base <cible> --promouvoir IDENTIFIANT` cherche le
compte par pseudo, lui met le rôle `superadmin`, écrit un événement
`role_change` au journal (auteur `serveur`, origine `serveur`), puis
s'arrête — pas d'écoute, pas de session. Refuse « Aucun compte … »
(exit 1) ; un compte déjà porteur répond « porte déjà » sans rien
écrire. La logique vit dans `noyau/src/auth.rs`
(`promouvoir_superadmin_sur`, les deux moteurs) ; le serveur ne fait
que l'appeler avant de construire son `Serveur`.

Vérifié en vrai, pas seulement en test : base PostgreSQL jetable
`gescom_promo` créée, promotion d'`employe` → rôle en base, ligne de
journal, re-promotion idempotente, pseudo inconnu refusé, base
supprimée. Tests : trois scénarios ajoutés à `auth_base.rs`
(promotion, refus, idempotence) — 16/16 sur SQLite et sur PostgreSQL,
suite SQLite complète au vert (399 tests).

En route : `cargo test` ne relie pas `target\debug\gescom-serveur.exe`
— le premier essai CLI a tourné un binaire d'avant la modification et
s'est comporté comme si `--promouvoir` n'existait pas. Rebuild
explicite du binaire, puis tout a marché. Le piège est consigné dans
[environnement-windows.md](modules/environnement-windows.md).

## L'écran des permissions par personne — **fait le 13/09/2026**

ETAPES #2 / D7 : les commandes existaient depuis le portage, pas
l'interface. Paramètres → Utilisateurs : chaque personne qui n'a ni
rôle protégé ni accès total porte un bouton « Permissions », qui ouvre
[ModalPermissionsUtilisateur.tsx](../../src/components/ModalPermissionsUtilisateur.tsx) :
le catalogue groupé, une coche de l'effet réel, et **trois états par
permission** — « Rôle » (rien en base), « Autorisée » (`accorde = 1`),
« Refusée » (`accorde = 0`, le retrait l'emporte). « Tout remettre au
rôle » efface les réglages personnels (`accorde = null`), le troisième
état sans lequel revenir en arrière exigeait de se souvenir de ce que
le rôle accordait.

Le noyau n'a pas bougé : `lire_permissions_utilisateur` et
`definir_permission_utilisateur` existaient sur les deux moteurs. Le
chemin exact que la fenêtre envoie a été rejoué par HTTP contre le
serveur PostgreSQL : ajout → effectif, retrait d'une permission du rôle
→ plus effectif, retour au rôle → état initial, refus « Permission
inconnue » et « Utilisateur introuvable » rendus. TypeScript au vert
(`npm run typecheck`). La fenêtre elle-même reste à voir à l'écran
(item qui attend le mode caisse réel).

## Les images depuis une caisse : le contenu, pas le chemin — **fait le 13/09/2026**

ETAPES #4 / D8 : les commandes d'image recevaient un **chemin local**,
qui ne désigne rien chez le serveur. La caisse lit désormais le fichier
elle-même et envoie le **contenu** en base64 ; le serveur range les
octets dans **son** dossier d'images et enregistre le chemin en base.

Toute la logique vit dans [images.rs](../src-tauri/noyau/src/images.rs),
partagée par la façade et le serveur : extension sur liste blanche
(png, jpg, jpeg, webp, svg), 10 Mo maximum, base64 standard, trois
refus nets (« Format refusé », « base64 », « trop lourde »). Sur SQLite
le dossier est le parent du fichier de base ; sur PostgreSQL
`data_dir()/ml.gescom.app`. Six commandes de plus au registre, chacune
en deux poignées : `sauvegarder_logo/entete/pied`, `supprimer_*`.
Le serveur sert **193 commandes**. La façade `logo.rs` fond de moitié
(202 → 132 l.) : elle ne copie plus de fichier, elle appelle le noyau.
Le front lit le fichier choisi (`@tauri-apps/plugin-fs`), l'encode en
base64 et envoie `{nom, contenu}`.

Deux trous fermés en route :

- **`supprimer_*` n'existait pas côté serveur** — les boutons de
  suppression étaient morts en mode caisse. Et vider la colonne ne
  suffisait pas : le repli de lecture au dossier ferait **revenir**
  l'image. `supprimer` efface donc le fichier aussi, puis la colonne.
- **Le plafond HTTP de 8 Mio refusait une image légitime de 10 Mo**
  avant que le noyau la juge. `CORPS_MAX` passe à 16 Mio : le plus gros
  contenu légitime (10 Mo × 4/3 de base64 + l'enveloppe JSON ≈ 14 Mio)
  atteint le contrôle du noyau, qui le refuse proprement.

10 scénarios dans
[images_base.rs](../src-tauri/noyau/tests/images_base.rs) — aller-retour
écrire/relire, genres distincts, réécriture qui écrase, les trois
refus, sans dossier refusé plutôt que faire semblant, suppression qui
efface colonne **et** fichier, détecteur — passés sur SQLite **et** sur
PostgreSQL.

Vérifié en vrai : `caisse_pg.py` rejoué par HTTP sur base neuve —
**141 clics, 141 ok, 0 en erreur**, y compris le refus « Image trop
lourde : 10485761 octets, maximum 10485760. » rendu après un corps de
14 Mio passé sous le nouveau plafond. Mesuré au passage : 389 tests
noyau SQLite, 1 073 tests workspace, 34 tests PostgreSQL, 97 scénarios
en onze fichiers.

## L'entretien devient un travail du serveur — **fait le 13/09/2026**

ETAPES #5 / D9 : `entretenir_base` restait une commande locale, mais
la caisse n'a pas la base — réindexer et compacter n'était pas sa
place. La route `POST /entretien` du serveur (même permission que la
sauvegarde, `sauvegarde:lancer`) fait le travail, et la console gagne
sa carte Administration avec le bouton Entretien. L'écran des caisses
garde le diagnostic, perd le bouton « Réparer et compacter » — il
renvoie à la console.

Ce que fait la route, dans l'ordre : vérification d'intégrité (une
base corrompue est refusée : « Restaurer la dernière sauvegarde »),
réaffectation des règlements fournisseur globaux restés sans pièce,
copie avant dans le dossier des sauvegardes (`VACUUM INTO` sur SQLite,
`pg_dump` sur PostgreSQL), compactage (`REINDEX`+`VACUUM` / `VACUUM
(ANALYZE)` — pas de REINDEX sur PostgreSQL, autovacuum veille sur les
index et un REINDEX bloquerait les caisses). Le verrou de la base se
**garde** pendant toute l'opération : les caisses patientent, la
console conseille de le faire hors ouverture. Refus net sur une base
en mémoire, comme la sauvegarde. Journal : type `entretien`, origine
`serveur`.

5 scénarios dans
[entretien_base.rs](../src-tauri/noyau/tests/entretien_base.rs) — la
réaffectation sans changer les montants, l'idempotence, une base saine
sans rien à réaffecter, le refus en mémoire, le détecteur — passés sur
les deux moteurs.

**La vérification HTTP a attrapé un interblocage que les scénarios ne
pouvaient pas voir** : les scénarios testent le noyau, pas la route.
`POST /entretien` garde le verrou de la base pendant l'opération, or
`sauvegarde::dossier` — appelé pour savoir où ranger la copie — relit
le réglage par `Base` et **reprend le verrou** lui-même : le serveur
se mordait la main, plus aucune requête ne répondait. Corrigé en
calculant le dossier **avant** de prendre le verrou (le même ordre que
`maintenant`, la sauvegarde). Reconstruit, rejoué : 401 sans jeton,
`etat: ok` avec — copie lisible par `pg_restore --list` (201 entrées),
trace au journal `origine = serveur`. Leçon : les routes du serveur ne
sont pas testées automatiquement ; un test de route, même un seul,
vaudrait cher (dette reprise dans [ETAPES.md](ETAPES.md) § Dette
connue).

Mesuré en fin de séance, les deux moteurs : **394 tests noyau
SQLite**, **414 tests workspace SQLite** (les chiffres précédents —
389, 1 073 — ne se reproduisent pas, la présente mesure fait foi) ;
sur PostgreSQL, suite complète du paquet sur `gescom_test` : **394
tests, 0 échec**, dont **140 scénarios** répartis dans **seize**
fichiers `*_base.rs`.

## Revue du code de la séance, et la carte recentrée — **16/09/2026**

Relecture des quatre lots du 13/09 (D6 `--promouvoir`, D7 écran des
permissions, D8 images par le serveur, D9 entretien au serveur) contre
les règles de CLAUDE.md. `cargo check --workspace --all-targets` : exit
0, aucun avertissement.

**Ce qui tient.** La correction d'interblocage de `POST /entretien` est
la bonne — `sauvegarde::dossier` reprend le verrou, le calculer avant
règle le cas, et `sauvegarde_manuelle` suit le même ordre. D8 met sa
validation une seule fois dans le noyau, n'interpole que des noms de
colonnes tirés d'une liste fermée, et aucun chemin reçu du réseau
n'atteint `join` : le nom sur disque vient du genre. D9 refuse net sur
base en mémoire et sur base corrompue, et journalise avec `dossier_id`.
D6 écrit sous transaction, est idempotent, refuse un compte inconnu.

**Cinq écarts trouvés**, tous dans le chemin de l'entretien ou autour :

1. **La réimputation réécrit de l'argent hors transaction.**
   `reimputer_paiements_globaux_sur_base` passe `base` directement à
   `reallouer_globaux_sur`, qui fait `DELETE` puis N `INSERT` par
   règlement global. Chaque ordre s'exécute pour lui-même : une coupure
   entre les deux fait disparaître un paiement fournisseur. C'est la
   règle 4 de CLAUDE.md, et l'aide est écrite en `&mut impl Acces`
   précisément pour être appelée depuis une `tx` — c'est ce que fait
   `regler_dette_fournisseur_sur_base`.
2. **Sur SQLite, la « copie avant » est faite après la réimputation** :
   le `VACUUM INTO` vit dans `persistance::entretenir`, appelé après. Sur
   PostgreSQL le `pg_dump` précède bien. Le filet ne couvre donc pas
   l'étape qui touche à l'argent — celle qui en a le plus besoin (cf. 1)
   — alors que la console annonce une copie avant.
3. **La lecture des images oublie le dossier sur PostgreSQL** :
   `lire_*_base64` sur base fait `c.base.sqlite().and_then(…)` → `None`
   sur PG, quand l'écriture et la suppression utilisent
   `dossier_des_images_base`. Invisible tant que la colonne porte le
   chemin ; le repli ne trouve rien le jour où elle est vide.
4. **Changer d'extension laisse l'ancienne image sur le disque**
   (`logo.jpg` par-dessus `logo.png`), et le repli de lecture balaie
   `png` en premier : une colonne vidée fait ressortir l'ancien logo. Le
   test de réécriture ne couvre que png → png.
5. **D6 construit le JSON du journal par `format!`** : un nom contenant
   `"` ou `\` produit un `nouveau_valeur` invalide.

Mineurs notés : `--promouvoir` cherche `pseudo` seul quand la connexion
accepte `pseudo OR email`, et ne regarde pas `u.actif` ; la modale des
permissions garde une coche « effective » figée au chargement et
applique une commande par permission, sans rafraîchir si l'une échoue.
Structurel : faire prendre à `sauvegarde::dossier` un `&mut Base` ferait
refuser par le compilateur la reprise de verrou qui a mordu le 13/09.

**La carte recentrée.** `deepseek-context/` supprimé : son contenu
encore vrai est replié ici — la dette dans [ETAPES.md](ETAPES.md)
§ Dette connue, l'environnement de la machine dans
[modules/environnement-windows.md](modules/environnement-windows.md).
Un instantané figé à côté d'une carte tenue à jour finit par dire le
contraire d'elle, et rien n'indique laquelle a raison.

## R1 et R5 corrigés — **16/09/2026**

Les deux écarts de la revue qui pouvaient abîmer des données sont
fermés ; R2, R3, R4 restent ouverts et décrits dans
[ETAPES.md](ETAPES.md) § Revue du 16/09/2026.

**R1 — l'argent ne se réécrit plus hors transaction.**
`reimputer_paiements_globaux_sur_base` ouvre maintenant
`base.transaction()` et passe `&mut tx` à `reallouer_globaux_sur` et
`imputer_paiements_fournisseur_sur`. Répartir un règlement global, c'est
l'effacer puis le reposer en plusieurs lignes : hors transaction, une
coupure entre le `DELETE` et les `INSERT` faisait **disparaître un
paiement fournisseur**. Les deux aides étaient déjà écrites en
`&mut impl Acces` pour ça — c'est ainsi que les appelle
`regler_dette_fournisseur_sur_base` — seul l'appel de l'entretien
passait `Base` nue. Rien d'autre n'a bougé : l'entretien PostgreSQL fait
toujours son `VACUUM (ANALYZE)` **après** que la transaction est
validée, le moteur le refuserait dedans.

**R5 — le journal de `--promouvoir` est sérialisé.**
`format!(r#"{{"nom":"{nom}"…}}"#)` devient
`serde_json::json!({ … }).to_string()` : un nom portant un guillemet ou
une barre oblique inverse produisait un `nouveau_valeur` que plus rien
ne relisait. Scénario ajouté dans
[auth_base.rs](../src-tauri/noyau/tests/auth_base.rs) — le compte est
renommé `Ba "Le Grand" \ Traoré`, promu, et la ligne de journal est
relue par `serde_json::from_str` puis comparée champ à champ.

Mesuré : `cargo check --workspace --all-targets` au vert sans
avertissement ; **395 tests noyau SQLite** (394 + le nouveau), 0 échec ;
sur PostgreSQL (`gescom_test`), `auth_base` **17/17** et
`entretien_base` **5/5**, 0 échec — c'est la transaction de R1 qui est
vérifiée là, sur le moteur où elle compte le plus.

Ce que cette correction ne prouve pas : aucun scénario ne coupe
l'alimentation au milieu d'une réimputation. L'atomicité repose sur la
transaction du moteur, pas sur un test — et les routes HTTP, elles, ne
sont toujours pas testées.

## R2 corrigé — la copie précède la réparation — **16/09/2026**

`persistance::entretenir` faisait deux choses en une : le `VACUUM INTO`
qui produit la copie, puis le `REINDEX; VACUUM;` qui compacte. Comme
l'appelant réparait les règlements fournisseur globaux **avant**
d'appeler cette fonction, la copie « de sécurité » contenait déjà la
réaffectation — elle ne protégeait que du compactage, jamais de l'étape
qui déplace de l'argent. Le chemin PostgreSQL, lui, faisait son
`pg_dump` en premier : les deux moteurs ne racontaient pas la même
histoire, et la console annonçait celle de PostgreSQL.

La fonction se scinde en **`copier_avant`** et **`compacter`**, et
`entretenir_base_sur_base` suit maintenant l'ordre annoncé : intégrité →
copie → réimputation → compactage. Aucun autre appelant à reprendre,
`entretenir` n'en avait qu'un.

Scénario ajouté dans
[entretien_base.rs](../src-tauri/noyau/tests/entretien_base.rs) : après
un entretien qui réaffecte, la base vivante n'a plus de règlement
global, mais **la copie rouverte en contient encore un**. SQLite
seulement — un dump PostgreSQL ne se relit pas comme une base.

Mesuré : **396 tests noyau SQLite**, 0 échec ; `entretien_base` **6/6**
sur PostgreSQL. R3 et R4 (les images) restent ouverts.

### Un scénario instable, trouvé en passant

`gestion_base::une_creance_se_regle_en_deux_fois_puis_un_reglement_s_annule`
échoue environ une fois sur trois : deux règlements écrits dans la même
seconde, et `lire_reglements_client_sur_base` trie `ORDER BY
p.date_paiement DESC` **sans départage** — l'ordre des deux lignes est
alors celui que le moteur veut, et le test lit `reglements[1]`. Sans
rapport avec R1 ni R2 : le chemin des créances n'a pas été touché, et
la suite complète repasse au vert au ré-essai. Noté dans ALERTES.md pour
qu'un échec de ce test n'accuse pas le prochain chantier. Non corrigé :
le remède (départager par `id` ou par `cree_le`) change un ordre
d'affichage, ça se décide.

## R3 et R4 corrigés — les images — **16/09/2026**

Les deux derniers écarts de la revue, tous deux sur le chemin des
images (D8), tous deux invisibles tant que la colonne `*_chemin` porte
un chemin valide — et tous deux prêts à mordre le jour où elle est vide,
c'est-à-dire après une restauration.

**R3 — la lecture retrouve le dossier sur PostgreSQL.** Les trois
`lire_*_base64` sur base calculaient leur repli par
`c.base.sqlite().and_then(dossier_des_images)` : `None` sur PostgreSQL,
où il n'y a pas de fichier de base à côté duquel chercher. L'écriture et
la suppression, elles, utilisent `dossier_des_images_base`
(`data_dir()/ml.gescom.app` sur PG). Le serveur rangeait donc ses images
dans un dossier que sa propre lecture ne regardait jamais. Les trois
poignées appellent maintenant la même fonction que les six autres.

**R4 — changer d'extension n'abandonne plus l'ancien fichier.** Le nom
sur disque vient du genre, l'extension du fichier choisi : poser un
`logo.jpg` par-dessus un `logo.png` laissait le png en place. Comme le
repli de lecture balaie les extensions dans l'ordre — `png` d'abord —
une colonne vidée faisait revenir l'ANCIEN logo, pas le dernier posé.
`poser_fichier` écrit le nouveau fichier **puis** efface les autres
extensions du même genre ; dans cet ordre, pour qu'un échec d'écriture
laisse l'image précédente en place. `supprimer` faisait déjà ce balayage
— c'est de là que vient le remède.

Scénario ajouté dans
[images_base.rs](../src-tauri/noyau/tests/images_base.rs) : png puis
jpg, le png a disparu du disque, et la colonne vidée le repli rend bien
le jpg. Passé sur les deux moteurs (11 scénarios), **397 tests noyau
SQLite**, 0 échec.

Ce que R3 n'a pas : un test. Le registre du serveur (`socle.rs`) n'en a
aucun, comme les routes — la correction s'est faite en lisant les neuf
poignées côte à côte, et rien n'aurait signalé l'écart. C'est la même
dette que le 13/09.

## La fenêtre contre le navigateur, et les modèles qui servent — **17/09/2026**

Séance ouverte par un essai réel dans la **fenêtre Tauri** (item 8
d'ETAPES, jamais fait). Elle a rendu visible ce qu'aucun essai HTTP ne
pouvait montrer.

**Le glisser-déposer ne marchait pas — et ce n'était pas le code.** Sur
Windows, `dragDropEnabled` vaut vrai par défaut : WebView2 installe son
propre récepteur, celui qui sert à lâcher des *fichiers* sur la fenêtre,
et il avale `dragstart`/`dragover`/`drop` avant la page. React ne
recevait jamais rien. Tous les essais passant par un navigateur, où rien
n'intercepte, personne ne l'avait vu. Un mot dans `tauri.conf.json`.
**Leçon générale : un essai par HTTP ne remplace pas la fenêtre.**

**L'atelier se pilote maintenant dans le document.** Cliquer dans
l'aperçu choisit le bloc, et on peut y **lâcher** un bloc avec un trait
qui montre où il tombe. L'aperçu passe de `sandbox=""` à
`sandbox="allow-same-origin"` — toujours **pas** de `allow-scripts`,
D50 tient : la page reste inerte, c'est l'atelier qui gagne le droit de
lire ce DOM. Ajoutés au passage : gras/souligné, un bloc Image
dimensionné en millimètres, et l'habillage des tableaux (filets,
couleurs, un rayon par coin — un `border-collapse: collapse` ignorant le
`border-radius`, un cadre porte l'arrondi).

**Les modèles servent enfin.** `imprimerParModele` était juste — vérifié
par sonde HTTP contre PostgreSQL, les 7 modèles répondent — mais
**personne ne l'appelait**, et son contrat (imprimer sans rien montrer)
ne correspondait à aucun écran. `ApercuPiece` liste désormais les
modèles, rend celui qu'on choisit **avec les vraies données**, et
imprime ce qu'on a vu. L'atelier devient un onglet de Paramètres qui
prend tout l'écran ; en-tête et pied quittent l'écran Société pour
devenir des blocs Image.

**Deux corrections de dates, venues du terrain.** Une boutique note sur
papier et saisit le soir : imposer la date de saisie fait tomber la
pièce dans le mauvais mois. `modifier_piece` prend `date_piece` (deux
versions, façade, serveur), et la règle d'immuabilité de `coeur` décide
seule de quand c'est permis. **La règle posée : deux dates, deux faits.**
La pièce dit quand l'affaire a eu lieu ; le mouvement de caisse ne bouge
pas — l'argent est entré dans le tiroir quand il y est entré, et la
caisse se lit **par session, jamais par date** (vérifié : toutes les
requêtes filtrent sur `session_id`, et `ouvrir_session_caisse` refuse
d'ouvrir une session passée). Antidater l'entrée changerait après coup
une session close et comptée.

**Les filtres de stock existaient déjà** dans le noyau — dates, article,
dépôt, type — et l'écran n'en passait aucun : 300 derniers mouvements,
recherche en mémoire. Un article sorti le mois dernier restait
introuvable **sans que rien ne le dise**, sur l'écran même où l'on va
justifier un écart d'inventaire.

Ajoutée aussi : la colonne `reference` sur `piece_commerciale` (le
numéro que le fournisseur porte sur *sa* facture), posée sur les trois
chemins de création et vérifiée par `schema_commun`.

Six commits, 398 tests noyau au vert.

### Deux défauts trouvés en fin de séance

- **La marge manquait dans l'aperçu d'un modèle** : rendu sans
  `apercu: true`, il ne portait que `@page`, qui ne fait **rien** dans
  une iframe. Le document s'affichait collé aux bords. L'aperçu rend
  maintenant deux géométries du même passage — écran et papier — et
  `designable` sépare le surlignage des blocs (atelier seul) de la
  géométrie de page.
- **Vingt blocs Image partagent une seule image.** Un bloc Image désigne
  l'un des **trois** emplacements de la société (logo, en-tête, pied) :
  vingt blocs pointent donc sur trois fichiers, et en remplacer un les
  change tous. C'est la limite de conception, pas un accident — mais
  elle bloque l'usage réel (un cachet, une signature, un QR). Il faut
  des **images propres au modèle**, avec une identité stable ; c'est le
  chantier suivant.

## I3 et I1 sur une branche, pendant que la fenêtre sert — **17/09/2026, suite**

L'utilisateur teste l'application à la main. Le watcher de `tauri dev`
rebâtit et **redémarre la fenêtre** à chaque fichier Rust touché : on
ne peut pas développer dans l'arbre qu'il surveille. D'où un second
répertoire de travail git (`.claude/worktrees/dates-images`, branche
`travail/dates-images`) avec son propre `target` — un `cargo check`
à froid, puis tout est incrémental. La fusion dans `main` attend la
fin des tests manuels, pour la même raison.

Au passage, le serveur d'essai a été **reconstruit et relancé depuis
`binaires/`** plutôt que depuis `target/debug/` : c'est le sidecar
que `tauri-build` recopie, et un serveur lancé depuis `target/debug`
verrouillait le fichier (le piège du 13/09). Lancé depuis `binaires/`,
le fichier de `target/debug` reste libre, et `tauri dev` recopie un
sidecar **à jour** — 194 commandes, le code du jour.

**I3 — la date d'un règlement.** `regler_creance_datee*` et
`regler_dette_fournisseur_datee*`, les fonctions d'origine restant des
enveloppes (20 appels intacts). Même règle `coeur::dates`, même
permission, même partage : le paiement porte la date de l'affaire, le
mouvement de caisse reste au jour — entrée côté client, **sortie** côté
fournisseur — avec « Règlement du jj/mm ». Un détail qui comptait : les
deux `INSERT` écrivaient `date_paiement` et `cree_le` avec le même
paramètre ; ils se séparent, sinon la date de l'affaire aurait aussi
été la date de création de la ligne.

**I1 — les images posées sur un document.** Table `image_document`,
`imageId` sur le bloc Image, quatre commandes, l'atelier qui choisit
entre les images de la société et les images posées. **Supprimer
refuse tant qu'un modèle pose l'image et nomme lesquels** — la règle
du modèle d'usine, appliquée aux images. **L'export emporte les
images** (version d'échange 2) et l'import les repose sous le même
identifiant, sans doublon ; un lot v1 se lit toujours.

**Trouvé en chemin, non corrigé (I5)** : `exporter_modeles` et
`importer_modeles` sont des commandes **locales** (elles lisent et
écrivent un fichier) qui touchent la **base locale de la caisse** —
vide en mode poste. Depuis une caisse, l'export sort un lot vide et
l'import écrit dans une base que le serveur ne verra jamais. Le geste
doit lire/écrire le fichier ici et parler au serveur pour le contenu.

Mesuré sur la branche : **412 tests noyau SQLite**, 0 échec ;
`gestion_base`, `fournisseurs_base`, `images_base`, `schema_commun`
au vert sur PostgreSQL ; typecheck et build front au vert.

## 17/09/2026 (suite) et 18/09/2026 — l'irrécouvrable, l'avoir accordé, les flottants, la palette

**I4 — la référence du tiers.** `definir_reference_piece`, la colonne
en fin de liste. Le script de vérification a attrapé un décalage : la
liste fournisseur de la fenêtre n'a pas `credit_ouvert`, la référence
y est à l'indice 18 et non 19. Les listes de la fenêtre construisent
toujours leur `WHERE` par `format!` sur des *conditions* (pas des
valeurs) — dette notée, pas réglée.

**L'irrécouvrable qui se réglait quand même.** Une créance sortie des
comptes acceptait un règlement ordinaire et revenait
« partiellement payée » en silence. Deux règles pures :
`peut_regler(statut)` refuse `irrecouvrable` et `annulee`, et
`statut_apres_recouvrement(total, paye)` ne rend `payee` que si tout
est rentré. Le **règlement exceptionnel** est son propre geste, avec
son propre motif de caisse (`recouvrement`) — le tiroir dit ce que
c'est. Au passage, un test bégayait : deux paiements écrits dans la
même tranche de 15 ms (l'horloge Windows) avaient la même
`date_paiement`, et la somme cumulée (`p2.date <= p.date`) comptait
les deux pour chacun. Le sous-requête départage maintenant par
`(date_paiement, cree_le, id)`.

**L'avoir accordé.** « Des avoirs liés à un client sans articles » :
un geste commercial, un dédommagement. Il fallait une permission que
**le patron seul** porte : `avoirs:accorder` n'entre dans aucun rôle
livré, elle vient avec `acces_total` ou se donne à la main. L'avoir
naît sans retour et sans `ligne_piece` — la table exige un article.
Le document imprimé sortait donc à zéro : `lire_donnees_piece` pose
une **ligne d'affichage** tirée de `avoir.montant` quand un AVC n'a
aucune ligne. Rien n'est écrit, c'est le rendu qui complète.

**Les blocs flottants.** « Un rectangle peut être sur un rectangle et
on doit pouvoir redimensionner sur l'aperçu. » Un cadre `flottant`
sur n'importe quel bloc, posé en absolu dans une `.feuille` dont le
coin haut-gauche est celui de la zone imprimable — la même origine à
l'écran (le corps porte la marge en `padding`) et sur le papier
(`@page` porte la marge). Deux flottants se recouvrent librement ; le
`z-index` est le rang dans la structure, donc réordonner la structure
change qui passe devant. Dans l'aperçu, l'atelier pose des écouteurs
`pointer*` sur le DOM de l'iframe (toujours sans `allow-scripts`,
D50) : on tire le bloc, ou son coin bas-droit ; le style bouge en
direct, le modèle n'est écrit qu'au relâcher. Le dépôt dans l'aperçu
passait par un *rang* parmi les blocs rendus — un bloc caché ou
flottant décalait la cible ; il passe par l'*identifiant* du bloc
devant lequel on lâche. Vérifié par un rendu hors application
(esbuild + Edge sans fenêtre) : image sur le tableau, « PAYÉ »
par-dessus les deux.

**La date de la réception.** Même règle, même permission, même partage
que la vente : la facture fournisseur et le règlement portent le jour
de l'affaire, le stock et la caisse bougent au jour de la saisie —
« Réception du jj/mm » en libellé de caisse. `enregistrer_achat_date`
dans les deux versions, l'original en enveloppe.

**La palette de commandes.** Ctrl+K partout. Les actions globales sont
celles du menu, avec les mêmes droits ; les onglets de Paramètres, et
le compte. Chaque écran déclare les siennes tant qu'il est monté
(`useActionsPalette`) — la caisse propose *ouvrir* ou *fermer* selon
l'état du tiroir. Une leçon de rechargement à chaud : exporter une
constante depuis un fichier de composant casse le Fast Refresh de ce
fichier ; la liste des onglets vit dans `lib/onglets-parametres.ts`.

Un accident en chemin : un `gescom_noyau:   :creances` dans
`commandes/creances.rs` — une frappe dans l'éditeur, pas un lot —
faisait échouer la façade Tauri ; remis d'aplomb.

Mesuré : **438 tests workspace SQLite**, 0 échec, 0 avertissement ;
`gestion_base`, `pieces_base`, `achats_base`, `fournisseurs_base`,
`postgres_amorcage` au vert sur PostgreSQL ; typecheck front au vert ;
serveur d'essai relancé depuis `binaires/` avec **200 commandes**.

## 18/09/2026 (suite) — I5, la palette qui cherche, Pièces, et la v2 close

**I5.** L'export et l'import des modèles étaient des commandes
*locales* qui lisaient et écrivaient un fichier **et** touchaient la
base de la caisse — vide en mode poste. Le découpage : le **serveur**
rend le lot (`exporter_modeles`, lecture) et reçoit le lot
(`importer_modeles`, `modeles:gerer`), sur les deux poignées ; le
**poste** lit et écrit le fichier avec `plugin-fs` (nouvelle capacité
`fs:allow-write-text-file`). Les façades Tauri gardent la même forme
(lot en entrée / en sortie), sans fichier. Vérifié par HTTP sur un
serveur jetable (SQLite, port 7399) : import d'un lot de deux, export
qui les rend.

**La palette cherche les tiers.** Deux lettres suffisent : cinq
clients, cinq fournisseurs, par les commandes paginées déjà là, avec
les droits du menu ; choisir ouvre la fiche. La réponse d'une frappe
dépassée est jetée. La fenêtre s'élargit.

**Pièces.** Du → au dans la barre, pour les deux côtés — la liste
fournisseur n'avait pas de bornes de date, elle en a (deux versions,
paramètres liés côté `Base`, scénario). Et le filtre dit le type :
sur « Commandes », le bouton crée une commande sans redemander ; sur
« Tout », le sélecteur revient. Un type qu'on ne crée pas à la main
(avoir fournisseur) retombe sur le choix.

**La v2 est close**, sur décision du propriétaire. Restent notés
l'installeur non signé (D5), l'impression papier jamais vérifiée à la
main, et le déclencheur de stock multi-dossier (v3).

Mesuré : **438 tests workspace SQLite**, 0 échec ; `pieces_base` au
vert sur PostgreSQL ; typecheck front au vert ; serveur d'essai relancé
avec **202 commandes**.

## 18/09/2026 (suite 2) — palette plus large, la date à la création

Deux retouches après la clôture. La palette passe à `max-w-3xl` — trop
étroite pour lire confortablement le détail d'un client trouvé.

Et **la date se saisit aussi à la création d'une pièce**, pas
seulement à son échéance de paiement : `creer_piece[_sur_base]` et
`creer_piece_fournisseur[_sur_base]` prennent `date_piece`, validée
par la même règle que la réception et le règlement
(`argent::date_de_la_piece`, fenêtre de 31 jours) et gardée par
`pieces:antidater` côté serveur — même geste que `modifier_piece`.
Rien d'autre ne bouge avec elle : `creer_piece*` ne touche ni le stock
ni la caisse, ça n'est pas son rôle. Champ « Date de la pièce » dans
`ModalNouvellePiece.tsx`, visible seulement à qui a le droit ; scénario
`une_piece_peut_naitre_deja_datee` (sans date, avec date dans la
fenêtre, trop loin dans le passé → refusée, rien n'est créé).

Mesuré : **439 tests workspace SQLite**, 0 échec ; `pieces_base`
18/18 sur PostgreSQL ; typecheck front au vert.

## 18/09/2026 (suite 3) — six retouches d'après capture

**L'avoir fournisseur remboursé n'est pas un impayé.** La liste des
pièces calculait `reste = total − payé` pour un AVF comme pour une
facture : un retour remboursé en espèces (statut `paye`, argent déjà
rentré) affichait 5 000 F de reste en rouge. Ce n'était pas la
démonstration, c'était la liste. La règle existait déjà, recopiée trois
fois dans `fournisseurs.rs` ; elle est maintenant dans `coeur`
(`credit_avoir_fournisseur` : remboursé ou annulé → 0, sinon le crédit)
et sert partout. Le reste d'un avoir se lit en ambre, et le pied de
tableau n'additionne que les factures. Scénario dans `achats_base`.

**La caisse plantait** depuis la palette : `useActionsPalette` était
appelé après le `return` du chargement — un rendu avec un hook de
moins, React jette l'écran. Remonté avant le retour anticipé. Les
autres pages déclarent leurs actions avant tout `return`, vérifié.

**La palette ne s'élargissait pas** : `DialogContent` porte
`sm:max-w-sm`, qui l'emporte sur `max-w-3xl` sans préfixe. `sm:max-w-3xl`.

**Chaque bloc se redimensionne dans l'aperçu.** Un bloc du flux prend
100 % de la largeur, c'est le flux ; tirer son coin bas-droit le
DÉTACHE (flottant, au cadre qu'il occupait) et le redimensionne dans le
même geste — plus besoin de trouver la case à cocher. La case garde
aussi le cadre à l'écran au lieu de sauter en (0,0). Le pied de page ne
se détache pas.

**« Client » devient « Fournisseur »** sur une pièce fournisseur, pour
tout champ posé sur `tiers.*` — sans retoucher les modèles enregistrés.

**Paramètres → Société** perd la section « Signatures » : les noms
vivent dans le bloc Signatures de chaque modèle ; les commandes
`*_config_signatures` restent pour le rendu de secours sans modèle.

Mesuré : **440 tests workspace SQLite**, 0 échec ; `achats_base`
9/9 sur PostgreSQL ; typecheck front au vert ; serveur d'essai
relancé, la liste des AVF vérifiée par HTTP (remboursé → reste 0).

## 18/09/2026 (suite 4) — la facture irrécouvrable, en rouge et hors total

Une vente passée en irrécouvrable (Paramètres → Irrécouvrable) sortait
déjà des créances ; sa facture, elle, restait dans la liste des pièces
comme un impayé ordinaire, et son reste s'additionnait au bas de
l'écran. Les listes client portent maintenant `irrecouvrable` (colonne
20, ajoutée en dernier — `CASE WHEN EXISTS … THEN 1 ELSE 0 END`, un
entier parce que PostgreSQL rend un booléen sur `EXISTS` nu). Les
filtres « impayés » et « en retard » l'écartent, sur les deux versions.
À l'écran : toute la ligne en rouge, badge « Irrécouvrable », et elle
ne compte dans aucun total, comme une pièce annulée — le pied de
tableau dit combien sont hors total et pourquoi.

`target/` (l'essai `pg_restore` du 13/09) est ignoré par git.

Mesuré : `gestion_base` 10/10, `pieces_base` 18/18, `listes_base` 6/6
sur PostgreSQL ; typecheck front au vert.

## 19/09/2026 — le service, l'installeur signé, les documents, la v3 commence

**Le serveur en service Windows (D12).** Un serveur qu'on lance à la
main ne tourne pas le jour où personne n'a ouvert de session.
`service.rs` parle à l'API Win32 par `windows-sys` (déjà dans le cache
local — pas de réseau vers crates.io ce jour-là) : `sc create` avec
relance sur incident, gestionnaire de contrôle, sortie redirigée dans
`ProgramData\Gescom\serveur.log` par `SetStdHandle` (que `println!`
relit à chaque écriture). `main.rs` se scinde en `preparer` et
`boucle` ; l'écouteur devient non bloquant pour que « sc stop » soit
entendu en 200 ms au lieu d'attendre la prochaine caisse. Configuration
dans `serveur.json`, la ligne de commande passe devant. Vérifié à la
main hors service (connexion, arrêt net) ; **le service lui-même n'a
pas pu être installé depuis cette session** — l'élévation est bloquée
— c'est la section A des tests manuels, script prêt.

**L'installeur du serveur, signé (D5 révisée).** Ce que la fenêtre ne
peut pas faire (droits utilisateur), un installeur à part le fait en
administrateur : service, pare-feu sur le port, certificat auto-signé
enregistré comme éditeur de confiance de la machine, configuration
demandée une fois et jamais écrasée, désinstallation qui garde la base.
`signer.ps1` crée le certificat une fois (clé privée dans le magasin,
jamais dans le dépôt), `construire_installeur_serveur.ps1` enchaîne
compilation, signature, makensis (celui que Tauri a téléchargé),
signature. Deux scripts PowerShell sans BOM : lus en ANSI, cassés sur
les accents — BOM ajouté, comme sur les autres. 2,4 Mo, signé,
`Get-AuthenticodeSignature` dit « racine non approuvée » jusqu'à ce que
l'installeur enregistre le certificat : c'est attendu.

**Les sockets.** Rien à faire : le serveur écoute déjà `0.0.0.0:7300`
(toutes les adresses, `--hote`/`--port`) et le canal d'événements
existe. La configuration du service permet de fixer les deux.

**Les documents.** `TESTS-MANUELS.md` (dix sections, résultat attendu à
chaque ligne — ce que les tests automatiques ne voient pas) ;
`MANUEL.md` passe en 2.0 : installation serveur/caisses, Ctrl+K, dates
saisissables, modèles à la place des signatures de Société, avoir
accordé, irrécouvrable, « Réception du ».

**La v3 commence (D13).** Le plan avait tout tranché ; c'est branché :
`lire_dossiers_sur`, `creer_dossier_sur` (une transaction : dossier,
exercice de l'année, magasin par défaut, client de passage — refus sur
SQLite, où les commandes `Connection` ne servent que le dossier
d'origine), le dossier mémorisé par personne dans `config_app`. La
session porte son dossier (`session_reseau.dossier_id`, ajoutée sur les
deux chemins de création — le serveur sur fichier passe par
`persistance/v2.rs`, pas par `amorcage`, et la première connexion l'a
dit). `Appelant` porte `dossier_id` et `session_id` ; `api::rpc` refuse
tout sauf `choisir_dossier` tant qu'il n'y a pas de dossier, et pose la
`Base` sur le dossier avant chaque poignée. `Registre::sur_base` : une
commande née sur `Base` seule, servie par la `Base` sur les deux
moteurs. Écran : « Quel dossier ouvrir ? » dans `PageLogin`, dossier
affiché sous le nom dans `Layout`. Vérifié par HTTP sur `gescom_essai` :
un dossier ESSAI2 créé, la reconnexion demande, le choix mémorise, le
second choix refuse, le nouveau dossier ne voit que son client de
passage. **210 commandes, 29 permissions.**

Mesuré : `dossiers_base` 5/5, `auth_base` 17/17, `postgres_amorcage`
21/21 sur PostgreSQL ; suite SQLite complète : **446 tests, 0 échec**.

## 19/09/2026 (suite) — cinq retours du terrain

**Le code client cassait.** « UNIQUE constraint failed: client.code »,
à chaque nouveau client, depuis la nouvelle base : le code suivait
`COUNT + 1`. Qu'un client soit supprimé ou qu'un import porte ses
propres numéros, et le compte retombe sur un code existant — jusqu'à
ce qu'il rattrape le trou. C'est le piège que D28 nomme pour les
pièces ; il vaut pour les tiers. Le code suit le plus grand déjà pris
(`coeur::tiers::code_client_suivant`), et un second dossier préfixe
les siens de son code : `client.code` est unique sur toute la base,
`QUINC-CLIENT00001` ne se heurte pas à `CLIENT00001`. Sur PostgreSQL,
`SUBSTR(code, ?2)` avec un entier lié refusait (« error serializing
parameter ») : `LENGTH(?2) + 7` avec le préfixe en texte.

**La remise globale au POS**, en % ou en francs. Pas stockée à part —
l'invariant est `SUM(prix_pratique × quantité) = dû` — mais répartie
sur les lignes au prorata, la dernière prenant le reste ; le total
affiché est celui des lignes baissées, donc ce qui part et ce que le
client paie.

**L'avoir sans marchandise depuis « Nouvelle pièce »** : type Avoir,
pas de ligne, un montant, le motif dans Note. Même commande que la
fiche client (`accorder_avoir_client`), même permission ; l'écran dit
pourquoi quand on ne l'a pas.

**L'impression « parfois »** : le fichier temporaire gardait le nom
demandé — deux fois le même pour la même pièce, et l'aperçu passait
le numéro nu, sans `.html`. WebView2 tenait encore l'ancien fichier
ou le sortait de son cache, ou devinait le type. Nom unique + `.html`
toujours, un dossier à part nettoyé après un jour, et un second label
si la fenêtre d'avant n'a pas fini de se fermer.

**Le tableau de bord** perd ses icônes ; l'intitulé passe en tête,
lisible.

## 19/09/2026 (suite 2) — le bon de livraison naît en brouillon

Modifier un bon de livraison émis laissait le stock tel quel : « 6
sacs » corrigé en « 9 » sur le papier, six sortis du magasin — le
trou que le propriétaire a nommé. La règle, dans `coeur` : un bon
(livraison, réception) **constate** un mouvement physique, donc il se
**prépare** en brouillon (rien ne bouge, on corrige librement),
**s'émet** (tout est livré d'un coup, `marquer_entierement_livre`, dans
la même transaction que le statut), puis il est **figé** — pour
corriger, l'annuler (le stock revient par le même chemin, écart négatif
ligne à ligne : `marquer_rien_livre`) et en refaire un. La saisie ligne
à ligne ne vient qu'après l'émission, pour l'écart (le livreur revient
avec deux sacs). Un bon en brouillon ne se facture pas : la facture
issue d'un bon ne bouge rien, la marchandise ne serait jamais sortie.
Le retour émis → brouillon est refusé. Par conversion d'une commande,
le BL naît émis et livré, comme avant : la marchandise part avec lui.

Six scénarios réécrits pour suivre la règle (ils livraient sur des
brouillons), un ajouté (`un_bon_de_livraison_cree_a_la_main_nait_en_
brouillon_et_ne_sort_le_stock_qu_a_l_emission`), trois tests purs dans
`coeur`. Écran : bouton « Émettre » sur un bon en brouillon, plus de
« Modifier » ni de « Livraison » ligne à ligne sur un bon émis /
brouillon respectivement.

