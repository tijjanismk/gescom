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
| le **catalogue** — 28 permissions, celles que les commandes vérifient réellement (`r.ecriture(nom, permission, …)`) | code | personne |
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
