# Tout ce qui restait ouvert, tranché

Le multi-dossier a son propre document :
[PLAN-MULTISOCIETE.md](PLAN-MULTISOCIETE.md). Celui-ci règle **tout le
reste** — les points qui traînaient sans décision et qui, chacun,
pouvaient bloquer un travail au moment de s'y mettre.

Même règle que partout ici : les chiffres sont **mesurés**, jamais
estimés. Écrit le 11 septembre 2026.

---

## D1 — Dans le code, un magasin s'appelle toujours `depot`

À l'écran, « dépôt » est devenu « magasin ». À l'intérieur, le logiciel
dit encore `depot` : la colonne, les fonctions, les identifiants.

| | coût | gain |
|---|---|---|
| **Laisser `depot` dans le code** | un décalage permanent entre l'écran et le code | aucun risque |
| Renommer partout | ~200 requêtes et une migration de base | aucun gain visible pour toi |

**Décision : on ne renomme pas.** Le commerçant lit « magasin », le
code dit `depot`, et la correspondance est écrite une fois pour toutes
dans le glossaire du chapitre 2 du plan.

Renommer ne changerait **rien** à ce que tu vois. Ce serait 200
occasions de casser quelque chose pour un bénéfice qui n'existe que
pour celui qui lit le code — c'est-à-dire moi.

---

## D2 — Les dates sortent du SQL

C'est la décision la plus utile de ce document, et elle ne se contente
pas de rendre le code portable.

### Le problème

Neuf endroits calculent un nombre de jours **à l'intérieur** de la
requête :

```sql
WHERE julianday('now') - julianday(cree_le) > 15
```

`julianday` n'existe pas dans PostgreSQL. Mais le vrai défaut est
ailleurs : cette requête demande l'heure **à la base de données**. Or
l'heure qui fait foi est celle du serveur. Un décalage entre les deux —
fuseau, horloge non réglée — et un chèque de 14 jours est compté pour
16. Le code porte d'ailleurs déjà un `'localtime'` collé à un endroit
et absent ailleurs : les deux ne donnent pas le même résultat.

### La décision

**Le calcul se fait en Rust, et le SQL ne fait plus que comparer.**

```sql
WHERE cree_le < ?1        -- ?1 = aujourd'hui moins 15 jours
```

Trois gains d'un coup :

1. **Portable** : une comparaison de texte marche sur les deux moteurs,
   puisque les dates sont rangées en clair (`2026-09-11T14:30:00`).
2. **Juste** : une seule horloge, celle du serveur. Plus de question de
   fuseau.
3. **Rapide** : une comparaison peut se servir d'un index ; un calcul
   sur chaque ligne, non. Sur une base de plusieurs années de ventes,
   la différence se voit.

Et pour les endroits qui **affichent** un nombre de jours de retard : la
requête rend la date, Rust calcule les jours au moment de composer la
réponse.

> Avant : « donne-moi les ventes dont l'âge dépasse 30 jours »
> Après : « donne-moi les ventes antérieures au 12 août »

---

## D3 — Les autres constructions : la table de correspondance

**54 occurrences mesurées**, réparties ainsi :

| ce qu'on écrit aujourd'hui | nb | ce qu'on écrira |
|---|---|---|
| `INSERT OR IGNORE INTO t …` | 26 | `INSERT INTO t … ON CONFLICT DO NOTHING` |
| `julianday(a) - julianday(b)` | 9 | une borne calculée en Rust (D2) |
| `strftime('%Y-%m', d)` | 5 | `substr(d, 1, 7)` |
| `substr(x, -5)` | 5 | `substr(x, length(x) - 4, 5)` |
| `date('now')`, `datetime('now')` | 6 | la date, passée en paramètre depuis Rust |
| `INSERT OR REPLACE INTO t …` | 3 | `ON CONFLICT (…) DO UPDATE SET …` |

Les découpages de date marchent parce que les dates sont rangées en
clair : le mois d'une date ISO, ce sont toujours les caractères 6 et 7.
`strftime('%m', d)` devient `substr(d, 6, 2)`.

### Deux règles qui vont avec

**Jamais deux variantes.** On n'écrit pas « si PostgreSQL alors ceci,
sinon cela ». On écrit du SQL que les deux moteurs acceptent. Deux
variantes, ce sont deux comportements à vérifier, et un jour l'une des
deux corrigée seule.

**On les réécrit module par module, avec le reste.** Pas de campagne
séparée : quand un module passe à PostgreSQL, ses constructions sont
traduites au passage, et ses scénarios le vérifient. Une campagne
séparée toucherait du code qu'on ne teste pas ce jour-là.

---

## D4 — Les sauvegardes : pg_dump, deux endroits

Tu avais écarté les sauvegardes en pensant au code. La question posée
sur les DONNÉES, la réponse est venue : on les fait.

> Il s'agit de tes ventes, tes clients, tes dettes. Si le disque du
> serveur lâche un mardi, les commits de GitHub ne te rendent pas les
> factures de l'année.

Et à Bamako, ce n'est pas une hypothèse d'école : coupures, surtensions,
vol du poste.

### Ce qui existe et ce qui manque

La sauvegarde actuelle utilise `VACUUM INTO`, une commande SQLite. **Elle
n'existe pas dans PostgreSQL.** Le jour où le serveur tourne sur
PostgreSQL, le bouton « Sauvegarder » ne fait plus rien d'utile.

### Décision

**Deux copies, deux endroits, automatiquement.**

| | |
|---|---|
| quoi | `pg_dump` — un fichier lisible, restaurable sur une autre machine |
| quand | tous les soirs, plus à la clôture de caisse |
| où | le disque du serveur **et** une clé ou un disque externe |
| garde | les 30 derniers jours, puis un par mois |

Le point qui compte plus que tout le reste : **une sauvegarde qu'on n'a
jamais restaurée n'est pas une sauvegarde.** L'écran doit proposer une
restauration d'essai, et le manuel doit dire de la faire une fois.

**Confirmé le 11/09/2026 : on fait `pg_dump`.**

**La restauration, le 20/09/2026** : `gescom-serveur --restaurer
FICHIER [--base …]`, serveur **arrêté**, depuis la machine du serveur.
Pas de bouton dans l'application : restaurer écrase tout ce qui a été
saisi depuis la sauvegarde, et un geste qui efface une journée de ventes
ne se fait pas d'un clic depuis une caisse. Sur SQLite, la base en place
est gardée à côté (`.avant-restauration-<date>`) ; sur PostgreSQL,
`pg_restore --clean --if-exists` remplace les tables. Un test de route
rejoue le cycle complet (sauvegarde, restauration, redémarrage).

---

## D5 — L'installeur n'est pas signé, et voici quand ça changera

Aujourd'hui : pas de signature. Windows affiche un avertissement à
chaque installation, qu'il faut écarter à la main.

**Décision : on ne signe pas tant que tu installes toi-même.** Tu es sur
place, tu sais ce que tu installes, l'avertissement ne t'apprend rien.

**Le jour où ça change :** quand un commerçant téléchargera Gescom sans
que tu sois derrière lui. À ce moment-là, l'avertissement ne dit plus
« fais attention », il dit « ce logiciel n'est pas fiable », et personne
n'installe. C'est le déclencheur, pas une date.

**Révisée le 19/09/2026 — l'installeur du serveur est signé, en
auto-signé.** Ça ne fait pas taire SmartScreen ailleurs (la chaîne de
confiance reste inconnue), mais l'installeur enregistre le certificat
comme éditeur de confiance de **la machine du serveur** : sur celle-là,
les mises à jour suivantes ne posent plus la question, et un fichier
modifié après signature ne s'installe plus. La clé privée reste dans le
magasin de l'utilisateur qui construit ; seule la partie publique
(`gescom.cer`) voyage. Le jour du certificat acheté, `outils\signer.ps1`
change d'empreinte, rien d'autre. → [installeur.md](modules/installeur.md)

---

## D6 — Le compte tout-puissant : aucun, et une commande de secours

Le rôle `superadmin` existe. **Personne ne le porte.** L'amorçage crée
deux comptes : `admin` (patron) et `employe`, tous deux obligés de
changer leur mot de passe à la première connexion.

Deux façons de combler le trou :

| | ce que ça donne | |
|---|---|---|
| un compte de secours livré | un identifiant et un mot de passe connus de tous ceux qui ont lu la documentation | **refusé** |
| **une commande sur le serveur** | il faut être devant la machine pour s'en servir | **retenu** |

**Décision : une commande du serveur**, du genre
`gescom-serveur --promouvoir admin`.

Le raisonnement : **qui est devant le serveur a déjà tout.** Il peut
lire la base, la copier, l'effacer. Lui donner un moyen propre de se
redonner les droits n'ouvre aucune porte qui ne soit déjà ouverte. Un
compte livré, lui, s'utilise depuis n'importe quelle caisse du magasin.

---

## D7 — Les permissions par personne : l'écran se fait

Les commandes existent, l'écran non. Ce sont les droits qu'on ajoute ou
retire **à une personne**, en plus de son rôle.

> Modibo est caissier, mais c'est lui qui reçoit les livraisons. Tu lui
> ajoutes « entrer du stock » sans en faire un magasinier.

**Décision : on fait l'écran**, dans la fiche utilisateur : la liste des
permissions, cochées par le rôle, avec la possibilité d'en ajouter ou
d'en retirer une.

Sans écran, ces commandes sont du code mort qui finit par se casser
sans que personne le remarque.

---

## D8 — Les images depuis une caisse : on envoie le fichier, pas son nom

Aujourd'hui, changer le logo depuis une caisse envoie au serveur le
**chemin** du fichier — `C:\Users\Awa\Bureau\logo.png`. Ce chemin ne
désigne rien chez le serveur, qui est une autre machine.

**Décision : la caisse lit le fichier et en envoie le contenu.** Le
serveur le range et le sert ensuite à toutes les caisses.

> Awa change le logo depuis sa caisse. Le lendemain, les factures
> imprimées aux trois comptoirs portent le nouveau logo, sans que
> personne n'ait touché aux deux autres postes.

---

## D9 — L'entretien de la base est un travail de serveur

`entretenir_base` (réorganiser les index, compacter) est aujourd'hui
une commande locale. Sur une caisse, elle ne sert à rien : la base
n'est pas là.

**Décision : elle passe dans la console du serveur**, avec les autres
opérations d'administration, et disparaît de l'écran des caisses.

---

## D10 — Le mot de passe de la base ne rentre pas dans le dépôt

L'adresse de connexion contient un mot de passe.

