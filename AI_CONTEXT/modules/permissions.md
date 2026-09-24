# Module : permissions

Rôle : qui a le droit de faire quoi. Un catalogue dans le code, des
rôles en base, du sur-mesure par personne. Le refus qui compte est celui
du noyau, **à chaque appel** ; l'écran ne fait que cacher des boutons.

## Fichiers

| | rôle |
|---|---|
| [noyau/src/portes.rs](../../src-tauri/noyau/src/portes.rs) | `CATALOGUE` ([l.64](../../src-tauri/noyau/src/portes.rs#L64)), `existe`, `permissions_de` / `permissions_de_sur` (les deux moteurs), `verifier_permission` / `_sur` |
| [noyau/src/roles.rs](../../src-tauri/noyau/src/roles.rs) | créer, modifier, supprimer un rôle ; permissions par personne — en deux versions (`_sur_base`) |
| [src/lib/droits.ts](../../src/lib/droits.ts) | `peut(droit)` côté écran : menu, onglets, choix de rôle |
| [src/components/OngletRoles.tsx](../../src/components/OngletRoles.tsx) | Paramètres → Rôles : les rôles, leurs cases par groupe, création |
| [src/components/ModalPermissionsUtilisateur.tsx](../../src/components/ModalPermissionsUtilisateur.tsx) | Paramètres → Utilisateurs → Permissions : le sur-mesure d'une personne, à trois états par permission |

## Les trois niveaux

| | où | qui le change |
|---|---|---|
| le **catalogue** — 33 permissions (`modeles:gerer` retirée avec l'atelier, v3 A-3 ; cinq de lecture ajoutées, v3 B-1 et C-1), celles que les commandes vérifient réellement (`r.ecriture(nom, permission, …)`, `r.sur_base(nom, Some(permission), …)`) | code | personne |
| les **rôles** — `role.permissions` (JSON), `role.acces_total` | base | le patron |
| le **sur-mesure** — `utilisateur_permission(utilisateur, permission, accorde)` | base | le patron, par personne |

Rôles livrés : `superadmin` (tout, protégé, compte de secours),
`patron` (tout), `employe` (l'ancien v1), `caissier`, `magasinier`,
`comptable`. Points de départ : `creer_role` en fabrique d'autres sans
recompiler.

## Une permission que seul le patron porte : `avoirs:accorder` (18/09/2026)

Un avoir **sans marchandise en face** (`accorder_avoir_client`) est un
crédit qui sort de nulle part — le geste le plus facile à détourner.
La permission est au catalogue mais **dans aucun rôle livré** : elle
vient avec `acces_total` (patron, superadmin), ou se donne à la main
par le sur-mesure. `avoirs:gerer` (le comptable) applique et rembourse
les avoirs existants ; il n'en crée pas.

## `dossiers:gerer` (19/09/2026, v3)

Créer un dossier, ouvrir, prolonger ou clore un exercice. Comme
`avoirs:accorder` : aucun rôle livré ne la porte, elle vient avec
`acces_total` ou se donne à la main.

## `journal:lire` — la première permission de LECTURE (23/09/2026, v3 B-1)

Jusqu'ici les lectures n'étaient pas filtrées (tout connecté lit tout).
L'Historique est la première lecture que le **serveur refuse** sans
permission : `lire_historique` et `lire_filtres_historique`. Rôles
livrés : `patron` (accès total) et `comptable` sur une base **neuve** ;
caissier, magasinier, employé ne l'ont pas. Une base déjà installée
reçoit `journal:lire` pour son comptable avec la migration des rôles
de C-1 (les quatre autres permissions de lecture) ; d'ici là, le
patron la donne par le sur-mesure. L'entrée de menu et les boutons
« Historique » des fiches suivent `peut("journal:lire")`.

## Les cinq permissions de LECTURE (23/09/2026, v3 C-1)

Les lectures n'étaient pas filtrées. Il y en a maintenant cinq, pas
une par commande (D19) : `achats:lire_prix`, `rapports:lire`,
`tiers:lire_solde`, `journal:lire`, `caisse:lire_autres`.

**Une table, un endroit** : [coeur/lecture.rs](../../src-tauri/noyau/src/coeur/lecture.rs)
dit, commande par commande, ce qui se **refuse** (`Refus`), ce qui se
**masque** (`Masque` : ces clés à `null`, à toute profondeur — jamais
un zéro), les **paramètres neutralisés** (`Neutre` : le filtre « avec
dette seulement » et le tri par dette de la liste des clients), ce qui
se **réduit à soi** (`AMoi` : les sessions de caisse) et ce qui se
**vérifie en base** (`SessionCaisseAMoi` : les mouvements d'une
session). `api::rpc` l'applique à chaque appel : refus avant, filtre
après. Aucune fonction du noyau ne teste un nom de rôle pour ça — la
seule qui le faisait (`lire_articles_avec_unites`, `role == "patron"`)
prend désormais `voir_prix_achat: bool` : la fenêtre v1 le donne au
patron, le serveur toujours, puis masque selon la permission.

Deux lignes de partage :
- `tiers:lire_solde` cache ce que **doit un tiers** (état, relevé,
  encours, listes de dettes). Le reste d'**une** pièce ou d'**un** reçu
  reste lisible : c'est le document en main.
- `achats:lire_prix` cache le coût là où il est incident (catalogue,
  stock, magasins, rapports) ; sur les documents d'achat (fiche
  fournisseur, factures à retourner, pièces fournisseur),
  `achats:creer` suffit.

Rôles livrés : patron tout (`acces_total`) ; comptable tout sauf
`caisse:lire_autres` ; caissier, magasinier, employé aucune. Une base
**installée** : `amorcage::lectures_du_comptable` ajoute au comptable
les quatre qui lui reviennent, **une fois** (marque
`migration_v3_lectures`) — appelée par `amorcer` et par le serveur sur
une base fichier.

Écrans : menu (Journal, Rapports : `rapports:lire` ; Historique :
`journal:lire`), accueil sans chiffres, onglets Créances et états de
dette, onglet Pièces fournisseur, onglet Retour fournisseur, valeur du
stock et des magasins, export CSV du catalogue, écarts de caisse — tous
suivent `peut(...)` et ne demandent pas ce qui serait refusé.

Reste ouvert, écrit : `lire_lignes_piece` d'une pièce fournisseur rend
ses prix sans condition (la réponse ne dit pas le type de la pièce).
Avec une seule caisse partagée par dossier, la session ouverte et les
mouvements du jour restent lisibles par tous les caissiers :
`caisse:lire_autres` porte sur l'historique des sessions.

## Des plafonds, pas seulement des portes (23/09/2026, v3 C-3)

Trois plafonds par **rôle** (`role.remise_max_pct`, `remboursement_max`,
`credit_max` ; vide = aucun), ajustables par **personne**
(`utilisateur_plafond`, chaque valeur posée l'emporte). Un rôle à accès
total n'en a jamais (on refuse même d'en poser).

- La règle : [coeur/plafonds.rs](../../src-tauri/noyau/src/coeur/plafonds.rs)
  — `fusionner`, `valider`, `remise_pct`, `verifier_vente`,
  `verifier_piece`, `verifier_remboursement`, le message
  « Remise de 40 % — votre plafond est 15 %. Demander au patron. »
- La base : [plafonds.rs](../../src-tauri/noyau/src/plafonds.rs) —
  `de` / `de_sur` (les plafonds d'une personne), `exiger_remboursement(_sur)`
  (lit `auteur::courant()` ; hors serveur, pas de plafond),
  `lire_sur`, `definir_role_sur`, `definir_utilisateur_sur` (journal
  `plafonds_modifies`).
- **Où se juge quoi** : l'argument suffit → **dans la poignée** du
  serveur, comme `pieces:antidater` : `creer_vente` (remise de chaque
  ligne, crédit laissé si `credit`), `creer_piece` / `modifier_piece`
  (remises de ligne et globale), `rembourser_avoir` (montant). Le
  montant n'est connu qu'au fond → **au point où l'argent sort**, dans
  le noyau : `retours::enregistrer_sortie_caisse(_sur)` (retour,
  reliquat) et `annuler_reglement(_sur)` quand l'argent est rendu
  (`remboursement = true`) — une contre-passation d'erreur de saisie ne
  se plafonne pas.
- Commandes : `lire_plafonds`, `definir_plafonds_role`,
  `definir_plafonds_utilisateur` (`utilisateurs:gerer`, nées sur `Base`).
- Écran : `components/EditeurPlafonds.tsx`, dans Paramètres → Rôles
  (chaque rôle sans accès total) et dans la fenêtre Permissions d'une
  personne.
- Limite écrite : un retour qui rend en deux sorties (part + reliquat)
  juge chaque sortie, pas leur somme.

## Une permission qui dépend des ARGUMENTS : `pieces:antidater`

Le registre vérifie la permission de base d'une commande avant de
l'appeler. `pieces:antidater` (17/09/2026) est différente : elle ne
porte pas sur la commande mais sur **un argument** — une date de vente,
de pièce ou de règlement antérieure à aujourd'hui. Elle se vérifie donc
**dans la poignée**, par `exiger_antidatage` / `exiger_antidatage_base`
([socle.rs](../../src-tauri/serveur/src/socle.rs)), après la permission
de base. Saisir la date du jour n'antidate pas. La raison n'est pas
comptable : antidater une vente en espèces masque un trou dans le
tiroir. Le rôle `patron` (accès total) l'a d'office ; un caissier ne
l'hérite pas.

## Règles

- [CONFIRMÉ] **Liste blanche** : une permission hors catalogue est refusée à tous, superadmin compris — une faute de frappe côté base ne doit ni ouvrir ni faire croire ([portes.rs:101](../../src-tauri/noyau/src/portes.rs#L101)).
- [CONFIRMÉ] `acces_total = 1` (`patron`, `superadmin`) rend **tout le catalogue**, y compris une commande ajoutée demain ; réaffirmé à chaque démarrage par `amorcage::acces_total_toujours_reaffirme`.
- [CONFIRMÉ] `superadmin` entre **même si la table des rôles est vide ou abîmée** ; `protege = 1` : ne se modifie ni ne se supprime ; les retraits personnels ne s'y appliquent pas.
- [CONFIRMÉ] Sur-mesure : `accorde = 1` ajoute, `accorde = 0` retire, ligne absente = le rôle. **Le retrait l'emporte.**
- [CONFIRMÉ] Un rôle encore porté ne se supprime pas ([roles.rs](../../src-tauri/noyau/src/roles.rs)).
- [CONFIRMÉ] Les permissions sont relues **à chaque appel** : un retrait agit tout de suite.
- [CONFIRMÉ] **Les lectures ne sont pas filtrées, et c'est une décision** : tout utilisateur connecté lit tout, marges comprises. Le catalogue n'a donc aucune permission en `:lire` — en afficher qui ne feraient rien serait mentir.
- [CONFIRMÉ] `registre::ecriture_libre` : une écriture sans permission, nommée pour qu'on ne l'utilise pas par paresse. Deux seulement : `connexion`, `changer_mot_de_passe` (sinon un caissier à qui l'amorçage impose de changer son mot de passe était enfermé dehors).
- [CONFIRMÉ] La **reprise** (migration) écrit ce que le code accordait avant — `patron` : `acces_total`, `employe` : sa liste — une seule fois, gardée par `config_app['roles_repris']`. Rejouée, elle écraserait les réglages du commerçant. « Base vide » se compte sur `utilisateur_auth`, pas sur les rôles : la reprise pose des rôles avant le seed.
- [CONFIRMÉ] Le serveur renvoie les permissions **avec l'identité** à la connexion ; `droits.ts` filtre le menu. Confort, pas sécurité.
- [CONFIRMÉ] `promouvoir_superadmin_sur` ([auth.rs](../../src-tauri/noyau/src/auth.rs)) : `gescom-serveur --promouvoir IDENTIFIANT` redonne `superadmin` à un compte existant — une commande du **serveur**, jamais servie par HTTP, qui exige d'être devant la machine. Le geste s'écrit au journal (`role_change`, auteur `serveur`). Le compte de secours **livré** reste refusé (D6).

## Par dossier (v3, C-2 — décision C2)

[acces_dossiers.rs](../../src-tauri/noyau/src/acces_dossiers.rs), table
`utilisateur_dossier (utilisateur_id, dossier_id, role_id)`. La règle,
pure (`role_dans`) :

- `superadmin` : partout ;
- **des lignes : exactement ces dossiers, avec ces rôles** — même si le
  rôle global est `patron` (le frère) ;
- **aucune ligne : comme avant** — partout avec l'accès total, sinon le
  dossier d'origine seulement.

Le rôle vient du dossier de la session, relu à chaque requête
(`sessions::etat_sur`) : retirer un dossier fait tomber la session qui y
travaille (401). La connexion ne propose que les dossiers ouverts à la
personne ; `choisir_dossier` rend le rôle et les permissions du dossier.
**Qui a des lignes perd `PERMISSIONS_DE_TOUTE_LA_BASE`**
(`utilisateurs:gerer`, `postes:gerer`, `sauvegarde:lancer`,
`parametres:modifier`) : comptes, postes, base et société sont communs à
tous les dossiers — sinon le frère se créerait un compte patron sans
ligne, qui voit tout. `definir_sur` refuse : ses propres dossiers, un
compte protégé, une liste vide (c'est une désactivation), le dernier
compte qui voit tous les dossiers. Un dossier créé par quelqu'un qui a
des lignes lui est donné avec son rôle d'ici. Écran : Paramètres →
Utilisateurs → **Dossiers** (avec plusieurs dossiers, aussi pour un
patron). La fenêtre monoposte ne connaît pas ces lignes (un dossier).

## Tests

[tests/permissions.rs](../../src-tauri/noyau/tests/permissions.rs) — 16
scénarios, dont « fermer ce qui devait passer », la panne la plus
probable un matin de mise à jour ;
`un_role_cree_a_la_main_fonctionne_sans_recompiler`. Éprouvé sur un
serveur réel : un caissier passe `creer_client_rapide`, est refusé sur
`creer_role`, passe `enregistrer_transfert` après un ajout personnel,
est refusé sur `ouvrir_session_caisse` après un retrait.

## Ce qui reste

1. ~~L'écran des **permissions par personne** — la commande
   `definir_permission_utilisateur` existe, pas l'interface (D7)~~
   **fait le 13/09/2026** :
   [ModalPermissionsUtilisateur.tsx](../../src/components/ModalPermissionsUtilisateur.tsx).
2. ~~Aucun compte `superadmin` n'est créé par l'amorçage (D6)~~
   **réglé le 13/09/2026** : `gescom-serveur --promouvoir IDENTIFIANT`.
3. La confidentialité des lectures, le jour où ce sera un sujet :
   des permissions `:lire` et un argument à `r.lecture`.
