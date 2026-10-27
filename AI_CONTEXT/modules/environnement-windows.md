# Environnement : Windows — contrôle d'application, pare-feu, PowerShell

Les pièges de la machine, pas du code. Leurs messages d'erreur
désignent autre chose que la cause. Le § 7 sépare le banc d'écran de
l'essai à la main.

## 1. Smart App Control bloque les binaires fraîchement liés

**Symptôme** — `cargo test` dit « test failed » :
```
could not execute process `…\target\debug\deps\gescom-….exe` (never executed)
Une stratégie de contrôle d'application a bloqué ce fichier. (os error 4551)
```
**Aucun test n'a échoué** : le binaire n'a pas démarré. SAC refuse ce
qu'il ne connaît pas ; un binaire que le compilateur vient de produire
n'a aucune réputation. Relier produit un fichier différent, qui passe
presque toujours — d'où l'impression d'un aléa. Vérifier :
`reg query "HKLM\SYSTEM\CurrentControlSet\Control\CI\Policy"` →
`VerifiedAndReputablePolicyState = 1` ; journal
`Microsoft-Windows-CodeIntegrity/Operational`, événements 3077/3118.
Le journal tranche ; un essai empirique (« la copie passe ») non.

**Remède, sans toucher au système** :
```powershell
.\outils\cargo-tenace.ps1 test --workspace
.\outils\cargo-tenace.ps1 build --release
```
[cargo-tenace.ps1](../../outils/cargo-tenace.ps1) relance cargo et, sur
un blocage, supprime le fichier nommé avant de recommencer (4 essais,
`GESCOM_ESSAIS`). Un échec qui n'est pas un blocage est rendu tel quel.
Depuis bash : relancer après `touch` du fichier de test concerné.
⚠️ il avale `-p` et `--` : `--package`, et les scénarios PostgreSQL
avec `cargo` direct.

**La vraie décision** : désactiver SAC est **irréversible** (retour
possible seulement par réinstallation) — c'est au propriétaire de la
machine de trancher, ce dépôt ne le fait pas. Et **le même mécanisme
touchera les clients** : l'installeur n'est pas signé (D5) ; le seul
remède durable est un certificat de signature de code.

## 2. Le pare-feu bloque les caisses, sans rien écrire

**Symptôme** — une caisse reçoit « Serveur injoignable », rien dans le
journal du serveur. Windows pose des règles à sa manière, aucune ne
couvre le port. **Remède** :
```powershell
.\outils\parefeu.ps1 -Etat      # sans droits
.\outils\parefeu.ps1 -Ouvrir    # PowerShell EN ADMINISTRATEUR
```
[parefeu.ps1](../../outils/parefeu.ps1) pose une règle sur le **port**,
profils Domaine et Privé ; **Public volontairement exclu** (hôtel,
cybercafé). Le serveur affiche au démarrage l'adresse à saisir et, si la
règle manque, la commande exacte ([reseau_local.rs](../../src-tauri/serveur/src/reseau_local.rs)).
L'adresse vient d'une socket UDP ouverte sans rien envoyer : énumérer
les cartes listait quatre adaptateurs virtuels et pas la vraie. Vérifié
depuis un second appareil le 11/09/2026.

Depuis le 27/10/2026, une **seconde règle**, « Gescom serveur
(recherche) » : **UDP 40000** entrant, mêmes profils, pour « Rechercher
le serveur » depuis les caisses (`parefeu.ps1 -Ouvrir`, l'installeur du
serveur). Sans elle, la caisse dit « Aucun serveur n'a répondu » et l'on
saisit l'adresse à la main. Le panneau Réseau de la console montre les
deux règles ; `reseau_local::regle_couvre` lit la sortie de `netsh` par
ses nombres (port ou plage), pas par ses intitulés traduits — l'ancienne
lecture ne voyait pas 40057 dans « 40000-40100 ».

