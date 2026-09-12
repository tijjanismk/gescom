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

## Les trois niveaux

| | où | qui le change |
|---|---|---|
| le **catalogue** — 23 permissions, celles que les commandes vérifient réellement (`r.ecriture(nom, permission, …)`) | code | personne |
| les **rôles** — `role.permissions` (JSON), `role.acces_total` | base | le patron |
| le **sur-mesure** — `utilisateur_permission(utilisateur, permission, accorde)` | base | le patron, par personne |

Rôles livrés : `superadmin` (tout, protégé, compte de secours),
`patron` (tout), `employe` (l'ancien v1), `caissier`, `magasinier`,
`comptable`. Points de départ : `creer_role` en fabrique d'autres sans
recompiler.

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

## Tests

[tests/permissions.rs](../../src-tauri/noyau/tests/permissions.rs) — 16
scénarios, dont « fermer ce qui devait passer », la panne la plus
probable un matin de mise à jour ;
`un_role_cree_a_la_main_fonctionne_sans_recompiler`. Éprouvé sur un
serveur réel : un caissier passe `creer_client_rapide`, est refusé sur
`creer_role`, passe `enregistrer_transfert` après un ajout personnel,
est refusé sur `ouvrir_session_caisse` après un retrait.

## Ce qui reste

1. L'écran des **permissions par personne** — la commande
   `definir_permission_utilisateur` existe, pas l'interface (D7).
2. Aucun compte `superadmin` n'est créé par l'amorçage (D6).
3. La confidentialité des lectures, le jour où ce sera un sujet :
   des permissions `:lire` et un argument à `r.lecture`.
