# Gescom Équipe — Manuel d'utilisation

*Version 0.1 — 28 octobre 2026 — le personnel, la paie, le suivi des clients.*

---

# 1. Ce qu'est Gescom Équipe

Une seconde fenêtre, à côté de la caisse, pour ce qui ne se fait pas au
comptoir :

- **Personnel** — qui travaille ici, comment on le paie, les jours
  travaillés ;
- **Paie** — les avances, les fiches de paie du mois, les versements,
  les bulletins, les cotisations ;
- **Suivi clients** — les échanges, les rappels, les prospects.

Équipe n'a **pas de base à elle** : elle parle au même serveur que la
caisse, avec **les mêmes comptes et les mêmes droits**, dans le dossier
choisi à la connexion. Elle s'installe après la caisse du même poste
(Guide d'installation, chapitre 7).

**Les montants se tapent comme on les dit** : « 15.000 », « 15 000 » et
« 15,000 » valent tous **15 000 F**. Un pourcentage ou une quantité
garde sa virgule (2,5 %).

---

# 2. Se connecter

1. Ouvrir **Gescom Équipe**. L'écran de connexion est celui de Gescom :
   **Pseudo ou email**, **Mot de passe**, **Se connecter**.
2. Avec plusieurs dossiers : « Quel dossier ouvrir ? ». La case « Ouvrir
   celui-ci directement la prochaine fois » évite la question. Pour
   changer de dossier : se déconnecter, se reconnecter.
3. Un mot de passe neuf ou donné par le patron se change avant tout le
   reste.

La session d'Équipe est **séparée** de celle de la caisse : se
déconnecter de l'une ne ferme pas l'autre.

**Le menu de gauche** n'affiche que ce que le compte a le droit de
faire : Accueil, Personnel, Paie, Suivi clients. En bas : le nom, le
rôle, **Se déconnecter**.

**« Serveur injoignable (…) »** : le serveur ne répond pas. Vérifier le
réseau, et que Gescom Serveur tourne sur l'ordinateur principal (ou que
la caisse de ce poste, sur un ordinateur unique, est ouverte). Si le
serveur redémarre, Équipe revient à l'écran de connexion.

## Qui fait quoi

Les droits se donnent dans la **caisse**, Paramètres → Utilisateurs →
Permissions.

| Permission | Ce qu'elle permet |
|---|---|
| Créer et modifier les fiches du personnel | nouvelle personne, modifier, départ, retour, marquer les jours |
| Donner une avance sur salaire | donner et annuler une avance |
| Préparer les fiches de paie et voir les salaires | préparer, corriger, recalculer, jeter un brouillon ; régler les cotisations |
| Valider et payer les fiches de paie | valider, verser, faire une rectificative |
| Suivre les clients | tout le suivi clients |

Le patron a tout. Le comptable reçoit d'office « préparer la paie » et
« suivre les clients » ; le caissier et l'employé « suivre les
clients ». « Donner une avance » se donne à la main.

> **Sans droit de paie**, les montants sont cachés : salaire, tarif,
> commission, plafond d'avance. Une fiche du personnel modifiée garde sa
> rémunération ; l'Historique cache les montants de la paie.

---

# 3. Le personnel

**Personnel → Fiches.** La liste, avec la fonction, la façon d'être
payé, le téléphone ; « sans contrat écrit » si c'est le cas. La case
**Voir les départs** montre aussi ceux qui sont partis.

## Ajouter une personne

**Nouvelle personne.**

1. **Nom** et **Ce qu'elle fait** (vendeuse, magasinier, livreur…) —
   les deux seuls champs obligatoires. Travailler sans contrat est
   permis.
2. **Comment elle est payée** — cocher un ou plusieurs modes, ils
   s'additionnent :

| Mode | Ce qu'on saisit | Ce que la paie calcule |
|---|---|---|
| Au mois | F par mois | le salaire, sur un mois civil |
| À la journée | F par jour travaillé | les jours marqués présents |
| À la commission | % | un pourcentage de l'argent reçu sur ses ventes (chapitre 5) |
| À la tâche | rien | des lignes ajoutées sur la fiche (« 3 livraisons × 1 000 F ») |

   Rien de coché : rien de fixe, des primes à la paie.
