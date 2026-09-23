# Module : les traces (v3, chantier B)

Rôle : savoir **qui a fait quoi** (le journal métier, lu par
l'Historique) et **ce qui s'est mal passé** (le journal technique du
serveur, les erreurs des caisses, les anomalies). Aucune nouvelle
écriture métier : le `journal` s'écrit partout depuis la v1, le
chantier B le fait lire.

## L'Historique (B-1)

| Couche | Fichier | Rôle |
|---|---|---|
| Règle | [coeur/historique.rs](../../src-tauri/noyau/src/coeur/historique.rs) | `TYPES` (type → libellé français, ordre du filtre), `libelle_type` (un type inconnu reste lisible), `borner_page` (1…200), `jour_filtre` (une date illisible est refusée, pas ignorée) |
| Lecture | [historique.rs](../../src-tauri/noyau/src/historique.rs) | `lire_historique_sur(acces, Filtre)` paginé, le plus récent d'abord ; `filtres_sur` (types + personnes qui ont réellement agi) |
| Commandes | [serveur/src/socle.rs](../../src-tauri/serveur/src/socle.rs) | `lire_historique`, `lire_filtres_historique` — **nées sur `Base`**, permission `journal:lire` (la première permission de lecture) |
| Écran | [pages/Historique.tsx](../../src/pages/Historique.tsx) | filtres Du / Au / Personne / Type / Recherche, puce du contexte (« Client : … »), pagination de 50 |
| Accès | `Layout.tsx` (menu), `FicheClient`, `FicheFournisseur`, `Pieces` (icône par ligne), `Stock` (au survol) | bouton « Historique » → `App.ouvrirHistorique(filtre, retour)` ; absent sans `journal:lire` |

**« Sur quoi » se résout à la lecture**, par sous-requêtes sur clé
primaire depuis `(entite_type, entite_id)` : le tiers (client ou
fournisseur), la pièce, l'article. Couverts : `client`, `fournisseur`,
`vente`, `paiement`, `ligne_vente`, `piece_commerciale`/`piece`,
`paiement_fournisseur`, `retour`, `avoir`, `cheque_recu`, `article`.
Un `transfert` est journalisé par son **bon** (plusieurs articles) :
il ne se rattache à aucun article — une sous-requête à plusieurs
lignes ferait refuser PostgreSQL. Rien n'est recopié à l'écriture :
une fiche renommée se lit sous son nom d'aujourd'hui.

La **recherche** porte sur le nom du tiers, le numéro de pièce,
l'article, l'auteur et le détail enregistré (`ancien_valeur`,
`nouveau_valeur`), en `LOWER(…) LIKE LOWER(…)`. Un filtre vide ou
fait d'espaces ne filtre pas.

L'écran ne garde que la **dernière** réponse (numéro de requête) :
deux filtres changés coup sur coup lançaient deux lectures, et la plus
lente écrasait la bonne — trouvé par le banc. Les identifiants
internes (UUID) du détail ne s'affichent pas.

**Rétention** : rien ne s'efface. Index `idx_journal_date`
(`date_evenement`) et `idx_journal_entite`.

Preuves : `noyau/tests/historique_base.rs` (5 scénarios, deux
moteurs : un règlement annulé retrouvé par le nom du client, les
filtres, la pagination sans perte ni doublon, la permission, le
détecteur) ; `serveur/tests/routes.rs::l_historique_se_lit_avec_journal_lire_et_nomme_qui_a_vendu`
(JSON de l'écran, auteur nommé, refus de l'employé) ; banc
`b1-historique.mjs` (23 vérifications).

## Les erreurs des caisses (B-2)

| Couche | Fichier | Rôle |
|---|---|---|
| Fenêtre | [lib/pont.ts](../../src/lib/pont.ts) | `installerRemonteeErreurs` (appelé par `main.tsx` avant le premier rendu) : `window.onerror` + `unhandledrejection` → `signalerErreur` ; `noterPage` (posé par `App`) ; rien sans session ; la même erreur une fois par 10 s ; un envoi raté est avalé (pas de boucle) |
| Route | [serveur/src/api.rs](../../src-tauri/serveur/src/api.rs) `journal_poste` | `POST /journal-poste` : jeton exigé (401), 4 Ko au plus (413), 10 par minute et par poste (429 au-delà, **une** ligne `AVERT` par minute le dit, le reste est jeté sans trace) |
| Règle | [serveur/src/journal_poste.rs](../../src-tauri/serveur/src/journal_poste.rs) | `Limiteur` (minute glissante), `nettoyer` (une seule ligne : un `\n` venu de la fenêtre fabriquerait une fausse ligne `[ERREUR]` ; caractères de contrôle retirés ; longueur bornée), `ligne` |
| Journal | [serveur/src/journal_technique.rs](../../src-tauri/serveur/src/journal_technique.rs) | niveau `POSTE` : `… [POSTE ] ip POST /journal-poste · CAISSE-1 · ventes · TypeError: … · pile : …` |

Pas de télémétrie, pas de clics : seulement les erreurs. Preuves : 3
unitaires (`journal_poste`), route
`les_erreurs_d_une_caisse_arrivent_au_journal_et_la_onzieme_est_jetee`,
banc `b2-journal-poste.mjs`.

## Le journal technique dans la console (B-3)

`GET /journal?n=200&niveau=ERREUR|REFUS|AVERT|POSTE|INFO|tout` —
jeton exigé, permission **`sauvegarde:lancer`** (comme la sauvegarde
et l'entretien : c'est la console du serveur, pas la fenêtre d'une
caisse), 1 à 1 000 lignes, niveau inconnu refusé (400). Lit le
fichier courant et, s'il n'a pas assez de lignes, la copie `.1`
([journal_technique.rs](../../src-tauri/serveur/src/journal_technique.rs)
`dernieres_lignes`, `filtrer` : le niveau se lit **à sa place** après
l'horodatage — un message qui contient « [ERREUR] » n'est pas une
erreur).

La console ([console.rs](../../src-tauri/serveur/src/console.rs)) gagne
la carte **Journal**, visible une fois identifié : boutons Tout /
Erreurs / Refus / Avertissements / Caisses / Infos, « Actualiser »,
plus récentes en haut, couleur par niveau. Les lignes s'écrivent par
`textContent` : un message venu d'une caisse ne peut rien injecter
dans la page du patron.

Preuves : unitaire `le_filtre_lit_le_niveau_a_sa_place` ; route
`le_journal_technique_se_lit_depuis_la_console_avec_la_permission_de_sauvegarde`
(401, 200, filtre, 400, 403 employé) ; banc `b3-console-journal.mjs` (10).
