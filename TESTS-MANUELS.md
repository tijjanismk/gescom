# Gescom — Tests manuels

*Ce que les tests automatiques ne voient pas : un écran, une imprimante,
deux machines, un service Windows.* À dérouler avant chaque livraison,
sur une **base d'essai** (jamais celle du magasin). Cocher, noter la
date et la version en tête.

```
Version testée : ________   Date : ________   Testeur : ________
Serveur : SQLite ☐   PostgreSQL ☐      Machines : 1 ☐   2+ ☐
```

Les scénarios automatiques couvrent déjà stock, caisse et créances
après chaque geste (440 tests, deux moteurs). Ici, on vérifie ce qui
passe par les yeux et par le réseau.

---

## A. Le serveur en service Windows

*Sur une machine où le service n'est pas installé. En administrateur.*

| # | Faire | Attendu | ☐ |
|---|---|---|---|
| A1 | Lancer `Gescom-Serveur_x.y.z_x64-setup.exe` en administrateur | Assistant en français ; page « Configuration du serveur » avec base et port préremplis | ☐ |
| A2 | Garder les valeurs, terminer | Pas d'erreur ; page finale nomme le journal, la configuration et `sc stop/start` | ☐ |
| A3 | Ouvrir **Services** | *Gescom Serveur* : **En cours d'exécution**, démarrage **Automatique** | ☐ |
| A4 | Menu Démarrer → Gescom → *Journal du serveur* | Le fichier s'ouvre ; dernière section « service démarré », base, écoute `0.0.0.0:7300`, nombre de commandes, adresse à saisir sur les caisses | ☐ |
| A5 | Navigateur : `http://localhost:7300` | La console répond (postes, sauvegardes) | ☐ |
| A6 | `sc stop GescomServeur` puis `http://localhost:7300` | Le service passe **Arrêté** en moins de 5 s ; la page ne répond plus ; le journal dit « Arrêt demandé » | ☐ |
| A7 | `sc start GescomServeur` | Redémarre ; la console répond de nouveau | ☐ |
| A8 | Gestionnaire des tâches → tuer `gescom-serveur.exe` | Il **revient tout seul** dans les 10 s (relance sur incident) | ☐ |
| A9 | Redémarrer la machine **sans ouvrir de session** ; depuis une autre machine, `http://<ip>:7300/sante` | Répond : le service démarre avant toute session | ☐ |
| A10 | Pare-feu : `outils\parefeu.ps1 -Etat` | Règle « Gescom serveur » PRÉSENTE, profils Domaine + Privé, port 7300 | ☐ |
| A11 | Panneau de configuration → Certificats (certlm) → Autorités racines / Éditeurs approuvés | « Gescom (auto-signe) » présent | ☐ |
| A12 | Relancer le **même** installeur (mise à jour) | Pas de page de configuration (elle existe) ; le service est arrêté, remplacé, relancé ; `serveur.json` intact | ☐ |
| A13 | Éditer `serveur.json` (port 7301), `sc stop` + `sc start` | Écoute sur 7301 (journal) ; remettre 7300 ensuite | ☐ |
| A14 | Désinstaller (Programmes et fonctionnalités) | Service retiré, règle de pare-feu retirée ; `C:\ProgramData\Gescom` (base, journal, sauvegardes) **conservé** | ☐ |
| A15 | Avec PostgreSQL : base `postgresql://…/gescom_essai` à l'installation | Le journal dit « PostgreSQL » ; amorçage si base neuve ; `sc stop/start` sans erreur | ☐ |
| A16 | Mot de passe PostgreSQL faux dans `serveur.json` | Le service **s'arrête** avec un message clair dans le journal — il ne sert pas une base vide à la place | ☐ |

## B. Les caisses et le réseau

*Deux machines sur le même réseau. Aucune base locale sur la caisse.*

| # | Faire | Attendu | ☐ |
|---|---|---|---|
| B1 | Installer `Gescom_x.y.z_x64-setup.exe` sur la caisse, premier lancement | Écran de connexion ou invitation à régler le réseau — jamais une base vide qui « marche » | ☐ |
| B2 | Paramètres → Réseau → Poste, adresse du serveur, nom « Caisse 1 » | Connecté ; le poste apparaît dans la console du serveur | ☐ |
| B3 | Débrancher le câble de la caisse, tenter une vente | « Serveur injoignable », **rien n'est enregistré**, pas de repli silencieux | ☐ |
| B4 | Rebrancher | La caisse se reconnecte ; la vente peut se faire | ☐ |
| B5 | `sc stop GescomServeur` pendant qu'une caisse est ouverte | La caisse le dit ; après `sc start`, il faut se reconnecter (les sessions sont révoquées au redémarrage) | ☐ |
| B6 | Deux caisses vendent en même temps le même article | Deux numéros de facture différents, jamais de doublon ; stock décrémenté deux fois | ☐ |
| B7 | Console → révoquer la session de Caisse 1 | Caisse 1 est déconnectée à son prochain geste | ☐ |
| B8 | Désactiver le poste dans la console | Caisse 1 ne peut plus se connecter tant qu'il est désactivé | ☐ |

