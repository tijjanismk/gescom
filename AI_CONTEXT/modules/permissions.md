# Module : rôles et permissions

Rôle : décider qui a le droit de faire quoi, **sans recompiler**.

## Ce que c'était

Un `match` codé en dur sur le **nom** du rôle :

```rust
"patron"  => true,
"employe" => matches!(permission, "ventes:creer" | …),
"lecture" => permission.ends_with(":lire"),
_         => false,
```

La colonne `role.permissions` existait en base depuis l'origine et
n'était **jamais lue**. Conséquences : ajouter un caissier ou un
magasinier demandait de toucher au code, et donner une permission de
plus à une seule personne était impossible.

## Les trois niveaux

| | Où | Qui le change |
|---|---|---|
| Le **catalogue** | `portes::CATALOGUE`, dans le code | personne — il suit les commandes |
| Les **rôles** | `role.permissions` (JSON), `role.acces_total` | le patron |
| Le **sur-mesure** | `utilisateur_permission` | le patron, par personne |

### Le catalogue reste du code

Il liste les **23 permissions que les commandes vérifient
réellement** — relevées sur les `r.ecriture(nom, permission, …)` du
socle, pas imaginées. Une permission qui ne figure dans aucune commande
serait une case à cocher sans effet, et c'est pire que pas de case.

Tout ce qui vient de la base est filtré par lui : une faute de frappe
dans un rôle ne doit ni ouvrir une porte qui n'existe pas, ni laisser
croire qu'une porte est ouverte.

⚠️ **Les lectures ne sont pas filtrées, et c'est une décision.**
`r.lecture(…)` ne demande aucune permission : tout utilisateur connecté
peut tout lire, marges et prix d'achat compris. Les rôles suffisent —
c'est au commerçant de choisir à qui il donne un compte. Le catalogue ne
contient donc volontairement aucune permission en `:lire` : en afficher
qui ne feraient rien serait mentir.

### `acces_total` : l'exception assumée

Une liste blanche a un défaut connu : une commande ajoutée demain est
invisible tant que personne n'a coché la case. Pour `patron` et
`superadmin`, ce serait absurde.

Ces deux rôles portent `acces_total = 1` : ils rendent tout le
catalogue, quoi qu'il arrive. Tous les autres sont explicites.

### `superadmin` : le compte de secours

- Court-circuite la base : **même si la table des rôles était vide ou
  abîmée, il entre.**
- `protege = 1` : ne se modifie ni ne se supprime.
- Les retraits personnels ne s'y appliquent pas.

Sans cette garantie, une mauvaise manipulation sur les rôles rendrait
l'application définitivement inadministrable.

### Le sur-mesure, dans les deux sens

`utilisateur_permission(utilisateur_id, permission, accorde)`.

- `accorde = 1` : « ce caissier fait aussi les achats » ;
- `accorde = 0` : « ce caissier-là ne touche pas au tiroir » ;
- ligne absente : ce que le rôle en dit.

Le **retrait l'emporte** sur l'ajout : entre deux lectures d'un réglage
contradictoire, on choisit la plus fermée. Répondre à ces demandes en
fabriquant un rôle par personne rendrait la liste illisible en trois
mois.

## Les rôles livrés

| rôle | ce qu'il fait |
|---|---|
| `superadmin` | tout, protégé, compte de secours |
| `patron` | tout, modifiable |
| `employe` | conservé à l'identique de la v1 |
| `caissier` | vend, encaisse, retours, clients, pièces |
| `magasinier` | articles, transferts, dépôts, livraisons, achats |
| `comptable` | créances, chèques, fournisseurs, avoirs, TVA |

Ce sont des **points de départ**. `creer_role` en fabrique d'autres sans
toucher au code — vérifié par le test
`un_role_cree_a_la_main_fonctionne_sans_recompiler`.

## La reprise : le point qui décide de la mise à jour

Les rôles existants ont `permissions = '[]'`. Basculer sur la lecture en
base sans les remplir **retirerait tous leurs droits à tout le monde**,
le matin de la mise à jour.

La migration écrit donc exactement ce que le code accordait jusque-là :
`patron` reçoit `acces_total`, `employe` retrouve sa liste. Une seule
fois, gardée par `config_app['roles_repris']` — rejouée, elle écraserait
les réglages que le commerçant vient d'ajuster.

### Le piège que le test a attrapé

La migration s'est mise à poser des rôles. Or `seed::base_est_vide`
comptait les **rôles** : une base neuve paraissait donc déjà amorcée, le
seed ne tournait pas, et **personne ne pouvait se connecter**.

« Vide » veut dire une seule chose ici : *il n'existe aucun moyen
d'entrer*. Le compte se fait désormais sur `utilisateur_auth`.

Symétriquement, sur une base neuve la reprise tourne **avant** que
`patron` existe : c'est le seed qui doit lui donner `acces_total`, sinon
le patron d'une installation neuve n'aurait aucun droit.

