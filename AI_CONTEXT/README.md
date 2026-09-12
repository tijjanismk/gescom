# AI_CONTEXT — carte du projet

Un index à charger **à la place** du code, pour trois questions qu'aucun
fichier ne répond seul : où vit une fonctionnalité, qui casse si je la
modifie, ce fichier est-il vivant. Les conventions de code sont dans
[CLAUDE.md](../CLAUDE.md) à la racine — c'est lui qu'on lit en premier.

## Quoi charger, selon la tâche

Ne charge pas tout : le dossier pèse ~40 k tokens, une tâche en a besoin
de 5 à 8 k.

| je vais… | charger |
|---|---|
| corriger n'importe quoi | [ALERTES.md](ALERTES.md) — deux arbres du même nom, lequel ouvrir |
| savoir où en est le projet | [ETAPES.md](ETAPES.md) (une page) ; l'historique daté est dans [JOURNAL.md](JOURNAL.md), à ne pas charger |
| toucher à l'argent (vente, facture, règlement, caisse) | [modules/coeur.md](modules/coeur.md), [modules/commandes-argent.md](modules/commandes-argent.md), [DOMAINE.md](DOMAINE.md) §Argent — puis **ouvrir `noyau/src/argent.rs`** |
| toucher aux pièces (devis → facture, avoirs) | [modules/commandes-pieces.md](modules/commandes-pieces.md), [modules/livraison-stock.md](modules/livraison-stock.md), [modules/numerotation.md](modules/numerotation.md) |
| toucher au stock, aux magasins, aux transferts | [modules/commandes-stock-depots.md](modules/commandes-stock-depots.md), [modules/livraison-stock.md](modules/livraison-stock.md) |
| toucher aux achats, fournisseurs, dettes | [modules/commandes-achat-fournisseur.md](modules/commandes-achat-fournisseur.md) |
| toucher aux clients, créances, retours, chèques | [modules/commandes-vente-client.md](modules/commandes-vente-client.md) |
| écrire ou porter du SQL, toucher à `Base` | [modules/postgresql.md](modules/postgresql.md) seul |
| toucher au serveur, aux sessions, à la console | [modules/reseau-v2.md](modules/reseau-v2.md) |
| toucher aux droits | [modules/permissions.md](modules/permissions.md) |
| toucher à un écran React | [modules/front-pages.md](modules/front-pages.md), [modules/front-composants.md](modules/front-composants.md) ; impression : [modules/front-lib.md](modules/front-lib.md) |
| toucher aux modèles de documents | [modules/modeles-documents.md](modules/modeles-documents.md) |
| installer, empaqueter, un binaire bloqué (4551) | [modules/installation.md](modules/installation.md), [modules/installeur.md](modules/installeur.md), [modules/environnement-windows.md](modules/environnement-windows.md) |
| comprendre une règle avant de la changer | [DOMAINE.md](DOMAINE.md) (règles avec `fichier:ligne`), [DECISIONS.md](DECISIONS.md) (D1…D11 + le récap en fin) |
| le multi-société (v3) | [PLAN-MULTISOCIETE.md](PLAN-MULTISOCIETE.md) |

Vue d'ensemble en une page : [ARCHITECTURE.md](ARCHITECTURE.md).

## Ce que la carte ne remplace pas

Le code source. Pour une condition, un ordre d'écriture ou une
transaction — `valider_facture_sur_base`, `enregistrer_achat_sur_base`,
`annuler_reglement_sur_base` — ouvrir le fichier dans `noyau/src/`. Un
résumé n'a jamais permis de voir qu'une écriture était faite avant le
garde-fou censé la refuser.

Les décisions **D1…D51** de la v1 vivent dans [CONTEXT.md](../CONTEXT.md)
et ne sont pas recopiées ; celles de la v2 (D1…D11) dans
[DECISIONS.md](DECISIONS.md).

## Régénérer

```bash
python <chemin>/carte.py . --json AI_CONTEXT/_genere/carte.json
```

`_genere/carte.json` (~50 k tokens) **ne se lit pas** : c'est la
matière d'[ALERTES.md](ALERTES.md), qu'on réécrit à la main après avoir
comparé au JSON précédent. Sous Git Bash, ne pas passer
`--alias '@/=src/'` : le shell le réécrit et tout paraît orphelin.

Le graphe ne voit que les imports statiques ; le pont React ↔ Rust est
une chaîne (`invoke("nom")`) et se vérifie par `grep`.

Régénéré le 12/09/2026 (214 fichiers).
