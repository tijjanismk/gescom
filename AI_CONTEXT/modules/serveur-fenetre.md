# Module : Gescom Serveur — la fenêtre, le panneau Réseau, la recherche

Demande du propriétaire (27/10/2026) : « une interface graphique dédiée à
mon serveur, comme sur Ciel », pour régler l'IP et le port, puis la
découverte automatique. Trois morceaux, une seule page : la console que
le serveur sert déjà ([console.rs](../../src-tauri/serveur/src/console.rs)).
D25 (la coque, jamais construite jusque-là), D12 (le service et sa
relance), D39 (la plage 40000-40100).

## Fichiers

| | rôle |
|---|---|
| [serveur-fenetre/](../../src-tauri/serveur-fenetre/) | crate `gescom-serveur-fenetre`, identifiant `ml.gescom.serveur`, titre « Gescom Serveur ». Sans base ni noyau (constantes de la plage recopiées, vérifiées par `noyau/tests/plage_de_ports.rs`) |
| [serveur-fenetre/src/main.rs](../../src-tauri/serveur-fenetre/src/main.rs) | lit le port dans `serveur.json` (sinon 40000), essaie `/sante` (le port du réglage, puis 40000), navigue vers `http://127.0.0.1:<port>/console` ; commandes `chercher_serveur`, `demarrer_service` ; un fil ramène à la page locale après 3 contrôles manqués (≈ 15 s) |
| [serveur-fenetre/page/](../../src-tauri/serveur-fenetre/page/) | la page LOCALE (`index.html`, `page.js`, `page.css`, `frontendDist`) : « Le serveur ne tourne pas », « Démarrer le service » (`Start-Process sc.exe -Verb RunAs`, l'UAC), « Réessayer », nouvel essai seul toutes les 5 s |
| [noyau/src/coeur/reseau_serveur.rs](../../src-tauri/noyau/src/coeur/reseau_serveur.rs) | les règles pures : `requete_locale`, `juger` (refus `PasDuPosteServeur`, `GereParLaLigneDeCommande`, `PortHorsPlage`, `EcouteInconnue`, `RienNeChange`), `pourquoi_pas`, `fusionner_configuration` (garde toutes les autres clés, refuse un fichier illisible), `adresses_a_saisir` (le nom du poste d'abord), `avertissement_dhcp` |
| [noyau/src/reseau_serveur.rs](../../src-tauri/noyau/src/reseau_serveur.rs) | `noter_changement_sur` : la ligne `reseau_serveur_modifie` (« Port ou écoute du serveur changé », ancien → nouveau « écoute:port ») au journal du dossier ouvert |
| [noyau/src/decouverte.rs](../../src-tauri/noyau/src/decouverte.rs) | le format UDP partagé : `question()` = `{"q":"gescom","v":1}`, `lire_question`, `Annonce` / `reponse` / `lire_reponse` (version 1 seulement), `source_du_reseau_local`, `regrouper` (un serveur par nom + port, le nom conseillé s'il mène à CE serveur), `ServeurTrouve` |
| [serveur/src/decouverte.rs](../../src-tauri/serveur/src/decouverte.rs) | le fil UDP du serveur : toujours le port 40000 (`decouverte::PORT`), réponse aux seules questions bien formées venues du réseau local, 20 par seconde au plus ; port pris → avertissement au journal technique, le serveur continue |
| [serveur/src/configuration.rs](../../src-tauri/serveur/src/configuration.rs) | le chemin de `serveur.json` (`GESCOM_SERVEUR_JSON` le remplace en débogage ; sans elle, un build de débogage **refuse d'écrire**), lecture, écriture sûre (temporaire puis renommage) |
| [serveur/src/api.rs](../../src-tauri/serveur/src/api.rs) | `GET /reseau`, `POST /reseau` |
| [serveur/src/reseau_local.rs](../../src-tauri/serveur/src/reseau_local.rs) | `nom_poste`, `cartes()` (`GetAdaptersAddresses` : IPv4 des cartes en service, origine DHCP, la principale d'abord), `regle_couvre` (port ou plage dans la sortie de `netsh`, quelle que soit la langue), `REGLE_TCP` / `REGLE_UDP` |
| [src/components/RechercheServeur.tsx](../../src/components/RechercheServeur.tsx) | le bouton « Rechercher le serveur » (commande locale `rechercher_serveurs`), dans `PageServeur` et Paramètres → Réseau |
| [src-tauri/src/reseau.rs](../../src-tauri/src/reseau.rs) | `rechercher_serveurs` : diffusion à 255.255.255.255:40000, au /24 du poste et à 127.0.0.1:40000, 2 s d'écoute (la question repart à 1 s), nom résolu en 1,5 s au plus |

## Le panneau Réseau (console, après connexion)

`GET /reseau` (toute session) : nom du poste, cartes et IP (DHCP ou
fixe, principale), port, écoute (`0.0.0.0` toutes les cartes,
`127.0.0.1` ce poste seulement), ce qu'on tape sur une caisse (le nom,
puis les IP ; rien si l'écoute est locale), pare-feu TCP et UDP,
recherche automatique, postes connectés **hors celui qui demande**,
`en_service`, `modifiable` et `pourquoi_pas`.

`POST /reseau {"port", "ecoute"}`, dans cet ordre : jeton (401) →
`parametres:modifier` (403) → un changement à la fois, pas pendant un
redémarrage (409) → corps lisible (400) → `juger` : requête de
127.0.0.1/::1 seulement (403), pas de `--port`/`--hote` sur la ligne de
commande, port de la plage, écoute connue, quelque chose change (409) →
session avec dossier (409) → écriture permise
(débogage : `GESCOM_SERVEUR_JSON`) → le nouveau port s'ouvre (sinon 409
« déjà pris ») → `serveur.json` lisible (sinon refus, jamais écrasé) →
**journal + fichier tout ou rien** (la ligne dans une transaction validée
APRÈS l'écriture ; échec de validation → l'ancien fichier est remis) →
200, réponse envoyée, puis `redemarrage` posé : la boucle s'arrête,
l'UDP est libéré, les sessions révoquées (`redemarrage_serveur`).

**Redémarrer** : en service, `exit(75)` sans `SERVICE_STOPPED` → la
relance de D12 (`sc failure` : 5 s, puis 30 s et 60 s si cela se répète
dans la journée) ; à la main, le serveur relance son exécutable avec les
mêmes arguments et `GESCOM_RELANCE=1` (le nouveau processus réessaie ses
ports 15 s), le journal technique écrit « nouveau processus (pid N) ».
La console attend `/sante` sur le nouveau port puis s'y rouvre.

**D'où vient le port** : en service, tout `serveur.json` ; lancé à la
main **sans `--port` ni `--hote`** (le raccourci de démarrage), seulement
`port` et `hote` de `serveur.json` (la base reste celle du poste, D24) ;
avec `--port`/`--hote` (la fenêtre de caisse monoposte, D39 ; les essais),
le fichier n'est pas lu et le panneau dit « le port est géré par Gescom
sur ce poste ».

## Garde-fous et choix

- La console distante n'a aucune commande Tauri : Tauri 2.11 refuse les
  commandes aux origines distantes, et chaque commande de la coque
  vérifie que l'appel vient de `tauri.localhost`.
- La fenêtre de caisse monoposte cherche aussi le port du service de son
  poste (`reseau.rs::port_du_service`) avant de lancer un serveur : un
  service déplacé sur 40007 n'est pas doublé par un second serveur sur
  une autre base.
- La recherche ne livre rien de la boutique (test qui fige les clés de
  l'annonce). Une réponse peut être usurpée sur le réseau local : la
  caisse **teste** toujours l'adresse avant de l'enregistrer.

## Preuves

- Unitaires : `coeur::reseau_serveur` (9), `decouverte` (9),
  `coeur::historique` (libellé), `serveur::configuration` (écriture
  sûre), `serveur::reseau_local` (`regle_couvre`), `serveur-fenetre`
  (lecture du réglage, candidats).
- Routes ([serveur/tests/routes.rs](../../src-tauri/serveur/tests/routes.rs)) :
  la question UDP sur 127.0.0.1 et la réponse (clés figées), UDP déjà pris
  (avertissement, le serveur sert), panneau en lecture et refus quand la
  ligne de commande décide (employé 403, patron 409), changement accepté :
  `serveur.json` d'essai réécrit avec ses autres clés, hors plage / écoute
  inconnue / rien ne change refusés sans rien écrire, ligne d'Historique
  signée par la session, le serveur relancé répond sur le nouveau port.
  Les essais coupent la recherche (`GESCOM_DECOUVERTE_PORT=0`) sauf ceux
  qui la testent, sur un port libre : jamais l'UDP 40000.
- `noyau/tests/plage_de_ports.rs` : la CSP de la coque énumère la plage,
  ses constantes recopiées suivent.

## Pas vu en vrai (à faire à la main : TESTS-MANUELS, section K)

La fenêtre ouverte, l'UAC de « Démarrer le service », la relance du
service après un changement de port, la diffusion entre deux machines
(et le pare-feu UDP posé par l'installeur), la résolution du nom du poste
depuis une caisse, l'installeur reconstruit avec la fenêtre (WebView2
n'est pas embarqué : présent sur Windows 11 et les Windows 10 à jour).

## Les icônes (28/10/2026)

Gescom Serveur et Gescom Équipe ont chacun leur jeu d'icônes
(`src-tauri/serveur-fenetre/icons/`, `src-tauri/equipe/icons/`) : le logo
de Gescom et une pastille en coin — bleu nuit avec une tour pour le
serveur, verte avec deux personnes pour l'équipe (choix du propriétaire).
La caisse garde `src-tauri/icons/`. Pour les refaire :
`python outils/icones/pastilles.py`, puis
`npx tauri icon outils/icones/<serveur|equipe>.png -o src-tauri/<serveur-fenetre|equipe>/icons`
(ne garder que les fichiers que `tauri.conf.json` cite).
