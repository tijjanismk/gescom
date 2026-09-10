# Module : commandes — caisse et argent

Rôle : la session de caisse, les mouvements du tiroir, les dépenses, la
clôture et les écarts. **Et le garde-fou qui gouverne tout le reste.**

## Fichiers

- [commandes/caisse.rs](../../src-tauri/src/commandes/caisse.rs) (568 l.)
- [utils.rs](../../src-tauri/noyau/src/utils.rs) (44 l.) — porte
  `exiger_session_caisse`, le garde-fou.

## Commandes exposées

| Commande | Écran appelant |
|---|---|
| `lire_resume_caisse` | Caisse |
| `lire_mouvements_caisse_du_jour` | Caisse |
| `ouvrir_session_caisse` | Caisse, ModalOuvrirCaisse |
| `fermer_session_caisse` ([caisse.rs:208](../../src-tauri/src/commandes/caisse.rs#L208)) | Caisse |
| `enregistrer_depense` | Caisse |
| `lire_sessions_caisse` | OngletHistoriqueCaisse |
| `lire_mouvements_session` | OngletHistoriqueCaisse |
| `lire_rapport_ecarts` | OngletHistoriqueCaisse |
| `lire_depenses_du_jour` | **aucun** |
| `modifier_depense` | **aucun** |

## Le garde-fou `CAISSE_FERMEE`

`utils::exiger_session_caisse(&conn) -> Result<String, String>` renvoie
l'id de la session ouverte, ou une erreur commençant par
`CAISSE_FERMEE`. Le front la reconnaît par cette chaîne
(`estCaisseFermee`, dans
[ModalOuvrirCaisse.tsx](../../src/components/ModalOuvrirCaisse.tsx)) et
propose d'ouvrir la caisse.

**16 sites d'appel**, à garder synchronisés — toute nouvelle opération
qui fait bouger un franc doit s'y ajouter :

| Fichier | Lignes |
|---|---|
| [achats.rs](../../src-tauri/src/commandes/achats.rs) | 105, 282, 384, 465 |
| [avoirs.rs](../../src-tauri/src/commandes/avoirs.rs) | 416 |
| [chantiers.rs](../../src-tauri/src/commandes/chantiers.rs) | 165 |
| [creances.rs](../../src-tauri/src/commandes/creances.rs) | 289, 621 |
| [fournisseurs.rs](../../src-tauri/src/commandes/fournisseurs.rs) | 698 |
| [pieces.rs](../../src-tauri/src/commandes/pieces.rs) | 1040, 1073, 1741 |
| [retours.rs](../../src-tauri/src/commandes/retours.rs) | 501, 569 |
| [ventes.rs](../../src-tauri/src/commandes/ventes.rs) | 455, 712 |

## Entrant

Appelle : `coeur::caisse` (solde théorique, écart),
`persistance::journal`.
Appelé par : `pages/Caisse.tsx`,
`components/OngletHistoriqueCaisse.tsx`,
`components/ModalOuvrirCaisse.tsx`.

## Règles métier

- [CONFIRMÉ] **Une seule caisse pour tous les dépôts** :
  `mouvement_caisse` n'a pas de `depot_id`, et ce n'est pas un oubli — il
  y a un tiroir physique unique. C'est ce qui sépare le multi-dépôt du
  multi-magasin (CONTEXT.md).
- [CONFIRMÉ] Le rapprochement porte sur `moyen = 'especes'` seulement, en
  excluant `motif = 'ouverture'` — le fond est déjà porté par
  `session_caisse.fond_ouverture` (D29,
  [coeur/caisse.rs:1](../../src-tauri/noyau/src/coeur/caisse.rs#L1)).
- [CONFIRMÉ] Mobile money et chèques sont tracés en mouvement de caisse
  mais **hors tiroir**. Les inclure créerait un écart fantôme à la
  fermeture.
- [CONFIRMÉ] Refuser vaut mieux qu'écrire à côté : sans session,
  `mouvement_caisse` n'est pas alimenté, `encaisse_jour` (table
  `paiement`) et `caisse_par_moyen` (table `mouvement_caisse`) divergent,
  et la clôture affiche un excédent inexplicable
  ([utils.rs:25-32](../../src-tauri/noyau/src/utils.rs#L25)).
- [DÉDUIT] `modifier_depense` et `lire_depenses_du_jour` sont exposées
  mais jamais appelées — corriger une dépense n'est donc pas faisable
  depuis l'interface, même si le back sait le faire. Vérifier avant de
  supprimer.
