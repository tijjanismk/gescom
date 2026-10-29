# Alertes

Généré depuis [_genere/carte.json](_genere/carte.json) le 12/09/2026
(214 fichiers), puis vérifié à la main. **À lire avant de corriger quoi
que ce soit** : la moitié des pièges ci-dessous sont des copies qui
semblent vivantes.

> Le script ne voit que les imports statiques. Le pont `invoke("nom")`
> entre React et Rust est une chaîne : les listes croisent donc les deux
> sources (`grep` sur le nom de la commande dans `src/`).

---

## Lisibilité (D46, 29/10/2026)

- ~~Le « jaune sur jaune »~~ **corrigé partout** (43 encadrés, 25 textes,
  boutons) et gardé par `src/lib/lisibilite.test.ts`. **Reste** : les
  verts, rouges et bleus pas passés au crible (rouge 600 et vert 600
  frôlent 4,5:1 sur blanc) ; ni clavier ni zoom 200 % vérifiés.

## Reporté le 28/10/2026 (décision du propriétaire)

- **Deux dossiers n'ont pas chacun leur société.** `parametres_societe`
  est une ligne pour toute l'installation (37 lectures) : un second
  dossier imprime l'en-tête du premier. Le plan multi-société voulait
  que « l'en-tête change ». Avec : créer un nouveau dossier par le même
  assistant que la boutique (société, dates, réglages, accès), et la TVA
  des articles — communs aux dossiers — qu'un dossier sans TVA devrait
  ramener à 0 à la vente. « On laisse pour cette version » (D45).

## Trouvé en v3

- ~~**Le mode « caisse par utilisateur » casse tout encaissement.**~~
  **Retiré le 27/10 (lot C)** : `definir_mode_caisse` refuse `true`
  (`MODE_CAISSE_INDISPONIBLE`), une base qui porte `'1'` se comporte en
  un tiroir par dossier (D46). L'en-tête de `caisses.rs` dit comment le
  reprendre un jour (passer `auteur::courant()`, une session par
  personne).
- ~~`lire_lignes_piece` rend les prix d'achat~~ **corrigé le 27/10 (lot
  C)** : `coeur/lecture.rs` masque les totaux d'une pièce fournisseur
  sans `achats:lire_prix` ni `achats:creer`.

## Trouvé le 25-26/10/2026

Tous faits le 27/10 par les lots A à E (voir JOURNAL) :
~~colonne « suivre le stock » de l'import CSV~~ (lot D, une valeur qui
changerait le suivi d'un article existant refuse la ligne) ;
~~article non suivi muet dans les achats~~ (lot C, « stock non suivi ») ;
~~doublons de passants~~ (lot D, « Rattacher à un client », la fiche de
passage désactivée, `piece_commerciale.tiers_code` figé) ; ~~nom figé
côté fournisseur~~ (lot D) ; ~~la fenêtre ouvre `gescom.db`~~ (lot E :
plus de `setup`, les façades `Connection` refusent toutes au même
endroit, `REFUS_SANS_BASE`) ; ~~CSP de `tauri.conf.json`~~ (lot A, D39 :
la plage 40000–40100).

- **Le grand nettoyage des façades `Connection`** reste un chantier :
  182 appels `etat.conn.lock()` refusent désormais, mais le code est là.
- ~~**`postgres_amorcage::le_compte_limite_ne_voit…`** échoue sur ce poste~~
  **corrigé le 27/10** : le test accepte le refus en anglais comme en
  français (`lc_messages`) ; `postgres_amorcage` 22/22 sur `gescom_test`.
- **Banc** : sous Windows, `window.__TAURI_INTERNALS__.invoke` est
  verrouillé — les boîtes natives s'interceptent par les requêtes
  `ipc.localhost` (`pw.mjs::boitesMuettes`) ; lancer `lancer.ps1` avec
  sa sortie redirigée vers un fichier, jamais dans un tube.

## Trouvé le 27/10/2026 en relisant, pas encore fait

- ~~**Le 1er janvier 2027, plus aucune vente.**~~ **Fait le 27/10** :
  l'avis de fin d'exercice au tableau de bord (orange dans le mois, rouge
  les 7 derniers jours ; « prolonger » pour le patron, « prévenir le
  patron » pour les autres) et D38 (prolonger jusqu'à 5 ans, couper,
  clore seulement un exercice terminé). Reste à voir à l'écran.
