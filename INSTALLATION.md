# Gescom — Guide d'installation

*Version 1.12 — 28 octobre 2026, revu le 1er novembre 2026 (D54 : la
désinstallation garde les données ; Gescom Serveur reprend la base d'un
poste seul) — tous les cas d'installation.*

---

# 1. Les trois programmes

| Programme | Fichier | Taille | Droits administrateur | Où l'installer |
|---|---|---|---|---|
| **Gescom** (la caisse) | `Gescom_1.12.0_x64-setup.exe` | 214 Mo | non | sur chaque poste qui vend |
| **Gescom Serveur** | `Gescom-Serveur_0.2.0_x64-setup.exe` | 4 Mo | **oui** | sur l'ordinateur qui garde la base, quand il y a plusieurs caisses |
| **Gescom Équipe** (personnel, paie, suivi clients) | `Gescom Equipe_0.1.0_x64-setup.exe` | 3 Mo | non | sur le poste du patron ou de qui fait la paie |

**La base** — les ventes, le stock, la caisse, le personnel — est tenue par
un **serveur**. Une caisse ne garde rien : tout ce qu'elle affiche vient du
serveur. Même sur un seul ordinateur, la caisse lance un serveur à côté
d'elle ; on ne le voit pas.

**L'installeur de la caisse contient WebView2** (le moteur d'affichage de
Windows) : il s'installe sans Internet. Celui d'Équipe ne le contient pas
— il est déjà là sur Windows 11, ou après la caisse.

**Les versions vont ensemble.** Une caisse d'une autre version que son
serveur refuse de se connecter. On met à jour le serveur et toutes les
caisses le même jour (chapitre 10).

---

# 2. Choisir son cas

| Votre boutique | Ce qu'on installe | Chapitre |
|---|---|---|
| Un seul ordinateur | la caisse (et Équipe si on fait la paie) | 3 |
| Deux caisses ou plus | Gescom Serveur sur l'ordinateur principal, la caisse sur chaque poste | 4 |
| Deux caisses, sans installer le serveur | la caisse partout ; un poste garde la base pour les autres — **déconseillé** | 5 |
| Base sur PostgreSQL (gros volume, technicien) | Gescom Serveur réglé sur PostgreSQL | 6 |
| La paie et le suivi des clients | Gescom Équipe, avec ou sans caisse sur le poste | 7 |
| Une caisse de plus, un poste renommé | rien à réinstaller ailleurs | 8 |
| On avait un seul ordinateur, on passe à plusieurs caisses | une opération à faire avec soin | 9 |

**Conseil** : dès deux caisses, prendre le cas du chapitre 4. Le serveur
y tourne comme un **service Windows** : il démarre avec l'ordinateur,
avant même qu'on ouvre une session, et se relance tout seul s'il tombe.

---

# 3. Un seul ordinateur

1. Lancer `Gescom_1.12.0_x64-setup.exe` (double-clic, pas besoin d'être
   administrateur). Suivre l'assistant.
2. Ouvrir **Gescom**. Au premier démarrage, l'écran **« Comment travaille
   ce poste ? »** demande, une seule fois, le rôle du poste. Choisir
   **« Ce poste garde la base »**.
3. Gescom affiche « Démarrage du serveur de ce poste… » puis l'écran de
   connexion. Rien à saisir : il prend le port 40000, ou le suivant libre
   de 40000 à 40100 si un autre programme l'occupe, et le retient.
4. Se connecter avec le compte **admin**, mot de passe **admin123**
   (l'écran de connexion d'une base neuve le rappelle). Une base neuve
   n'a que ce compte.
5. **L'assistant de création du dossier** prend l'écran : votre
   identifiant et votre mot de passe (`admin123` est refusé), la société,
   les dates de travail, la TVA, le magasin, le fond de caisse, les
   vendeurs. Rien d'autre ne s'ouvre avant « Créer mon dossier », sur
   aucun poste (manuel, « Créer le dossier »).

