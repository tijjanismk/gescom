# Gescom — Guide d'installation

*Version 1.12 — 28 octobre 2026 — tous les cas d'installation.*

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
| La paie et le suivi des clients | Gescom Équipe, après la caisse du même poste | 7 |
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

1. Clic droit sur `Gescom-Serveur_0.2.0_x64-setup.exe` → **Exécuter en
   tant qu'administrateur**. Windows peut afficher « éditeur inconnu » :
   le certificat est celui de Gescom, pas d'un grand éditeur. Accepter.
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
> retrouvent par « Rechercher le serveur », mais il faut le faire.

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
  technicien ne l'ouvre pas à la main (TCP 40000 à 40100, UDP 40000).

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

Le mot de passe reste dans `C:\ProgramData\Gescom\serveur.json`,
lisible par les administrateurs de cet ordinateur seulement ; il n'est
envoyé nulle part.

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
sauvegarde a besoin de l'adresse du propriétaire dans la variable
d'environnement `GESCOM_PG_SAUVEGARDE` — à vérifier sur place qu'elle
est bien lue par le service.

---

# 7. Gescom Équipe

Gescom Équipe n'a **ni base ni réglage à elle** : elle parle au même
serveur que la caisse **du même poste**, avec les mêmes comptes et les
mêmes droits.

1. **D'abord la caisse**, installée et branchée sur ce poste (chapitre 3
   ou 4).
2. Lancer `Gescom Equipe_0.1.0_x64-setup.exe`.
3. Ouvrir **Gescom Équipe**, se connecter avec son compte habituel,
   choisir le dossier s'il y en a plusieurs.

| Poste | Équipe fonctionne ? |
|---|---|
| Un seul ordinateur (« Ce poste garde la base ») | oui |
| Une caisse branchée sur un serveur | oui |
| L'ordinateur du serveur, avec sa caisse | oui |
| L'ordinateur du serveur, **sans** caisse | oui si le serveur écoute sur 40000 |
| Un poste **sans caisse**, serveur ailleurs | **non** : rien ne permet de lui donner l'adresse. Installer aussi la caisse et la brancher (chapitre 4.2) |

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

> Paramètres → Réseau demande d'être **connecté**. Si une caisse ne
> trouve plus du tout son serveur, l'écran de connexion n'offre pas de
> lien pour en changer : voir le chapitre 14.

---

# 9. D'un seul ordinateur à plusieurs caisses

**Attention** : installer Gescom Serveur sur un ordinateur qui travaille
déjà seul (« Ce poste garde la base ») ne reprend **pas** ses données.
Le service démarre sur une base **neuve et vide** dans `C:\ProgramData\Gescom`,
et la caisse de ce poste s'y branche : elle paraît vide.

La façon sûre — **non encore essayée en vrai, à faire avec un technicien** :

1. Sur l'ordinateur unique : **Paramètres → Sauvegarde → Sauvegarder
   maintenant**. Noter où la copie est rangée. Fermer Gescom.
2. Installer Gescom Serveur sur l'ordinateur qui sera le principal
   (chapitre 4.1).
3. Dans une invite de commandes **en administrateur** sur cet
   ordinateur, arrêter le service, remettre la copie dans **sa** base, le
   relancer :

```
sc stop GescomServeur
"C:\Program Files\Gescom Serveur\gescom-serveur.exe" --restaurer <chemin de la copie> --base C:\ProgramData\Gescom\gescom.db
sc start GescomServeur
```

   La commande vérifie la copie avant de la mettre en place et garde
   l'ancienne base à côté (`…avant-restauration-<date>`). **Ne pas
   oublier `--base`** : sans lui, la copie irait dans la base de
   l'utilisateur Windows, pas dans celle du service.
4. Sur l'ancien ordinateur unique : **Paramètres → Réseau → En faire une
   caisse d'un autre poste**, puis Rechercher le serveur.
5. Vérifier sur une caisse : les dernières ventes, le stock, la caisse
   du jour.

