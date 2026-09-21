# Plan v3 : plus simple à imprimer, plus sûr à faire tourner

Ce document **pose les bases avant qu'on code**. Comme
[PLAN-MULTISOCIETE.md](PLAN-MULTISOCIETE.md), il ne liste pas des
tâches : il tranche, avec l'exemple, le coût et une recommandation
à chaque fois. Ce qui n'est pas tranché est en fin de document.

Écrit le 21 septembre 2026, la v2 close. Rien n'est en production.

---

## 1. Ce que la v3 contient — en une page

| # | chantier | ce que ça change pour la boutique |
|---|---|---|
| A | **Pièces commerciales simplifiées** | un en-tête et un pied de page qu'on **téléverse** (une image), les **signatures** qu'on règle, et c'est tout. L'atelier de blocs glisser-déposer disparaît. |
| B | **Un vrai système de journal** | ce que le logiciel a fait se **lit** : qui a fait quoi (déjà écrit, jamais montré), les erreurs des caisses remontées au serveur, les anomalies sur le tableau de bord. |
| C | **Des droits plus complets** | ce qu'on **voit** se règle (prix d'achat, marges, rapports), les droits valent **par dossier**, des **plafonds** (remise, remboursement) et le patron **voit qui est connecté**. |
| D | **Plusieurs dossiers** (fondation posée le 19/09, dormante) | l'écran des dossiers et des exercices, le garde-fou des dates, la migration d'une base existante. → [PLAN-MULTISOCIETE.md](PLAN-MULTISOCIETE.md), déjà tranché — avec trois précisions du 21/09 (§ 5) : les **dates de travail** se donnent à la création et se **prolongent** ; **SQLite reste** un moteur de plein droit ; plusieurs dossiers demandent **le serveur**, pas PostgreSQL. |
| E | **Le plan comptable** (fondation) | le plan SYSCOHADA en base, chaque opération de Gescom **affectée** à ses comptes, et les journaux (ventes, achats, caisse) **lus** depuis ce qui existe et exportés pour le comptable. La comptabilité complète (balance, bilan, OD) viendra après — la v3 pose le sol. |

L'ordre est **A, B, C, D, E** (D20, E ajouté le 21/09) : A parce que
c'est ce qui se voit tous les jours et que l'atelier actuel coûte à
chaque retour du terrain ; B parce que sans lui on corrige C, D et E à
l'aveugle ; D après C parce que les droits par dossier en dépendent ;
E en dernier parce qu'il lit tout ce que les autres écrivent.

---

## 2. Chantier A — les pièces commerciales, simplifiées

### Ce qu'on a, honnêtement

