# Plan : plusieurs dossiers, plusieurs années

Ce document **tranche l'architecture avant qu'on code**. Il ne liste pas
des tâches : il pose des décisions, chacune avec son exemple, son coût
et une recommandation.

Écrit le 11 septembre 2026. Rien n'est en production.

---

## 1. Ce qu'on veut, en trois exemples

**Exemple A — deux commerces.** Tu tiens la quincaillerie, ton frère
tient une boutique. Même logiciel, même serveur, mais les ventes de
l'un ne doivent jamais apparaître dans les comptes de l'autre.

**Exemple B — changer d'année.** Le 1er janvier 2027 arrive. Tu veux
ouvrir 2027 sans perdre 2026, et pouvoir revenir dans 2026 quelques
semaines, le temps de finir les derniers papiers.

**Exemple C — un article partout.** Le sac de ciment CIMAF est le même
article dans les deux commerces. Tu ne veux pas le saisir deux fois, ni
le corriger deux fois quand son nom change.

---

## 2. Les mots, une fois pour toutes

| mot | ce que ça veut dire ici |
|---|---|
| **dossier** | un commerce, comme chez Ciel : ses ventes, ses clients, son stock, sa caisse |
| **exercice** | une période comptable **à l'intérieur** d'un dossier — en général une année |
| **magasin** | un lieu où il y a du stock (à l'écran ; dans le code il s'appelle encore `depot`) |

Le point important, et c'est celui qui décide de tout le reste :

> **Un dossier n'est PAS une année.** Un dossier contient plusieurs
> exercices qui se suivent. On ouvre un dossier, on y travaille des
> années durant.

C'est le modèle de Ciel, et c'est ce qui évite le piège du chapitre 4.

---

## 3. Décision 1 — ce qui est commun, ce qui appartient à un dossier

### Commun à TOUS les dossiers

| quoi | pourquoi |
|---|---|
| **articles, unités de vente, catégories** | ta décision : le sac de ciment est le même partout |
| utilisateurs, rôles, permissions | un caissier qui travaille dans les deux commerces reste la même personne, avec un seul mot de passe |
| postes et sessions | une caisse est une machine, pas un commerce |
| modèles de facture | une mise en page se partage ; c'est l'en-tête qui change, pas la structure |

### Propre à UN dossier

| quoi | pourquoi |
|---|---|
| **magasins et stock** | ta décision : chaque commerce a sa marchandise |
| mouvements de stock | ils disent d'où vient et où va cette marchandise |
| ventes, factures, pièces, avoirs | c'est l'activité du commerce |
| caisse et mouvements de caisse | le tiroir est celui d'un commerce |
| clients et fournisseurs | voir la décision 5 |
| journal | il retrace ce qui s'est passé dans ce commerce |
| suites de numéros | chaque dossier a sa propre `FAC-2026-00001` |

### Ce que ça donne en pratique

> Tu changes le nom d'un article : il change dans les deux commerces.
> Tu vends un sac : le stock baisse **dans un seul** des deux.

---

## 4. Décision 2 — les années ne vident pas le stock

C'est le piège de ce genre de découpage, et il faut le nommer.

Si un dossier était une année, alors le stock appartiendrait à l'année.
Le 1er janvier 2027, tes magasins seraient vides et il faudrait
ressaisir un inventaire d'ouverture.

**Pour une quincaillerie, c'est absurde** : le 31 décembre au soir il y
a 400 sacs dans le magasin, le 1er janvier au matin il y en a toujours
400. Personne ne les a déplacés.

### La décision

**Le stock suit le dossier, pas l'année.** L'exercice ne sert qu'à
dater : il dit quelles écritures sont encore modifiables et lesquelles
sont figées.

| | |
|---|---|
| un dossier | a **un** stock, continu d'une année sur l'autre |
| un dossier | a **plusieurs** exercices, qui se suivent |
| une écriture | appartient à l'exercice qui contient sa date |

