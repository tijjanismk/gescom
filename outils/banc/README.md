# Le banc d'écran

Des parcours Playwright qui cliquent dans le **vrai écran** (servi par
Vite) contre le **vrai serveur** (sur une base SQLite jetable, données
de démo). Ce que les scénarios Rust ne voient pas : un bouton qui
n'enregistre rien, un aperçu qui ne suit pas le réglage.

```bash
cd outils/banc && npm install          # Playwright, une fois
./lancer.sh neuf                       # serveur :7300 + écran :1420
node a1-documents.mjs                  # un parcours
npm run tout                           # tous, dans l'ordre
```

## Sous Windows : les deux installations de la v3

```powershell
cd outils\banc ; npm install ; cd ..\.. ; npm install      # Playwright, et Vite pour l'écran
$env:CHROMIUM = "C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe"   # Edge déjà là, rien à télécharger
.\outils\banc\lancer.ps1 monoposte  -Neuf   # la vraie fenêtre Tauri, seule (elle lance son serveur)
node outils\banc\suite.mjs p                # p1 … p6, dans la fenêtre
.\outils\banc\lancer.ps1 multiposte -Neuf   # serveur sur PostgreSQL gescom_test, chaque navigateur un poste
node outils\banc\suite.mjs                  # a1 … i3 (le banc ci-dessous, sur PostgreSQL)
node outils\banc\suite.mjs m                # m2 … m4, deux postes à la fois
.\outils\banc\lancer.ps1 sqlite -Neuf       # comme lancer.sh : serveur SQLite
.\outils\banc\lancer.ps1 arreter
```

- **monoposte** — la boutique à une caisse telle qu'elle s'installe :
  `gescom.exe` seul (build de débogage), piloté par le port de débogage
  de WebView2 (`:9222`). Au premier démarrage il demande le rôle du
  poste ; « Ce poste garde la base » : il lance lui-même son serveur
  SQLite (`127.0.0.1:7300`) et s'y branche (D22/D24 révisés le
  25/10/2026). `GESCOM_DONNEES` (lu **seulement en débogage**, par la
  fenêtre et par le serveur) et un profil WebView2 à part tiennent tout
  dans `.travail/monoposte` : la base et la session du propriétaire
  (`%APPDATA%\ml.gescom.app`) ne sont pas ouvertes — `p1` le vérifie.
- **multiposte** — le lanceur **refuse toute base qui n'est pas
  `gescom_test`** ; `-Neuf` y fait `DROP SCHEMA` puis pose la démo
  avec `examples/amorcer --demo` (sur PostgreSQL, le serveur ne la
  pose pas lui-même). Ne pas lancer en même temps que des `cargo test`
  sur `gescom_test`.
- `suite.mjs` joue tout sans s'arrêter au premier échec, garde chaque
  sortie dans `.travail/suite/` et fait le bilan ; `i2` est sauté
  (et dit) sans `pdftoppm`.
- Vite tourne avec `vite.banc.config.ts` : la config du projet, sans
  surveiller `.travail` (le profil WebView2 y a des fichiers verrouillés).