- ~~**Une horloge qui recule.**~~ **Fait le 27/10 (lot C)** : plus de
  5 min de recul sur la dernière écriture → toute écriture refusée
  (`HORLOGE_RECULE`), lectures libres ; Paramètres → Horloge pour le
  seul cas d'une heure réglée en avance puis corrigée (`confirmer_horloge`,
  au journal). **Limite** : `changer_mot_de_passe` est une écriture,
  donc bloqué pendant l'alerte.
- **Smart App Control chez le client.** La fenêtre n'est pas signée et le
  serveur l'est par un certificat auto-signé : SmartScreen se contourne,
  Smart App Control (Windows 11 neuf) bloque sans recours sauf à le
  désactiver. Ce poste de développement en souffre déjà (4551).
- ~~**CSP confirmée**~~ **Corrigé le 27/10 (lot A, D39)** : `connect-src`
  énumère les ports 40000–40100, et rien d'autre (`tests/plage_de_ports.rs`).
- **Le serveur embarqué peut être vieux.** `src-tauri/binaires/` (ignoré
  par git) gardait un `gescom-serveur` du 20/09 ; chaque construction de
  la fenêtre le recopie dans `target/<profil>/gescom-serveur.exe`, sans
  erreur. **Toujours `outils/preparer_serveur.ps1` avant `tauri build`**,
  et construire `gescom` AVANT `gescom-serveur` en débogage (le banc le
  fait).
- **Duplicata, les limites assumées** : une impression annulée ou
  « enregistrée en PDF » consomme l'original (le bouton du patron
  rattrape) ; une facture POS retouchée (note, échéance) après sa
  première impression sort en duplicata avec un texte qui diffère de
  l'original.