3. **Plus d'informations (facultatif)** : téléphone, **Entrée le**, pièce
   d'identité, **Compte Gescom** (obligatoire pour la commission : c'est
   le compte dont les ventes comptent), magasin, contrat écrit et sa
   date, **Déclarée (INPS)** et son numéro, **plafond de ses avances en
   cours**, note.
4. **Enregistrer.**

Un compte Gescom ne se lie qu'à **une** personne active : sinon ses
ventes compteraient deux fois.

## Un départ

L'icône **Départ** sur la ligne : **Part le** (aujourd'hui par défaut),
un motif si on veut, **Confirmer le départ**. La fiche reste, avec tout
son historique.

Gescom prévient sans refuser :

- s'il reste une **avance** en cours : elle se retiendra sur la dernière
  fiche, ou restera due ;
- si une fiche validée **paie des jours après le départ** : faire une
  rectificative (le trop-versé deviendra une avance) ;
- pour un vendeur à la commission : **ce que ses clients paieront après
  son départ ne lui rapportera rien**.

Un départ ne se date pas avant des jours déjà marqués : les effacer
d'abord dans Jours travaillés.

## Un retour

**Voir les départs** → **Revient** → **Revient le** → **Confirmer le
retour**. Après une absence, la date de retour devient sa **date
d'entrée** : les mois d'absence ne se paient pas. Sa paie jusqu'au
départ doit être validée avant.

## Les jours travaillés

**Personnel → Jours travaillés**, un **Mois** à choisir. Une ligne par
personne, une colonne par jour (week-ends grisés), le total à droite.

- Un clic sur une case la fait tourner : vide → **P** (présent) →
  **½** (demi-journée) → **A** (absent) → vide.
- Un clic sur le **numéro du jour** : tout le monde présent, sans
  toucher aux absences déjà notées.
- On ne marque ni un jour à venir, ni avant l'entrée, ni après le départ.
- Un jour déjà payé par une fiche validée est **gelé** : pour le
  changer, faire une rectificative de cette fiche.

Ce que compte la ligne est ce que lira la paie d'un journalier. Pour un
salaire au mois au prorata, **un jour non marqué compte présent**.

---

# 4. Les avances

**Paie → Avances.** Le trou le plus courant n'est pas le calcul du
salaire : c'est l'avance donnée au milieu du mois et oubliée à la paie.
Ici, elle est notée, puis **retenue d'office** sur la fiche suivante.

## Donner une avance

Bloc **Donner une avance** :

1. **À qui**, **Montant**, **Donnée en** (Espèces, Orange Money, Moov
   Money).
2. **Le** — le jour où l'argent a été donné : aujourd'hui par défaut,
   au plus 31 jours en arrière, jamais dans le futur, jamais avant
   l'entrée.
3. **Pour quoi** (facultatif).
4. **Pris dans le tiroir de la caisse** — à cocher quand l'argent sort du
   tiroir (espèces seulement). La sortie s'écrit alors **dans la caisse
   ouverte**, avec l'avance ; la date est forcément aujourd'hui.
5. **Retenir par morceaux** — pour une grosse avance : « 10 000 F par
   paie ». Sans cette case, toute l'avance se retient sur la prochaine
   paie.
6. **Donner l'avance.**

> **Pris dans le tiroir ? Cochez la case.** Sinon il manquera l'argent à
> la clôture de la caisse. Et ne saisissez pas en plus une dépense
> « Salaire » dans la caisse : ce serait compté deux fois.

> La caisse doit être **ouverte** pour une avance prise dans le tiroir.
> Fermée, Gescom refuse (« CAISSE_FERMEE ») et rien n'est écrit : ouvrir
> la caisse d'abord, dans Gescom.

Une avance datée d'une période **déjà payée** est acceptée : l'écran dit
qu'elle sera retenue sur la paie suivante.

## La liste

Pour chaque personne, ce qui est **en cours**. Chaque avance dit sa
date, son moyen, « pris dans le tiroir », « X F par paie », « X F déjà
retenus ». **Voir aussi les retenues et annulées** montre l'historique.
Une avance née d'un trop-versé se dit « versé en trop sur PAIE-…,
repris en avance ».

