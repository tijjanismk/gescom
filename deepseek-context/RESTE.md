# Ce qui restera APRÈS le plan

Ce qui est hors de portée du plan (décisions qui ne sont pas du code,
chantier v3, dette accumulée) et qu'on retrouvera une fois les items
1…11 faits.

## La v3 (multi-société, multi-dossier, exercices) — le gros morceau

Fondation posée : colonne `dossier_id` sur les 23 tables cloisonnées,
`Base` porte son dossier, détecteur qui refuse **avant d'exécuter** toute
requête qui oublie le filtre, tables `dossier` et `exercice`, règle des
dates pure et testée.

Reste : les écrans (choisir un dossier, ouvrir un exercice, clôturer),
la migration d'une base existante vers plusieurs dossiers, et le
déclencheur de stock multi-dossier sur SQLite (item 10 du plan).
→ `AI_CONTEXT/PLAN-MULTISOCIETE.md`

## Décisions qui ne sont pas du code

- **Signature de l'installeur** (D5) : achat d'un certificat de
  signature de code. En attendant, chaque installation dépend de
  SmartScreen — et le même mécanisme (SAC) peut bloquer les clients.
- **Impression papier réelle** : jamais vérifiée sur imprimante (item 9
  du plan, il faut le matériel).

## Dette technique héritée de la v1

⚠️ Liste tirée de `CONTEXT.md` (mis à jour le 06/09/2026, ère v1) :
**vérifier avant de s'y atteler** qu'un chantier v2 ne l'a pas déjà
réglée — `AI_CONTEXT/ETAPES.md` et les fiches `modules/*.md` sont la
source de vérité plus récente. Les plus notables :

- **Pas de restauration dans l'application** : le contrôle d'intégrité
  dit « restaurer la dernière sauvegarde », aucun bouton ne le fait.
  Depuis que les sauvegardes partent vraiment (VACUUM INTO / pg_dump),
  c'est la moitié manquante du filet.
- **Les routes HTTP du serveur ne sont pas testées automatiquement** :
  les scénarios couvrent le noyau, pas `serveur/src/api.rs`. La
  vérification à la main de `POST /entretien` a attrapé le 13/09/2026
  un interblocage réel (`sauvegarde::dossier` reprend le verrou de la
  base que le gestionnaire tenait déjà) que rien n'aurait signalé.
  Un test de route — même un seul, qui démarre le serveur et joue une
  commande — vaudrait cher.
- **Les commandes Tauri n'ont aucun test** — premier endroit où en
  ajouter. Priorité nommée : `creer_vente`, `valider_facture`,
  `regler_dette_fournisseur` (les trois commandes qui écrivent des
  créances et de la caisse).
- **Un chèque rejeté ne défait pas son mouvement de caisse** : l'entrée
  `moyen = 'cheque'` écrite à l'encaissement reste. Le correctif propre
  est un mouvement INVERSE, pas une suppression — reste à décider si un
  rejet exige une caisse ouverte (D46).
- **Bon de livraison partiel** : le suivi gère le partiel, le document
  non — la conversion copie toutes les lignes à quantité pleine.
- `lire_fournisseurs_pagines` construit son WHERE par `format!()` — à
  passer en paramètres liés.
- `ModalImpression` fait doublon avec `ApercuPiece` ; codes-barres non
  dessinés (on imprime le numéro) ; pièces historiques en `validee`
  (migration possible `UPDATE … SET statut='paye'`, seulement si toutes
  soldées).

## Le rituel de fin de séance (pour ne pas l'oublier)

Une ligne dans `AI_CONTEXT/ETAPES.md`, une section datée dans
`AI_CONTEXT/JOURNAL.md`, la fiche `AI_CONTEXT/modules/*.md` touchée si
une signature ou une règle a bougé. Commits en français, un par lot,
message = le pourquoi. (CLAUDE.md « Après une séance »)
