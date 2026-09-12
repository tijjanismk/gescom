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

Dernière mise à jour : **12 septembre 2026**.
État : **392 tests SQLite + 21 tests PostgreSQL**, tous au vert — les 21
rejoués sur une vraie instance (`gescom_test`), plus **83 scénarios**
répartis en douze fichiers `*_base.rs` qui tournent sur les deux
moteurs. **Le portage est complet : 186 des 187 commandes du serveur
sont servies sur `Base`** (la 187e, le catalogue des permissions, ne
touche pas la base).

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

**La sauvegarde** est tranchée plutôt que portée : `VACUUM INTO`
copie un fichier SQLite ; une base PostgreSQL se sauvegarde avec
`pg_dump` sur le serveur. `sauvegarder_base_sur_base` **refuse
clairement** sur PostgreSQL au lieu de faire semblant, et
`lire_config_sauvegarde` rend le moteur pour que l'écran le dise.
Brancher `pg_dump` est un chantier à part.

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
l'application sait faire — sauf la copie de sauvegarde, qui se fait
autrement.

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
GESCOM_PG="postgresql://..." cargo test -p gescom-noyau \
    --test tableau_bord_base -- --test-threads=1     # idem, tableau de bord
# ... et de même pour pieces_base, achats_base, retours_base.

# ⚠️ Les scénarios PostgreSQL font DROP SCHEMA : JAMAIS sur la base du
# serveur (`gescom`). Une base jetable existe pour ça : `gescom_test`.
# `cargo-tenace.ps1` avale `-p` et `--` : passer `--package`, et lancer
# les scénarios PostgreSQL avec cargo directement, depuis bash.
```

`cargo-tenace` et non `cargo` : Smart App Control bloque les binaires
fraîchement liés, et l'erreur ne ressemble pas à ce qu'elle est. →
[environnement-windows.md](modules/environnement-windows.md)

Les tests PostgreSQL ne tournent que si `GESCOM_PG` est défini. Un test
qui exige un service tiers ne doit pas faire échouer la suite de
quelqu'un qui ne l'a pas installé.
