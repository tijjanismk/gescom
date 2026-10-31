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

## 19/09/2026 (suite 3) — la livraison, ses filtres, et ce que fait le stock

**« missing field ligne_id »** à la saisie d'une livraison : l'écran
envoyait `ligneId` / `quantiteLivree` — en camelCase, comme les
arguments de premier niveau — mais un champ imbriqué n'est renommé par
personne, et `LigneLivraison` attend `ligne_id`. La structure accepte
les deux (`serde(alias)`), l'écran envoie désormais la même graphie que
les lignes d'une pièce. Trouvé par le propriétaire, pas par un test :
aucun scénario ne passait par le JSON de l'écran.

**Les filtres suivent la livraison.** Quand le suivi est actif, un
filtre Livraison / Réception (non livré, partiel, livré) — le troisième
axe, celui qui répond à « payé, pas livré ». En passant : le filtre
Échéance existait et ne s'appliquait jamais ; la case Impayés envoyait
`impaySeulement`, que le serveur ne lisait pas. Les deux marchent.

**Annuler par avoir une facture issue d'un BL.** Le bon a fait sortir
la marchandise, la facture n'a rien sorti (`stock_confie_a_un_bon`) ;
l'avoir d'annulation constate un retour : la marchandise **revient une
fois**, le bon garde sa trace « livré », sortie + retour = point de
départ, et l'historique dit les deux mouvements. Scénario ajouté pour
que la réponse reste vraie.

**« Une livraison vient de telle commande » est dans les tables** :
`piece_commerciale.piece_origine_id`, posé à la conversion, et c'est
lui que remonte `stock_confie_a_un_bon`. À l'écran, la flèche ↗ à
côté du numéro.

## 20/09/2026 — la dette qu'on pouvait payer sans le propriétaire

Ce qui restait de la v2 se partageait en deux : ce qui demande le
propriétaire devant sa machine (le service en élevé, le papier, deux
postes) et ce qui ne demande que du temps. Le second lot, ce jour.

**Restaurer, enfin.** Depuis le 13/09 la restauration était « jouée à
la main » — une commande `pg_restore` dans une fiche. Elle est dans le
serveur : `gescom-serveur --restaurer FICHIER`, hors ligne, qui refuse
un fichier qui n'est pas une base (`PRAGMA integrity_check`, table
`vente` exigée), garde l'ancienne base à côté, et dit quoi faire
ensuite. Pas de bouton dans l'écran, et c'est voulu (D4).

