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