**Décision, déjà appliquée, écrite ici pour qu'elle ne se perde pas :**
elle vit dans `poste.json` ou dans une variable d'environnement. Jamais
dans le code, jamais dans un document versionné, jamais dans un exemple
de la documentation.

---

## D11 — Débrancher SQLite : ce que ça veut dire vraiment

Tu as dit « on débranche tout de SQLite ». En allant voir, j'ai trouvé
un fait qui change l'ordre des travaux, et je préfère le poser ici
plutôt que de le découvrir au milieu du portage.

### Le fait

**Le serveur ne sait pas ouvrir PostgreSQL.** Pas « il est mal
configuré » : il n'en a pas le code. Il ouvre sa base avec
`persistance::ouvrir_base`, qui ne connaît que SQLite, et il passe
ensuite cette connexion SQLite à ses **186 commandes**.

> Lui donner une adresse `postgresql://…` ne le ferait pas basculer :
> il créerait un fichier SQLite portant ce nom bizarre, l'amorcerait,
> et tout marcherait — sur une base vide. C'est le genre de panne qui
> ne dit rien.

Ce n'est donc pas une impression de ta part : **ta base est
effectivement SQLite, et elle ne peut pas être autre chose
aujourd'hui.**

### Ce que ça change dans l'ordre des travaux

Porter la vente et la facture était présenté comme « le dernier gros
morceau avant qu'une boutique tourne sur PostgreSQL ». C'est
nécessaire, mais **ce n'est pas suffisant** : même parfaitement
portées, elles ne serviraient à rien tant que le serveur ne peut pas
ouvrir une connexion PostgreSQL pour les appeler.

**Décision : le serveur tient une `Base`, pas une `Connection`.**

| | mesuré |
|---|---|
| commandes servies | 187 |
| endroits qui reçoivent la connexion SQLite | 186 |

Ces 186 endroits ne se convertissent pas tous d'un coup — la façade
existe précisément pour éviter ça. Le serveur tiendra les deux pendant
la transition : une `Base` pour ce qui est porté, et la connexion
SQLite pour le reste, **tant que la cible est un fichier**. Dès que la
cible est une adresse PostgreSQL, une commande non portée doit **refuser
clairement**, et non retomber en silence sur une base vide.

> « Cette opération n'est pas encore disponible sur PostgreSQL » est un
> mauvais message. C'est quand même mille fois mieux qu'une vente
> enregistrée là où personne ne la lira.

### L'ordre qui en découle

1. ~~Le serveur accepte une adresse PostgreSQL et tient une `Base`.~~
   **Fait le 11/09/2026.**
2. ~~Les commandes non portées refusent franchement sur PostgreSQL.~~
   **Fait le 11/09/2026**, dans le même geste.
3. La vente et la facture — le plus gros morceau, et le plus délicat.
   **Portées dans le noyau** (`creer_vente_sur_base`,
   `valider_facture_sur_base`, testées), **mais pas encore branchées** :
   voir le point ⚠️ ci-dessous.
4. Le reste des commandes, module par module.

#### Les points 1 et 2, en détail

`gescom-serveur` accepte désormais `--base postgresql://…` aussi bien
qu'un chemin de fichier. Ce qu'il fait de chaque cas :

| | fichier (aujourd'hui) | PostgreSQL |
|---|---|---|
| `Connection` SQLite (186 commandes) | ouverte, comme avant — **rien ne change** | `None` : aucune n'existe |
| `Base` | une **seconde** connexion vers le même fichier (WAL le permet) | l'unique connexion |
| une commande non portée appelée | fonctionne, comme avant | refuse avec « n'est pas encore disponible sur PostgreSQL », **avant** de toucher au registre |
| `/sante` | comme avant | répond quand même — c'est la route qu'un poste interroge *avant* tout jeton |
| sauvegarde automatique | comme avant (`VACUUM INTO`) | désactivée, refuse clairement (D4 : `pg_dump` pas encore écrit) |

Zéro changement de comportement sur une cible fichier — c'était la
condition. `Serveur.conn` est passé de `Mutex<Connection>` à
`Option<Mutex<Connection>>`, et chaque appelant (six endroits dans
`api.rs`, plus `sauvegarde.rs`) refuse au lieu de paniquer ou de
retomber sur une base vide.

✅ **Corrigé le 11/09/2026, le même jour : l'authentification est
portée.** `sessions` et `portes::permissions_de` l'étaient déjà —
trouvés en lisant le code, pas refaits. Il manquait
`postes::inscrire_ou_retrouver` ; écrit (`inscrire_ou_retrouver_sur`,
`lire_sur`). `api.rs` bascule `connexion`, `déconnexion`,
`authentifier` et le contrôle de permission de `/rpc` sur `srv.base`
**sans condition de moteur** — SQLite et PostgreSQL empruntent
désormais le même chemin pour se connecter. `/sante` compte aussi les
sessions actives sur les deux moteurs (`sessions::lister_actives_sur`).

12 scénarios dans
[auth_base.rs](../src-tauri/noyau/tests/auth_base.rs), dont le login
complet rejoué de bout en bout — identifier le compte, vérifier le mot
de passe, inscrire le poste, ouvrir la session, lire les permissions —
exactement dans l'ordre où `api.rs::connexion` le fait.

**Ce qui reste vrai** : les 186 commandes de vente, stock, pièces sont
encore sur `Connection` et refusent sur PostgreSQL avec le message
prévu plus haut. C'était le morceau suivant — module par module,
comme prévu — et douze d'entre elles sont faites, le même jour :

### Douze commandes branchées — **fait le 11/09/2026**

`Registre::aussi_sur_base(nom, poignee_base)` complète une entrée déjà
enregistrée avec sa version `Base` — appelée seulement quand `conn` est
absent (PostgreSQL). Zéro risque sur une cible fichier : `poignee`
(`Connection`) reste l'unique chemin tant qu'il existe.

Branchées : le comptoir entier (`lire_clients`, `lire_client_generique`,
`lire_depots`, `lire_depot_defaut`, `lire_articles_avec_unites`,
`creer_client_rapide`, `creer_article_rapide`, `modifier_client`,
`lire_clients_avec_creances`, `lire_config_scanner`), et les deux qui
comptent le plus : `creer_vente`, `valider_facture`.

**Une caisse connectée à un serveur PostgreSQL peut désormais vendre.**

### Les 186 branchées — **fait le 12/09/2026**

Le reste a suivi le même chemin, en huit lots d'une journée (pièces,
achats, retours ; réglages ; journal et rapports ; dépôts, avoirs,
créances ; fournisseurs ; listes et livraisons ; caisse et système).
186 des 187 commandes ont leur version `Base` ; la 187e ne lit pas la
base. Le tableau des lots est dans [ETAPES.md](ETAPES.md).

Ce que D11 a coûté, et qu'on ne referait pas autrement : **le trait
`Acces`** (`Base` et `Transaction` sous une même signature), sans lequel
chaque aide partagée aurait existé en deux copies ; et **le filet**
`tests/schema_commun.rs`, qui compare la base de la fenêtre à celle du
serveur — sept colonnes ou tables n'existaient que d'un côté.

Ce qui s'est décidé en route, sans nouvelle case :
- une commande **refuse** sur PostgreSQL ce qu'elle ne sait pas y faire,
  plutôt que de faire semblant — la copie de sauvegarde l'a été un
  après-midi, le temps d'écrire `pg_dump` (D4, fait) ;
- les écarts de moteur se règlent **une fois en SQL commun ou en Rust**,
  jamais en deux variantes (D3 tenue : `jours_depuis`, `SUBSTR`,
  `ON CONFLICT DO NOTHING`, sous-requête au lieu d'un alias dans
  `HAVING`) ;
- les versions `Base` corrigent au passage ce que les versions SQLite
  toléraient : recherches insensibles à la casse sur les deux moteurs,
  paramètres liés partout, transactions autour des écritures composées.

## Ce qui n'est pas une décision, mais un travail à faire

Ces points ne demandent pas d'arbitrage — seulement qu'on s'y mette :

- **Une vraie impression papier**, jamais essayée à la main. Le
  glisser-déposer du pied de page non plus.
- **La fenêtre en mode caisse**, jamais utilisée pour de bon : tous les
  essais sont passés par le navigateur.

Ce sont les deux endroits où un défaut ne se verra qu'en s'en servant.

---

## Récapitulatif

