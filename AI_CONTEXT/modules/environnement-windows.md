# Environnement : Windows — contrôle d'application, pare-feu, PowerShell

Trois pièges de la machine, pas du code. Leurs messages d'erreur
désignent autre chose que la cause.

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