**Trois tests de routes.** Le noyau avait 158 scénarios et le serveur
zéro — c'est par là qu'étaient passés l'interblocage du 13/09 et le
« missing field ligne_id » du 19/09. `serveur/tests/routes.rs` lance le
vrai exécutable sur une base temporaire et un port libre, parle HTTP à
la main (pas de client dans les dépendances de test), et rejoue :
connexion / mauvais mot de passe / jeton révoqué / commande inconnue /
permission refusée ; un bon de livraison qu'on émet, qu'on livre avec
le JSON de l'écran en snake **et** en camel, le stock qui suit ; une
sauvegarde, un client créé après, `--restaurer`, le serveur relancé,
le client absent. Soixante secondes chacun sur cette machine (le
binaire en debug s'amorce lentement), et ils attrapent ce que la
lecture laissait passer.

**Le chèque rejeté.** Il supprimait le paiement et le mouvement. Il
écrit maintenant l'inverse — paiement négatif, sortie de caisse — et
ne demande pas de caisse ouverte : la banque refuse quand elle veut,
pas aux heures d'ouverture (D14). Le scénario du cycle du chèque a
changé d'assertions : deux paiements dont la somme fait zéro, une
sortie `cheque_rejete`, la créance rouverte.

**Cinq listes qui collaient du texte dans le SQL.** Les versions
`Connection` de `lire_toutes_pieces_client/fournisseur` et des trois
paginations (clients, stocks, fournisseurs) échappaient l'apostrophe à
la main et laissaient `%` et `_` faire joker. Filtres liés (`?n`),
un scénario `filtres_lies` qui cherche « O'Brien 100% » et le trouve.

**Deux cosmétiques.** `ModalImpression` (le POS avait sa propre
impression, sans les modèles) est retiré : après la vente, le même
`ApercuPiece` que dans Pièces. Et les étiquettes dessinent un vrai
EAN-13 — un SVG de 95 modules à 0,33 mm, pas des barres en CSS —
avec le numéro en clair dessous ; les trois codes d'essai se relisent
module par module. À vérifier à la douchette sur du papier (TESTS-
MANUELS, H).

458 tests SQLite, 0 échec ; PostgreSQL vert sur les cinq fichiers
touchés. Rien de committé : lot posé avec celui du 19/09 (suite 3).

## 20/09/2026 (suite) — rendre ce qu'on a payé avec un avoir

Question du propriétaire : « comment les retours sur factures avec
remise sont gérés, est-ce logique, manque-t-il une fonctionnalité de
base ? » La remise, elle, est bien gérée : fondue dans `prix_pratique`
(au POS par prorata, sur une facture par ligne → globale → TVA), et le
retour crédite ce prix net — on rend ce qu'on a encaissé.

Le trou était à côté. Le plafond « jamais plus que versé » excluait les
paiements par avoir. Scénario écrit avant le correctif : un client
obtient un avoir de 800 F, rachète avec, rend l'article → `numero_avoir:
null`, « 800 F non attribuables » dans le journal. Il avait tout perdu.

Règle pure `repartir_retour` (espèces, avoir, non attribuable) : l'argent
revient en argent, l'avoir en avoir, quel que soit le mode demandé ; le
reliquat d'échange demandé en espèces est borné pareil. L'écran dit au
vendeur combien ouvrir le tiroir et combien part en avoir. Deux
scénarios (`rendre_ce_qu_on_a_paye_avec_un_avoir_rend_l_avoir`,
`un_remboursement_rend_les_especes_en_especes_et_l_avoir_en_avoir`),
verts sur les deux moteurs. 461 tests.

Noté sans le corriger : l'échange valorise le remplacement à
`prix_reference` (tarif plein, sans remise), et l'avoir de retour porte
une TVA à zéro — le montant est juste, la ventilation du document non.

## 20/09/2026 (suite 2) — « on n'a pas de système de log ? »

Non. Le journal métier (`journal`, trente types d'événements) disait
qui a fait quoi ; le reste — un montant non attribuable, une base
indisponible, une commande refusée — partait sur stderr, que personne
ne lit quand le serveur est un service, et `serveur.log` n'était que
cette sortie brute, sans heure ni niveau.

**Les anomalies vont dans le journal.** `journal::anomalie[_sur]` : un
événement `anomalie` avec un message lisible et les chiffres, écrit
dans la transaction du geste — abandonnée, l'anomalie l'est aussi. Le
cahier du jour (`lire_journal_du_jour*`) les rend en `anomalies`, et
l'écran Journal les montre en rouge, en tête, seulement s'il y en a.
Les quatre `eprintln!` du noyau sont remplacés.

**Le journal technique du serveur.** Soixante lignes sans crate :
`2026-09-20T13:21:42 [REFUS ] 127.0.0.1 POST /rpc · commande_x ·
269edae1@7be67b69 · 404 · Le serveur ne connaît pas…`. Le contexte est
posé par le fil qui sert la requête (thread-local), enrichi de la
commande et de l'appelant dès qu'on les connaît. Une ligne par refus,
par erreur, par commande lente (plus de deux secondes), par démarrage,
arrêt et sauvegarde ; une commande qui réussit n'écrit rien. Rotation à
5 Mo, trois copies. Le fichier suit la base (`gescom.db` → `gescom.log`)
sauf en service (`ProgramData\Gescom\serveur.log`, où stderr va déjà —
on n'y recopie pas). Le test de routes lit le fichier : le démarrage,
les refus, le contexte, et **pas** le mot de passe.

La fenêtre, elle, ne remonte toujours rien : ce sera avec la v3.
463 tests.
## 20/09/2026 (suite 3) — l'en-tête choisit ses coordonnées

Question du propriétaire, en regardant l'atelier de modèles : l'adresse
d'exemple (« Hamdallaye… ») qu'on voit dans l'aperçu ne se cliquait
pas, et il n'y avait aucun moyen de dire « garder le téléphone, enlever
le RCCM » sur un modèle — la case « Afficher les coordonnées » était
tout ou rien.

Six cases remplacent la case unique (le nom reste toujours affiché,
c'est l'ancre du bloc) : adresse, téléphone, **téléphone 2** (existait
dans Paramètres → Société sans jamais apparaître sur un document),
e-mail, NIF, RCCM. `champsSociete?: string[]` sur le bloc en-tête ;
absent, un modèle enregistré avant ce réglage retombe sur les cinq
champs d'origine — aucune migration, vérifié par un script headless
(rendu avec et sans `champsSociete`).

## 21/09/2026 — la v2 est close

« v2 est fini ? », puis « fin v2 ». Le code l'était déjà : ce qui
restait à fermer, c'était quatre vérifications qui demandent d'être
devant la machine — l'installeur en élevé, une vraie impression
papier, deux postes sur le réseau, une restauration jouée pour de vrai
— et une décision en suspens sur le bon de livraison partiel.

**Décidé : on ferme sans dérouler les quatre vérifications.** Elles
restent dans TESTS-MANUELS.md, dette connue et non bloquante — à faire
le jour où l'occasion se présente (une vraie installation, une vraie
imprimante), rien dans le code n'y fait obstacle.

**D16 — le bon de livraison partiel reste un document à quantité
pleine.** Le suivi (stock, créance, statut) connaît le partiel et c'est
lui qui gouverne la gestion ; le document imprimé copie toutes les
lignes de la commande, même quand la livraison réelle est partielle.
Assumé, pas dans le chemin critique — se corrige à la main sur la
copie papier, comme avant Gescom.

**La v3 commence à la prochaine séance.**

## 21/09/2026 (suite) — la v3 posée avant d'être codée

Le propriétaire : « pour v3 quelles fonctionnalités ; simplifier les
pièces commerciales avec en-tête et pied téléversables et seulement
les signatures personnalisables ; un système de log ; un RBAC plus
complet ; on pose les bases avant de coder. »

[PLAN-V3.md](PLAN-V3.md), dans l'esprit du plan multi-société : des
décisions avec leur exemple et leur coût, pas des tâches. Quatre
chantiers — **A** pièces simplifiées, **B** journal, **C** droits,
**D** dossiers (déjà tranché en septembre).

Le point qui coûte : **l'atelier de modèles part** (D17). 4 700 lignes
écrites ce mois-ci, un retour du terrain par jour depuis le 16/09. Le
générateur historique (`genererPDF.ts`), qui imprime la même facture
depuis des mois et sait déjà poser logo, en-tête et pied, devient le
seul ; il gagne des cases à cocher par genre et trois signatures avec
image (D18). Rien à migrer : aucun modèle en production.

Et un constat qui a pesé sur le chantier B : le journal métier
(trente types d'événements) est écrit partout et **lu nulle part** —
aucun écran ne répond à « qui a annulé ce règlement ». L'Historique
sera la première permission de lecture ; il y en aura cinq (D19), et
l'employé ne verra plus les prix d'achat.

Quatre questions posées, quatre réponses dans le sens des
recommandations (D17–D20). Il ne reste rien à décider pour commencer
A-1.

## 21/09/2026 (suite 2) — trois précisions sur les dossiers, et le plan comptable

Le propriétaire, après le plan : les dates de travail d'un dossier se
donnent à sa création et se prolongent (D21) ; SQLite reste, « pour un
utilisateur qui n'a pas besoin de beaucoup de machines » (D22) ; le
plan comptable entre dans la v3, comme fondation (D23).

D22 est celle qui change le code : le multi-dossier ne demande plus
PostgreSQL mais **le serveur**, qui servira tout par `Base` sur les
deux moteurs — les 202 commandes y sont, et les scénarios tournent sur
SQLite par défaut. Le chemin `conn` du serveur part ; la fenêtre
monoposte garde ses façades et un dossier. Le déclencheur de stock
SQLite multi-dossier (item 9) passe en tête du chantier D.

D23 tient en une règle : le plan SYSCOHADA en base, les opérations
affectées, les journaux **lus** et exportés — **rien de stocké**. Une
écriture à côté de la vente serait une seconde vérité.

## 21/09/2026 (suite 3) — deux rôles, un seul produit (D24, D25)

« Ce que je prévois : le logiciel qui convient à mon client, v1 ou v2
— mais v3 serait un complet pour jouer les deux rôles. » Le réflexe a
été de vérifier avant de proposer : le mécanisme existe déjà.
[reseau.rs](../../src-tauri/src/reseau.rs) porte depuis la v2 un
réglage `monoposte` / `poste` dans `poste.json`, lu une fois, jamais
deviné — c'est exactement le garde-fou qu'on allait redemander contre
le bug du cache vidé de septembre. Les deux rôles jouent déjà dans le
même exécutable.

Ce qui manquait n'était pas une architecture : trois finitions. **D24**
— le choix se fait à l'installation (« une seule caisse » ou
« plusieurs »), pas dans un réglage qu'on découvre après coup. **D25**
— le serveur gagne une fenêtre : une coque Tauri fine autour de la
console web déjà construite, pas une interface refaite. Le rôle
« complet » embarque le multi-dossier (D13/D22, déjà au chantier D).

Recommandé et confirmé : réutiliser plutôt que reconstruire.

## 23/09/2026 — revue de la v2 : quatre défauts, une branche

« Review le v2 s'il n'a pas un bug majeur. » La suite passait (442
tests SQLite), la v2 était close. La revue a lu le serveur (`api.rs`,
`http.rs`, `socle.rs`) puis les chemins d'argent (`argent.rs`,
`creances.rs`, `caisses.rs`), et **chaque défaut a été reproduit par un
test avant d'être corrigé** — sur la branche `correctif/revue-v2`,
pas sur `main`.

**Le majeur : l'auteur.** Le serveur connaissait l'utilisateur de la
session et ne passait que son rôle ; le noyau prenait le premier compte
actif de ce rôle. Test : Awa et Bakary, tous deux caissiers ; Bakary
vend, la base dit Awa. Ventes, remises, règlements, ouverture de
caisse : tout signé par le premier venu, dans ~100 appels. Correction
sans toucher une signature — le serveur sert chaque requête sur son
fil, il y pose l'utilisateur (`noyau::auteur`, garde qui se retire en
tombant) et les cinq aides d'auteur le lisent d'abord (D26). Le test de
route échoue si on retire la garde d'`api.rs` : vérifié.

**Trois trous gardés par l'écran seul** (D27). `regler_creance` en mode
« avoir » : 1 600 F de dette effacés, zéro avoir consommé, zéro franc
en caisse. `enregistrer_paiement` à −50 000 F : la caisse à −49 200 F.
Une vente de −5 sacs : le stock de 200 à 205, la vente « payée ». Au
passage, la même chose côté réception, pièces et règlement fournisseur.
Les règles sont dans `coeur/saisie.rs` ; `enregistrer_paiement` passe
désormais par `regler_creance_datee`.

**Deux défauts de transaction.** `regler_creance_datee` (SQLite, le
chemin du serveur sur fichier) écrivait en trois ordres séparés et
ignorait l'échec de l'écriture en caisse — maintenant tout ou rien,
prouvé par un déclencheur qui fait échouer l'insertion en caisse. Et
sur PostgreSQL, vérifié en direct (`BEGIN; SELECT 1/0; COMMIT;` répond
`ROLLBACK`) : une écriture ignorée (`let _ =`) avorte la transaction et
`COMMIT` ne dit rien — la vente disparaissait, la commande rendait
`Ok`. `Transaction::valider` refuse désormais une transaction avortée.

Vu et laissé : la caisse nominative, dormante et inutilisable en
l'état (tous les `exiger*` passent `None`), à reprendre avec le
chantier C de la v3 ; l'auteur en argument plutôt que par le fil.

Tests : `noyau/tests/revue_v2_base.rs` (11 scénarios, SQLite et
PostgreSQL), un test de route HTTP, 12 tests unitaires. Suite :
466 tests SQLite, 461 sur PostgreSQL, 0 échec hors `installation.rs`
(propre à Windows, la mesure a été faite sous Linux).

## 23/09/2026 (suite) — v3, chantier A : les documents simplifiés

« Commence la v3, fais des tests sur l'interface et fais toutes les
étapes. » Branche `v3`, partie de `correctif/revue-v2`. Un banc d'écran
d'abord (`outils/banc/`) : le vrai serveur sur une base jetable, l'écran
servi par Vite, Playwright qui clique — les scénarios Rust ne voient pas
un bouton qui n'enregistre rien.

**A-1** — les réglages en base, par genre (sept), dans `config_app` ;
l'écran Paramètres → Documents. En-tête et pied reviennent de l'atelier
avec le choix des coordonnées. **A-2** — un seul habillage
(`lib/impression.ts`) pour tous les générateurs ; l'aperçu s'ouvre au
format du genre ; l'onglet Documents redessine un exemple à chaque case.
Deux choses apprises en route : écrire tous les genres à chaque
enregistrement figeait les défauts d'usine (un genre jamais réglé ne
s'écrit plus) ; un TTC par taux contredisait d'un franc le total
arrondi (retiré du récapitulatif). **A-3** — l'atelier part, tables
comprises. Le banc a trouvé que le reçu et les relevés perdaient leur
titre sous un en-tête image — corrigé.

Décision prise sans le propriétaire, notée : les signatures d'usine du
reçu et du relevé sont celles du générateur historique (« Le caissier »,
« Le client / Pour l'entreprise »), pas « rien » comme l'exemple du plan
(§ A3) — la règle « libellés d'usine = générateur historique » du même
paragraphe l'emporte, et évite qu'un reçu perde sa signature à la mise
à jour.

Mesuré : workspace 490 tests (la fenêtre Tauri compile et teste sous
Linux depuis cette séance) ; `documents_base` 13 scénarios sur SQLite
et PostgreSQL ; banc : 42 vérifications d'une base neuve.

## 23/09/2026 (suite 2) — v3, chantier B : les traces

**B-1 — l'Historique.** Le `journal` s'écrit depuis la v1 ; il se lit
maintenant, sur les deux moteurs, par une page qui dit quand, qui,
quoi, sur quoi, avant → après. « Sur quoi » se résout à la lecture
(sous-requêtes sur clé primaire) plutôt que de recopier le nom à
l'écriture : une fiche renommée se lit sous son nom d'aujourd'hui. Un
transfert, journalisé par son bon, ne se rattache à aucun article —
une sous-requête à plusieurs lignes fait refuser PostgreSQL.

`journal:lire` est la première permission de **lecture** : le serveur
refuse, l'écran cache. Elle est dans le rôle comptable d'une base
neuve ; les bases installées la recevront avec la migration des rôles
de C-1, qui ajoute les quatre autres. La liste « Personne » ne lit pas
les comptes (qui demande `utilisateurs:gerer`) mais les auteurs réels
du journal.

Le banc a trouvé une course dans l'écran : deux filtres changés coup
sur coup, la réponse lente du premier écrasait la bonne. Seule la
dernière requête affiche désormais.

Plan : `lire_journal_sur` s'appelle `lire_historique_sur` (le module
`journal.rs` existe déjà et écrit les anomalies).

**B-2** — les erreurs de la fenêtre (`onerror`, `unhandledrejection`)
arrivent `[POSTE ]` dans le journal technique, bornées (4 Ko, 10 par
minute et par poste). Le message est ramené à une ligne : un `\n` venu
d'une caisse aurait fabriqué une fausse ligne `[ERREUR]`. **B-3** — la
console du serveur lit ce journal (même permission que la sauvegarde),
lignes écrites en texte : le banc vérifie qu'une ligne piégée
`<img onerror>` n'injecte rien. **B-4** — les anomalies se marquent
vues dans une table à côté du journal (qui reste append-only) ; le
tableau de bord les compte en rouge.

Choix faits sans le propriétaire : marquer une anomalie vue demande
`journal:lire` (c'est un accusé de lecture, pas une écriture métier) ;
quand deux personnes la marquent, la première reste.

Mesuré : `historique_base` 6 scénarios (deux moteurs), 4 routes
nouvelles ou étendues, banc complet 91 vérifications d'une base neuve.

## 23/09/2026 (suite 3) — v3, chantier C : des droits plus complets

**C-4** — il n'existait aucune commande pour désactiver un compte.
`activer_utilisateur` le fait et ferme ses sessions dans la même
transaction ; refus : soi-même, le compte de secours, le dernier compte
à accès total. Paramètres → Utilisateurs montre les sessions ouvertes
avec la dernière commande.

**C-1** — cinq permissions de lecture, en une table (`coeur/lecture.rs`)
appliquée par `api::rpc` : refus, masque à `null`, paramètres
neutralisés. Deux lignes de partage choisies : le reste d'**une**
pièce reste lisible (c'est le document en main), le solde d'un **tiers**
non ; sur les documents d'achat, `achats:creer` suffit à lire les prix.
Le banc a trouvé que le serveur sur fichier SQLite ne passait pas par
`amorcer` : la migration du comptable est appelée aussi là. Au
passage, la sauvegarde automatique n'est plus demandée par qui ne peut
pas sauvegarder (un 403 à chaque connexion).

**C-3** — trois plafonds. Jugés dans la poignée quand l'argument suffit,
au point où l'argent sort quand le montant se calcule au fond du noyau.

Relevé, pas corrigé (ALERTES) : le mode « caisse par utilisateur »
ferait échouer tout encaissement — dormant, aucun écran ne l'offre.

C-2 (droits par dossier) se fait avec le chantier D, comme le plan le
dit.

## 23/09/2026 (suite 4) — v3, chantier D : plusieurs dossiers, et C-2

**D-1 à D-3** — le stock d'un second dossier se range chez lui ; le
serveur sert tout par `Base` (le chemin `Connection` part, 3 000 → 1 800
lignes de `socle.rs`) ; un dossier naît avec ses dates de travail.

**D-4** — le garde-fou des dates existait, personne ne l'appelait :
`api::rpc` juge maintenant chaque écriture datée (`date_d_ecriture`).
Pas de trou entre exercices ; une prolongation ne mord pas sur la suite.

**D-5** — la base d'avant prend le nom de la société et un exercice qui
couvre sa plus ancienne écriture (sinon un règlement tardif d'une vente
de 2024 était refusé par D-4).

**C-2** — un rôle par dossier. Précision à la décision : qui n'a que
certains dossiers perd ce qui est commun à tous (comptes, postes,
sauvegarde, société) — sinon le frère se créait un compte patron qui
voit tout.

**D-6** — le compte PostgreSQL limité, RLS sur chaque table cloisonnée.
Rejouer les scénarios sous ce compte a trouvé la sauvegarde partielle
(`pg_dump` ne voyait qu'un dossier) : elle passe par l'adresse du
propriétaire (`GESCOM_PG_SAUVEGARDE`) ou refuse.

## 24/09/2026 — v3, chantier E : le plan comptable, comme fondation

**E-1** — le plan SYSCOHADA en base (134 comptes, classes 1 à 7), commun
à tous les dossiers ; les sous-comptes par dossier. Permission
`comptabilite:gerer`, donnée au comptable.

**E-2** — 28 opérations, chacune un défaut et les comptes qu'elle
accepte ; la table ne garde que ce qui diffère.

**E-3** — les journaux ventes, achats, règlements, caisse, **fabriqués à
la lecture** (rien n'est stocké) et exportés en CSV. Prouvé : chaque
écriture équilibrée, les ventes du journal égales au chiffre du cahier
du jour, chaque opération sur son compte affecté — sur SQLite,
PostgreSQL et sous le compte limité. Pas encore : les retours de
marchandise (leur effet passe par l'avoir utilisé), les OD, la TVA sur
achats (non saisie). La v3 du plan est faite.

## 24/09/2026 (suite) — Gescom Équipe : personnel, puis paie

Le patron veut une seconde application pour le personnel, la paie et le
suivi client, sur la même base. Décidé (PLAN-EQUIPE, D28–D34) : une
seconde fenêtre qui ne parle qu'au serveur Gescom, dans le même dépôt ;
et une paie **souple**, parce que la plupart de ces commerces n'ont pas
de contrat : une personne, une façon d'être payée, rien d'obligatoire
au-delà.

**F** — la fenêtre (F-1), les fiches (F-2 : au mois, à la journée, à la
commission, à la tâche, cumulables, ou rien de fixe ; les montants
masqués à qui ne paie pas), les jours travaillés (F-3). PostgreSQL a
trouvé un `?2 = 1` typé `int4` ; le banc un « -0 » (somme vide d'un
`f64`).

**G-1** — les avances d'abord, parce que le trou le plus courant n'est
pas le calcul du salaire mais l'avance sortie du tiroir au milieu du
mois et oubliée à la paie. Une avance est une sortie de caisse au nom
de la personne ; la fiche de paie retiendra ce qui est en cours.

**G-2** — la fiche de paie, **stockée** (contrairement aux journaux) :
c'est un papier qu'on remet, et ce qu'une personne a touché ne doit pas
bouger si l'on corrige son salaire le mois suivant. Chaque ligne dit
d'où elle vient (« 22 jours × 2 500 F », « 3 % de 250 000 F de ventes
signées », « Avance du 05/09/2026 »). Choix : le prorata d'un salaire au
mois se mesure sur les jours **marqués** (un jour non marqué n'est pas
su, on ne le retire pas) ; une avance qui dépasse le net se reporte ;
la validation refuse une fiche qui n'est plus à jour (un jour marqué
après le calcul) plutôt que de figer un chiffre périmé ; une erreur se
corrige par une rectificative qui rend d'abord ce que l'ancienne avait
retenu. Au passage, les avances passent l'auteur à `exiger_sur` : sous
une caisse nominative, elles auraient été refusées.

**Correctif** — « Les chiffres de la boutique ne sont pas ouverts à votre
compte » chez le patron : sa session gardée par le navigateur datait
d'avant la v3 et ne connaissait pas `rapports:lire`. Les droits sont
maintenant relus auprès du serveur à chaque démarrage (`lire_mes_droits`).
Le banc complet oubliait les parcours F et G (`[a-e]`) : corrigé.

**G-3** — payer et imprimer. Chaque versement est une sortie de caisse,
comme l'avance ; on peut payer en plusieurs fois, jamais plus que le
reste. Une rectificative reprend les versements déjà faits pour la même
période. Le bulletin est un 8ᵉ genre de document, réglé avec les autres
dans Gescom ; Équipe l'imprime depuis son aperçu, faute de la commande
d'impression locale de la caisse.

**G-4** — les cotisations restent **vides** tant que le comptable ne les
a pas saisies (D33 : un taux faux sur un bulletin est pire qu'aucun), et
ne touchent que les personnes déclarées. La part patronale est dite sur
la fiche et le bulletin mais n'entre pas dans le net. Le journal de paie
PA se lit comme les autres ; la preuve qui compte : ce qu'il doit au
personnel (422) égale ce qui reste à verser sur les fiches. Il n'est
servi qu'à qui prépare la paie — il nomme ce que chacun gagne. Le
chantier G est fait.

**Révision** — le propriétaire : « la paie doit être indépendante de la
caisse mais visible dans les chiffres ». Avances et versements ne
touchent plus la caisse du jour (ni session, ni mouvement) : le moyen
dit d'où vient l'argent. Le coût des salaires (brut + charges
patronales des fiches validées) apparaît dans Rapports → CA mensuel, à
côté du CA, avec ce qui reste après les salaires — pour qui voit la
paie seulement.

**Correctif** — « la colonne av.piece_id n'existe pas » dans l'Historique,
chez le propriétaire, sur sa base PostgreSQL d'avant certaines colonnes.
Dix-neuf colonnes n'étaient rejouées que par la fenêtre SQLite ; le
serveur les rejoue maintenant à chaque démarrage, et un test garde les
deux listes d'accord.

## 25/09/2026 — Gescom Équipe : H-1 et H-2, le chantier CRM est fait

**H-1** — les échanges, et la fiche 360. Le point délicat était D34 :
« les relances de créance existantes sont des échanges comme les
autres, pas une liste de plus ». Deux façons de le tenir : dupliquer
les relances dans la nouvelle table `echange` (facile, mais deux
vérités sur le même fait), ou les fondre à la lecture. Choisi la
seconde — `crm::lister_echanges_sur` joint `relance_creance` à `vente`
à `client` et mêle le résultat aux vrais échanges, triés par date.
`relance_creance` reste ce qu'elle est ; rien n'est recopié. L'écran
Équipe → Suivi clients (annoncé « bientôt » depuis F-1) devient réel :
chercher un client (la recherche de Gescom, pas une seconde), sa fiche
360 avec ses chiffres et son fil.

**H-2** — rappels et prospects. D34 est explicite : « pas de second
fichier clients ». Un prospect est donc une ligne `client` comme les
autres, avec `statut = 'prospect'` et son origine (`origine_prospect` —
distincte d'`origine`, qui dit d'où vient la *ligne*, pas d'où vient le
*prospect* : les deux se seraient marché dessus). Il redevient
`client` tout seul à sa première vente : la bascule vit dans
`creer_vente_datee_sur[_base]` lui-même, sur les deux moteurs et les
deux chemins (fenêtre et serveur) — pas de bouton « convertir » à
chercher, pas d'étape qu'on oublie. Un rappel s'attribue à une
personne (un compte actif) et se retrouve dans « Mes rappels » ; se
marquer fait ne l'efface pas.

Au passage : `client.statut`/`origine_prospect` sont deux colonnes
neuves sur une table déjà en service — posées à trois endroits
(schema.sql, la fenêtre, le serveur), comme l'a appris le correctif de
la veille sur `avoir.piece_id`. `lire_fiche_client` rend maintenant
`statut`, sans rien changer pour qui l'appelait déjà.

Le chantier CRM (H) est fait ; Gescom Équipe (F, G, H) est fini.
Bancs `h1-echanges-360.mjs`, `h2-rappels-prospects.mjs` ;
`f1-fenetre-equipe.mjs` corrigé (Suivi clients n'est plus une annonce).

**Correctif** — « Les entrées de stock sans fournisseur : la somme n'est
mentionnée nulle part. » Le propriétaire a raison sur trois points à la
fois : sans fournisseur, l'achat n'avait ni pièce ni paiement ; « à
crédit », il n'avait rien du tout (on ne peut rien devoir à personne) ;
et le journal AC le passait au 401 sans que rien ne le solde. Sa règle :
sans fournisseur, comptant ; à crédit (acompte ou non), avec le
fournisseur. Comme la vente a son « client de passage », l'achat a
maintenant son **« Fournisseur divers »** : chaque achat sans
fournisseur y a sa facture payée et son paiement, et se lit sur sa
fiche. Les achats d'avant restent sans pièce, mais leur sortie de
caisse solde enfin le 401 dans le journal RG.

En l'écrivant, un test a buté sur un défaut plus ancien : dans un second
dossier, la première facture fournisseur prend `FAF-2026-00001`, déjà
pris par le premier dossier, et la base refuse (le numéro est unique
partout, le compteur par dossier). Même chose pour les autres pièces.
Pas corrigé ici : il faut décider à quoi ressemble le numéro imprimé
d'un second dossier (préfixe du code, comme les codes clients ?).

## 25/09/2026 (soir) — les pièces comme dans Word, le filigrane, la sauvegarde qui ne partait pas

**Les retours du propriétaire sur les pièces.** L'en-tête et le pied
« ne se répètent pas comme dans Word » : l'en-tête n'était qu'en haut de
la première page, le pied qu'au bas de la dernière — ils vivaient dans
le flux du document. Tout ce qui s'imprime en A4/A5 passe maintenant par
`impression::documentHTML` : le document est la cellule d'un tableau
dont le `<thead>` porte l'en-tête (le moteur d'impression répète un
thead en haut de chaque page) ; le pied est en `position:fixed` au bas
de la page (un élément fixe se répète à l'impression) et une copie
invisible dans le `<tfoot>` lui réserve sa hauteur, pour que le texte
ne passe jamais dessous. Collés au bord du papier, sans marge d'aucun
côté ; le texte garde la sienne. Pas de script : l'aperçu n'en exécute
pas. Prouvé en vrai : une facture de 70 lignes imprimée en PDF par
Chromium (le moteur de la fenêtre), trois pages lues pixel par pixel.

« Pour acquit » et « Le fournisseur » : soulignés, la place de signer
dessous, au lieu d'un libellé puis d'un trait 22 mm plus bas.

**Le filigrane.** Une image ou un mot au milieu de chaque page, estompé
et flouté « pour ne pas se mélanger aux tableaux ». Premier essai posé
DERRIÈRE le texte : les lignes alternées des tableaux le hachaient une
sur deux dès que la boîte d'impression n'imprime pas les fonds — son
réglage par défaut. Posé par-dessus en `mix-blend-mode: multiply`, il
teinte le papier sans toucher au noir du texte, fonds imprimés ou non.

**La revue de la sauvegarde** — la fonctionnalité primordiale à côté de
laquelle on était passés. Le fil du serveur dormait 24 heures avant sa
première copie ; le poste d'une boutique s'éteint le soir. Aucune copie
automatique n'est donc jamais partie chez un commerçant, et l'écran ne
pouvait pas le voir : il lisait la « dernière sauvegarde » dans le
journal, où cette copie n'écrit rien. Maintenant le dossier fait foi —
la date d'une copie est dans son nom — et le serveur regarde peu après
son démarrage puis chaque heure. Chaque copie est relue avant d'être
crue. Le `essai-restauration/…essai.dump` trouvé dans `target/` chez le
propriétaire est le reste de l'essai de restauration du 13/09 : rien à
garder.

**Le jaune du Journal** : le bandeau du titre était jaune ; il devient
un filet. Deux autres textes jaunes sur fond jaune (tableau de bord,
relances) sont passés en ambre foncé. Les emojis restants (« ⚠ » dans
la fenêtre du serveur et dans des commentaires) sont des mots.

## 22/10/2026 — Revue de `rh` : ce qui méritait correction

Une revue à fond des deux commits de la branche (H-1/H-2 et l'achat
sans fournisseur) : dix angles, seize vérifications, les suites SQLite
et PostgreSQL rejouées. Quinze constats retenus, et parmi eux quatre
vrais défauts, tous corrigés le jour même.

**Le prospect et la facture.** La bascule prospect → client vivait dans
les deux `creer_vente_datee_sur*`, mais une vente naît aussi d'une
facture validée (`valider_facture_sur*`) : un prospect facturé depuis
Pièces restait prospect, avec « Ventes : 1 » sur sa fiche. Une seule
aide, `prospect_devient_client[_sur]`, appelée aux quatre endroits, et
son erreur se propage — le `let _ = tx.executer(...)` d'origine était
exactement ce que la revue du 23/09 interdit dans une transaction.

**Le second dossier.** Le défaut « trouvé en route, pas corrigé » du
25/09 était devenu une régression : l'achat sans fournisseur, qui ne
réservait pas de numéro, était le seul achat qui marchait encore dans
un second dossier ; en lui donnant une FAF, on le faisait heurter
`FAF-2026-00001` du premier dossier, et le retour arrière annulait
aussi le compteur — chaque essai heurtait à nouveau. Tranché comme les
codes clients : un dossier autre que celui d'origine préfixe ses
numéros de son code (`B-FAF-2026-00001`), dans `reserver_numero_sur`,
un seul endroit. Le format imprimé d'un second dossier est donc « le
code du dossier, un tiret, le numéro » ; si le propriétaire en veut un
autre, c'est une ligne.

**Une seule porte pour le Fournisseur divers.** La règle « comptant
seulement » tenait dans Achats, mais Pièces lui créait une FAF à crédit,
le retour fournisseur lui faisait un avoir, le règlement de dette lui
payait du tiroir, et sa fiche se renommait — la dette envers personne
revenait par quatre portes. Deux règles pures dans `coeur::tiers`,
jugées dans les deux versions ; sa fiche est figée comme celle du
client générique.

**Le rappel orphelin.** Un rappel s'attribuait à n'importe quel compte
actif ; avec les droits par dossier (C-2), la comptable qui n'entre que
dans la boutique B ne verrait jamais celui posé dans A. Refusé par
`role_dans_sur`, l'aide qui garde déjà `renommer_dossier`.

Et le reste : `lire_fiche_client` de la fenêtre rend `statut` (règle
2), le générateur de code client est une seule fonction
(`comptoir::prochain_code_client_sur`), l'attribut clippy retrouve sa
fonction, les tables `echange`/`rappel` ont leurs index, l'écran Suivi
clients affiche ses erreurs au lieu d'un rond qui tourne ou d'un
« Aucun client. ». Hors branche, un test de `pieces_base` datait une
pièce au 3 septembre en dur : passé le 4 octobre, la règle des 31 jours
le refusait — relatif au jour maintenant.

Laissé de côté, volontairement : l'aplatissement du plomb `Option`
dans `enregistrer_achat_date*` (gros, sans défaut), le rachat des
anciens achats sans fournisseur à crédit (un 401 jamais soldé, à
décider), les copies de petites aides (`texte`, `jour`, `noter`,
formatteurs du front).

**Banc d'écran, le soir même.** Les trente parcours Playwright rejoués
sur base neuve (serveur de débogage + Vite, Chrome du poste) : 378
vérifications, tout passe — `h1`, `h2`, `i1` compris, après les
corrections. Deux fragilités du banc corrigées en route, pas du
produit : sur un build de débogage, bcrypt met deux secondes par
vérification, et `pw.mjs` (`connecter`) comme `c2` attendaient un délai
fixe de 2,5 s avant de continuer — le jeton n'était pas encore là, le
parcours plantait sur « Jeton absent », et le premier changement de mot
de passe coupé en route laissait la base avec un mot de passe que le
parcours suivant ne connaissait pas. L'aide attend maintenant que la
page quitte la connexion (jusqu'à vingt secondes), que la fenêtre de
changement de mot de passe se ferme, et que la boîte « Quel dossier
ouvrir ? » paraisse. Sous Git Bash, `lancer.sh` ne sait pas arrêter
l'instance précédente (`lsof` absent) : tuer les ports 7300 et 1420 à
la main avant `neuf`.

## 25/10/2026 — Le banc d'écran sur les deux installations de la v3

Demande du propriétaire : des tests Playwright, la v3 comme référence
pour le monoposte et le multiposte. Le banc (`outils/banc/`) ne
connaissait qu'un serveur SQLite et un navigateur ; il monte
maintenant, sous Windows (`lancer.ps1`), les deux installations telles
qu'elles se posent chez un commerçant, chacune sur une base jetable.

**Monoposte : la vraie fenêtre.** Playwright pilote `gescom.exe`
(débogage) par le port de débogage de WebView2. Pour qu'elle n'ouvre
pas la base du propriétaire (`%APPDATA%\ml.gescom.app`, sa base de
790 Ko et son `poste.json`), une variable `GESCOM_DONNEES` remplace
son dossier de données — **lue seulement en build de débogage**
(`lib.rs::dossier_donnees`, servie à la base, à `poste.json` et aux
images) : un exécutable livré l'ignore. `p1` vérifie que les fichiers
du propriétaire n'ont pas bougé. Le profil WebView2 est à part aussi.

Constat en route : **la fenêtre n'ouvre plus de base locale.** En mode
« monoposte » (le défaut de `poste.json`), `App.tsx` affiche « Brancher
ce poste » et `appeler` refuse sans serveur ; c'est ce que dit
ARCHITECTURE.md (« une seule machine ne veut pas dire sans serveur »),
pas PLAN-V3 § 6 ni le tableau de D22, qui décrivent encore la fenêtre
sur sa base SQLite par les façades `Connection`. Le monoposte du banc
est donc le serveur SQLite + la fenêtre sur `127.0.0.1`. La fenêtre
crée encore un `gescom.db` local à son démarrage (`lib.rs`), que plus
rien ne lit depuis l'écran. Non corrigé : à trancher.

**Multiposte : PostgreSQL `gescom_test`, rien d'autre.** Le lanceur
refuse toute autre base, y fait `DROP SCHEMA` avec `-Neuf`, puis pose
la démo par `examples/amorcer --demo` — sur PostgreSQL le serveur ne
l'écrit pas lui-même (`GESCOM_DEMO` ne joue que sur SQLite) : sans
elle, la moitié du banc tombait faute d'articles. Le journal du
serveur va dans `.travail` (`--journal`) : par défaut, sous Windows,
il s'écrivait dans `%APPDATA%\ml.gescom.app\serveur.log`.

**Nouveaux parcours.** `p1` premier démarrage (branchement, `poste.json`
réécrit, un seul dossier, cache vidé sans retour au branchement), `p2`
tous les écrans dans WebView2, `p3` l'argent (refus caisse fermée,
caisse ouverte **par l'écran**, stock et caisse au franc près, crédit
puis règlement, fermeture à écart 0), `p4` la copie automatique du
25/09 qui part seule (20 s par `GESCOM_SAUVEGARDE_DELAI_S`). `m2` deux
caisses en même temps (12 devis sans doublon, 10 ventes : stock −10 lu
pareil des deux côtés, caisse commune juste), `m3` ce qu'un poste fait
l'autre le voit (la comptable encaisse une créance faite sur A ; A
ferme la caisse, elle est refusée), `m4` deux dossiers ouverts en même
temps sur deux postes. `suite.mjs` joue tout sans s'arrêter et fait le
bilan. `b4` et `c1` écrivaient dans `essai.db` : `ecrireEnBase` vise la
base du mode (SQLite, ou `gescom_test` par `psql` en UTF-8 sur l'entrée
standard — sur la ligne de commande, Windows cassait « n° »). `i3`
accepte le `.dump` de PostgreSQL.

**Résultats, bases neuves, Edge du poste.** Monoposte 4/4 parcours,
71 vérifications. Multiposte : le banc d'origine 31/31 (385
vérifications ; `d5` rejoué seul après un `ERR_NO_BUFFER_SPACE` de
Windows), M 3/3 (35). SQLite, pour comparer : 31/31 (385) et M 3/3.
`i2` sauté partout (`pdftoppm` absent). Workspace Rust 660/660.

Pièges du banc, pas du produit : la fenêtre charge `localhost`, que
Windows résout en `::1` — un Vite sur `127.0.0.1` lui refusait la
connexion (page blanche) ; le surveillant de Vite tombait sur les
fichiers verrouillés du profil WebView2 (`vite.banc.config.ts` ignore
`.travail`) ; les boîtes natives de `plugin-dialog` bloqueraient un
geste (`boitesMuettes` les intercepte dans la page) ; le serveur et
Vite lancés par `Start-Process` héritent de la sortie de l'appelant —
rediriger `lancer.ps1` vers un fichier. L'employé n'a ni la caisse du
patron, ni les soldes, ni `creances:gerer` (C-1) : `m3` prend une
comptable.

## 25/10/2026 (suite) — la v3 en monoposte comme en multiposte

Le banc a montré que D22 et D24 décrivaient une fenêtre monoposte qui
n'existait plus depuis le 11/09 (elle ne parle qu'au serveur), et que
73 commandes de la v3 n'existent que côté serveur. Le propriétaire :
« je veux seulement que v3 travaille en mono et multiposte ». Retenu,
choix écrits avant le code et acceptés : un seul chemin (le serveur),
mais le poste qui garde la base n'a plus rien à saisir.

**Ce qui change.** `poste.json` porte le rôle : `monoposte` (« ce poste
garde la base »), `poste` (caisse d'un autre poste), `a_choisir` (neuf
ou illisible — on demande, on ne devine plus « monoposte », ce qui
faisait d'une caisse fraîche un poste principal à base vide). En
`monoposte`, la fenêtre se branche sur `127.0.0.1:7300` et, s'il ne
répond pas, lance `gescom-serveur.exe` à côté d'elle, comme le
raccourci, sans fenêtre noire, et ne l'arrête pas en se fermant. Le
serveur du raccourci, lui, s'efface sur la caisse d'une autre machine
(`role_poste.rs`) ; il tourne dans le doute — et sur « caisse de
127.0.0.1 », le réglage du poste principal du propriétaire entre le
11/09 et aujourd'hui. Changer de rôle : Paramètres → Réseau, avec une
confirmation qui dit que les ventes du poste principal ne suivent pas.

**Trouvé par le banc en route.** React lance les effets deux fois en
développement : deux `demarrer_serveur_local` simultanés lançaient deux
serveurs, le second trouvait le port pris et son échec s'affichait
alors que le premier répondait — un verrou (une demande à la fois) et
un dernier `/sante` avant de déclarer l'échec. Les effets d'`App.tsx`
(droits relus, canal, sauvegarde automatique) appelaient le serveur
avant qu'il soit joint : ils attendent `serveurJoint`. La page de
connexion disait « Poste caisse » sur le poste principal.
`definir_config_reseau` prenait un `String` et recevait `null` quand le
nom du poste était laissé vide — `Option` maintenant.

**Tests.** `role_poste` : 6 tests purs (autre machine, 127.0.0.1,
localhost, `::1`, adresse de ce poste, nom résolu ou introuvable,
fichier absent ou illisible). Banc monoposte refait sur la fenêtre
seule (`lancer.ps1 monoposte` ne démarre plus de serveur) : `p1` le
rôle demandé puis le serveur lancé par la fenêtre ; `p5` le serveur
tué puis relancé à la réouverture (retour à la connexion : un serveur
qui redémarre ferme les sessions), le port 7300 pris par un autre
programme (l'écran le dit, « Réessayer » repart) ; `p6` le rôle caisse,
le serveur « du raccourci » qui s'arrête sur une caisse (journal, pas de
base créée) et tourne sur 127.0.0.1, le retour au rôle « garde la base »
par Réseau. Monoposte 6/6 (100), multiposte 31/31 + 3/3, SQLite 31/31 +
3/3, Rust 666/666, `tsc` propre, release vérifiée (`cargo check
--release`). Sur PostgreSQL, a1, a2 et a2b ont échoué une fois en tête
de passage (délais, « Jeton absent ») dans une période où Windows
rendait `ERR_NO_BUFFER_SPACE` ; rejoués seuls sur base neuve, verts.

**Pas fait.** La fenêtre ouvre encore un `gescom.db` local au démarrage
(le même fichier que la base par défaut du serveur) ; le retrait des
façades `Connection` reste le grand nettoyage d'ALERTES.md. Le poste
principal du propriétaire est réglé « caisse de 127.0.0.1 » : il marche
comme avant ; Paramètres → Réseau → « Ce poste garde la base » lui
donnerait le lancement automatique du serveur.

## 26/10/2026 — D35 : chaque vente sans nom a sa fiche « passage »

La question du propriétaire (« comment les comptants sont traités ? »)
a montré un trou : le serveur refusait au « Comptant » le crédit et
l'avoir, mais acceptait une vente « comptant » payée en partie ou pas
du tout, et une facture à crédit validée depuis Pièces. La marchandise
sortait, rien n'entrait, et la dette, au nom de « Comptant »,
n'apparaissait dans aucune relance ni aucun rapport. L'écran envoyait
toujours le total — mais le serveur juge la saisie (D27).

Sa proposition — un code client de passage incrémenté, crédit bloqué,
un passage vers client ordinaire — est devenue D35, avec sept
garde-fous écrits avant le code (DECISIONS.md). Ce qui a été fait :

- `noyau/src/passage.rs` : la fiche « Client de passage » créée DANS la
  transaction de la vente (compteur `PAS`, code `PAS-000123` préfixé du
  dossier), `est_generique = 1` pour qu'elle sorte d'office de toutes
  les listes et de tous les chiffres qui écartaient déjà le
  « Comptant », `statut = 'passage'` pour la distinguer de lui ;
  `lire_passages_sur` (code ou numéro de facture) ; `promouvoir_sur` ;
  `figer_noms_sur`.
- La règle `coeur::saisie::verifier_vente_passage` (payé en entier, pas
  de crédit), jugée avant d'écrire dans les deux versions de
  `creer_vente` et de `valider_facture`.
- `creer_facture_depuis_vente` prend désormais le client de la VENTE :
  l'écran passe le « Comptant », la facture du ticket va à la fiche.
- `piece_commerciale.tiers_nom` (schema, amorçage, v2) : le nom écrit à
  l'émission, figé au premier renommage (promotion ou
  `modifier_client`) ; la pièce imprimée le lit en premier.
- `lire_client_generique` ne rend que le « Comptant », `lire_clients`
  pas les fiches de passage.
- Écran : Clients → « Passages », « Passer en client ordinaire ».

Trois anciens tests vendaient au « Comptant » sans encaisser : adaptés
(un client nommé pour la vente à découvert ; caisse ouverte et total
encaissé dans deux tests de routes).

**Le banc, à corriger lui-même.** L'aide qui faisait taire les boîtes
natives de Tauri (`boitesMuettes`) n'avait jamais rien intercepté :
`window.__TAURI_INTERNALS__.invoke` est non inscriptible, le remplacer
ne faisait rien, sans erreur. `p6` échouait une fois sur deux sur la
confirmation « Ce poste garde la base » ; c'est le propriétaire, qui
regardait la fenêtre, qui a vu la vraie boîte Windows s'ouvrir.
L'interception passe maintenant par les requêtes `http://ipc.localhost/
plugin:dialog|…`, que Playwright détourne ; `p6` vérifie que la
confirmation est demandée. Aussi : Vite du banc sans HMR (son
SharedWorker de reconnexion faisait planter la connexion de Playwright
à WebView2), le port 7301 libéré avant et après l'essai du serveur
« du raccourci », `arreterPort` tolérant quand rien n'écoute.

**Résultats** — voir la ligne D35 d'ETAPES. Pendant le premier passage
multiposte, la machine s'est mise en veille au milieu de `j1` (aucune
ligne au journal du serveur pendant 32 minutes) : rejoué seul ensuite.

**Pas fait** : rattacher à la promotion les autres tickets d'un même
passant (doublons) ; le nom figé côté fournisseur (le même trou existe
quand on renomme un fournisseur) ; `postgres_amorcage` dépend de la
langue des messages de PostgreSQL.

## 26/10/2026 (suite) — D37 le menu réglable, D36 les articles sans stock

Deux demandes du propriétaire pour le petit commerçant, décidées avant
le code (DECISIONS.md), faites dans cet ordre.

**D37 — le menu se règle.** Paramètres → Menu : une case par entrée,
« Boutique simple » (Tableau de bord, Ventes, Achats, Stock, Clients,
Caisse, Rapports) et « Complet ». Quatre entrées ne se cachent jamais
(`coeur::menu`, le serveur le refuse) : sans Caisse on n'encaisse plus,
sans Paramètres on ne réaffiche plus rien. Cacher n'est qu'un affichage
— l'Historique enregistre toujours, payer par chèque reste possible.
Le réglage vit par dossier dans `config_app` ; `Layout` filtre le menu
et la palette Ctrl K (`lib/menu.ts`, événement `gescom:menu`).

**D36 — un article peut ne pas suivre son stock.** La colonne
`article.gere_en_stock` existait depuis le début, lue par les alertes et
la valeur du stock, sans écran pour la régler. Case « Suivre le stock »
à la création, « Ne plus suivre / Suivre le stock » sur la fiche
article. Non suivi : jamais à découvert (le drapeau de la ligne de vente
reste à 0, la caisse n'annonce ni rupture ni découvert), hors de l'écran
Stock (« Non suivis » le nomme), transfert et inventaire refusés. Les
mouvements s'écrivent quand même : le stock reste la somme des
mouvements et aucun des trente endroits qui en écrivent n'a bougé.
Cesser dit combien d'unités sortent du stock (« 200 kg sortent… ») ;
reprendre demande un comptage pour chaque magasin et pose les
ajustements dans la même transaction. L'article étant commun aux
dossiers, la réponse nomme les autres dossiers où il a du stock.

**Trouvé en route** : la liste des clients écrasait parfois le bon
résultat par celui d'une recherche précédente arrivée en retard
(`j1` échouait une fois sur trois) — seule la dernière demande écrit
maintenant la liste. Dans un navigateur (le banc multiposte) la boîte
native de confirmation n'existe pas : « Ne plus suivre le stock » se
teste dans la vraie fenêtre (`p7`).

**Résultats** — voir les lignes D37 et D36 d'ETAPES.

**Pas fait** : la case à l'import CSV (proposée, pas codée) ; un article
non suivi reste proposé dans les écrans d'achat comme les autres.

## 26/10/2026 (fin) — essai à la main, poste.json avec BOM

**Le propriétaire teste à la main.** Base d'essai isolée dans
`outils/banc/.travail/monoposte` (jamais `%APPDATA%`), remplie PAR LE
SERVEUR avec `outils/banc/donnees-essai.mjs` (connexion `admin` au mot
de passe d'usine, sur base neuve) : 100 articles de `catalogue_test.csv`,
12 clients, 5 fournisseurs, 39 ventes au comptoir sur trois semaines,
8 crédits dont 4 à moitié réglés, 4 achats, 4 dépenses, un article non
suivi. Pour refaire : `lancer.ps1 arreter`, base effacée, serveur lancé
sur `.travail/monoposte/serveur.db` (mêmes options que la fenêtre),
`node donnees-essai.mjs`, puis la fenêtre avec `GESCOM_DONNEES` et un
`poste.json` « monoposte » (scripts de travail `.travail/essai-manuel.ps1`
et `essai-fenetre.ps1`, hors git). Le script échoue si le mot de passe
d'usine a déjà été changé — c'est voulu.

**Trouvé en route** : un `poste.json` enregistré avec un BOM (Bloc-notes,
ou `Set-Content -Encoding utf8` de PowerShell 5.1) paraissait illisible :
le poste perdait son rôle et redemandait « Comment travaille ce poste ? ».
Corrigé aux trois lectures (fenêtre `reseau.rs`, Équipe `main.rs`,
serveur `role_poste.rs`) : le BOM est retiré avant de lire. Compilé, pas
testé (demande du propriétaire). **Pas commité**, avec
`donnees-essai.mjs`.

## 26/10/2026 (soir) — retours de l'essai à la main : Relances, passage, taille du texte, retours dans Pièces

Quatre retours du propriétaire, d'après une capture de Relances. Choix
écrits avant le code, acceptés (« Oui, vas-y ») ; deux tranchés par lui.

**Relances : les tuiles du tableau de bord.** Les quatre cartes teintées
(orange, rouge, bleu) deviennent `KpiCard` dans `GRILLE`, comme
l'accueil ; « Sans relance » teinte son bord s'il y en a. Au passage, le
tri affichait « jours » (la valeur brute du `Select`) au lieu
d'« Ancienneté ». Le « payé 223 700 F » égal au reste n'était pas un
défaut : le jeu d'essai règle la moitié de quatre crédits.

**Le code de passage ne s'imprime plus** (choix du propriétaire : le
code seul, pas le bloc). `lire_donnees_piece` (deux versions) rend
`client_code: null` pour une fiche `statut = 'passage'`, le reçu un code
vide ; promu client ordinaire, il redevient son code et s'imprime.

**Le texte imprimé grandit** (choix du propriétaire : ~11 pt en A4).
Mesuré : le corps d'une pièce sortait à 11 px = 8,25 pt, les détails à
7 pt ; l'usage est 10–12 pt pour un document imprimé, jamais moins de 10
(Practical Typography, guides de facturation). Plutôt que reprendre la
soixantaine de tailles écrites à la main, `documentHTML` pose le contenu
et le texte du pied dans un `zoom` qui porte la police du document à
11 pt en A4 et 10 pt en A5 (`echelleTexte`) : toutes les proportions
restent, les bandeaux images et les marges ne bougent pas. Vu dans
Chrome à la largeur du papier : facture A4 et A5, reçu, relevé, rien ne
déborde.

**Pièces dit ce que les retours ont fait.** Un retour sur une facture à
crédit éteint la dette par un paiement « retour » : la facture passait
« Payé » sans un franc encaissé, et rien ne le disait. Troisième badge
sous le statut — « Retour partiel », « Retournée », « Remboursée » — et
« −X F rendus » sous le total, le détail au survol. « Remboursée » se
juge sur l'argent **sorti du tiroir**, pas sur le mode choisi : un
« remboursement » sur une facture à crédit ne rend rien (vu en écrivant
le test, la première version de la règle l'aurait dit remboursée).
Règle `coeur::pieces::etat_retour`, colonnes 21..23 des listes client.

**Pas fait** : le côté fournisseur (retours fournisseur dans Pièces) ;
le bon d'échange et le ticket gardent leur taille ; pas de réglage
« taille du texte » par genre (un défaut seulement, comme demandé).

**Revu le même soir, à la demande du propriétaire.** « Client de
passage » sur une facture, c'est « presque une insulte » ; le code, lui,
retrouve la facture. Donc l'inverse du premier choix : le papier porte
le code à la place du nom (`passage::tiers_a_imprimer`, facture, reçu,
bon d'échange), y compris sur une facture émise avant une promotion.
Pièces, pour une vue d'un coup d'œil (ses deux choix sur quatre
proposés) : une facture entièrement revenue affiche « Remboursée » ou
« Retournée » **à la place** de son statut, ligne grisée ; une pastille
« Retours » avec leur nombre. Vu sur le banc SQLite (jeu d'essai + trois
retours posés par le serveur). **Trouvé en le regardant** : la colonne
« Payé » compte la dette éteinte par un retour (paiement `mode =
'retour'`) comme un encaissement — 15 000 F « payés » sur une facture où
rien n'est entré. Proposé, accepté, corrigé : les listes client rendent
`encaisse` (`total_paye` moins les paiements `mode = 'retour'`, colonne
24) ; « Payé », son total de pied et le « Déjà payé » d'Encaisser le
lisent, la fenêtre d'encaissement ajoute « Éteint par un retour » pour
que le compte tombe juste. `total_paye` ne change pas : c'est lui qui
fait le reste dû. **Reste pareil ailleurs** : Relances (« payé »), la
fiche client et l'« Acompte reçu » imprimé sur la facture comptent
encore la part éteinte par un retour.

**Le cadre client, les noms longs, une page de trop.** Le cadre du tiers
grandit (au moins la moitié de la largeur en A4, nom plus gros, plus
d'air). En essayant un nom très long en A5, le PDF de Chrome sortait
**deux pages** pour un contenu qui en tenait une : le `zoom` CSS du
texte trompe le découpage en pages. Remplacé par un vrai agrandissement
des tailles (`agrandirTexte`) : facture A4/A5, noms long et sans espace,
reçu, relevé — une page chacun, rien ne déborde. **La page blanche que
le propriétaire voit après impression n'est pas expliquée par là** : sa
facture de 12:40 (avant le zoom) tient sur une page dans Chrome ; elle
vient du chemin d'impression de la fenêtre (WebView2, boîte de Windows).
Essai à la main relancé sur sa base d'essai pour l'observer.

**La fenêtre blanche.** Observé par le propriétaire : après « Imprimer »,
la fenêtre d'impression reste ouverte, blanche — surtout gênant avec le
bon de sortie. Le script du document appelait `window.close()`, que
WebView2 refuse sur une fenêtre ouverte par l'application. Désormais le
document change aussi son titre en `__gescom_impression_finie__` après
la boîte d'impression (imprimé ou annulé), et la fenêtre Tauri, qui
l'écoute (`on_document_title_changed`, `commandes/impression.rs`), se
ferme elle-même. Vaut pour tout ce qui passe par `imprimer_facture` :
pièces A4/A5, facture + bon de sortie, bon de sortie seul, ticket, bon
d'échange. **Vérifié par le propriétaire** dans la vraie fenêtre : elle
se ferme toute seule, avec la facture et avec le bon de sortie.

## 26/10/2026 (nuit) — retourner plusieurs articles en un geste

Demande du propriétaire : « si on doit cliquer à chaque article, et les
échanges aussi ». Plan écrit et accepté (tout d'un coup), après ses trois
raisons de vouloir « Remboursée » dans Pièces — **pas deux fois**, **le
vrai chiffre d'affaires**, **le crédit du client** (pas « surveiller la
caisse »).

**Une fenêtre de retour** (`ModalRetour`) : tous les articles de la
vente, une case et une quantité chacun (ce qui reste, l'article tout
revenu grisé), rembourser / avoir / échange, et en échange les articles
pris (plusieurs). Pièces n'a plus qu'une entrée « Retour ou échange… » ;
l'écran Retours, trois boutons par vente. Les trois anciennes fenêtres
(un article, un mode) sont parties.

**Le serveur** : `enregistrer_retour_groupe`, né sur `Base` (D22 — la
fenêtre ne parle plus qu'au serveur, comme D36/D37). L'ancien retour
d'un article est devenu un cas de la même fonction (`retourner_une_ligne`,
dans la transaction de l'appelant). Tout ou rien ; une seule pièce AVC
par geste ; l'échange réglé une fois, reliquat en avoir réparti au
prorata ; **le plafond de remboursement se juge sur le geste** (trouvé en
écrivant : chaque article sous le plafond aurait laissé passer un total
au-dessus) ; une ligne d'une autre vente est refusée (l'ancien retour ne
vérifiait pas que la ligne était de la vente).

**La liste dit où est partie la valeur** : « 2 200 F en avoir »,
« rendus en argent », « dette annulée », « échangés » (le « rendus » de
la capture était faux pour un avoir) ; un avoir de retour **rendu ensuite
en espèces** fait passer la facture « Remboursée » ; le pied de Pièces
déduit la marchandise revenue et le dit. **Pas touché, signalé** : le
tableau de bord et les rapports ne déduisent aucun retour de leur
chiffre d'affaires.

## 26/10/2026 (nuit, suite) — le duplicata, côté serveur

Demande du propriétaire : des duplicatas optionnels en filigrane sur ses
pièces. Choix du propriétaire et plan dans
[PLAN-DUPLICATA.md](PLAN-DUPLICATA.md) ; plan en cinq lignes accepté,
« commence par la table impression et noter_impression ». Fait : le
premier lot, **noyau et serveur**. Rien à l'écran encore.

**Le serveur compte les impressions** (comme D27 : l'écran ne protège
rien). Table `impression` cloisonnée (`TABLES_CLOISONNEES` : colonne,
index, politique RLS du compte limité), dans `schema.sql` — les deux
chemins d'amorçage la lisent. La première impression d'une pièce ou d'un
reçu est l'original, les suivantes des duplicatas, quel que soit le
poste ou la personne. Trois commandes nées sur `Base` :
`lire_etat_impression` (l'aperçu, n'écrit rien), `noter_impression` (au
clic, fait foi), `reimprimer_original` (permission
`pieces:reimprimer_original`, le patron ; journal `original_reimprime`).

**Écarts au plan, choisis en écrivant** : `objet_type` (les noms du
journal, pour que l'Historique retrouve client et pièce) au lieu de
`genre`, déjà pris par « facture, devis… » ; `rang` et `poste_id` en
plus ; l'original du patron en commande à part, pour que la permission
se juge au registre comme partout ; un brouillon ne se compte pas ; la
lecture d'état s'ajoute, sans elle l'aperçu ne pourrait pas montrer le
duplicata avant d'imprimer.

**Preuves** : `tests/impression_base.rs`, 8 scénarios (original puis
duplicata, autre poste et autre personne, original du patron tracé et
lu dans l'Historique, droit réservé au patron, brouillon, objet inconnu
ou introuvable, autre dossier, détecteur) — verts sur SQLite,
PostgreSQL et PostgreSQL sous compte limité ; `--workspace` SQLite vert
(715). **Reste** : la case par genre, le rendu (filigrane + mention), les
quatre chemins d'impression et le bouton du patron, la vérification dans
Chrome.

## 27/10/2026 — le duplicata : la case par genre, le rendu ; une relecture

**La case** (« enchaîne sur la case par genre ») : `ReglageGenre::duplicata`,
non par défaut (optionnel : une mise à jour ne change pas le papier),
refusée sur relevé et bulletin. Le serveur la lit : il déduit le genre
(`genre_imprime` : pièce → `genre_de_piece`, rouleau → ticket, reçu),
d'où un `format` facultatif sur les trois commandes. Case décochée :
l'impression se compte quand même — cochée plus tard, la feuille déjà
sortie reste l'original — et l'original du patron est refusé (rien à
rattraper). Case « Duplicata » dans Paramètres → Documents.

**Le rendu** : `Habillage.duplicata = { originalLe }`, posé par
l'appelant d'après le serveur. « DUPLICATA » en travers de chaque page
(15 %, par-dessus le filigrane de la société) et un cadre net en haut du
corps ; le ticket, deux lignes sans filigrane. Vu dans le navigateur
(générateur chargé depuis Vite) : facture A4 sur une page, reçu A5 à
600 px sur 794, ticket lisible, original sans aucune marque.

**Relecture demandée par le propriétaire** (« ce qu'on n'a pas pensé, les
erreurs de logique ») : une **erreur à moi**, corrigée — une facture ou
un avoir *fournisseur* passait pour une facture et serait sorti en
« DUPLICATA » ; ce n'est pas notre pièce (`hors_compte`). Le reste est
consigné dans [ALERTES.md](ALERTES.md) § 27/10 : le **1er janvier 2027**
(exercices finissant le 31/12, aucun avertissement, seul le patron
prolonge), l'**horloge qui recule**, **Smart App Control** chez le
client, la **CSP** confirmée sans l'adresse du serveur.

**Preuves** : `impression_base.rs` 10/10 (+ case décochée, ticket) sur
SQLite, PostgreSQL et compte limité ; `documents_base` 14/14 sur
PostgreSQL ; unitaires du noyau 235 ; `--workspace` SQLite 719 (lancé
avant le correctif fournisseur, rejoué depuis sur les fichiers touchés) ;
`tsc` propre.

## 27/10/2026 (suite) — le duplicata à l'écran : chemins d'impression, bouton du patron

« Attaque le lot 4. » Une aide, [lib/duplicata.ts](../src/lib/duplicata.ts),
pour les quatre chemins : l'aperçu **lit** (`lire_etat_impression`), le
clic **note** (`noter_impression`) et la feuille est **regénérée d'après
cette réponse** — un autre poste a pu imprimer entre-temps. Le serveur ne
répond pas : **on n'imprime pas**, une feuille sans marque serait un
original de plus. `ApercuPiece` (et le ticket du POS, qui passe par lui),
`ApercuRecu` (paiement client ou fournisseur), `Pieces.tsx` et
`FicheClient.tsx` (impressions directes). Bouton « Réimprimer l'original »
dans les deux aperçus, pour qui a `pieces:reimprimer_original`, quand la
prochaine feuille serait un duplicata ; l'aperçu dit « original imprimé
le … — celle-ci sortira en duplicata ». Le bon de sortie ne se compte pas.

**Trouvé en essayant** : passer d'A4 au ticket montrait un instant la
marque et le bouton d'A4. L'état est maintenant rattaché au format pour
lequel il a été lu, et les boutons sont grisés tant qu'il n'est pas
arrivé. (Une partie de l'éclair venait du rechargement à chaud de React,
qui gardait l'ancien composant : mesuré à 0 faux sur 217 après
rechargement complet.)

**Preuves** : `tsc` propre ; serveur construit et lancé sur une base
jetable, appelé par HTTP avec les paramètres de l'écran — 14/14 ;
l'écran dans le navigateur contre ce serveur (aperçu, avertissement,
bouton, ticket). **Puis l'essai du propriétaire dans la vraie fenêtre**
(banc `monoposte -Neuf`, vraie boîte d'impression Windows) : original
sans marque, duplicata marqué, original du patron sans marque et tracé
dans l'Historique, ticket sans marque — conforme. Non rapportés :
l'impression annulée, le reçu à l'écran. **À savoir** : un serveur d'avant ce
lot répond « commande inconnue » et l'écran refuse alors d'imprimer — le
serveur se met à jour avec la fenêtre.

**Une perte, de mon fait** : pour cet essai, `lancer.ps1 monoposte -Neuf`
a supprimé `outils/banc/.travail/monoposte` — la base d'essai que le
propriétaire remplissait à la main depuis le 26/10, et ses sauvegardes
(sans corbeille, pas de cliché lisible sans droits d'administrateur).
L'en-tête d'ETAPES disait de ne pas l'effacer ; il n'avait pas été lu
avant de lancer le banc. Consigné dans ALERTES § 27/10.

## 27/10/2026 — D38 les exercices comme dans Ciel, et l'avis avant leur fin

Ouvert par la relecture du même jour : chaque dossier amorcé en 2026 a un
exercice qui finit le 31/12/2026 ; le 1er janvier, toute écriture datée
est refusée (D21) et rien ne prévenait. Mené dans une session à part
(« Prévenir avant la fin de l'exercice »), sur les fichiers des dossiers
seulement.

**L'avis** : `coeur/fin_exercice.rs` (pure) dit le dernier jour
écrivable sans interruption depuis aujourd'hui, jugé par
`dossier_accepte` — l'avis prédit exactement le refus. Rien au-delà de 30
jours, orange dans le mois, rouge les 7 derniers et le jour du refus.
`lire_avis_fin_exercice` (ouverte à tous), bandeau `AvisFinExercice` dans
les deux branches du tableau de bord : « prolonger ou ouvrir le suivant »
et un bouton vers Paramètres → Dossiers pour qui a `dossiers:gerer`,
« prévenir le patron » sinon.

**D38, demandé en cours de route par le propriétaire** (« on suit la
logique de Ciel si elle n'altère pas la nôtre ») : dates libres
(juillet → juin), plusieurs années (5 au plus — une faute d'année
bloquerait le dossier des dizaines d'années), **couper** le dernier
exercice ouvert (le suivant s'ouvre le lendemain dans la même
transaction), et **clore seulement un exercice terminé, dans l'ordre** :
clore celui qu'on vit arrêtait toutes les ventes sur-le-champ, sans
remède. L'écran dit la raison avant la confirmation. D21 ne bouge pas.

**Preuves** : `exercice_coupe_base` 5/5, `fin_exercice_base` 7/7 sur
SQLite, PostgreSQL et compte limité ; six anciens tests qui closaient un
exercice encore en cours réécrits (même vérification, exercice terminé) ;
le test de route du serveur (409, clôture acceptée une fois l'exercice
terminé). Au passage, `postgres_amorcage::le_compte_limite_ne_voit…`,
rouge sur ce poste depuis le début (il attendait le refus en anglais,
ce PostgreSQL répond en français), accepte les deux : 22/22 sur
`gescom_test`. **Pas vu à l'écran.**

## 27/10/2026 (soir) — les lots A à E, et le serveur sur 40000–40100

Les cinq sessions des lots n'ont jamais démarré : créées disque plein
(1,2 Go libres, 5,4 Go par worktree). Place libérée (19,6 Go : caches
de compilation régénérables, restes d'essais), puis, à la demande du
propriétaire (« on fait ici seulement les lots »), les lots faits ici
l'un après l'autre, chacun par un agent, relu et rejoué ensuite. Les
quatre questions ouvertes du plan prises dans le sens proposé : retour
déduit à sa date, horloge qui refuse, caisse par utilisateur retirée,
`gescom.db` seulement plus ouvert.

**A — la CSP, puis le port (D39).** Élargir `connect-src` à `http:
https:` a été refusé par le contrôle de sécurité. Le propriétaire a
choisi le port 7300 seul, puis : « prends un autre port dans une plage
de 40000 à 40100 », repli automatique, 7300 abandonné. La CSP des deux
fenêtres énumère les 101 ports (`tests/plage_de_ports.rs` la compare à
`protocole`) ; le serveur local du poste qui garde la base prend le
suivant libre et `poste.json` le retient (sans sonder toute la plage :
sous Windows une connexion refusée sur 127.0.0.1 coûte deux secondes) ;
une caisse réglée en 7300 redemande une fois ; installeur et pare-feu
ouvrent la plage. **Vérifié sur le build release** : la CSP compilée
porte la plage ; appliquée par Edge à une page locale, elle joint le
serveur release sur 40000 et refuse un témoin hors plage (39999, CORS
ouvert) et 7300. La vraie fenêtre release refuse de tourner hors d'une
installation (la garde) : **pas essayée installée**.

**B — des chiffres justes après un retour.** CA net = ventes − retours
datés dans la période (tableau de bord, courbe, rapport mensuel, tops) ;
l'échange ne se déduit pas (son remplacement n'est pas une vente). La
dette éteinte par un retour (`paiement.mode = 'retour'`) n'est plus
« payée » : Relances, fiche et liste clients, « Acompte reçu » imprimé
(+ « Éteint par retour »). D41 se solde dans la transaction du règlement
qui crée le résidu, sur tous les chemins de règlement.

**C — garde-fous.** L'horloge : plus de 5 min de recul sur la dernière
écriture → toute écriture refusée ; le piège d'une heure réglée en
avance puis corrigée a son geste du patron (Paramètres → Horloge, au
journal). La caisse par utilisateur est retirée (refusée net, un tiroir
par dossier). `lire_lignes_piece` masque les prix d'achat. « Stock non
suivi » dans les écrans d'achat.

**D — les données.** Le nom du fournisseur figé sur ses pièces émises ;
« Rattacher à un client » une fiche de passage (toute son histoire, la
fiche désactivée, nom et code figés sur ses pièces) ; la colonne
« suivre le stock » de l'import CSV (refuse plutôt que de faire reprendre
un suivi sans comptage) ; Retours paginé par 50.

**E — le ménage.** La fenêtre n'ouvre plus `gescom.db` ; les 182 façades
`Connection` refusent au même endroit (le grand nettoyage reste à
faire). L'essai à la main a son dossier (`essai-main.ps1`, 40050), le
banc marque les siens et `-Neuf` refuse le reste ; la base d'essai a été
**copiée** (empreintes identiques), l'ancien dossier attend une décision
à la main.

**Trouvé en route** : `src-tauri/binaires/` gardait un serveur du 20/09,
recopié sans erreur dans `target/<profil>` à chaque construction de la
fenêtre — `preparer_serveur.ps1` l'a remplacé ; voir ALERTES. Ce qui
reste (TVA des retours, relevé client, facture réglée par avoir, dette
fournisseur en brouillon, en-tête CSV partiel…) est listé dans ALERTES.

**Preuves** : `retours_chiffres_base` 9, `garde_fous_base` 6,
`donnees_base` 6 sur SQLite, PostgreSQL et compte limité ; noyau SQLite
complet vert ; **noyau PostgreSQL complet 748/748** (62 séries) ;
serveur 11 + routes 14 ; fenêtre `gescom` 20 ; `tsc`
propre ; essai release de la CSP ci-dessus. **Rien vu à l'écran.**

## 27/10/2026 (nuit) — D40, la fenêtre « Gescom Serveur » et la recherche automatique

Le propriétaire : « pour l'IP et le port, une interface graphique dédiée
à mon serveur, comme sur Ciel », puis : comment Ciel fait-il découvrir
le serveur ? Vérifié dans les guides de Sage et les forums : rien
d'automatique — le client désigne le serveur par son nom sur le réseau
local, ce que Gescom accepte déjà. Proposé mieux, accepté : la recherche
automatique. Choix écrits avant, acceptés (D40).

La coque de D25, jamais construite, l'est : une fenêtre sans base qui
affiche la console du serveur et, quand il ne répond pas, propose de
démarrer le service. La console gagne son panneau Réseau ; le changement
de port ou d'écoute ne se fait que depuis le poste du serveur, par le
patron, et c'est le serveur qui réécrit `serveur.json` et redémarre. La
recherche répond en UDP sur 40000 sans rien dire de la boutique ; la
caisse a son bouton « Rechercher le serveur ». Fait par un agent (coupé
une fois par une panne réseau, repris là où il était), relu et rejoué.

**Trouvé au passage** : la lecture des règles de pare-feu ne voyait pas
40057 dans « 40000-40100 » (corrigé, toute langue de Windows) ; et
`check --package gescom` recopie le serveur de `binaires/` sur
`target\debug\gescom-serveur.exe` — `binaires/` refait par
`preparer_serveur.ps1` après ce chantier.

**Preuves** : `coeur::reseau_serveur` et `decouverte` (règles pures) ;
serveur 13 + routes 18 ; `gescom-serveur-fenetre` 2 ; `plage_de_ports` 3
(la CSP de la nouvelle fenêtre et sa copie de la plage) ; noyau 291 ;
`tsc` propre. **Pas vu en vrai** — section K de TESTS-MANUELS.

## 28/10/2026 — D41, la paie relue et corrigée

Le propriétaire, avant de construire les installeurs : « la paie aussi,
mais vérifie-la pour d'éventuelles erreurs de logique ». Relecture par un
agent qui n'avait pas écrit le code, chaque doute rejoué par un test
jetable : **10 défauts confirmés**. Les plus lourds en argent : un départ
le 15 payé le mois entier (et sa dernière fiche impossible à faire à
l'écran) ; un prorata compté sur les jours *marqués* (une entrée au 15
payée entière, deux absences marquées → 0 F) ; un gestionnaire sans droit
de paie qui efface un salaire en changeant un téléphone ; une
rectificative qui reprend le salaire d'aujourd'hui (5 000 F de prime
oubliée → 20 000 F à verser). Et « 15.000 » tapé à l'écran faisait 15 F.

Quatre choix tranchés par le propriétaire, tous dans le sens proposé : un
jour non marqué compte présent ; valider avant la fin du mois est permis,
puis les jours sont figés ; le trop-versé devient une avance ; la
commission se calcule nette des retours. Le reste suit (D41). Corrigé
par un second agent, puis rejoué ici. Les questions de principe (paie
hors caisse, maximum de retenue, avance datée du jour, commission sur
crédit impayé, barème ITS) restent ouvertes dans D41.

**Preuves** : `paie_corrections_base` 17 — chaque exemple de la relecture
au franc près (Bakary 30 000 F, Issa 21 935 F en octobre, Moussa
37 500 F, Awa 37 500 F, deux absences 56 000 F, rectificative 5 000 F,
trop-versé repris → octobre 52 000 F, CA de septembre 60 000 F) — sur
SQLite, PostgreSQL et compte limité ; les tests de la paie existants
ajustés aux nouvelles règles ; noyau + serveur SQLite 837/837 ;
`tsc` propre. **Écrans d'Équipe pas vus.**

## 28/10/2026 (suite) — la paie commitée, les trois installeurs construits

Commit `5fd5c82` sur `rh` (la paie, D41), à la demande du propriétaire ;
rien de poussé. Puis les trois installeurs, depuis ce commit, sans rien
installer :

| installeur | où | taille |
|---|---|---|
| caisse | `src-tauri\target\release\bundle\nsis\Gescom_1.12.0_x64-setup.exe` | 213,5 Mo (dont WebView2 hors ligne 205 Mo) |
| Gescom Équipe | `src-tauri\target\release\bundle\nsis\Gescom Equipe_0.1.0_x64-setup.exe` | 2,5 Mo |
| serveur | `dist\Gescom-Serveur_0.2.0_x64-setup.exe` | 4,2 Mo, signé et horodaté (service + fenêtre signés aussi) |

Ordre suivi : `preparer_serveur.ps1`, `npm run tauri build`,
`npm run equipe:build`, `construire_installeur_serveur.ps1` (après le
client : `vite build` vide `dist\`). **Deux ralentissements** : Microsoft
avait publié un nouveau WebView2, que Tauri retélécharge (la copie du
23/10 n'est pas reprise) — le téléchargement s'est figé sur une coupure
d'Internet, et une liaison figée ne repart pas : arrêter `tauri build` et
relancer ; et 0,3 Go de mémoire libre (Chrome, un Playwright d'un autre
projet) a fait traîner `vite build`. Disque : 4,2 Go libres après coup ;
`%LOCALAPPDATA%\tauri\x64` garde sept WebView2 (~1,6 Go), un seul sert.
**Pas essayé** : aucune installation, la fenêtre Gescom Serveur, l'accord
administrateur (TESTS-MANUELS, section K), les écrans d'Équipe.

Le propriétaire a ensuite tranché les cinq questions de D41, chacune dans
le sens proposé — **D42** : case « pris dans le tiroir » ; l'avance
étalée à la saisie et datée dans le passé (31 jours au plus) ; la
commission sur l'encaissé ; l'ITS saisi à la main.

## 28/10/2026 (fin) — D42 codée

Plan en cinq lots et huit choix de détail, acceptés tels quels. **Le
tiroir** : `caisses::mouvement_especes_sur`, appelée dans la transaction
de l'avance ou du versement avant toute autre écriture — caisse fermée,
rien ne s'écrit. Une avance du tiroir est d'aujourd'hui ; annulée,
l'argent revient au tiroir (caisse ouverte exigée). Le journal comptable
CA ne lit que les dépenses : rien n'est compté deux fois. **L'avance** :
`Saisie` (date, mensualité, tiroir) ; la date suit la règle des ventes
(31 jours), jamais avant l'entrée ; la retenue s'arrête à la mensualité,
par paie. **La commission** : l'argent reçu dans la période sur les
ventes du compte faites pendant l'emploi, moins les retours non-échange
diminués de la dette qu'ils éteignent ; la bascule lit la dernière fiche
« sur le vendu » (`vendu_jusqu_au`). Trois anciens tests commissionnaient
des ventes à crédit jamais payées : leurs ventes sont maintenant payées,
ce qu'ils prouvent reste vrai.

**Incident** : la suite complète en debug a rempli le disque (0 octet,
`LNK1180`, `no space on device`). `target\debug\incremental` supprimé
(6,7 Go rendus), suite relancée avec `CARGO_INCREMENTAL=0 -j 2` : 2,75 Go
libres au plus bas. Le serveur de l'essai à la main ne tournait pas.

**Preuves** : workspace SQLite 877/877 ; PostgreSQL `gescom_test` (huit
séries de la paie, des avances, du personnel, des journaux et de
l'Historique), compte limité (cinq) ; `tsc` propre. **Pas vu** : les
écrans d'Équipe et l'avertissement de la caisse dans la vraie fenêtre ;
les installeurs du jour ne contiennent pas D42.

**Commit `b03ac85`**, puis les trois installeurs reconstruits avec D42
(13 h 32 → 13 h 46), sans rien installer : caisse 213,5 Mo, Équipe
2,5 Mo, serveur 4,2 Mo signé et horodaté ; `gescom-serveur.exe` porte bien
le motif `avance_salaire`. Place faite d'abord, avec l'accord du
propriétaire : les six anciennes copies de WebView2 de
`%LOCALAPPDATA%\tauri\x64` supprimées (celle du jour gardée).

## 28/10/2026 (soir) — le guide d'installation et les manuels en PDF

Demande du propriétaire : « tous les cas » d'installation et les
manuels en PDF ; trois documents choisis, rangés dans `docs/manuels/`.
Trois agents ont relevé dans le code les libellés exacts (installation,
nouveautés de la caisse depuis le 25/10, écrans d'Équipe).

- **`INSTALLATION.md`** (nouveau, 15 chapitres) : les trois programmes,
  un seul poste, plusieurs caisses avec le service, sans service
  (déconseillé), PostgreSQL et le compte limité, Équipe (sur quels
  postes), ajouter/renommer, passer d'un poste unique à plusieurs
  caisses, mise à jour, port, désinstallation, sauvegarde, problèmes.
- **`MANUEL.md`** 2.1 : section 0 réduite à l'essentiel (40000,
  « Rechercher le serveur »), le menu par dossier, les passages, le
  duplicata, les retours groupés, les articles non suivis, la colonne
  CSV, la sauvegarde quotidienne (elle disait « une fois par semaine »),
  la restauration **avec `--base`** (sans lui, elle allait dans la
  mauvaise base), « 13 bis » exercices et horloge, l'avertissement de la
  dépense « Salaire ».
- **`MANUEL-EQUIPE.md`** (nouveau) : connexion et droits, personnel et
  jours, avances (tiroir, date, morceaux), fiches, rectificative,
  versements, bulletin, cotisations et ITS, suivi clients.
- **`outils/manuels_pdf.ps1`** : pandoc (Anaconda) → HTML à la mise en
  page de `genererManuel.ts` → Edge sans fenêtre → PDF A4. Le script
  reste en ASCII (PowerShell 5.1 lit mal l'UTF-8 sans BOM). L'ancien
  `Gescom_Manuel.pdf` de la racine (09/09) est retiré.
- En route, trois libellés faux depuis D42 corrigés : la permission
  `personnel:avancer` (« sortie de caisse »), l'accueil d'Équipe
  (« versements depuis la caisse »), la ligne D42 du récapitulatif.

Les relevés ont trouvé des risques réels (désinstallation qui peut
effacer la base, passage monoposte → service, écran de connexion sans
lien réseau, Équipe sans caisse) : ALERTES, « Trouvé le 28/10/2026 en
écrivant le guide ». **Rien n'a été essayé à l'écran** ; les manuels
décrivent le code.

## 28/10/2026 (nuit) — D43 l'assistant du premier démarrage, D44 les doublons

Demande du propriétaire : un assistant de création du dossier « comme
dans Ciel » ; en cours de route, l'import/export des fournisseurs et la
gestion des doublons (articles aussi), des icônes qui distinguent les
trois applications, l'Historique de caisse « jaune sur jaune » avec des
pictogrammes inutiles, et « changer l'état de la TVA dans l'app ».
Chaque choix proposé avant le code et accepté (DECISIONS D43, D44).

- **Relevé** : une base neuve démarrait « Ma boutique », année civile,
  `admin123` et `employe123` ; rien ne savait qu'elle était neuve ; la
  devise n'était écrite qu'à l'amorçage ; ni TVA par défaut, ni
  numérotation réglable, ni fond proposé ; l'import d'articles perdait
  TOUT sur un code-barre déjà pris (erreur SQL) et remettait à 0 la TVA
  d'un article existant quand la colonne était vide ; aucun import ni
  export de fournisseurs, aucune vérification de doublon à leur création.
- **D43** : marque `assistant_dossier` posée par l'amorçage, retirée par
  la démonstration (`amorcage::mode_essai`, le banc et les scénarios) ;
  verrou dans `api.rs` ; `/sante.assistant_a_faire` ; `AssistantDossier`
  en huit étapes, tout en une transaction ; `employe` / `employe123`
  seulement en démonstration ; TVA des nouveaux articles appliquée par le
  comptoir, Paramètres et l'import ; premier numéro de facture ; fond
  habituel ; puis l'état de la TVA en tête de Paramètres → TVA
  (`definir_tva_boutique`, « aussi les articles existants ? »), qui
  remplace un « Taux global » non transactionnel.
- **D44** : `coeur/doublons.rs`, aperçu `simuler` (le même code dans une
  transaction qui retombe), `fournisseurs_csv.rs`, `forcer` pour un
  homonyme, `ImportCsvAvecApercu`, `ImportExportFournisseurs`.
- **Icônes** : `outils/icones/pastilles.py` + `tauri icon` ;
  `serveur-fenetre/icons` et `equipe/icons` à part.
- **Historique de caisse** : le halo de la page Caisse se peignait par-
  dessus les blocs non positionnés (`relative z-[1]` manquait) ; orange
  500 à 2,80:1 remplacé par orange 800 (7,31:1), rouge 700, vert 700 ;
  pictogrammes retirés devant les chiffres.
- **Branche** : le travail est parti de `main` (8372aa4) ; `rh` avait
  D41 et D42 en plus. Remis par-dessus `rh` (même contenu de base),
  sans conflit ; mes décisions renumérotées D43/D44.
- **Tests** : `assistant_base` 10, `doublons_base` 9 (SQLite, PostgreSQL,
  compte limité) ; trois tests réalignés sur la fin du compte employé
  d'usine (`seed`, `auth_base`, `postgres_amorcage`) ; la sauvegarde du
  matin attendait la première apparition d'un fichier et voyait parfois
  le `-journal` d'une copie en cours (une fois sur deux après D43, le
  démarrage étant plus rapide d'un bcrypt) : elle attend la fin de la
  copie. La recherche UDP a échoué une fois puis passé : le poste.
- **Vu dans le navigateur** (serveur jetable 40060, base neuve, l'écran
  par Vite) : rappel du compte d'usine, assistant complet, refus de
  `admin123`, dossier créé et renommé dans la barre, TVA, caisse ouverte
  avec 25 000 proposés, Historique après une clôture à −500 F. Le serveur
  du poste sur 40000 (pas le mien) n'a pas été touché : le banc l'aurait
  arrêté.
- **Reste** : la vraie fenêtre (logo, import de fichiers, icônes
  installées, `p1` avec `-SansDemo`) ; Gescom Équipe sur une base neuve
  dit seulement « dossier pas encore créé » ; fusionner deux fiches déjà
  en double reste un autre chantier. Rien n'est commité.

## 29/10/2026 — D45 le seuil d'alerte ; l'assistant retiré puis rétabli

- **Le seuil** (« on définit le seuil pour les articles lors de la
  création ») : relevé d'abord — aucun seuil par article, 5 en dur au
  tableau de bord et aux rapports, 10 dans la page Stock. Deux choix
  posés : sans seuil, celui du dossier (5 d'office) ; un seuil par
  article, pas par magasin. Codé sur les trois chemins de schéma,
  `coeur::stock::statut_stock`, création (Paramètres, Achats), fiche,
  import/export, tableau de bord, rapport, page Stock ; Paramètres →
  Société porte le seuil du dossier.
- **Deux demandes du propriétaire en route.** « La création de dossier
  doit être la même et deux dossiers peuvent coexister » : relevé —
  `parametres_societe` est une ligne pour l'installation (37 lectures) ;
  trois choix posés, réponse « on laisse pour cette version » (ALERTES).
  « On enlève l'assistant de création au 1er démarrage » : retiré
  (module renommé `reglages_dossier`, rappel du compte d'usine gardé),
  puis « on peut revenir ici » : retour à l'état du commit `86f7ba8` en
  gardant le seuil (choix posé). Le travail du retrait est gardé dans un
  patch hors du dépôt.
- **Tests** : `seuil_base` 5 (SQLite, PostgreSQL, compte limité),
  workspace SQLite 915/915 ; la ligne exportée finit désormais par la
  colonne « Seuil » (`donnees_base` réaligné).
- **Vu dans le navigateur** (serveur jetable 40060) : le champ à la
  création (« celui du dossier : 5 »), le badge orange à 6 sous 10,
  « 1 alerte » au tableau de bord, la fiche qui dit « en alerte sous
  10 », puis 0 qui coupe l'alerte. **Pas commité.**

## 29/10/2026 (suite) — l'essai dans la vraie fenêtre, D46 la lisibilité

- **La vraie fenêtre** : `gescom.exe` de débogage construit depuis ce
  travail, son écran Vite sur 1420, un `poste.json` d'essai qui la
  branche sur un serveur jetable (40060, base vierge) — le serveur du
  propriétaire sur 40000 n'a pas été touché (le banc l'aurait arrêté).
  Construire la fenêtre recopie l'ancien serveur embarqué sur
  `target/debug/gescom-serveur.exe` : serveur reconstruit après (piège
  connu du banc). Le propriétaire a fait l'essai : connexion qui rappelle
  le compte d'usine, assistant complet, **logo par la boîte Windows**,
  **import d'articles** (3 créés avec leurs seuils ; « riz  LOCAL »
  doublon dans le fichier et le code-barre déjà pris refusés), import de
  fournisseurs depuis la page Fournisseurs (l'étape de l'assistant avait
  été sautée). Deux CSV d'essai posés sur son Bureau (`essai-gescom`).
- **« Jaune sur jaune » dans le résultat de l'import**, « regarder si
  le problème est récurrent » : relevé — 43 encadrés sur fond orange,
  ambre ou jaune pâle dans 22 écrans, 25 textes orange clair, des
  boutons orange 600 à 3,56:1. Deux choix posés : partout avec un
  garde-fou ; pastilles à fond pâle et texte 900. 105 lignes par règle
  (script), puis relues à la main : les champs surlignés ont un bord
  plein (le liseré y faisait bizarre), une ligne de tableau ne porte pas
  de bordure (fond neutre), les bords internes des encadrés passent au
  gris. `src/lib/lisibilite.test.ts` refuse désormais chaque rechute
  (prouvé sur un faux écran : trois fautes nommées). Le propriétaire a
  refait l'import : « c'est lisible ». D46. **Pas commité.**

## 29/10/2026 (fin) — l'accueil d'Équipe sur toute la largeur, les captures

- **L'accueil de Gescom Équipe** (« trois cards dans l'accueil ne
  prennent pas la largeur totale, deux en haut et une en bas ») : il
  tenait dans 768 px (`max-w-3xl`) sur deux colonnes ; ses cartes
  prennent désormais toute la largeur, autant par ligne que la place le
  permet (comme les tuiles de la caisse). L'accueil de la caisse, lui,
  remplissait déjà la largeur (vérifié à 1600 px).
- **Pas d'installeurs** : la construction prend trop de place (9,7 Go
  libres). À la place, 18 captures à 1600 × 900 sur trois serveurs
  jetables (40062 base neuve pour l'assistant, 40061 démonstration avec
  six ventes et une clôture à −500 F), par Edge déjà installé (rien
  d'installé) ; posées sur le Bureau du propriétaire,
  `essai-gescom\captures`. Gescom Équipe ne s'ouvre pas en fenêtre
  d'essai : elle lit le `poste.json` de la vraie caisse, donc le vrai
  serveur — vue dans le navigateur.
- **L'Historique de caisse en tuiles** (« enlève l'emoji danger et le
  tiret rouge à côté, utilise des cards comme dans le dashboard ») :
  clôtures, justes, manques, excédents, puis cumul, moyenne et pire
  manque en `KpiPetit` sur la grille du tableau de bord, chacun avec son
  mot (« écart nul », « moins d'argent que prévu ») ; le diagnostic dans
  une tuile de texte. Plus de triangle ni de liseré.
- **La fiche client d'Équipe** (Suivi clients) sur toute la largeur, ses
  chiffres en tuiles ; puis, à la demande, **tous les écrans d'Équipe**
  (Personnel, Paie, Fiches du mois, Cotisations, Suivi clients et ses
  listes) sans largeur limitée. Restent étroits, exprès : « Bientôt » et
  le message « serveur injoignable » (du texte à lire). Captures 18 à 22.

## 29/10/2026 (soir) — le logo TIJ sur les trois applications

- **Le plan pour compléter Équipe** (« qu'est-ce qu'on peut ajouter à
  Équipe ») : EQ-1 à EQ-6 dans [PLAN-EQUIPE.md](PLAN-EQUIPE.md) § 5,
  une session chacune, décisions D47 à D52 réservées, une base
  PostgreSQL par session (`gescom_test_eq1` … `eq6`, créées). Les six
  ont proposé leur décision et attendent l'accord.
- **Une icône moderne avec TIJ, en lien avec la gestion commerciale** :
  quatre propositions (barres qui montent, monogramme, ticket de caisse,
  sac de boutique), rendues en grand et à 32 px sur barre claire et
  sombre (capture 24). **Choix du propriétaire : A, les barres** — T, I
  et J en trois barres de plus en plus hautes, le J finit en flèche.
- Le logo vit en vectoriel (`outils/icones/tij.svg`, des formes, aucune
  police) ; `rendre.mjs` le rend en 1024 px par Edge (rien installé) ;
  `tauri icon` en tire les jeux des trois applications.
- **Les pastilles passent en haut à gauche** : en bas à droite, elles
  cachaient le crochet du J et on lisait « TI ». Le coin haut gauche est
  vide dans le nouveau logo. Capture 25 : les trois, lisibles à 32 px.
- Les écrans n'affichaient pas l'ancien logo (une icône `Store` sur le
  choix du profil) : rien à changer dedans. Pas d'installeur construit :
  les icônes se verront à la prochaine construction.

## 29-30/10/2026 — D47 le prêt au personnel (EQ-1)

- **Le relevé d'abord** : l'avance étalée de D42 (« X F par paie ») faisait
  déjà l'essentiel d'un prêt — retenue bornée, report, rectificative,
  trop-versé, journal PA. Plutôt qu'un second circuit, D47 fait du prêt
  une **avance d'une autre nature** (`avance.nature = 'pret'`) et n'ajoute
  que ce qu'il a de plus : l'échéance **par mois civil** (un journalier
  à la quinzaine ne la paie qu'une fois), la première retenue le mois
  suivant (reculable de trois mois), une échéance manquée qui allonge le
  prêt sans se doubler, les avances retenues d'abord, un seul prêt à la
  fois, le départ qui retient tout le reste, puis le remboursement à part
  ou la remise (`pret_reglement`, PA trésorerie / 421 et 658 / 421).
  Proposé, accepté tel quel. Droit : `paie:valider`.
- **Soldé ne s'écrit pas** : `montant - retenu = 0`. La rectificative rend
  ce qu'elle avait retenu et retient de nouveau : un prêt soldé par une
  fiche qu'on rectifie se rouvre de lui-même (scénario).
- **Une régression trouvée par la suite complète** : en généralisant le
  libellé d'annulation au prêt, « Avance annulée » était devenu « Avance
  annulé(e) » — `paie_base` le cherchait. Rétabli (« Avance annulée »,
  « Prêt annulé »).
- **Le disque** : six sessions compilaient en même temps, C: est tombé à
  2,4 Go (LNK1180, os error 112). Pause demandée, `target` effacé,
  reprise une à une. `--workspace` ne passe plus dans un worktree neuf
  (la fenêtre Tauri veut `binaires\gescom-serveur-*.exe`) : noyau et
  serveur testés à part, 909/909.
- **L'écran dans le navigateur** (serveur jetable 40071, Vite 1441) : le
  prêt annonce « 6 échéances, de novembre 2026 à avril 2027 », la fiche
  de novembre porte « Prêt du 30/10/2026 (reste 50 000 F après cette
  paie) − 10 000 F », le remboursement à part laisse 45 000 F. Au passage,
  hors EQ-1 : dans un navigateur, la boîte « changer le mot de passe par
  défaut » reste ouverte après un changement réussi (la confirmation est
  une boîte Tauri) — noté, pas corrigé.
- **Après fusion** (plan § 5) : EQ-2 et EQ-3 lisent `avance` — filtrer
  `nature = 'avance'` ou montrer les prêts à part.

