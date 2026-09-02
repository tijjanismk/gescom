# Gescom — Manuel d'utilisation

*Version 1.2 — à imprimer et garder près de la caisse.*

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
2. Chercher l'article, ajuster la quantité
3. Ajuster le prix ou saisir une **remise en %**
4. En haut à droite : **Comptant** ou **Crédit**
5. **Encaisser**

## Le prix et la remise

Le prix se remplit tout seul. Pour accorder une remise, saisissez le
pourcentage : le prix se recalcule et l'économie s'affiche.

Vous pouvez aussi taper directement le nouveau prix.

## Comptant ou crédit

**Comptant** — le client paie tout de suite.

**Crédit** — le client doit de l'argent. Vous pouvez saisir un
**acompte** : ce qu'il verse aujourd'hui.

## La marchandise est dans un autre dépôt

Sous la quantité, l'application indique où se trouve l'article :

```
🏬 5 ici · 30 à Djelibougou
```

Si vous demandez 20 et qu'il n'y en a que 5 sur place, une fenêtre
s'ouvre : **« D'où sort la marchandise ? »**

Indiquez combien vient de chaque endroit, envoyez quelqu'un chercher le
complément, puis validez au retour. Chaque stock baissera au bon
endroit.

## La facture

Après la vente, choisissez le format : A4, A5, ou ticket 58/80 mm.
**Passer** si le client n'en veut pas — la facture reste disponible dans
l'écran Pièces.

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

## Relancer

**Relances.** La liste des clients en retard, avec le nombre de jours.

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

## Régler une dette

**Fournisseurs** → fiche du fournisseur → **Régler**.

Vous pouvez imputer le paiement sur une facture précise, ou laisser
« Règlement global » — il s'imputera de la plus ancienne à la plus
récente.

---

# 7. Les retours

**Retours**, onglet **Retours**.

Trouvez la vente, choisissez la ligne et la quantité, puis :

| Mode | Effet |
|---|---|
| **Remboursement** | On rend l'argent |
| **Avoir** | Le client garde un crédit |
| **Échange** | Il repart avec autre chose |

> **Important** : si le client n'avait pas tout payé, le retour éteint
> d'abord sa dette. On ne lui rend que ce qu'il avait réellement versé.

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

> **Un chèque rejeté annule le paiement.** La créance du client se
> rouvre : il redoit l'argent.

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

---

# 10. Les dépôts

Si vous avez plusieurs lieux de stockage, un sélecteur apparaît en haut
à gauche.

**Tous les dépôts** — vue d'ensemble.
**Un dépôt précis** — le tableau de bord et le journal ne montrent que
lui, et les ventes en sortent.

> La caisse reste commune : il n'y a qu'un tiroir.

## Transférer

**Transferts.** Choisissez le départ, l'arrivée, les articles. Un bon
numéroté est créé, imprimable et signable par celui qui reçoit.

Un transfert n'est **ni une vente ni un achat** : votre chiffre
d'affaires ne bouge pas.

---

# 11. Le journal

**Journal.** Tout ce qui s'est passé dans la journée :

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
Nom;Categorie;Unite;Prix;Prix achat;TVA %;Code barre;Stock
Sucre;Alimentaire;kg;1500;1200;18;;50
```

Seuls le **nom** et le **prix** sont obligatoires. Une catégorie
inconnue est créée. Un article de même nom est mis à jour, jamais
dupliqué.

Les lignes incorrectes sont signalées une par une — les autres sont
importées.

## Codes-barres

**Paramètres → Codes-barres.** Bouton **Générer les manquants** pour
attribuer un code à tout le catalogue.

Un article qui a déjà un code fabricant le garde.

Cochez des articles et cliquez **Étiquettes** pour imprimer, quatre par
ligne.

---

# 13. Sauvegarder

**Paramètres → Sauvegarde**, chaque samedi soir, sur une clé USB.

> **Ne copiez jamais le fichier de base à la main.** Les écritures
> récentes vivent dans un fichier annexe : vous récupéreriez une base
> vide sans le savoir. Utilisez toujours le bouton.

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

---

# 16. En cas de doute

Chaque opération est enregistrée avec l'heure, le montant et l'auteur.
Rien ne disparaît, même annulé.

Le **Journal** est le premier endroit à regarder : il raconte la journée
dans l'ordre.
