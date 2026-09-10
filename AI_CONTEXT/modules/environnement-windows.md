# Environnement : Windows — contrôle d'application, pare-feu, PowerShell

Trois pièges de la machine, pas du code. Ils font perdre du temps parce
que leurs messages d'erreur désignent autre chose que la cause.

## 1. Smart App Control bloque les binaires fraîchement liés

### Le symptôme

```
error: test failed, to rerun pass `-p gescom --lib`
Caused by:
  could not execute process `…\target\debug\deps\gescom-bb42dbdb…exe`
  (never executed)
Caused by:
  Une stratégie de contrôle d'application a bloqué ce fichier. (os error 4551)
```

Cargo dit « test failed ». **Aucun test n'a échoué** — le binaire n'a
même pas démarré. C'est là qu'on perd une demi-heure à chercher une
régression qui n'existe pas.

### La preuve

Mesuré sur le poste de développement, pas supposé :

| Où | Valeur |
|---|---|
| `HKLM\SYSTEM\CurrentControlSet\Control\CI\Policy` → `VerifiedAndReputablePolicyState` | **1** = activé, en application |
| Journal `Microsoft-Windows-CodeIntegrity/Operational` | événements **3118** (« Smart App Control Block Details »), **3077** et **3033** nommant l'exécutable |
| Politique citée | `{0283ac0f-fff1-49ae-ada1-8a933130cad6}` — celle de SAC |
| Blocages en 24 h | **23** |

Pour vérifier soi-même :

```powershell
reg query "HKLM\SYSTEM\CurrentControlSet\Control\CI\Policy"
Get-WinEvent -FilterHashtable @{
  LogName='Microsoft-Windows-CodeIntegrity/Operational'; Id=3077
} -MaxEvents 5 | Format-List TimeCreated, Message
```

⚠️ **Correction d'un diagnostic antérieur.** Plus tôt dans le
développement, Smart App Control avait été mis hors de cause après
qu'une copie du fichier bloqué se soit exécutée normalement. C'était
une mauvaise conclusion : SAC décide par réputation, et une copie
fraîche peut passer là où l'original échoue. Le journal d'événements
tranche, l'essai empirique non.

### Pourquoi c'est intermittent

SAC refuse ce qu'il ne connaît pas. Un binaire que le compilateur vient
de produire n'a, par construction, aucune réputation. Relier produit un
fichier différent, qui passe presque toujours — d'où l'impression d'un
aléa.

### Le contournement, sans toucher au système

```powershell
.\outils\cargo-tenace.ps1 test --workspace
.\outils\cargo-tenace.ps1 build --release
```

[cargo-tenace.ps1](../../outils/cargo-tenace.ps1) relance cargo, et sur
un blocage supprime le fichier que cargo a nommé avant de recommencer
— quatre fois au plus (`GESCOM_ESSAIS` pour changer). Un échec qui
n'est pas un blocage est rendu tel quel, sans rien effacer.

Vérifié dans les deux cas : le passage nominal, et **la reprise elle-même**
— un blocage est survenu pendant le portage des commandes, le script a
supprimé le binaire, relancé, et la suite est passée sans intervention.

### La vraie décision, qui n'est pas technique

Smart App Control est conçu pour une machine qui n'exécute que des
applications signées et connues. Un poste qui compile ses propres
binaires est l'exact contraire. Le garder, c'est accepter ces blocages
indéfiniment.

**Le désactiver est irréversible** : Windows ne permet de repasser de
« désactivé » à « activé » que par une réinstallation. C'est pour cela
que ce dépôt ne le fait pas et ne le recommande pas à la légère — c'est
au propriétaire de la machine de trancher.

### ⚠️ Le même mécanisme touchera les clients

`Gescom_1.12.0_x64-setup.exe` n'est **pas signé**. Sur un Windows 11
récent où Smart App Control est actif ou en évaluation, l'installeur
peut être bloqué de la même façon — chez le commerçant, sans
explication utilisable pour lui.

