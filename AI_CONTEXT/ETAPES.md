# Les étapes : où en est-on, quoi ensuite

Une page, **l'état courant**. Les chiffres sont mesurés, jamais estimés.
Le récit daté de chaque avancée vit dans [JOURNAL.md](JOURNAL.md), les
décisions dans [DECISIONS.md](DECISIONS.md), le multi-société dans
[PLAN-MULTISOCIETE.md](PLAN-MULTISOCIETE.md).

Dernière mise à jour : **24 septembre 2026 — la v3 est faite (A à E,
branche `v3`, poussée) ; Gescom Équipe (personnel, paie, suivi
client) est planifiée** — [PLAN-EQUIPE.md](PLAN-EQUIPE.md), D28 à D34,
chantiers F, G, H, sur la branche `rh`. Avant : **23 septembre 2026 — revue de la v2, trois
défauts corrigés sur la branche `correctif/revue-v2`** (à fusionner
dans `main` ; § « Revue du 23/09/2026 » plus bas). Avant : **21
septembre 2026 — la v2 est close.** Le
code est fini (tout ce qui pouvait se faire sans le propriétaire
devant sa machine l'a été : restauration, routes HTTP testées, chèque
rejeté, filtres liés, codes-barres dessinés, retour d'une vente payée
avec un avoir (D15), anomalies dans le cahier du jour, journal
technique, coordonnées de l'en-tête au champ). **Les vérifications
manuelles (installeur en élevé, impression papier, deux machines,
restauration jouée pour de vrai) n'ont pas été déroulées** — décision
du propriétaire : on ferme quand même, c'est de la dette connue, pas
un blocage. Le bon de livraison partiel reste un document à quantité
pleine (D16), assumé. **La v3 commence à la prochaine séance.**
État : **463 tests workspace SQLite** (`--workspace`, mesuré le
20/09, 0 échec, 0 avertissement) ; sur **PostgreSQL** (`gescom_test`) :
suite complète 394/394 le 13/09, puis rejoués sans échec les fichiers
touchés à chaque séance — le 20/09 : `journal_rapports_base`,
`pieces_base`, `listes_base`, `achats_base`, `gestion_base`,
`retours_base`, `caisse_base` ;
**162 scénarios** en dix-sept fichiers `*_base.rs` qui tournent sur
les deux moteurs (`GESCOM_PG`), plus **3 tests de routes HTTP**
(`serveur/tests/routes.rs`, le vrai exécutable, le vrai JSON de
l'écran). Serveur : **210 commandes** (202 le
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
| ~~Modèles de documents, atelier, import/export~~ **retirés en v3 (A-3, D17)** : un générateur par genre, réglé dans Paramètres → Documents | [documents](modules/documents.md) |
| **PostgreSQL : 202/202 commandes servies sur `Base`**, sauvegarde `pg_dump`, filet `schema_commun` | [postgresql](modules/postgresql.md) |

## v2 — close le 21/09/2026

Le code est fini ; les quatre lignes ci-dessous restent **dette connue,
pas testées**, décision du propriétaire de fermer sans les dérouler.
Elles se vérifient le jour où l'occasion se présente (une vraie
installation, une vraie imprimante, un deuxième poste) — rien dans le
code n'empêche de le faire alors.

## v2 — ce qui reste (dette assumée, pas bloquante)

| # | quoi | pourquoi ça compte |
|---|---|---|
| 1 | ~~**L'installeur n'est pas signé**~~ **le serveur a le sien, signé, le 19/09/2026** (D5 révisée, D12) : `outils\construire_installeur_serveur.ps1`, service Windows, pare-feu, certificat enregistré sur la machine. Celui de la fenêtre reste non signé | chaque installation dépend de SmartScreen ; tenable tant qu'on déploie soi-même (D5) |
| 2 | ~~L'écran des permissions **par personne**~~ **fait le 13/09/2026** : Paramètres → Utilisateurs → « Permissions », à trois états par permission (rôle / autorisée / refusée) | les commandes existent, l'interface non (D7) |
| 3 | ~~Aucun compte `superadmin` n'est créé~~ **réglé le 13/09/2026** : `gescom-serveur --promouvoir IDENTIFIANT` redonne le rôle `superadmin` à un compte existant, depuis la machine du serveur — pas de compte de secours livré (D6) | le rôle existe, personne ne le porte (D6) |
| 4 | ~~Écriture des images depuis une caisse~~ **fait le 13/09/2026** : la caisse lit le fichier et envoie le **contenu** en base64 ; le serveur le range dans son dossier d'images et enregistre le chemin. Refus net : format inconnu, base64 illisible, plus de 10 Mo. La suppression efface aussi le fichier (D8) | la commande recevait un *chemin* local, qui ne désigne rien chez le serveur (D8) |
| 5 | ~~`entretenir_base` reste locale~~ **fait le 13/09/2026** : la route `POST /entretien` du serveur (permission `sauvegarde:lancer`) vérifie l'intégrité, réaffecte les règlements fournisseur globaux, copie avant (VACUUM INTO / pg_dump), compacte (REINDEX+VACUUM / VACUUM ANALYZE) — bouton « Entretien » dans la console ; la caisse garde le diagnostic, perd le bouton | c'est un travail de serveur (D9) |
| 6 | ~~La restauration `pg_restore` jamais jouée~~ **jouée le 13/09/2026** : dump de `gescom_essai` → `gescom_restaure`, serveur redémarré dessus | D4 le demande ; la commande est dans [postgresql.md](modules/postgresql.md) |
| 7 | Une **vraie impression papier**, le glisser-déposer du pied | jamais vérifiés à la main — **la v2 a fermé sans, le 21/09/2026** |
| 8 | ~~La fenêtre en **mode caisse**~~ **essayée le 16/09/2026** : `gescom.exe` branché au serveur PostgreSQL (`gescom_essai`), tableau de bord, POS et atelier vus à l'écran avec de vraies données. Elle a révélé le glisser-déposer cassé (`dragDropEnabled`), invisible depuis un navigateur. Reste l'impression papier (item 7) | tous les essais passaient par HTTP ; **un essai par navigateur ne remplace pas la fenêtre** |
| 9 | ~~Le déclencheur de stock sur SQLite multi-dossier~~ | **réglé en v3 D-1** : le déclencheur pose le dossier du mouvement ; les lignes mal rangées reviennent à l'amorçage |
| 10 | **L'installeur en élevé, deux machines, restauration réelle** : jamais déroulés (TESTS-MANUELS §A, B, J3) | code prêt, geste non fait — dette assumée à la clôture (21/09/2026) |

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
| K15 | **Choisir quelles coordonnées afficher dans l'en-tête** : la case « Afficher les coordonnées de la société » était tout-ou-rien (nom + adresse + téléphone + email + NIF + RCCM) — impossible de garder le téléphone et enlever le RCCM sur un modèle précis. Six cases (le nom reste toujours affiché, c'est l'ancre du bloc) ; **téléphone 2** ajouté, il existait dans Paramètres → Société sans apparaître sur aucun document. `champsSociete?: string[]` sur le bloc, absent = les cinq d'origine (rétro-compatible, aucune migration) | `lib/modeles/types.ts`, `rendu.ts`, `src/pages/Modeles.tsx` |
| K14 | **Un système de traces, enfin** : (a) les **anomalies** du noyau (montant ni imputable ni remboursable, lien vente→pièce non écrit) étaient des `eprintln!` sur une console que personne ne regarde — elles sont des événements `anomalie` du `journal`, écrits dans la transaction du geste (annulée avec lui), et le **cahier du jour** les montre en rouge en tête ; (b) le **journal technique du serveur** (`serveur/src/journal_technique.rs`, sans crate) : une ligne horodatée par refus (401/403/404/409 avec le message), par erreur (5xx), par commande lente (> 2 s), par démarrage/arrêt/sauvegarde, avec le contexte `ip METHODE /route · commande · utilisateur@poste` ; rotation à 5 Mo × 3 ; `--journal FICHIER` ou clé `journal` de `serveur.json`, sinon `gescom.log` à côté de la base fichier / `ProgramData` en service ; jamais de mot de passe (D10). Le test de routes vérifie les lignes et l'absence du mot de passe. Ce qui reste (v3) : les erreurs de la fenêtre remontées au serveur | `noyau/src/journal.rs`, `retours.rs`, `argent.rs`, `serveur/src/journal_technique.rs`, `api.rs`, `main.rs`, `src/pages/Journal.tsx` |
| K13 | **Rendre ce qu'on a payé avec un avoir rend l'avoir** (D15) : le plafond « jamais plus que versé » excluait les paiements par avoir (« pas d'argent reçu ») — un client qui achetait avec son avoir puis rendait la marchandise perdait tout, une ligne dans le journal et rien pour lui. Règle pure `coeur::calcul::repartir_retour` : l'argent revient en argent, l'avoir en avoir, le surplus est signalé ; appliquée au remboursement (espèces + avoir en un geste, l'écran dit combien ouvrir le tiroir) et au reliquat d'échange. Deux scénarios | `coeur/calcul.rs`, `noyau/src/retours.rs`, `src/components/ModalsRetour.tsx` |
| K12 | **La dette v2 payable sans le propriétaire** (20/09) : (a) **restaurer** — `sauvegarde::restaurer` + `gescom-serveur --restaurer FICHIER`, hors ligne : `pg_restore --clean --if-exists` sur PostgreSQL, sur SQLite contrôle d'intégrité + table `vente` exigée, copie `.avant-restauration-<date>` gardée, `-wal`/`-shm` retirés ; (b) **trois tests de routes HTTP** qui lancent le vrai binaire sur une base temporaire : 401/403/404, la livraison avec le JSON de l'écran (snake et camel), sauvegarde → `--restaurer` → redémarrage ; (c) **chèque rejeté** : plus de suppression, une contre-passation (`paiement` négatif `origine = 'rejet_cheque'`, sortie de caisse `cheque_rejete`) — D14 ; (d) les cinq listes filtrées du poste (`lire_toutes_pieces_client/fournisseur`, clients, stocks, fournisseurs paginés) **lient** leurs filtres au lieu de coller du texte échappé — scénario `filtres_lies` avec apostrophe et `%` ; (e) `ModalImpression` retiré, le POS ouvre `ApercuPiece` (modèles, formats) ; (f) **étiquettes EAN-13 dessinées** en SVG (`lib/ean13.ts`, 95 modules, 31 mm), le numéro en clair dessous | `noyau/src/sauvegarde.rs`, `cheques.rs`, `pieces.rs`, `pagination.rs`, `serveur/src/main.rs`, `serveur/tests/routes.rs`, `noyau/tests/filtres_lies.rs`, `src/lib/ean13.ts`, `src/pages/Ventes.tsx` |
| K11 | **Livraison : les filtres suivent, et deux filtres qui ne filtraient rien** — la saisie ligne à ligne échouait sur « missing field ligne_id » (l'écran envoyait `ligneId` en camelCase, un champ imbriqué n'est pas renommé : `serde(alias)` + l'écran en snake_case) ; le filtre **Livraison / Réception** (non livré, partiel, livré) apparaît quand le suivi est actif ; le filtre **Échéance** existait sans jamais s'appliquer, la case **Impayés** envoyait `impaySeulement` que le serveur ne lisait pas. Et la réponse à « que fait le stock si on annule une facture issue d'un BL » : la marchandise **revient une fois** (retour porté par l'avoir), le bon garde sa trace « livré » — scénario `annuler_par_avoir_une_facture_issue_d_un_bon_…` | `livraisons.rs`, `src/components/FiltresAvances.tsx`, `ModalLivraison.tsx`, `src/pages/Pieces.tsx` |
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

## Revue du 23/09/2026 — branche `correctif/revue-v2`

Relecture de la v2 close, à la recherche d'un défaut majeur. Quatre
défauts **reproduits par un test avant d'être corrigés** ; le récit est
dans [JOURNAL.md](JOURNAL.md) § 23/09/2026, les règles dans
[ALERTES.md](ALERTES.md) et [DECISIONS.md](DECISIONS.md) D26–D27.

| # | quoi | où |
|---|---|---|
| V1 | **L'auteur d'un geste était le premier compte de son rôle** (D26) : deux caissiers, toutes les ventes, remises, règlements et ouvertures de caisse signés par le premier. Le serveur pose l'utilisateur de la session sur le fil de la requête ; les cinq aides d'auteur le lisent d'abord. ~100 appels corrigés, aucune signature touchée | `noyau/src/auteur.rs`, `argent.rs`, `comptoir.rs`, `serveur/src/api.rs` |
| V2 | **Le serveur ne jugeait pas la saisie** (D27) : `regler_creance` en mode « avoir » effaçait la dette sans avoir ni argent ; `enregistrer_paiement` acceptait un montant négatif (entrée de caisse négative) ; une vente, une réception ou une pièce acceptait une quantité négative (le stock remontait) ; un règlement fournisseur négatif faisait « apparaître » de l'argent. Règles pures dans `coeur/saisie.rs`, appelées par les deux versions ; `enregistrer_paiement` passe par `regler_creance_datee` | `coeur/saisie.rs`, `creances.rs`, `argent.rs` (dont `regler_dette_fournisseur*`), `achats.rs`, `pieces.rs` |
| V3 | **Le règlement d'une créance sur SQLite n'était pas en transaction** (règle 4) — c'est le chemin du serveur sur fichier ; l'écriture en caisse pouvait échouer en silence. Tout ou rien, prouvé par une panne simulée | `creances.rs::regler_creance_datee` |
| V4 | **PostgreSQL : une erreur ignorée dans une transaction faisait disparaître toute l'opération**, la commande rendant `Ok` (COMMIT d'une transaction avortée = ROLLBACK muet). `Transaction::valider` vérifie que la transaction vit encore et refuse sinon | `noyau/src/base.rs` |