## Annuler une avance

L'icône d'annulation, seulement tant que **rien n'en a été retenu** (une
erreur de saisie). Prise dans le tiroir, **l'argent revient dans la
caisse** — elle doit être ouverte.

## Les règles

- Le **plafond** d'une personne compte ce qu'elle doit déjà : au-delà,
  Gescom refuse.
- Ce qui dépasse le net d'une paie **se reporte** sur la suivante.
- La retenue par morceaux compte **à chaque paie** : un journalier payé à
  la quinzaine rembourse donc deux fois par mois.

---

# 5. Les fiches de paie

**Paie → Fiches du mois.** Choisir le **Mois**. Le bloc **À préparer**
liste les personnes employées au moins un jour de ce mois (avec « entrée
le » ou « partie le »).

## Préparer

La case **au prorata** (salariés au mois), puis **Préparer la fiche de
…**. Le brouillon s'ouvre : chaque ligne dit d'où elle vient, puis
**Brut**, **Retenues**, **Net à payer**.

Ce que la fiche calcule toute seule :

| Ligne | Comment |
|---|---|
| Salaire du mois | sur un **mois civil entier** ; une arrivée ou un départ en cours de mois ne paie que les jours d'emploi |
| Au prorata | on retire les absences, et une demi-journée par « ½ » ; un jour non marqué compte présent |
| Jours | les jours marqués présents × le tarif |
| Commission | le pourcentage de **l'argent reçu** dans le mois sur ses ventes, moins ce qui a été rendu (retours) ; jamais négative |
| Avances | les avances en cours, les plus anciennes d'abord, chacune au plus de sa retenue par paie |
| Cotisations | seulement pour une personne déclarée, et seulement celles réglées (chapitre 7) |

> **La commission suit l'argent reçu.** Une vente à crédit compte le
> mois où le client paie, au fil de ses paiements ; jamais payée, elle
> ne rapporte rien. Une vente déjà commissionnée sur une ancienne fiche
> ne compte pas une seconde fois. Sans compte Gescom lié : « pas de
> compte lié, aucune vente à compter ».

## Corriger un brouillon

**Ajouter** : **Prime**, **À la tâche** (Combien × Prix de chacune) ou
**Retenue** (casse, absence… — une retenue dit toujours pourquoi), puis
**Ajouter la ligne**. Une ligne ajoutée se retire par sa corbeille. Les
lignes calculées ne se corrigent pas ici : sur la fiche de la personne,
les jours, ou la caisse.

**Recalculer** relit tout (après des jours marqués, une avance…).
**Jeter le brouillon** le supprime.

## Valider

**Valider la fiche** (droit de valider) : elle reçoit son numéro
(`PAIE-2026-00001`) et **ne change plus**. Ses jours sont gelés. On peut
valider avant la fin du mois. Les retenues ne peuvent pas dépasser ce
que la personne a gagné.

## Corriger une fiche validée : la rectificative

**Faire une rectificative.** Une nouvelle fiche s'ouvre, qui **garde les
conditions de l'ancienne** — le salaire et le taux de l'époque, même si
la fiche du personnel a changé depuis. L'ancienne reste valable jusqu'à
la validation de la nouvelle, puis s'affiche barrée, « remplacée
par … ». Les versements déjà faits passent sur la nouvelle.

Si la rectificative révèle qu'on a **trop versé**, la différence devient
une **avance**, retenue sur la fiche suivante.

## Verser le salaire

Sur une fiche validée qui a un reste :

1. **Montant** — vide : tout ce qui reste.
2. **Par** : Espèces, Orange Money, Moov Money, Virement, Chèque.
3. **Pris dans le tiroir** (espèces) : la sortie s'écrit dans la caisse
   ouverte, comme pour une avance.
4. **Verser.**

On peut verser en plusieurs fois. La fiche affiche « reste X F »,
« payée », « versé en trop » ou « repris en avance ».

## Le bulletin

