# Les étapes : où en est-on, quoi ensuite

Une page, **l'état courant**. Les chiffres sont mesurés, jamais estimés.
Le récit daté de chaque avancée vit dans [JOURNAL.md](JOURNAL.md), les
décisions dans [DECISIONS.md](DECISIONS.md), le multi-société dans
[PLAN-MULTISOCIETE.md](PLAN-MULTISOCIETE.md).

Dernière mise à jour : **19 septembre 2026** (le serveur en service
Windows et son installeur signé ; fondation v3 posée mais dormante — la
v2 d'abord).
État : **453 tests workspace SQLite** (`--workspace`, mesuré le
18/09, 0 échec, 0 avertissement) ; sur **PostgreSQL** (`gescom_test`) :
suite complète 394/394 le 13/09, puis rejoués sans échec les fichiers
touchés à chaque séance — le 18/09 : `gestion_base`, `pieces_base`,
`achats_base`, `fournisseurs_base`, `postgres_amorcage` ;
**158 scénarios** en dix-sept fichiers `*_base.rs` qui tournent sur
les deux moteurs (`GESCOM_PG`). Serveur : **210 commandes** (202 le
18/09 ; +8 de la v3 le 19/09 : dossiers, exercices, choix du dossier),
`POST /entretien` vérifié par HTTP (D9). **29 permissions**
(`dossiers:gerer` le 19/09, patron seul). Dernier commit : voir `git
log`.

---

## Les trois produits

| | ce que c'est | état |
|---|---|---|
| **v1** | un poste, SQLite, pas de serveur | **livré**, ne bouge plus |
| **v2** | serveur + clients, le client ne parle **qu'**au serveur | **close le 18/09/2026** (décision du propriétaire) : fonctionnelle sur SQLite et PostgreSQL ; restent l'installeur non signé (D5), l'impression papier jamais vérifiée à la main, et le déclencheur multi-dossier (v3) |
| **v3** | multi-société, multi-dossier, exercices | **pas commencée** — décision du propriétaire le 19/09/2026 : la v2 d'abord. Fondation posée et **dormante** (D13 : avec un seul dossier, rien ne change à l'écran ni au serveur) ; aucun écran de gestion, on n'y touche pas avant le feu vert |

**Une seule machine ne veut pas dire sans serveur.** La boutique à une
caisse installe les deux sur le même ordinateur. Il n'y a qu'un seul
chemin : le client parle au serveur, jamais à une base locale — deux
pannes réelles ont appris que le repli silencieux est pire que l'arrêt.
→ [ARCHITECTURE.md](ARCHITECTURE.md)

---

## v2 — fait

