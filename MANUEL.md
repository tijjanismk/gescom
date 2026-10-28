# Gescom — Manuel d'utilisation

*Version 2.1 — 28 octobre 2026 — à imprimer et garder près de la caisse.*

---

# 0. Installer

L'installation a son propre document : le **Guide d'installation**
(`INSTALLATION.md`, imprimé à part). L'essentiel :

- **Gescom** (la caisse) s'installe sur chaque poste, sans droits
  administrateur. Au premier démarrage, l'écran **« Comment travaille
  ce poste ? »** demande, une seule fois, son rôle.
- **Un seul ordinateur** : choisir **« Ce poste garde la base »**. Gescom
  lance lui-même son serveur ; rien à saisir.
- **Plusieurs caisses** : installer d'abord **Gescom Serveur**, en
  administrateur, sur l'ordinateur principal. Puis, sur chaque caisse,
  choisir **« Ce poste est une caisse d'un autre poste »**, cliquer
  **Rechercher le serveur**, **Essayer**, **Enregistrer**.
- Le serveur écoute sur le port **40000** (de 40000 à 40100). Sa
  fenêtre, **Gescom Serveur** au menu Démarrer, donne son nom, son
  adresse et les postes connectés.
- **Gescom Équipe** (personnel, paie, suivi clients) s'installe après la
  caisse du même poste ; elle a son propre manuel.

Si la caisse dit **« Serveur injoignable »** : le serveur tourne-t-il
(fenêtre Gescom Serveur → « Démarrer le service ») ? l'ordinateur
principal est-il allumé ? le réseau Wi-Fi est-il en « Privé » ? Le
Guide d'installation, chapitre 14, détaille les autres cas.

---

# 1. La journée type

```
MATIN     Ouvrir la caisse, compter l'argent du tiroir
JOURNÉE   Vendre, encaisser, acheter, noter les dépenses
SOIR      Compter le tiroir, clôturer, imprimer le journal
SAMEDI    Sauvegarder sur la clé USB
```

**Si vous ne deviez retenir qu'une chose** : ouvrir la caisse le matin
et la clôturer le soir. Tout le reste en dépend.

---

# 2. Se connecter

Deux comptes existent.

**Patron** — accès complet : prix d'achat, marges, rapports, paramètres.

**Employé** — vend, encaisse, consulte le stock. Il ne voit ni les prix
d'achat, ni les rapports, ni la sauvegarde.

Chacun doit avoir son mot de passe. Ne les partagez pas : c'est ce qui
permet de savoir qui a fait quoi en cas d'écart.

Les deux comptes livrés (`admin` / `employe`) exigent un nouveau mot de
passe à la première connexion. Le patron ajuste ensuite ce que chacun
peut faire : **Paramètres → Utilisateurs → Permissions**, permission par
permission.

## Aller vite : Ctrl + K

Partout dans l'application, **Ctrl + K** ouvre la palette : tapez
quelques lettres, la liste se réduit, Entrée exécute.

- une page (« ventes », « caisse », « stock ») ;
- un geste de l'écran où vous êtes (« nouveau client », « ouvrir la
  caisse ») ;
- un onglet de Paramètres ;
- **le nom d'un client ou d'un fournisseur** — deux lettres suffisent,
  choisir ouvre sa fiche.

La palette ne propose que ce que votre compte a le droit de faire.

## Un menu à la taille de la boutique

**Paramètres → Menu.** Une case par entrée du menu : décocher ce dont la
boutique ne se sert pas (Relances, Chèques, Historique…). **Boutique
simple** ne garde que l'essentiel, **Complet** remet tout ; puis
**Enregistrer le menu**.

Tableau de bord, Ventes, Caisse et Paramètres restent toujours. Le
réglage vaut pour toutes les caisses du dossier, et la palette Ctrl + K
le suit. Cacher une entrée ne change rien aux données ni aux règles :
elle disparaît seulement de l'écran. Réservé au patron (modifier les
paramètres).

---

# 3. La caisse

## Ouvrir le matin

**Caisse → Ouvrir.** Comptez les billets et pièces présents dans le
tiroir, saisissez ce montant.

Ce montant est le *fond d'ouverture*. Il n'est pas un revenu.

## Pendant la journée

Chaque encaissement s'inscrit tout seul. Vous n'avez rien à saisir.

**Sauf les dépenses.** Transport, carburant, déjeuner, monnaie prêtée :
tout ce qui sort du tiroir doit être noté, sinon la caisse ne tombera
pas juste le soir.

**Caisse → Dépense.** Libellé, montant, poste.

> **Le poste « Salaire / main d'œuvre »** est pour la main-d'œuvre d'un
> jour (porteur, déchargeur). Une avance ou un salaire du personnel
> suivi dans Gescom Équipe se donne **depuis Équipe**, case « pris dans
> le tiroir » : la sortie s'écrit alors dans la caisse toute seule.
> Saisi ici en plus, ce salaire serait compté deux fois.

## Clôturer le soir

**Caisse → Clôturer.** Comptez réellement les espèces du tiroir et
saisissez le montant. **Ne recopiez pas le solde théorique** — c'est
tout l'intérêt de l'opération.

L'écart s'affiche :

