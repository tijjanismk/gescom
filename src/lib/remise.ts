// lib/remise.ts — Calcul des remises sur lignes et facture globale

export interface LigneVente {
  article_id: string;
  unite_vente_id: string;
  depot_source_id: string;
  source_approvisionnement: string;
  article_nom: string;
  unite_libelle: string;
  quantite: number;
  facteur: number;
  prix_reference: number;
  prix_pratique: number;
  remise_pct: number;      // % remise sur la ligne (0-100)
  remise_montant: number;  // montant remise calculé
  taux_tva: number;
  montant_ht: number;      // après remise ligne
  montant_tva: number;
}

export interface TotauxVente {
  total_brut: number;       // avant toute remise
  total_remise_lignes: number;
  total_ht: number;         // après remises lignes
  remise_globale_pct: number;
  remise_globale_montant: number;
  total_net_ht: number;     // après remise globale
  total_tva: number;
  total_ttc: number;        // montant final à payer
}

/** Calcule la remise et le montant HT d'une ligne */
export function calculerLigne(
  prix_reference: number,
  quantite: number,
  remise_pct: number,
): { montant_brut: number; remise_montant: number; montant_ht: number; prix_pratique: number } {
  const montant_brut = Math.round(prix_reference * quantite);
  const remise_montant = Math.round(montant_brut * remise_pct / 100);
  const montant_ht = montant_brut - remise_montant;
  const prix_pratique = quantite > 0 ? Math.round(montant_ht / quantite) : prix_reference;
  return { montant_brut, remise_montant, montant_ht, prix_pratique };
}

/** Calcule tous les totaux d'une vente */
export function calculerTotaux(
  lignes: LigneVente[],
  remise_globale_pct: number,
): TotauxVente {
  const total_brut = lignes.reduce(
    (s, l) => s + Math.round(l.prix_reference * l.quantite), 0
  );
  const total_remise_lignes = lignes.reduce((s, l) => s + l.remise_montant, 0);
  const total_ht = lignes.reduce((s, l) => s + l.montant_ht, 0);

  const remise_globale_montant = Math.round(total_ht * remise_globale_pct / 100);
  const total_net_ht = total_ht - remise_globale_montant;

  const total_tva = lignes.reduce(
    (s, l) => s + Math.round(l.montant_ht * l.taux_tva), 0
  );
  const total_ttc = total_net_ht + total_tva;

  return {
    total_brut,
    total_remise_lignes,
    total_ht,
    remise_globale_pct,
    remise_globale_montant,
    total_net_ht,
    total_tva,
    total_ttc,
  };
}

/** Formate un montant FCFA */
export function fmt(n: number): string {
  return new Intl.NumberFormat("fr-ML").format(n);
}