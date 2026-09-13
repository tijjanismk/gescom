# Les étapes : où en est-on, quoi ensuite

Une page, **l'état courant**. Les chiffres sont mesurés, jamais estimés.
Le récit daté de chaque avancée vit dans [JOURNAL.md](JOURNAL.md), les
décisions dans [DECISIONS.md](DECISIONS.md), le multi-société dans
[PLAN-MULTISOCIETE.md](PLAN-MULTISOCIETE.md).

Dernière mise à jour : **13 septembre 2026**.
État : **394 tests noyau SQLite** (`cargo test -p gescom-noyau`) et
**414 tests workspace SQLite** (`--workspace`), tous au vert ;
**394 tests noyau sur PostgreSQL** (suite complète sur `gescom_test`,
0 échec) ; **140 scénarios** en seize fichiers `*_base.rs` qui
tournent sur les deux moteurs (`GESCOM_PG`). Serveur : **193
commandes**, rejouées **141/141** par HTTP sur base neuve, et
`POST /entretien` vérifié par HTTP (D9). Dernier commit : voir
`git log`.

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
| Serveur HTTP écrit à la main, 193 commandes, canal d'événements, sessions révocables à chaque appel, console | [reseau-v2](modules/reseau-v2.md) |
| Droits : 23 permissions, rôles en base, permissions par personne (commandes) | [permissions](modules/permissions.md) |
| Stock = somme des mouvements ; numérotation transactionnelle (0 doublon sur 100 × 4 connexions) ; le stock bouge au document qui le constate | [livraison-stock](modules/livraison-stock.md), [numerotation](modules/numerotation.md) |
| Installeur empaqueté, non signé (D5) | [installeur](modules/installeur.md) |
| Modèles de documents, atelier, import/export | [modeles-documents](modules/modeles-documents.md) |
| **PostgreSQL : 193/193 commandes servies sur `Base`**, sauvegarde `pg_dump`, filet `schema_commun` | [postgresql](modules/postgresql.md) |

## v2 — ce qui reste

| # | quoi | pourquoi ça compte |
|---|---|---|
| 1 | **L'installeur n'est pas signé** | chaque installation dépend de SmartScreen ; tenable tant qu'on déploie soi-même (D5) |
| 2 | ~~L'écran des permissions **par personne**~~ **fait le 13/09/2026** : Paramètres → Utilisateurs → « Permissions », à trois états par permission (rôle / autorisée / refusée) | les commandes existent, l'interface non (D7) |
| 3 | ~~Aucun compte `superadmin` n'est créé~~ **réglé le 13/09/2026** : `gescom-serveur --promouvoir IDENTIFIANT` redonne le rôle `superadmin` à un compte existant, depuis la machine du serveur — pas de compte de secours livré (D6) | le rôle existe, personne ne le porte (D6) |
| 4 | ~~Écriture des images depuis une caisse~~ **fait le 13/09/2026** : la caisse lit le fichier et envoie le **contenu** en base64 ; le serveur le range dans son dossier d'images et enregistre le chemin. Refus net : format inconnu, base64 illisible, plus de 10 Mo. La suppression efface aussi le fichier (D8) | la commande recevait un *chemin* local, qui ne désigne rien chez le serveur (D8) |
| 5 | ~~`entretenir_base` reste locale~~ **fait le 13/09/2026** : la route `POST /entretien` du serveur (permission `sauvegarde:lancer`) vérifie l'intégrité, réaffecte les règlements fournisseur globaux, copie avant (VACUUM INTO / pg_dump), compacte (REINDEX+VACUUM / VACUUM ANALYZE) — bouton « Entretien » dans la console ; la caisse garde le diagnostic, perd le bouton | c'est un travail de serveur (D9) |
| 6 | ~~La restauration `pg_restore` jamais jouée~~ **jouée le 13/09/2026** : dump de `gescom_essai` → `gescom_restaure`, serveur redémarré dessus | D4 le demande ; la commande est dans [postgresql.md](modules/postgresql.md) |
| 7 | Une **vraie impression papier**, le glisser-déposer du pied | jamais vérifiés à la main |
| 8 | La fenêtre en **mode caisse**, pour de bon | tous les essais passent par HTTP — `outils/caisse_pg.py` rejoué le 13/09 : 141/141 ok sur PostgreSQL, base neuve ; la fenêtre Tauri elle-même n'a pas été utilisée |
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

# redonner le role superadmin a un compte, devant la machine (D6)
./src-tauri/target/debug/gescom-serveur.exe --base <cible> --promouvoir admin

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
