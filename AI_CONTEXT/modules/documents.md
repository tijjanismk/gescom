# Module : documents imprimés (v3, chantier A — D17, D18)

Rôle : ce qui s'imprime — facture, devis, commande, livraison, reçu,
relevé, ticket, bons de sortie et d'échange. **Un générateur par
genre, une mise en page fixe** ; ce qui varie d'une boutique à
l'autre est un **réglage**, pas un dessin. L'atelier de modèles
(septembre 2026, 4 700 lignes) est **retiré** : `pages/Modeles.tsx`,
`lib/modeles/*`, `EditeurPiedPage`, `noyau/src/modeles.rs`, les images
« posées » et leurs tables (`modele_document`, `image_document`,
supprimées par migration idempotente, deux chemins).

## Où vit quoi

| Couche | Fichier | Rôle |
|---|---|---|
| Règle | [coeur/documents.rs](../../src-tauri/noyau/src/coeur/documents.rs) | 7 genres, défauts d'usine, validation, fusion, coordonnées |
| Stockage | [documents.rs](../../src-tauri/noyau/src/documents.rs) | `config_app` : `documents_reglages`, `documents_coordonnees`, `documents_signature_<genre>_<rang>` |
| Commandes | [serveur/src/socle.rs](../../src-tauri/serveur/src/socle.rs) | 6, **nées sur `Base`** (D22) |
| Types, chargement | [lib/documents.ts](../../src/lib/documents.ts) | miroir TS, `genreDePiece`, `choisirImage` |
| Habillage | [lib/impression.ts](../../src/lib/impression.ts) | la page (`documentHTML` : en-tête et pied répétés, filigrane), coordonnées, signatures + cachet, montant en lettres |
| Générateurs | `genererPDF.ts`, `genererRecu.ts`, `genererReleve.ts` | un par genre, lisent l'habillage |
| Exemple | [lib/exemples-documents.ts](../../src/lib/exemples-documents.ts) | données fictives pour l'aperçu des réglages |
| Écran | [OngletDocuments.tsx](../../src/components/OngletDocuments.tsx) | Paramètres → Documents |

**Le rendu reste à l'écran** (TypeScript) : une seule génération pour
l'aperçu et l'impression (règle du 17/09). **Le réglage est en base** :
le serveur le distribue à toutes les caisses.

## Les genres et leur type de pièce

