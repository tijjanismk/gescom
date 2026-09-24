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
| La caisse : ouverture, dépenses par catégorie (`salaire` existe déjà) | les avances et les salaires sortent **de la caisse**, comme aujourd'hui, mais rattachés à la personne |
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
  Money, virement) : chaque versement est une **sortie de caisse** —
  caisse ouverte exigée (règle 4 de CLAUDE.md), catégorie `salaire`.
- **Le bulletin** s'imprime par le générateur de pièces (A-2), avec en-
  tête, pied et signatures (« L'employé », « Pour la société ») ; un
  employé sans compte le signe sur papier.

### D32 — Les avances d'abord

Le trou le plus courant n'est pas le calcul du salaire : c'est
**l'avance** donnée au milieu du mois, sortie de la caisse sur un bout
de papier, et oubliée à la paie. Une avance est une sortie de caisse
**rattachée à la personne** ; elle se retient d'office sur sa
prochaine fiche (reportable si elle dépasse le net). Le plafond d'une
avance est un réglage (par personne ou par rôle, comme C-3).

### D33 — Les cotisations : facultatives, jamais devinées

Pas de barème en dur. Pour les personnes **déclarées**, les
cotisations (INPS part salariale et patronale, AMO, ITS…) sont des
**lignes de réglage** du dossier : un libellé, une base (brut, ou
plafonné), un taux ou un barème, qui les paie (salarié / employeur),
son compte. **Vides par défaut** : tant que le comptable ne les a pas
remplies, aucune n'est retenue — un taux faux sur un bulletin est pire
qu'aucun. Un modèle « Mali » pourra être proposé, sourcé et daté, à
valider par le comptable avant d'être appliqué ; il n'est jamais actif
d'office.

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
| `personnel:avancer` | donner une avance (sortie de caisse) | — (à donner) |
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
| **G-1** ✓ | Les avances (D32) : sortie de caisse rattachée, plafond | scénario : caisse fermée refusée, plafond, l'avance apparaît dans la caisse et sur la personne |
| **G-2** ✓ | La fiche de paie : calcul, brouillon, validation, numéro, rectificative (D31) | scénario : les trois personnes de F-2 ; la commission égale les ventes signées ; l'avance retenue ; validée, plus rien ne change |
| **G-3** | Payer et imprimer : versements depuis la caisse, bulletin par le générateur | scénario : deux versements, reste dû ; banc : le bulletin |
| **G-4** | Cotisations facultatives (D33) et journal PA | scénario : déclaré / non déclaré ; PA équilibré |
| **H-1** | Échanges et fiche 360 (D34) | scénario : un appel, une relance existante, la créance, sur la fiche |
| **H-2** | Rappels et prospects | scénario : un rappel attribué apparaît à la bonne personne ; un prospect devient client à sa première vente |

---

## 4. Ce qui n'est PAS dans Équipe (pour l'instant)

Pointeuse, congés payés calculés, heures supplémentaires légales,
déclarations INPS/impôts générées, prêts au personnel sur plusieurs
mois (au-delà d'une avance reportée), campagnes SMS du CRM, pipeline de
ventes à étapes. Rien de ce qui précède ne l'empêche d'arriver après.