| Écart | Ce que ça veut dire |
|---|---|
| **0 F** | Tout est juste |
| **Manque** | Une sortie d'argent n'a pas été saisie |
| **Excédent** | Une vente n'a pas été enregistrée |

> **Orange Money et Moov Money ne sont pas dans le tiroir.** On ne
> compte que les espèces. Les chèques non plus.

## L'historique

**Caisse → Historique.** Les clôtures passées, avec leur écart.

Un écart isolé ne dit rien. C'est la suite qui parle, et l'application
vous donne son interprétation : manques réguliers, excédents réguliers,
ou écarts irréguliers.

---

# 4. Vendre

**Ventes.**

1. Choisir le client, ou laisser **Comptant** pour un client de passage
   (il paie alors tout, tout de suite — voir plus bas)
2. Chercher l'article, ajuster la quantité
3. Ajuster le prix ou saisir une **remise en %**
4. En haut à droite : **Comptant** ou **Crédit**
5. **Encaisser**

## La vente d'hier, saisie ce soir

Le cahier se recopie le soir, ou le samedi pour la semaine. Juste
au-dessus d'**Encaisser**, un champ **« Vente du »** : laisser vide pour
aujourd'hui, ou poser la date réelle de la vente.

Deux règles, et elles ne se discutent pas :

- **la vente prend la date saisie ; la caisse, non.** L'argent est
  compté dans le tiroir du jour où on le saisit — c'est ce que le
  comptage du soir vérifie. Le mouvement de caisse porte « Vente du
  jj/mm » pour qu'on s'y retrouve ;
- **pas plus de 31 jours en arrière, jamais dans le futur**, et
  seulement pour qui a la permission *antidater* (le patron, par
  défaut). Antidater une vente en espèces est la façon la plus simple
  de masquer un trou dans le tiroir : c'est pour ça que c'est un droit à
  part.

La même chose existe pour un règlement (« Réglé le »), une réception de
marchandise (« Réception du ») et une pièce créée à la main (« Date de
la pièce »).

## Le prix et la remise

Le prix se remplit tout seul. Pour accorder une remise sur un article,
saisissez le pourcentage : le prix se recalcule et l'économie
s'affiche. Vous pouvez aussi taper directement le nouveau prix.

**Remise sur tout le panier** : sous le panier, le champ **Remise**, en
**%** ou en **F** au choix. Elle se répartit sur les lignes ; le total
affiché est ce que le client paie, et la facture montre la remise
ligne par ligne.

## Comptant ou crédit

**Comptant** — le client paie tout de suite.

**Crédit** — le client doit de l'argent. Vous pouvez saisir un
**acompte** : ce qu'il verse aujourd'hui. Le bouton reste grisé tant
qu'aucun client n'est choisi : on ne fait pas crédit à quelqu'un sans
nom.

## Les clients de passage

Une vente au **Comptant** (sans nom) ouvre sa propre fiche **« passage »**,
avec un code `PAS-000123`. Elle n'encombre ni la liste des clients ni
les chiffres, mais on la retrouve.

- **Un passant paie tout, tout de suite.** Pas de crédit, pas de
  paiement en partie : sinon Gescom refuse et dit de créer le client
  d'abord.
- **Sur le papier**, la facture et le reçu portent le code `PAS-…`,
  jamais « Client de passage ».
- **Clients → Passages** : les retrouver par leur code ou par le numéro
  de facture. Deux gestes (permission « modifier les clients ») :
  - **Passer en client ordinaire** : lui donner un nom (obligatoire) et
    un téléphone ; il devient un client comme les autres ;
  - **Rattacher à un client** : le passant était en fait un client
    connu ; ses ventes, paiements, retours et pièces passent sur la
    fiche de ce client. **Ce geste ne se défait pas.**
- Un passant qui rapporte un article est remboursé ou fait un échange :
  pas d'avoir.

## La marchandise est dans un autre dépôt

Sous la quantité, l'application indique où se trouve l'article :

```
5 ici · 30 à Djelibougou
```

Si vous demandez 20 et qu'il n'y en a que 5 sur place, une fenêtre
s'ouvre : **« D'où sort la marchandise ? »**

Indiquez combien vient de chaque endroit, envoyez quelqu'un chercher le
complément, puis validez au retour. Chaque stock baissera au bon
endroit.

## La facture

Après la vente, choisissez le format : A4, A5, ou ticket 58/80 mm.
**Passer** si le client n'en veut pas — la facture reste disponible dans
l'écran Pièces. La fenêtre d'impression se ferme toute seule après
« Imprimer ».

---

# 4 bis. Les pièces commerciales

L'écran **Pièces** rassemble tous les documents, côté client et côté
fournisseur. Chacun découle du précédent.

```
CLIENT       Devis → Commande → Livraison → Facture
FOURNISSEUR  Bon de commande → Réception → Facture
```

Vous n'êtes obligé de passer par aucune étape : un client pressé peut
recevoir directement une facture.

## Le bon de livraison : préparer, émettre

