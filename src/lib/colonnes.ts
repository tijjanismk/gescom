// lib/colonnes.ts — les colonnes des pièces commerciales (D58).
//
// Le miroir de `coeur::documents` (COLONNES, COLONNES_FIXES,
// GENRES_A_COLONNES) et la règle qui dit ce que la saisie et le papier
// montrent. Une colonne n'apparaît que cochée : plus d'« auto ».
//
// Deux exceptions, qui ne cachent rien et n'inventent rien :
//   - la loi : une pièce dont aucune ligne ne porte de TVA n'imprime
//     jamais « HT », « TVA » ni « TTC » — ni colonne TVA, ni
//     récapitulatif, même cochés (le propriétaire, 01/11/2026) ;
//   - le garde-fou : cacher une colonne ne change jamais un montant.
//     Une remise cachée se dit sous la désignation, une TVA cachée se
//     voit dans les totaux.
//
// Pur et sans import d'alias : `node --test src/lib/colonnes.test.ts`.

export type Colonne =
  | "reference" | "designation" | "quantite" | "prix_unitaire"
  | "remise" | "tva" | "total";

/** Le catalogue, dans l'ordre du tableau — `coeur::documents::COLONNES`. */
export const COLONNES: readonly Colonne[] =
  ["reference", "designation", "quantite", "prix_unitaire", "remise", "tva", "total"];

/** Toujours là. L'unité suit la quantité (« 3 sacs »). */
export const COLONNES_FIXES: readonly Colonne[] =
  ["designation", "quantite", "prix_unitaire", "total"];

/** Celles qui se cochent. */
export const COLONNES_REGLABLES: readonly Colonne[] = ["reference", "remise", "tva"];

/** Les genres qui ont un tableau de lignes. */
export const GENRES_A_COLONNES: readonly string[] =
  ["facture", "devis", "bon_commande", "bon_livraison"];

export const LIBELLES_COLONNE: Record<Colonne, string> = {
  reference: "Référence",
  designation: "Désignation",
  quantite: "Quantité",
  prix_unitaire: "Prix unitaire",
  remise: "Remise %",
  tva: "TVA %",
  total: "Total",
};

/**
 * Une liste lue (réglage du genre, pièce, état de l'écran) remise en
 * ordre : connues seulement, fixes toujours, sans doublon. Absente :
 * les fixes seules, comme une base neuve.
 */
export function normaliserColonnes(choix: readonly string[] | null | undefined): Colonne[] {
  const c = new Set(choix ?? []);
  return COLONNES.filter(x => c.has(x) || COLONNES_FIXES.includes(x));
}

/** Coche ou décoche une colonne réglable ; une fixe ne bouge pas. */
export function basculer(colonnes: readonly Colonne[], c: Colonne, montrer: boolean): Colonne[] {
  if (COLONNES_FIXES.includes(c)) return normaliserColonnes(colonnes);
  const reste = colonnes.filter(x => x !== c);
  return normaliserColonnes(montrer ? [...reste, c] : reste);
}

/** Les colonnes du tableau de saisie, dans l'ordre, avec leur poids. */
const POIDS_SAISIE: readonly [string, number][] = [
  ["reference", 9], ["designation", 28], ["unite", 10], ["quantite", 9], ["prix_unitaire", 13],
  ["remise", 8], ["tva", 8], ["total", 12], ["action", 4],
];

/**
 * La largeur de chaque colonne VISIBLE du tableau de saisie, en %, qui
 * font 100 ensemble. Des largeurs fixes écrites pour le tableau complet
 * débordaient (104 %) et, une colonne cachée, le navigateur répartissait
 * le reste à sa façon : en-têtes et champs ne tombaient plus en face.
 * L'unité et la colonne du bouton « retirer » sont toujours là.
 */
