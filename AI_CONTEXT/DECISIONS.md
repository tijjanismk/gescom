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

1. Le serveur accepte une adresse PostgreSQL et tient une `Base`.
2. Les commandes non portées refusent franchement sur PostgreSQL.
3. La vente et la facture — le plus gros morceau, et le plus délicat.
4. Le reste des commandes, module par module.

---

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
| D4 | `pg_dump` tous les soirs, deux endroits, restauration à essayer |
| D5 | pas de signature tant que tu installes toi-même |
| D6 | aucun compte de secours ; une commande sur le serveur |
| D7 | l'écran des permissions par personne se fait |
| D8 | les images voyagent par leur contenu, pas par leur chemin |
| D9 | l'entretien de la base passe côté serveur |
| D10 | le mot de passe de la base reste hors du dépôt |
| D11 | le serveur tient une `Base` ; sur PostgreSQL, une commande non portée **refuse** au lieu de retomber sur SQLite |

Aucune case n'attend de réponse.
