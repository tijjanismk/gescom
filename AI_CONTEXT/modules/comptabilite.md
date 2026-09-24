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
| Écran | [components/OngletComptabilite.tsx](../../src/components/OngletComptabilite.tsx) | Paramètres → Comptabilité (`comptabilite:gerer`) : le plan par classe, recherche, nouveau sous-compte |

**Table** `compte_comptable (numero, dossier_id, libelle, classe, parent,
origine, cree_le)`, clé `(numero, dossier_id)`. `dossier_id = ''` : le
plan commun ; sinon un sous-compte du dossier (`4111 Client Coulibaly`).
**Pas cloisonnée** (le commun se lit de partout) : chaque lecture filtre
`dossier_id IN ('', dossier courant)`. Un sous-compte : 4 à 12 chiffres,
classe 1 à 7, un parent dans le plan, jamais un numéro déjà visible.

Commandes : `lire_plan_comptable` (ouverte), `ajouter_sous_compte`
(`comptabilite:gerer`).

Preuves : unitaires (le plan tient debout, parent, sous-compte),
`plan_comptable_base.rs` (4, SQLite, PostgreSQL et compte limité),
banc `e1-plan-comptable.mjs`.