Un bon de livraison (ou de réception, côté fournisseur) créé à la main
naît en **brouillon** : on le prépare, on corrige les lignes, rien ne
bouge dans le magasin. Le bouton **Émettre** constate le mouvement :
toute la marchandise du bon sort (ou entre), et le bon ne se modifie
plus. S'il faut le corriger ensuite : l'**annuler** — la marchandise
revient — et en refaire un.

Le livreur revient avec deux sacs refusés ? Sur le bon émis, menu **…
→ Livraison** : on saisit ce qui est réellement parti, ligne à ligne,
et le stock suit l'écart. Un bon en brouillon ne se facture pas : il
faut l'émettre d'abord.

**Annuler par avoir une facture issue d'un bon** : la marchandise
revient au magasin (une fois), le bon garde sa trace de livraison. Si
le client n'a rien rendu, ce n'est pas une annulation : c'est une
créance à traiter.

**Filtrer sur la livraison** : dans les filtres avancés de Pièces,
quand le suivi est actif — non livré, partiellement, livré. C'est là
qu'on trouve le « payé, pas encore livré ».

Un bon issu d'une **commande** (bouton →) naît directement émis : la
marchandise part avec lui.

## Faire avancer un document

Le bouton **→** de la ligne transforme la pièce en la suivante. La pièce
d'origine est archivée, elle n'est plus modifiable.

Sur une commande client, le menu **…** propose aussi **« → Livraison +
facture »** : les deux documents d'un coup, quand la marchandise part
avec sa facture.

**Une pièce ne se transfère qu'une fois.** Une commande déjà facturée ne
peut plus produire une seconde facture, et un bon de livraison déjà
facturé non plus. L'application refuse et vous dit quel document existe
déjà — c'est ce qui empêche de facturer deux fois la même marchandise.

Si ce document est faux, annulez-le : la pièce d'origine redevient
transférable.

## Ce qui est annulé ne compte plus

Une pièce annulée reste visible — c'est la trace, et la faire disparaître
serait pire. Mais son montant sort de tous les totaux : bas de l'écran
Pièces, créances du client, chiffre d'affaires.

Même sort pour une **facture irrécouvrable** (voir *Les créances*) :
toute la ligne est **en rouge**, elle ne compte dans aucun total, et
elle ne figure plus parmi les impayés ni les retards. L'écran l'écrit
sous le tableau : *« dont 1 annulée, 1 irrécouvrable, hors total »*.

## Les avoirs dans la liste

Le « reste » d'un **avoir** n'est pas un impayé : c'est un **crédit**,
en ambre. Côté client, ce que le client peut encore consommer ; côté
fournisseur, ce que le fournisseur doit déduire de sa prochaine
facture. Un avoir fournisseur **remboursé** en espèces ne vaut plus rien
— il est marqué *Payé* et son reste est vide.

## Accorder un avoir sans marchandise

Un geste commercial, un dédommagement, une remise après coup : le client
n'a rien rendu, on lui doit quand même. **Fiche du client → Avoirs →
Accorder un avoir** : montant, motif obligatoire. Ou **Pièces →
Nouvelle pièce → Avoir** : sans ajouter d'article, saisir le montant
et mettre le motif dans « Note ». Le crédit entre au
compte du client avec une pièce AVC numérotée et une trace au journal ;
il se consomme sur une prochaine vente ou se rembourse.

C'est un crédit qui sort de nulle part : **seul le patron** peut le
faire (permission *Accorder un avoir sans marchandise*, qu'aucun rôle
livré ne porte — on la donne à la main, à une personne).

## Créer une pièce à la main

Bouton **Nouvelle pièce** en haut de l'écran. Le filtre choisi dit déjà
le type : sur « Commandes » le bouton s'appelle *Nouvelle commande* et
la fenêtre ne redemande pas le type ; sur « Tout », on le choisit.

La fenêtre demande le tiers, les articles, éventuellement une
**échéance** (quand le client doit payer) et, pour qui peut antidater,
la **date de la pièce** (le jour de l'affaire, si on la saisit après
coup). Ce sont deux dates différentes.

**Du → au**, dans la barre : n'afficher que les pièces d'une période —
côté client comme côté fournisseur.

## Les modèles de documents

**Paramètres → Documents.** Chaque document imprimé — facture, devis, bon
de livraison, reçu, relevé, ticket — suit un **modèle** : une suite de
blocs (en-tête, titre, champs, tableau, totaux, texte, signatures, pied
de page) qu'on réordonne, règle et prévisualise avec des données
d'exemple.

**Les signatures** sont un bloc du modèle : deux noms, un trait sous
chacun. Vider les deux noms retire le bloc. Sur un document de deux
pages, elles partent à la fin.

**Poser un bloc où l'on veut.** Dans l'aperçu, tirer le **coin
bas-droit** d'un bloc : il se détache et se redimensionne. Une fois
détaché, il se déplace à la main et peut chevaucher les autres — un
cachet « PAYÉ » en travers du tableau, un logo dans un coin. L'ordre de
la liste de gauche est l'ordre de superposition : ce qui vient plus bas
passe devant.

**D'une caisse à l'autre.** Les modèles vivent sur le serveur : toutes
les caisses impriment pareil. **Exporter** sort un fichier, **Importer**
le reprend — pour copier ses modèles vers un autre magasin.