## C. Se connecter, les droits

| # | Faire | Attendu | ☐ |
|---|---|---|---|
| C1 | `admin` / `admin123` sur une base neuve | Changement de mot de passe **imposé** avant tout | ☐ |
| C2 | Mauvais mot de passe | Refus, sans dire lequel des deux est faux | ☐ |
| C3 | Se connecter en `employe` | Pas de prix d'achat, pas de rapports, pas de Paramètres → Utilisateurs | ☐ |
| C4 | Patron : Utilisateurs → Permissions → retirer *ventes:creer* à employe | L'employé ne voit plus Ventes dans le menu ni dans Ctrl+K ; un appel direct est refusé par le serveur | ☐ |
| C5 | Donner *Accorder un avoir sans marchandise* à une personne | Le bouton apparaît dans la fiche client pour elle seule | ☐ |
| C6 | Rôle créé à la main (Paramètres → Rôles) | Utilisable sans redémarrer quoi que ce soit | ☐ |

## D. La palette (Ctrl + K)

| # | Faire | Attendu | ☐ |
|---|---|---|---|
| D1 | Ctrl+K sur chaque page du menu | S'ouvre partout, fenêtre large, champ en haut | ☐ |
| D2 | Taper « cai » | *Caisse* en premier ; sur la page Caisse, *Ouvrir la caisse* ou *Fermer la caisse* selon l'état | ☐ |
| D3 | Taper « reg » (sans accent) | Trouve *Règlement* / *Réglages* : la recherche ignore les accents | ☐ |
| D4 | Taper deux lettres d'un nom de client | Groupe *Clients* (5 max) ; Entrée ouvre la fiche | ☐ |
| D5 | Idem avec un fournisseur | Groupe *Fournisseurs* ; Entrée ouvre la fiche | ☐ |
| D6 | Flèches ↓↑ puis Entrée ; Échap | Le curseur traverse les groupes ; Échap ferme | ☐ |
| D7 | Employé sans *clients:creer* | Le groupe *Clients* n'apparaît pas | ☐ |
| D8 | Aller sur Caisse, attendre le chargement, revenir | **Pas d'écran blanc** (régression du hook après le chargement) | ☐ |

## E. Vendre

| # | Faire | Attendu | ☐ |
|---|---|---|---|
| E1 | Vente comptant, caisse ouverte | Facture créée, stock −, caisse + ; Journal l'affiche à l'heure | ☐ |
| E2 | Vente comptant, caisse **fermée** | Refus « caisse fermée », rien n'est écrit | ☐ |
| E3 | Vente à crédit avec acompte | Créance = total − acompte ; l'acompte seul entre en caisse | ☐ |
| E4 | « Vente du » = avant-hier (patron) | La facture et la vente sont datées d'avant-hier ; le mouvement de caisse est **d'aujourd'hui** avec « Vente du jj/mm » | ☐ |
| E5 | « Vente du » = il y a 40 jours | Refus, message parle de 31 jours | ☐ |
| E6 | « Vente du » = demain | Refus « dans le futur » | ☐ |
| E7 | Même chose en `employe` (sans *antidater*) | Le champ n'est pas proposé ; un appel direct est refusé | ☐ |
| E8 | Article dans un autre magasin, quantité > stock local | Fenêtre « D'où sort la marchandise ? » ; chaque stock baisse au bon endroit | ☐ |

## F. Acheter, recevoir

| # | Faire | Attendu | ☐ |
|---|---|---|---|
| F1 | Réception comptant | Stock +, caisse −, FAF payée | ☐ |
| F2 | Réception à crédit | Dette dans Fournisseurs ; rien en caisse | ☐ |
| F3 | « Réception du » = il y a 3 jours | FAF et paiement datés d'il y a 3 jours ; stock et caisse **d'aujourd'hui**, libellé « Réception du jj/mm » | ☐ |
| F4 | Facturer deux fois le même bon de réception | Refus, nomme la facture existante | ☐ |
| F5 | Retour fournisseur en **avoir** | AVF *Émis*, reste = crédit en **ambre** ; la dette baisse | ☐ |
| F6 | Retour fournisseur **remboursé** | AVF *Payé*, reste **vide** ; entrée en caisse | ☐ |

## G. Les pièces

