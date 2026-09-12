# Les étapes : où en est-on, quoi ensuite

Une page, **l'état courant**. Les chiffres sont mesurés, jamais estimés.
Le récit daté de chaque avancée vit dans [JOURNAL.md](JOURNAL.md), les
décisions dans [DECISIONS.md](DECISIONS.md), le multi-société dans
[PLAN-MULTISOCIETE.md](PLAN-MULTISOCIETE.md).

Dernière mise à jour : **12 septembre 2026**.
État : **392 tests SQLite + 21 tests PostgreSQL**, tous au vert, plus
**84 scénarios** en dix fichiers `*_base.rs` qui tournent sur les deux
moteurs (`GESCOM_PG`). Dernier commit : voir `git log`.

---

## Les trois produits

| | ce que c'est | état |
|---|---|---|
| **v1** | un poste, SQLite, pas de serveur | **livré**, ne bouge plus |
| **v2** | serveur + clients, le client ne parle **qu'**au serveur | **fonctionnelle**, sur SQLite et PostgreSQL |
| **v3** | multi-société, multi-dossier, exercices | fondation posée, écrans pas commencés |

**Une seule machine ne veut pas dire sans serveur.** La boutique à une
caisse installe les deux sur le même ordinateur. Il n'y a qu'un seul
chemin : le client parle au serveur, jamais à une base locale — deux
pannes réelles ont appris que le repli silencieux est pire que l'arrêt.
→ [ARCHITECTURE.md](ARCHITECTURE.md)

---

## v2 — fait

| chantier | fiche |
|---|---|
| Serveur HTTP écrit à la main, 187 commandes, canal d'événements, sessions révocables à chaque appel, console | [reseau-v2](modules/reseau-v2.md) |
| Droits : 23 permissions, rôles en base, permissions par personne (commandes) | [permissions](modules/permissions.md) |
| Stock = somme des mouvements ; numérotation transactionnelle (0 doublon sur 100 × 4 connexions) ; le stock bouge au document qui le constate | [livraison-stock](modules/livraison-stock.md), [numerotation](modules/numerotation.md) |
| Installeur empaqueté, non signé (D5) | [installeur](modules/installeur.md) |
| Modèles de documents, atelier, import/export | [modeles-documents](modules/modeles-documents.md) |
| **PostgreSQL : 187/187 commandes servies sur `Base`**, sauvegarde `pg_dump`, filet `schema_commun` | [postgresql](modules/postgresql.md) |

## v2 — ce qui reste

| # | quoi | pourquoi ça compte |
|---|---|---|
| 1 | **L'installeur n'est pas signé** | chaque installation dépend de SmartScreen ; tenable tant qu'on déploie soi-même (D5) |
| 2 | L'écran des permissions **par personne** | les commandes existent, l'interface non (D7) |
| 3 | Aucun compte `superadmin` n'est créé | le rôle existe, personne ne le porte (D6) |
| 4 | Écriture des images depuis une caisse | la commande reçoit un *chemin* local, qui ne désigne rien chez le serveur (D8) |
| 5 | `entretenir_base` reste locale | c'est un travail de serveur (D9) |
| 6 | **La restauration `pg_restore` jamais jouée** | D4 le demande ; la commande est dans [postgresql.md](modules/postgresql.md) — à essayer sur une base jetable |
| 7 | Une **vraie impression papier**, le glisser-déposer du pied | jamais vérifiés à la main |
| 8 | La fenêtre en **mode caisse**, pour de bon | tous les essais passent par HTTP — dont `outils/caisse_pg.py`, 129 clics ok sur PostgreSQL le 12/09 ; la fenêtre Tauri elle-même n'a pas été utilisée |
| 9 | Le déclencheur de stock sur SQLite multi-dossier | un mouvement de `dossier-b` crée sa ligne de stock dans `defaut` ; sans effet tant qu'une base SQLite n'a qu'un dossier — à régler avec la v3 |

## v3 — fondation posée, pas commencée

Un dossier = **une société × un exercice**. Forme retenue : une colonne
`dossier_id` sur les 23 tables cloisonnées, `Base` porte son dossier, un
détecteur refuse **avant d'exécuter** toute requête qui oublie le
filtre. Tables `dossier` et `exercice`, règle des dates pure et testée.
Pas cloisonnés, et c'est voulu : `utilisateur`, `role`, `poste`,
`session_reseau`, `modele_document`, `article`, `unite_vente`,
`config_app`, `parametres_societe`.

Reste : les écrans (choisir un dossier, ouvrir un exercice, clôturer),
et la migration d'une base existante vers plusieurs dossiers.
→ [PLAN-MULTISOCIETE.md](PLAN-MULTISOCIETE.md)

---

## Comment travailler ici

```bash
# préparer une base, avant même de lancer le serveur
cargo run -p gescom-noyau --example amorcer -- <cible> [--demo]

# le serveur
./src-tauri/target/debug/gescom-serveur.exe --base <cible>

# les tests SQLite (cargo-tenace : Smart App Control bloque les binaires frais)
.\outils\cargo-tenace.ps1 test --workspace

# les tests PostgreSQL — depuis bash, sur la base JETABLE, jamais celle du serveur
GESCOM_PG="postgresql://postgres:…@127.0.0.1:5432/gescom_test" \
  cargo test -p gescom-noyau --test pieces_base -- --test-threads=1
```

- Les scénarios PostgreSQL font `DROP SCHEMA` : **jamais sur `gescom`**.
- `cargo-tenace.ps1` avale `-p` et `--` : passer `--package`, et lancer
  les scénarios PostgreSQL avec `cargo` directement, depuis bash.
- Les tests PostgreSQL ne tournent que si `GESCOM_PG` est défini : un
  test qui exige un service tiers ne doit pas faire échouer la suite de
  quelqu'un qui ne l'a pas.
- Les conventions de code (SQL portable, `_sur_base`, `Acces`) sont dans
  [CLAUDE.md](../CLAUDE.md) à la racine.