**Où sont les données** : `C:\Users\<votre nom>\AppData\Roaming\ml.gescom.app\`
— la base `gescom.db`, le réglage du poste `poste.json`, le journal, et
le dossier `sauvegardes` (une copie par jour, quatorze gardées).

> Ne copiez pas `Gescom.exe` sur une clé USB ou dans un autre dossier :
> déplacé, il refuse de démarrer (« Ce fichier Gescom.exe a été déplacé
> hors de son dossier d'installation »). Vos données ne sont pas
> touchées. Il faut passer par l'installeur.

Si l'écran « Le serveur de ce poste ne répond pas » s'affiche, il dit
pourquoi et propose **Réessayer** : voir le chapitre 14.

---

# 4. Plusieurs caisses avec Gescom Serveur

## 4.1 Le serveur, sur l'ordinateur principal

Choisir un ordinateur **qui reste allumé** pendant les heures
d'ouverture.

1. **Fermer Gescom** (la caisse) sur cet ordinateur, s'il y est. Puis
   clic droit sur `Gescom-Serveur_0.2.0_x64-setup.exe` → **Exécuter en
   tant qu'administrateur**. Windows peut afficher « éditeur inconnu » :
   le certificat est celui de Gescom, pas d'un grand éditeur. Accepter.
   Si Gescom est encore ouvert, l'installeur le dit : le fermer, puis
   **Réessayer** (**Annuler** arrête l'installation sans rien changer).
2. Page **« Configuration du serveur »** (une seule fois) :

| Question | Réponse habituelle |
|---|---|
| Base de données | laisser la valeur proposée : un fichier dans `C:\ProgramData\Gescom` (pour PostgreSQL, chapitre 6) |
| Port d'écoute | **40000** — les caisses ne joignent qu'un port de 40000 à 40100 |

3. L'installeur fait le reste :
   - il copie le service et sa fenêtre dans `C:\Program Files\Gescom Serveur` ;
   - il fait reconnaître le certificat Gescom par cet ordinateur ;
   - il ouvre le **pare-feu** : TCP 40000 à 40100, et UDP 40000 pour
     « Rechercher le serveur » — réseaux Domaine et Privé seulement ;
   - si cet ordinateur **travaillait seul** et que sa base a des ventes,
     il propose de la **copier dans Gescom Serveur** (chapitre 9) ;
   - il installe et démarre le service Windows **GescomServeur** ;
   - il ajoute au menu Démarrer → Gescom : **Gescom Serveur** (la
     fenêtre), Console du serveur, Journal du serveur, Configuration du
     serveur.
4. Ouvrir **Gescom Serveur**. Sa carte **Réseau** donne le **nom du
   poste** et son adresse : c'est ce que les caisses chercheront.

> Le réseau Wi-Fi du magasin doit être en **Privé** dans Windows, pas
> en Public : sinon le pare-feu bloque les caisses.

> Une adresse **fixe** pour cet ordinateur vaut mieux (réglage de la
> box). Sinon, un jour, la box en donne une autre ; les caisses le
> retrouvent depuis leur écran de connexion (**Réglage du serveur** →
> **Rechercher le serveur**), mais il faut le faire.

**Si un fichier `C:\ProgramData\Gescom\serveur.json` existe déjà**, la
page de configuration est sautée : l'ancienne configuration est gardée.

## 4.2 Chaque caisse

1. Lancer `Gescom_1.12.0_x64-setup.exe` sur le poste.
2. Ouvrir **Gescom**. À **« Comment travaille ce poste ? »**, choisir
   **« Ce poste est une caisse d'un autre poste »**.
3. Écran **« Brancher ce poste »** :
   - cliquer **Rechercher le serveur** : en deux secondes, le serveur
     apparaît par son nom ; cliquer dessus remplit l'adresse ;
   - sinon taper l'**Adresse du serveur** à la main : son nom ou son IP
     (`192.168.1.10`), avec `:40001` seulement si le port n'est pas
     40000 ;
   - donner un **Nom de ce poste** (« Caisse 1 ») ;
   - cliquer **Essayer** : « Serveur joignable » doit s'afficher ;
   - cliquer **Enregistrer** (il ne s'active qu'après un essai réussi).
4. Se connecter.

## 4.3 Une caisse sur l'ordinateur du serveur

Installer la caisse comme sur les autres postes et choisir aussi **« Ce
poste est une caisse d'un autre poste »**, puis **Rechercher le
serveur** : il se trouve lui-même. La caisse et le service partagent
alors la même base, celle de `C:\ProgramData\Gescom`.

---

# 5. Plusieurs caisses sans installer le serveur (déconseillé)

C'est possible : un poste choisit « Ce poste garde la base », les autres
« Ce poste est une caisse d'un autre poste » et le trouvent. Mais :

- le serveur de ce poste **s'arrête quand on ferme sa session Windows** ;
- l'installeur de la caisse **n'ouvre pas le pare-feu** (il n'a pas les
  droits administrateur) : les autres caisses sont bloquées tant qu'un
  technicien ne l'ouvre pas à la main, dans PowerShell **en
  administrateur** sur ce poste (les deux règles que pose l'installeur
  de Gescom Serveur) :

```
netsh advfirewall firewall add rule name="Gescom serveur" dir=in action=allow protocol=TCP localport=40000-40100 profile=domain,private
netsh advfirewall firewall add rule name="Gescom serveur (recherche)" dir=in action=allow protocol=UDP localport=40000 profile=domain,private
```

Pour plusieurs caisses, préférer le chapitre 4.

---

# 6. Le serveur sur PostgreSQL

Pour un technicien. À préparer **avant** l'installation :

1. PostgreSQL installé, avec une base vide créée pour Gescom (Gescom ne
   crée pas la base, il crée ses tables au premier démarrage).
2. Un compte propriétaire de cette base.

À la page « Configuration du serveur », mettre l'adresse de la base :

```
postgresql://utilisateur:motdepasse@127.0.0.1:5432/gescom
```

Le mot de passe reste dans `C:\ProgramData\Gescom\serveur.json`, sur cet
ordinateur ; il n'est envoyé nulle part. **Attention** : avec les droits
de Windows par défaut, ce fichier se lit par **tous les comptes** de
l'ordinateur (pas seulement les administrateurs) — y ouvrir une session
seulement pour qui peut voir la base.

**Le compte limité** (recommandé : chaque dossier ne voit que ses
lignes, même en cas d'erreur du programme). Une fois, puis **après
chaque mise à jour** :

```
"C:\Program Files\Gescom Serveur\gescom-serveur.exe" --base <adresse du propriétaire> --compte-limite gescom_app
```

La commande crée le compte `gescom_app` et dit quoi mettre dans la clé
`base` de `serveur.json`. Redémarrer ensuite le service.

**Les sauvegardes** passent par `pg_dump`, cherché dans le PATH puis
dans `C:\Program Files\PostgreSQL\*\bin`. Sous le compte limité, la
sauvegarde a besoin de l'adresse du **propriétaire** dans la variable
`GESCOM_PG_SAUVEGARDE`. Le service la lit dans **son** environnement,
que Windows lui donne **au démarrage de l'ordinateur**, avec les
variables **système** seulement. Donc, dans PowerShell en administrateur :

```
setx /M GESCOM_PG_SAUVEGARDE "postgresql://proprietaire:motdepasse@127.0.0.1:5432/gescom"
```

puis **redémarrer l'ordinateur** (un `sc stop` / `sc start` ne suffit
pas). Une variable de l'utilisateur, ou posée sans redémarrer, ne lui
parvient pas : la sauvegarde refuse alors en le disant (« compte
PostgreSQL limité… GESCOM_PG_SAUVEGARDE »). Comme `serveur.json`, une
variable système se lit par tous les comptes de l'ordinateur.

---

# 7. Gescom Équipe

Gescom Équipe n'a **pas de base à elle** : elle parle au serveur de la
boutique, avec les mêmes comptes et les mêmes droits. Son seul réglage
est l'adresse de ce serveur.

1. Lancer `Gescom Equipe_0.1.0_x64-setup.exe`.
2. Ouvrir **Gescom Équipe**.
   - Sur un poste qui a **déjà la caisse**, branchée : rien à régler,
     Équipe prend l'adresse de la caisse.
   - Sur un poste **sans caisse** (le bureau du patron, par exemple),
     ou si l'écran dit **« Le serveur ne répond pas »** : cliquer
     **Réglage du serveur**, sous « Se connecter ». Puis **Rechercher le
     serveur** (ou taper son adresse), **Essayer**, **Enregistrer**.
3. Se connecter avec son compte habituel, choisir le dossier s'il y en
   a plusieurs.

| Poste | Équipe fonctionne ? |
|---|---|
| Un seul ordinateur (« Ce poste garde la base ») | oui, sans réglage |
| Une caisse branchée sur un serveur | oui, sans réglage |
| L'ordinateur du serveur, avec sa caisse | oui, sans réglage |
| L'ordinateur du serveur, **sans** caisse | oui si le serveur écoute sur 40000 ; sinon Réglage du serveur |
| Un poste **sans caisse**, serveur ailleurs | oui : **Réglage du serveur**, une fois |

> L'adresse réglée dans Équipe **passe devant** celle de la caisse du
> même poste. Pour revenir à celle de la caisse : Réglage du serveur →
> **Reprendre celle de la caisse**.

Ce que chacun voit dépend de ses droits (Paramètres → Utilisateurs →
Permissions, dans la caisse). Le détail : le manuel de Gescom Équipe.

---

# 8. Ajouter, renommer, changer de rôle

- **Une caisse de plus** : chapitre 4.2 sur le nouveau poste. Rien à
  faire sur le serveur.
- **Renommer un poste, changer son serveur, changer son rôle** :
  **Paramètres → Réseau** (permission « gérer les postes »).
  - « En faire une caisse d'un autre poste » : **Tester** (obligatoire),
    ou **Rechercher le serveur**, puis Enregistrer.
  - « Ce poste garde la base » : une confirmation prévient que les ventes
    du poste principal n'y sont pas.

> Paramètres → Réseau demande d'être **connecté**. Une caisse qui ne
> trouve plus son serveur passe par son **écran de connexion** :
> **Réglage du serveur** → Rechercher le serveur (ou taper l'adresse) →
> Essayer → Enregistrer. Ce lien n'existe que sur une caisse d'un autre
> poste et dans Équipe : sur l'ordinateur qui garde la base, le serveur
> est sur place.

---

# 9. D'un seul ordinateur à plusieurs caisses

Un ordinateur qui travaillait seul (« Ce poste garde la base ») a sa base
dans `C:\Users\<nom>\AppData\Roaming\ml.gescom.app\gescom.db`. Gescom
Serveur, lui, sert la sienne, dans `C:\ProgramData\Gescom`. **L'installeur
de Gescom Serveur reprend la base du poste** (D54) :

1. Sur l'ordinateur unique : **Paramètres → Sauvegarde → Sauvegarder
   maintenant** (une précaution : rien ne doit se perdre, mais une copie
   de plus ne coûte rien). **Fermer Gescom.**
2. **Installer Gescom Serveur sur cet ordinateur** (chapitre 4.1), sous
   le compte Windows qui utilisait la caisse. S'il trouve la base du
   poste avec des ventes et que celle du serveur est neuve, il demande :

   > Ce poste travaillait seul : il a déjà une base Gescom.
   > `C:\Users\Awa\AppData\Roaming\ml.gescom.app\gescom.db`
   > 1 234 ventes, 210 articles, 56 clients ; dernière vente le 31/10/2026.
   >
   > La base de Gescom Serveur est neuve : elle ne contient rien.
   >
   > Copier la base de ce poste dans Gescom Serveur ?
   >
   > Oui : la copie est vérifiée, puis le service démarre dessus. La base
   > du poste reste à sa place, sans changement.
   >
   > Non : Gescom Serveur démarre sur une base vide, et la caisse de ce
   > poste s'y branchera. La base du poste reste à sa place ; pour la
   > reprendre plus tard, relancer cet installeur.

   **Oui** : la copie se fait (`VACUUM INTO`, les dernières ventes
   comprises), puis elle est **vérifiée** — intégrité, et le même nombre
   de lignes dans chaque table que l'originale — avant de prendre la
   place de la base neuve du serveur, qui est gardée à côté
   (`gescom.db.avant-reprise-<date>`). Le logo, l'en-tête et le pied de
   page sont copiés dans `C:\ProgramData\Gescom`. Puis le service démarre
   dessus. Si la copie échoue, l'installeur dit pourquoi ; rien n'est
   perdu, le service démarre sur sa base neuve.
3. Ouvrir Gescom sur cet ordinateur : il trouve le service tout seul, et
   les ventes sont là. Vérifier les dernières ventes, le stock, la caisse
   du jour.
4. Installer les autres caisses (chapitre 4.2).

**Après la reprise**, la base du poste reste où elle était, intacte, avec
une marque à côté (`reprise.json`). Plus rien ne la sert : si le service
était arrêté, le serveur que lance Gescom refuse de la servir (« la base
de ce poste a été reprise par Gescom Serveur… ») plutôt que de laisser
cette caisse vendre dans l'ancienne base pendant que les autres vendent
dans la vraie. Il faut alors redémarrer le service (fenêtre Gescom
Serveur → Démarrer le service).

**Les cas où l'installeur ne copie pas** — il le dit dans une boîte :

| Cas | Ce qui se passe | Que faire |
|---|---|---|
| On a répondu **Non** | le service démarre sur une base vide | relancer l'installeur tant que la base du serveur n'a rien enregistré : il repropose |
| **Les deux bases ont des ventes** | aucune n'est touchée, le serveur garde la sienne ; la boîte revient à chaque installation | choisir avec un technicien : garder celle du serveur (commande `--laisser` ci-dessous, la boîte ne revient plus), ou remettre celle du poste (sauvegarde puis `--restaurer`, chapitre 13) |
| Gescom Serveur est sur **PostgreSQL** | rien n'est copié | un technicien, avec un outil à part |
| La base du poste **ne se lit pas** | rien n'est copié | restaurer d'abord une sauvegarde du poste |
| La caisse tournait sous **un autre compte Windows** que celui qui installe | rien n'est trouvé, rien n'est dit | la commande à la main, ci-dessous |

**À la main** (technicien), dans une invite de commandes **en
administrateur** :

```
sc stop GescomServeur
"C:\Program Files\Gescom Serveur\gescom-serveur.exe" --reprendre "C:\Users\<nom>\AppData\Roaming\ml.gescom.app\gescom.db" --examiner
"C:\Program Files\Gescom Serveur\gescom-serveur.exe" --reprendre "C:\Users\<nom>\AppData\Roaming\ml.gescom.app\gescom.db"
sc start GescomServeur
```

`--examiner` dit seulement ce qui se ferait. Sans `--base`, la cible est
la base du service (la clé `base` de `serveur.json`). La commande refuse
une base de serveur qui a déjà des ventes, une base sur PostgreSQL, et
un service en marche (« ouverte par un autre programme : arrêter le
service »). Pour **garder la base du serveur** et ne plus être
questionné : `--reprendre "<base du poste>" --laisser` (rien n'est
copié, la marque est posée). Ce qui a été fait s'écrit dans
`C:\ProgramData\Gescom\serveur.log`.

---

# 10. Mettre à jour

**Toujours le même jour, dans cet ordre** : le serveur, puis chaque
caisse, puis Équipe. Faire une sauvegarde avant.

| Programme | Comment | Ce qui est gardé |
|---|---|---|
| Gescom Serveur | fermer Gescom sur cet ordinateur, puis relancer le nouvel installeur en administrateur : il arrête le service, remplace, relance | `serveur.json`, la base, les sauvegardes, le journal (la page de configuration ne revient pas) |
| Gescom (caisse) | lancer le nouvel installeur par-dessus | la base, `poste.json`, les sauvegardes |
| Gescom Équipe | lancer le nouvel installeur par-dessus | son réglage |

Au premier démarrage après une mise à jour, **le serveur adapte la base
tout seul** (nouvelles colonnes, nouvelles tables). Sur PostgreSQL sous
le compte limité, relancer la commande `--compte-limite` (chapitre 6).

---

# 11. Changer le port ou l'écoute du serveur

Dans **Gescom Serveur** (ou la console), carte **Réseau** → « Changer le
port ou l'écoute » → **Appliquer…** → **Changer et redémarrer le
serveur**.

- Seulement **sur l'ordinateur du serveur**, par un compte qui peut
  modifier les paramètres. Impossible sur un poste unique : « le port est
  géré par Gescom ».
- L'écran dit avant combien de caisses sont connectées : **elles perdent
  le serveur** et doivent le retrouver : sur chaque caisse et chaque
  Gescom Équipe, écran de connexion → **Réglage du serveur** →
  **Rechercher le serveur** → Essayer → Enregistrer.

> À éviter sans nécessité : il faut passer sur chaque poste.

---

# 12. Désinstaller

| Programme | Ce qui part | Ce qui reste |
|---|---|---|
| Gescom Serveur | le service, les règles du pare-feu, les programmes, les raccourcis | **`C:\ProgramData\Gescom`** : la base, `serveur.json`, les sauvegardes, le journal |
| Gescom (caisse) | les programmes, les raccourcis | **`C:\Users\<nom>\AppData\Roaming\ml.gescom.app`** : la base d'un poste seul, `poste.json`, les sauvegardes, le journal |
| Gescom Équipe | le programme | **`C:\Users\<nom>\AppData\Roaming\ml.gescom.equipe`** : l'empreinte du poste, par laquelle le serveur le reconnaît |

**La désinstallation ne touche jamais aux données** (D54). La case
« Supprimer les données de l'application » que Windows montrait pour la
caisse et pour Équipe n'existe plus ; la page de confirmation de la
caisse dit où restent les données.

**Pour repartir de zéro** (un poste qu'on donne, une base d'essai à
jeter) : désinstaller, puis effacer **à la main** le dossier du tableau
ci-dessus — après avoir copié son dossier `sauvegardes` sur une clé USB
si la base compte. Pour Gescom Serveur, arrêter le service avant
d'effacer `C:\ProgramData\Gescom`.

---

# 13. Sauvegarder et restaurer

- **Automatique** : le serveur vérifie cinq minutes après son démarrage
  puis chaque heure ; si la dernière copie a vingt-quatre heures, il en
  fait une. **Quatorze** copies gardées.
- **Où** : un seul ordinateur → `AppData\Roaming\ml.gescom.app\sauvegardes` ;
  Gescom Serveur → `C:\ProgramData\Gescom\sauvegardes` ; un autre
  dossier se choisit dans **Paramètres → Sauvegarde**.
- **À la main** : Paramètres → Sauvegarde → **Sauvegarder maintenant**,
  ou la console du serveur → Administration.
- **Hors de la boutique** : chaque samedi, copier le dossier
  `sauvegardes` sur une clé USB gardée ailleurs.

**Restaurer** ne se fait pas à l'écran : serveur arrêté, dans une invite
de commandes en administrateur, avec `--restaurer <copie>` **et `--base
<la base à remplacer>`** :

```
sc stop GescomServeur
"C:\Program Files\Gescom Serveur\gescom-serveur.exe" --restaurer <chemin de la copie> --base C:\ProgramData\Gescom\gescom.db
sc start GescomServeur
```

La commande vérifie la copie avant de la mettre en place et garde
l'ancienne base à côté (`…avant-restauration-<date>`). Sur un ordinateur
qui a Gescom Serveur, **sans `--base` elle refuse** et affiche les deux
commandes possibles : la base du service, et celle du compte Windows qui
tape la commande (un poste seul). Sur un poste seul sans Gescom Serveur,
`--base` peut s'omettre : c'est la base du compte.

---

# 14. Problèmes courants

| Ce qu'on voit | Pourquoi | Que faire |
|---|---|---|
| « éditeur inconnu » / SmartScreen | la caisse n'est pas signée, le serveur l'est par Gescom | « Informations complémentaires » → « Exécuter quand même » |
| « Ce fichier Gescom.exe a été déplacé hors de son dossier d'installation » | le programme a été copié | lancer Gescom depuis le menu Démarrer, ou réinstaller |
| « Le serveur de ce poste ne répond pas » (un seul ordinateur) | le serveur intégré n'a pas démarré ; la raison est écrite | Réessayer ; si « introuvable » : réinstaller Gescom ; si « aucun port libre » : fermer le programme qui occupe 40000 |
| « Le serveur ne répond pas à … » sur l'écran de connexion, ou « Serveur injoignable (…) » | serveur arrêté, ordinateur principal éteint, réseau, ou adresse changée | ouvrir **Gescom Serveur** sur le principal : « Le serveur ne tourne pas » → **Démarrer le service** ; vérifier le réseau Privé ; si l'adresse ou le port a changé : **Réglage du serveur** sur l'écran de connexion |
| « Aucun serveur Gescom n'a répondu sur ce réseau » | pare-feu (UDP 40000), autre réseau Wi-Fi, serveur arrêté | taper l'adresse à la main ; vérifier le pare-feu |
| L'adresse tapée dans Réglage du serveur est refusée | elle ne répond pas, ce n'est pas un serveur Gescom, ou son port est hors de 40000-40100 : l'écran le dit | lire l'adresse dans **Gescom Serveur** (« Sur les caisses, taper ») ; l'ancienne adresse reste tant qu'une nouvelle n'a pas répondu |
| « HORLOGE_RECULE — L'horloge de l'ordinateur qui garde la base recule… » | l'heure de l'ordinateur a été reculée | remettre l'heure juste ; si elle était en avance, le patron confirme dans Paramètres → Horloge |
| « Le … est hors des dates de travail » | l'exercice est fini | le patron le prolonge ou en ouvre un nouveau (manuel d'utilisation) |
| La caisse paraît vide après l'installation du serveur | on a répondu Non à la reprise, ou la caisse tournait sous un autre compte Windows | chapitre 9 : relancer l'installeur, ou la commande `--reprendre` |
| « Gescom est ouvert sur ce poste » en installant Gescom Serveur | la caisse tient sa base et relancerait son serveur | fermer Gescom, Réessayer |
| « Serveur non démarré : la base de ce poste a été reprise par Gescom Serveur… » au journal `gescom.log` du poste | le service est arrêté, et l'ancienne base n'est plus servie (exprès) | démarrer le service (fenêtre Gescom Serveur) |
| « Le pare-feu de Windows n'a pas de règle « Gescom serveur » » (console, journal) | règle retirée ou jamais posée | réinstaller Gescom Serveur, ou les lignes `netsh` du chapitre 5 |
| « Impossible d'écouter sur … » au journal du serveur | un autre Gescom tourne déjà sur l'ordinateur | n'en garder qu'un |

**Les journaux** : `C:\ProgramData\Gescom\serveur.log` (le serveur) ; à
côté de la base pour un ordinateur unique. Ils disent l'heure, ce qui a
été refusé et pourquoi ; jamais de mot de passe.

---

# 15. Ce qui n'a pas encore été essayé en vrai

Ce guide est écrit d'après le programme. N'ont pas encore été faits sur
une vraie machine, par un essai complet : l'installation du service en
administrateur, la fenêtre Gescom Serveur et son accord administrateur,
la recherche du serveur entre deux ordinateurs, le changement de port,
le passage d'un ordinateur unique à plusieurs caisses (chapitre 9 : la
copie et sa vérification sont prouvées par des tests, les boîtes de
l'installeur pas encore vues), et la désinstallation de la caisse et
d'Équipe sans la case. En cas de doute sur l'un d'eux, faire une
sauvegarde d'abord.
