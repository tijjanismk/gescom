# AI_CONTEXT — carte du projet

Index destiné à être chargé **à la place** de la lecture du code, pour
répondre à trois questions qu'aucun fichier ne répond seul : où vit une
fonctionnalité, qui casse si je la modifie, ce fichier est-il vivant.

Ordre de lecture conseillé :

1. [ARCHITECTURE.md](ARCHITECTURE.md) — une page, la stack et le pont
   `invoke`.
2. [ALERTES.md](ALERTES.md) — code mort, doublons, commandes injoignables,
   fichiers les plus sollicités. **À lire avant de corriger quoi que ce
   soit.**
3. [DOMAINE.md](DOMAINE.md) — les règles métier avec leur `fichier:ligne`.
4. `modules/` — une fiche par module.

| Fiche | Couvre |
|---|---|
| [coeur](modules/coeur.md) | règles pures et testées |
| [persistance](modules/persistance.md) | schéma, migrations, journal |
| [commandes-argent](modules/commandes-argent.md) | caisse, et le garde-fou `CAISSE_FERMEE` |
| [commandes-vente-client](modules/commandes-vente-client.md) | POS, créances, retours, avoirs, relances, chèques |
| [commandes-achat-fournisseur](modules/commandes-achat-fournisseur.md) | achats, dettes, TVA, irrécouvrable |
| [commandes-pieces](modules/commandes-pieces.md) | cycle documentaire, `valider_facture` |
| [commandes-stock-depots](modules/commandes-stock-depots.md) | dépôts, transferts, catalogue, codes-barres |
| [commandes-systeme](modules/commandes-systeme.md) | auth, réglages, rapports, sauvegarde |
| [front-pages](modules/front-pages.md) | les 18 écrans, l'état global d'`App.tsx` |
| [front-composants](modules/front-composants.md) | modales, onglets, primitives shadcn |
| [front-lib](modules/front-lib.md) | génération des documents imprimables |
| [reseau-v2](modules/reseau-v2.md) | **v2** — serveur, sessions, canal, caisse par utilisateur |
| [modeles-documents](modules/modeles-documents.md) | **v2** — moteur de documents, atelier, import/export |
| [installation](modules/installation.md) | **v2** — Gescom s'installe, il ne se copie pas |
| [postgresql](modules/postgresql.md) | **v2** — la façade `Base`, le trait `Acces`, les 186 commandes portées, le filet `schema_commun` |
| [environnement-windows](modules/environnement-windows.md) | Smart App Control, PowerShell — les pièges de la machine |

## Ce que la carte ne remplace pas

Le code source. Pour une condition, un ordre d'écriture ou une
transaction — `valider_facture`, `enregistrer_achat`, `annuler_reglement`
— ouvrir le fichier. Un résumé n'a jamais permis de voir qu'une écriture
était faite avant le garde-fou censé la refuser.

Les décisions numérotées **D1…D51** vivent dans
[CONTEXT.md](../CONTEXT.md) et ne sont pas recopiées ici. Cette carte y
renvoie.

## Régénérer

```bash
python <chemin>/carte.py . --json AI_CONTEXT/carte.json --md
```

L'alias par défaut du script (`@/=src/`) est déjà le bon.

⚠️ Sous Git Bash, **ne pas** passer `--alias '@/=src/'` explicitement :
la conversion de chemins MSYS le réécrit en `@C:/Program Files/Git/=src/`,
aucun import ne résout, et le script rapporte 69 orphelins au lieu de 11.
Si le nombre d'orphelins explose, c'est ça.

Ensuite : comparer `carte.json` à l'ancien et ne réécrire que les fiches
dont les symboles ou les dépendances ont bougé.

Le graphe ne voit que les imports statiques. Le pont React ↔ Rust est une
chaîne de caractères (`invoke("nom")`) et n'y figure pas : les tables
commande → écran des fiches `commandes-*` ont été construites par `grep`
et doivent être refaites de même.

Généré le 2026-09-09 sur le commit `eb359f0`. 115 fichiers, 43 117 lignes
de source (1,71 Mo) ; les fiches pèsent 68 ko, soit **4 %**.