| | décision |
|---|---|
| D1 | `depot` reste `depot` dans le code ; « magasin » à l'écran |
| D2 | les calculs de dates passent en Rust, le SQL ne fait que comparer |
| D3 | 54 constructions traduites module par module, jamais deux variantes |
| D4 | `pg_dump` tous les soirs, deux endroits, restauration à essayer — **branché le 12/09/2026**, `--restaurer` **le 20/09/2026** |
| D5 | pas de signature tant que tu installes toi-même |
| D6 | aucun compte de secours ; une commande sur le serveur |
| D7 | l'écran des permissions par personne se fait |
| D8 | les images voyagent par leur contenu, pas par leur chemin |
| D9 | l'entretien de la base passe côté serveur |
| D10 | le mot de passe de la base reste hors du dépôt |
| D11 | le serveur tient une `Base` ; sur PostgreSQL, une commande non portée **refuse** au lieu de retomber sur SQLite — **186/187 portées le 12/09/2026** ; **dépassée par D22 le 23/09/2026 (v3 D-2)** : le chemin `Connection` du serveur est parti |
| D12 | le serveur est un **service Windows** (`GescomServeur`), installé à part, en administrateur ; il démarre avec la machine et se relance seul |
| D42 | **le tiroir, l'avance, la commission** (les questions de D41) : case « pris dans le tiroir » qui écrit la sortie de caisse, sinon hors caisse ; l'avance s'étale à la saisie (tout, ou X F par mois) et se date dans le passé (31 jours au plus) ; la commission sur l'**encaissé**, moins l'argent rendu ; l'ITS saisi à la main, pas de barème — codé le 28/10 |
| D41 | **la paie suit l'emploi et fige ses conditions** : seulement les jours d'emploi de la période ; prorata en entiers sur les jours du mois, un jour non marqué compte présent ; salaire au mois sur un mois civil ; la rectificative reprend les conditions figées de la fiche ; jours figés après validation ; trop-versé → avance ; commission nette des retours ; montants masqués partout sans droit de paie ; « 15.000 » = 15 000 F |
| D40 | **la fenêtre « Gescom Serveur »** (la coque de D25) et son panneau **Réseau** — port 40000–40100 et écoute changés par le patron, depuis le poste du serveur seulement, appliqués par le serveur lui-même — et la **recherche automatique** des caisses (UDP 40000, rien de la boutique) |
| D39 | **le serveur écoute de 40000 à 40100** (40000 par défaut, 7300 abandonné) : la CSP des deux fenêtres énumère ces ports et rien d'autre ; sur le poste qui garde la base, si 40000 est pris, le serveur local prend le suivant libre et `poste.json` le retient ; installeur et pare-feu suivent la plage |
| D38 | **des exercices comme dans Ciel** : dates libres (juillet → juin se donne), plusieurs années (**5 au plus**), et le dernier exercice ouvert **se coupe** en plein milieu — le suivant s'ouvre le lendemain, au moins jusqu'à l'ancienne fin ; on ne **clôt** qu'un exercice **terminé**, dans l'ordre ; D21 tient (pas de trou, refus hors dates) |
| D36 | **un article peut ne pas suivre son stock** (`article.gere_en_stock`) : il se vend normalement, jamais à découvert, hors des écrans de stock (Stock, inventaire, alertes, valeur), transfert refusé ; ses mouvements s'écrivent quand même ; cesser de suivre est tracé, reprendre exige un comptage par magasin |
| D37 | **le menu se règle par dossier** (Paramètres → Menu, patron) : cacher n'est qu'un affichage — aucune donnée, aucune règle, aucune trace ne change ; Tableau de bord, Ventes, Caisse, Paramètres toujours visibles ; « Boutique simple » / « Complet » |
| D35 | **chaque vente sans nom a sa fiche « passage »** (code `PAS-000123`, réservé dans la transaction) : hors des listes et des chiffres, payée en entier à la vente (ni crédit, ni acompte, ni avoir), « Passer en client ordinaire » garde l'historique ; une facture garde le nom écrit à son émission |
| D34 | le **suivi client** s'appuie sur la fiche client de Gescom : échanges, rappels, prospects, fiche 360 — pas de second fichier — [PLAN-EQUIPE.md](PLAN-EQUIPE.md) |
| D33 | **cotisations facultatives, jamais devinées** : lignes de réglage du dossier, vides par défaut, seulement pour les personnes déclarées |
| D32 | **les avances d'abord** : rattachées à la personne, retenues d'office sur la fiche suivante ; **hors caisse** (révisé le 24/09) |
| D31 | **la fiche de paie se stocke** (un document remis) : brouillon → validée (figée, numérotée) → payée ; une erreur = une rectificative. **Révisé le 24/09** : la paie est indépendante de la caisse (ni session exigée, ni mouvement), son coût se lit dans Rapports → CA mensuel |
| D30 | **une personne, pas un contrat** : nom, ce qu'elle fait, comment elle est payée (au mois, à la journée, à la commission, à la tâche, rien de fixe) ; tout le reste facultatif |
| D29 | **Équipe vit dans le même dépôt** : `equipe.html`, `src/equipe/`, `src-tauri/equipe/`, modules `personnel`/`paie`/`crm` en version `Base` seule (exception écrite à la règle 2) |
| D28 | **Gescom Équipe est une seconde fenêtre qui ne parle qu'au serveur** : aucune base à elle ; mêmes comptes, droits, dossiers ; le monoposte lance le serveur sur sa machine (D22) |
| D27 | **le serveur juge la saisie** : pas de mode « avoir » sans avoir consommé, montant > 0, quantité > 0, prix ≥ 0, remise 0–100 — règles dans `coeur/saisie.rs`, les deux versions |
| D26 | **l'auteur d'un geste est l'utilisateur de la session**, posé par le serveur sur le fil de la requête (`noyau::auteur`) — plus le premier compte du rôle |
| D25 | **le serveur gagne une fenêtre** : une coque Tauri fine qui affiche la console existante — pas une interface reconstruite |
| D24 | **deux rôles, un seul produit** : simple (monoposte, existe déjà) ou complet (serveur + plusieurs postes/dossiers), choisis **à l'installation**, jamais devinés en cours de route — **révisé le 25/10/2026** : le rôle se demande au premier démarrage de la fenêtre (« garde la base » / « caisse d'un autre poste ») ; le serveur du raccourci s'efface sur une caisse |
| D23 | le **plan comptable SYSCOHADA** en base, les opérations affectées en réglage, les journaux lus et exportés — rien de stocké ; la comptabilité complète après |
| D22 | **SQLite reste** ; plusieurs dossiers demandent **le serveur**, pas PostgreSQL — le serveur sert tout par `Base` sur les deux moteurs, la fenêtre monoposte garde un dossier (révise D11 et D13) — **révisé le 25/10/2026** : la fenêtre n'ouvre jamais de base ; le poste qui garde la base lance son serveur SQLite et s'y branche sur 127.0.0.1 |
| D21 | les **dates de travail** d'un dossier se donnent à sa création et se prolongent ; une écriture hors dates est refusée en disant quoi faire |
| D20 | la v3 se fait dans l'ordre A (pièces simplifiées), B (journal), C (droits), D (dossiers), E (plan comptable) — [PLAN-V3.md](PLAN-V3.md) |
| D19 | cinq permissions de **lecture** (prix d'achat, rapports, soldes, historique, autres caisses) ; `employe` n'en porte aucune |
| D18 | trois signatures au plus par genre de document, libellé + image facultative (cachet) |
| D17 | **l'atelier de modèles part** : un seul générateur, mise en page fixe par genre, réglages en cases ; `modele_document` et `image_document` supprimées — révoque les décisions de l'atelier des 17–18/09 |
| D16 | le bon de livraison partiel reste un document à quantité pleine ; seul le suivi (et le stock) connaît le partiel |
| D15 | un retour rend l'argent en argent et l'avoir en avoir ; ce qui a été payé avec un avoir ne devient jamais des espèces |
| D14 | un chèque rejeté se contre-passe (paiement négatif + sortie de caisse), sans exiger une caisse ouverte |
| D13 | le dossier (v3) se choisit **à la connexion**, mémorisé **par personne** côté serveur ; en changer, c'est se déconnecter ; plusieurs dossiers ~~demandent PostgreSQL~~ **demandent le serveur** (D22, 21/09) — **dormante** : la v3 n'est pas commencée, un seul dossier = comportement d'avant |

Aucune case n'attend de réponse.

---

## D12 — Le serveur tourne en service Windows, installé à part

Jusqu'ici, `gescom-serveur.exe` partait au démarrage de la **session**
de l'utilisateur, posé par l'installeur de la fenêtre (droits d'un
utilisateur, pas d'administrateur). Un poste redémarré sans session
ouverte : pas de serveur, et cinq caisses qui disent « injoignable ».

**Décision : un installeur du serveur à part**, `Gescom-Serveur_x.y.z_
x64-setup.exe`, qui demande les droits administrateur et fait ce que
la fenêtre ne pouvait pas — poser un **service** (`sc create`, démarrage
automatique, relance sur incident), ouvrir le **pare-feu** sur le port,
enregistrer le **certificat**. La configuration (base, port) vit dans
`%ProgramData%\Gescom\serveur.json`, le journal à côté : un service n'a
pas de console. `sc stop` / `sc start` pour l'arrêter et le relancer.

Sans crate d'enveloppe : quatre appels Win32 (`windows-sys`), dans
`serveur/src/service.rs`. L'arrêt est propre — l'écouteur est non
bloquant et regarde un drapeau — et révoque les sessions.

---

## D13 — Le dossier se choisit à la connexion (v3)

Ce que le plan multi-société avait tranché (décision 3), branché le
19/09/2026 : `/connexion` rend la liste des dossiers ouverts ; un seul,
il s'ouvre tout seul ; plusieurs, celui demandé, sinon celui
**mémorisé pour cette personne**, sinon la session s'ouvre **sans
dossier** et seule `choisir_dossier` passe — une fois. Le serveur pose
le dossier de la session sur la `Base` avant chaque commande portée.
Sur une base fichier, les commandes `Connection` ne servent que le
dossier d'origine : un second dossier y est **refusé** (création comme
connexion), pas servi de travers.

---

## D14 — Un chèque rejeté se contre-passe, sans caisse ouverte

Avant : rejeter un chèque **supprimait** le paiement et son mouvement de
caisse, comme s'ils n'avaient jamais existé. Le journal de caisse du
jour de l'encaissement ne bouclait plus, et rien ne disait qu'un chèque
avait été reçu puis refusé.

**Décision (20/09/2026) : un rejet écrit l'inverse, il n'efface pas.**
Un `paiement` négatif (`origine = 'rejet_cheque'`, `annule_paiement_id`
vers l'encaissement) rouvre la créance ; une **sortie de caisse**
`cheque_rejete` du même montant, en moyen `cheque`, défait l'entrée.
L'historique dit les deux.

**Le rejet n'exige pas de caisse ouverte.** La règle « l'argent sort du
tiroir → caisse ouverte » vaut pour les espèces ; un chèque refusé par
la banque n'a jamais été dans le tiroir, et la nouvelle arrive quand
elle arrive — souvent des jours après, à un moment où la caisse est
close. La sortie se rattache à la session ouverte si elle existe, sinon
à celle qui a reçu le chèque, pour que le journal reste lisible.

---

## D15 — L'argent revient en argent, l'avoir en avoir

Un retour ne rend jamais plus que ce que le client a versé (D33). Le
plafond ne comptait que l'argent : les paiements par avoir en étaient
exclus, « pas d'argent reçu ». Vrai — mais un avoir consommé est un
crédit dépensé. Le client qui achetait avec son avoir puis rendait la
marchandise se retrouvait sans rien : ni espèces (jamais versées), ni
avoir (consommé). Le montant partait dans le journal en « non
attribuable ».

**Décision (20/09/2026) : chaque part revient sous sa forme.** La part
payée en argent revient en argent (ou en avoir si le client le
préfère) ; la part payée avec un avoir **revient toujours en avoir**,
même si le client demande un remboursement — un avoir n'a jamais été
de l'argent, on ne le convertit pas au tiroir. Le vendeur voit à
l'écran combien rendre en espèces et combien part en avoir. Sur un
échange, le reliquat demandé en espèces est borné de la même façon.

La règle est pure (`coeur::calcul::repartir_retour`) : espèces,
avoir, non attribuable — ce dernier reste signalé, jamais rendu.

---

## D16 — Le bon de livraison partiel reste un document à quantité pleine

Le suivi d'une livraison (non livré / partiel / livré) est exact et
gouverne le stock (K11) : c'est lui qui répond à « payé, pas livré ».
Le DOCUMENT imprimé, lui, copie toutes les lignes de la commande à
quantité pleine, même quand la livraison réelle est partielle.

**Décision (21/09/2026, à la clôture de la v2) : on laisse tel quel.**
Ce qui compte pour la gestion — le stock, la créance, le statut — est
juste. Le document qui suit le camion resterait à corriger un jour,
mais rien n'y oblige avant que le terrain le demande : un BL partiel
imprimé à quantité pleine se corrige à la main sur la copie papier, ce
qui est déjà la pratique boutique avant Gescom. Reste noté dans la
dette connue, pas dans le chemin critique.

---

## D17 — L'atelier de modèles part

L'atelier de septembre (blocs, glisser-déposer, flottants, export et
import, images posées — 4 700 lignes) a coûté un retour du terrain
par jour depuis le 16/09. Le propriétaire veut **simple** : un en-tête
et un pied qu'on téléverse, des signatures qu'on règle, et une facture
qui sort toujours pareille.

**Décision (21/09/2026) : un seul générateur, une mise en page fixe
par genre, quelques cases à cocher.** L'atelier, `lib/modeles/*`, les
tables `modele_document` et `image_document` et leurs commandes sont
retirés par la v3 (migration qui supprime, rien à reprendre : aucun
modèle en production). Pas de « mode avancé » caché : du code que
personne ne maintient casse au premier changement de TVA sans qu'on
le voie. Les décisions prises pour l'atelier les 17 et 18 septembre
(blocs flottants, images posées, export depuis une caisse) sont
révoquées par celle-ci. D8 (les images voyagent par leur contenu)
reste : c'est ainsi que l'en-tête, le pied et les cachets arrivent au
serveur. → [PLAN-V3.md](PLAN-V3.md) § 2.

## D18 — Trois signatures, avec image

Par genre de document, de zéro à **trois** emplacements de signature,
chacun avec son libellé (« Le client », « Pour la société », « Reçu
par ») et **une image facultative** — le cachet de la société, une
signature scannée — imprimée au-dessus du trait. C'est ce que les
« images posées » de l'atelier servaient vraiment à faire. Stockées
en JSON dans `parametres_societe.signatures`. (21/09/2026)

## D19 — Cinq permissions de lecture, et l'employé ne voit pas les prix d'achat

Les lectures n'étaient pas filtrées, et c'était une décision. Elle
change, **sans** créer une permission `:lire` par commande : cinq
seulement, celles qu'un patron veut vraiment cacher — `achats:lire_prix`
(prix d'achat, marges, valeur du stock), `rapports:lire`,
`tiers:lire_solde`, `journal:lire`, `caisse:lire_autres`. Sans la
permission, la commande rend la donnée **sans le champ** (`null`,
jamais un zéro qui ressemble à une valeur) ou refuse. Le rôle
`employe` — celui des vendeurs, le plus répandu — **n'en porte
aucune** ; le patron rend un droit à une personne par le sur-mesure.
(21/09/2026) → [PLAN-V3.md](PLAN-V3.md) § 4.

## D20 — L'ordre de la v3 : A, B, C, D, E

Pièces simplifiées d'abord (ce qui se voit tous les jours), le journal
ensuite (pour corriger le reste avec des traces), les droits, puis les
dossiers (un seul commerce tourne), puis le plan comptable, qui lit
tout ce que les autres écrivent. (21/09/2026)

## D21 — Les dates de travail se donnent à la création du dossier

À la création, le dossier posait d'office l'année civile comme
premier exercice. Désormais on **donne** les dates de travail
(proposées à l'année civile), et elles se **prolongent** — la commande
existe. Une écriture hors dates est refusée par le garde-fou, avec le
remède dans le message (prolonger, ou ouvrir l'exercice suivant). Un
dossier n'a jamais de trou entre deux exercices. (21/09/2026) →
[PLAN-V3.md](PLAN-V3.md) § 5.

## D22 — SQLite reste ; plusieurs dossiers demandent le serveur, pas PostgreSQL

Un commerçant seul, une machine, n'a pas à installer PostgreSQL. Les
deux moteurs restent de plein droit. Ce que le multi-dossier demande,
c'est **le serveur** — qui peut tourner sur la même machine, sur la
même base SQLite — parce que la fenêtre monoposte parle à la base par
ses façades `Connection`, qui ne connaissent qu'un dossier. Le serveur,
lui, sert **tout par `Base`** sur les deux moteurs : les 202 commandes
y sont portées et les scénarios `*_base.rs` tournent sur SQLite par
défaut — le chemin le plus testé. Son chemin `conn` sur SQLite (D11)
part ; D13 est révisée sur un mot. Le déclencheur de stock SQLite
multi-dossier se corrige avant. (21/09/2026)

## D23 — Le plan comptable comme fondation : SYSCOHADA en base, journaux lus, rien de stocké

Le Mali est dans l'OHADA. La v3 pose le plan SYSCOHADA révisé en base
(commun, sous-comptes par dossier), l'affectation de chaque type
d'opération de Gescom à ses comptes (réglage, défauts livrés), et les
journaux — ventes, achats, caisse, règlements — **générés à la
lecture** et exportés en CSV pour le comptable. **Aucune écriture
n'est stockée** : ce serait une seconde vérité à côté de la vente,
et les deux finiraient par se contredire. Balance, bilan, OD,
lettrage, TVA : après, sur ce sol, sans rien refaire. (21/09/2026)

## D24 — Deux rôles, un seul produit, un choix qui ne se devine jamais

Le propriétaire vend à deux profils : le commerçant seul, une machine,
qui ne branchera jamais de deuxième caisse ; et celui qui en a
plusieurs, ou plusieurs boutiques. Aujourd'hui ce sont deux produits
distincts dans le discours (« v1 ou v2 ») — **v3 doit jouer les deux
rôles dans le même produit**.

**Ce qui existe déjà, vérifié dans le code** : le réglage `monoposte` /
`poste` ([reseau.rs](../../src-tauri/src/reseau.rs)), persisté dans
`poste.json`, lu une fois, jamais deviné — « une installation
existante qui se met à jour ne change pas de mode toute seule un
matin ». En `monoposte` la fenêtre ouvre sa base SQLite locale, aucun
réseau ; en `poste` elle ne parle qu'au serveur, sans repli silencieux
(la correction de septembre 2026 sur le bug du cache vidé). Les deux
rôles jouent donc déjà dans le même exécutable.

**Ce que D24 ajoute** : le choix se fait **à l'installation** — un
écran « Une seule caisse » / « Plusieurs caisses ou boutiques » qui
écrit `poste.json` une fois pour toutes — plutôt qu'un réglage
découvert après coup dans Paramètres → Réseau. Changer de rôle plus
tard reste possible, mais c'est un geste volontaire, comme migrer une
base (PLAN-MULTISOCIETE), jamais une bascule automatique. Le rôle
« complet » embarque le multi-dossier (D13/D22, chantier D) ; le rôle
« simple » reste à un seul dossier, invisible à l'écran.

**Ce qu'on ne fait pas** : reconstruire l'architecture. Le mécanisme
existe et est éprouvé ; D24 le rend explicite au bon moment, sans rien
casser de ce qui tourne. (21/09/2026) → [PLAN-V3.md](PLAN-V3.md) § 6.

## D25 — Le serveur gagne une fenêtre

Le serveur n'a aujourd'hui qu'une console web
([console.rs](../../src-tauri/serveur/src/console.rs), déjà construite
et testée) et le service Windows invisible (D12). Pour le rôle
« complet » d'un produit qu'on vend, quelqu'un doit pouvoir double-
cliquer une icône et voir un tableau de bord — pas ouvrir un navigateur
sur `localhost:7300`, pas taper `sc start`.

**Décision : une coque, pas une reconstruction.** Une application Tauri
fine qui affiche la **même page** que la console web (le HTML ne
change pas, le format des données non plus) — pour que rien ne soit
maintenu en double. Le service Windows (D12) reste disponible pour qui
préfère l'invisible-au-démarrage ; la fenêtre est la façon normale d'y
toucher. (21/09/2026) → [PLAN-V3.md](PLAN-V3.md) § 6.

## D26 — L'auteur d'un geste est l'utilisateur de la session

**Le défaut (revue du 23/09/2026).** Le serveur connaissait
l'utilisateur de la session (`Appelant.utilisateur_id`) mais ne passait
au noyau que son **rôle** ; le noyau retrouvait l'auteur par
`SELECT … WHERE r.nom = ? LIMIT 1` (`id_utilisateur_par_role*`), ou
pire par « le premier compte actif » (`id_utilisateur_courant*`). Deux
caissiers du même rôle : toutes les ventes, remises, règlements et
ouvertures de caisse signés par le premier. L'écart de clôture et
l'antidatage n'étaient imputables à personne — c'est tout ce que D46
(v1) et `pieces:antidater` cherchent à empêcher.

**Décision.** Le serveur pose l'utilisateur de la session sur le fil
de la requête (`noyau::auteur::poser`, une garde qui le retire en
tombant) avant chaque commande — `/rpc`, `/sauvegarde`, `/entretien`.
Les cinq aides d'auteur (`argent::id_utilisateur_par_role[_sur]`,
`argent::id_utilisateur_courant[_pub|_sur]`, `comptoir::auteur_courant`)
le lisent **en premier**. Une centaine d'appels corrigés sans toucher
une signature : le serveur sert chaque requête sur son propre fil.

**Ce qu'on ne fait pas** : passer l'identifiant en argument à ~200
fonctions (deux versions chacune) dans ce correctif. À faire quand une
signature bouge de toute façon (v3, chantier C — droits) ; le rôle ne
sert alors plus qu'au repli. Hors serveur (fenêtre monoposte v1,
tâches du serveur sans session), rien n'est posé : l'ancien repli
s'applique, et la v1 ne bouge plus.

Garde : `revue_v2_base::deux_caissiers_du_meme_role_signent_chacun_leur_vente`,
et `serveur/tests/routes.rs::deux_comptes_du_meme_role_signent_chacun_leur_vente`
qui échoue si on retire la garde d'`api.rs`.

## D27 — Le serveur juge la saisie, pas l'écran

**Le défaut (revue du 23/09/2026).** Trois gestes n'étaient gardés que
par l'écran, alors qu'une commande du serveur s'appelle sans lui :

- `regler_creance` en mode `"avoir"` : la créance passait « payée »
  **sans qu'aucun avoir soit consommé**, ni argent en caisse ;
- `enregistrer_paiement` (plus appelée par l'écran, encore servie avec
  `paiements:creer`) : montant **négatif** accepté — une « entrée » de
  caisse négative —, aucun plafond au reste, `peut_regler` ignoré ;
- `creer_vente` à **quantité négative** : le stock remontait, la vente
  passait « payée », sans retour ni avoir. Même trou dans la réception
  (`enregistrer_achat`) et les pièces (`creer_piece*`, `modifier_piece`) ;
- `regler_dette_fournisseur` : montant négatif accepté — une sortie de
  caisse négative, de l'argent qui « apparaît » dans le tiroir.

**Décision.** Les règles sont dans
[coeur/saisie.rs](../src-tauri/noyau/src/coeur/saisie.rs), pures et
testées, et appelées par les deux versions :
`verifier_mode_encaissement` (`especes`, `orange_money`, `moov_money`,
`cheque`, `virement` — **`avoir` n'est pas un mode** : un avoir se
consomme, il ne se déclare pas), `verifier_montant` (> 0),
`verifier_ligne` (quantité et facteur > 0 et finis, prix ≥ 0),
`verifier_remise_pct` (0 à 100). `enregistrer_paiement[_sur_base]`
devient une enveloppe de `regler_creance_datee[_sur_base]` : une seule
porte d'encaissement, une seule série de gardes. Le sens du stock est
porté par le **geste** (vente, retour, réception), jamais par le signe
d'une quantité. Une pièce sans ligne reste permise (avoir accordé, K7).

Au passage, `regler_creance_datee` (version `Connection`, celle du
serveur sur fichier SQLite) écrit désormais paiement, statut, pièce,
caisse et journal **dans une transaction** (règle 4) ; l'écriture en
caisse n'est plus ignorée en cas d'échec.

## D22 et D24 révisés (25/10/2026) — la fenêtre ne garde jamais de base ; le rôle du poste se demande

**Ce qui était faux.** D22 et D24 (21/09) affirmaient, « vérifié dans le
code », qu'en `monoposte` la fenêtre ouvre sa base SQLite locale par ses
façades `Connection`. Depuis le 11/09 (commit `1da32c3`, « le client ne
parle qu'au serveur ») ce n'était plus vrai : en `monoposte`, la fenêtre
affichait « Brancher ce poste » et `appeler` refusait sans serveur. Toute
la v3 a ensuite été construite côté serveur : **73 commandes** (réglages
des documents, Historique, droits, dossiers, exercices, plan comptable,
paie, suivi client) n'existent pas dans la fenêtre seule. Constaté par le
banc d'écran le 25/10.

**Décision du propriétaire** (25/10) : « je veux seulement que v3
travaille en mono et multiposte ». Retenu, sur proposition de
l'assistant :

1. **La fenêtre ne parle jamais qu'au serveur**, dans les deux rôles —
   un seul chemin, celui qui a fermé la panne du cache vidé.
2. **`monoposte` = ce poste garde la base** (une seule caisse, ou le
   poste principal) : la fenêtre se branche toute seule sur
   `127.0.0.1:7300` ; s'il ne répond pas, elle lance le
   `gescom-serveur.exe` livré à côté d'elle (`reseau.rs`,
   `demarrer_serveur_local`), mêmes réglages que le raccourci, sans
   fenêtre noire, et ne l'arrête pas en se fermant (la sauvegarde
   automatique et Gescom Équipe en ont besoin). Rien à saisir.
3. S'il ne démarre pas : un écran qui le dit, avec la raison,
   « Réessayer » et « Ce poste est une caisse d'un autre poste » —
   jamais un écran de connexion qui échouerait.
4. **`poste` = la caisse d'un autre poste** : inchangé (adresse saisie,
   essayée avant d'être enregistrée). Une caisse ne lance jamais de
   serveur.
5. **Le rôle se demande au premier démarrage** (`poste.json` neuf ou
   illisible → `a_choisir`) : deviner « monoposte » faisait d'une caisse
   fraîchement installée un poste principal avec une base vide. Une
   installation existante garde son mode. C'est l'écran que D24
   prévoyait, fait dans la fenêtre (l'installeur n'a pas les droits
   administrateur).
6. **Le serveur du raccourci de démarrage s'efface sur une caisse**
   (`serveur/src/role_poste.rs`) : lancé sans `--base`, il lit
   `poste.json` et s'arrête si ce poste est la caisse d'une **autre**
   machine. Dans le doute il tourne : `poste` sur `127.0.0.1` (le
   réglage du poste principal entre le 11/09 et le 25/10), une adresse
   de cette machine, un nom qu'on ne sait pas résoudre. Le service
   Windows (D12), les tests et le banc passent `--base`.
7. **Changer de rôle reste un geste volontaire** : Paramètres → Réseau
   (« Ce poste garde la base », avec confirmation : les ventes du poste
   principal ne s'y trouvent pas ; « En faire une caisse d'un autre
   poste »), et le lien de retour sur l'écran de branchement.
8. En build de débogage, la fenêtre ne lance de serveur que si
   `GESCOM_DONNEES` est posé (et alors sur ce dossier) : un essai
   n'ouvre jamais la base du propriétaire.

**Ce qui reste.** La fenêtre ouvre encore un `gescom.db` local à son
démarrage (`lib.rs`) — le même fichier que la base par défaut du serveur,
sans danger en WAL — et ses façades `Connection` ne servent plus à
l'écran : leur retrait est le grand nettoyage annoncé dans ALERTES.md.

## D35 — Chaque vente sans nom a sa fiche « passage » (25/10/2026)

**Le constat.** Toutes les ventes sans nom allaient au client unique
« Comptant » (`est_generique = 1`). Le serveur lui refusait le crédit
(D40) et l'avoir, mais **pas une vente « comptant » payée en partie ou
pas du tout** (`argent.rs`, `a_encaisser = min(montant_paye, …)`), ni
une facture à crédit validée depuis Pièces (`valider_facture`) : la
marchandise sortait, rien n'entrait, et la dette, au nom de
« Comptant », n'apparaissait dans aucune relance ni aucun rapport (ils
filtrent `est_generique = 0`). L'écran envoyait toujours le total — mais
le serveur juge la saisie (D27), l'écran ne protège rien.