| # | Faire | Attendu | ☐ |
|---|---|---|---|
| G1 | Filtre « Commandes » → bouton | S'appelle *Nouvelle commande* ; la fenêtre n'a pas de sélecteur de type | ☐ |
| G2 | Filtre « Tout » → bouton | *Nouvelle pièce client* ; sélecteur présent | ☐ |
| G3 | Côté fournisseur, filtre « Bons cmd. » | *Nouveau bon de commande*, type figé | ☐ |
| G4 | Du → au dans la barre, côté fournisseur | Ne montre que la période ; vider revient à tout | ☐ |
| G5 | Nouvelle pièce, « Date de la pièce » = hier (patron) | La pièce est datée d'hier ; l'échéance est un autre champ | ☐ |
| G6 | Devis → Commande → Livraison + facture → Valider | Le stock ne sort **qu'une fois**, à la validation | ☐ |
| G7 | Paramètres → Irrécouvrable sur une vente à crédit | Dans Pièces : ligne **toute rouge**, badge *Irrécouvrable*, hors total, absente d'« impayés » et « en retard » | ☐ |
| G8 | Règlement ordinaire sur cette créance | Refus ; *Règlement exceptionnel* passe et encaisse | ☐ |
| G9 | Annuler une facture validée | Refus : passer par un avoir | ☐ |
| G10 | Fiche client → Avoirs → Accorder un avoir (patron) | AVC numérotée, crédit visible, journal ; client comptant refusé ; motif vide refusé | ☐ |
| G11 | Imprimer cet avoir | Une ligne « motif — montant », total = montant | ☐ |

## H. Les modèles de documents et l'impression

| # | Faire | Attendu | ☐ |
|---|---|---|---|
| H1 | Paramètres → Modèles de documents, choisir la facture | Aperçu avec données d'exemple, structure à gauche | ☐ |
| H2 | Glisser un bloc dans l'aperçu | Il tombe à l'endroit visé (trait de dépôt) | ☐ |
| H3 | Tirer le **coin bas-droit** d'un bloc du flux | Il se détache à sa place et se redimensionne dans le même geste ; « flottant » apparaît dans la structure | ☐ |
| H4 | Tirer un bloc flottant | Il se déplace ; on peut le poser **sur** un autre ; le plus bas dans la structure passe devant | ☐ |
| H5 | Cocher « flottant » dans les réglages | Le bloc garde sa place à l'écran (ne saute pas en haut à gauche) | ☐ |
| H6 | Enregistrer, ouvrir une vraie facture → œil | Le document est celui du modèle, blocs flottants compris | ☐ |
| H7 | **Imprimer sur papier** une facture A4 et un ticket 80 mm | Marges, blocs flottants, signatures en fin de dernière page — **jamais vérifié à la main jusqu'ici** | ☐ |
| H8 | Pièce fournisseur avec le modèle facture | Le champ dit « Fournisseur », pas « Client » | ☐ |
| H9 | Exporter les modèles → fichier ; Importer sur une **autre caisse** | Les modèles arrivent, images comprises ; le modèle actif de la caisse n'est pas changé | ☐ |
| H10 | Paramètres → Société | **Plus** de section « Signatures » (elles sont dans les modèles) | ☐ |

## I. La caisse

| # | Faire | Attendu | ☐ |
|---|---|---|---|
| I1 | Ouvrir avec un fond, vendre, dépense, clôturer en saisissant le vrai comptage | L'écart affiché est le bon ; Historique le garde | ☐ |
| I2 | Ouvrir deux fois | Refus « session déjà ouverte » | ☐ |
| I3 | Mode « caisse par utilisateur » (Paramètres → Ventes) | Chacun son tiroir ; l'un ne voit pas celui de l'autre | ☐ |

## J. Sauvegarde et restauration

| # | Faire | Attendu | ☐ |
|---|---|---|---|
| J1 | Console → Sauvegarder maintenant | Fichier daté dans le dossier réglé (`.db` ou dump PostgreSQL) | ☐ |
| J2 | Console → Entretien | Intégrité vérifiée, copie faite avant, compactage ; message de fin | ☐ |
| J3 | Restaurer la sauvegarde sur une base neuve, relancer le service | Les données reviennent ; les caisses se reconnectent | ☐ |
| J4 | Abîmer volontairement le fichier SQLite, `sc start` | Le service **refuse** de démarrer : « BASE ABIMEE … restaurer » dans le journal | ☐ |

---

## Ce qu'on note quand ça casse

Version, machine (serveur ou caisse), le geste exact, ce qui était
attendu, ce qui s'est passé, et les 20 dernières lignes de
`C:\ProgramData\Gescom\serveur.log`. Sans le journal, on cherche
deux fois plus longtemps.
