# DeepSeek context — reprendre le projet sans relire la conversation

Ce dossier est un **instantané** du projet Gescom au 13/09/2026, écrit
pour qu'un autre agent (DeepSeek ou autre) reprenne le travail là où
cette séance l'a laissé, sans accès à la conversation.

Trois fichiers :

| fichier | répond à |
|---|---|
| [ETAT-PRESENT.md](ETAT-PRESENT.md) | où en est le projet, quel environnement, quels pièges |
| [PLAN.md](PLAN.md) | ce qui reste à faire, dans l'ordre, et qui peut le faire |
| [RESTE.md](RESTE.md) | ce qui restera APRÈS le plan — hors de portée, décisions à prendre |

## Comment s'en servir

Ce dossier n'est **pas** une copie de la documentation : la source de
vérité reste [CLAUDE.md](../CLAUDE.md) (règles de code non négociables),
[AI_CONTEXT/](../AI_CONTEXT/) (la carte du projet) et
[CONTEXT.md](../CONTEXT.md) (décisions D1…D51 de la v1).

Ordre de lecture pour un agent qui débarque :

1. [CLAUDE.md](../CLAUDE.md) à la racine — avant d'écrire une ligne.
2. Ce dossier (les trois fichiers).
3. Selon la tâche : `AI_CONTEXT/README.md` indique quelle fiche charger
   (`modules/*.md`), et `AI_CONTEXT/ALERTES.md` avant toute correction.

En cas de conflit entre ce dossier et `AI_CONTEXT/`, **`AI_CONTEXT` gagne** :
il est mis à jour à chaque séance, ce dossier est un instantané figé.
