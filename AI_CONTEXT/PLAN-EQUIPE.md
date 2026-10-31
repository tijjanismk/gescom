# Gescom Équipe — le personnel, la paie, le suivi des clients

> « J'aimerais créer une autre app à côté pour le CRM, la gestion de
> personnel et de paie, qui peuvent communiquer avec la même base. »
> Puis, sur la paie : « La plupart de ces commerces n'ont même pas de
> contrat, il faut qu'on soit flexible. » — le propriétaire, 24/09/2026.

Réponses du propriétaire le 24/09/2026 : **par le serveur Gescom**,
**personnel, puis paie, puis CRM**, **même dépôt**, **paie souple**.
Ce document pose les décisions D28 à D34 et l'ordre des chantiers F,
G, H. Il se lit après [PLAN-V3.md](PLAN-V3.md) : Équipe s'appuie sur
tout ce que la v3 a posé (dossiers, droits par dossier, historique,
compte PostgreSQL limité, plan comptable et journaux).

---

## 1. Ce qui existe déjà et qu'Équipe réutilise

| Existant | Ce qu'Équipe en fait |
|---|---|
| Le serveur, `/rpc`, le registre, `api::rpc` (permissions, lectures, dates de travail) | Équipe n'a **aucune base à elle** : elle appelle les mêmes commandes, plus les siennes |
| Comptes, rôles, sessions, **droits par dossier** (C-2) | la même connexion ; un employé peut avoir un compte, pas l'inverse |
| Dossiers et exercices (D) | un employé appartient à un dossier (une société) ; une fiche de paie tombe dans les dates de travail |
| La caisse : ouverture, dépenses par catégorie (`salaire` existe déjà) | **plus rien** depuis la révision du 24/09 (D31, D32) : la paie est indépendante de la caisse ; avant : comme aujourd'hui, mais rattachés à la personne |
| Ventes signées par l'utilisateur de la session (D26) | la commission sur les ventes d'un vendeur se calcule sans rien saisir |
| Les clients, ventes, créances, relances | le suivi client (CRM) **n'a pas de seconde fiche client** |
| Plan comptable, affectations, journaux lus (E) | un journal de paie de plus, lu comme les autres |
| `src/lib/pont.ts`, les composants `src/components/ui`, le banc | la fenêtre Équipe les importe tels quels |

---

## 2. Les décisions

### D28 — Une seconde fenêtre, qui ne parle qu'au serveur

**Gescom Équipe** (nom provisoire) est une application à part : son
icône, sa fenêtre, son menu. Elle n'ouvre **aucune base** : elle se
connecte au serveur Gescom, avec les mêmes comptes et les mêmes droits,
dans le dossier choisi à la connexion. Toutes les règles (auteur du
geste, saisie jugée par le serveur, cloisonnement, dates de travail,
compte PostgreSQL limité) restent à **un seul endroit**.

Conséquence assumée : un commerçant en **monoposte** (SQLite sans
serveur) qui veut Équipe lance le serveur sur sa machine — D22 le
permet déjà, sur le même fichier SQLite. Pas de « mode local » pour
Équipe : il y aurait deux programmes qui écrivent la même base sans
arbitre.

### D29 — Même dépôt, une entrée de plus

- **Écran** : `equipe.html` (seconde entrée Vite) et `src/equipe/` ;
  elle importe `src/lib/pont.ts` et `src/components/ui/*` sans copie.
- **Coque** : `src-tauri/equipe/`, une coque Tauri fine (comme D25) qui
  lit la même adresse de serveur (`poste.json`) que la caisse du poste.
- **Logique** : `noyau/src/personnel.rs`, `paie.rs`, `crm.rs`, règles
  pures dans `noyau/src/coeur/` ; commandes enregistrées dans le
  serveur (`socle.rs`).
- **Une exception écrite à la règle 2 de CLAUDE.md** : ces modules
  n'ont **que la version `Base`**. La fenêtre monoposte ne les sert pas
  (D28) ; écrire une version `Connection` que rien n'appelle serait du
  code mort à maintenir.

### D30 — Une personne, pas un contrat

La plupart des boutiques n'ont ni contrat écrit ni déclaration. Une
fiche exige **trois choses** : le nom, ce que la personne fait, comment
elle est payée. Tout le reste est facultatif et se complète quand on
l'a : téléphone, date d'entrée, pièce d'identité, contrat écrit
(oui/non, date), numéro INPS, compte utilisateur lié, magasin.

Comment elle est payée — **un mode, ou plusieurs qui s'ajoutent** :