| parcours | mode | ce qu'il prouve |
|---|---|---|
| `p1-premier-demarrage.mjs` | monoposte | la fenêtre neuve demande le rôle (`poste.json` « à choisir ») ; « garde la base » : elle lance son serveur (aucun ne tournait) et arrive à la connexion, sans adresse ; `poste.json` en `monoposte` sur 127.0.0.1 ; patron connecté, un seul dossier, rien à choisir ; cache vidé : rien n'est redemandé ; la base du propriétaire intacte |
| `p2-tous-les-ecrans.mjs` | monoposte | chaque menu et chaque onglet de Paramètres s'ouvre dans WebView2 sans erreur ; Réseau dit « ce poste garde la base », serveur ici |
| `p3-argent.mjs` | monoposte | caisse fermée : vente refusée, stock intact ; caisse ouverte **par l'écran** (10 000 F) ; vente comptant ×2 : stock −2, caisse + prix ; crédit puis règlement : créance aller-retour, argent entré ; fermeture à écart 0 |
| `p4-sauvegarde-auto.mjs` | monoposte | la copie automatique part seule après le démarrage (20 s ici, `GESCOM_SAUVEGARDE_DELAI_S`), nommée à la seconde, une vraie base ; Paramètres → Sauvegarde la montre |
| `p5-serveur-relance.mjs` | monoposte | le serveur arrêté : à sa réouverture la fenêtre le relance seule, retour à la connexion (les sessions d'avant sont fermées), même base ; le port 7300 pris par un autre programme : l'écran le dit, sans connexion qui échouerait, « Réessayer » repart une fois la place libre |
| `p6-role-caisse.mjs` | les deux | un poste neuf dit « caisse » : branchement essayé puis enregistré, même empreinte ; le serveur lancé comme par le raccourci (sans `--base`) s'arrête sur la caisse d'une autre machine, journal à l'appui, sans créer de base — et tourne sur « caisse de 127.0.0.1 » ; Paramètres → Réseau : la caisse redevient « garde la base » (confirmation), la fenêtre rejoint son serveur |
| `m2-deux-caisses.mjs` | multiposte | deux postes, deux comptes, en même temps : 12 devis sans numéro en double ; 10 ventes du même article (5 + 5) : stock −10 lu pareil des deux côtés, caisse commune + 10 × prix |
| `m3-un-poste-l-autre-voit.mjs` | multiposte | crédit sur A ; la comptable sur B lit la créance et encaisse 300 F ; A relit la créance et la caisse ; A ferme la caisse : B refusé, la créance ne bouge pas |
| `m4-deux-dossiers.mjs` | multiposte | deux dossiers ouverts en même temps sur deux postes : une caisse chacun, clients séparés, articles communs mais stock à soi, deux numérotations |

## Le banc d'origine (serveur SQLite, `lancer.sh`)

Chaque parcours affiche `ok` / `ECHEC` par vérification et sort en
erreur au premier échec. Base, journaux et captures : `.travail/`
(ignoré par git). `CHROMIUM=/chemin/chrome` pour un navigateur déjà
installé.

| parcours | étape | ce qu'il prouve |
|---|---|---|
| `a1-documents.mjs` | A-1 | Paramètres → Documents : en-tête, coordonnées, réglage, cachet, exemple en direct, rechargement, usine |
| `a2-rendu.mjs` | A-2 | chaque genre rendu par le vrai générateur avec les vraies données : colonnes, récap TVA, lettres, signatures, cachet, coordonnées, ticket, reçu fournisseur |
| `a2b-apercu.mjs` | A-2 | l'aperçu d'une pièce s'ouvre au format réglé du genre, sans choix de modèle — après `a2-rendu`, qui pose les réglages et les pièces |
| `a3-atelier.mjs` | A-3 | l'atelier est parti : ni onglet, ni commande ; la Société renvoie à Documents |
| `b1-historique.mjs` | B-1 | un règlement annulé retrouvé par le nom du client ; filtres type / dates / tout effacer ; ouvert depuis une fiche client (puce, retour), une pièce, un article ; l'employé n'a ni menu ni réponse du serveur |
| `b2-journal-poste.mjs` | B-2 | une erreur et une promesse rejetée de la fenêtre arrivent `[POSTE ]` dans le journal du serveur, avec le poste, l'écran et la pile ; une boucle n'envoie qu'une ligne |
| `b3-console-journal.mjs` | B-3 | la console du serveur (`:7300`) montre son journal après identification, les plus récentes en haut ; filtres Caisses / Erreurs / Tout ; une ligne piégée venue d'une caisse s'affiche en texte, rien n'est injecté |
| `b4-anomalies.mjs` | B-4 | le compteur rouge du tableau de bord ouvre l'Historique sur les anomalies à vérifier ; « Marquer vue » la retire, « Vue par Patron le … » s'affiche, le compteur redescend |
| `c1b-droits-relus.mjs` | — | une session du navigateur sans `rapports:lire` (comme avant la v3) : au rechargement les droits sont relus, le tableau de bord chiffré revient sans reconnexion, la session est réécrite ; Équipe de même |
| `c4-sessions.mjs` | C-4 | deux navigateurs, deux postes : la session de la caisse B listée avec sa dernière commande ; « Déconnecter » la renvoie à la connexion ; « Désactiver » ferme sa session dans le même geste et le compte ne se reconnecte plus ; réactiver |
| `c1-lectures.mjs` | C-1 | un caissier : ni Journal, ni Rapports, ni Historique, accueil sans chiffres, fiche client sans encours ni état de créance, pas de pièces fournisseur, aucun prix d'achat, cahier refusé ; un comptable : tout, prix d'achat compris |
| `c2-droits-dossiers.mjs` | C-2 | Paramètres → Utilisateurs → Dossiers : le frère patron chez lui seulement, la comptable comptable ici et caissière là ; le frère entre sans choisir, sans Utilisateurs ni Sauvegarde, ses clients seulement ; la comptable choisit parmi deux, le menu suit le rôle du dossier |
| `c3-plafonds.mjs` | C-3 | Paramètres → Rôles : plafonds du caissier saisis, relus, 150 % refusé ; la caissière refusée à 40 % (« Demander au patron »), vendue à 10 % ; le patron passe ; sur-mesure à 25 % pour une personne |
| `d2-tous-les-ecrans.mjs` | D-2 | chaque entrée du menu et chaque onglet de Paramètres s'ouvre, servi par `Base` sur SQLite, sans erreur dans la console ni message d'erreur à l'écran |
| `d3-dossiers.mjs` | D-3 | Paramètres → Dossiers : créer avec ses dates de travail (dites en lettres), refus du même code ; se reconnecter dans le nouveau dossier : son client de passage, son magasin, le stock à zéro (articles communs), son exercice |
| `d4-exercices.mjs` | D-4 | un dossier aux dates passées : une dépense refusée avec le remède ; Prolonger (plus court refusé), Clore en deux temps, le lendemain refusé, Ouvrir l'exercice suivant ; la dépense n'est plus refusée pour ses dates ; l'Historique nomme les trois gestes |
| `d5-dossier-d-origine.mjs` | D-5 | le dossier au nom d'usine invite à être nommé ; « Le nommer » (vide refusé), même code, la barre suit après rechargement, l'Historique dit le renommage ; rendu à « Ma boutique » par le crayon, l'invitation revient |
| `e1-plan-comptable.mjs` | E-1 | Paramètres → Comptabilité : 134 comptes rangés par classe, recherche « caisse » → 571, sous-compte client ajouté sous 411 et marqué « ce dossier », doublon et hors-plan refusés, le comptable a la permission, l'Historique le dit |
| `e2-affectations.mjs` | E-2 | 28 opérations au défaut (espèces 571, ventes 701, loyer 622) ; la liste des espèces ne propose que la trésorerie ; espèces sur un sous-compte de caisse, tenu après rechargement, retour au défaut ; l'Historique le dit |
| `e3-journaux.mjs` | E-3 | une vente et un loyer du jour ; Rapports → Journaux comptables : VT, AC, RG, CA équilibrés, le loyer sur 622, ventes du journal = CA du cahier ; filtre RG ; Exporter CSV : nom, BOM, en-tête, lignes RG, fichier équilibré |
| `f1-fenetre-equipe.mjs` | F-1 | Gescom Équipe (`equipe.html`) : le patron voit Personnel, Paie, Suivi clients ; un module à venir le dit ; la caissière (mot de passe changé d'abord) ne voit que le suivi client ; le magasinier « rien pour l'instant » ; déconnexion |
| `f2-personnel.mjs` | F-2 | Équipe → Personnel : Awa au mois sans contrat, Moussa à la journée, Fanta à la commission avec son compte ; compte déjà lié refusé ; modifier ; départ (motif) et retour ; Issa gère sans la paie : voit comment, pas combien |
| `f3-jours-travailles.mjs` | F-3 | Personnel → Jours travaillés, mois dernier : « tous présents » le 3, une case qui tourne P → ½ → A avec le total qui suit (2, 1,5, 1), une absence que « tous présents » ne défait pas ; demain ne s'ouvre pas |
| `g1-avances.mjs` | G-1 | Équipe → Paie → Avances, hors caisse : caisse fermée, l'avance se donne quand même ; au-delà du plafond refusé avec les chiffres ; 15 000 F en cours ; une avance annulée reste visible, marquée ; la caisse du jour n'est pas touchée |
| `g2-fiches-paie.mjs` | G-2 | Équipe → Paie → Fiches du mois : Moussa, ses jours × 2 500 F ; 3 livraisons ajoutées ; une retenue sans motif refusée ; validée avec un numéro, plus rien ne s'ajoute ; Awa, 60 000 F moins son avance ; une rectificative (l'avance pas retenue deux fois, + une prime) ; l'ancienne reste, « remplacée par » |
| `g3-verser-bulletin.mjs` | G-3 | une fiche validée payée en deux fois, hors caisse (20 000 en espèces, le reste en Orange Money), le reste suit sur la fiche et dans la liste, trop refusé, « payée » ; le bulletin : titre, numéro, ligne, net en lettres, versements, deux signatures ; Gescom → Documents → Bulletin de paie : pas de rouleau, l'exemple s'affiche |
| `g4-cotisations-journal.mjs` | G-4 | Équipe → Paie → Cotisations : vides par défaut ; INPS salariale 3,6 % et patronale 16,4 % plafonnée à 80 000 ; 150 % refusé ; Kadia déclarée 96 400 net et 13 120 de charge à part, bulletin avec la retenue, la charge et le n° INPS ; Awa non déclarée 60 000 ; Rapports → Journaux → Paie : PA équilibré sur 661 / 422 / 431 / 664 ; Rapports → CA mensuel : salaires du mois et « après salaires » |
| `h1-echanges-360.mjs` | H-1 | Équipe → Suivi clients : un client trouvé par la recherche, sa fiche 360 (chiffres, échanges) ; une relance de créance déjà là se retrouve dans le même fil qu'un nouvel échange noté (genre, quoi, suite prévue) — pas une liste de plus |
| `h2-rappels-prospects.mjs` | H-2 | Un prospect créé (nom, téléphone, origine), sa fiche marquée « prospect » ; un rappel posé dessus pour le patron apparaît dans « Mes rappels », se marque fait ; sa première vente le rend client tout seul, sans bouton à chercher |
| `i1-achat-sans-fournisseur.mjs` | — | Achats sans fournisseur (correctif du 25/09) : « À crédit » fermé et l'écran dit pourquoi ; la confirmation dit « Fournisseur divers », payé comptant ; une FAF numérotée, payée, de 800 F ; choisir « Fournisseur divers » dans la liste ne rouvre pas le crédit |
| `i2-pages-repetees.mjs` | — | Pièces comme dans Word (25/09) : une facture de 70 lignes imprimée en PDF, chaque page lue pixel par pixel — en-tête en haut et pied en bas, bord à bord, filigrane au milieu ; facture + bon de sortie de même ; « Pour acquit » souligné ; le filigrane réglé par Paramètres → Documents (mot, intensité) et relu. Demande `pdftoppm` |
| `i3-sauvegarde.mjs` | — | Paramètres → Sauvegarde lit le dossier du serveur : vide, en alerte ; après une copie (nommée à la seconde), la date en clair et la liste |
| `j1-client-de-passage.mjs` | D35 | une vente au « Comptant » payée en partie : refusée, le stock ne bouge pas ; payée en entier : sa fiche de passage `PAS-…`, la facture du ticket à elle (« Client de passage ») ; Clients : pas dans la liste, « Passages » la retrouve par le numéro du ticket ; « Passer en client ordinaire » : il entre dans la liste sous son nom, sa facture émise garde « Client de passage » |