Tests : `noyau/tests/revue_v2_base.rs` (11 scénarios, les deux
moteurs, dont 2 sur la version `Connection`), un test de route HTTP
(`deux_comptes_du_meme_role_signent_chacun_leur_vente`, qui échoue si
on retire la garde d'`api.rs`), 12 tests unitaires (`coeur::saisie`,
`auteur`). Suite complète : **466 tests SQLite**
(`-p gescom-noyau -p gescom-serveur`) et **461 sur PostgreSQL** (noyau,
`GESCOM_PG`, `--test-threads=1`), mesurés le 23/09 sous Linux ; seul
`installation.rs` échoue, il ne vaut que sous Windows.

**Ce qui reste, vu pendant la revue, pas corrigé ici** :
- la **caisse nominative** (`caisse_par_utilisateur`) est dormante et
  cassée : tous les appels à `caisses::exiger*` passent `None` comme
  utilisateur (l'activer bloquerait tout encaissement) et l'ouverture
  refuse une seconde session. Aucun écran ne l'active ; la commande
  `definir_mode_caisse` reste servie (`caisse:configurer`). À reprendre
  avec la v3 (chantier C), en passant par `auteur::courant()` ;
- l'auteur passé en **argument** plutôt que par le fil (D26, « ce qu'on
  ne fait pas ») ;