L'atelier de modèles (septembre 2026) : 6 modèles d'usine, des blocs
(en-tête, titre, champs, tableau, totaux, texte, signatures, image,
pied de page), glisser-déposer, blocs flottants au millimètre, export
et import, images posées. **4 700 lignes** de TypeScript
(`pages/Modeles.tsx` 2 035, `lib/modeles/*` 2 330, `EditeurPiedPage`
409). Chaque retour du terrain depuis le 16/09 en a touché un morceau
(palette, redimension, champ « Client » sur une pièce fournisseur,
coordonnées de l'en-tête au champ hier).

À côté, le **générateur historique** (`genererPDF.ts`, 964 lignes)
imprime la même facture depuis des mois, en A4, A5 et ticket 80 mm,
avec le logo, l'image d'en-tête et l'image de pied que Paramètres →
Société sait déjà téléverser (`logo_chemin`, `entete_chemin`,
`pied_chemin`, D8 : par le contenu, pas le chemin).

### Décision A1 — un seul générateur, une mise en page fixe par genre

> Le commerçant ne dessine pas sa facture. Il pose son en-tête, son
> pied, ses signatures, et la facture sort **toujours pareille**.

**Ce qu'on garde** : le générateur historique, remis au propre, un par
genre (facture, devis, bon de livraison, bon de commande, reçu, relevé,
ticket). **Ce qu'on retire** : l'atelier de blocs, la table
`modele_document`, l'export/import de modèles, les blocs flottants,
les images posées, `lib/modeles/*`. Le rendu reste **côté écran** (une
seule génération pour l'aperçu et l'impression — la règle du 17/09
tient toujours).

**Ce que ça coûte** : on jette 4 700 lignes écrites ce mois-ci. Il
faut le dire tel quel. Ce qu'on gagne : plus rien à expliquer, plus de
« le bloc s'est détaché », une facture identique sur toutes les
caisses, et un seul endroit à corriger quand la TVA change.

**Le risque nommé** : quelqu'un aura un jour besoin d'une colonne de
plus (le poids, la référence fabricant). La réponse n'est pas un
atelier, c'est **une case à cocher** par colonne optionnelle dans les
réglages du genre (§ A4). Si dix cases ne suffisent plus, on aura
appris quelque chose et on rediscutera — pas avant.

**Rien à migrer** : aucune boutique n'a de modèle personnalisé en
production ; ceux de `gescom_essai` sont des essais. La table part
avec la v3 (une migration qui la supprime, idempotente).

### Décision A2 — l'en-tête et le pied sont des images

> Tu fais faire ton en-tête chez l'imprimeur, en PNG, avec ton logo,
> ton adresse, ton NIF, comme sur ton papier à lettres. Tu le
> téléverses une fois. Il s'imprime en haut de chaque pièce.

- **En-tête** : une image, largeur de la page, hauteur libre (30 mm
  conseillés). Sans image : le nom de la société en gros et ses
  coordonnées, comme aujourd'hui — et **c'est là** que les cases
  « quelles coordonnées » d'hier trouvent leur place définitive.
- **Pied de page** : une image, bas de chaque page, hauteur libre.
  Sans image : le texte `pied_facture` (mentions, RIB, « merci »).
- Les deux existent déjà en base et dans Paramètres → Société. On ne
  crée rien ; on **retire** ce qui les doublait dans l'atelier.
- Le ticket 80 mm ignore les images (largeur) et garde le nom + le
  téléphone. Réglable ? Non : un ticket est un ticket.

### Décision A3 — les signatures se règlent, et seulement elles

> Sur la facture : « Le client » à gauche, « Pour la société » à
> droite. Sur le bon de livraison : « Le magasinier », « Le
> transporteur », « Reçu par ». Sur le reçu : rien.

Un réglage **par genre** : de 0 à 3 emplacements, chacun avec son
libellé, et pour chacun **une image facultative** (cachet, signature
scannée) qui s'imprime au-dessus du trait — c'est ce que les « images
posées » de l'atelier servaient vraiment à faire. Les libellés
d'usine sont ceux du générateur historique. Stockage : une table
`signature_document (genre, rang, libelle, image_id)` ou un JSON dans
`config_app` — le JSON suffit (une dizaine de lignes par boutique) et
évite une table cloisonnée de plus. **Recommandation : JSON dans
`parametres_societe.signatures`**, même forme que le bloc `signatures`
d'aujourd'hui, étendu à trois.

### Décision A4 — quelques interrupteurs par genre, pas un éditeur

Ce que le générateur sait faire optionnellement, en cases à cocher
dans Paramètres → Documents, **par genre** :

| réglage | valeurs | défaut |
|---|---|---|
| format | A4, A5, ticket 80 mm | A4 (ticket pour le reçu de caisse) |
| colonne remise | oui / non | oui si une ligne en porte |
| colonne TVA | oui / non | selon `tva_active` |
| montant en lettres | oui / non | oui (facture, reçu) |
| récapitulatif TVA | oui / non | oui si TVA |
| référence article | oui / non | non |
| mention de bas de page (texte) | libre | `pied_facture` |

Sept lignes. Si le terrain en réclame une huitième, on l'ajoute — une
case, pas un bloc.

### Ce que ça retire de la v2, et où

| part | sort |
|---|---|
| `pages/Modeles.tsx`, `lib/modeles/*`, `EditeurPiedPage.tsx` | supprimés |
| `noyau/src/modeles.rs`, `images.rs` (images posées), leurs commandes, `modele_document`, `image_document` | supprimés ; migration qui **supprime** les deux tables |
| `ApercuPiece` (« Mes modèles ») | garde l'aperçu, perd le choix du modèle |
| `TESTS-MANUELS.md` § H (H1–H6) | remplacés par « téléverser un en-tête, régler trois signatures, imprimer » |
| D-numéros touchés | D8 reste (images par contenu) ; les décisions de l'atelier (17–18/09) sont **révoquées par A1**, notées comme telles dans DECISIONS |

---

## 3. Chantier B — le journal, jusqu'au bout

### Ce qu'on a

- Le **journal métier** (`journal`, trente types : `vente_creee`,
  `retour_enregistre`, `remise_accordee`, `reglement_annule`,
  `role_change`…) — écrit partout, **lu nulle part** (constaté le
  20/09 : aucun écran ne l'affiche). Les **anomalies** (20/09) y sont,
  et le cahier du jour les montre.
- Le **journal technique du serveur** (20/09) : refus, erreurs,
  commandes lentes, démarrages, sauvegardes, rotation 5 Mo × 3.
- La **fenêtre** ne remonte rien : 61 `console.error` que personne ne
  voit.

### Décision B1 — « Historique » : le journal métier se lit

> « Qui a annulé le règlement de Coulibaly mardi ? » — aujourd'hui la
> réponse est dans la base et nulle part à l'écran.

Un écran **Historique** (menu, permission `journal:lire` — la première
permission de lecture, voir C1) : filtre par jour, par personne, par
type, par pièce ; la ligne dit *quand, qui, quoi, sur quoi, avant →
après*. Sur les deux moteurs, paginé. Depuis une fiche client, un
bouton « historique » ouvre l'écran filtré sur ce client ; idem pièce
et article.

**Coût** : une commande de lecture paginée (`lire_journal_sur`) qui
joint `utilisateur` et résout `entite_id` en libellé ; un écran. Pas
de nouvelle écriture. **Rétention** : rien n'est effacé — un journal
qu'on purge n'est plus un journal ; on verra à 500 000 lignes, pas
avant (index sur `date_evenement`, `entite_id`).

### Décision B2 — les erreurs des caisses remontent au serveur

> Une caisse affiche « erreur technique » à 11 h 04. Ce soir, dans le
> journal du serveur : `11:04:12 [POSTE ] CAISSE-1 · Ventes.tsx ·
> TypeError: … ` — et la ligne d'avant dit quelle commande a refusé.

`window.onerror` et `unhandledrejection` → `POST /journal-poste`
(jeton exigé, 4 Ko max, 10 par minute par poste — au-delà on jette :
un poste en boucle d'erreur ne doit pas remplir le disque du
serveur). Le serveur écrit `[POSTE ]` dans le journal technique avec
le nom du poste. Rien de plus : pas de télémétrie, pas de « clics ».

### Décision B3 — le journal technique se lit dans la console

La console du serveur (`http://localhost:7300`) gagne un onglet
**Journal** : les 200 dernières lignes, filtre par niveau (ERREUR /
REFUS / AVERT / POSTE / INFO), bouton « tout ». Lecture seule, même
permission que la sauvegarde. C'est le seul endroit : la fenêtre n'a
pas à afficher un fichier du serveur.

### Décision B4 — les anomalies sur le tableau de bord

Le cahier du jour les montre déjà ; le tableau de bord gagne un
compteur « anomalies à vérifier » (rouge, seulement s'il y en a), qui
ouvre l'Historique filtré. Une anomalie se **marque vue** (par qui,
quand) — sinon le compteur ne redescend jamais.

---

## 4. Chantier C — des droits plus complets

### Ce qu'on a

29 permissions **d'écriture** au catalogue, six rôles livrés, du
sur-mesure par personne à trois états, tout relu à chaque appel,
liste blanche, `acces_total`. **Les lectures ne sont pas filtrées, et
c'était une décision** : tout utilisateur connecté lit tout, marges
comprises.

### Décision C1 — des permissions de lecture, cinq et pas trente

> Un caissier n'a pas à voir le prix d'achat du sucre, ni la marge
> du mois, ni le relevé de dette d'un client qui n'est pas au
> comptoir.

On ne crée **pas** une permission `:lire` par commande (« en afficher
qui ne feraient rien serait mentir », permissions.md). On en crée
**cinq**, qui correspondent à ce qu'un patron veut vraiment cacher :

| permission | ce qu'elle découvre |
|---|---|
| `achats:lire_prix` | prix d'achat, marges, valeur du stock |
| `rapports:lire` | tableau de bord chiffré, rapports, cahier du jour |
| `tiers:lire_solde` | dettes clients / fournisseurs, relevés |
| `journal:lire` | l'Historique (B1) |
| `caisse:lire_autres` | les sessions et mouvements des **autres** caisses |

Sans la permission, la commande **rend la donnée sans le champ**
(`prix_achat: null`, `marge: null`) ou refuse (rapports, journal) —
jamais un zéro qui ressemble à une vraie valeur. Les rôles livrés :
`patron` tout ; `comptable` tout sauf `caisse:lire_autres` ;
`caissier` et `magasinier` aucune ; `employe` (l'ancien v1) : à
décider (§ 6).

### Décision C2 — les droits valent par dossier

> Ton frère est patron de sa boutique et n'a rien à faire dans la
> tienne. Ta comptable voit les deux.

Avec plusieurs dossiers (D), un rôle **par dossier** :
`utilisateur_dossier (utilisateur_id, dossier_id, role_id)`. Un
utilisateur sans ligne pour un dossier ne le voit pas à la connexion.
`superadmin` et `acces_total` restent globaux. Le sur-mesure reste par
personne, tous dossiers — le cas « plus de droits dans A que dans B »
se règle par deux rôles, pas par un sur-mesure par dossier. **Coût** :
`Appelant` porte déjà `dossier_id` ; `permissions_de_sur` prend le
dossier en plus. Tant qu'il n'y a qu'un dossier, rien ne change.

### Décision C3 — des plafonds, pas seulement des portes

> Le caissier peut faire une remise, mais pas 40 %. Il peut rembourser
> un retour, mais au-delà de 50 000 F c'est le patron qui valide.

Trois plafonds, **par rôle** (colonnes sur `role`, sur-mesure par
personne possible) : `remise_max_pct`, `remboursement_max`,
`credit_max` (vente à crédit). Vide = pas de plafond. La vérification
vit dans `coeur` (pure, testée) et se fait **dans la poignée**, sur
l'argument — le mécanisme de `pieces:antidater`. Le dépassement est un
refus clair : « Remise de 40 % — votre plafond est 15 %. Demander au
patron. » Pas de « validation à distance » (workflow) dans cette v3 :
le patron passe et fait le geste lui-même, avec son compte.

### Décision C4 — voir qui est connecté, et le déconnecter

La console montre déjà les postes. Un onglet **Sessions** dans
Paramètres → Utilisateurs : qui, sur quel poste, depuis quand,
dernière commande ; bouton **Déconnecter** (`revoquer_sur`, existe),
et **désactiver un compte** ferme ses sessions dans le même geste
(aujourd'hui `actif = 0` laisse le jeton vivre jusqu'à expiration —
un trou, à fermer en premier).

### Ce qui n'est PAS dans C

- Pas d'approbation à distance (« demander au patron » par
  notification). Le patron est dans la boutique.
- Pas de permission par magasin (« ce caissier ne voit que le dépôt
  B »). Le dossier (D) fait ce découpage quand il est nécessaire.
- Pas de double authentification.

---

## 5. Chantier D — plusieurs dossiers

Tout est tranché dans [PLAN-MULTISOCIETE.md](PLAN-MULTISOCIETE.md) et
la fondation est posée (D13, 19/09). Trois précisions du propriétaire
le 21/09/2026, qui deviennent D21 et D22.

### Décision D21 — les dates de travail se donnent à la création, et se prolongent

> Tu ouvres le dossier « Quincaillerie » le 15 mars, tu dis : je
> travaille dedans du 1er mars 2026 au 28 février 2027. En janvier
> 2027 tu n'as pas fini les papiers : tu prolonges jusqu'au 31 mars.

Aujourd'hui `creer_dossier_sur(code, societe)` pose **d'office**
l'année civile comme premier exercice. Désormais la création
**demande** les dates de travail (`date_debut`, `date_fin`) — proposées
à l'année civile, modifiables — et c'est le premier exercice.
`prolonger_exercice_sur` existe déjà (`date_prolongation`) ; l'écran
des exercices le rend visible. Le garde-fou `verifier_date_sur`,
branché avant chaque écriture datée, dit clairement : « Le 5 avril
2027 est hors des dates de travail (jusqu'au 31 mars). Prolonger
l'exercice ou en ouvrir un nouveau. » Un dossier n'a jamais de
« trou » : un exercice commence le lendemain de la fin du précédent.

### Décision D22 — SQLite reste, et plusieurs dossiers demandent le serveur, pas PostgreSQL

> Un commerçant seul, une machine : SQLite, rien à installer. Il veut
> un second dossier pour la boutique de son frère : il lance le
> serveur **sur la même machine**, sur la même base SQLite. PostgreSQL
> n'entre en jeu qu'avec plusieurs caisses qui écrivent en même temps.

Les deux moteurs restent de plein droit, et D13 est révisée sur un
mot : plusieurs dossiers demandent **le serveur**, pas PostgreSQL.
Ce qui le rend possible : les 202 commandes ont leur version `Base`,
et les 162 scénarios `*_base.rs` tournent **sur SQLite par défaut** —
c'est le chemin le plus testé de tout le logiciel. Donc :

| | moteur | chemin | dossiers |
|---|---|---|---|
| fenêtre **monoposte** | SQLite | façades `Connection` | **un seul** (le dossier d'origine) |
| **serveur** | SQLite ou PostgreSQL | **`Base` pour tout** | plusieurs |

Ce que ça change dans le code : le serveur cesse d'avoir deux chemins
(`conn` sur SQLite, `Base` sur PostgreSQL — D11) ; il sert **tout par
`Base`** sur les deux moteurs, et le chemin `conn` du serveur part.
`creer_dossier_sur` refuse quand il est appelé **par la fenêtre
monoposte** (« Plusieurs dossiers demandent le serveur — le lancer sur
cette machine suffit »), plus quand la base est SQLite. Le déclencheur
de stock SQLite multi-dossier (ETAPES item 9) doit être corrigé
**avant** — c'est la première tâche de D. Ce qui ne change pas : la
fenêtre monoposte garde ses façades `Connection`, un dossier, comme
depuis le début.

### L'ordre de D

1. Le déclencheur de stock SQLite multi-dossier (item 9) — sinon D22
   n'est pas vrai.
2. Le serveur sert tout par `Base` ; le chemin `conn` du serveur part
   (les tests de routes tournent sur SQLite : ils le prouvent).
3. L'écran des dossiers : créer **avec ses dates de travail** (D21),
   son magasin, son client de passage ; le choix à la connexion
   existe déjà.
4. L'écran des exercices : ouvrir, prolonger, clore ; le garde-fou
   `verifier_date_sur` branché avant chaque écriture datée.
5. La migration d'une base existante : tout ce qui est en `defaut`
   devient le premier dossier, nommé par le patron, avec ses dates.
6. C2 (droits par dossier) se branche ici.
7. Le compte PostgreSQL limité (D10) — pour ceux qui ont PostgreSQL.

---

## 5 bis. Chantier E — le plan comptable, comme fondation

> « Inclure les plans comptables comme prévision dans le futur, mais
> inclus dans v3. » — le propriétaire, 21/09/2026.

### Ce qu'on a

Rien de comptable, et c'est voulu jusqu'ici : Gescom tient le
commerce (stock, caisse, créances, dettes), pas la comptabilité. Mais
le comptable du commerçant, lui, ressaisit chaque mois les ventes,
les achats et la caisse dans son logiciel. C'est ce temps-là que la
fondation vise.

### Décision D23 — SYSCOHADA en base, les opérations affectées, les journaux lus — rien de stocké

Le Mali est dans l'OHADA : le plan est le **SYSCOHADA révisé**. La v3
fait trois choses, et pas une de plus :

1. **Le plan en base** : `compte_comptable (numero, libelle, classe,
   parent)`, **commun à tous les dossiers** (le plan est le même pour
   tout le monde), semé avec les comptes usuels d'un commerce (une
   centaine, classes 1 à 7 : 401, 411, 4431, 4452, 521, 571, 601, 603,
   31x, 701, 7019…). Le patron ou le comptable ajoute des
   sous-comptes (`4111 Client Coulibaly`) — par dossier, avec un
   `dossier_id` facultatif.
2. **L'affectation** : quelle opération de Gescom va sur quel compte,
   en réglage (`affectation_comptable`, par dossier) : vente comptant →
   571 / 701 / 4431 ; vente à crédit → 411 ; règlement client → 571 ou
   521 / 411 ; achat → 601 / 4452 / 401 ; paiement fournisseur ; avoir ;
   retour ; dépense de caisse par catégorie ; écart de clôture ;
   créance irrécouvrable → 6511. Des défauts livrés qui tiennent la
   route ; un écran pour les changer.
3. **Les journaux, lus** : journal des ventes, des achats, de caisse,
   des règlements — **générés à la lecture** depuis `vente`, `achat`,
   `paiement`, `mouvement_caisse`, par période, avec le compte de
   chaque ligne — et **exportés** (CSV, colonnes date / journal /
   compte / libellé / débit / crédit / pièce) pour le logiciel du
   comptable. **Rien n'est stocké** : une écriture comptable
   enregistrée à côté de la vente serait une seconde vérité, et les
   deux finiraient par se contredire — le principe de l'exercice
   (PLAN-MULTISOCIETE § 4) vaut ici aussi. Un test le prouve : le
   total débit = total crédit sur chaque journal, et le journal des
   ventes d'un mois = le chiffre d'affaires du cahier du jour sommé.

### Ce qui n'est PAS dans E (et viendra après)

Balance, grand livre avec report à nouveau, bilan et compte de
résultat, opérations diverses saisies à la main, lettrage,
immobilisations, déclaration de TVA. Tout cela **lit** ce que E pose ;
rien de E ne devra être refait pour l'accueillir — c'est le sens de
« fondation ».

---

## 6. Ce qui a été décidé le 21/09/2026

Quatre questions posées au propriétaire, quatre réponses — toutes
suivent la recommandation. Numérotées dans
[DECISIONS.md](DECISIONS.md) :

| | décision |
|---|---|
| **D17** | **L'atelier part.** Un seul générateur, mise en page fixe par genre, réglages en cases. `modele_document` et `image_document` supprimées par migration. Pas de « mode avancé ». |
| **D18** | **Trois signatures au plus** par genre, chacune avec libellé et **image facultative** (cachet, signature scannée). |
| **D19** | **`employe` ne voit pas les prix d'achat** ni les marges : cinq permissions de lecture (C1), le rôle des vendeurs n'en porte aucune. Le patron rend le droit par le sur-mesure. |
| **D20** | **L'ordre est A, B, C, D** — puis **E**, ajouté le même jour. |
| **D21** | **Les dates de travail** se donnent à la création du dossier (proposées à l'année civile) et se **prolongent** ; le garde-fou refuse une écriture hors dates en disant quoi faire. |
| **D22** | **SQLite reste** un moteur de plein droit ; plusieurs dossiers demandent **le serveur** (sur la même machine s'il le faut), pas PostgreSQL. Le serveur sert tout par `Base` sur les deux moteurs ; la fenêtre monoposte garde un dossier. |
| **D23** | **Le plan comptable SYSCOHADA** en base, l'affectation des opérations en réglage, les journaux **lus** et exportés — **rien de stocké**. La comptabilité complète viendra après, sur ce sol. |

Il ne reste **rien** à décider pour commencer A.

## 7. L'ordre des travaux

| étape | contenu | ce qui le prouve |
|---|---|---|
| A-1 | Réglages Documents (format, cases, signatures) en base + écran | scénarios `documents_base.rs` sur les deux moteurs |
| A-2 | Le générateur unique lit ces réglages ; en-tête / pied images ; signatures avec image | rendu headless comparé genre par genre (script, comme le 20/09) |
| A-3 | Retrait de l'atelier, des tables, des commandes ; migration | `schema_commun` + le socle régénéré ; workspace vert |
| B-1 | `lire_journal_sur` paginé + écran Historique + `journal:lire` | scénario : un règlement annulé se retrouve par le nom du client |
| B-2 | `POST /journal-poste` + `window.onerror` | test de route : une erreur envoyée est dans le fichier, la 11ᵉ de la minute non |
| B-3 | Onglet Journal de la console | test de route : 200 lignes, filtre |
| B-4 | Compteur d'anomalies + « vue » | scénario |
| C-4 | Sessions + désactiver ferme les sessions | scénario `reseau.rs` |
| C-1 | Cinq permissions de lecture | scénarios `permissions.rs` : le caissier reçoit `prix_achat: null` |
| C-3 | Plafonds dans `coeur`, vérifiés dans la poignée | unitaires + scénario |
| C-2 | Droits par dossier | avec D |
| D-1 | Déclencheur de stock SQLite multi-dossier (item 9) | scénario `cloisonnement.rs` sur SQLite |
| D-2 | Le serveur sert tout par `Base` ; le chemin `conn` part | tests de routes (SQLite) + workspace vert |
| D-3 | Écran des dossiers, création avec dates de travail (D21) | scénario `dossiers_base.rs` : dates données, hors dates refusé avec le message |
| D-4 | Écran des exercices, garde-fou branché | scénarios : prolonger, clore, écriture le lendemain d'une clôture |
| D-5 | Migration d'une base existante vers un premier dossier nommé | scénario sur une base v2 amorcée |
| D-6 | Compte PostgreSQL limité (D10) | postgres_amorcage |
| E-1 | `compte_comptable` semé (SYSCOHADA), sous-comptes par dossier | `schema_commun` + scénario : les deux moteurs, ~100 comptes |
| E-2 | `affectation_comptable` + défauts livrés + écran | scénario : chaque type d'opération a un compte |
| E-3 | Journaux lus (ventes, achats, caisse, règlements) + export CSV | scénario : débit = crédit ; ventes du mois = CA du cahier |

Chaque étape est un commit, un scénario, une ligne dans ETAPES.