## 3. PowerShell 5.1 lit un `.ps1` sans BOM comme de l'ANSI

Un script avec des accents en UTF-8 **sans** BOM donne « Le
terminateur ' est manquant » trente lignes plus haut que la faute :
`n'est` devient deux caractères, l'apostrophe ouvre une chaîne. Tout
`.ps1` du dépôt est en **UTF-8 avec BOM**
(`b"\xef\xbb\xbf" + contenu.encode("utf-8")`). Corollaire : pas de
second paramètre positionnel après un `ValueFromRemainingArguments` —
`cargo-tenace.ps1 test` lisait « test » comme un nombre d'essais ; d'où
la variable d'environnement.

## 4. Un serveur qui tourne verrouille le sidecar : `tauri build` échoue

**Symptôme** — `npm run tauri dev`/`build` meurt sur
`error: failed to run custom build command for `gescom v0.1.0``,
panique `Os { code: 5, kind: PermissionDenied, message: "Accès
refusé." }` à `tauri-build-…/src/lib.rs:80` (`fs::remove_file(&dest)`).
**Ce n'est pas SAC** — c'est le serveur : un `gescom-serveur.exe`
lancé verrouille la copie sidecar `target\debug\gescom-serveur.exe`,
que tauri-build remplace à chaque build et ne peut pas supprimer.

**Remède** : arrêter le serveur (`Stop-Process` sur son PID, vérifié le
13/09/2026 : PID 26956, `target\debug`) avant de builder l'appli. Le
release sur un autre port ne gêne pas le build — le verrou est sur le
fichier `target\debug\gescom-serveur.exe`, pas sur le port.

**L'envers, sans erreur (27/10/2026)** : quand rien ne tourne, la copie
réussit — et `target\debug\gescom-serveur.exe` devient le **vieux**
sidecar `src-tauri\binaires\gescom-serveur-x86_64-pc-windows-msvc.exe`
(20/09/2026, 10 Mo). Ça arrive à la première construction de `gescom`
(même un `cargo check`) après un changement de `tauri.conf.json` : le
script de construction repasse. Un essai lancerait alors un serveur
d'avant la v3. **Remède** : construire `gescom` **avant**
`gescom-serveur` ; un `cargo-tenace build --package gescom-serveur`,
même sans rien à recompiler, remet le bon fichier (vérifié : 16,8 Mo,
le build du jour). `lancer.ps1 monoposte` et `essai-main.ps1` le font
dans cet ordre.

## 5. `cargo test` ne relie pas `target\debug\gescom-serveur.exe`