Sur une pièce fournisseur, le champ « Client » du modèle s'imprime
« Fournisseur » tout seul.

## Voir avant d'imprimer

Le bouton **œil** montre le document exactement tel qu'il sortira. Vous
pouvez y changer le format — A4, A5, ticket — et voir le résultat avant
d'engager du papier.

## Original et duplicata

Une pièce n'a qu'un **original**. Pour un genre de document (facture,
devis, bon de commande, bon de livraison, reçu, ticket), cocher la case
**Duplicata** dans **Paramètres → Documents** : la première impression
est l'original, toutes les suivantes — sur n'importe quelle caisse —
portent **DUPLICATA** en travers et la date de l'original. L'aperçu le
dit avant : « Original imprimé le … — celle-ci sortira en duplicata. »

**Réimprimer l'original** (dans l'aperçu) est réservé au patron, et
noté dans l'Historique. La case est décochée par défaut : sans elle,
chaque impression sort sans mention. Si le serveur ne répond pas, rien
ne part à l'imprimante : réessayer.

## La facture ne devient réelle qu'à sa validation

Une facture reste en **brouillon** tant que vous ne l'avez pas validée.
C'est la validation qui crée la vente, sort la marchandise du stock et
encaisse l'argent. Avant, rien n'a bougé.

## Suivre les livraisons

Réglage à activer dans **Paramètres → Ventes** si vous livrez. Chaque
pièce reçoit alors un second badge — *livré*, *partiellement livré* — à
côté de son statut de paiement. C'est ce qui rend visible le cas « payé,
pas encore livré ».

C'est une information de suivi : **le stock et la caisse ne bougent
pas**. La marchandise sort toujours à la validation de la facture.

---

# 5. Les créances

Un client qui doit de l'argent apparaît dans **Clients** et dans
**Relances**.

## Encaisser un règlement

Deux chemins :

**Pièces** → trouver la facture → bouton **₣ Encaisser**
**Clients** → fiche du client → onglet Créances → **Régler**

Le montant est prérempli avec le reste dû. Modifiez-le pour un paiement
partiel.

Quand la facture est soldée, elle passe automatiquement en **Payé**.

Le champ **« Réglé le »** pose la date réelle du versement quand on le
saisit après coup — même règle que pour la vente : la créance prend
cette date, le tiroir garde celle du jour.

## Une créance perdue

Un client parti sans adresse, une dette qu'on ne reverra pas :
**Paramètres → Irrécouvrable**, choisir la vente, dire pourquoi. Elle
sort des créances, des relances et des totaux ; sa facture passe en
rouge dans Pièces.

Ce n'est pas un effacement. Si l'argent revient malgré tout, des mois
plus tard, le patron l'encaisse par **Règlement exceptionnel** au même
endroit — le règlement ordinaire refuse, exprès, pour qu'une créance
déclarée perdue ne se solde pas en douce.

## Remettre un relevé au client

**Clients → l'imprimante sur sa ligne**, ou **sa fiche → État de
créance**. Le document liste ses factures non soldées, ce qu'il a déjà
versé et ce qui reste. Les avoirs dont il dispose sont déduits — on ne
réclame pas plus que ce qui est réellement dû.

Pour la vue d'ensemble : **Clients → État des créances** sort tous les
clients débiteurs sur une page, du plus gros au plus petit. C'est la
liste d'appel du lundi matin.

Côté fournisseur, les mêmes boutons donnent l'**état de dette**.

## Voir tous les règlements d'un client

**Sa fiche → onglet Règlements.** Chaque encaissement y figure : la
date, la facture, le moyen, le montant, et **qui l'a saisi**.

C'est le premier écran à ouvrir quand un client conteste : vous lui
montrez ce qui a été enregistré, et par qui.

**Filtrez** par période, par moyen de paiement, ou par numéro de facture
— une partie du numéro suffit, tapez « 12 » plutôt que le numéro entier.

La colonne **Reste dû après** donne le solde de cette facture-là juste
après ce versement : la dette qui descend, ligne par ligne. À ne pas
confondre avec le **reste dû total**, rappelé en orange en bas de
l'onglet — un gros total encaissé ne dit rien de ce qui reste à payer.

**Imprimer** sort exactement ce que le filtre affiche, critères indiqués
en en-tête. C'est le document à remettre au client qui conteste : les
annulations y figurent, en rouge pour la correction et barré pour le
règlement annulé.

## Le reçu de règlement

Le bouton **œil** sur une ligne sort un reçu A5 : le montant en chiffres
**et en toutes lettres**, le moyen, la facture, ce qui reste dû après ce
versement, et le nom de celui qui a encaissé.

Le montant en lettres n'est pas une décoration : c'est ce qui empêche de
transformer 5 000 en 50 000 d'un coup de stylo.

Le reçu n'est pas une pièce numérotée : c'est l'impression d'un règlement
déjà enregistré, comme le bon de sortie l'est d'une facture.

## Corriger un règlement

Un client conteste, ou le caissier s'est trompé de montant. **Onglet
Règlements → Annuler** sur la ligne concernée.

> **Le règlement n'est jamais effacé.** Une ligne de correction vient
> l'annuler, et les deux restent visibles. C'est ce qui vous permet de
> prouver au client ce qui s'est passé.