- ~~**Le banc `monoposte -Neuf` efface la base d'essai à la main.**~~
  **Séparé le 27/10 (lot E)** : l'essai à la main a son dossier
  (`outils/banc/essai-main.ps1`, `.travail/essai-main/`, port 40050,
  `-Neuf` met de côté au lieu d'effacer) ; chaque dossier porte une
  marque `.proprietaire` et `lancer.ps1` refuse ce qui n'est pas à lui.
  **À faire à la main** : `.travail/monoposte` n'a pas de marque, donc
  le banc monoposte le refuse — vérifier la copie dans `essai-main`,
  puis le déplacer ou l'effacer.

## Trouvé le 27/10/2026 pendant les lots, pas encore fait

- **Rapport TVA** : la TVA des marchandises rendues reste collectée
  (`ligne_vente.montant_tva` des ventes de la période ;
  `chantiers::lire_resume_tva` pareil). Proposé : la déduire au prorata
  de `retour.quantite`, à la date du retour. Le cahier du jour
  (`journal.rs`) et le CA par magasin (`depots.rs`) ne sont pas nets des
  retours non plus.
- **Relevé client imprimé** (`lire_etat_creances_client`) : sa colonne
  « Payé » compte encore la dette éteinte par un retour. Dans l'onglet
  Règlements de la fiche client, une ligne `retour` garde « Reçu » et
  « Annuler ».
- **Une facture réglée en partie par avoir** imprime cette part comme
  due : `lire_donnees_piece` exclut les paiements `mode = 'avoir'` de
  `total_paye` (défaut antérieur, vu au lot B).
- `lire_etat_dette_fournisseur` compte une facture fournisseur encore en
  **brouillon** comme une dette.
- **Import CSV** : un en-tête présent mais incomplet retombe sur les
  positions v1.3 (`Nom;Categorie;Unite;Prix;Stock` lit le stock comme
  prix d'achat).
- Le reçu d'une vente **sans** pièce lit la fiche cliente d'aujourd'hui
  (pas de nom figé).
- ~~**Paie, questions de D41**~~ **tranchées et codées le 28/10 (D42)** :
  case « pris dans le tiroir », l'avance étalée et datée, la commission
  sur l'encaissé, l'ITS à la main. **Restent** : l'étalement compte par
  paie (un journalier à la quinzaine rembourse deux fois par mois) ; une
  vente annulée après une commission déjà versée ne se reprend pas (comme
  avant) ; les écrans d'Équipe jamais vus dans la vraie fenêtre. Le banc `f2-personnel` a changé
  (« Confirmer le retour ») ; les écrans d'Équipe n'ont pas été vus.
- **D40, limites assumées** : un service relancé trop souvent dans la
  journée attend 30 puis 60 s (la règle de relance de D12) ; le raccourci
  « Console du serveur » garde le port de l'installation (la fenêtre
  Gescom Serveur, elle, suit `serveur.json`) ; un `serveur.json` créé par
  un serveur lancé à la main fait sauter la page de configuration de
  l'installeur. Le test `un_serveur_rallume_chaque_matin…` a vu une fois
  un `…db-journal` pendant que `VACUUM INTO` écrivait (course du test,
  pas du code) — à rendre le test patient.
- `MANUEL.md` parle encore de 7300 ; `p6-role-caisse.mjs` lance un
  serveur sur 7301 (hors de la plage D39) ; le commentaire de `LOCALES`
  dans `pont.ts` parle encore d'une base locale.

## Trouvé le 28/10/2026 en écrivant le guide d'installation, pas encore fait

Relevé dans le code (non essayé en vrai) ; le guide et les manuels le
disent aux utilisateurs en attendant.

- **Désinstaller la caisse peut effacer la base d'un poste unique** : la
  case standard de Tauri « supprimer les données de l'application » fait
  `RmDir /r $APPDATA\ml.gescom.app` (base, sauvegardes, `poste.json`).
  Le libellé français affiché n'est pas vérifié. À retirer ou à garder.
- **Installer Gescom Serveur sur un poste unique qui a des données** :
  la fenêtre reprend le service sur 40000 (`reseau.rs`), donc la base
  neuve et vide de `ProgramData`. Aucun passage monoposte → service
  prévu ; le guide (chapitre 9) donne un `--restaurer … --base`, non essayé.
- **`--restaurer` sans `--base`** écrit dans la base de l'utilisateur qui
  tape la commande, pas dans celle du service (`main.rs:146`) — le
  manuel le disait ainsi, corrigé.
- **L'écran de connexion n'a pas de lien vers le réglage du réseau**, et
  Paramètres → Réseau demande d'être connecté : une caisse qui ne trouve
  plus son serveur (port changé, IP changée) est coincée sans technicien.
  La console de D40 conseille pourtant « Paramètres → Réseau → Rechercher
  le serveur ».
- **Gescom Équipe sur un poste sans caisse** ne peut pas recevoir
  d'adresse (elle lit le `poste.json` de la caisse, sinon 127.0.0.1:40000).
  L'écran « Gescom Équipe a besoin du serveur Gescom » (`AppEquipe.tsx`)
  ne s'affiche jamais (le mode vaut toujours « poste ») et son texte est
  périmé.
- `outils\parefeu.ps1` n'est pas livré, alors que des messages y renvoient ;
  le message de `--compte-limite` dit « dans poste.json » (c'est la clé
  `base` de `serveur.json`) ; `GESCOM_PG_SAUVEGARDE` lu par le service
  LocalSystem : non vérifié.
- `ListePassages.tsx:242` dit encore que les factures gardent « Client de
  passage » (le papier imprime le code depuis le 26/10).
- Droits : « passer en client ordinaire » demande `clients:modifier`, que
  l'employé a et le caissier non ; un employé peut décocher « Suivre le
  stock » à la création mais pas le changer ensuite (`parametres:modifier`).

## Ce que la revue du 23/09/2026 a corrigé (branche `correctif/revue-v2`)

Trois règles, qui valent pour tout ce qui s'écrit ensuite :

⚠️ **L'auteur vient de la session, jamais du rôle** (D26). Ne jamais
retrouver « qui » par `WHERE role = ? LIMIT 1` : passer par les aides
`argent::id_utilisateur_*` / `comptoir::auteur_courant`, qui lisent
d'abord `noyau::auteur::courant()`. **Une nouvelle route authentifiée
du serveur pose `auteur::poser(&appelant.utilisateur_id)`** avant
d'appeler le noyau, comme `/rpc`, `/sauvegarde` et `/entretien`.
L'auteur vit sur le FIL de la requête : ne pas déléguer une écriture à
un autre fil sans le reposer.

⚠️ **Le serveur juge la saisie** (D27). Une commande qui reçoit un
montant, un mode de paiement, une quantité, un prix ou une remise
appelle [coeur/saisie.rs](../src-tauri/noyau/src/coeur/saisie.rs) dans
ses deux versions — l'écran ne protège rien, la commande s'appelle
sans lui. `avoir` n'est pas un mode de paiement.

⚠️ **PostgreSQL : une erreur ignorée avorte TOUTE la transaction.**
Un `let _ = tx.executer(...)` ou un `.ok()` en échec rend les
instructions suivantes muettes et `COMMIT` répond `ROLLBACK` sans
erreur — la vente disparaissait pendant que la commande rendait `Ok`.
`Transaction::valider` vérifie maintenant que la transaction vit
encore et **refuse** sinon (« Rien n'a été enregistré… »). Conséquence :
sur PostgreSQL, une écriture « non bloquante » qui échoue fait échouer
le geste entier. Si elle doit vraiment être facultative, l'écrire
APRÈS `valider()`, hors transaction.

## Ce que la revue du 16/09/2026 a corrigé

Les cinq écarts R1…R5 sont fermés (détail dans [ETAPES.md](ETAPES.md)
§ Revue du 16/09/2026). Deux règles en sont sorties, qui valent pour la
suite : **ce qui écrit plusieurs lignes ouvre une transaction, même dans
un outil d'entretien**, et **une copie de sécurité se prend avant ce
qu'elle protège, pas après**.

⚠️ **Deux dates, deux faits** (17/09/2026) : la date d'une vente, d'une
pièce ou d'un règlement se saisit — le mouvement de caisse, jamais.
L'argent est entré dans le tiroir quand il y est entré, et la caisse se
lit par session. La règle est dans `coeur/dates.rs`, le droit
d'antidater est la permission `pieces:antidater`, vérifiée par le
serveur. Antidater une vente en espèces est la façon de masquer un trou
dans le tiroir : ne pas « simplifier » ça.

⚠️ **L'horloge Windows tique par 15 ms** : deux écritures du même tic
portent le même `maintenant_iso()`. Un `ORDER BY` sur une date seule
n'a pas d'ordre entre elles — départager par `cree_le, id`, et un test
ne lit jamais une ligne par sa position.

⚠️ **Un helper du serveur qui prend le verrou de la base ne s'appelle
pas verrou en main** : `sauvegarde::dossier` relit son réglage par
`Base` et reprend le mutex — c'est l'interblocage du 13/09, corrigé en
calculant le dossier avant de verrouiller. Le mutex n'est pas réentrant.

## Deux fichiers du même nom : lequel ouvrir

`src-tauri/src/commandes/<module>.rs` et `src-tauri/noyau/src/<module>.rs`
existent en parallèle pour 29 modules. **Ce n'est pas un doublon** :

- `noyau/src/<module>.rs` — **la logique**. C'est là qu'on lit et qu'on
  modifie. Chaque commande y a deux versions : `fn(conn: &Connection)`
  (SQLite, chemin de la fenêtre) et `fn_sur_base(base: &mut Base)` (les
  deux moteurs, chemin du serveur). **Une correction se fait dans les
  deux**, ou la fenêtre et le serveur divergent.
- `src-tauri/src/commandes/<module>.rs` — une façade `#[tauri::command]`
  de 200 lignes qui délègue au noyau. Ne contient aucune règle.

Le script signale 180 « symboles dupliqués » entre les deux arbres :
c'est ce motif, voulu, et rien d'autre.

## Fichiers orphelins — vérifiés

| Fichier | Verdict |
|---|---|
| [src/components/SelecteurProfil.tsx](../src/components/SelecteurProfil.tsx) | **Mort.** Remplacé par `PageLogin.tsx`. |
| [src/components/ui/table.tsx](../src/components/ui/table.tsx) | **Mort.** Primitive jamais adoptée, 13 fichiers écrivent leur `<table>`. |
| [src/lib/session.ts](../src/lib/session.ts) | **Mort et dupliqué.** La règle des 8 h vit dans `App.tsx`. |
| [src/lib/tauri.ts](../src/lib/tauri.ts) | **Mort.** Ancienne façade typée sur `invoke`. |
| [src/lib/remise.ts](../src/lib/remise.ts) | **Mort.** Le calcul de remise est en Rust. |
| `outils/generer_socle.py` | **Vivant, hors application.** Régénère le bloc de `serveur/src/socle.rs` entre ses marqueurs. |
| `creer_tiers_test.py`, `generer_catalogue.py`, `nettoyer_imports.py`, `preparer_demo_video.py`, `t_regles.py` | **Vivants, hors application.** Outillage lancé à la main ; `t_regles.py` rejoue les règles du cœur en Python, ce n'est pas un doublon. |

`portes.rs` n'est **plus** orphelin : déclaré dans `lib.rs`, il porte le
catalogue des permissions et `permissions_de_sur` (les deux moteurs).

## Commandes enregistrées, jamais appelées par l'écran

15 sur 193. Déclarées côté Rust, servies par le serveur, aucun `invoke`
dans `src/`. Ne rien supprimer : plusieurs sont des demi-chantiers
volontairement laissés branchés.

```
pieces::annuler_facture_par_avoir     pieces::changer_statut_piece
pieces_pos::modifier_facture_pos      pieces_pos::valider_facture_credit
avoirs::appliquer_avoir_vente         creances::solder_residus_creances
comptoir::lire_clients_avec_creances  chantiers::lire_factures_fournisseur_ouvertes
caisse::lire_depenses_du_jour         caisse::modifier_depense
depots::lire_stock_depot              depots::lire_stock_article_depots
parametres::lire_stocks
pagination::lire_ventes_paginees      pagination::lire_ventes_recentes_paginee
```

Deux qui devaient l'être, faits le 27/10 :
- ~~les deux `pagination::lire_ventes_*`~~ — `lire_ventes_recentes_paginee`
  sert l'écran Retours et la fenêtre de retour de Pièces (lot D) ;
  `lire_ventes_recentes` n'est plus appelée ;
- ~~`creances::solder_residus_creances`~~ — D41 se solde maintenant dans
  la transaction du règlement qui crée le résidu (`creances::solder_residu_sur`,
  lot B) ; la commande reste pour rattraper une base ancienne.

## Fichiers les plus sollicités

Modifier une signature ici casse loin.

| fichier | importé par |
|---|---|
| [noyau/src/coeur/pieces.rs](../src-tauri/noyau/src/coeur/pieces.rs) | 77 |
| [noyau/src/pieces.rs](../src-tauri/noyau/src/pieces.rs) | 74 |
| [src/lib/pont.ts](../src/lib/pont.ts) | 48 — **le** pont vers le serveur |
| [noyau/src/coeur/caisse.rs](../src-tauri/noyau/src/coeur/caisse.rs) | 39 |
| [noyau/src/fournisseurs.rs](../src-tauri/noyau/src/fournisseurs.rs), [chantiers.rs](../src-tauri/noyau/src/chantiers.rs) | 39 |

## Fichiers volumineux

| fichier | lignes | pourquoi |
|---|---|---|
| [noyau/src/pieces.rs](../src-tauri/noyau/src/pieces.rs) | 3 579 | le cycle documentaire, en deux versions |
| [serveur/src/socle.rs](../src-tauri/serveur/src/socle.rs) | 3 100 | 210 commandes × 2 poignées ; le bloc généré est entre marqueurs |
| [noyau/src/argent.rs](../src-tauri/noyau/src/argent.rs) | 2 420 | vente, facture, règlement — **ouvrir avant de toucher à l'argent** |
| [noyau/src/achats.rs](../src-tauri/noyau/src/achats.rs) | 2 030 | |
| [src/pages/Pieces.tsx](../src/pages/Pieces.tsx), [Ventes.tsx](../src/pages/Ventes.tsx) | 1 700 | |

Les modules du noyau ont doublé de taille avec le portage : la moitié
« `Connection` » deviendra du code mort le jour où la fenêtre passera
elle aussi par le serveur (D11). C'est le prochain grand nettoyage, pas
un bug.

## Régénérer

```bash
python <chemin>/carte.py . --json AI_CONTEXT/_genere/carte.json
```

Sous Git Bash, ne pas passer `--alias '@/=src/'` : l'alias par défaut est
le bon, et le shell mange les guillemets. Puis relire ce fichier :
c'est lui qu'on charge, pas le JSON.
