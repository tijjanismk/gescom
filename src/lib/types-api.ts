// lib/types-api.ts
//
// Formes RÉELLEMENT renvoyées par les commandes Tauri.
//
// Pourquoi ce fichier : les commandes renvoient du `serde_json::Value`,
// donc TypeScript ne peut rien vérifier tout seul. Une annotation
// `invoke<Truc>(...)` est une DÉCLARATION D'INTENTION, pas une garantie.
// Trois bugs sont nés de cet écart :
//
//   1. `lire_taux_tva` renvoie `taux_tva`, le POS lisait `taux`
//      -> TVA à 0 sur toutes les ventes, pendant des mois
//   2. `lire_creances_ouvertes` renvoie `id`, la fiche client lisait
//      `vente_id` -> "invalid args venteId"
//   3. `creer_fournisseur` renvoie {id, nom}, Achats.tsx l'annotait
//      `<string>` -> "invalid type: map, expected a string"
//
// RÈGLE : toute commande appelée depuis le front déclare son type ici,
// et `invoke` l'utilise. Jamais d'annotation inline, jamais de `any`
// sur un résultat d'invoke.
//
// En cas de doute, la source de vérité est le `serde_json::json!` de la
// commande Rust — pas ce fichier. Si les deux divergent, corriger ici.

// =====================================================================
//  Ventes & clients
// =====================================================================

export interface ClientApi {
  id: string;
  nom: string;
  code: string;
  telephone?: string | null;
  adresse?: string | null;
  nif?: string | null;
}

export interface UniteApi {
  id: string;
  libelle: string;
  facteur: number;
  prix_reference: number;
}

/** `lire_articles_avec_unites` — fournit taux_tva_defaut nativement (D19). */
export interface ArticleApi {
  id: string;
  nom: string;
  unite_base: string;
  stock: number;
  taux_tva_defaut: number;
  unites: UniteApi[];
  /** Présent uniquement si role = "patron" (§7). */
  dernier_prix_achat?: number | null;
}

/** `creer_vente` — le règlement est inclus dans la même transaction. */
export interface CreerVenteResultat {
  vente_id: string;
  total: number;
  total_regle: number;
  reste: number;
  statut: "payee" | "partiellement_payee" | "creance_ouverte";
}

/** `lire_taux_tva` — ⚠️ la clé est `taux_tva`, PAS `taux`. */
export interface TauxTvaApi {
  id: string;
  nom: string;
  unite_base: string;
  taux_tva: number;
}

// =====================================================================
//  Créances
// =====================================================================

/**
 * `lire_creances_ouvertes`
 * ⚠️ `id` ET `vente_id` désignent la même vente. `vente_id` a été ajouté
 * pour les appelants qui l'attendaient ; ne pas supprimer `id`, d'autres
 * écrans l'utilisent.
 */
export interface CreanceOuverteApi {
  id: string;
  vente_id: string;
  date_vente: string;
  statut: string;
  client_id: string;
  client_nom: string;
  client_code: string;
  telephone?: string | null;
  numero_facture?: string | null;
  total: number;
  total_paye: number;
  reste: number;
}

/** `lire_creances_relances` — ici la clé est bien `vente_id`. */
export interface CreanceRelanceApi {
  vente_id: string;
  date_vente: string;
  statut: string;
  client_id: string;
  client_nom: string;
  client_code: string;
  telephone?: string | null;
  facture_num?: string | null;
  total: number;
  total_paye: number;
  reste: number;
  jours_retard: number;
  nb_relances: number;
  derniere_relance?: string | null;
}

// =====================================================================
//  Fournisseurs
// =====================================================================

/** `creer_fournisseur` — ⚠️ renvoie un OBJET, pas une chaîne. */
export interface CreerFournisseurResultat {
  id: string;
  nom: string;
}

/**
 * `lire_fournisseurs_pagines` et `lire_fournisseurs_avec_dettes`.
 * La dette vient des factures fournisseur (FAF), plus du stock (D9).
 */
export interface FournisseurApi {
  id: string;
  nom: string;
  telephone?: string | null;
  adresse?: string | null;
  nif?: string | null;
  est_voisin: boolean;
  total_achats: number;
  total_paye: number;
  dette: number;
  nb_achats?: number;
}

/** `enregistrer_achat` — une facture FAF par achat avec fournisseur. */
export interface EnregistrerAchatResultat {
  piece_id: string | null;
  numero: string | null;
  total: number;
  statut: "paye" | "emis";
}

/** Ligne envoyée à `enregistrer_achat`. Clés en snake_case (serde). */
export interface LigneAchatEnvoi {
  article_id: string;
  unite_vente_id: string;
  quantite: number;
  facteur: number;
  prix_achat: number;
}

// =====================================================================
//  Retours & avoirs
// =====================================================================

/** `enregistrer_retour` — renvoie un objet depuis la création des AVC. */
export interface EnregistrerRetourResultat {
  retour_id: string;
  montant_credit: number;
  /** Numéro de la pièce AVC, si un avoir a été créé. */
  numero_avoir: string | null;
}

export interface AvoirApi {
  id: string;
  montant: number;
  cree_le: string;
}