> Le 1er janvier 2027, tes 400 sacs sont toujours là. Ce qui change,
> c'est que les factures de 2026 ne se modifient plus.

### Conséquence technique

Une seule colonne `dossier_id` sur les tables propres à un dossier.
L'exercice n'est **pas** une colonne : c'est une période, et la date de
l'écriture suffit à savoir de quel exercice elle relève. Une colonne de
plus serait une deuxième vérité à maintenir, et les deux finiraient par
se contredire.

Une table `exercice` : dossier, date de début, date de fin, date de
prolongation, clos ou non. La règle « cette date est-elle acceptée ? »
est déjà écrite, avec neuf scénarios.

---

## 5. Décision 3 — ouvrir un dossier, en changer

C'est la partie que tu vois, et elle se décide maintenant parce qu'elle
commande où vit le choix du dossier.

### À la connexion

Après le mot de passe, **le logiciel demande quel dossier ouvrir**.

```
   ┌──────────────────────────────────────┐
   │  Bonjour Tidiani                     │
   │                                      │
   │  Quel dossier ouvrir ?               │
   │   ▸ Quincaillerie du Fleuve          │
   │   ▸ Boutique Modibo                  │
   │                                      │
   │  ☑ Ouvrir celui-ci directement       │
   │     la prochaine fois                │
   └──────────────────────────────────────┘
```

**S'il n'y a qu'un dossier, l'écran ne s'affiche pas.** Il s'ouvre tout
seul. Personne ne doit cliquer chaque matin sur un choix qui n'en est
pas un — c'est exactement le cas de toutes les installations
aujourd'hui.

### Le dossier par défaut

La case à cocher mémorise le choix. Deux endroits possibles :

| | ce que ça donne | |
|---|---|---|
| **par utilisateur** | Tidiani ouvre toujours la quincaillerie, où qu'il se connecte | **recommandé** |
| par poste | la caisse du fond ouvre toujours le même dossier, quel que soit le caissier | |

**Recommandation : par utilisateur**, avec une exception possible plus
tard pour les postes de caisse. La session appartient déjà à une
personne ; y rattacher le dossier ne demande rien de neuf. Et un
caissier qui change de comptoir retrouve son commerce.

### Changer de dossier : on se déconnecte

**Décision : pas de sélecteur dans la barre.** Changer de dossier, c'est
fermer sa session et en ouvrir une autre.

Trois raisons, dans l'ordre d'importance :

**L'argent ne doit pas pouvoir se tromper de commerce.** Un bouton à
portée de clic, posé à côté de la caisse toute la journée, finit un
jour par être cliqué pendant qu'un panier est ouvert. La vente
commencée dans un commerce se termine dans l'autre, et rien dans les
chiffres ne le dira.

**Ce n'est pas une opération courante.** Chez Ciel non plus : on
changeait de dossier quand il y avait un problème sur l'un d'eux, pas
plusieurs fois par jour. Une commande rare n'a pas sa place dans la
barre permanente ; elle a sa place là où on l'attend, c'est-à-dire à
l'ouverture.

**Se reconnecter est franc.** Le tiroir se ferme, la session se ferme,
la suivante s'ouvre sur un état propre. Aucun reste du dossier
précédent — ni panier, ni magasin sélectionné, ni session de caisse —
ne peut traîner.

> Tu veux passer sur la boutique de ton frère : tu te déconnectes, tu
> te reconnectes, tu choisis son dossier. Trois secondes, et aucun
> doute sur ce qui appartient à qui.

Si un jour le besoin apparaît vraiment, la porte reste ouverte — mais
avec la règle qui va avec : **refus de changer tant qu'une caisse est
ouverte ou qu'un panier est en cours**, exactement comme le changement
de mode de caisse est déjà refusé caisse ouverte.

### Deux dossiers en même temps

C'est possible, et ça marche sans rien de plus : chaque fenêtre a sa
session, chaque session a sa connexion, chaque connexion porte son
dossier. Deux fenêtres ouvertes sur deux dossiers ne se mélangent pas.