**La proposition du propriétaire** : un code client de passage
incrémenté, traité comme un client ordinaire, crédit bloqué, et un
passage vers client ordinaire. Retenue avec ces garde-fous (proposés
par l'assistant, acceptés le 25/10/2026) :

1. **Une fiche par vente sans nom.** Quand une vente ou une facture
   arrive au « Comptant », le serveur crée dans la MÊME transaction une
   fiche « Client de passage », code `PAS-000123` (préfixe du dossier
   comme les autres codes), numéro réservé au compteur
   (`compteur_piece`, clé `PAS`) — pas de doublon à deux caisses.
   La fiche est `est_generique = 1`, `statut = 'passage'`.
2. **Hors des listes et des chiffres.** `est_generique = 1` la fait
   sortir d'office de la liste des clients, des relances, des rapports
   de créances et de « meilleurs clients » : ces lectures filtrent déjà
   `est_generique = 0`, aucune à oublier. Un filtre « Passages »
   (Clients) la retrouve par son code ou le numéro de sa facture.
3. **Payée en entier à la vente.** Règle pure
   `coeur::saisie::verifier_vente_passage` : pour un client de passage
   (le « Comptant » ou une fiche passage), pas de crédit, et
   l'encaissement couvre le total — sinon refus AVANT d'écrire, dans
   les deux versions de `creer_vente` et de `valider_facture`. Les
   retours restent remboursement ou échange (déjà en place : la fiche
   est `est_generique`).
4. **« Passer en client ordinaire »** (`passage::promouvoir_sur`,
   permission `clients:modifier`) : nom obligatoire, téléphone
   facultatif ; la fiche devient `est_generique = 0`,
   `statut = 'client'`, garde son code et ses achats ; geste tracé au
   journal (Historique). Une seule porte : `modifier_client` refuse
   toujours une fiche générique.
5. **Une facture garde le nom écrit à son émission.** Une pièce ne
   portait que `tiers_id` : renommer un client changeait le nom des
   factures déjà émises à la réimpression (OHADA). Colonne
   `piece_commerciale.tiers_nom` : avant tout changement de nom
   (promotion, `modifier_client`), le nom en cours est figé sur les
   pièces de ce client qui n'en ont pas encore ; la lecture d'une pièce
   à imprimer rend `COALESCE(tiers_nom, nom du client)`. Figer au
   renommage revient à figer à l'émission : jusque-là, les deux noms
   sont les mêmes.
6. **Les ventes passées ne bougent pas.** Ce qui est au « Comptant » y
   reste ; le « Comptant » reste la porte de l'écran (« client de
   passage » au comptoir, rien à saisir) — c'est le serveur qui
   remplace, vente par vente, par une fiche neuve.
   `lire_client_generique` rend toujours le « Comptant », jamais une
   fiche passage.