## Deux commandes libérées

`changer_mot_de_passe` exigeait `utilisateurs:gerer`. Un caissier — à
qui l'amorçage impose justement de changer son mot de passe à la
première connexion — **ne pouvait pas le faire**. Il était enfermé
dehors dès le premier jour.

`registre::ecriture_libre` existe pour ces cas : une écriture que tout
utilisateur connecté peut faire. Nommée explicitement pour qu'on ne
l'utilise pas par paresse — les commandes sans permission se comptent
sur les doigts d'une main (`connexion`, `changer_mot_de_passe`).

## Règles métier

- [CONFIRMÉ] Une permission hors catalogue est refusée à **tous**, y
  compris au superadmin : elle ne correspond à aucune commande, et la
  laisser passer masquerait une faute de frappe côté code.
- [CONFIRMÉ] Le superadmin passe même sans rôle en base.
- [CONFIRMÉ] Un rôle `protege` ne se modifie ni ne se supprime.
- [CONFIRMÉ] Un rôle encore porté par quelqu'un ne se supprime pas.
- [CONFIRMÉ] Les permissions sont relues **à chaque appel** : un retrait
  prend effet tout de suite, comme la révocation d'une session.
- [CONFIRMÉ] La reprise ne retire aucun droit et ne se rejoue pas.

## Tests

[noyau/tests/permissions.rs](../../src-tauri/noyau/tests/permissions.rs)
— 16 scénarios. Un système de droits rate de deux façons opposées :
laisser passer ce qui devait être fermé, ou fermer ce qui devait passer.
La seconde est la plus probable le jour d'une mise à jour, et c'est elle
qu'on vérifie le plus.

Éprouvé aussi sur un serveur réel : un caissier est accepté sur
`creer_client_rapide`, refusé sur `creer_role` et
`enregistrer_transfert` ; après un ajout personnel de
`stock:transferer` il passe la porte ; après un retrait personnel de
`caisse:mouvementer` il est refusé sur `ouvrir_session_caisse`.

## L'écran

`Paramètres → Rôles`
([OngletRoles.tsx](../../src/components/OngletRoles.tsx)) : la liste des
rôles, ce que chacun permet, et de quoi en créer d'autres. Les
permissions s'y cochent par groupe — cliquer sur « Caisse » coche ou
décoche toute la famille.

Deux règles de présentation, qui comptent plus qu'on ne croit :

- un rôle à **accès total** montre ses cases cochées : afficher une
  liste vide laisserait croire qu'il ne peut rien ;
- le rôle **protégé** n'offre aucun bouton. Un bouton qui échoue à tous
  les coups est pire que pas de bouton.

### Le menu et les onglets suivent les permissions

`role === "patron"` décidait de tout : la barre latérale, les onglets de
Paramètres, le choix de rôle à la création d'un utilisateur. Cela
marchait avec deux rôles. Avec des rôles créés par le commerçant, un
« magasinier » tombait dans la liste de l'employé — **sans Transferts,
qui est justement son travail**.

Le serveur renvoie donc les permissions **avec l'identité**, à la
connexion, et [droits.ts](../../src/lib/droits.ts) les lit :

```ts
const navigation = NAV.filter(n => !n.droit || peut(n.droit));
```

⚠️ Ceci ne sécurise rien — c'est du confort. Cacher un bouton évite
qu'on clique dessus pour rien ; le refus qui compte est celui du noyau,
à chaque appel. Un écran périmé ne peut pas ouvrir une porte que le
serveur ferme.

Les écrans qui ne font que **lire** — Tableau de bord, Stock, Journal,
Rapports — restent visibles par tous. Le noyau ne filtre pas les
lectures : les cacher ici ne serait qu'un décor, et mieux vaut une règle
vraie qu'une règle qui fait semblant.

Mesuré sur un serveur réel : le patron voit 16 entrées de menu, une
caissière 10. Et comme on lui avait personnellement ajouté
`stock:transferer` puis retiré `caisse:mouvementer`, son menu montre
« Transferts » et pas « Caisse » — les réglages individuels remontent
jusqu'à la barre latérale.

## Ce qui reste

1. **La confidentialité des lectures n'est pas traitée**, et c'est une
   décision : les rôles suffisent, c'est au commerçant de choisir à qui
   il donne un compte. Un caissier voit donc les marges. Le jour où ce
   sera un sujet, le catalogue accueillera des permissions en `:lire` et
   `r.lecture` prendra un argument.
2. **Aucun compte `superadmin` n'est créé** par l'amorçage : le rôle
   existe, personne ne le porte. À décider — un compte de secours livré
   avec un mot de passe connu est aussi un risque.
3. **Les permissions d'une personne** se modifient par commande
   (`definir_permission_utilisateur`), mais l'écran ne l'expose pas
   encore : les ajouts et retraits individuels se font pour l'instant
   par le serveur.
