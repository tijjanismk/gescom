# Les étapes : où en est-on, quoi ensuite

Une page, **l'état courant**. Les chiffres sont mesurés, jamais estimés.
Le récit daté de chaque avancée vit dans [JOURNAL.md](JOURNAL.md), les
décisions dans [DECISIONS.md](DECISIONS.md), le multi-société dans
[PLAN-MULTISOCIETE.md](PLAN-MULTISOCIETE.md).

Dernière mise à jour : **16 septembre 2026** (revue du code de la séance
du 13/09 ; `deepseek-context/` replié ici et supprimé).
État : **396 tests noyau SQLite** (`cargo test -p gescom-noyau`,
mesuré le 16/09 après R1, R2 et R5) ; **414 tests workspace SQLite**
(`--workspace`, mesure du 13/09, non rejouée depuis) ;
**394 tests noyau sur PostgreSQL** (suite complète sur `gescom_test`,
0 échec le 13/09 ; `auth_base` et `entretien_base` rejoués le 16/09,
23 tests, 0 échec) ; **142 scénarios** en seize fichiers `*_base.rs` qui
tournent sur les deux moteurs (`GESCOM_PG`). Serveur : **193
commandes**, rejouées **141/141** par HTTP sur base neuve, et
`POST /entretien` vérifié par HTTP (D9). `cargo check --workspace
--all-targets` au vert le 16/09, sans avertissement. Dernier commit :
voir `git log`.

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

## Revue du 16/09/2026 — à corriger, par ordre de gravité

Relecture des lots du 13/09 contre les règles de CLAUDE.md. Le récit est
dans [JOURNAL.md](JOURNAL.md) § 16/09/2026. **R1, R2 et R5 sont
corrigés ; R3 et R4 restent ouverts.**

| # | quoi | où |
|---|---|---|
| R1 | ~~**La réimputation des règlements globaux réécrivait de l'argent hors transaction**~~ **corrigé le 16/09/2026** : `reimputer_paiements_globaux_sur_base` ouvre `base.transaction()` et passe `&mut tx` aux deux aides — le `DELETE` du règlement global et les `INSERT` qui le reposent sont désormais tout ou rien (règle 4) | `noyau/src/chantiers.rs`, `noyau/src/argent.rs` (`reallouer_globaux_sur`) |
| R2 | ~~**Sur SQLite la copie de sécurité était faite APRÈS la réimputation**~~ **corrigé le 16/09/2026** : `persistance::entretenir` se scinde en `copier_avant` et `compacter`, et l'entretien copie → réimpute → compacte, comme le `pg_dump` le fait déjà sur PostgreSQL. Scénario : la copie relue montre l'état d'avant | `noyau/src/parametres.rs`, `noyau/src/persistance/mod.rs` |
| R3 | **La lecture des images oublie le dossier sur PostgreSQL** : `lire_*_base64` sur base fait `base.sqlite().and_then(…)` → `None`, quand l'écriture et la suppression utilisent `dossier_des_images_base` | `serveur/src/socle.rs` |
| R4 | **Changer d'extension laisse l'ancienne image** (`logo.jpg` sur `logo.png`) et le repli de lecture balaie `png` d'abord : colonne vidée → ancien logo. `supprimer` sait déjà balayer les extensions | `noyau/src/images.rs` (`poser_fichier`) |
| R5 | ~~**Le JSON du journal de `--promouvoir` est construit par `format!`**~~ **corrigé le 16/09/2026** : `serde_json::json!(…).to_string()`, avec un scénario qui promeut un nom portant guillemet et barre oblique inverse | `noyau/src/auth.rs` |

Mineurs : `--promouvoir` cherche `pseudo` seul (la connexion accepte
`pseudo OR email`) et ne regarde pas `u.actif` ; la modale des
permissions garde une coche « effective » figée au chargement et
n'annule rien si une des commandes échoue en cours d'enregistrement.
Structurel : donner à `sauvegarde::dossier` un `&mut Base` plutôt qu'un
`&Arc<Serveur>` ferait refuser par le compilateur la reprise de verrou
qui a mordu le 13/09.

## Dette connue

Reprise de l'ancien `deepseek-context/RESTE.md`, vérifiée le 16/09 —
`ALERTES.md` et les fiches `modules/*.md` restent plus récentes.

- **Aucune restauration dans l'application** : le contrôle d'intégrité
  dit « restaurer la dernière sauvegarde », aucun bouton ne le fait.
- **Les routes HTTP ne sont pas testées** : les scénarios couvrent le
  noyau, pas `serveur/src/api.rs`. C'est ce qui a laissé passer
  l'interblocage du 13/09, et R3 est encore sur ce chemin. Un seul
  test de route vaudrait cher.
- **Les commandes Tauri n'ont aucun test** — à commencer par
  `creer_vente`, `valider_facture`, `regler_dette_fournisseur`.
- **Un chèque rejeté ne défait pas son mouvement de caisse** : le
  correctif propre est un mouvement INVERSE, pas une suppression ;
  reste à décider si un rejet exige une caisse ouverte (D46).
- **Bon de livraison partiel** : le suivi gère le partiel, le document
  non — la conversion copie toutes les lignes à quantité pleine.
- `lire_fournisseurs_pagines` construit son `WHERE` par `format!()`.
- `ModalImpression` fait doublon avec `ApercuPiece` ; codes-barres non
  dessinés ; pièces historiques restées en `validee`.
- **Un scénario instable** : `gestion_base::une_creance_se_regle_en_deux_fois_puis_un_reglement_s_annule`
  échoue environ une fois sur trois. Deux règlements écrits dans la même
  seconde et `lire_reglements_client_sur_base` trie `ORDER BY
  p.date_paiement DESC` **sans départage** : l'ordre des deux lignes est
  alors celui que le moteur veut. Vu le 16/09/2026, antérieur aux
  corrections R1/R2 (le chemin des créances n'a pas été touché).
- Hors code : signature de l'installeur (D5), impression papier réelle.

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