---

# 10. Mettre à jour

**Toujours le même jour, dans cet ordre** : le serveur, puis chaque
caisse, puis Équipe. Faire une sauvegarde avant.

| Programme | Comment | Ce qui est gardé |
|---|---|---|
| Gescom Serveur | relancer le nouvel installeur en administrateur : il arrête le service, remplace, relance | `serveur.json`, la base, les sauvegardes, le journal (la page de configuration ne revient pas) |
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
  le serveur** et doivent le retrouver (Rechercher le serveur).

> À éviter sans nécessité : une caisse déconnectée ne peut pas ouvrir
> Paramètres → Réseau avant de se reconnecter (chapitre 14).

---

# 12. Désinstaller

| Programme | Ce qui part | Ce qui reste |
|---|---|---|
| Gescom Serveur | le service, les règles du pare-feu, les programmes, les raccourcis | **`C:\ProgramData\Gescom`** : la base, `serveur.json`, les sauvegardes, le journal |
| Gescom (caisse) | les programmes, les raccourcis | `AppData\Roaming\ml.gescom.app` — **sauf si on coche la case qui supprime les données de l'application** |
| Gescom Équipe | le programme | son réglage, sauf la même case |

> **Sur un ordinateur unique, ne cochez jamais la case qui supprime les
> données de l'application** en désinstallant Gescom : elle efface la
> base, les sauvegardes et le réglage du poste. Copiez d'abord le
> dossier `sauvegardes` sur une clé USB.

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

**Restaurer** ne se fait pas à l'écran : serveur arrêté, en ligne de
commande, avec `--restaurer <copie>` **et `--base <la base à remplacer>`**
(chapitre 9, étape 3). L'ancienne base est gardée à côté.

---

# 14. Problèmes courants

| Ce qu'on voit | Pourquoi | Que faire |
|---|---|---|
| « éditeur inconnu » / SmartScreen | la caisse n'est pas signée, le serveur l'est par Gescom | « Informations complémentaires » → « Exécuter quand même » |
| « Ce fichier Gescom.exe a été déplacé hors de son dossier d'installation » | le programme a été copié | lancer Gescom depuis le menu Démarrer, ou réinstaller |
| « Le serveur de ce poste ne répond pas » (un seul ordinateur) | le serveur intégré n'a pas démarré ; la raison est écrite | Réessayer ; si « introuvable » : réinstaller Gescom ; si « aucun port libre » : fermer le programme qui occupe 40000 |
| « Serveur injoignable (…) » sur une caisse | serveur arrêté, ordinateur principal éteint, réseau, ou adresse changée | ouvrir **Gescom Serveur** sur le principal : « Le serveur ne tourne pas » → **Démarrer le service** ; vérifier le réseau Privé |
| « Aucun serveur Gescom n'a répondu sur ce réseau » | pare-feu (UDP 40000), autre réseau Wi-Fi, serveur arrêté | taper l'adresse à la main ; vérifier le pare-feu |
| Une caisse ne trouve plus le serveur et on ne peut pas se connecter pour changer l'adresse | l'écran de connexion n'a pas de lien vers le réglage du réseau | remettre le serveur à son ancienne adresse ou son ancien port, se connecter, puis Paramètres → Réseau ; à défaut, un technicien corrige `poste.json` |
| « HORLOGE_RECULE — L'horloge de l'ordinateur qui garde la base recule… » | l'heure de l'ordinateur a été reculée | remettre l'heure juste ; si elle était en avance, le patron confirme dans Paramètres → Horloge |
| « Le … est hors des dates de travail » | l'exercice est fini | le patron le prolonge ou en ouvre un nouveau (manuel d'utilisation) |
| La caisse paraît vide après l'installation du serveur | c'est la base neuve du service | chapitre 9 |
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
le passage d'un ordinateur unique à plusieurs caisses (chapitre 9), et
la désinstallation de la caisse. En cas de doute sur l'un d'eux, faire
une sauvegarde d'abord.
