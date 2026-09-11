# Module : l'installeur

Rôle : livrer les **deux** exécutables, et faire en sorte que le second
soit trouvé et lancé.

## Ce qui manquait

`gescom-serveur.exe` n'était pas empaqueté. Un commerçant qui installait
Gescom obtenait la fenêtre — et rien pour faire tourner plusieurs
caisses. Le serveur se copiait à la main : pas de raccourci, pas de
désinstallation, pas de mise à jour. Un fichier qu'on livre sans que
personne ne sache qu'il existe n'est pas livré.

## Comment il est embarqué

Tauri embarque un exécutable supplémentaire par `externalBin`, qui exige
le **triplet de la cible** en suffixe :

```
src-tauri/binaires/gescom-serveur-x86_64-pc-windows-msvc.exe
```

Le suffixe disparaît à l'installation : le commerçant trouve
`gescom-serveur.exe` à côté de `Gescom.exe`.

[`outils/preparer_serveur.ps1`](../../outils/preparer_serveur.ps1)
compile en release et copie sous ce nom. **Il tourne avant
`npm run tauri build`** — sans lui, la compilation du crate applicatif
échoue sur `resource path … doesn't exist`.

Le dossier `src-tauri/binaires/` est ignoré par git : il se régénère.

### Deux pièges PowerShell, tous deux rencontrés

- `$ErrorActionPreference = "Stop"` fait échouer le script sur la
  première ligne que cargo écrit sur stderr — « Compiling tauri » en
  fait partie. On vérifie `$LASTEXITCODE`, qui dit la vérité.
- `cargo-tenace.ps1 build --release -p gescom-serveur` : PowerShell lit
  `-p` comme un **nom de paramètre**, cargo reçoit `build --release`
  seul, et compile tout l'espace de travail — dont le crate applicatif,
  qui exige le binaire qu'on est en train de fabriquer. D'où
  `-Arguments @("build", "--release", "-p", "gescom-serveur")`.

## Les raccourcis

[`src-tauri/nsis/hooks.nsh`](../../src-tauri/nsis/hooks.nsh), déclaré par
`bundle.windows.nsis.installerHooks`.

- **Menu Démarrer** — « Gescom Serveur », à côté de l'application.
- **Démarrage de la session** — un serveur qu'il faut penser à lancer
  chaque matin est un serveur qui ne tournera pas : les caisses
  afficheraient « Serveur injoignable » et personne ne ferait le lien
  avec la fenêtre noire qu'on a oublié d'ouvrir.

Dans le démarrage de **l'utilisateur**, pas en service Windows : un
service demanderait les droits administrateur que cet installeur n'a pas
(`installMode: currentUser`) et un enveloppeur de service en plus. Le
poste principal d'une boutique reste allumé sur une session ouverte.

À la **désinstallation**, `taskkill` sur le serveur avant de retirer les
fichiers : tant qu'il tourne, il tient la base ouverte et la
désinstallation échoue avec un message incompréhensible.

## Ce que l'installeur ne fait pas

### Le pare-feu

`installMode: currentUser` — pas de droits administrateur, donc aucune
règle de pare-feu n'est posée. Elle échouerait en silence, et **laisser
croire que le réseau est ouvert est pire que ne rien faire**.

Le serveur affiche lui-même la commande à lancer, à chaque démarrage,
tant que la règle manque (`reseau_local.rs`).

### La signature

L'installeur n'est **pas signé** — décision assumée pour l'instant. Un
certificat auto-signé n'enlèverait pas l'avertissement : Windows vérifie
la chaîne de confiance, pas la présence d'une signature. Voir
[environnement-windows.md](environnement-windows.md).

## La taille : 267 Mo

`webviewInstallMode: offlineInstaller` embarque le runtime WebView2
complet — **258 Mo sur les 267**. C'est cohérent avec un produit
local-first destiné à des postes sans Internet garanti, et c'est parfait
sur une clé USB.

Pour un téléchargement, `downloadBootstrapper` ramènerait l'installeur
à une vingtaine de méga-octets, au prix d'une connexion obligatoire
pendant l'installation. C'est un arbitrage commercial, pas technique.

## Vérifier ce qui est réellement livré

```bash
7z l target/release/bundle/nsis/Gescom_1.12.0_x64-setup.exe
```

`strings` ne montre rien : NSIS compresse sa charge utile en LZMA.

Mesuré sur la version 1.12.0 — 9 fichiers, dont :

| fichier | taille |
|---|---|
| `gescom.exe` | 16,4 Mo |
| `gescom-serveur.exe` | **5,1 Mo** |
| `MicrosoftEdgeWebView2RuntimeInstaller.exe` | 258,6 Mo |

Le binaire release du serveur a été démarré sur une base neuve : il
amorce, annonce les comptes créés, répond sur `/sante` et délivre un
jeton.
