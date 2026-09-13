# Plan — ce qu'on va faire, dans l'ordre

Source : `AI_CONTEXT/ETAPES.md` « v2 — ce qui reste » (9 items), plus ce
que la séance du 13/09 a révélé. Ordre choisi : d'abord ce qui débloque
l'essai réel sur PostgreSQL (le chantier en cours), puis les items de
code, puis ce qui demande du monde ou des décisions.

| # | quoi | pourquoi / où | qui peut le faire |
|---|---|---|---|
| 1 | ~~Retirer le message périmé « aucune des 186 commandes ne répond » dans `serveur/src/main.rs`~~ **fait le 13/09/2026** | le serveur ment au démarrage ; le dernier chantier du portage est fini (187/187) | agent, seul |
| 2 | ~~Essai `pg_restore` sur une base jetable~~ **fait le 13/09/2026** : dump de `gescom_essai` → base neuve `gescom_restaure`, serveur redémarré dessus sans « BASE NEUVE », HTTP 200 | D4 l'exige : une sauvegarde n'existe que si la restauration marche | agent, seul |
| 3 | ~~Relancer le serveur PG et rejouer l'essai HTTP~~ **rejoué le 13/09/2026 : 141 clics, 141 ok** (base `gescom_essai` recréée à neuf + démo — l'ancienne gardait un schéma d'avant `types_postgres` ; le dernier rejeu inclut les pas D8). **Reste : la fenêtre Tauri elle-même** en mode caisse sur ce serveur | dernier « reste à faire » du portage : les scénarios disent ce que le SQL fait, pas ce que l'écran montre | agent + utilisateur (la fenêtre Tauri n'a encore jamais été utilisée en mode caisse) |
| 4 | ~~Créer un compte `superadmin` à l'amorçage~~ **fait le 13/09/2026** : `gescom-serveur --promouvoir IDENTIFIANT` (D6 — pas de compte livré). Logique dans `noyau/src/auth.rs`, appel dans `serveur/src/main.rs`, 3 scénarios dans `tests/auth_base.rs`, vérifié en vrai sur une base PostgreSQL jetable (promotion, idempotence, refus, journal) | ETAPES #3, D6 : le rôle existe, personne ne le porte | agent, seul |
| 5 | ~~Écran des permissions par personne~~ **fait le 13/09/2026** : `src/components/ModalPermissionsUtilisateur.tsx` + bouton « Permissions » dans Paramètres → Utilisateurs. Chemin HTTP rejoué contre le serveur PG (ajout, retrait, retour au rôle, refus), typecheck au vert. La fenêtre elle-même reste à voir (item 3) | ETAPES #2, D7 : les commandes existent, l'interface non. Front React | agent, seul |
| 6 | ~~Écriture des images depuis une caisse~~ **fait le 13/09/2026** : la caisse lit le fichier et envoie le **contenu** en base64 (`{nom, contenu}`) ; le serveur le range dans **son** dossier d'images (SQLite : parent du fichier de base ; PG : `data_dir()/ml.gescom.app`) et enregistre le chemin. Validation partagée dans `noyau/src/images.rs` (png/jpg/jpeg/webp/svg, 10 Mo, base64 standard) ; `supprimer_*` efface colonne **et** fichier ; `CORPS_MAX` 8 → 16 Mio ; 10 scénarios `images_base.rs` sur les deux moteurs ; rejeu HTTP 141/141 sur base neuve. Serveur : **193 commandes** | ETAPES #4, D8 : la commande recevait un *chemin* local, qui ne désigne rien chez le serveur — faire passer les octets par le réseau | agent, seul |
| 7 | ~~`entretenir_base` devient un travail du serveur~~ **fait le 13/09/2026** : route `POST /entretien` (même permission que la sauvegarde), verrou gardé pendant l'opération ; intégrité refusée si corrompue ; réaffectation des règlements globaux ; copie avant (`VACUUM INTO` / `pg_dump`) ; compactage (`REINDEX`+`VACUUM` / `VACUUM (ANALYZE)`) ; console : carte Administration + bouton Entretien ; la caisse ne garde que `diagnostiquer_base`. 5 scénarios `entretien_base.rs` sur les deux moteurs | ETAPES #5, D9 : c'est un travail de serveur, il reste local (la caisse ne devrait jamais y toucher) | agent, seul |
| 8 | ~~Documenter le piège tauri-build (serveur qui verrouille le sidecar) dans `AI_CONTEXT/modules/environnement-windows.md`~~ **fait le 13/09/2026** : sections 4 (sidecar verrouillé) et 5 (`cargo test` ne relie pas le binaire nu) | vécu le 13/09 : erreur `Os { code: 5, PermissionDenied }` qui ne ressemble pas à ce qu'elle est | agent, seul |
| 9 | Impression papier réelle + glisser-déposer du pied de document | ETAPES #7 : jamais vérifiés à la main — il faut une imprimante | utilisateur |
| 10 | Déclencheur de stock SQLite multi-dossier | ETAPES #9 : un mouvement de `dossier-b` crée sa ligne de stock dans `defaut` ; sans effet tant qu'une base SQLite n'a qu'un dossier | avec la v3 |
| 11 | Signature de l'installeur | ETAPES #1, D5 : décision d'achat d'un certificat de signature de code — pas du code | utilisateur |

## Règles de la maison à respecter sur chaque item

- La logique vit dans `noyau/src/<module>.rs`, jamais dans les façades
  Tauri ni dans le serveur.
- Toute commande existe en deux versions : `fn(conn: &Connection)` et
  `fn_sur_base(base: &mut Base)` — une correction se fait dans les deux.
- SQL portable, une seule fois (placeholders liés, `LOWER(x) LIKE LOWER(…)`,
  `CAST(SUM(…) AS BIGINT)`, dates par `SUBSTR(x, 1, 10)`, durées en Rust).
- Ce qui écrit plusieurs lignes tourne dans une transaction ; l'argent
  sort du tiroir → caisse ouverte exigée avant d'écrire.
- Une règle métier vit dans `coeur/` (pure, testée) et s'appelle des
  deux versions ; les décisions numérotées ne se rediscutent pas dans le
  code.
- Tout ça est détaillé dans [CLAUDE.md](../CLAUDE.md) — le relire avant
  d'écrire une ligne.

## Après chaque lot

Une ligne dans `AI_CONTEXT/ETAPES.md`, une section datée dans
`AI_CONTEXT/JOURNAL.md`, la fiche `modules/*.md` touchée si une
signature ou une règle a bougé. Commit en français, un par lot,
message = le pourquoi. **Règle de l'utilisateur en ce moment : ne pas
commiter avant un changement majeur — demander avant de commiter.**