- les autres commandes d'argent n'ont pas été relues une à une contre
  `coeur/saisie.rs` : avoirs, dépenses, chèques et retours ont déjà
  leur garde `montant <= 0` ou `quantite <= 0`, mais aucune ne vérifie
  le mode de paiement contre la liste ;
- sur PostgreSQL, les écritures « non bloquantes » (`let _ =`) dans
  une transaction font désormais échouer le geste si elles échouent
  (V4) : c'est voulu, mais une trace facultative devrait s'écrire
  après `valider()`.

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

- ~~**Aucune restauration dans l'application**~~ **fait le 20/09/2026**
  en ligne de commande, serveur arrêté : `gescom-serveur --restaurer
  FICHIER` (K12). Pas de bouton dans l'écran : restaurer par-dessus une
  base en service est le geste qu'on ne veut pas rendre facile.
- ~~**Les routes HTTP ne sont pas testées**~~ **trois tests le
  20/09/2026** (`serveur/tests/routes.rs`) : le vrai exécutable sur une
  base temporaire, connexion, refus, une livraison avec le JSON de
  l'écran, sauvegarde et restauration. Ils auraient attrapé le
  « missing field ligne_id » du 19/09.
- **Les commandes Tauri n'ont aucun test** — à commencer par
  `creer_vente`, `valider_facture`, `regler_dette_fournisseur`.
- ~~**Les erreurs de la fenêtre ne remontent pas**~~ — fait en v3
  B-2 : `window.onerror` et `unhandledrejection` → `POST
  /journal-poste` → `[POSTE ]` dans le journal technique. Les
  `console.error` d'une erreur *attrapée* restent locaux.
- ~~**Un chèque rejeté ne défait pas son mouvement de caisse**~~
  **corrigé le 20/09/2026** : contre-passation, pas suppression ; un
  rejet n'exige pas de caisse ouverte (D14).
