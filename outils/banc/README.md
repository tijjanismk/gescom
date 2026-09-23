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
| `a2-apercu.mjs` | A-2 | l'aperçu d'une pièce s'ouvre au format réglé du genre, sans choix de modèle |
| `a3-atelier.mjs` | A-3 | l'atelier est parti : ni onglet, ni commande ; la Société renvoie à Documents |
| `b1-historique.mjs` | B-1 | un règlement annulé retrouvé par le nom du client ; filtres type / dates / tout effacer ; ouvert depuis une fiche client (puce, retour), une pièce, un article ; l'employé n'a ni menu ni réponse du serveur |
| `b2-journal-poste.mjs` | B-2 | une erreur et une promesse rejetée de la fenêtre arrivent `[POSTE ]` dans le journal du serveur, avec le poste, l'écran et la pile ; une boucle n'envoie qu'une ligne |
| `b3-console-journal.mjs` | B-3 | la console du serveur (`:7300`) montre son journal après identification, les plus récentes en haut ; filtres Caisses / Erreurs / Tout ; une ligne piégée venue d'une caisse s'affiche en texte, rien n'est injecté |
| `b4-anomalies.mjs` | B-4 | le compteur rouge du tableau de bord ouvre l'Historique sur les anomalies à vérifier ; « Marquer vue » la retire, « Vue par Patron le … » s'affiche, le compteur redescend |
| `c4-sessions.mjs` | C-4 | deux navigateurs, deux postes : la session de la caisse B listée avec sa dernière commande ; « Déconnecter » la renvoie à la connexion ; « Désactiver » ferme sa session dans le même geste et le compte ne se reconnecte plus ; réactiver |
| `c1-lectures.mjs` | C-1 | un caissier : ni Journal, ni Rapports, ni Historique, accueil sans chiffres, fiche client sans encours ni état de créance, pas de pièces fournisseur, aucun prix d'achat, cahier refusé ; un comptable : tout, prix d'achat compris |
| `c3-plafonds.mjs` | C-3 | Paramètres → Rôles : plafonds du caissier saisis, relus, 150 % refusé ; la caissière refusée à 40 % (« Demander au patron »), vendue à 10 % ; le patron passe ; sur-mesure à 25 % pour une personne |
| `d2-tous-les-ecrans.mjs` | D-2 | chaque entrée du menu et chaque onglet de Paramètres s'ouvre, servi par `Base` sur SQLite, sans erreur dans la console ni message d'erreur à l'écran |
| `d3-dossiers.mjs` | D-3 | Paramètres → Dossiers : créer avec ses dates de travail (dites en lettres), refus du même code ; se reconnecter dans le nouveau dossier : son client de passage, son magasin, le stock à zéro (articles communs), son exercice |
| `d4-exercices.mjs` | D-4 | un dossier aux dates passées : une dépense refusée avec le remède ; Prolonger (plus court refusé), Clore en deux temps, le lendemain refusé, Ouvrir l'exercice suivant ; la dépense n'est plus refusée pour ses dates ; l'Historique nomme les trois gestes |