7. **Doublons** : une même personne passée trois fois a trois fiches ;
   rattacher ses autres tickets à la promotion viendra plus tard (noté,
   pas fait).

**Mise en œuvre.** `coeur/saisie.rs` (la règle, testée) ; nouveau
module `noyau/src/passage.rs` (créer la fiche — `&mut impl Acces` côté
`Base`, une version `rusqlite` pour les façades `Connection` —, lire
les passages, promouvoir, figer les noms) ; `creer_vente` et
`valider_facture`, deux versions chacune ; `lire_client_generique`,
`modifier_client` et `lire_donnees_piece`, deux versions ; colonne
`tiers_nom` dans `schema.sql`, `amorcage.rs` et `persistance/v2.rs` ;
serveur : `lire_passages`, `promouvoir_passage` ; écran Clients :
filtre « Passages » et « Passer en client ordinaire ». `lire_passages`
et `promouvoir_passage` n'ont que leur version `Base` : la fenêtre ne
passe plus par les façades `Connection` (D22 révisé), comme les
commandes de la v3. Tests : `noyau/tests/passage_base.rs` sur SQLite
et PostgreSQL (stock, caisse, créance après chaque geste, refus sans
écriture, deux ventes deux codes, promotion, nom figé, le détecteur),
et un parcours du banc.

## D37 — Le menu se règle par dossier (26/10/2026)

**Le besoin** (le propriétaire) : un petit commerçant n'a que faire de
Relances, Historique, Chèques… et se perd dans quinze entrées. Un onglet
de Paramètres pour régler ce que le menu montre.