| mode | se règle par | se calcule par |
|---|---|---|
| au mois | un montant | le montant (au prorata des jours si on le veut) |
| à la journée | un tarif du jour | jours travaillés × tarif |
| à la commission | un pourcentage | % des ventes **signées** par son compte (D26) sur la période |
| à la tâche | rien d'avance | des lignes saisies sur la fiche (« 3 livraisons × 1 000 ») |
| rien de fixe | — | des primes saisies |

Un départ ne supprime rien : la fiche passe « partie », son historique
reste. **Déclaré ou non** est un drapeau de la fiche (§ D33).

### D31 — La fiche de paie, un document qu'on remet

Contrairement aux journaux (D23), la fiche de paie **se stocke** : c'est
un papier qu'on remet à quelqu'un, et ce qu'il a touché ne doit pas
changer si on corrige un tarif le mois suivant — comme une vente.

- **Période** libre (le mois par défaut ; la semaine ou la quinzaine
  pour les journaliers).
- **Calculée** depuis la fiche : base, jours, commission, lignes à la
  tâche, primes ; **moins** les avances de la période et les retenues
  saisies (casse, absence) ; **moins** les cotisations s'il est
  déclaré (§ D33). Chaque ligne dit d'où elle vient.
- **Brouillon → validée → payée.** Le brouillon se recalcule et se
  corrige ; validée, elle est **figée et numérotée** (`PAIE-2026-00001`,
  dans le dossier) ; une erreur se corrige par une **fiche
  rectificative**, jamais en réécrivant. Validée par qui a `paie:valider`.
- **Payée** en un ou plusieurs versements (espèces, Orange Money, Moov
  Money, virement). **Révisé le 24/09 par le propriétaire** : « la paie
  doit être indépendante de la caisse mais visible dans les chiffres ».
  Un versement ne touche **pas** la caisse du jour (ni session exigée,
  ni mouvement) ; le moyen dit d'où vient l'argent. Le coût des
  salaires se lit dans Rapports → CA mensuel, à côté du chiffre
  d'affaires.