**Bulletin**, puis **Imprimer** : le numéro, la période, le salarié
(téléphone, entrée, N° INPS s'il est déclaré), ce qui est dû, ce qui est
retenu, le **net à payer en chiffres et en lettres**, les charges de
l'employeur pour information, les versements, et deux signatures
(« L'employé », « Pour la société »). Un brouillon ne s'imprime pas. Une
fiche remplacée porte en rouge « ne vaut plus ».

Le format et les signatures se règlent dans Gescom → Paramètres →
Documents → Bulletin de paie.

---

# 6. Ce que Gescom refuse, et pourquoi

| Message | Ce qu'il faut faire |
|---|---|
| « Une personne payée au mois se paie sur un mois entier… » | préparer le mois du 1er au dernier jour |
| « … n'était pas employé(e) … » | vérifier les dates d'entrée et de départ |
| « … a déjà une fiche … » | ouvrir la fiche existante |
| « … est calculée : elle se corrige sur la fiche de la personne, les jours ou la caisse » | corriger à la source, puis Recalculer |
| « Les retenues … dépassent ce que la personne a gagné » | réduire une retenue avant de valider |
| « La fiche … est validée : elle ne change plus » | faire une rectificative |
| « Un brouillon ne se paie pas » | valider d'abord |
| « Il ne reste que X F à verser » | verser le reste, pas plus |
| « … on dépasserait son plafond » | une avance plus petite, ou voir qui prépare la paie |
| « Seules les espèces se prennent dans le tiroir » | décocher la case, ou choisir Espèces |
| « CAISSE_FERMEE — la caisse n'est pas ouverte » | ouvrir la caisse dans Gescom |
| « Une fiche de paie en a déjà retenu une partie : elle ne s'annule plus » | la laisser ; corriger par la paie |

---

# 7. Les cotisations

**Paie → Cotisations.** Elles ne s'appliquent qu'aux personnes
**déclarées**, et seulement une fois saisies ici. **Tant qu'il n'y en a
aucune, rien n'est retenu** : c'est le comptable qui les remplit, avec
les taux en vigueur. Un taux faux sur un bulletin est pire qu'aucun.

**Ajouter une cotisation** (droit de préparer la paie) :

- **Nom** (« INPS part salariale ») ;
- **Qui la paie** : le salarié (une retenue sur son salaire) ou
  l'employeur (une charge, qui ne diminue pas le net) ;
- **Taux %**, **Plafond** (facultatif), **Compte** (facultatif, 431…).

Les brouillons se recalculent pour la prendre ; les fiches validées ne
changent pas.

> **L'ITS** suit un barème par tranches et la situation de famille : il
> ne se calcule pas ici. Le comptable le porte sur la fiche, en
> **retenue** (« ITS du mois »).

---

# 8. Le suivi des clients

**Suivi clients**, trois onglets. Ce sont les clients de Gescom : pas de
second fichier.

## Clients et fiche 360

Chercher par nom, code ou téléphone ; un clic ouvre la **fiche 360** :
chiffre d'affaires, ventes, encours, puis :

- **Rappels** — **Pour qui** (moi par défaut), **Quand**, **Quoi**,
  **Rappeler**. L'icône « Marquer fait » le range.
- **Échanges** — **Genre** (Appel, Visite, WhatsApp, Note), **Ce qui
  s'est passé**, **Suite prévue**, **Noter**. Les relances de créance
  faites depuis la caisse y figurent aussi.

## Prospects

**Nouveau prospect** : un client sans vente encore — nom, téléphone,
d'où il vient (salon, recommandation…). **Il devient client tout seul à
sa première vente.**

## Mes rappels

Les rappels non faits qui vous sont attribués. Un clic ouvre le client.

---

# 9. En cas de doute

- Une fiche validée ne se corrige jamais en douce : **rectificative**.
- Une avance oubliée se saisit **à sa vraie date** (31 jours au plus).
- L'argent qui sort du tiroir passe par **la case « pris dans le
  tiroir »**, jamais par une dépense « Salaire » en plus.
- Avant une opération qui vous inquiète : une sauvegarde (dans Gescom,
  Paramètres → Sauvegarde → Sauvegarder maintenant).
