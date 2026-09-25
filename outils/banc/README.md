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