**Retenu** (proposé par l'assistant, accepté le 26/10/2026) :

1. **Paramètres → Menu** : une case par entrée du menu, montrer ou
   cacher. Deux réglages tout faits : **« Boutique simple »** (Tableau de
   bord, Ventes, Achats, Stock, Clients, Caisse, Rapports) et
   **« Complet »** (tout).
2. **Toujours visibles, sans case** : Tableau de bord, Ventes, Caisse,
   Paramètres — sans Caisse on n'encaisse plus, sans Paramètres on ne
   réaffiche plus rien. La règle est pure (`coeur::menu`) : le serveur
   refuse de les cacher, quel que soit l'écran.
3. **Cacher n'est qu'un affichage.** Aucune donnée, aucune règle, aucune
   trace ne change : l'Historique continue d'enregistrer chaque geste
   même caché ; les créances restent suivies dans Clients si Relances
   est caché ; payer par chèque reste possible si Chèques est caché.
   Les liens d'un écran vers un autre (fiche client → Historique)
   restent valables. La palette (Ctrl K) suit le même réglage.
4. **Par dossier** : toutes les caisses d'un dossier voient le même
   menu ; chaque dossier a le sien (`config_app`, clé
   `menu_caches:<dossier>`).
5. **Le patron seul** (`parametres:modifier`) ; le changement est tracé
   (journal `menu_modifie`, l'Historique le dit).
6. **Les droits restent au-dessus** : une entrée paraît si la personne a
   le droit ET si elle n'est pas cachée.

**Mise en œuvre.** `coeur/menu.rs` (entrées, toujours visibles,
« Boutique simple », la validation, testés) ; `noyau/src/menu.rs`
(`lire_menu_sur`, `definir_menu_sur`, version `Base` seule comme les
réglages de la v3) ; serveur `lire_menu`, `definir_menu` ; écran
`lib/menu.ts`, `Layout` (menu et palette), onglet `OngletMenu`. Tests :
`noyau/tests/menu_base.rs` sur les deux moteurs, un parcours du banc.

## D36 — Un article peut ne pas suivre son stock (26/10/2026)

**Le besoin** (le propriétaire) : sachets, crédit de recharge,
livraison, coupe, réparation, vrac qu'on ne compte jamais. Aujourd'hui
leurs ventes font passer le stock en négatif, déclenchent des alertes et
encombrent l'écran Stock. La colonne `article.gere_en_stock` existait,
lue par les alertes et la valeur du stock, mais aucun écran ne la
réglait : elle valait toujours 1.

**Retenu** (proposé par l'assistant, accepté le 26/10/2026) :

1. **Une case « Suivre le stock »** (Paramètres → Articles), cochée par
   défaut ; la colonne existante, rien d'ajouté à la base.
2. **Un article non suivi se vend normalement** — chiffre d'affaires,
   caisse, factures inchangés — et n'est **jamais à découvert** : le
   drapeau `vente_a_decouvert` reste à 0 (deux versions de `creer_vente`
   et de `valider_facture`), la caisse n'annonce ni rupture ni découvert.
3. **Hors des écrans de stock** : écran Stock (filtre « Non suivis »
   pour le retrouver), inventaire (ajustement refusé), alertes de
   rupture et valeur du stock (déjà), **transfert refusé** — il n'y a
   rien à déplacer.
4. **Les mouvements s'écrivent quand même** (le stock reste la somme des
   mouvements) : on sait combien de sachets sont sortis ce mois-ci, et
   les trente endroits qui écrivent un mouvement ne bougent pas.
5. **Changer la case** (`changer_suivi_stock`, `parametres:modifier`,
   journal `suivi_stock_change`) :
   - **cesser de suivre** un article qui a du stock est permis ; la
     réponse dit combien d'unités sortent ainsi du stock et de sa
     valeur, l'écran le demande avant, l'Historique le note ;
   - **reprendre le suivi** exige un **comptage pour chaque magasin
     actif** du dossier (`coeur::stock::verifier_comptages`) : le stock
     repart de la quantité réelle (mouvement `ajustement`, motif « Reprise
     du suivi du stock »), jamais d'un chiffre faux. Tout dans une
     transaction.
6. **Achats et retours** inchangés : un achat se paie et se trace, un
   retour rembourse ou échange ; leurs mouvements s'écrivent sans se
   voir.

**Le risque nommé** : marquer « non suivi » un article qu'on vend
beaucoup pour ne plus voir ses manques. Garde-fous : la permission
réservée et la trace dans l'Historique.

**Mise en œuvre.** `coeur::stock::verifier_comptages` (testée) ;
`noyau/src/suivi_stock.rs` (`changer_suivi_stock_sur`, `est_suivi_sur` /
`est_suivi`) ; le découvert dans `creer_vente` et `valider_facture`, le
refus dans `enregistrer_transfert` et `enregistrer_ajustement_inventaire`
(deux versions chacun) ; `lire_stocks_pagines` (filtre `nonSuivis`),
`lire_articles_avec_unites` et `lire_articles_complets` disent
`gere_en_stock` ; écrans Articles, Stock, Ventes. Tests :
`noyau/tests/suivi_stock_base.rs` sur les deux moteurs, un parcours du
banc.

## D38 — Des exercices comme dans Ciel (27/10/2026)

**Le besoin** (le propriétaire) : une boutique dont l'année va de
juillet à juin ; une autre qui veut prolonger de plus de deux ans pour ne
pas y repenser ; et pouvoir, comme dans Ciel, arrêter l'exercice en
cours en plein milieu de l'année pour repartir sur un autre cycle. « On
suit la logique de Ciel si elle n'altère pas la nôtre. »

**Retenu** (proposé par l'assistant, accepté le 27/10/2026) :

1. **Dates libres** : un exercice commence n'importe quel jour (à la
   création du dossier ; le suivant, le lendemain du précédent).
2. **Plusieurs années, cinq au plus** (`coeur::fin_exercice::DUREE_MAX_ANS`) :
   une prolongation va au plus 5 ans après la fin prévue, un exercice
   ouvert ou créé dure au plus 5 ans. Une prolongation ne se raccourcit
   pas : une faute d'année (2062 pour 2026) bloquerait le dossier des
   dizaines d'années. L'écran propose +1/+2/+3 ans et dit l'écart en clair.
3. **Couper** (`couper_exercice`, `dossiers:gerer`) : le dernier
   exercice ouvert s'arrête au jour choisi, le suivant s'ouvre le
   lendemain dans la même transaction, **au moins jusqu'à l'ancienne
   fin** (par défaut un an). Journal `exercice_coupe` (avec l'ancienne
   fin) et `exercice_ouvert`.

4. **Clore, seulement un exercice terminé, et dans l'ordre**
   (`juger_cloture`, ajouté le même jour — « on force l'utilisateur à
   éviter les incohérences ») : sa fin, prolongation comprise, est
   passée à l'horloge du serveur, et aucun exercice plus ancien n'est
   encore ouvert. Clore celui qu'on vit arrêtait toutes les ventes
   sur-le-champ, sans remède (le suivant ne commence qu'après sa fin) ;
   pour s'arrêter plus tôt : couper, puis clore la partie terminée.

**Ce qui ne change pas de la nôtre** : D21 — pas de trou, refus hors
dates en disant quoi faire ; un exercice clos ne se rouvre, ne se
prolonge ni ne se coupe ; on ne coupe pas un exercice qui a déjà un
suivant (trou ou chevauchement) ; rien de ce qui s'écrivait la veille
n'est refusé le lendemain. Aucune écriture ne porte son exercice (elles
s'y rangent par leur date) : couper ne déplace aucune donnée, les
rapports vont par dates.

**Ce qu'on ne prend pas** : rouvrir un exercice clos ; raccourcir sans
ouvrir la suite. **Le risque nommé** : la numérotation suit l'année
civile (`FAC-2026-…`), pas l'exercice — un exercice juillet → juin
change de série au 1er janvier. La suite reste continue et
chronologique ; numéroter par exercice serait un autre chantier.

**Mise en œuvre.** `dossiers::juger_coupe` (pure, testée),
`couper_exercice_sur`, plafond dans `dates_de_travail` et
`prolonger_exercice_sur` ; écran `ExercicesDossier.tsx` (bouton Couper
sur le dernier exercice, phrase en clair). Tests :
`noyau/tests/exercice_coupe_base.rs` et `fin_exercice_base.rs` sur les
deux moteurs et sous le compte limité.

## D39 — Le serveur écoute de 40000 à 40100 (27/10/2026)

**Le besoin.** La fenêtre parle au serveur par `fetch`, mais sa CSP
(`connect-src`) ne l'y autorisait pas : en build livré, risque de ne
joindre personne (ALERTES, lot A). Ouvrir tout `http:` a été refusé par
le contrôle de sécurité ; le propriétaire a d'abord choisi le seul port
7300, puis : « prends un autre port dans une plage de 40000 à 40100 ».

**Retenu** (choix du propriétaire, 27/10/2026) :

1. **40000 par défaut**, plage **40000–40100** (`protocole::PORT_DEFAUT`,
   `PORT_MAX`, `port_dans_la_plage`, `adresse_joignable`). **7300 est
   abandonné** : rien n'était livré.
2. **La CSP des deux fenêtres énumère les 101 ports**, et rien d'autre
   (une CSP ne connaît pas les plages). `noyau/tests/plage_de_ports.rs`
   vérifie que les deux `tauri.conf.json` et la copie de Gescom Équipe
   suivent `protocole`.
3. **Repli sur le poste qui garde la base** : si 40000 est pris par un
   autre programme, le serveur local prend le suivant libre de la plage,
   et `poste.json` le retient (`reseau::demarrer_serveur_local`). Un
   serveur Gescom qui répond déjà sur la machine est repris tel quel —
   jamais deux serveurs sur la même base. On ne sonde pas toute la plage
   d'avance : sous Windows, une connexion refusée sur 127.0.0.1 peut
   prendre deux secondes.
4. **Refuser plutôt que faire semblant** : la fenêtre refuse d'enregistrer
   une adresse de caisse hors de la plage ; une caisse encore réglée en
   7300 redemande son rôle une fois ; l'installeur du serveur refuse un
   port hors de la plage ; le serveur lancé hors de la plage tourne mais
   l'écrit en avertissement au journal technique (essais, console).
5. **Le pare-feu ouvre toute la plage** (`outils/parefeu.ps1`,
   l'installeur) : un poste principal rabattu sur 40001 reste joignable
   par les caisses.

**Pas pris** : `http:` / `https:` sans port (refusé) ; un port libre au
hasard (la CSP est figée au build).

## D40 — La fenêtre « Gescom Serveur », son panneau Réseau, la recherche automatique (27/10/2026)

**Le besoin** (le propriétaire) : « une interface graphique dédiée à mon
serveur, comme sur Ciel », pour l'IP et le port ; puis, ayant appris que
Ciel laisse le client désigner le serveur par son nom, la recherche
automatique (« Rechercher le serveur »).

**Retenu** (proposé, accepté le 27/10/2026) :

1. **La coque de D25, enfin construite** : `src-tauri/serveur-fenetre/`
   (`ml.gescom.serveur`), sans base ni noyau. Elle lit le port dans
   `serveur.json` (sinon 40000), affiche la console servie par le
   serveur — la même page que dans le navigateur, rien en double — et,
   s'il ne répond pas, une page locale « Le serveur ne tourne pas » avec
   « Démarrer le service » (accord administrateur) et « Réessayer ».
   L'installeur du serveur la pose au menu Démarrer.
2. **Le panneau Réseau dans la console** : nom du poste, cartes et IP
   (fixe ou donnée par la box, qui avertit et conseille le nom), port,
   écoute, ce qu'on tape sur une caisse (le nom d'abord), pare-feu TCP
   et UDP, postes connectés. **Changer** le port (40000–40100) ou
   l'écoute : `parametres:modifier`, **seulement depuis le poste du
   serveur** (127.0.0.1 / ::1), jamais sur un serveur lancé avec
   `--port`/`--hote` (le monoposte de D39 : géré par Gescom). Règle pure
   `coeur/reseau_serveur.rs`.
3. **Le serveur applique lui-même** : il essaie le nouveau port, réécrit
   `serveur.json` en gardant le reste (fichier temporaire puis renommage),
   écrit `reseau_serveur_modifie` à l'Historique (tout ou rien), répond,
   puis redémarre — en service, en sortant en échec pour la relance de
   D12 ; lancé à la main, en se relançant avec les mêmes arguments.
4. **La recherche automatique** : le serveur répond en UDP sur 40000
   (`decouverte::PORT`, même quand son TCP est ailleurs dans la plage) à
   `{"q":"gescom","v":1}`, venu du réseau local, 20 fois par seconde au
   plus : « Gescom Serveur », le nom du poste, ses IPv4, son port, les
   versions — **rien de la boutique**. Côté caisse, `rechercher_serveurs`
   (diffusion, /24 du poste, 127.0.0.1 ; 2 s) et le bouton « Rechercher
   le serveur » (branchement, Paramètres → Réseau) : un clic remplit
   l'adresse, de préférence le nom du poste s'il mène bien à ce serveur.
   Le pare-feu ouvre UDP 40000 (Domaine et Privé).

**Précisions prises au passage** (le plus prudent) : un serveur lancé à
la main sans `--port`/`--hote` lit `port` et `hote` dans `serveur.json`
(sinon il reprendrait l'ancien port après un changement) ; en service,
`--hote` passe devant la clé du fichier ; un build de débogage n'écrit
`serveur.json` que si `GESCOM_SERVEUR_JSON` le désigne (les tests) ; la
fenêtre de caisse monoposte vérifie aussi le port du service de son poste
avant de lancer un serveur (jamais deux serveurs sur deux bases).

**Le risque nommé** : changer le port coupe les caisses jusqu'à ce
qu'elles retrouvent le serveur — l'écran le dit avant, avec le nombre de
postes connectés et « Rechercher le serveur ». **Pas vu en vrai** : la
fenêtre, l'accord administrateur, la relance par Windows, la diffusion
entre deux machines (TESTS-MANUELS, section K).

## D41 — La paie suit l'emploi et fige ses conditions (28/10/2026)

**Le constat** (relecture demandée par le propriétaire, 10 défauts
confirmés par des tests) : la paie ignorait les dates d'entrée et de
départ, comptait le prorata sur les jours *marqués*, laissait un
gestionnaire sans droit de paie effacer un salaire, reprenait le salaire
d'aujourd'hui dans une rectificative, perdait les jours marqués après une
validation, oubliait le trop-versé, commissionnait une vente rendue,
payait un salaire entier sur une quinzaine, montrait les montants dans
l'Historique ; et « 15.000 » tapé à l'écran faisait 15 F.

**Retenu** (quatre choix du propriétaire, les autres évidents) :

1. **L'emploi borne tout** : « à préparer » = employé au moins un jour de
   la période ; préparer hors de l'emploi est refusé ; le calcul se borne
   aux jours d'emploi, avec ou sans prorata ; un départ ne se date pas
   avant des jours déjà marqués ; un compte de vente repris ne commissionne
   le nouveau qu'à partir du lendemain du départ de l'ancien ; un retour
   après interruption devient la nouvelle entrée.
2. **Prorata** (choix : *un jour non marqué compte présent*) : salaire ×
   (jours d'emploi − absences − ½ par demi-journée) ÷ jours du mois, **en
   entiers** — plus aucun flottant sur l'argent de la paie.
3. **Le salaire au mois se paie sur un mois civil** ; une autre période
   est refusée (un journalier, lui, se paie à la quinzaine).
4. **La fiche fige ses conditions** (`fiche_paie.conditions` : salaire,
   tarif, commission, déclaré, cotisations) ; la rectificative les reprend
   (D31 : « ne change pas si on corrige un tarif le mois suivant ») ; une
   fiche plus ancienne se relit sur ses lignes.
5. **Valider avant la fin du mois est permis, puis les jours sont figés**
   (choix) : marquer un jour d'une fiche validée est refusé, sauf pendant
   une rectificative en brouillon.
6. **Le trop-versé devient une avance** (choix), créée dans la validation
   de la rectificative et retenue d'office (D32 ; `avance.trop_verse_fiche_id`).
7. **La commission se calcule nette des retours** (choix), à leur date ;
   l'échange ne se déduit pas (comme le CA, lot B) ; jamais négative.
8. **Sans droit de paie**, la modification d'une fiche du personnel garde
   la rémunération ; les réponses et l'Historique masquent les montants.
9. **« 15.000 », « 15 000 », « 15,000 » valent 15 000 F** (`lib/montant.ts`,
   `MoneyInput`) ; le serveur borne les saisies (10⁹ F, 100 000 unités).

**Questions laissées ouvertes** (des décisions, pas des défauts) : la paie
et l'avance hors caisse (D31/D32 révisés) quand l'argent sort du tiroir ;
un maximum de retenue d'avance par mois ; l'avance datée du jour de la
saisie ; la commission sur des ventes à crédit jamais payées ; un barème
progressif (ITS) que D33 promet et que le code n'a pas. **Tranchées le
28/10/2026 : D42.**

## D42 — Le tiroir, l'avance étalée et datée, la commission sur l'encaissé (28/10/2026)

**Le besoin** : les cinq questions que D41 laissait ouvertes. Le
propriétaire les a tranchées le 28/10/2026, chacune dans le sens proposé.

**Retenu** :

1. **« Pris dans le tiroir »** (précise D31/D32 sans les renverser) : une
   avance ou un versement de salaire en espèces porte une case « pris dans
   le tiroir ». Cochée, la sortie de caisse s'écrit dans la même
   transaction (`mouvement_caisse_id`, colonne qui existait déjà), sur la
   caisse ouverte du dossier — fermée, refus `CAISSE_FERMEE` avant
   d'écrire (règle 4). Non cochée, hors caisse comme depuis le 24/09
   (Orange Money, Moov, virement, espèces du patron). La dépense de caisse
   « Salaire / main d'œuvre » ne sert plus pour le personnel suivi dans
   Équipe : le salaire serait compté deux fois (compte 661, paie et
   dépense).
2. **L'avance s'étale à la saisie** : « tout sur la prochaine paie » (par
   défaut, la règle d'avant) ou « X F par mois » — l'accord passé avec
   l'employé, écrit et imprimé. La fiche retient au plus la mensualité, et
   jamais plus que le net (le reste se reporte, comme avant). Pas de
   plafond de dossier.
3. **L'avance a une date choisie, bornée** : aujourd'hui par défaut ; une
   date passée est permise, jamais future, jamais avant l'entrée, au plus
   31 jours en arrière. Si la paie de cette période est déjà validée,
   l'écran dit qu'elle sera retenue sur la suivante. L'Historique garde le
   jour de la saisie.
4. **La commission se calcule sur l'encaissé** (remplace D41 point 7) :
   elle suit l'argent reçu sur les ventes du vendeur — une vente comptant
   compte le jour même, une vente à crédit au fil des règlements, dans la
   période où ils arrivent — moins l'argent rendu sur ses ventes, à la
   date du retour. Une dette éteinte par un retour ne compte pas : rien
   n'avait été encaissé.
5. **L'ITS se saisit à la main** : pas de barème ; le comptable le porte en
   retenue sur la fiche. D33 est corrigé : une cotisation est un taux,
   plafonné ou non — « ou un barème » n'existe pas. À revoir quand un
   comptable le demande, avec une source officielle à jour.

**Les risques nommés** : la commission sur l'encaissé ne paie rien au
vendeur pour un règlement arrivé après son départ (l'écran le dit) ; à la
bascule, une vente à crédit déjà commissionnée sur le vendu (fiche validée
avant D42) ne doit pas l'être une seconde fois à son règlement — la règle
de la fiche est figée dans ses conditions (D41 point 4).

**Mise en œuvre** (28/10/2026, plan accepté avec ses huit choix de
détail) : `avances::donner_avance_sur` (`Saisie` : moyen, motif, date,
mensualité, `du_tiroir`) ; `paie::verser_avec_sur` ;
`caisses::mouvement_especes_sur` écrit la sortie (motifs
`avance_salaire`, `salaire`) ou le retour au tiroir à l'annulation
(`avance_salaire_annulee`, caisse ouverte exigée) ; règles pures
`coeur::avances::{verifier_tiroir, verifier_mensualite, verifier_date}`
(31 jours, la règle des ventes ; prise dans le tiroir = aujourd'hui) ;
colonne `avance.mensualite` ; `retenir_avances` borne chaque avance à sa
mensualité — **par paie** (un journalier payé à la quinzaine rembourse
donc deux fois par mois) ; la commission : `Conditions.commission_sur_encaisse`
(absent = vendu), `paie::encaisse` et `vendu_jusqu_au` (la bascule). Le
journal comptable CA ne lit que `motif = 'depense'` : l'argent du tiroir
n'y est pas compté deux fois. Écrans : Avances (date, case, « retenir par
morceaux »), versement, avertissement sur la dépense « Salaire » de la
caisse, note au départ d'un vendeur, texte ITS dans Cotisations. Tests :
`paie_tiroir_base` 8, `commission_encaisse_base` 5 ; `paie_base` et
`paie_corrections_base` passés sur des ventes payées.

## D43 — L'assistant de création du dossier, au premier démarrage (28/10/2026)

**Le besoin** (le propriétaire) : « comme dans Ciel », un assistant qui
définit la société, les dates de travail du premier exercice et les
réglages qui aident ensuite, au lieu d'une base neuve qui démarre avec
« Ma boutique », l'année civile et `admin123`.

**Retenu** (proposé, accepté le 28/10/2026) :

1. **Connexion d'abord, puis l'assistant prend l'écran.** Le patron entre
   avec `admin` / `admin123` (l'écran de connexion d'une base neuve le
   dit : `/sante` porte `assistant_a_faire`). Pas de route ouverte sans
   mot de passe (Ciel crée son dossier avant toute connexion ; chez nous
   ce serait une porte ouverte au réseau local). L'assistant marche depuis
   n'importe quel poste.
2. **Une marque, pas une devinette** : l'amorçage d'une base neuve pose
   `config_app.assistant_dossier = a_faire` ; une base sans la marque
   (toutes celles d'avant) ne voit jamais l'assistant. Une base de
   démonstration (`GESCOM_DEMO=1`, `donnees_demo`, le banc) la retire
   (`amorcage::mode_essai`).
3. **Le serveur ne sert que l'assistant** tant qu'il reste à faire
   (`assistant::COMMANDES_PERMISES`, refus `ASSISTANT_A_FAIRE`, lu en base
   à chaque appel) : personne ne vend dans un dossier sans nom. Un
   vendeur qui se connecte lit « le patron n'a pas encore créé le
   dossier ».
4. **Les étapes** : vous (nom, identifiant, mot de passe — celui d'usine
   refusé) ; la société (raison sociale obligatoire, adresse, deux
   téléphones, email, NIF, RCCM, site, pied de facture, logo) — elle
   renomme aussi le dossier ; les dates du premier exercice (D21, D38 :
   libres, 5 ans au plus, **elles doivent comprendre aujourd'hui**) ; les
   réglages ; les vendeurs (mot de passe provisoire, à changer) ; le
   récapitulatif et « Créer mon dossier » ; puis la reprise des données
   (articles, fournisseurs), qui vient après : le dossier a alors son
   magasin et sa TVA.
5. **Les réglages** : devise FCFA, montrée, pas réglable (les montants
   sont des francs entiers) ; TVA des nouveaux articles 0 ou 18 % (clé
   `tva_par_defaut:<dossier>`, appliquée par le comptoir, Paramètres et
   l'import) ; premier numéro de facture de l'année, **seulement dans
   l'assistant et si aucune facture n'est sortie** (le préfixe `FAC` ne
   change pas) ; nom du magasin principal ; fond de caisse habituel
   (`fond_caisse_habituel:<dossier>`, proposé à l'ouverture, modifiable).
   Ensuite (le propriétaire, le même soir : « on doit pouvoir changer
   l'état de la TVA dans l'app ») : **l'état de la TVA en tête de
   Paramètres → TVA** (« La boutique facture-t-elle la TVA ? »), et
   Gescom **demande** s'il l'applique aussi aux articles existants —
   oui : tous, en une transaction (`definir_tva_boutique`) ; non : les
   nouveaux seulement. Il remplace l'ancien « Taux TVA global » de cet
   onglet, qui envoyait une commande par article (une coupure laissait
   le catalogue à moitié). Le fond de caisse reste dans Paramètres →
   Société.
6. **Plus de compte `employe` / `employe123` d'usine** : un deuxième mot
   de passe public. Il n'existe plus qu'en démonstration.
7. **Tout en une transaction** (`assistant::terminer_sur`), le serveur
   juge chaque champ (`coeur/assistant.rs`) ; le logo part par sa
   commande. Trace `assistant_dossier_fait` à l'Historique, sans mot de
   passe.

**Ce qu'on ne prend pas de Ciel** : le dossier de démonstration au
choix (le générateur de base de démonstration est un autre chantier) ;
le nombre de décimales (aucune en FCFA) ; régler le préfixe des pièces.

**Les risques nommés** : sur un serveur neuf branché au réseau, une
caisse peut se connecter en `admin123` avant le patron (c'était déjà le
cas ; l'écran le dit) ; un premier numéro de facture ne vaut que pour
l'année en cours — la série repart à 1 chaque 1er janvier, comme
toujours ; `parametres_societe` n'est pas cloisonnée (une société qui
imprime par installation) : l'assistant ne sert qu'au premier dossier,
les suivants se créent dans Paramètres → Dossiers. Gescom Équipe n'a
pas l'assistant : sur une base neuve, ses écrans disent « dossier pas
encore créé ».

**Mise en œuvre.** `noyau/src/assistant.rs`, `coeur/assistant.rs`,
marque posée par `amorcage::amorcer` et `seed` ; verrou dans
`serveur/src/api.rs` ; commandes `lire_assistant`, `terminer_assistant`,
`lire_reglages_dossier`, `definir_reglages_dossier`. Écran
`src/pages/AssistantDossier.tsx` (App le montre à la place de tout),
`ReglagesDuDossier.tsx`, `lib/reglagesDossier.ts`. Tests :
`noyau/tests/assistant_base.rs`, route HTTP
`une_base_neuve_ne_sert_que_l_assistant…`, banc `p1-premier-demarrage`
(`lancer.ps1 monoposte -Neuf -SansDemo`).

## D44 — Les doublons d'articles et de fournisseurs, l'import et l'export (28/10/2026)

**Le besoin** (le propriétaire) : l'import et l'export des fournisseurs,
la gestion des doublons, idem pour les articles.

**Retenu** (proposé, accepté le 28/10/2026) :

1. **Un doublon se reconnaît sans la casse ni les espaces en trop**
   (`coeur::doublons::normaliser_nom` : « Riz 25 kg » = « riz  25 KG ») ;
   les accents comptent. Articles : le nom, et le code-barre. Fournisseurs :
   le même NIF (même entreprise, refus) ou le même nom (homonyme possible).
2. **L'import montre un aperçu avant d'écrire** (`simuler`) : le même code,
   dans une transaction qui retombe — l'aperçu ne peut pas mentir.
   Créés, mis à jour / complétés, laissés (doublons), refusés, ligne par
   ligne.
3. **Articles** : une ligne répétée (même article, même unité) est
   refusée ; un code-barre déjà porté par un autre article refuse SA ligne
   à la création (avant : erreur SQL et tout l'import perdu), n'est pas
   repris à la mise à jour ; un article désactivé du même nom est dit, pas
   mis à jour en silence ; une colonne TVA vide garde le taux d'un article
   existant (avant : remis à 0) et donne celui du dossier (D43) à un
   neuf ; sans « mettre à jour », les unités d'un article créé par le même
   fichier passent quand même. La création dans Paramètres refuse un nom
   vide et un doublon, comme le comptoir.
4. **Fournisseurs** : export CSV (même format que le catalogue, sans le
   « Fournisseur divers ») ; import qui crée, ou **complète** un doublon
   (seulement ses champs vides, jamais son nom), ou le laisse ; à la
   main, un NIF déjà connu est refusé, un homonyme demande « créer quand
   même » (`FOURNISSEUR_HOMONYME`, `forcer`), un nom vide ou le nom
   réservé du « Fournisseur divers » est refusé, à la création comme à
   la modification.

**Ce qu'on ne fait pas** : fusionner deux fiches déjà en double (leurs
pièces, leurs dettes) — un autre chantier ; rapprocher un homonyme par
son téléphone (deux numéros pour une personne, un numéro pour deux
boutiques : trop d'erreurs).

**Mise en œuvre.** `coeur/doublons.rs`, `fournisseurs_csv.rs`,
`catalogue_csv::importer_articles_csv_sur_base` (aperçu, doublons),
`refuser_doublon_article_sur` ; commandes `exporter_fournisseurs_csv`,
`importer_fournisseurs_csv`, paramètre `simuler`. Écran
`ImportCsvAvecApercu.tsx`, `ImportExportFournisseurs.tsx` (page
Fournisseurs, assistant), `lib/homonyme.ts`. Tests :
`noyau/tests/doublons_base.rs`.

## D45 — Le seuil d'alerte des articles (28/10/2026, nuit)

**Le besoin** (le propriétaire) : « on définit le seuil pour les
articles lors de la création ». Relevé : aucun seuil par article ; 5 en
dur au tableau de bord et dans les rapports, 10 dans la page Stock.

**Retenu** (deux choix posés, acceptés) :

1. **Un seuil par article** (`article.seuil_alerte`, unité de base),
   commun aux dossiers comme l'article ; vide : **le seuil du dossier**
   (`config_app seuil_alerte_defaut:<dossier>`, Paramètres → Société,
   **5 d'office** — rien ne change pour les articles existants).
2. **Saisi à la création** (Paramètres → Articles, Achats → nouvel
   article), changé sur la fiche de l'article (`definir_seuil_article`),
   par la colonne **Seuil** de l'import (vide : inchangé), rendu à
   l'export.
3. **Un seul calcul** (`coeur::stock::statut_stock`) : « alerte »
   strictement sous le seuil (le stock minimum de Ciel), « rupture » à 0
   ou moins ; **0 coupe l'alerte**. Tableau de bord (total du dossier ou
   du magasin choisi), rapport du stock et page Stock (la ligne du
   magasin) comptent pareil — fini le 5 ici, 10 là.
4. Le serveur juge : positif ou nul, au plus un million ; un article
   qui ne suit pas son stock n'a pas de seuil (refus qui le dit).

**Pas pris** : un seuil par magasin (proposé, écarté : une saisie par
magasin pour une boutique qui n'en a souvent qu'un).

**Au passage, deux décisions du propriétaire le même soir.** (a) Créer
un **nouveau dossier par le même assistant** et donner **à chaque
dossier sa société** : « on laisse pour cette version » (ALERTES). (b)
L'assistant du premier démarrage a été retiré, puis **rétabli** sur sa
demande (« on peut revenir ici ») : D43 reste telle qu'écrite.

**Mise en œuvre.** Colonne dans `schema.sql`, `persistance/mod.rs`,
`amorcage.rs` ; `coeur::stock::{SEUIL_ALERTE_DEFAUT, verifier_seuil,
statut_stock}` ; `assistant::seuil_defaut_sur` ;
`parametres::definir_seuil_article_sur` ; `creer_article_complet` et
`creer_article_rapide` prennent `seuilAlerte` ; `definir_reglages_dossier`
prend `seuilDefaut`. Écran `ChampSeuil.tsx` (création, fiche),
`ReglagesDuDossier`, badge de `Stock.tsx`. Tests :
`noyau/tests/seuil_base.rs`.
