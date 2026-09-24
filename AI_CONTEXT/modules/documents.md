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
| Habillage | [lib/impression.ts](../../src/lib/impression.ts) | en-tête, coordonnées, pied, signatures + cachet, montant en lettres |
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

`noyau/tests/documents_base.rs` (12 scénarios, deux moteurs), 11
unitaires dans `coeur::documents`, et le banc d'écran
(`outils/banc/`) : réglages par l'écran, rendu genre par genre,
aperçu au format réglé.
