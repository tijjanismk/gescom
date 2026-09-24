// lib/impression.ts — ce que tous les documents imprimés partagent (v3, A-2).
//
// Un seul générateur par genre (D17), mais un seul HABILLAGE pour tous :
// l'en-tête (image, ou nom + coordonnées cochées), le pied (image, ou la
// mention), les signatures avec leur cachet (D18), le montant en
// lettres. Les écrire une fois ici, c'est ce qui fait que la facture, le
// reçu et le relevé d'une même boutique se ressemblent.

import { appeler as invoke } from "@/lib/pont";
import {
  type Coordonnee, type Genre, type ReglageGenre, type ReglagesDocuments,
  type SignatureDoc, lireReglagesDocuments,
} from "@/lib/documents";

/** Ce qui habille un document : les images de la société et le réglage de son genre. */
export interface Habillage {
  logo?: string | null;
  entete?: string | null;
  pied?: string | null;
  reglage?: ReglageGenre | null;
  coordonnees?: Coordonnee[] | null;
}

export const COORDONNEES_HISTORIQUES: Coordonnee[] = [
  "adresse", "telephone", "telephone2", "nif", "rccm",
];

/** Charge tout ce qu'il faut pour habiller un document de ce genre. */
export async function chargerHabillage(genre: Genre): Promise<Habillage & { reglages: ReglagesDocuments | null }> {
  const [logo, entete, pied, reglages] = await Promise.all([
    invoke<string | null>("lire_logo_base64").catch(() => null),
    invoke<string | null>("lire_entete_base64").catch(() => null),
    invoke<string | null>("lire_pied_base64").catch(() => null),
    // Un réglage illisible ne doit jamais empêcher d'imprimer : le
    // générateur retombe alors sur sa mise en page d'usine.
    lireReglagesDocuments().catch(() => null),
  ]);
  return {
    logo, entete, pied, reglages,
    reglage: reglages?.genres[genre] ?? null,
    coordonnees: reglages?.coordonnees ?? null,
  };
}

/** Le même habillage, pour un autre genre (bon de sortie d'une facture…). */
export function pourGenre(
  h: Habillage & { reglages?: ReglagesDocuments | null },
  genre: Genre,
): Habillage {
  return { ...h, reglage: h.reglages?.genres[genre] ?? h.reglage ?? null };
}