⚠️ **Mais elles ne doivent pas partager leur mémoire.** Le magasin
sélectionné, le panier en cours, le dossier : tout cela est retenu par
le navigateur. Si les deux fenêtres écrivent au même endroit, la
seconde écrase la première. À vérifier au moment de le faire.

---

## 6. Décision 4 — le prix est commun

**Décision : un seul prix de référence, commun à tous les dossiers.**

J'avais présenté cela comme un problème. Ça n'en est pas un, parce que
le logiciel ne s'est jamais servi de ce prix comme d'un prix imposé.

### Ce qui existe depuis le début

Au comptoir, **le montant se saisit librement**. Le prix de l'article
n'est qu'un point de départ. Quand le caissier saisit autre chose,
l'écran le dit :

```
   Sac de ciment CIMAF          5 800 F
   Prix majoré de 300 F
```

Et dans l'autre sens, la ligne est marquée « (remise) ».

> Ton frère vend le sac 5 800 F : il tape 5 800. L'écran signale la
> majoration, la vente s'enregistre au vrai prix, et ton prix à toi ne
> bouge pas.

C'est exactement ce qu'il faut : **le prix commun est une référence, pas
une contrainte.** Et l'écart reste visible ligne par ligne, ce qui vaut
mieux qu'un prix différent par dossier — lequel aurait effacé l'écart
en le rendant « normal ».

### Ce que ça coûte, honnêtement

Un dossier qui vend systématiquement au-dessus verra la mention
« majoré » sur chaque ligne. Un signal qui s'allume toujours finit par
ne plus être lu.

Ce n'est pas bloquant et ça ne se corrige pas à l'avance : si ça devient
gênant à l'usage, un prix de référence propre au dossier se rajoute
sans rien casser, puisque la lecture du prix passe déjà par un seul
endroit.

## 7. Décision 5 — les clients et les fournisseurs

Ils ne sont pas dans le même cas que les articles.

Un article est une **chose** : le sac de ciment est le même objet pour
tout le monde. Un client est une **relation** : sa dette, son
historique, sa confiance appartiennent à un commerce.

> Awa doit 30 000 F à la quincaillerie. Elle ne doit rien à la boutique
> de ton frère. Si le client est commun, les deux commerces voient la
> même dette — et l'un réclame de l'argent qu'on ne lui doit pas.

**Décision : clients et fournisseurs appartiennent au dossier.** Le même
nom peut exister dans les deux, avec deux codes différents. C'est
voulu : ce sont deux relations distinctes.

---

## 8. Décision 6 — comment le logiciel sait où il travaille

La connexion à la base **porte son dossier**. Il n'est pas passé en
argument de fonction en fonction : tant qu'il voyage, il peut manquer
quelque part, et il suffit d'un endroit.

- À l'ouverture de session, l'utilisateur choisit son dossier (ch. 5).
- La connexion retient ce choix et le dit à PostgreSQL.
- Toute écriture tombe automatiquement dans ce dossier.

⚠️ **Si plus tard le serveur réutilise ses connexions** (une réserve
partagée), chaque prise doit reposer son dossier — sinon elle garde
celui du client précédent. Ce défaut ne se voit qu'en charge, et il
mélange réellement deux commerces.

---

## 9. Décision 7 — la garantie, parce que ce découpage a un défaut connu

Le défaut tient en une phrase : **une requête qui oublie le filtre
mélange deux dossiers**. Et elle ne prévient pas — elle rend des
chiffres plausibles, calculés sur les ventes de quelqu'un d'autre.

**1. Le détecteur (fait).** Il repère une requête qui touche une table
cloisonnée sans filtrer, nomme la table, et refuse **avant**
d'exécuter. Il tourne dans les tests : un filtre oublié échoue là, pas
chez toi.

**2. Le moteur (à faire).** PostgreSQL sait refuser lui-même une requête
non filtrée. Mais **il ne le fait pas pour le compte tout-puissant
`postgres`**, avec lequel le logiciel se connecte aujourd'hui. Tant que
ce sera le cas, la séparation repose uniquement sur le code.