L'application demande **ce qui s'est réellement passé**, et ce n'est pas
une formalité — la réponse décide du sort de votre caisse :

**Erreur de saisie.** L'argent n'est jamais entré : montant faux, mauvais
client, ligne saisie deux fois. On corrige une écriture.

**Remboursement.** L'argent était bien entré, vous le rendez. Des billets
sortent du tiroir, la caisse doit être ouverte.

Un motif est obligatoire. C'est lui que verra le client, et c'est lui qui
explique la correction en cas de contrôle.

> Une erreur de saisie sur une journée **déjà clôturée** ne touche pas à
> la caisse d'aujourd'hui : le tiroir avait été compté ce soir-là, et
> l'écart avait déjà absorbé la ligne fausse.

## Relancer

**Relances.** La liste des clients en retard, triée par **ancienneté**.
En haut, quatre tuiles : créances ouvertes, sans relance, relances de la
semaine, montant en jeu. Sous chaque créance, « payé » ne compte que
l'argent reçu ; ce qu'un retour a effacé est dit à part (« éteint par
retour »).

Bouton **Relancer** : le message est prérempli avec le nom, le montant
et la référence. WhatsApp s'ouvre, vous vérifiez, vous envoyez.

Chaque relance est enregistrée — vous saurez qui a déjà été relancé et
quand.

---

# 6. Acheter

**Achats.**

1. Choisir le fournisseur
2. Ajouter les articles reçus, avec le prix payé
3. **Comptant** ou **À crédit**
4. Valider

Le stock augmente et une facture fournisseur est créée. À crédit, la
dette apparaît dans **Fournisseurs**.

**« Réception du »**, au-dessus du bouton : la marchandise est arrivée
un autre jour et on saisit ce soir. La facture fournisseur et le
règlement prennent cette date ; **le stock entre et l'argent sort
aujourd'hui** — c'est aujourd'hui qu'on compte l'un et l'autre.

**Achat comptant : la caisse doit être ouverte.** L'argent sort du
tiroir, donc il doit être écrit dans la caisse. Même règle qu'à la vente,
pour la même raison : sinon le comptage du soir tombe faux d'exactement
ce montant, sans rien pour l'expliquer.

**Un bon de réception ne se facture qu'une fois.** Le facturer une
seconde fois doublerait la dette envers le fournisseur, et le double
paiement suivrait. L'application refuse et vous dit quelle facture existe
déjà.

## Voir et contester les paiements

**Sa fiche → onglet Paiements.** Exactement le symétrique de l'onglet
Règlements du client : chaque versement avec sa date, sa facture, son
moyen, qui l'a saisi, et la colonne **Reste à payer après**.

**Filtrez** par période, moyen ou numéro de facture — une partie du
numéro suffit. **Imprimer** sort exactement ce que le filtre affiche.

**Annuler** corrige un versement : montant saisi deux fois, mauvais
fournisseur, mauvais montant. Deux cas à distinguer, et c'est le choix
qui décide du tiroir :

| Ce qui s'est passé | Effet sur la caisse |
|---|---|
| Erreur de saisie | Aucun — l'argent n'était jamais sorti |
| Le fournisseur rend l'argent | **Entrée** de caisse, tiroir ouvert requis |

Le paiement n'est jamais effacé : une ligne de correction vient
l'annuler, et la facture redevient due. Les deux lignes restent visibles.

## Le reçu d'un paiement fournisseur

**Sa fiche → onglet Paiements → bouton œil.** Même document que côté
client, dans l'autre sens : la preuve de ce que vous lui avez versé, à
lui opposer s'il prétend n'avoir rien reçu.

## Régler une dette

**Fournisseurs** → fiche du fournisseur → **Régler**.

Vous pouvez imputer le paiement sur une facture précise, ou laisser
« Règlement global » — il s'imputera de la plus ancienne à la plus
récente.

---

# 7. Les retours

**Retours**, onglet **Retours** (cinquante ventes par page) : dépliez la
vente, puis **Rembourser**, **Échanger** ou **Avoir**. Depuis **Pièces**,
le menu « … » d'une facture → **Retour ou échange…** ouvre la même
fenêtre.

**Plusieurs articles en un geste.** La fenêtre « Retour » liste les
articles de la vente : cochez ceux qui reviennent (**Tout cocher**
existe) et ajustez chaque quantité. Ce qui est déjà revenu est dit, et
une ligne entièrement revenue est grisée. Choisissez le mode :

| Mode | Effet |
|---|---|
| **Rembourser** | On rend l'argent |
| **Avoir** | Le client garde un crédit (pas pour un client de passage) |
| **Échange** | Il repart avec autre chose : « Ajouter un article pris en échange… », puis le bilan dit qui paie quoi |

Le geste passe en entier ou pas du tout ; l'écran dit à la fin ce qu'il
faut rendre au client.