`facture` (facture, acompte, avoirs, côté client et fournisseur),
`devis` (devis, proforma), `bon_commande` (commande client, BCF),
`bon_livraison` (BL, bon de réception — et les bons de sortie et
d'échange prennent ses signatures), `recu`, `releve`, `ticket`, `bulletin` (le bulletin de paie de Gescom Équipe, G-3 : A4 ou A5, jamais rouleau).
`genre_de_piece` / `genreDePiece` : même table des deux côtés.

## Un réglage

`format` (a4, a5, thermique_80, thermique_58 ; ticket : rouleau
seulement), `colonne_remise` / `colonne_tva` / `recap_tva`
(**auto** = selon le document, oui, non), `montant_lettres`,
`reference_article` (le `code_barre` de l'article, rendu par
`lire_donnees_piece` en `article_reference`), `mention` (vide = le
`pied_facture` de la société), `signatures` (0 à 3 libellés, ≤ 40
caractères ; ticket : aucune). Les **coordonnées** sous le nom (quand
l'en-tête n'est pas une image) se choisissent une fois pour tous les
documents.

## La page (25/09)

Tout document A4/A5 se monte par `impression::documentHTML(h, page)` :
`page` donne le titre, le papier (a4, a5), la marge du texte, la
mention de pied, « Imprimé le », les styles propres et le contenu.

- **En-tête et pied répétés sur chaque page, au bord du papier** :
  l'en-tête est le `<thead>` d'un tableau porteur, le pied un
  `position:fixed` réservé par sa copie invisible dans le `<tfoot>`.
  `@page { margin: 0 }` ; la marge est celle de la cellule.
- Les styles d'un document ne touchent pas le cadre (sélecteurs
  `table.cadre > … > td` plus précis qu'un `td` nu).
- À l'écran, `body { min-height: <une page> }` : le pied tombe au bas
  du papier comme à l'impression.
- Le **filigrane** (`blocFiligrane`) : image ou mot, `opacity`, `blur`,
  largeur en % de la page ; posé par-dessus en `mix-blend-mode:
  multiply`. Réglé une fois (`documents_filigrane`,
  `documents_filigrane_image` ≤ 1 Mo), retiré genre par genre
  (`ReglageGenre.filigrane`, absent = oui ; ticket : non).
- Les signatures (`blocSignatures`) : libellé **souligné**, place de
  signer dessous, cachet dedans. Plus de trait séparé.
- **La taille du texte** (26/10/2026) : le texte courant sort à **11 pt
  en A4, 10 pt en A5** (l'usage d'un document imprimé : 10–12 pt ; avant,
  ~8 pt et des détails à 7). `documentHTML` passe tout le document par
  `agrandirTexte(html, echelleTexte(papier, police))` : chaque
  `font-size: Npx` est multiplié (×1,33 pour une pièce A4 à 11 px), les
  cadres, les bandeaux images et les marges du papier non. **Pas de
  `zoom` CSS** : essayé d'abord, le moteur d'impression découpait mal le
  contenu zoomé — une facture A5 presque pleine sortait sur deux pages.
  Le ticket et le bon d'échange (`genererBonEchangeHTML`, sa propre page)
  n'y passent pas.
- Le **cadre du tiers** (`.doc-tiers`) : au moins 48 % de la largeur en
  A4 (52 % en A5), le nom en 14 px (12 en A5) avant agrandissement ; un
  nom très long élargit le cadre jusqu'à la page puis passe à la ligne,
  même sans espace (`overflow-wrap: anywhere`).
- **Jamais « Client de passage » sur le papier** (le propriétaire :
  « presque une insulte ») : `passage::tiers_a_imprimer` met le code
  (`PAS-000123`, il retrouve la facture) à la place du nom, sans le
  répéter dessous — `lire_donnees_piece` et le reçu (deux versions), et
  le bon d'échange côté écran. Vaut aussi pour une facture émise avant
  une promotion, dont le nom est figé.

Commandes : `enregistrer_filigrane_documents { reglage }`,
`poser_image_filigrane { image | null }` (`parametres:modifier`) ;
`lire_reglages_documents` rend `filigrane`.

## Règles

- [CONFIRMÉ] **Un réglage abîmé retombe sur l'usine** : une facture
  sort toujours (`un_reglage_abime_en_base_n_empeche_pas_d_imprimer`).
- [CONFIRMÉ] **Seuls les genres réglés sont enregistrés** ; les autres
  suivent l'usine, y compris quand une version l'améliore
  (`un_genre_jamais_regle_suit_l_usine`). « Réglages d'usine » retire
  le genre de l'objet enregistré et ses cachets.
- [CONFIRMÉ] **Les libellés v2** (`signature_facture_gauche`…) sont
  repris tant que le genre n'a pas été réglé ; une paire vidée en v2
  reste vide (`les_libelles_de_la_v2_passent_en_v3`).
- [CONFIRMÉ] **Un cachet part avec sa signature** : réduire le nombre
  de signatures efface les images des emplacements retirés.
- [CONFIRMÉ] Cachet : PNG, JPEG ou WebP, **512 ko** au plus, en data URL
  en base (D8 : le contenu voyage ; `pg_dump` l'emporte).
- [CONFIRMÉ] Le **reçu et le relevé côté fournisseur** retournent les
  libellés d'usine du client (« Le caissier » → « Le bénéficiaire »,
  « Le client » → « Le fournisseur ») ; un libellé choisi reste tel quel.
- [CONFIRMÉ] Le **récapitulatif TVA** donne base HT et TVA par taux,
  **pas** de TTC par taux : le TTC imprimé suit l'arrondi du prix
  unitaire de `valider_facture`, une somme par taux s'en écarterait
  d'un franc.
- [DÉDUIT] Sans réglage lisible côté écran (serveur injoignable pour
  cette lecture), le générateur imprime la mise en page d'usine avec
  les signatures historiques (`SIGNATURES_USINE`).

## Preuves

`noyau/tests/documents_base.rs` (14 scénarios, deux moteurs), 14
unitaires dans `coeur::documents`, et le banc d'écran
(`outils/banc/`) : réglages par l'écran, rendu genre par genre,
aperçu au format réglé.
