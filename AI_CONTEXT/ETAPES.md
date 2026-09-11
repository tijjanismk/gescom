# Les étapes : ce qui est fait, ce qui reste

Ce fichier répond à une seule question : **où en est-on, et quoi
ensuite ?** Il se relit au début de chaque séance et se met à jour à la
fin. Les chiffres qu'il contient sont **mesurés**, jamais estimés — ce
projet a déjà payé une estimation à la louche (« 614 `rusqlite::`,
1556 placeholders » était faux d'un facteur deux).

Dernière mise à jour : **11 septembre 2026**.
État : **207 tests SQLite + 14 tests PostgreSQL**, tous au vert.

---

## Les trois produits

| | ce que c'est | état |
|---|---|---|
| **v1** | un poste, SQLite, pas de serveur | **livré**, ne bouge plus |
| **v2** | serveur + clients, le client ne parle **qu'**au serveur | en cours |
| **v3** | multi-société, multi-dossier, exercices | pas commencée |

**Une seule machine ne veut pas dire sans serveur.** La boutique à une
caisse installe les deux sur le même ordinateur ; la fenêtre se connecte
sur `127.0.0.1`. Ce poste est à la fois serveur et caisse.

⚠️ Ce document a d'abord décrit la v2 comme gardant un mode « monoposte »
où la fenêtre ouvrait une base SQLite locale. **C'était une lecture
fautive**, et elle a coûté deux pannes réelles. Il n'y a qu'un seul
chemin. Voir [ARCHITECTURE.md](ARCHITECTURE.md).

---

## v2 — ce qui est FAIT

### Le socle réseau
- Serveur HTTP/1.1 écrit à la main, **zéro dépendance réseau** ajoutée.
- 187 commandes servies, canal d'événements en longue attente (30 s),
  jamais d'écho au poste qui a provoqué l'événement.
- Sessions : bcrypt en base, jeton↔session en mémoire, validité relue
  **à chaque appel** pour qu'une révocation prenne effet tout de suite.
- Console du serveur, servie par le serveur lui-même dans un navigateur.
- Amorçage partagé : le serveur prépare sa base comme la fenêtre.
- → [reseau-v2.md](modules/reseau-v2.md)

### Le client ne parle qu'au serveur
- Plus de repli sur une base locale. Un repli silencieux, c'est une
  vente écrite dans une base vide pendant qu'on croit vendre.
- Écran de **branchement** avant la connexion, qui n'enregistre
  qu'**après un essai réussi**.
- Liste fermée des commandes qui restent locales : imprimer, ouvrir un
  fichier, régler le réseau.

### Les droits
- Catalogue de **23 permissions**, relevées sur les commandes réelles.
- Rôles en base, créables sans recompiler : `superadmin`, `patron`,
  `employe`, `caissier`, `magasinier`, `comptable`.
- Permissions **par personne**, en plus ou en moins de son rôle.
- Menu et onglets pilotés par les permissions, pas par le nom du rôle.
- → [permissions.md](modules/permissions.md)

### Les règles métier réparées
| quoi | pourquoi ça comptait |
|---|---|
| Stock = somme de ses mouvements | un compteur ne dit pas *pourquoi* il vaut ça |
| Numérotation par compteur transactionnel | **5 doublons sur 100** mesurés avec l'ancien calcul |
| Le stock bouge au document qui le constate | une facture ne sort plus ce qu'un bon a déjà sorti |
| Miroir fournisseur complet | le bouton « → Facture fourn. » était **mort** |
| Fond de caisse compté une fois | la clôture annonçait un manque **tous les soirs** |
| Vente à découvert recalculée en base | un écran périmé créait un découvert invisible |

### L'installeur
- `gescom-serveur.exe` **empaqueté**, raccourcis menu Démarrer et
  démarrage de session.
- Pas signé — décision assumée. → [installeur.md](modules/installeur.md)

### Les documents
- Atelier de modèles, glisser-déposer, import/export.
- Facture « classique » avec TVA détaillée ligne par ligne.
- Pied de page **dessiné**, placé au millimètre, répété sur chaque page.

---

## v2 — ce qui RESTE

### Bloquant avant de vendre
1. ~~Le pare-feu~~ — **vérifié le 11/09/2026** : un téléphone sur le
   même Wi-Fi joint le serveur. La règle posée par `parefeu.ps1` marche
   depuis un vrai second appareil, pas seulement depuis la boucle
   locale.
2. **L'installeur n'est pas signé.** Chaque installation dépend de
   l'humeur de SmartScreen. Auto-signé + racine posée à la main tient
   tant qu'on déploie soi-même.

### Fonctionnel
3. L'écran des permissions **par personne** : les commandes existent,
   l'interface non.
4. Aucun compte `superadmin` n'est créé. Le rôle existe, personne ne le
   porte. Un compte de secours à mot de passe connu est aussi un risque.
5. Écriture des images (logo, en-tête, pied) depuis une caisse : la
   commande reçoit un *chemin* local, qui ne désigne rien chez le
   serveur.
6. `entretenir_base` (REINDEX, VACUUM) reste locale alors que c'est un
   travail de serveur.

### Jamais vérifié à la main
7. Le glisser-déposer du pied de page, et une **vraie impression
   papier**.
8. La fenêtre en mode caisse, utilisée pour de bon. Tous les essais sont
   passés par le serveur en HTTP.

---

## Le portage PostgreSQL

### Fait
- **Schéma dialect-neutre** : `schema.sql` accepté par les deux moteurs.
- **Façade `Base`** : `?1` → `$1` traduit au passage, et un type `Ligne`
  commun pour que les 435 fermetures de lecture survivent sans être
  touchées. → [base.rs](../src-tauri/noyau/src/base.rs)
- **Amorçage portable** : schéma, tables v2, rôles, comptes, dépôt,
  client générique, et les données de démonstration.
- **Transactions** : `Base::transaction()` — tout ou rien, sur les deux
  moteurs. Indispensable dès qu'on touche à l'argent. La plomberie est
  partagée entre `Base` et `Transaction`, pour qu'une correction ne
  s'applique pas à une seule.
- Erreurs PostgreSQL lisibles : `db error` devenait trois mots inutiles,
  on remonte maintenant la contrainte, la colonne et la table.

### Les modules portés, dans l'ordre

| lot | modules | ce qu'il débloque |
|---|---|---|
| 1 | `auth`, `sessions`, `portes` | **entrer** dans la base |
| 2 | `catalogue` | l'écran de caisse s'affiche |
| 3 | `comptoir` | créer clients et articles — le premier lot qui **écrit** |
| 4 | fondation d'`argent` | la **numérotation**, dont tout dépend |

La règle des permissions vit dans une **fonction pure** appelée par les
deux lectures : deux copies d'un calcul de droits finissent toujours par
diverger, et personne ne s'en aperçoit avant qu'un caissier fasse ce
qu'il ne devait pas. Même principe pour le préfixe des séries.

Vérifié sur une base réelle : connexion du patron avec ses 23
permissions, catalogue de 8 articles avec leurs unités regroupées, prix
d'achat masqué pour l'employé, dépôt inconnu qui retombe sur le défaut,
client créé puis modifié (accent conservé, champ vidé redevenu NULL),
article refusé en doublon sans rien écrire.

Et le test qui compte le plus : **quatre connexions PostgreSQL
simultanées réservant 100 numéros — aucun doublon, suite continue.**
C'est le scénario qui sortait 5 doublons sur 100 avec l'ancien calcul.

### Reste — le vrai volume

| | mesuré |
|---|---|
| points d'appel (`execute`, `query_row`, `query_map`, `prepare`) | **739** |
| `params!` à convertir | 480 |
| fermetures de lecture | 435 |
| paramètres `conn:` dans les signatures | 242 |
| constructions SQL non portables (hors placeholders) | ~60 |

**Le serveur ne sait pas encore faire tourner une boutique sur
PostgreSQL.** Vendre, facturer, encaisser passent par SQLite.

**Le prochain morceau est identifié et volontairement laissé seul :**
`creer_vente_sur` (280 lignes) et `valider_facture_sur` (290 lignes).
Ce sont les deux fonctions où une erreur ne se corrige pas par un clic —
elles écrivent la vente, ses lignes, les mouvements de stock, le
paiement et le mouvement de caisse. Elles méritent leur propre séance et
leurs propres scénarios, pas d'être expédiées à la fin d'une autre.

Ensuite : `pieces`, `achats`, `retours`, puis le reste. La façade permet
de porter module par module sans rien casser — ce qui n'est pas porté
continue de tourner sur SQLite.

Deux chantiers à part :
- la **sauvegarde** — `VACUUM INTO` n'existe pas côté PostgreSQL ;
- les **~60 constructions** — `julianday`, `strftime`, `INSERT OR
  IGNORE`, `substr(x, -5)`. À réécrire en SQL que les deux moteurs
  acceptent, pas en deux variantes.

---

## v3 — décidé, pas commencé

### Multi-société et multi-dossier
Modèle Ciel : un dossier = **une société × un exercice**, avec une date
de début, une date de fin, et une prolongation possible.

⚠️ **À trancher AVANT de porter les 739 appels.** Le découpage en
dossiers décide de la forme de la base : l'ajouter après obligerait à
reprendre chaque requête une seconde fois.

Deux formes possibles, non tranchées :

| | avantage | prix |
|---|---|---|
| une base par dossier | étanche, archivage naturel, **aucune requête à changer** | pas de vue consolidée |
| une colonne `dossier_id` | vue consolidée, changement instantané | une requête qui oublie le filtre **mélange deux sociétés** |

### Dépôt → magasin
`depot` et `magasin` désignent la même chose. Renommer à l'écran coûte
une heure ; renommer jusque dans la base touche ~200 requêtes et demande
une migration.

**À faire en même temps que le multi-dossier**, qui rouvre de toute
façon la forme de la base. Séparément, ce serait reprendre les mêmes
fichiers deux fois.

---

## Comment travailler ici

```bash
# préparer une base, avant même de lancer le serveur
cargo run -p gescom-noyau --example amorcer -- <cible> [--demo]

# le serveur
./src-tauri/target/debug/gescom-serveur.exe --base <cible>

# les tests
.\outils\cargo-tenace.ps1 test --workspace          # SQLite
GESCOM_PG="postgresql://..." cargo test -p gescom-noyau \
    --test postgres_amorcage -- --test-threads=1     # PostgreSQL
```

`cargo-tenace` et non `cargo` : Smart App Control bloque les binaires
fraîchement liés, et l'erreur ne ressemble pas à ce qu'elle est. →
[environnement-windows.md](modules/environnement-windows.md)

Les tests PostgreSQL ne tournent que si `GESCOM_PG` est défini. Un test
qui exige un service tiers ne doit pas faire échouer la suite de
quelqu'un qui ne l'a pas installé.