- ~~**Bon de livraison partiel**~~ **tranché le 21/09/2026 (D16)** : le
  suivi gère le partiel (stock, créance, statut, juste) ; le document
  reste à quantité pleine, assumé, pas dans le chemin critique.
- ~~`lire_fournisseurs_pagines` construit son `WHERE` par `format!()`~~
  **liés le 20/09/2026**, ainsi que les quatre autres listes du poste.
- ~~`ModalImpression` fait doublon avec `ApercuPiece` ; codes-barres non
  dessinés~~ **faits le 20/09/2026** ; pièces historiques restées en
  `validee`.
- ~~Un scénario instable (`gestion_base`, une fois sur trois)~~ **réglé
  le 17/09/2026** : l'horloge Windows tique par 15 ms, deux règlements
  du même tic ont le même horodatage. Départage `cree_le, id` dans les
  deux requêtes, et le scénario cherche chaque règlement par son
  montant, pas par sa position.
- Hors code : signature de l'installeur (D5), impression papier réelle,
  installeur en élevé, deux machines, restauration réelle — **TESTS-
  MANUELS.md non déroulé à la clôture de la v2 (21/09/2026)**, décision
  du propriétaire.

## v3 — en cours (branche `v3`, depuis `correctif/revue-v2`)

Une ligne par étape de [PLAN-V3.md](PLAN-V3.md) § 9, dans l'ordre.
Chaque écran est essayé dans un Chromium piloté (banc : le vrai
serveur sur une base jetable + l'écran Vite), captures à l'appui.

| étape | fait | preuve |
|---|---|---|
| A-1 | **Réglages Documents en base + écran** (23/09) : `coeur/documents.rs` (7 genres, défauts d'usine repris des paires de signatures v2, validation : 3 signatures, format, ticket sans signature ; un réglage abîmé retombe sur l'usine), `documents.rs` (dans `config_app` : `documents_reglages`, `documents_coordonnees`, images de signature en data URL ≤ 512 ko). Cinq commandes **nées sur `Base`** (D22). Paramètres → **Documents** : en-tête et pied téléversés (de retour de l'atelier), coordonnées cochées, par genre format / colonnes auto-toujours-jamais / montant en lettres / référence / mention / trois signatures avec cachet | `documents_base.rs` (10 scénarios, SQLite et PostgreSQL), 11 unitaires ; parcours écran `banc/a1.mjs` (13 vérifications : téléverser, régler, cachet, recharger, usine) |
| A-2 | **Le générateur unique lit les réglages** (23/09) : `lib/impression.ts` (l'habillage commun : en-tête image ou nom + coordonnées cochées, pied image ou mention, jusqu'à trois signatures avec cachet, montant en lettres), `genererPDF.ts` réécrit dessus (colonnes remise / TVA / référence, récapitulatif TVA par taux, sans TTC par taux pour ne pas contredire l'arrondi du total), reçu et relevés aussi (le côté fournisseur retourne « Le caissier » → « Le bénéficiaire »). `lire_donnees_piece` (deux versions) rend `article_reference` et `societe.site_web`. L'aperçu (`ApercuPiece`) s'ouvre au format réglé du genre et **perd le choix du modèle**. Paramètres → Documents montre un **exemple redessiné à chaque case**, avant d'enregistrer. Un genre jamais réglé **n'est pas enregistré** : il suit l'usine | rendu genre par genre dans la vraie page (`banc/a2.mjs`, 13 vérifications, captures A4 / A5 / ticket / reçu) ; aperçu par l'écran (`a2ui.mjs`, 7) ; exemple (`a2doc.mjs`, 3) ; `documents_base.rs` +2 |
| A-3 | **L'atelier part** (23/09, D17) : `pages/Modeles.tsx`, `lib/modeles/*` (2 330 l.), `EditeurPiedPage`, `noyau/src/modeles.rs`, les images « posées » (`images.rs`, façades Tauri), 12 commandes du serveur, la permission `modeles:gerer` (28 au catalogue), l'onglet et la route. Tables `modele_document` et `image_document` **supprimées par migration idempotente** (deux chemins). Au passage : le **reçu et les relevés perdaient leur titre** dès qu'un en-tête image était posé — l'image remplace le nom, plus le titre. Le banc d'écran entre au dépôt (`outils/banc/`) | `documents_base::les_tables_de_l_atelier_partent_a_l_amorcage` (deux moteurs) ; `schema_commun` vert ; workspace **490 tests** (Tauri compris) ; banc `a1…a3` : **42 vérifications** d'une base neuve |
| B-1 | **L'Historique** (23/09) : le journal métier se lit enfin. `coeur/historique.rs` (types en français, page bornée 1…200, date illisible refusée), `historique.rs` (`lire_historique_sur` paginé, « sur quoi » résolu à la lecture : tiers, pièce, article depuis `entite_type` ; recherche sur nom, numéro, article, auteur, détail ; `filtres_sur`). Deux commandes **nées sur `Base`**, permission **`journal:lire`** (la première de lecture, 29 au catalogue ; comptable sur base neuve, bases installées avec C-1). Écran **Historique** (menu, filtres Du/Au/Personne/Type/Recherche, avant → après, 50 par page) ouvert aussi depuis la fiche client, la fiche fournisseur, une pièce, un article (puce retirable, retour). Index `idx_journal_date`. Le banc a trouvé une course : la réponse lente d'un ancien filtre écrasait la bonne — seule la dernière s'affiche | `historique_base.rs` (5 scénarios, SQLite et PostgreSQL) ; route `l_historique_se_lit_avec_journal_lire_et_nomme_qui_a_vendu` ; banc `b1-historique.mjs` (**23 vérifications**, base neuve et relancée) |
| B-2 | **Les erreurs des caisses remontent au serveur** (23/09) : `window.onerror` et `unhandledrejection` → `POST /journal-poste` (jeton exigé, 4 Ko, 10 par minute et par poste, au-delà jeté et dit une fois) ; le serveur écrit `[POSTE ]` avec le nom du poste, l'écran ouvert et la pile. Un message venu de la fenêtre tient sur **une** ligne : un retour à la ligne aurait fabriqué une fausse ligne `[ERREUR]` du journal | 3 unitaires `journal_poste` ; route `les_erreurs_d_une_caisse_arrivent_au_journal_et_la_onzieme_est_jetee` (401, 413, dix acceptées, la onzième 429, aucune fausse ligne) ; banc `b2-journal-poste.mjs` (6) |
| B-3 | **Le journal technique se lit dans la console** (23/09) : `GET /journal` (200 dernières lignes, niveau ERREUR / REFUS / AVERT / POSTE / INFO / tout, lit aussi la copie tournée si besoin), permission `sauvegarde:lancer` ; carte **Journal** de la console du serveur, plus récentes en haut, lignes écrites en texte (une ligne piégée venue d'une caisse n'injecte rien) | unitaire `le_filtre_lit_le_niveau_a_sa_place` ; route `le_journal_technique_se_lit_depuis_la_console_avec_la_permission_de_sauvegarde` ; banc `b3-console-journal.mjs` (10) |
| B-4 | **Les anomalies sur le tableau de bord** (23/09) : compteur rouge « N anomalies à vérifier » (seulement s'il y en a, avec `journal:lire`) qui ouvre l'Historique filtré ; « Marquer vue » signé par la session (D26), la première personne reste ; table `anomalie_vue` à côté du journal (qui reste append-only), cloisonnée. **Chantier B terminé** | `historique_base::une_anomalie_vue_dit_par_qui_et_le_compteur_redescend` (SQLite et PostgreSQL) ; banc `b4-anomalies.mjs` (10) ; banc complet `a1…b4` : **91 vérifications** d'une base neuve |
| C-4 | **Sessions, et désactiver ferme les sessions** (23/09) : il n'existait **aucune commande pour désactiver un compte** ; `activer_utilisateur` (née sur `Base`) le fait et ferme ses sessions dans la même transaction, journal `utilisateur_desactive` ; refus : soi-même, superadmin, le dernier compte à accès total. `session_reseau.derniere_commande` posée avec `derniere_vue` (une écriture). Paramètres → Utilisateurs : Désactiver / Réactiver, **Sessions ouvertes** (qui, poste, depuis, dernière commande, Déconnecter) | `sessions_base.rs` (3 scénarios, SQLite et PostgreSQL) ; `reseau`, `postgres_amorcage`, `schema_commun` verts ; banc `c4-sessions.mjs` (17, deux navigateurs) |
| C-1 | **Cinq permissions de lecture** (23/09) : `achats:lire_prix`, `rapports:lire`, `tiers:lire_solde`, `journal:lire`, `caisse:lire_autres` (33 au catalogue). Une table dans `coeur/lecture.rs` (refus, masque à `null` à toute profondeur, paramètres neutralisés, « à moi », session vérifiée en base) appliquée par `api::rpc` à chaque appel ; `lire_articles_avec_unites` ne teste plus `role == "patron"`. Comptable : tout sauf `caisse:lire_autres` ; caissier, magasinier, employé : rien (D19) ; bases installées : le comptable reçoit les siennes **une fois** (y compris sur une base fichier, qui ne passe pas par `amorcer` — trouvé par le banc). Écrans : menu, accueil sans chiffres, onglets Créances / Pièces fournisseur / Retour fournisseur, valeur du stock, export CSV, écarts de caisse suivent `peut()`. Au passage : la sauvegarde auto n'est plus demandée par qui ne peut pas sauvegarder (un 403 à chaque connexion) | 7 unitaires `coeur::lecture` ; `lectures_base.rs` (5 scénarios, SQLite et PostgreSQL : le caissier reçoit `prix_achat: null`) ; route `le_caissier_recoit_prix_achat_null_et_pas_le_tableau_de_bord` ; banc `c1-lectures.mjs` (20) ; noyau + serveur **505 tests** verts (hors `installation`, Windows) ; banc complet d'une base neuve : **128** |
| C-3 | **Des plafonds, pas seulement des portes** (23/09) : `remise_max_pct`, `remboursement_max`, `credit_max` par rôle (colonnes, deux chemins de migration) et par personne (`utilisateur_plafond`) ; règle dans `coeur/plafonds.rs`, jugée **dans la poignée** quand l'argument suffit (vente : remise de chaque ligne et crédit laissé ; pièce : remises ; remboursement d'avoir) et **au point de sortie de l'argent** sinon (retour, règlement rendu). Refus clair : « Remise de 40 % — votre plafond est 15 %. Demander au patron. » ; le patron n'a pas de plafond. Écran : plafonds dans Paramètres → Rôles et dans les permissions d'une personne | 8 unitaires `coeur::plafonds` ; `plafonds_base.rs` (3 scénarios, SQLite et PostgreSQL : un refus n'écrit ni stock, ni caisse, ni paiement) ; route `le_plafond_du_caissier_se_juge_sur_l_argument_de_la_vente` ; banc `c3-plafonds.mjs` (12) ; noyau + serveur **517 tests** (hors `installation`, Windows) ; banc complet d'une base neuve `a1…c3` : **140** |
| D-1 | **Le déclencheur de stock multi-dossier** (23/09, ETAPES item 9) : sur SQLite, la ligne de stock d'un magasin de `dossier-b` naissait dans le dossier d'origine (la valeur par défaut de la colonne) — `dossier-b` ne voyait jamais son stock. Le déclencheur (`dossiers::DECLENCHEUR_STOCK_SQLITE`, `DROP` puis `CREATE` pour remplacer l'ancien corps) pose `NEW.dossier_id` ; PostgreSQL aussi, au lieu de se fier au défaut de session ; `REPARER_STOCK_DOSSIER` range à l'amorçage les lignes déjà mal placées (deux chemins) | `cloisonnement::un_mouvement_du_second_dossier_range_son_stock_dans_ce_dossier` (SQLite et PostgreSQL, réparation comprise) ; `stock_mouvements`, `livraison_stock`, `schema_commun` verts |
| D-2 | **Le serveur sert tout par `Base`** (23/09, D22) : `api::rpc` n'a plus qu'un chemin ; les **190 poignées `Connection`** de `socle.rs` sont parties (3 000 → 1 800 lignes, transformation scriptée : chaque `lecture`/`ecriture` + `aussi_sur_base` devient un `sur_base` avec sa permission et son drapeau) ; `Registre` réduit à `sur_base` ; `Serveur.conn` retiré (la connexion brute prépare le fichier au démarrage puis se ferme ; intégrité et `VACUUM INTO` par `Base::sqlite()`) ; le refus « plusieurs dossiers demandent PostgreSQL » à la connexion et au choix du dossier est levé (la création suit en D-3) | routes (9) sur SQLite, toutes par `Base` ; `registre_base` réécrit ; banc complet d'une base neuve **171 vérifications**, dont `d2-tous-les-ecrans.mjs` (31 écrans et onglets) ; journal du serveur : 0 `[ERREUR]` |
| D-3 | **Les dossiers, avec leurs dates de travail** (23/09, D21, D22) : `creer_dossier_sur(…, date_debut, date_fin)` — données ou l'année civile (`dates_de_travail`, pure), premier exercice, magasin, client de passage, journal `dossier_cree` ; **plus de refus sur SQLite**. Le refus de date dit la borne en lettres et quoi faire (« Le 5 avril 2027 est hors des dates de travail (jusqu'au 28 février 2027). Prolonger l'exercice ou en ouvrir un nouveau. »), aussi quand aucun exercice ne couvre la date. Paramètres → **Dossiers** : liste et création. Le banc choisit le dossier à la connexion sans le mémoriser | 3 unitaires (`en_lettres`, `dates_de_travail`) + messages ; `dossiers_base.rs` (7, SQLite et PostgreSQL) ; `cloisonnement.rs` (20) ; banc `d3-dossiers.mjs` (10) ; banc complet d'une base neuve **181** |
| D-4 | **Les exercices à l'écran, le garde-fou branché** (23/09, D21) : `api::rpc` juge **chaque écriture datée** avant de l'exécuter — `dossiers::date_d_ecriture(nom)` nomme la clé de date (`dateVente`, `datePiece`, `datePaiement`, `dateReception`) ou « aujourd'hui » pour ~35 écritures d'argent et de stock ; hors d'un exercice ouvert : 409 `Metier` avec le remède, rien d'écrit. Ni les réglages, ni l'ouverture/fermeture de caisse (on ferme toujours son tiroir). `ouvrir_exercice_sur` refuse un trou (« Un exercice commence le lendemain du précédent : le 1er janvier 2027 ») ; `prolonger_exercice_sur` refuse un exercice clos, une date qui ne va pas plus loin, une date dans l'exercice suivant ; `clore_exercice_sur` refuse le déjà-clos ; les trois au journal (`exercice_ouvert/prolonge/clos`). Paramètres → Dossiers → **Dates de travail** : Prolonger, Clore (en deux temps), Ouvrir l'exercice suivant (proposé du lendemain à un an moins un jour). La fenêtre monoposte n'est pas gardée (pas d'écran d'exercices) | 2 unitaires ; `dossiers_base.rs` (9, SQLite et PostgreSQL : prolonger, clore, le lendemain d'une clôture, pas de trou ni de chevauchement, l'exercice d'un autre dossier introuvable) ; route `une_ecriture_hors_des_dates_de_travail_est_refusee_avant_d_ecrire` ; banc `d4-exercices.mjs` (19) ; banc complet d'une base neuve **200** |
| D-5 | **La base d'avant devient un dossier à son nom et à ses dates** (23/09) : tout est déjà dans `defaut` ; au premier démarrage v3 (`amorcer` sur PostgreSQL, `serveur::main` sur un fichier SQLite), `dossiers::migrer_dossier_d_origine_sur` — une fois, marque `migration_v3_dossier_origine` — nomme le dossier comme la société de Paramètres → Société s'il porte encore « Ma boutique » (`nom_d_origine`, pure), et recule le début de son exercice unique au 1er janvier de la plus ancienne écriture (ventes, pièces, paiements, caisse, stock, retours, paiements fournisseurs — `debut_d_origine`, pure) : sans ça, un règlement tardif d'une vente de 2024 était refusé par le garde-fou de D-4. Plusieurs exercices ou un dossier déjà renommé : on n'y touche pas. Journal `dossier_d_origine`. Commande `renommer_dossier` (`dossiers:gerer`, le code ne change pas, journal `dossier_renomme`) ; Paramètres → Dossiers : invitation « Donnez-lui le nom de votre société » tant qu'il s'appelle « Ma boutique », crayon par dossier, la barre suit. Banc : `a2-apercu` renommé `a2b-apercu` (il dépend de `a2-rendu`, il échouait sur une base neuve) | 2 unitaires ; `dossier_d_origine.rs` (4, SQLite et PostgreSQL, dont le chemin **fichier** du serveur : `initialiser_tables` + `amorcer_si_vide`, écritures de 2025, premier démarrage, second démarrage sans effet) ; banc `d5-dossier-d-origine.mjs` (11) ; banc complet d'une base neuve **211** ; noyau 517 (hors `installation`), serveur 15 |
| D-6 | **Le compte PostgreSQL limité — le moteur cloisonne** (23/09, D10, PLAN-MULTISOCIETE décision 7) : `gescom-serveur --base <propriétaire> --compte-limite NOM` migre puis crée/remet d'aplomb un rôle `LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOBYPASSRLS` (mot de passe de `GESCOM_MDP_COMPTE_LIMITE` ou tiré au hasard, affiché une fois ; cité par `format(%L)`, jamais concaténé), DML seulement, et une **politique RLS** par table cloisonnée (`dossier_id = gescom_dossier()`, en lecture ET en écriture ; `exercice` se lit partout). Rejouable à chaque mise à jour. Sous ce compte : `amorcer` refuse en le disant et le serveur ne migre pas (`Base::peut_migrer`) ; `pg_dump` (sauvegarde, entretien) prend l'adresse du propriétaire dans `GESCOM_PG_SAUVEGARDE`, sinon refus (« sa copie serait partielle ») ; `creer_dossier` écrit les affaires du nouveau dossier chez lui (`Transaction::ecrire_dans`, `set_config` local). **`GESCOM_PG_LIMITE=1`** rejoue tous les scénarios `*_base` sous ce compte : il a trouvé la sauvegarde partielle, et des tests qui lisaient un autre dossier que la session | 4 unitaires ; `postgres_amorcage::le_compte_limite_ne_voit_et_n_ecrit_que_le_dossier_de_sa_session` (requête sans filtre → le moteur filtre ; écrire chez l'autre → « row-level security » ; DDL refusé ; créer un dossier passe) ; **213 scénarios `*_base` verts sous le compte limité**, 237 en propriétaire, 528 SQLite ; bout à bout : serveur lancé sous `gescom_app`, connexion, `creer_dossier`, sauvegarde refusée puis faite avec `GESCOM_PG_SAUVEGARDE` |
| E-1 | **Le plan comptable SYSCOHADA en base** (24/09, D23) : `compte_comptable` (clé `numero, dossier_id` ; `''` = commun), semé de **134 comptes** usuels d'un commerce, classes 1 à 7 (`coeur::plan_comptable::SYSCOHADA`, parent = plus long préfixe) ; sous-comptes par dossier (`ajouter_sous_compte`, 4 à 12 chiffres, rattachés au plan, journal `sous_compte_cree`). Permission `comptabilite:gerer` (34), donnée au comptable (rôle livré + migration unique `migration_v3_comptabilite`). `amorcage::migrations_de_donnees` regroupe les migrations de données v3 (lectures du comptable, comptabilité, plan) pour `amorcer` et le serveur sur fichier. Paramètres → **Comptabilité** : le plan par classe, recherche, nouveau sous-compte | 3 unitaires ; `plan_comptable_base.rs` (4, SQLite, PostgreSQL, compte limité : semé une fois et pas écrasé, sous-compte propre au dossier, base installée, détecteur) ; `schema_commun` ; banc `e1-plan-comptable.mjs` (10) |
| E-2 | **L'affectation comptable** (24/09, D23) : `coeur::affectations::OPERATIONS` — 28 opérations (trésorerie par mode de paiement : espèces 571, Orange/Moov Money 552, chèques 513, virements 521 ; ventes 701 / remises 7019 / TVA 4431 / clients 411 / retours / avoirs / irrécouvrables 6511 ; achats 601 / TVA 4452 / fournisseurs 401 / retours ; écarts de caisse 658 / 758 ; dépenses par catégorie de l'écran Caisse : loyer 622, transport 618, carburant 6053…), chacune avec son défaut et les débuts de compte qu'elle accepte. `affectation_comptable` (cloisonnée, RLS) ne garde que ce qui diffère, par dossier ; le défaut ne s'écrit pas. `definir_affectation` refuse un compte absent du plan du dossier ou qui ne convient pas (« il faut un compte qui commence par 70 »), journal `affectation_modifiee`. Paramètres → Comptabilité → **Affectations** : par groupe, une liste qui ne propose que les comptes permis, retour au défaut | 4 unitaires (chaque opération a un compte du plan qui lui convient) ; `affectations_base.rs` (3, SQLite, PostgreSQL, compte limité) ; banc `e2-affectations.mjs` (12) |
| E-3 | **Les journaux, lus, et leur export** (24/09, D23) : `journaux_comptables::ecritures_sur(du, au, journal)` fabrique à la lecture VT (vente : 411 TTC / 701 HT + 4431 TVA ; irrécouvrable : 6511 / 411 du reste dû), AC (achats du stock : 601 / 401, arrondis comme le cahier), RG (paiements clients : trésorerie du mode / 411, avoir utilisé : 701 / 411 ; paiements fournisseurs : 401 / trésorerie), CA (dépense par catégorie / trésorerie ; écart de clôture 658 ou 758) — **rien n'est stocké** ; un montant négatif (annulation) se contre-passe ; toute écriture passe `equilibrer`. `lire_journaux_comptables`, `exporter_journaux_csv` (`rapports:lire`) : CSV point-virgule, `JJ/MM/AAAA`, UTF-8 avec BOM. Rapports → **Journaux comptables**. Pas encore : retours de marchandise, OD, TVA sur achats | 4 unitaires ; `journaux_comptables_base.rs` (4, SQLite, PostgreSQL, compte limité : une journée — comptant, TVA 18 %, crédit réglé en Orange Money, irrécouvrable, achat, paiement fournisseur, loyer, taxi, manquant de 200 — chaque journal équilibré, **ventes du journal = CA TTC et HT du cahier du jour**, chaque opération sur son compte, l'affectation changée suit, CSV équilibré) ; banc `e3-journaux.mjs` (14) ; banc complet d'une base neuve **260** ; noyau 549 SQLite (hors `installation`), 248 PostgreSQL, 224 sous le compte limité ; serveur 16 |
| F-1 | **Gescom Équipe, la seconde fenêtre** (24/09, D28, D29) : `equipe.html` + `src/equipe/` (seconde entrée Vite), qui réutilise `pont.ts`, l'écran de connexion (titre en paramètre), le choix du dossier et la fenêtre de mot de passe ; menu selon les droits (`src/equipe/modules.ts`), accueil, modules à venir annoncés (« Arrive avec l'étape F-2 ») ; « rien pour l'instant, demander au patron » pour qui n'a aucun droit ici ; écran « Équipe a besoin du serveur Gescom » si le poste est en monoposte. Session à elle (`gescom_equipe_session`). Coque Tauri `src-tauri/equipe/` (`gescom-equipe`, `ml.gescom.equipe`) : une seule commande, `lire_config_reseau`, qui prend le serveur de la caisse du poste (`poste.json` de Gescom) ou le serveur local, et une empreinte à elle (`…-equipe`) ; `npm run equipe:dev` / `equipe:build`. `droits.ts` ne dépend plus de `App.tsx` (`poserDroits`). Catalogue **39** : `personnel:gerer`, `personnel:avancer`, `paie:preparer`, `paie:valider`, `crm:suivre` ; le comptable reçoit `paie:preparer` + `crm:suivre`, caissier et employé `crm:suivre` (migrations uniques `donner_au_role`, rejouées après l'amorçage d'une base neuve) | banc `f1-fenetre-equipe.mjs` (10 : patron, caissière avec mot de passe à changer, magasinier sans rien, déconnexion) ; coque compilée et lancée sous Xvfb (elle répond à la page : `equipe.json` écrit) ; banc complet d'une base neuve **270** ; noyau 549 SQLite |
| C-2 | **Les droits valent par dossier** (23/09, décision C2) : `utilisateur_dossier` ; `acces_dossiers::role_dans` (pure) — superadmin partout ; des lignes = exactement ces dossiers et ces rôles (le frère, patron global, n'entre que chez lui) ; aucune ligne = comme avant (accès total partout, sinon le dossier d'origine). `sessions::etat_sur` rend le rôle **du dossier** et révoque si le dossier est retiré ; la connexion ne liste que les dossiers ouverts (403 « Aucun dossier ne vous est ouvert » / « Ce dossier ne vous est pas ouvert ») ; `choisir_dossier` rend rôle et permissions, `PageLogin` les prend. **Précision** : qui a des lignes perd `utilisateurs:gerer`, `postes:gerer`, `sauvegarde:lancer`, `parametres:modifier` (communs à tous les dossiers — sinon il se créerait un compte qui voit tout) ; `renommer_dossier` exige d'entrer dans le dossier ; un dossier créé est donné à son créateur restreint. Commandes `lire/definir_dossiers_utilisateur` (`utilisateurs:gerer`), journal `droits_dossiers_modifies`. Paramètres → Utilisateurs → **Dossiers** (bouton avec plusieurs dossiers, patron compris) | 3 unitaires ; `acces_dossiers_base.rs` (4, SQLite et PostgreSQL : le frère et la comptable, session révoquée au retrait, on ne s'enferme pas dehors, le créateur entre) ; route `le_frere_n_ouvre_que_sa_quincaillerie_et_la_comptable_choisit` ; banc `c2-droits-dossiers.mjs` (13) ; banc complet d'une base neuve **224** ; noyau 524 (hors `installation`), serveur 16 |