| chantier | fiche |
|---|---|
| Serveur HTTP écrit à la main, **202 commandes**, canal d'événements, sessions révocables à chaque appel, console | [reseau-v2](modules/reseau-v2.md) |
| Droits : **28 permissions** (dont `pieces:antidater`, vérifiée sur l'argument, et `avoirs:accorder`, patron seul), rôles en base, permissions par personne, écran | [permissions](modules/permissions.md) |
| Stock = somme des mouvements ; numérotation transactionnelle (0 doublon sur 100 × 4 connexions) ; le stock bouge au document qui le constate | [livraison-stock](modules/livraison-stock.md), [numerotation](modules/numerotation.md) |
| Installeur empaqueté, non signé (D5) | [installeur](modules/installeur.md) |
| Modèles de documents, atelier, import/export | [modeles-documents](modules/modeles-documents.md) |
| **PostgreSQL : 202/202 commandes servies sur `Base`**, sauvegarde `pg_dump`, filet `schema_commun` | [postgresql](modules/postgresql.md) |

## v2 — ce qui reste

| # | quoi | pourquoi ça compte |
|---|---|---|
| 1 | ~~**L'installeur n'est pas signé**~~ **le serveur a le sien, signé, le 19/09/2026** (D5 révisée, D12) : `outils\construire_installeur_serveur.ps1`, service Windows, pare-feu, certificat enregistré sur la machine. Celui de la fenêtre reste non signé | chaque installation dépend de SmartScreen ; tenable tant qu'on déploie soi-même (D5) |
| 2 | ~~L'écran des permissions **par personne**~~ **fait le 13/09/2026** : Paramètres → Utilisateurs → « Permissions », à trois états par permission (rôle / autorisée / refusée) | les commandes existent, l'interface non (D7) |
| 3 | ~~Aucun compte `superadmin` n'est créé~~ **réglé le 13/09/2026** : `gescom-serveur --promouvoir IDENTIFIANT` redonne le rôle `superadmin` à un compte existant, depuis la machine du serveur — pas de compte de secours livré (D6) | le rôle existe, personne ne le porte (D6) |
| 4 | ~~Écriture des images depuis une caisse~~ **fait le 13/09/2026** : la caisse lit le fichier et envoie le **contenu** en base64 ; le serveur le range dans son dossier d'images et enregistre le chemin. Refus net : format inconnu, base64 illisible, plus de 10 Mo. La suppression efface aussi le fichier (D8) | la commande recevait un *chemin* local, qui ne désigne rien chez le serveur (D8) |
| 5 | ~~`entretenir_base` reste locale~~ **fait le 13/09/2026** : la route `POST /entretien` du serveur (permission `sauvegarde:lancer`) vérifie l'intégrité, réaffecte les règlements fournisseur globaux, copie avant (VACUUM INTO / pg_dump), compacte (REINDEX+VACUUM / VACUUM ANALYZE) — bouton « Entretien » dans la console ; la caisse garde le diagnostic, perd le bouton | c'est un travail de serveur (D9) |
| 6 | ~~La restauration `pg_restore` jamais jouée~~ **jouée le 13/09/2026** : dump de `gescom_essai` → `gescom_restaure`, serveur redémarré dessus | D4 le demande ; la commande est dans [postgresql.md](modules/postgresql.md) |
| 7 | Une **vraie impression papier**, le glisser-déposer du pied | jamais vérifiés à la main — **reste ouvert à la clôture de la v2** |
| 8 | ~~La fenêtre en **mode caisse**~~ **essayée le 16/09/2026** : `gescom.exe` branché au serveur PostgreSQL (`gescom_essai`), tableau de bord, POS et atelier vus à l'écran avec de vraies données. Elle a révélé le glisser-déposer cassé (`dragDropEnabled`), invisible depuis un navigateur. Reste l'impression papier (item 7) | tous les essais passaient par HTTP ; **un essai par navigateur ne remplace pas la fenêtre** |
| 9 | Le déclencheur de stock sur SQLite multi-dossier | un mouvement de `dossier-b` crée sa ligne de stock dans `defaut` ; sans effet tant qu'une base SQLite n'a qu'un dossier — à régler avec la v3 |

## Séance du 17/09/2026 — ce qui reste ouvert

| # | quoi | où |
|---|---|---|
| I1 | ~~**Vingt blocs Image partagent une seule image**~~ **fait le 17/09/2026** : table `image_document` (deux chemins de création, `schema_commun`), `imageId` sur le bloc Image, quatre commandes, **suppression refusée tant qu'un modèle la pose** (le refus nomme lesquels), **l'export emporte les images** (version d'échange 2, un lot v1 se lit toujours). Quatre scénarios | `noyau/src/images.rs`, `noyau/src/modeles.rs`, `lib/modeles/*` |
| I2 | ~~**La date au POS**~~ **fait le 17/09/2026** : `creer_vente_datee_sur*` (les `creer_vente_sur*` restent des enveloppes, 33 appels intacts), règle pure dans `coeur/dates.rs` (pas de futur, 31 jours de recul max), permission **`pieces:antidater`** vérifiée par le serveur, `libelle` de caisse « Vente du 03/09 ». Deux scénarios + six tests unitaires | `noyau/src/coeur/dates.rs`, `noyau/src/argent.rs` |
| I3 | ~~**La date d'un règlement**~~ **fait le 17/09/2026** : `regler_creance_datee*` et `regler_dette_fournisseur_datee*` (les fonctions d'origine restent des enveloppes), même règle, même permission, le `paiement` porte la date de l'affaire, la caisse reste au jour avec « Règlement du jj/mm ». Champ « Réglé le » dans les fiches client et fournisseur. Deux scénarios | `noyau/src/creances.rs`, `noyau/src/argent.rs` |
| I5 | ~~**L'export et l'import des modèles ne marchent pas depuis une caisse.**~~ **fait le 18/09/2026** : `exporter_modeles` (lecture, rend le lot) et `importer_modeles` (`modeles:gerer`, reçoit le lot) sont des commandes du **serveur**, sur les deux poignées ; l'écran lit et écrit le **fichier** lui-même (`plugin-fs`, capacité `fs:allow-write-text-file`). Plus dans `LOCALES`. Vérifié par HTTP sur un serveur jetable : import de 2 → export les rend | `serveur/src/socle.rs`, `src/pages/Modeles.tsx` |
| I4 | ~~**Le champ de saisie de la référence**~~ **fait le 17/09/2026** : `definir_reference_piece`, colonne `reference` en fin des quatre listes (attention : la liste fournisseur de la fenêtre n'a pas `credit_ouvert`, l'indice y est 18 et non 19), recherche `LOWER(COALESCE(pc.reference,''))`, champ « réf. » sous le numéro | `noyau/src/pieces.rs`, `src/pages/Pieces.tsx` |

## Séance du 18/09/2026 — fait

| # | quoi | où |
|---|---|---|
| J1 | **L'irrécouvrable ne se règle plus en douce** : `peut_regler` (coeur) refuse le chemin ordinaire ; le **règlement exceptionnel** (`regler_creance_exceptionnel`, permission `chantiers:gerer`) encaisse sous le motif de caisse `recouvrement`, plafonné au reste, et la vente ne redevient `payee` que si tout est rentré (`statut_apres_recouvrement`). Bouton dans Paramètres → Irrécouvrable. La somme cumulée des paiements départage les horodatages identiques (horloge Windows à 15 ms) par `(date, cree_le, id)` | `noyau/src/creances.rs`, `noyau/src/coeur/calcul.rs` |
| J2 | **L'avoir accordé sans marchandise** (geste commercial) : `accorder_avoir_client[_sur_base]`, permission **`avoirs:accorder`** que seul `acces_total` porte — aucun rôle livré ne l'a, le patron la donne à la main s'il le veut. Avoir `retour_id NULL` + pièce AVC `origine 'geste'`, sans `ligne_piece` ; le document imprimé porte une ligne d'affichage tirée de `avoir.montant`. Client de passage, montant nul, motif vide : refusés. Bouton dans la fiche client → Avoirs | `noyau/src/avoirs.rs`, `noyau/src/pieces.rs` (`ligne_d_avoir_accorde`), `src/pages/FicheClient.tsx` |
| J3 | **Les blocs flottants** : `flottant {xMm,yMm,largeurMm,hauteurMm}` sur tout bloc ; posé en absolu dans une `.feuille` dont le coin haut-gauche est celui de la zone imprimable (même origine écran / papier) ; **deux blocs peuvent se recouvrir, l'ordre de la structure est l'ordre de superposition** (`z-index` = rang). Dans l'aperçu : tirer pour déplacer, le coin bas-droit pour redimensionner (écouteurs `pointer*` posés par l'atelier sur le DOM de l'iframe, le modèle n'est écrit qu'au relâcher). Le dépôt dans l'aperçu se fait désormais **par identifiant** de bloc, pas par rang — un bloc caché ou flottant ne décale plus la cible | `src/lib/modeles/types.ts`, `rendu.ts`, `src/pages/Modeles.tsx` |
| J4 | **La date de la réception** : `enregistrer_achat_date[_sur_base]` (l'original reste une enveloppe), même règle et même permission que la vente : la FAF et le `paiement_fournisseur` portent la date de l'affaire, le stock et la caisse bougent au jour (« Réception du jj/mm »). Champ ambre au-dessus d'« Enregistrer la réception » | `noyau/src/achats.rs`, `src/pages/Achats.tsx` |
| J5 | **La palette de commandes** (Ctrl+K, toutes les pages) : navigation (mêmes droits que le menu), onglets de Paramètres (`lib/onglets-parametres.ts`, exporté hors du composant pour le rechargement à chaud), compte, et les actions **déclarées par l'écran monté** (`useActionsPalette`) : Clients, Fournisseurs, Pièces, Caisse (ouvrir / fermer selon l'état). Chaque action porte son `droit` | `src/lib/palette.ts`, `src/components/PaletteCommandes.tsx`, `src/components/Layout.tsx` |
| J6 | La date au POS est **juste au-dessus d'Encaisser** (cadre ambre quand elle est posée) | `src/pages/Ventes.tsx` |
| J7 | **La palette cherche les tiers** : dès deux lettres, cinq clients et cinq fournisseurs dont le nom correspond (`lire_*_pagines`, mêmes droits que le menu) ; choisir ouvre la fiche. Plus large (`max-w-3xl`, élargie une seconde fois le 18/09) | `src/components/PaletteCommandes.tsx` |
| J8 | **Pièces : Du → au dans la barre** (la même paire que les filtres avancés), et la liste fournisseur accepte enfin `date_debut`/`date_fin` (deux versions, serveur, façade, scénario). **Le filtre dit le type** : sur « Commandes », le bouton devient « Nouvelle commande » et la fenêtre n'a plus de sélecteur ; sur « Tout », on choisit — client comme fournisseur | `noyau/src/pieces.rs`, `src/pages/Pieces.tsx`, `src/components/ModalNouvellePiece.tsx` |
| J9 | **La date se saisit aussi à la création d'une pièce**, pas seulement à l'échéance : `creer_piece[_sur_base]` et `creer_piece_fournisseur[_sur_base]` prennent `date_piece` (même règle que la réception/le règlement, `pieces:antidater` côté serveur). Rien d'autre ne bouge — `creer_piece*` ne touche ni stock ni caisse. Champ « Date de la pièce » dans la modale, visible à qui a le droit ; scénario `une_piece_peut_naitre_deja_datee` | `noyau/src/pieces.rs`, `noyau/src/argent.rs` (`date_de_la_piece`), `src/components/ModalNouvellePiece.tsx` |
| K5 | **« UNIQUE constraint failed: client.code »** à chaque nouveau client dès qu'un code plus grand que le nombre de clients existait (client supprimé, import) : le code suivait `COUNT + 1`. Il suit le **plus grand déjà pris** (`coeur::tiers::code_client_suivant`, D28), et un dossier autre que l'origine préfixe ses codes de son code (`QUINC-CLIENT00001`) — `client.code` est unique sur toute la base. Deux scénarios | `noyau/src/comptoir.rs`, `coeur/tiers.rs`, `dossiers.rs` |
| K6 | **POS : remise globale en % ou en francs**, répartie sur les lignes au prorata (la dernière prend le reste) ; l'invariant `SUM(prix_pratique × quantité) = dû` tient, HT/TVA se relisent sur les lignes remisées | `src/pages/Ventes.tsx` |
| K7 | **Avoir sans marchandise depuis « Nouvelle pièce »** : type Avoir, aucune ligne, un montant + le motif dans Note → `accorder_avoir_client` (permission `avoirs:accorder`, sinon l'écran le dit). Le crédit se consomme sur une vente ou se rembourse | `src/components/ModalNouvellePiece.tsx` |
| K8 | **Impression « parfois » cassée** : le fichier temporaire portait le nom demandé tel quel — le même deux fois (cache ou fichier encore tenu par la webview : ancien document ou page blanche), parfois **sans `.html`** (le numéro de pièce nu, WebView2 devinait le type). Nom unique + `.html` garantis, dossier `gescom_impression` nettoyé après 24 h, second essai de label si la fenêtre précédente n'a pas fini de se fermer | `src-tauri/src/commandes/impression.rs` |
| K10 | **Un bon se prépare en brouillon, s'émet, et ne se modifie plus** : modifier un BL émis laissait le stock à l'ancienne quantité. Règle dans `coeur` (`constate_le_stock`, `changement_de_statut`, `peut_livrer`, `peut_transferer_un_bon`) : un bon (livraison, réception) créé à la main naît **brouillon** (rien ne bouge, modifiable), **Émettre** le livre entièrement (`marquer_entierement_livre`, même transaction que le statut), puis il est figé ; la saisie ligne à ligne ne vient qu'après pour corriger ; **annuler** un bon émis ramène la marchandise (`marquer_rien_livre`) ; un bon en brouillon ne se facture pas (sa facture ne sortirait jamais rien). Par conversion d'une commande, le BL naît émis et livré, comme avant. Bouton « Émettre » dans Pièces ; six scénarios adaptés, un ajouté | `coeur/pieces.rs`, `noyau/src/pieces.rs`, `livraisons.rs`, `src/pages/Pieces.tsx` |
| K9 | **Tableau de bord sans icônes** : `KpiCard` / `KpiPetit` perdent la tuile, l'intitulé passe en tête en `text-sm font-semibold` ; « tinted » teinte le bord | `src/components/ui/KpiVerre.tsx` |
| K1 | **Le serveur en service Windows** (D12) : `service.rs` (`windows-sys`, quatre appels), `--installer-service` / `--desinstaller-service` / `--service`, configuration `ProgramData\Gescom\serveur.json`, journal `serveur.log`, boucle d'écoute non bloquante qui s'arrête sur `sc stop` et révoque les sessions. **Pas déroulé en élevé** (UAC bloqué depuis la session) : TESTS-MANUELS §A | `serveur/src/service.rs`, `main.rs` |
| K2 | **L'installeur du serveur, signé** (D5 révisée) : `signer.ps1` (certificat auto-signé, 10 ans, clé jamais exportée), `installeur_serveur.nsi` (admin, page base/port, certificat, pare-feu, service, désinstallation qui garde les données), `construire_installeur_serveur.ps1`. Construit : `dist\Gescom-Serveur_0.2.0_x64-setup.exe`, 2,4 Mo, signé | `outils/` |
| K3 | **TESTS-MANUELS.md** (dix sections, à cocher) et **MANUEL.md v2.0** (installation, palette, dates, modèles, avoir accordé, irrécouvrable) | racine |
| K4 | **La v3 commence** (D13) : `dossiers::lire_dossiers_sur`, `creer_dossier_sur` (exercice + magasin + client de passage, refus sur SQLite), `dossier_memorise_sur` ; la session porte son dossier (`session_reseau.dossier_id`, deux chemins de création), `Appelant.dossier_id`/`session_id`, `api::rpc` place la `Base` sur le dossier ; `Registre::sur_base` ; 8 commandes ; écran de choix dans `PageLogin`, dossier affiché dans `Layout`. Vérifié par HTTP sur `gescom_essai` : second dossier créé, choix, mémorisation, refus du second choix. **5 scénarios** `dossiers_base` sur les deux moteurs | `noyau/src/dossiers.rs`, `sessions.rs`, `registre.rs`, `serveur/src/api.rs`, `src/pages/PageLogin.tsx` |
| J11 | **La facture irrécouvrable se lit en rouge et ne pèse nulle part** : les listes client rendent `irrecouvrable` (colonne 20, `CASE WHEN EXISTS(vente irrecouvrable)`, un entier pour que PostgreSQL et SQLite s'accordent) ; « impayés » et « en retard » l'excluent (deux versions) ; à l'écran, ligne rouge, badge « Irrécouvrable », hors des totaux comme une annulée. Scénario dans `gestion_base` | `noyau/src/pieces.rs`, `src/pages/Pieces.tsx` |
| J10 | **Six retouches d'après capture** : (a) un AVF **remboursé** affichait « Payé » et un reste rouge de son montant — la règle `credit_avoir_fournisseur` est dans `coeur` (remboursé/annulé → 0, sinon le crédit) et sert aux deux listes et aux trois états de dette ; le reste d'un avoir se lit en ambre (crédit), et le pied de tableau n'additionne que les factures ; (b) la palette ne s'élargissait pas : `DialogContent` pose `sm:max-w-sm`, il faut `sm:max-w-3xl` ; (c) **Caisse plantait** : `useActionsPalette` était appelé après le `return` du chargement (nombre de hooks variable) — remonté avant ; (d) Paramètres → Société n'a plus la section « Signatures » : les noms vivent dans le bloc Signatures de chaque modèle ; (e) **chaque bloc se redimensionne dans l'aperçu** : tirer le coin bas-droit d'un bloc du flux le détache (flottant, à sa place, même cadre) dans le même geste ; cocher « flottant » garde aussi le cadre à l'écran ; (f) un champ « Client » posé sur `tiers.*` s'imprime « Fournisseur » sur une pièce fournisseur | `coeur/calcul.rs`, `noyau/src/pieces.rs`, `fournisseurs.rs`, `src/pages/Caisse.tsx`, `Modeles.tsx`, `lib/modeles/rendu.ts`, `ParametresSociete.tsx` |

**La règle posée le 17/09, à ne pas défaire** : *deux dates, deux faits*.
Elle est écrite dans [coeur/dates.rs](../src-tauri/noyau/src/coeur/dates.rs)
et gardée par la permission `pieces:antidater` (28 permissions au catalogue).
La pièce dit quand l'affaire a eu lieu et se saisit ; le mouvement de
caisse ne bouge pas. La caisse se lit **par session**, jamais par date —
et on ne peut pas ouvrir la session d'un jour passé. Antidater une
entrée changerait après coup une session close et comptée, et c'est
aussi ainsi qu'on masque un trou dans le tiroir.

## Revue du 16/09/2026 — les cinq écarts, tous corrigés

Relecture des lots du 13/09 contre les règles de CLAUDE.md. Le récit est
dans [JOURNAL.md](JOURNAL.md) § 16/09/2026. **Les cinq sont corrigés**
(R1, R2, R5 puis R3, R4) ; restent les mineurs ci-dessous.

| # | quoi | où |
|---|---|---|
| R1 | ~~**La réimputation des règlements globaux réécrivait de l'argent hors transaction**~~ **corrigé le 16/09/2026** : `reimputer_paiements_globaux_sur_base` ouvre `base.transaction()` et passe `&mut tx` aux deux aides — le `DELETE` du règlement global et les `INSERT` qui le reposent sont désormais tout ou rien (règle 4) | `noyau/src/chantiers.rs`, `noyau/src/argent.rs` (`reallouer_globaux_sur`) |
| R2 | ~~**Sur SQLite la copie de sécurité était faite APRÈS la réimputation**~~ **corrigé le 16/09/2026** : `persistance::entretenir` se scinde en `copier_avant` et `compacter`, et l'entretien copie → réimpute → compacte, comme le `pg_dump` le fait déjà sur PostgreSQL. Scénario : la copie relue montre l'état d'avant | `noyau/src/parametres.rs`, `noyau/src/persistance/mod.rs` |
| R3 | ~~**La lecture des images oubliait le dossier sur PostgreSQL**~~ **corrigé le 16/09/2026** : les trois `lire_*_base64` sur base appellent `dossier_des_images_base`, le même que l'écriture et la suppression | `serveur/src/socle.rs` |
| R4 | ~~**Changer d'extension laissait l'ancienne image**~~ **corrigé le 16/09/2026** : `poser_fichier` écrit le nouveau fichier puis efface les autres extensions du même genre — dans cet ordre, pour qu'un échec d'écriture laisse l'image précédente. Scénario : png → jpg, le png disparaît et le repli rend le jpg | `noyau/src/images.rs` |
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
  l'interblocage du 13/09, et R1 à R3 étaient toutes sur ce chemin —
  corrigées par lecture, pas par un test qui les aurait attrapées. Un
  seul test de route vaudrait cher.
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
- ~~Un scénario instable (`gestion_base`, une fois sur trois)~~ **réglé
  le 17/09/2026** : l'horloge Windows tique par 15 ms, deux règlements
  du même tic ont le même horodatage. Départage `cree_le, id` dans les
  deux requêtes, et le scénario cherche chaque règlement par son
  montant, pas par sa position.
- Hors code : signature de l'installeur (D5), impression papier réelle.

## v3 — fondation posée, pas commencée

Un dossier = **une société × un exercice**. Forme retenue : une colonne
`dossier_id` sur les 23 tables cloisonnées, `Base` porte son dossier, un
détecteur refuse **avant d'exécuter** toute requête qui oublie le
filtre. Tables `dossier` et `exercice`, règle des dates pure et testée.
Pas cloisonnés, et c'est voulu : `utilisateur`, `role`, `poste`,
`session_reseau`, `modele_document`, `article`, `unite_vente`,
`config_app`, `parametres_societe`.

Posé le 19/09/2026 (D13), **sans le lancer** : le choix du dossier à la
connexion, la création d'un dossier, les commandes d'exercices — tout
reste dormant tant qu'il n'y a qu'un dossier, et c'est le cas. La v3
ne commence pas avant le feu vert du propriétaire ; on finit la v2.
Ce qui restera alors : l'écran de
gestion (créer un dossier, ouvrir / prolonger / clore un exercice —
les commandes existent), le garde-fou `verifier_date_sur` branché
avant chaque écriture, et la migration d'une base existante vers
plusieurs dossiers.
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