> Aujourd'hui : si je me trompe, le détecteur m'arrête pendant les
> tests. Avec un compte limité : même si je me trompe **et** que le test
> manque, PostgreSQL refuse.

**Recommandation :** créer un compte PostgreSQL applicatif limité au
moment où le multi-dossier servira vraiment. Ça ne change rien pour
toi, sauf une ligne de configuration à l'installation.

---

## 10. Ce que ce plan change dans le code déjà écrit

La fondation posée le 11/09 tient, **sauf sur un point** : j'ai
cloisonné les articles, et ta décision dit l'inverse.

| | |
|---|---|
| garder | la table `dossier`, la colonne `dossier_id`, le détecteur, la règle des dates, les suites de numéros par dossier |
| **corrigé le 11/09/2026** | `article`, `unite_vente`, `categorie` retirés de `TABLES_CLOISONNEES` et de leurs filtres |
| reste à ajouter | une table `exercice` rattachée au dossier ; aujourd'hui les dates sont portées par le dossier lui-même |

Le nom `dossier_id` est **conservé** : c'est bien la notion de Ciel.

Rien n'est en production : ces corrections ne coûtent que le temps de
les faire, aucune migration chez un commerçant.

### La correction, en détail

`categorie`, `article`, `unite_vente` sont sortis de
[`TABLES_CLOISONNEES`](../src-tauri/noyau/src/dossiers.rs) : plus de
colonne `dossier_id`, plus de filtre. `stock_depot` reste cloisonné —
c'est le stock, pas l'article, qui appartient au magasin d'un dossier.

Trois endroits touchés :
- `catalogue::lire_articles_avec_unites_sur` — le filtre de dossier ne
  porte plus que sur la jointure `stock_depot` (dans le `ON`, pas le
  `WHERE`, pour ne pas transformer le `LEFT JOIN` en `INNER` quand un
  article n'a encore aucun stock dans ce magasin) ;
- `comptoir::creer_article_rapide_sur` — la recherche de doublon par nom
  porte sur **tous** les dossiers, et les `INSERT` dans `article` /
  `unite_vente` ne portent plus `dossier_id` ;
- les tests `postgres_amorcage.rs` et `cloisonnement.rs` vérifient
  maintenant qu'un article créé dans un dossier reste visible depuis
  l'autre, en plus de l'isolation des clients.

Vérifié : 226 tests SQLite au vert. Les scénarios PostgreSQL
correspondants n'ont pas pu être rejoués dans cette séance — pas
d'instance PostgreSQL accessible ici — à revérifier avec `GESCOM_PG`
défini avant de considérer le morceau clos.

---

## 11. Ce qui n'est PAS dans ce plan

- **Renommer `depot` en `magasin` jusque dans la base.** À l'écran c'est
  fait ; dans le code il s'appelle encore `depot`. Ce n'est pas gênant
  et ça touche ~200 endroits. À décider séparément.
- **Les sauvegardes.** Écarté sur ta demande.
- **La signature de l'installeur.**
- **La fin du passage à PostgreSQL** — suivi dans [ETAPES.md](ETAPES.md),
  et elle avance en parallèle.

---

## 12. Ce qu'il reste à décider

**Rien.** Toutes les décisions de ce plan sont prises.

## 13. L'ordre des travaux

1. **La vente et la facture sur PostgreSQL.** Indépendant du
   multi-dossier, et c'est ce qui empêche aujourd'hui une boutique
   entière de tourner dessus.
2. Corriger la fondation : articles communs, table `exercice`.
3. L'écran d'ouverture de dossier, avec le défaut mémorisé — et
   l'escamotage quand il n'y en a qu'un.
4. Créer un dossier : lui poser son magasin et son client de passage.
   Un dossier neuf n'en a pas, et sans eux il ne peut pas vendre.
5. L'écran des exercices : ouvrir, prolonger, clore.
6. Le compte PostgreSQL limité.