## v3 — plan posé le 21/09/2026, le code n'a pas commencé

Quatre chantiers, dans cet ordre (D20) : **A** pièces commerciales
simplifiées (l'atelier part, D17 ; en-tête et pied téléversés ;
trois signatures avec image, D18 ; cases à cocher par genre) ; **B** le
journal jusqu'au bout (Historique lisible, erreurs des caisses
remontées, onglet Journal de la console, anomalies au tableau de
bord) ; **C** droits plus complets (cinq permissions de lecture, D19 ;
droits par dossier ; plafonds de remise / remboursement / crédit ;
sessions visibles et révocables) ; **D** plusieurs dossiers, ci-dessous
— dates de travail données à la création et prolongeables (D21),
SQLite reste et le multi-dossier demande le serveur, pas PostgreSQL
(D22), et **deux rôles dans un seul produit** : simple (monoposte,
déjà là) ou complet, choisis à l'installation, jamais devinés (D24),
le serveur gagnant une fenêtre — une coque sur sa console existante,
pas une reconstruction (D25) ; **E** le plan comptable SYSCOHADA en
base, opérations affectées, journaux lus et exportés, rien de stocké
(D23). Tout est tranché : → [PLAN-V3.md](PLAN-V3.md). Première
étape : A-1, les réglages Documents en base.

### D — la fondation multi-dossier, posée, dormante

Un dossier = **une société × un exercice**. Forme retenue : une colonne
`dossier_id` sur les 23 tables cloisonnées, `Base` porte son dossier, un
détecteur refuse **avant d'exécuter** toute requête qui oublie le
filtre. Tables `dossier` et `exercice`, règle des dates pure et testée.
Pas cloisonnés, et c'est voulu : `utilisateur`, `role`, `poste`,
`session_reseau`, `modele_document`, `article`, `unite_vente`,
`config_app`, `parametres_societe`.

Posé le 19/09/2026 (D13), **sans le lancer** : le choix du dossier à la
connexion, la création d'un dossier, les commandes d'exercices — tout
reste dormant tant qu'il n'y a qu'un dossier, et c'est le cas. C'est
le chantier **D** du plan v3, le dernier des quatre (D20). Ce qui
reste : l'écran de
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