// =====================================================================
//  Caisse
// =====================================================================

/**
 * `lire_resume_caisse`
 * ⚠️ `total_*` = tous moyens (indicateurs d'activité).
 *    `*_especes` = tiroir physique, base du rapprochement (D29).
 *    `solde_theorique` est calculé sur les ESPÈCES uniquement.
 */
export interface ResumeCaisseApi {
  session_id: string | null;
  statut: "ouverte" | "fermee" | "aucune";
  fond_ouverture: number;
  total_entrees: number;
  total_sorties: number;
  entrees_especes: number;
  sorties_especes: number;
  solde_theorique: number;
  nb_transactions: number;
  ouvert_le: string | null;
}

export interface MouvementCaisseApi {
  id: string;
  sens: "entree" | "sortie";
  moyen: string;
  montant: number;
  /** vente | achat | reglement_fournisseur | remboursement | ouverture | ... */
  motif: string;
  date_mouvement: string;
}

// =====================================================================
//  Pièces commerciales
// =====================================================================

export type TypePiece =
  | "devis" | "proforma" | "commande_client" | "bon_livraison"
  | "facture" | "facture_acompte" | "avoir_client"
  | "bon_commande_fournisseur" | "bon_reception"
  | "facture_fournisseur" | "avoir_fournisseur";

export type StatutPiece =
  | "brouillon" | "emis" | "accepte" | "transfere"
  | "validee" | "paye" | "annule";

export interface LignePieceApi {
  article_nom: string;
  unite_libelle: string;
  quantite: number;
  prix_unitaire: number;
  remise_pct: number;
  remise_montant: number;
  taux_tva: number;
  montant_tva: number;
  montant_ht: number;
}

/** `lire_donnees_piece` — source de l'impression écran Pièces. */
export interface DonneesPieceApi {
  piece: {
    numero: string;
    type_piece: TypePiece;
    statut: StatutPiece;
    date_piece: string;
    date_echeance?: string | null;
    remise_globale: number;
    note?: string | null;
    tiers_type: "client" | "fournisseur";
    client_nom: string;
    client_code?: string | null;
    client_telephone?: string | null;
    client_adresse?: string | null;
    client_nif?: string | null;
  };
  lignes: LignePieceApi[];
  societe: SocieteApi;
  totaux: {
    total_ht: number;
    total_tva: number;
    total_net: number;
    total_ttc: number;
    remise_montant: number;
  };
}

// =====================================================================
//  Société & impression POS
// =====================================================================

export interface SocieteApi {
  nom: string;
  adresse?: string | null;
  telephone?: string | null;
  telephone2?: string | null;
  email?: string | null;
  nif?: string | null;
  rccm?: string | null;
  pied_facture?: string | null;
  devise: string;
}

/**
 * `lire_donnees_facture` — source de l'impression POS.
 * ⚠️ `prix_pratique` et `montant` sont en TTC (D8 : le prix stocké est
 *    ce que le client paie). `prix_unitaire_ht` et `montant_ht` sont les
 *    valeurs à afficher quand `a_tva` est vrai.
 */
export interface LigneFactureApi {
  article_nom: string;
  unite_libelle: string;
  quantite: number;
  prix_pratique: number;
  prix_reference: number;
  montant: number;
  taux_tva: number;
  montant_tva: number;
  montant_ht: number;
  prix_unitaire_ht: number;
}

export interface DonneesFactureApi {
  societe: SocieteApi;
  vente: {
    id: string;
    date_vente: string;
    statut: string;
    mode_reglement: string;
    client_nom: string;
    client_code: string;
    client_telephone?: string | null;
    client_adresse?: string | null;
    client_nif?: string | null;
    numero_facture?: string | null;
  };
  lignes: LigneFactureApi[];
  paiements: { montant: number; mode: string; date_paiement: string }[];
  /** TTC — montant dû. */
  total: number;
  total_ht: number;
  total_tva: number;
  a_tva: boolean;
  total_paye: number;
  reste: number;
}

// =====================================================================
//  Dashboard
// =====================================================================

/**
 * `lire_resume_dashboard`
 * ⚠️ Les noms diffèrent de ceux qu'annonçait ARCHITECTURE.md :
 *    ca_mois_precedent (pas ca_hier), total_creances (pas creances_ouvertes),
 *    factures_brouillon (pas factures_a_valider), caisse_solde.
 */
export interface ResumeDashboardApi {
  ca_jour: number;
  ca_semaine: number;
  ca_mois: number;
  ca_mois_precedent: number;
  nb_ventes_jour: number;
  nb_ventes_mois: number;
  total_creances: number;
  nb_creances_ouvertes: number;
  nb_creances_en_retard: number;
  total_avoirs_ouverts: number;
  stock_ruptures: number;
  stock_alertes: number;
  caisse_solde: number;
  caisse_session_ouverte: boolean;
  factures_brouillon: number;
  commandes_en_attente: number;
}

// =====================================================================
//  Enveloppe de pagination
// =====================================================================

export interface Pagine<T> {
  donnees: T[];
  total: number;
  pages: number;
  page: number;
}