**Ce que Pièces montre ensuite.** Une facture dont tout est revenu
affiche **Remboursée** ou **Retournée** à la place de son statut ;
sinon un badge **Retour partiel**. Sous le total : ce qui a été rendu
en argent, en avoir, en dette annulée ou échangé. La pastille
**Retours** filtre ces factures. Le chiffre d'affaires déduit les
retours au jour du retour (pas l'échange).

> **Important** : si le client n'avait pas tout payé, le retour éteint
> d'abord sa dette. On ne lui rend que ce qu'il avait réellement versé.

> **Payé avec un avoir ?** Ce qui avait été payé avec un avoir revient
> **en avoir**, jamais en espèces — même si vous choisissez
> « Remboursement ». L'écran vous dit alors combien rendre en billets et
> quel avoir a été créé. Un article vendu avec remise se rembourse au
> prix payé, remise déduite.

## Retour fournisseur

Onglet **Retour fournisseur**. Choisissez le fournisseur, sa facture,
les lignes à retourner.

Deux modes : **avoir** (le fournisseur crédite votre compte, la dette
baisse) ou **remboursement** (il rend l'argent, entrée en caisse).

---

# 8. Les chèques

Un chèque n'est pas de l'argent tant qu'il n'est pas encaissé. Il n'entre
pas dans le compte du tiroir.

À la vente, quand vous choisissez **Chèque**, saisissez le numéro et la
banque.

**Chèques** : la liste, avec le montant en attente et une alerte sur les
chèques non déposés depuis plus de 15 jours.

| Bouton | Quand |
|---|---|
| **Déposer** | Vous l'avez porté à la banque |
| **Encaissé** | L'argent est arrivé |
| **Rejeté** | La banque a refusé |

> **Un chèque rejeté ne s'efface pas, il se contre-passe.** Le
> paiement reste dans l'historique, suivi d'une ligne négative « Chèque
> rejeté » ; la caisse enregistre une sortie du même montant en chèque.
> La créance du client se rouvre : il redoit l'argent. Vous pouvez
> rejeter un chèque caisse fermée — la banque ne prévient pas aux
> heures d'ouverture.

---

# 9. Le stock

**Stock.** La liste, avec alerte sur les ruptures.

## Entrer de la marchandise

Bouton **Entrée**. Si l'article a plusieurs conditionnements, choisissez
l'unité : saisissez **10 sacs**, pas 500 kg. La conversion est
automatique.

## Corriger après un inventaire

Bouton **Ajuster**. Saisissez la quantité réellement comptée et le
motif. L'écart est enregistré.

## Imprimer l'état du stock

Bouton **État du stock**. Le document sort avec une colonne **Compté**
vide : emportez-le dans les rayons, cochez à la main, puis saisissez les
écarts dans l'application.

## Un article qu'on ne compte pas

Sachets, recharges, services, vrac : certains articles se vendent sans
qu'on suive leur stock. **Paramètres → Articles**, case **« Suivre le
stock »** décochée à la création ; pour un article existant, le déplier
puis **Ne plus suivre le stock** (le patron).

- Il se vend normalement, jamais « à découvert » ;
- il sort de l'écran Stock (la pastille **Non suivis** le montre) et de
  la valeur du stock ; les achats disent « stock non suivi » ;
- on ne le transfère pas et on ne l'inventorie pas.

**Reprendre le suivi** (bouton **Suivre le stock**) demande de compter
l'article dans **chaque** magasin : le bouton reste grisé tant qu'un
magasin n'a pas sa quantité.

---

# 10. Les dépôts

Si vous avez plusieurs lieux de stockage, un sélecteur apparaît en haut
à gauche.

**Tous les dépôts** — vue d'ensemble.
**Un dépôt précis** — le tableau de bord et le journal ne montrent que
lui, et les ventes en sortent.

> La caisse reste commune : il n'y a qu'un tiroir.

> **Le sélecteur n'apparaît qu'à partir de deux dépôts ouverts.** Avec un
> seul, il n'y a rien à choisir : il reste masqué.

## Ouvrir, renommer, fermer

**Paramètres → Magasins.**

**Créer** un dépôt, le **renommer**, ou désigner celui **par défaut** —
celui que la caisse et les ventes proposent d'abord.

## Fermer un dépôt

Une réserve qu'on n'utilise plus se **ferme** ; elle ne se supprime pas.
Son historique reste entier : les ventes qui en sont sorties, les
transferts, les mouvements de stock.

Un dépôt fermé disparaît des sélecteurs. Il n'apparaît plus ni dans les
ventes, ni dans les transferts, ni dans le filtre du tableau de bord.

**S'il reste du stock dedans**, Gescom vous le dit et demande
confirmation. La marchandise n'est pas perdue : elle est **gelée**. Elle
sort des états de stock tant que le dépôt est fermé, et revient telle
quelle à la réouverture. Rien n'a bougé, rien n'a été transféré.

> **Si le dépôt fermé était celui que vous consultiez**, le filtre
> repasse tout seul sur **Tous les dépôts**. Sans quoi le tableau de bord
> continuerait d'afficher les chiffres d'un lieu fermé — c'est-à-dire des
> zéros partout, sans dire pourquoi, et sans moyen d'en sortir puisque le
> sélecteur ne propose plus ce dépôt.

## Rouvrir

**Paramètres → Magasins → Rouvrir.** Le dépôt revient dans les sélecteurs,
avec le stock qu'il avait au moment de la fermeture.

## Transférer

**Transferts.** Choisissez le départ, l'arrivée, les articles. Un bon
numéroté est créé, imprimable et signable par celui qui reçoit.

Un transfert n'est **ni une vente ni un achat** : votre chiffre
d'affaires ne bouge pas.

> Un transfert **refuse** de mettre le dépôt de départ à découvert. On ne
> déplace que ce qu'on a. Il refuse aussi un article qui ne suit pas son
> stock : il n'y a rien à déplacer.

---

# 11. Le journal

**Journal.** Tout ce qui s'est passé dans la journée :

- **Anomalies à vérifier**, en rouge, en tête — seulement s'il y en a :
  ce que le programme a constaté sans pouvoir le régler (un montant de
  retour ni imputable ni remboursable, par exemple). À regarder le
  jour même.
- Les ventes, ligne par ligne
- **Hors du jour** — l'argent reçu aujourd'hui sur des ventes anciennes
- Les achats, les retours, les dépenses
- Les impayés
- Le récapitulatif

> **Deux chiffres à ne pas confondre** : le *chiffre d'affaires du jour*
> (ce que vous avez vendu) et l'*encaissé du jour* (l'argent reçu, y
> compris sur des ventes anciennes). Ils ne s'additionnent pas.

Imprimable, avec un emplacement pour les signatures.

---

# 12. Le catalogue

## Reprendre une liste Excel

**Paramètres → Import/Export → Choisir un fichier.**

Format attendu :

```
Nom;Categorie;Unite;Prix;Prix achat;TVA %;Code barre;Stock;Suivre le stock
Sucre;Alimentaire;kg;1500;1200;18;;50;oui
Recharge 1000;Services;unite;1000;950;;;;non
```

Seuls le **nom** et le **prix** sont obligatoires. **Suivre le stock**
(oui / non) est facultatif : vide, un nouvel article suit son stock.
L'import ne change pas ce réglage sur un article qui existe déjà. Une catégorie
inconnue est créée. Un article de même nom est mis à jour, jamais
dupliqué.

Les lignes incorrectes sont signalées une par une — les autres sont
importées.

## Codes-barres

**Paramètres → Codes-barres.** Bouton **Générer les manquants** pour
attribuer un code à tout le catalogue.

Un article qui a déjà un code fabricant le garde.

Cochez des articles et cliquez **Étiquettes** pour imprimer, quatre par
ligne. Chaque étiquette porte le code-barres dessiné (EAN-13, 31 mm,
lisible à la douchette sur une imprimante de bureau) et le numéro en
clair dessous, pour le saisir à la main si la douchette manque. Un code
fabricant qui n'a pas treize chiffres s'imprime en numéro seul.

---

# 13. Sauvegarder

La base vit sur le **serveur** : c'est lui qui sauvegarde, dans le
dossier réglé dans **Paramètres → Sauvegarde** (par défaut le dossier
`sauvegardes` à côté de la base). La fenêtre **Gescom Serveur** (la
console, sur l'ordinateur du serveur) montre la dernière sauvegarde et
permet d'en lancer une.

## La sauvegarde automatique

Le serveur vérifie cinq minutes après son démarrage, puis chaque heure :
si la dernière copie a vingt-quatre heures, il en fait une. **Quatorze**
copies sont gardées. Un ordinateur éteint le soir est sauvegardé le
lendemain matin.

Si le dossier réglé n'est pas joignable (clé USB absente), un message
vous le dit. **Ne l'ignorez pas** : tant qu'il s'affiche, aucune copie
n'est faite. Et chaque samedi, copiez le dossier des sauvegardes sur une
clé gardée hors de la boutique.

## La sauvegarde à la main

Le bouton reste là pour les jours qui comptent : après un inventaire,
avant une opération importante, avant de confier l'ordinateur à
quelqu'un.

> **Ne copiez jamais le fichier de base à la main.** Les écritures
> récentes vivent dans un fichier annexe : vous récupéreriez une base
> vide sans le savoir. Utilisez toujours le bouton.

## Restaurer une sauvegarde

Ça se fait **sur l'ordinateur du serveur, service arrêté**, jamais
depuis une caisse : restaurer efface tout ce qui a été saisi depuis la
sauvegarde, et ce geste-là ne doit pas tenir en un clic.

1. Arrêter le service : `sc stop GescomServeur`.
2. Dans une invite de commandes **en administrateur**, dans le dossier
   d'installation (`C:\Program Files\Gescom Serveur`) :
   `gescom-serveur.exe --restaurer "D:\Sauvegardes\gescom_….db" --base C:\ProgramData\Gescom\gescom.db`
   (ou le fichier `.dump` et l'adresse de la base sur PostgreSQL).
   **N'oubliez pas `--base`** : sans lui, la copie irait dans la base de
   l'utilisateur Windows qui tape la commande, pas dans celle du
   service.
3. Le programme vérifie que le fichier est bien une base Gescom, garde
   la base en place à côté (`…avant-restauration-<date>`) et dit ce
   qu'il a fait. Un fichier abîmé est refusé, rien n'est touché.
4. Relancer : `sc start GescomServeur`. Les caisses se reconnectent.

Faites-le **une fois pour de vrai** sur une base d'essai, avant d'en
avoir besoin : une sauvegarde qu'on n'a jamais restaurée n'est pas une
sauvegarde.

## Vérifier et réparer

**Paramètres → Sauvegarde → État de la base → Vérifier la base**, après une coupure
de courant et **avant** de saisir quoi que ce soit. Saisir par-dessus
une base abîmée rend la sauvegarde inutile.

Si la base est saine, un second bouton apparaît : **Réparer et
compacter**. Il réaffecte les règlements fournisseur enregistrés
globalement sur les factures qu'ils couvrent, et réduit la taille du
fichier. Les montants ne changent pas, seule leur affectation. Une copie
de sécurité est faite avant.

---

# 13 bis. L'année et l'heure

## Les dates de travail (les exercices)

Chaque dossier écrit dans ses **dates de travail** : un exercice, qui
commence n'importe quel jour (juillet → juin se fait) et dure jusqu'à
cinq ans. Hors de ces dates, **plus aucune vente ne s'enregistre**.

**Le tableau de bord prévient** : un bandeau orange dans le dernier mois,
rouge les sept derniers jours. Le patron y a un bouton **Dates de
travail** ; les autres lisent « Prévenir le patron ».

**Paramètres → Dossiers**, bloc « Dates de travail » (le patron) :

- **Prolonger** : choisir la nouvelle fin, ou +1 / +2 / +3 ans.
  Une prolongation ne se raccourcit pas : vérifiez l'année.
- **Couper** : arrêter l'exercice en cours à un jour choisi ; le suivant
  s'ouvre le lendemain. Rien n'est déplacé : les rapports vont par dates.
- **Clore** : seulement un exercice **terminé**, et dans l'ordre. Pour
  s'arrêter plus tôt : couper, puis clore la partie terminée. Un
  exercice clos ne se rouvre pas.
- **Ouvrir l'exercice suivant.**

> La numérotation des factures suit l'année civile (`FAC-2026-…`), pas
> l'exercice : un exercice juillet → juin change de série au 1er janvier.

## L'horloge de l'ordinateur

Si l'heure de l'ordinateur qui garde la base **recule** de plus de
cinq minutes, **plus rien ne s'enregistre** (« HORLOGE_RECULE ») ; on
peut encore consulter. Une vente au mauvais jour fausserait la caisse.

- Le plus souvent : **remettre la date et l'heure de Windows** sur cet
  ordinateur.
- Si l'heure de Windows est juste (elle avait été **avancée** par
  erreur, puis corrigée) : le patron le confirme dans **Paramètres →
  Horloge → L'heure de Windows est juste**.

---

# 14. Ce qu'il ne faut pas faire

**Clôturer sans compter.** Recopier le solde théorique vide la clôture
de son sens. Vous ne verrez jamais un écart.

**Oublier les dépenses.** La caisse sera en excédent tous les soirs,
vous perdrez confiance dans le chiffre, et vous arrêterez de clôturer.

**Modifier une facture imprimée.** Elle est figée, et c'est voulu. Pour
corriger, émettez un avoir — il documente la correction au lieu de la
masquer.

**Partager les mots de passe.** En cas d'écart, plus moyen de savoir qui
était en caisse.

---

# 15. Problèmes courants

| Situation | Solution |
|---|---|
| « Une session est déjà ouverte » | La caisse d'hier n'a pas été clôturée |
| « Aucune session de caisse ouverte » | Ouvrir la caisse avant de saisir une dépense |
| « Stock insuffisant » sur un transfert | Vérifier le dépôt de départ |
| « Pièce payée — non modifiable » | Émettre un avoir |
| Bouton Encaisser absent | La facture est en brouillon : la valider d'abord |
| Le stock est faux | Stock → Ajuster, en indiquant le motif |
| Un montant semble faux | Journal, retrouver l'opération à l'heure près |
| L'application ne démarre pas | Redémarrer l'ordinateur, puis appeler |
| « Serveur injoignable » | Le service tourne-t-il (fenêtre Gescom Serveur → Démarrer le service) ? L'ordinateur principal est-il allumé, le Wi-Fi en « Privé » ? Paramètres → Réseau → Rechercher le serveur |
| La date saisie est refusée | Plus de 31 jours en arrière, dans le futur, le compte n'a pas la permission *antidater*, ou la date sort des dates de travail (13 bis) |
| « HORLOGE_RECULE » | L'heure de l'ordinateur du serveur a reculé : la remettre juste (13 bis) |
| « … est hors des dates de travail » | L'exercice est fini : le patron le prolonge (13 bis) |
| « Pas de crédit pour un client de passage » | Choisir ou créer le client avant de vendre à crédit |
| « … ne suit pas son stock : rien à transférer ni à compter » | Reprendre le suivi d'abord (Paramètres → Articles) |
| « Accorder un avoir » absent | Seul le patron a cette permission ; il la donne à la main dans Utilisateurs → Permissions |

---

# 16. En cas de doute

Chaque opération est enregistrée avec l'heure, le montant et l'auteur.
Rien ne disparaît, même annulé.

Le **Journal** est le premier endroit à regarder : il raconte la journée
dans l'ordre.