export function largeursSaisie(colonnes: readonly string[]): { cle: string; largeur: string }[] {
  const c = normaliserColonnes(colonnes);
  const visibles = POIDS_SAISIE.filter(([cle]) =>
    cle === "unite" || cle === "action" || c.includes(cle as Colonne));
  const total = visibles.reduce((s, [, p]) => s + p, 0);
  return visibles.map(([cle, p]) => ({ cle, largeur: `${(p * 100 / total).toFixed(2)}%` }));
}

export interface LigneMontant {
  taux_tva?: number | null;
  remise_pct?: number | null;
}

const aTva = (l: LigneMontant) => (Number(l.taux_tva) || 0) > 0;
const aRemise = (l: LigneMontant) => (Number(l.remise_pct) || 0) > 0;

export interface ChoixImpression {
  reference: boolean;
  remise: boolean;
  tva: boolean;
  recapTva: boolean;
  /** « Remise 10 % » sous la désignation : colonne cachée, ligne remisée. */
  remiseSousDesignation: boolean;
  /** Une ligne porte une TVA : Montant HT, Total HT, TVA, TOTAL TTC. */
  documentATva: boolean;
}

/**
 * Ce que le papier montre. Les colonnes viennent de la pièce si elle en
 * garde, sinon du réglage de son genre.
 */
export function choixImpression(
  colonnes: readonly string[] | null | undefined,
  recapTva: boolean,
  lignes: readonly LigneMontant[],
): ChoixImpression {
  const c = normaliserColonnes(colonnes);
  const documentATva = lignes.some(aTva);
  const remise = c.includes("remise");
  return {
    reference: c.includes("reference"),
    remise,
    // La loi d'abord : pas de TVA facturée, pas un mot de TVA.
    tva: c.includes("tva") && documentATva,
    recapTva: recapTva && documentATva,
    remiseSousDesignation: !remise,
    documentATva,
  };
}

export interface AlertesSaisie {
  /** Lignes qui portent une TVA que la colonne cachée ne montre pas. */
  tvaCachee: number;
  /** Lignes qui portent une remise que la colonne cachée ne montre pas. */
  remiseCachee: number;
}

/** Ce que l'écran de saisie doit dire avant d'enregistrer. */
export function alertesSaisie(
  colonnes: readonly string[],
  lignes: readonly LigneMontant[],
): AlertesSaisie {
  const c = normaliserColonnes(colonnes);
  return {
    tvaCachee: c.includes("tva") ? 0 : lignes.filter(aTva).length,
    remiseCachee: c.includes("remise") ? 0 : lignes.filter(aRemise).length,
  };
}

export interface LigneSaisie extends LigneMontant {
  quantite: number;
  prix_unitaire: number;
}

/**
 * Les totaux que la saisie montre, avec la TVA même quand sa colonne
 * est cachée : la même suite que `pieces::lire_donnees_piece` (donc que
 * le papier et la créance) — remise de ligne, remise globale par ligne,
 * TVA, prix unitaire TTC arrondi, somme tronquée. Un franc d'écart ici
 * ferait mentir l'écran sur ce que le client devra.
 */
export function totauxSaisie(lignes: readonly LigneSaisie[], remiseGlobalePct: number) {
  let totalTva = 0;
  let sommeTtc = 0;
  const hts = lignes.map(l => {
    const brut = Math.round(l.prix_unitaire * l.quantite);
    return brut - Math.round(brut * (Number(l.remise_pct) || 0) / 100);
  });
  const totalHt = hts.reduce((s, x) => s + x, 0);
  lignes.forEach((l, i) => {
    const ht = hts[i] - Math.round(hts[i] * remiseGlobalePct / 100);
    const tva = Math.round(ht * (Number(l.taux_tva) || 0));
    totalTva += tva;
    const ttc = ht + tva;
    const puTtc = l.quantite > 0 ? Math.round(ttc / l.quantite) : ttc;
    sommeTtc += puTtc * l.quantite;
  });
  const remiseMontant = Math.round(totalHt * remiseGlobalePct / 100);
  return {
    totalHt,
    remiseMontant,
    totalNet: totalHt - remiseMontant,
    totalTva,
    totalTtc: Math.trunc(sommeTtc),
  };
}
