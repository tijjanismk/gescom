# Module : contrôle d'installation

Rôle : Gescom s'installe, il ne se copie pas.

**Ce n'est pas une licence.** Rien à activer, rien à demander à
personne, aucun fichier à recevoir, aucune date d'expiration. Installer
suffit. Le seul comportement ajouté : un `Gescom.exe` posé sur une clé
USB ou copié dans un autre dossier refuse de démarrer.

## Fichiers

| Fichier | Rôle |
|---|---|
| [noyau/src/installation.rs](../../src-tauri/noyau/src/installation.rs) | la vérification |
| [src/garde.rs](../../src-tauri/src/garde.rs) | le refus, avant Tauri |
| [noyau/tests/installation.rs](../../src-tauri/noyau/tests/installation.rs) | le scénario réel |

## Ce sur quoi ça s'appuie

L'installateur NSIS écrit deux traces, vérifiées dans
`target/release/nsis/x64/installer.nsi` :

| Clef | Valeur |
|---|---|
| `HKCU\Software\Gescom\Gescom` (défaut) | le dossier d'installation, **sans** guillemets |
| `HKCU\…\Uninstall\Gescom` → `InstallLocation` | le même, **entre** guillemets |

Au démarrage, le dossier de l'exécutable est comparé à celui-là.

## Règles métier

- [CONFIRMÉ] Le contrôle tourne **avant** `tauri::Builder` — pas de
  fenêtre, pas de base ouverte, pas de plugin chargé. Une copie sur clé
  USB s'arrête sans avoir touché au disque
  ([lib.rs](../../src-tauri/src/lib.rs)).
- [CONFIRMÉ] Le refus s'affiche par `MessageBoxW` de Windows, pas par la
  boîte de Tauri : celle-ci a besoin de la boucle d'événements, qui n'a
  pas démarré. Un refus **silencieux** serait pire que tout — double-clic,
  rien ne se passe, appel du client qui croit ses données perdues
  ([garde.rs](../../src-tauri/src/garde.rs)).
- [CONFIRMÉ] Inactif en build de débogage : sinon `cargo tauri dev` ne
  démarrerait plus, aucun installateur n'étant jamais passé. Le verdict
  est journalisé sur la sortie d'erreur.
- [CONFIRMÉ] Les deux ruches sont lues, `HKCU` puis `HKLM` : l'installateur
  est en mode `currentUser`, mais passer un jour en « pour tous les
  utilisateurs » ne doit pas casser le contrôle chez tout le monde à la
  fois.
- [CONFIRMÉ] `InstallLocation` est écrite **entre guillemets** par NSIS ;
  les garder ferait échouer toute comparaison de chemin.
- [CONFIRMÉ] La comparaison passe par `canonicalize`, avec repli textuel
  insensible à la casse et à l'antislash final : le registre dit
  `C:\Users\…`, l'exécutable `C:\USERS\…`, et refuser pour ça bloquerait
  une installation valide (test `la_casse_et_le_slash_final_ne_comptent_pas`).
- [CONFIRMÉ] `current_exe()` en échec **laisse passer** : bloquer sur un
  cas qu'on ne comprend pas fabriquerait une panne au lieu d'en éviter une.
- [CONFIRMÉ] Chaque refus dit quoi faire, et que les données ne sont pas
  dans le fichier (test `les_messages_disent_quoi_faire`).

## Vérifié sur une vraie machine

| Lancé depuis | Verdict |
|---|---|
| `target\debug\deps` | `Deplacee` → refusé |
| `C:\Users\…\AppData\Local\Gescom` | `Installee` → autorisé |

Le second en copiant temporairement le binaire de test dans le dossier
d'installation réel, sans toucher à `Gescom.exe`.

Pour lire le verdict sur un poste :

```bash
cargo test -p gescom-noyau --test installation -- --nocapture
```

## Ce que ça n'empêche pas

Relancer l'installateur sur dix machines. Démonter l'exécutable pour
retirer le test. Ce n'est pas ce qui est visé : ce qui est empêché,
c'est le **glisser-déposer** — l'employé qui copie un fichier, la clé
USB qui circule.

Une licence liée au poste (signature Ed25519, activation hors ligne,
essai de 30 jours, outil de signature) a été écrite au commit
`38092c1`, puis **retirée** par celui qui a introduit ce module : le
projet ne vend pas encore, et une activation à gérer avant la première
vente coûte plus qu'elle ne rapporte. Le code reste récupérable dans
l'historique.

## Non couvert

`gescom-serveur.exe` n'a pas ce contrôle : il n'est pas empaqueté par
l'installateur NSIS aujourd'hui. À reprendre quand le multiposte se
livrera.