- **Le bulletin** s'imprime par le générateur de pièces (A-2), avec en-
  tête, pied et signatures (« L'employé », « Pour la société ») ; un
  employé sans compte le signe sur papier.

### D32 — Les avances d'abord

Le trou le plus courant n'est pas le calcul du salaire : c'est
**l'avance** donnée au milieu du mois sur un bout de papier, et
oubliée à la paie. Une avance est **rattachée à la personne** (hors
caisse depuis la révision du 24/09, comme les versements — D31) ; elle se retient d'office sur sa
prochaine fiche (reportable si elle dépasse le net). Le plafond d'une
avance est un réglage (par personne ou par rôle, comme C-3).

### D33 — Les cotisations : facultatives, jamais devinées

Pas de barème en dur. Pour les personnes **déclarées**, les
cotisations (INPS part salariale et patronale, AMO…) sont des
**lignes de réglage** du dossier : un libellé, une base (brut, ou
plafonné), un taux, qui les paie (salarié / employeur), son compte.
**Vides par défaut** : tant que le comptable ne les a pas remplies,
aucune n'est retenue — un taux faux sur un bulletin est pire qu'aucun.
Un modèle « Mali » pourra être proposé, sourcé et daté, à valider par
le comptable avant d'être appliqué ; il n'est jamais actif d'office.
**Corrigé le 28/10/2026 (D42)** : il n'y a pas de barème. L'ITS, qui va
par tranches et selon la famille, se saisit à la main sur la fiche, en
retenue, par le comptable.

### D34 — Le suivi client s'appuie sur la fiche client de Gescom

Pas de second fichier clients. Le CRM ajoute autour du client existant :
- les **échanges** : appel, visite, message WhatsApp, note — qui, quand,
  quoi, et la suite prévue ;
- les **rappels** : « rappeler Awa jeudi », attribués à quelqu'un, qui
  apparaissent à la personne concernée ;
- les **prospects** : un client sans vente encore, avec son origine ;
- la **fiche 360** : ventes, créance, relances (déjà en base), échanges
  et rappels, sur un écran.
Les relances de créance existantes (`relance_creance`) sont des échanges
comme les autres, pas une liste de plus.

### Droits (s'ajoutent au catalogue, 34 → 39)

| permission | pour | livré à |
|---|---|---|
| `personnel:gerer` | créer, modifier, faire partir une fiche | patron (accès total) |
| `personnel:avancer` | donner une avance | — (à donner) |
| `paie:preparer` | calculer et corriger les brouillons | comptable |
| `paie:valider` | valider, payer, faire une rectificative | patron |
| `crm:suivre` | échanges, rappels, prospects | caissier, employé, comptable |

**Lecture** (comme C-1) : ce que gagne une personne ne se lit qu'avec
`paie:preparer` ou `paie:valider` ; les autres voient la fiche sans
montant. Qui n'a que certains dossiers (C-2) ne voit que leur personnel.

### Comptabilité (E)

Affectations ajoutées : salaires 661, rémunérations dues 422, avances
421, cotisations salariales 431 (et patronales 664/431). Un journal
**PA** (paie) lu comme les quatre autres : fiche validée 661 / 422 (et
cotisations) ; versement 422 / trésorerie ; avance 421 / trésorerie,
retenue 422 / 421.

---

## 3. L'ordre des chantiers

Chaque étape : un commit, un scénario sur SQLite, PostgreSQL et le
compte limité, une ligne dans ETAPES, un parcours du banc.

| étape | contenu | preuve |
|---|---|---|
| **F-1** ✓ | La fenêtre Équipe : `equipe.html`, connexion au serveur (même écran de connexion, même choix de dossier), menu selon les droits, coque Tauri | banc : connexion, dossier, menu d'un patron / d'un caissier |
| **F-2** ✓ | Les fiches du personnel (D30) : liste, fiche, modes de paiement, départ ; lien facultatif avec un compte | scénario : trois fiches — mensuel sans contrat, journalier, vendeur à la commission lié à un compte ; cloisonnement par dossier ; historique |
| **F-3** ✓ | Les jours travaillés : une grille du mois, présent / absent / demi-journée | scénario : un journalier, 22 jours, la paie en lira le compte |
| **G-1** ✓ | Les avances (D32) : rattachées à la personne, plafond (hors caisse depuis le 24/09) | scénario : caisse fermée refusée, plafond, l'avance apparaît dans la caisse et sur la personne |
| **G-2** ✓ | La fiche de paie : calcul, brouillon, validation, numéro, rectificative (D31) | scénario : les trois personnes de F-2 ; la commission égale les ventes signées ; l'avance retenue ; validée, plus rien ne change |
| **G-3** ✓ | Payer et imprimer : versements (hors caisse depuis le 24/09), bulletin par le générateur | scénario : deux versements, reste dû ; banc : le bulletin |
| **G-4** ✓ | Cotisations facultatives (D33) et journal PA | scénario : déclaré / non déclaré ; PA équilibré |
| **H-1** ✓ | Échanges et fiche 360 (D34) | scénario : un appel, une relance existante, la créance, sur la fiche |
| **H-2** ✓ | Rappels et prospects | scénario : un rappel attribué apparaît à la bonne personne ; un prospect devient client à sa première vente |

---

## 4. Ce qui n'est PAS dans Équipe (pour l'instant)

Pointeuse, congés payés calculés, heures supplémentaires légales,
déclarations INPS/impôts générées, campagnes SMS du CRM, pipeline de
ventes à étapes. Rien de ce qui précède ne l'empêche d'arriver après.
(Les prêts sur plusieurs mois sont sortis de cette liste le 29/10/2026 :
EQ-1 ci-dessous.)

---

## 5. Compléter Équipe : EQ-1 à EQ-6 (29/10/2026)

> « Qu'est-ce qu'on peut ajouter à Équipe pour la compléter ? », puis
> « écris le plan en ces étapes et chaque étape ouvre sa conversation »
> — le propriétaire, 29/10/2026.

Six étapes **indépendantes**, une session chacune, chacune dans son
worktree tiré de `claude/gallant-neumann-9efb9f`. Aucune n'attend le
résultat d'une autre ; ce qui relierait deux étapes (la tuile des prêts
sur l'accueil, le bouton WhatsApp sur la liste à relancer) est noté
« après fusion » et attend que les deux soient réunies.

| étape | contenu | décision réservée | fichiers à elle | preuve |
|---|---|---|---|---|
| **EQ-1** | **Le prêt au personnel sur plusieurs mois** : 60 000 F prêtés pour la Tabaski ou la rentrée, retenus 10 000 F par mois. Échéance fixe ; la retenue ne dépasse jamais le net du mois, le reste se reporte ; soldé, il se ferme ; hors caisse comme l'avance (D31, D32) ; journal PA 421 / trésorerie | **D47** | `noyau/src/prets.rs`, `coeur/pret.rs`, `src/equipe/Prets.tsx`, `tests/prets_base.rs` ; la retenue entre dans le calcul de `paie.rs` | scénario : un prêt sur six fiches, une fiche trop maigre (report), la rectificative, PA équilibré |
| **EQ-2** | **L'état de paie du mois** : une ligne par personne — brut, avances, retenues, cotisations, net, déjà versé, reste dû — et les totaux ; imprimable, exportable en CSV ; lecture seule | **D48** | `noyau/src/etat_paie.rs`, `src/equipe/EtatPaie.tsx`, `tests/etat_paie_base.rs` | scénario : trois personnes, deux versements partiels ; les totaux égalent la somme des fiches et `salaires_par_mois_sur` |
| **EQ-3** | **L'accueil du jour** : en ouvrant Équipe, ce qui attend la personne connectée — ses rappels dus, les fiches en brouillon à valider, les fiches validées pas soldées, les avances en cours — chaque ligne mène à l'écran ; selon ses droits (C-1) | **D49** | `noyau/src/equipe_jour.rs`, `src/equipe/Accueil.tsx`, `tests/equipe_jour_base.rs` | scénario : un caissier ne voit aucun montant, le patron voit tout ; un rappel fait disparaît |
| **EQ-4** | **Les clients à relancer** : une liste tirée de la base, sans saisie — sans achat depuis N jours (60 d'office, réglage du dossier), créance en retard, meilleurs clients qui ralentissent ; un clic crée l'échange ou le rappel (D34) | **D50** | `noyau/src/crm_relances.rs`, règle pure dans `coeur/`, `src/equipe/ClientsARelancer.tsx`, `tests/crm_relances_base.rs` | scénario : un client dormant, un en retard, un actif absent de la liste ; cloisonnement par dossier |
| **EQ-5** | **WhatsApp sans service payant** : un bouton ouvre WhatsApp (`wa.me`) avec le texte prêt — le bulletin résumé (période, net, versé, reste) pour l'employé, un message pour le client depuis la fiche 360 ; c'est la personne qui appuie sur « Envoyer » ; l'échange `whatsapp` se note | **D51** | `src/lib/whatsapp.ts` (numéro malien en +223, texte encodé) et son test `node --test`, boutons dans `ApercuBulletin.tsx` et la fiche 360 | test du numéro (76 11 22 33, 0022376…, +223…) ; banc : le lien ouvert porte le bon numéro et le bon texte |
| **EQ-6** | **Les absences notées, sans calcul légal** : le motif d'un jour non travaillé (maladie, permission, congé, absence) dans la grille des jours (F-3), et le compte de l'année par motif sur la fiche ; aucune paie de congé calculée (§ 4) | **D52** | la grille des jours (`JoursTravailles.tsx`) et son module du noyau, `tests/absences_base.rs` | scénario : un mois avec trois motifs ; la paie à la journée ne compte que les jours travaillés, comme avant |

### Règles communes aux six sessions

- **Décisions écrites avant le code**, avec leur pourquoi, et l'accord
  du propriétaire avant d'écrire (méthode habituelle) ; la décision prend
  le numéro réservé ci-dessus, aucun autre.
- Règles de `CLAUDE.md` : logique dans `noyau`, SQL portable, le serveur
  juge la saisie (D27), qui agit vient de la session (D26), version
  `Base` seulement (exception D29), colonnes dans `schema.sql` **et**
  `amorcage.rs`.
- **Fichiers partagés** (`serveur/src/socle.rs`, `schema.sql`,
  `amorcage.rs`, `equipe/modules.ts`, `AppEquipe.tsx`, `Paie.tsx`,
  `ETAPES.md`, `JOURNAL.md`, `DECISIONS.md`, `modules/equipe.md`) : y
  **ajouter** ses lignes, ne rien y réordonner ni reformater — les six
  branches se fusionnent ensuite.
- **Pas de retouche de mise en page** : la session « Onglets de la
  paie » a un travail non commité sur les mêmes écrans
  (`claude/xenodochial-hodgkin-4f567b`) ; il sera fusionné à part.
- **PostgreSQL : une base par session**, déjà créée : `gescom_test_eq1`
  … `gescom_test_eq6` (`postgresql://postgres:user@127.0.0.1:5432/gescom_test_eqN`).
  Jamais `gescom`, jamais `gescom_test` (autres sessions). Le compte
  limité est commun à toutes les bases : s'il échoue sur le rôle,
  rejouer la série seule.
- **Ports d'essai** : serveur `4007N`, écran Vite `144N` (N = numéro de
  l'étape). Ne jamais toucher 40000 (le propriétaire), 40050 (l'essai à
  la main), 40060-40062, 1420, 1431.
- Fini quand : tests SQLite, PostgreSQL et compte limité verts, `npx tsc
  --noEmit ; node --test src/lib/*.test.ts` vert, l'écran vu par le
  propriétaire, ETAPES / JOURNAL / DECISIONS / `modules/equipe.md` à
  jour. **Commit seulement quand le propriétaire le demande ; jamais de
  push.**

### Après fusion

La tuile « prêts en cours » sur l'accueil (EQ-1 + EQ-3) ; le bouton
WhatsApp sur la liste à relancer (EQ-4 + EQ-5) ; la colonne « prêt »
dans l'état de paie (EQ-1 + EQ-2).
