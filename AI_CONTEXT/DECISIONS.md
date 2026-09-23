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
| D11 | le serveur tient une `Base` ; sur PostgreSQL, une commande non portée **refuse** au lieu de retomber sur SQLite — **186/187 portées le 12/09/2026** |
| D12 | le serveur est un **service Windows** (`GescomServeur`), installé à part, en administrateur ; il démarre avec la machine et se relance seul |
| D27 | **le serveur juge la saisie** : pas de mode « avoir » sans avoir consommé, montant > 0, quantité > 0, prix ≥ 0, remise 0–100 — règles dans `coeur/saisie.rs`, les deux versions |
| D26 | **l'auteur d'un geste est l'utilisateur de la session**, posé par le serveur sur le fil de la requête (`noyau::auteur`) — plus le premier compte du rôle |
| D25 | **le serveur gagne une fenêtre** : une coque Tauri fine qui affiche la console existante — pas une interface reconstruite |
| D24 | **deux rôles, un seul produit** : simple (monoposte, existe déjà) ou complet (serveur + plusieurs postes/dossiers), choisis **à l'installation**, jamais devinés en cours de route |
| D23 | le **plan comptable SYSCOHADA** en base, les opérations affectées en réglage, les journaux lus et exportés — rien de stocké ; la comptabilité complète après |
| D22 | **SQLite reste** ; plusieurs dossiers demandent **le serveur**, pas PostgreSQL — le serveur sert tout par `Base` sur les deux moteurs, la fenêtre monoposte garde un dossier (révise D11 et D13) |
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