export function esc(s: unknown): string {
  return String(s ?? "")
    .replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

/** Auto / toujours / jamais, contre ce que le document contient. */
export function choisir(choix: string | undefined, siAuto: boolean): boolean {
  if (choix === "oui") return true;
  if (choix === "non") return false;
  return siAuto;
}

// =====================================================================
//  En-tête
// =====================================================================

/**
 * Le bloc société, quand il n'y a PAS d'image d'en-tête : logo, nom
 * (toujours), puis les coordonnées cochées, dans l'ordre du catalogue.
 */
export function blocSociete(
  societe: Record<string, unknown>,
  h: Habillage,
  tailleNom = 17,
): string {
  const coord = h.coordonnees ?? COORDONNEES_HISTORIQUES;
  const lignes: string[] = [];
  const a = (k: Coordonnee) => coord.includes(k) && societe[k];
  if (a("adresse")) lignes.push(esc(societe.adresse));
  const tels = [a("telephone") ? societe.telephone : null, a("telephone2") ? societe.telephone2 : null]
    .filter(Boolean).map(esc);
  if (tels.length) lignes.push(`Tél : ${tels.join(" / ")}`);
  if (a("email")) lignes.push(esc(societe.email));
  if (a("site_web")) lignes.push(esc(societe.site_web));
  if (a("nif")) lignes.push(`NIF : ${esc(societe.nif)}`);
  if (a("rccm")) lignes.push(`RCCM : ${esc(societe.rccm)}`);
  return `
    ${h.logo ? `<img src="${h.logo}" alt="Logo"
         style="max-height:55px;max-width:180px;object-fit:contain;display:block"/>` : ""}
    <div class="doc-soc" style="font-size:${tailleNom}px;font-weight:bold;margin-top:4px">${esc(societe.nom)}</div>
    ${lignes.map(l => `<div class="doc-coord" style="font-size:10px;color:#555">${l}</div>`).join("")}`;
}

/** L'image d'en-tête, pleine largeur, au bord de la page. */
export function imageEntete(h: Habillage): string {
  return h.entete
    ? `<img class="doc-entete" src="${h.entete}" alt=""
            style="width:100%;height:auto;display:block;margin-bottom:12px"/>`
    : "";
}

/** L'image de pied, pleine largeur, en bas de la page. */
export function imagePied(h: Habillage): string {
  return h.pied
    ? `<img class="doc-pied" src="${h.pied}" alt="" style="width:100%;height:auto;display:block"/>`
    : "";
}

/**
 * La mention de bas de page, quand il n'y a pas d'image de pied : celle
 * du réglage, sinon celle qu'on donne (le pied de facture de la société).
 * Interpolée BRUTE : le commerçant peut y mettre `<b>` ou `<br>`.
 */
export function mention(h: Habillage, defaut: string): string {
  return h.reglage?.mention?.trim() || defaut;
}

// =====================================================================
//  Signatures (D18)
// =====================================================================

/**
 * Jusqu'à trois emplacements : le libellé, l'image (cachet, signature
 * scannée) au-dessus du trait, le trait. Rien si aucune signature.
 *
 * `page-break-inside:avoid` : un trait orphelin en haut de page ne se
 * signe pas — le bloc part en entier.
 */
export function blocSignatures(signatures: SignatureDoc[] | undefined | null): string {
  const s = (signatures ?? []).filter(x => x.libelle?.trim());
  if (s.length === 0) return "";
  const largeur = s.length === 1 ? "45%" : s.length === 2 ? "45%" : "30%";
  const une = (x: SignatureDoc) => `
    <div class="doc-sig" style="width:${largeur}">
      <div style="font-size:10px;color:#333">${esc(x.libelle)}</div>
      <div style="height:22mm;display:flex;align-items:flex-end;justify-content:center">
        ${x.image ? `<img src="${x.image}" alt="" style="max-height:21mm;max-width:100%;object-fit:contain"/>` : ""}
      </div>
      <div style="border-bottom:1px solid #000"></div>
    </div>`;
  return `
    <div class="doc-signatures" style="display:flex;justify-content:${s.length === 1 ? "flex-end" : "space-between"};
                margin-top:14px;padding-top:4px;page-break-inside:avoid">
      ${s.map(une).join("")}
    </div>`;
}

// =====================================================================
//  Montant en lettres
// =====================================================================

/**
 * Le montant en toutes lettres.
 *
 * Sur un reçu, c'est ce qui empêche de transformer 5 000 en 50 000 d'un
 * coup de stylo. Le franc CFA n'a pas de centime : pas de décimales à
 * écrire.
 */
export function enLettres(n: number): string {
  if (n === 0) return "zéro";
  if (n < 0) return "moins " + enLettres(-n);

  const U = ["", "un", "deux", "trois", "quatre", "cinq", "six", "sept",
             "huit", "neuf", "dix", "onze", "douze", "treize", "quatorze",
             "quinze", "seize"];
  const D: Record<number, string> = {
    2: "vingt", 3: "trente", 4: "quarante", 5: "cinquante",
    6: "soixante", 8: "quatre-vingt",
  };

  const sousCent = (x: number): string => {
    if (x < 17) return U[x];
    if (x < 20) return "dix-" + U[x - 10];
    const d = Math.floor(x / 10), u = x % 10;
    // 70 et 90 se disent « soixante-dix » et « quatre-vingt-dix ».
    if (d === 7 || d === 9) {
      const base = d === 7 ? "soixante" : "quatre-vingt";
      const reste = x - (d === 7 ? 60 : 80);
      return base + (reste === 11 && d === 7 ? "-et-onze" : "-" + sousCent(reste));
    }
    if (u === 0) return D[d] + (d === 8 ? "s" : "");
    if (u === 1 && d !== 8) return D[d] + "-et-un";
    return D[d] + "-" + U[u];
  };

  const sousMille = (x: number): string => {
    if (x < 100) return sousCent(x);
    const c = Math.floor(x / 100), r = x % 100;
    const tete = c === 1 ? "cent" : U[c] + " cent" + (r === 0 ? "s" : "");
    return r === 0 ? tete : tete + " " + sousCent(r);
  };

  const tranches: [number, string, string][] = [
    [1_000_000_000, "milliard", "milliards"],
    [1_000_000, "million", "millions"],
    [1_000, "mille", "mille"],
  ];

  let reste = n;
  const bouts: string[] = [];
  for (const [valeur, sing, plur] of tranches) {
    const q = Math.floor(reste / valeur);
    if (q > 0) {
      // « mille » est invariable et ne prend pas « un » devant.
      if (valeur === 1000 && q === 1) bouts.push("mille");
      else bouts.push(sousMille(q) + " " + (q > 1 ? plur : sing));
      reste %= valeur;
    }
  }
  if (reste > 0) bouts.push(sousMille(reste));
  return bouts.join(" ");
}

export function blocMontantLettres(titre: string, montant: number, devise = "francs CFA"): string {
  // « Arrêtée la présente facture », « Arrêté le présent devis ».
  const feminin = /^FACTURE/i.test(titre);
  return `
    <div class="doc-lettres" style="margin:6px 0 4px;font-size:10.5px;color:#222">
      ${feminin ? "Arrêtée la présente" : "Arrêté le présent"} ${esc(titre.toLowerCase())}
      à la somme de : <strong>${esc(enLettres(montant))} ${esc(devise)}</strong>
    </div>`;
}
