# Module : comptabilité (v3, chantier E — D23)

Gescom tient le commerce ; le comptable ressaisissait chaque mois ses
ventes, achats et caisse. Le chantier E pose une **fondation** : le plan
en base, quelle opération va sur quel compte, et des journaux **lus**
(rien n'est stocké — une écriture enregistrée à côté de la vente serait
une seconde vérité).

| Couche | Fichier | Rôle |
|---|---|---|
| Règles pures | [coeur/plan_comptable.rs](../../src-tauri/noyau/src/coeur/plan_comptable.rs) | `SYSCOHADA` (≈130 comptes usuels, classes 1 à 7), `classe`, `parent_de` (plus long préfixe), `valider_sous_compte` |
| Base | [plan_comptable.rs](../../src-tauri/noyau/src/plan_comptable.rs) | `semer_sur` (idempotent, `ON CONFLICT DO NOTHING`), `lire_sur`, `libelle_sur`, `ajouter_sous_compte_sur` (journal `sous_compte_cree`) |
| Migration | `amorcage::migrations_de_donnees` | sème le plan et donne `comptabilite:gerer` au comptable (une fois) — `amorcer` et `serveur::main` (fichier) |
| Affectations (pures) | [coeur/affectations.rs](../../src-tauri/noyau/src/coeur/affectations.rs) | `OPERATIONS` (28 : trésorerie par mode de paiement, ventes, achats, écarts de caisse, dépenses par catégorie), chacune son **défaut** et ses **préfixes** permis ; `cle_tresorerie(mode)`, `cle_depense(categorie)`, `verifier` |
| Affectations (base) | [affectations.rs](../../src-tauri/noyau/src/affectations.rs) | `compte_sur`, `table_sur`, `lire_sur`, `definir_sur` (journal `affectation_modifiee`) — la table `affectation_comptable` (cloisonnée) ne garde que ce qui diffère du défaut |
| Écran | [components/OngletComptabilite.tsx](../../src/components/OngletComptabilite.tsx) | Paramètres → Comptabilité (`comptabilite:gerer`) : le plan par classe, recherche, nouveau sous-compte |

**Table** `compte_comptable (numero, dossier_id, libelle, classe, parent,
origine, cree_le)`, clé `(numero, dossier_id)`. `dossier_id = ''` : le
plan commun ; sinon un sous-compte du dossier (`4111 Client Coulibaly`).
**Pas cloisonnée** (le commun se lit de partout) : chaque lecture filtre
`dossier_id IN ('', dossier courant)`. Un sous-compte : 4 à 12 chiffres,
classe 1 à 7, un parent dans le plan, jamais un numéro déjà visible.

Commandes : `lire_plan_comptable` (ouverte), `ajouter_sous_compte`,
`lire_affectations`, `definir_affectation` (`comptabilite:gerer`).

**Affectation** : une opération absente de `affectation_comptable` vaut
son défaut livré — le défaut ne s'écrit jamais, pour qu'une correction
de défaut dans une version suivante profite à qui ne l'a pas touché.
Un compte doit exister (plan ou sous-compte du dossier) et convenir à
l'opération (une vente en `70…`, une trésorerie en `5…`).

Preuves : unitaires (le plan tient debout, parent, sous-compte),
`plan_comptable_base.rs` (4, SQLite, PostgreSQL et compte limité),
banc `e1-plan-comptable.mjs` ; unitaires des affectations (chaque
opération a un compte du plan qui lui convient, chaque mode de
paiement sa trésorerie), `affectations_base.rs` (3, trois moteurs),
banc `e2-affectations.mjs`.