C'est un problème de distribution, pas de développement, et il a la
même racine. Le seul remède est un certificat de signature de code. À
chiffrer avant les premières ventes : sans lui, chaque installation
dépend de l'humeur de SmartScreen et de SAC sur la machine d'en face.

## 2. Le pare-feu bloque les caisses, sans rien écrire

### Le symptôme

La caisse affiche « Serveur injoignable ». Le serveur ne voit rien
passer — pas de connexion refusée, pas de ligne de journal, rien. Les
deux machines se croient en panne l'une de l'autre.

### Ce que Windows fait tout seul, et pourquoi ça ne suffit pas

Au premier lancement de `gescom-serveur.exe`, Windows propose une boîte
de dialogue. Si quelqu'un clique « Autoriser », une règle est créée —
mais mesurée sur ce poste, elle vaut :

```
Inbound Allow  profils=Public
  …	arget\debug\gescom-serveur.exe
```

Deux défauts, chacun suffisant :

- **profil Public seulement.** Un réseau de boutique est classé
  *Privé*. La règle ne s'applique donc pas là où on en a besoin.
- **attachée au chemin de l'exécutable.** Déplacer ou réinstaller le
  serveur perd la règle, sans avertissement.

### Le remède

```powershell
.\outils\parefeu.ps1 -Etat      # sans droits particuliers
.\outils\parefeu.ps1 -Ouvrir    # PowerShell EN ADMINISTRATEUR
.\outils\parefeu.ps1 -Fermer
```

[parefeu.ps1](../../outils/parefeu.ps1) pose une règle sur le **port**,
pour les profils **Domaine et Privé**. Elle survit au déplacement de
l'exécutable et couvre le cas réel : un réseau local de boutique.

Le profil **Public est volontairement laissé de côté** — c'est celui
des réseaux où l'on ne connaît pas ses voisins, un hôtel, un cybercafé.
Y ouvrir la base d'un commerce serait un mauvais service.

`-Etat` montre aussi les règles posées par Windows, pour qu'on ne croie
pas la situation réglée parce qu'il y en a une.

### Le serveur le dit lui-même

Depuis
[reseau_local.rs](../../src-tauri/serveur/src/reseau_local.rs), chaque
démarrage affiche l'adresse à saisir sur les caisses et, si la règle
manque, la commande exacte pour la poser :

```
  pour les caisses : 192.168.100.177:7300   ← l'adresse à saisir…

  ⚠ PARE-FEU : aucune règle « Gescom serveur » n'autorise le port 7300.
```

L'adresse vient d'une socket UDP ouverte vers une adresse de
documentation, **sans rien envoyer** : cela suffit à faire choisir à
Windows l'interface qu'il utiliserait. Énumérer les cartes donnerait
aussi les adaptateurs VMware, Hyper-V et Bluetooth — et le commerçant
recopierait la mauvaise. C'est exactement ce qui s'est produit pendant
la mise au point : quatre adresses virtuelles listées, la vraie absente.

## 3. PowerShell 5.1 lit un `.ps1` sans BOM comme de l'ANSI

Un script contenant des accents et enregistré en UTF-8 **sans** BOM
produit des erreurs de syntaxe qui ne désignent pas la vraie ligne :

```
Le terminateur ' est manquant dans la chaîne.
Accolade fermante « } » manquante dans le bloc d'instruction.
```

`n'est` devient deux caractères, l'apostrophe ouvre une chaîne qui ne se
ferme jamais, et l'analyseur signale une accolade trente lignes plus
haut.

Tout `.ps1` de ce dépôt doit donc être écrit en **UTF-8 avec BOM** :

```python
p.write_bytes(b"\xef\xbb\xbf" + contenu.encode("utf-8"))
```

Corollaire du même travers : ne pas donner à un script un second
paramètre positionnel après un `ValueFromRemainingArguments`.
PowerShell attribue la première position au second paramètre, et
`cargo-tenace.ps1 test` se fait lire comme un nombre d'essais valant
« test ». D'où le réglage par variable d'environnement.
