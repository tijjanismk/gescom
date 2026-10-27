# Le duplicata — plan (26/10/2026, commencé)

Demande du propriétaire : « on peut coder les duplicata optionnels sur
mes pièces commerciales sous forme de filigrane ? ». Proposé, discuté,
**choix du propriétaire notés ci-dessous**. Plan en cinq lignes accepté
le 26/10 (« commence par la table impression et noter_impression »).

## Où on en est

1. ✅ **Noyau et serveur** (26/10) : table `impression`, trois commandes
   (`lire_etat_impression`, `noter_impression`, `reimprimer_original`),
   permission `pieces:reimprimer_original`, scénario
   `tests/impression_base.rs` — détail dans
   [modules/documents.md](modules/documents.md).
2. ✅ **La case par genre** (27/10) : `ReglageGenre::duplicata`, non
   par défaut, refusée sur relevé et bulletin (`GENRES_DUPLICATA`) ;
   case « Duplicata » dans Paramètres → Documents. Le serveur la lit :
   genre déduit (`coeur::documents::genre_imprime` — pièce →
   `genre_de_piece`, rouleau → ticket, reçu → recu), d'où le paramètre
   `format` des trois commandes. Case décochée : l'impression se compte
   quand même (cochée plus tard, la feuille déjà sortie reste
   l'original), la réponse dit `duplicata: false`, et
   `reimprimer_original` est refusé.
3. ✅ **Le rendu** (27/10) : `Habillage.duplicata = { originalLe }`,
   posé par l'appelant d'après le serveur. `documentHTML` : « DUPLICATA »
   en travers de chaque page (15 %, par-dessus le filigrane société, en
   `multiply`) et un cadre « DUPLICATA — original imprimé le … » en haut
   du corps ; le ticket : deux lignes sous le titre, sans filigrane.
   L'exemple de Paramètres → Documents montre la feuille réimprimée quand
   la case est cochée. Vu dans le navigateur : facture A4 sur une page,
   reçu A5 600 px sur 794 (la mention prend 33 px), ticket.
   **Corrigé en relisant** : une facture ou un avoir FOURNISSEUR ne se
   compte pas (`hors_compte`) — c'est sa pièce, pas la nôtre.
4. ✅ **Les chemins d'impression et le bouton du patron** (27/10) :
   [lib/duplicata.ts](../src/lib/duplicata.ts) — `lireEtatImpression`
   (aperçu), `noterImpression` (clic, ou l'original du patron),
   `marque`, `proposerOriginal`. `ApercuPiece` (et donc le ticket du POS)
   et `ApercuRecu` : l'aperçu montre l'état annoncé et dit « original
   imprimé le … » ; au clic, la feuille est **regénérée d'après le
   verdict** du serveur ; bouton « Réimprimer l'original » pour qui a la
   permission, quand la prochaine feuille serait un duplicata.
   `Pieces.tsx` et `FicheClient.tsx` (impression directe) : notent puis
   impriment. **Le serveur ne répond pas → on n'imprime pas** (une
   feuille sans marque serait un original de plus). Le bon de sortie ne
   se compte pas ; « facture + bon » se compte comme la facture.
   L'aperçu rattache l'état au format pour lequel il a été lu (A4 →
   ticket : pas de marque d'A4 sur le ticket) et grise les boutons
   tant qu'il n'est pas arrivé (~190 ms mesurées).
5. ✅ **Vérifié** (27/10) : serveur construit, lancé sur une base
   jetable, appelé par HTTP comme l'écran (`essai_duplicata.py`, 14/14 :
   paramètres, permission du patron refusée à l'employé, poste pris de
   la session, Historique, brouillon et facture fournisseur non
   comptés) ; puis l'écran dans le navigateur contre ce serveur :
   aperçu en duplicata, avertissement, bouton du patron, ticket sans
   marque. **Puis dans la vraie fenêtre** (27/10, banc `monoposte -Neuf`,
   le propriétaire à l'écran, boîte d'impression Windows) : original
   sans marque, duplicata marqué, original du patron sans marque et
   tracé dans l'Historique, ticket sans marque — **conforme**. Non
   rapporté : l'impression annulée (étape 7 du guide) et le reçu.
5. ☐ Le rendu vérifié dans Chrome.

**Écarts au plan, choisis en écrivant** : la table dit `objet_type`
(les noms du journal : `piece_commerciale`, `paiement`,
`paiement_fournisseur`) et non `genre`, qui veut déjà dire « facture,
devis… » dans `coeur/documents.rs` ; elle garde aussi `rang` (l'horloge
tique par 15 ms) et `poste_id`. L'original du patron est **une commande
à part** (`reimprimer_original`), pour que le serveur juge la permission
comme partout ailleurs. Un brouillon ne se compte pas. Une lecture
`lire_etat_impression` s'ajoute : l'aperçu doit montrer le duplicata
avant d'imprimer (règle du 17/09 : l'aperçu est ce qui part).

## Pourquoi

Une facture ou un reçu réimprimé est identique à l'original : une
seconde feuille « originale » sert à se faire rembourser deux fois, à
prouver un paiement déjà compté, à contester une dette. Une pièce
numérotée n'a qu'un original (usage comptable, OHADA).

## Décidé par le propriétaire (26/10/2026)

- **Documents** : les pièces A4/A5 (factures, devis, BL, avoirs…), les
  **reçus de règlement**, et le **ticket de caisse** (mention seulement,
  pas de filigrane sur un ticket).
- **Original réimprimé** : un bouton **« Réimprimer l'original »
  réservé au patron**, tracé dans l'Historique ; sinon, toute impression
  après la première sort en duplicata.
- **Optionnel** : une case par genre dans Paramètres → Documents.

## Proposé par l'assistant (à confirmer avec le plan détaillé)

1. **Le serveur décide** (comme D27) : une table `impression` (genre,
   objet_id — la pièce ou le paiement —, imprime_le, auteur_id,
   original, dossier_id ; cloisonnée, RLS, `schema.sql` + `amorcage.rs`)
   garde chaque impression ; la première est l'original, les suivantes
   des duplicatas, quel que soit le poste. L'aperçu ne compte pas. Une
   commande `noter_impression` (née sur `Base`) appelée au moment
   d'imprimer rend `{ duplicata, original_le }`.
2. **Deux marques** : le filigrane « DUPLICATA » en travers de la page
   **et** une mention nette près du titre (« DUPLICATA — original
   imprimé le 26/10/2026 ») — le filigrane est volontairement pâle
   (3–30 %) et peut disparaître à la photocopie ou si les fonds ne
   s'impriment pas. Le filigrane société (logo) reste ; « DUPLICATA »
   passe par-dessus, en travers.
3. **Le point délicat** : après la boîte d'impression Windows on ne sait
   pas si l'on a imprimé ou annulé — une première impression annulée
   fait sortir la suivante en duplicata. D'où le bouton du patron.
4. Chemins d'impression à couvrir : `Pieces.tsx::handleImprimer`,
   `FicheClient.tsx` (imprimer_piece), `ApercuPiece.tsx` (bouton
   imprimer, et le ticket du POS qui passe par lui), `ApercuRecu.tsx`.
   Le générateur : `impression.ts::documentHTML` (filigrane, mention),
   `genererPDF.ts` (pièces, ticket), `genererRecu.ts`.
5. Tests : un scénario `*_base` (première = original, deuxième =
   duplicata, autre poste = duplicata, original du patron tracé, refusé
   sans le droit, cloisonnement, détecteur) sur les trois moteurs ; le
   rendu vérifié dans Chrome (filigrane + mention, une seule page).