Après une modification de `serveur/src/main.rs`, `cargo test
--workspace` compile le binaire en **harnais de test**
(`target\debug\deps\gescom_serveur-<hash>.exe`) mais laisse le binaire
nu `target\debug\gescom-serveur.exe` tel quel — la date du fichier
tranche (vérifié le 13/09/2026 : un essai CLI a tourné un binaire
d'avant la modification et s'est comporté comme si l'option
n'existait pas). **Remède** : `.\outils\cargo-tenace.ps1 build
--package gescom-serveur` avant tout essai de la ligne de commande du
serveur.

**Et l'inverse** (vu le 27/10/2026) : construire ou même `cargo check`
la fenêtre `gescom` recopie le serveur embarqué (`binaires/`,
`externalBin`) PAR-DESSUS `target\debug\gescom-serveur.exe`, avec sa
date d'origine — cargo croit ensuite le binaire à jour, et `routes.rs`
tourne l'ancien serveur. Après un `check --package gescom`, toucher
`serveur/src/main.rs` (ou `build --package gescom-serveur`) avant les
tests du serveur.

## 6. Trois bases PostgreSQL sur cette machine, à ne jamais confondre

PostgreSQL local sur `127.0.0.1:5432` (`psql` dans
`C:\Program Files\PostgreSQL\18\bin`). Le mot de passe ne s'écrit pas
ici : il est local, et D10 interdit un secret dans le dépôt.

| base | usage |
|---|---|
| `gescom` | **la base du serveur**. Jamais `GESCOM_PG` dessus : les scénarios font `DROP SCHEMA public CASCADE` |
| `gescom_test` | jetable, pour les tests automatisés |
| `gescom_essai` | jetable, pour les essais à la main (serveur + clics) |

Le serveur d'essai tourne sur le **port 40000** (plage 40000-40100, D39) ; une instance
`gescom.exe` (la fenêtre Tauri) peut tourner en même temps — vérifier
avant d'ouvrir un serveur sur ce port. Pour que les autres postes le
joignent : `outils/parefeu.ps1 -Ouvrir` (admin). Les comptes de la base
d'essai exigent un changement de mot de passe à la première connexion ;
`outils/caisse_pg.py` sait le rejouer.

## 7. Le banc et l'essai à la main : deux dossiers, deux marques

**Symptôme (27/10/2026)** — `lancer.ps1 monoposte -Neuf` a effacé
`outils/banc/.travail/monoposte`, sauvegardes comprises, sans corbeille :
c'était aussi le dossier de l'essai à la main du propriétaire (scripts de
travail hors git, port 7300).

**Depuis le lot E.14** (détail : [outils/banc/README.md](../../outils/banc/README.md)) :

| | dossier (sous `outils/banc/.travail/`) | `.proprietaire` | port du serveur |
|---|---|---|---|
| banc monoposte (`lancer.ps1 monoposte`) | `monoposte/` (profil WebView2 dans `webview/`) | `banc` | 40000 |
| essai à la main (`essai-main.ps1`, suivi par git) | `essai-main/` (idem) | `essai-main` | 40050, écrit dans son `poste.json` |

- Chacun ne travaille **que** dans un dossier absent (créé, marqué) ou
  qui porte sa marque, `-Neuf` ou non ; sinon refus, avec ce qu'il faut
  faire. Le banc refuse **avant** d'arrêter quoi que ce soit.
- `-Neuf` du banc efface son dossier ; `-Neuf` de l'essai le **met de
  côté** (`essai-main.mis-de-cote-<date>`), jamais d'effacement.
- La première ouverture de l'essai **copie** `.travail/monoposte`
  (l'original n'est pas touché ; refus si un serveur tient cette base).
  Faite le 27/10/2026 (9 fichiers, 7,5 Mo ; tailles vérifiées, bases
  et `-wal` identiques au SHA-256, source inchangée).
  Le `.travail/monoposte` d'avant n'a pas de marque : le banc le refuse
  jusqu'à ce qu'on le déplace ou l'efface **à la main**.
- Ils partagent l'écran Vite `:1420` (`devUrl`, figé au build) : le banc
  arrête ce qui y écoute ; la fenêtre de l'essai perd son écran, pas ses
  données. Ne pas jouer les deux à la fois.
- Les `.ps1` du banc sont en UTF-8 **avec** BOM (§ 3) ; `lancer.ps1` ne
  l'avait pas, ajouté le 27/10.

## 8. Les variables d'essai du serveur (27/10/2026)

Lues **en débogage seulement** (un exécutable livré les ignore) :

| variable | effet |
|---|---|
| `GESCOM_SERVEUR_JSON` | remplace `%ProgramData%\Gescom\serveur.json` (serveur et fenêtres). Sans elle, un serveur de débogage **refuse d'écrire** ce fichier : le panneau Réseau dit pourquoi |
| `GESCOM_DECOUVERTE_PORT` | le port UDP de « Rechercher le serveur » ; `0` la coupe. `routes.rs` la coupe partout sauf dans les deux tests qui l'éprouvent, sur un port libre : aucun essai ne prend l'UDP 40000 |

`GESCOM_RELANCE=1` est posée par le serveur lui-même sur le processus
qu'il relance après un changement de port (il réessaie ses ports 15 s) ;
on ne la pose pas à la main.
