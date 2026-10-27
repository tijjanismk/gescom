// lib/duplicata.ts — le duplicata (26/10/2026, PLAN-DUPLICATA) : ce que
// l'écran demande au serveur avant d'imprimer.
//
// L'écran ne décide jamais si une feuille est un original : deux postes
// qui impriment la même facture se croiraient chacun « premier ». Le
// serveur compte (`noyau::impression`) :
// - l'APERÇU lit l'état (`lire_etat_impression`) — n'écrit rien ;
// - le CLIC note l'impression (`noter_impression`) et c'est SA réponse
//   qui habille la feuille : un autre poste a pu imprimer entre-temps ;
// - le patron peut ressortir l'original (`reimprimer_original`), tracé
//   dans l'Historique.
//
// Si le serveur ne répond pas au clic, on n'imprime pas : une feuille
// sans marque serait un « original » de plus — exactement ce que le
// duplicata empêche.

import { appeler as invoke } from "@/lib/pont";
import { peut } from "@/lib/droits";
import type { Habillage } from "@/lib/impression";

/** Les noms du journal : ce qui s'imprime et se compte. */
export type ObjetImprime = "piece_commerciale" | "paiement" | "paiement_fournisseur";

/** Ce que l'aperçu montre avant d'imprimer. */
export interface EtatImpression {
  genre: string;
  /** La case « Duplicata » du genre (Paramètres → Documents). */
  actif: boolean;
  /** Faux pour un brouillon ou une pièce du fournisseur. */
  compte: boolean;
  impressions: number;
  duplicata: boolean;
  original_le: string | null;
}

/** Le verdict du clic, qui fait foi. */
export interface VerdictImpression {
  genre: string;
  actif: boolean;
  compte: boolean;
  duplicata: boolean;
  original_le: string | null;
  rang: number;
}

/** Le bon de sortie n'est pas la facture : il ne se compte pas. */
function seCompte(format?: string): boolean {
  return format !== "bon_sortie";
}

/** L'état, pour l'aperçu. `null` pour ce qui ne se compte pas. */
export async function lireEtatImpression(
  objetType: ObjetImprime, objetId: string, format?: string,
): Promise<EtatImpression | null> {
  if (!seCompte(format)) return null;
  return invoke<EtatImpression>("lire_etat_impression", { objetType, objetId, format: format ?? null });
}

/**
 * Au clic : note l'impression et rend le verdict. `original` : le bouton
 * du patron (refusé par le serveur sans `pieces:reimprimer_original`).
 * Une erreur remonte : l'appelant n'imprime pas.
 */
export async function noterImpression(
  objetType: ObjetImprime, objetId: string, format?: string, original = false,
): Promise<VerdictImpression | null> {
  if (!seCompte(format)) return null;
  return invoke<VerdictImpression>(original ? "reimprimer_original" : "noter_impression",
    { objetType, objetId, format: format ?? null });
}

/** La marque à poser sur l'habillage, d'après la réponse du serveur. */
export function marque(
  v: { duplicata: boolean; original_le: string | null } | null | undefined,
): Habillage["duplicata"] {
  return v?.duplicata && v.original_le ? { originalLe: v.original_le } : null;
}

/** Le bouton du patron : seulement quand la prochaine feuille serait un
 *  duplicata, et pour qui en a le droit (le serveur juge de toute façon). */
export function proposerOriginal(e: EtatImpression | null): boolean {
  return !!e?.duplicata && peut("pieces:reimprimer_original");
}

/** Le message d'un refus d'imprimer : on dit pourquoi, et quoi faire. */
export function refusImpression(e: unknown): string {
  return `Impression non comptée par le serveur : ${typeof e === "string" ? e : JSON.stringify(e)}. `
    + "Rien n'est parti à l'imprimante ; réessayer.";
}
