# Module : modèles de documents

Rôle : le moteur de génération des pièces. Un modèle est une **donnée**
(du JSON en base), pas du code — le commerçant change sa mise en page
sans attendre une version de Gescom.

Couvre cinq genres : `facture`, `bon_sortie`, `recu_paiement`,
`releve_creance`, `journal_caisse`.

## Où vit quoi, et pourquoi

| Couche | Fichier | Rôle |
|---|---|---|
| Stockage | [noyau/src/modeles.rs](../../src-tauri/noyau/src/modeles.rs) | CRUD, actif, export/import. Ne rend **rien**. |
| Table | [persistance/v2.rs](../../src-tauri/noyau/src/persistance/v2.rs) | `modele_document` + index unique de l'actif |
| Façade Tauri | [commandes/modeles.rs](../../src-tauri/src/commandes/modeles.rs) | 8 commandes, dont l'écriture fichier |
| Façade serveur | [serveur/src/socle.rs](../../src-tauri/serveur/src/socle.rs) | les mêmes, en RPC — la voie de déploiement |
| Modèle | [lib/modeles/types.ts](../../src/lib/modeles/types.ts) | blocs, champs, catalogue des chemins |
| Rendu | [lib/modeles/rendu.ts](../../src/lib/modeles/rendu.ts) | modèle + données → HTML |
| Données | [lib/modeles/contexte.ts](../../src/lib/modeles/contexte.ts) | normalise les formes des écrans |
| Usine | [lib/modeles/defauts.ts](../../src/lib/modeles/defauts.ts) | les 6 modèles livrés |
| Service | [lib/modeles/service.ts](../../src/lib/modeles/service.ts) | semis, images, impression |
| Atelier | [pages/Modeles.tsx](../../src/pages/Modeles.tsx) | l'éditeur glisser-déposer |

**Le rendu est en TypeScript, pas en Rust.** L'aperçu de l'éditeur se
redessine à chaque frappe : un rendu côté serveur imposerait un
aller-retour par caractère, ou un second moteur pour l'aperçu — et deux
moteurs finissent par ne plus donner le même document.

**Le stockage est en Rust, pas en TypeScript.** C'est ce qui permet au
serveur v2 de distribuer les modèles à toutes les caisses.

## Le modèle

Une suite de **blocs** empilés, pas un canevas libre en x/y : sur un
document commercial, ce qui doit bouger c'est l'ordre et le contenu.
Un canevas libre laisserait poser un total à cheval sur le pied de page.

Dix types : `entete`, `titre`, `champs`, `tableau`, `totaux`, `texte`,
`signatures`, `trait`, `espace`, `saut_page`.

Chaque bloc lit ses données par un **chemin** (`piece.numero`,
`totaux.total_ttc`, `journal.ecart`). Le chemin est une chaîne : c'est
ce qui permet d'ajouter un champ sans toucher au code. Le catalogue des
chemins connus est dans `types.ts` — un chemin absent du catalogue reste
saisissable à la main.

Les blocs `texte` et `titre` interpolent `{{chemin}}`.

## Règles métier

- [CONFIRMÉ] **Un seul modèle actif par genre**, garanti par un index
  unique partiel en base et non seulement par l'écran : deux actifs et
  le document imprimé dépendrait de l'ordre de lecture (test
  `activer_desactive_lautre`).
- [CONFIRMÉ] Le **premier** modèle d'un genre devient actif tout seul ;
  le second ne lui vole pas la place (tests
  `le_premier_modele_dun_genre_devient_actif`,
  `le_second_ne_vole_pas_la_place_du_premier`).
- [CONFIRMÉ] Supprimer l'actif **désigne le suivant** — sans repli,
  l'impression retomberait en silence sur le générateur historique
  (test `supprimer_lactif_designe_le_suivant`).
- [CONFIRMÉ] Un modèle d'usine (`est_defaut = 1`) **ne se supprime
  pas** : il faut un chemin de retour après une mise en page ratée
  (test `un_modele_dusine_ne_se_supprime_pas`).
- [CONFIRMÉ] L'import **n'écrase pas le choix du modèle actif** : il
  appartient au poste qui reçoit. Importer le lot d'une autre boutique
  ne change pas la facture qui sort de votre imprimante (test
  `importer_ne_change_pas_le_modele_actif_du_poste`).
- [CONFIRMÉ] Un fichier sans le marqueur `gescom-modeles`, ou d'une
  version d'échange plus récente, est **refusé** (tests
  `un_fichier_etranger_est_refuse`, `un_fichier_trop_recent_est_refuse`).
- [CONFIRMÉ] Un modèle mal formé dans un lot **n'annule pas les
  autres** : le bilan dit lequel manque (test
  `un_modele_casse_ne_fait_pas_echouer_le_lot`).
- [CONFIRMÉ] `assurerModelesParDefaut` n'écrase **jamais** un modèle
  existant, même d'usine : une mise à jour de Gescom ne doit pas
  reprendre la main sur la mise en page du commerçant
  ([service.ts](../../src/lib/modeles/service.ts)).
- [CONFIRMÉ] Les images (logo, en-tête, pied) ne sont **pas** dans le
  modèle : ce sont celles de la société. Les y mettre gonflerait
  l'export de plusieurs centaines de ko et exporterait le logo d'une
  boutique vers une autre.
- [DÉDUIT] Le bon de sortie ne se greffe pas sur un modèle dans
  `ModalImpression` — la case disparaît dès qu'un modèle est choisi.
  Le modèle `std-bon-sortie` existe pour ça, mais l'impression des
  **deux en un seul document** n'a pas été portée.

## Déploiement — les deux voies

1. **Par le serveur (v2).** Les modèles sont en base ; le poste
   principal les distribue. Un modèle corrigé est vu par toutes les
   caisses au rechargement suivant. C'est le vrai déploiement.
2. **Par fichier.** `Exporter` écrit un `.json` indenté (marqueur +
   version + modèles). `Importer` le reprend. Sert à passer d'une
   *installation* à une autre — clé USB, nouvelle boutique — pas d'un
   poste à l'autre du même magasin.

## Ce que ça ne remplace pas encore

`genererPDF.ts` (964 l.) reste le générateur par défaut : les formats
A4/A5/thermique historiques impriment comme avant. Les modèles
s'ajoutent dans `ModalImpression` sous « Mes modèles » et ne sont
utilisés que si le commerçant les choisit. Bascule volontaire — le
générateur historique imprime la même facture depuis des mois, et le
jour du changement doit être choisi après avoir vu son modèle à l'écran.

`genererRecu.ts`, `genererReleve.ts` et l'écran Journal ne sont **pas**
encore branchés sur le moteur : les contextes existent
(`contexteRecu`, `contexteReleve`, `contexteJournal`), les modèles
d'usine aussi, mais aucun écran n'appelle `imprimerParModele` pour ces
trois genres. C'est le raccordement suivant, et il est mécanique.

## Tests

[noyau/tests/modeles.rs](../../src-tauri/noyau/tests/modeles.rs) — 12
scénarios : l'actif unique, le repli à la suppression, la protection des
modèles d'usine, l'aller-retour d'export et les trois refus d'import.
