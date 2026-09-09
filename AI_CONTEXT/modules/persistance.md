# Module : persistance

Rôle : ouvrir la base, poser le schéma, appliquer les migrations, écrire
le journal. Aucune règle métier — sauf les cinq contrôles d'anomalies.

## Fichiers

- [persistance/mod.rs](../../src-tauri/src/persistance/mod.rs) (371 l.) —
  ouverture, diagnostic, migrations, entretien.
- [persistance/schema.sql](../../src-tauri/src/persistance/schema.sql)
  (470 l.) — 27 `CREATE TABLE`, régénéré depuis une base réelle.
- [persistance/journal.rs](../../src-tauri/src/persistance/journal.rs)
  (30 l.) — journal append-only.

## Fonctions exposées

- `ouvrir_base(chemin) -> Result<Connection>` — pose
  `PRAGMA journal_mode=WAL` et `foreign_keys=ON`.
- `initialiser_tables(&conn) -> Result<()>` — exécute `schema.sql`, puis
  ~30 `ALTER TABLE … ` et `CREATE TABLE IF NOT EXISTS` idempotents.
- `verifier_integrite(&conn) -> Result<Option<String>>` —
  `PRAGMA quick_check(1)`. `None` = sain.
- `anomalies_metier(&conn) -> Vec<(String, i64)>` — cinq requêtes de
  cohérence, vide = base saine.
- `entretenir(&conn, copie_avant) -> Result<u64>` — `VACUUM INTO` puis
  `REINDEX; VACUUM;`, renvoie la taille finale en octets.
- `journal::ecrire_evenement(conn, type, entite_type, entite_id, auteur, ancien, nouveau, origine)`
  — ne lève jamais d'erreur bloquante.

## Tables

```
role · utilisateur · utilisateur_auth · config_app · parametres_societe
categorie · article · unite_vente · depot · stock_depot
client · fournisseur
vente · ligne_vente · facture(legacy) · paiement
piece_commerciale · ligne_piece
session_caisse · mouvement_caisse
mouvement_stock · transfert · retour · avoir
paiement_fournisseur · creance_irrecouvrable · relance_creance
journal
```

Créées hors `schema.sql`, dans les migrations de `mod.rs` :
`cheque_recu`, et les recréations idempotentes de `piece_commerciale`,
`ligne_piece`, `paiement_fournisseur`, `creance_irrecouvrable`,
`relance_creance`.

## Entrant

`lib.rs` (au démarrage), `commandes/parametres.rs`
(`diagnostiquer_base`, `entretenir_base`), `commandes/sauvegarde.rs`.
`journal::ecrire_evenement` est appelé depuis presque toutes les
commandes d'écriture.

## Règles métier

- [CONFIRMÉ] **Un `execute_batch` par `CREATE TABLE`** — les regrouper
  provoquait un stack overflow (commentaire dans le fichier, et D26).
- [CONFIRMÉ] Les migrations sont des `.ok()` silencieux : rejouer
  `initialiser_tables` sur une base à jour ne fait rien. Corollaire —
  **une migration qui échoue vraiment ne se voit pas**.
- [CONFIRMÉ] `quick_check` et non `integrity_check` : dix fois plus
  rapide, et suffit pour le cas réel (base tronquée par une coupure de
  courant). Ne répare rien, dit de restaurer.
- [CONFIRMÉ] Le chemin de `VACUUM INTO` est un **paramètre lié**, jamais
  interpolé — une apostrophe dans un nom d'utilisateur Windows cassait
  la requête. Même piège signalé dans `sauvegarde.rs`.
- [CONFIRMÉ] `VACUUM INTO` produit une copie **cohérente**, WAL inclus.
  Une copie de fichier à la main donne une base amputée des écritures
  récentes (piège documenté dans MANUEL.md §13).
- [DÉDUIT] `facture` est une table legacy (numéros `GESCOM-…`) : plus
  alimentée, conservée pour l'historique. À ne pas confondre avec
  `piece_commerciale`, qui porte les factures actuelles.

## Colonnes ajoutées par migration — à connaître

Elles n'apparaissent pas dans `schema.sql`, seulement dans `mod.rs` :

| Table | Colonne | Pourquoi |
|---|---|---|
| `vente` | `piece_id` | facture POS auto (D16) |
| `mouvement_stock` | `fournisseur_id`, `prix_achat_unitaire` | sans quoi tout l'historique se recalcule au dernier prix connu |
| `paiement_fournisseur` | `piece_id`, `annule_paiement_id` | imputation écrite ; contestation |
| `paiement` | `annule_paiement_id` | contestation côté client |
| `avoir` | `piece_id` | lien avoir → AVC (D44) |
| `ligne_piece` | `quantite_livree` | suivi de livraison, **informatif** (D49) |
| `unite_vente` | `code_barre` | le carton a son propre EAN (D45) |
| `mouvement_caisse` | `libelle`, `categorie` | dépenses libres |
| `transfert` | `bon`, `unite_vente_id`, `motif` | bons BTR (v1.2) |
| `retour` | `ligne_vente_id` | une vente répartie sur 2 dépôts crée 2 lignes du même article (v1.3) |
