// lib/impression.ts — ce que tous les documents imprimés partagent (v3, A-2).
//
// Un seul générateur par genre (D17), mais un seul HABILLAGE pour tous :
// l'en-tête (image, ou nom + coordonnées cochées), le pied (image, ou la
// mention), les signatures avec leur cachet (D18), le montant en
// lettres. Les écrire une fois ici, c'est ce qui fait que la facture, le
// reçu et le relevé d'une même boutique se ressemblent.

import { appeler as invoke } from "@/lib/pont";
import {
  type Coordonnee, type FiligraneDoc, type Genre, type ReglageGenre, type ReglagesDocuments,
  type SignatureDoc, lireReglagesDocuments,
} from "@/lib/documents";

/** Ce qui habille un document : les images de la société et le réglage de son genre. */
export interface Habillage {
  logo?: string | null;
  entete?: string | null;
  pied?: string | null;
  reglage?: ReglageGenre | null;
  coordonnees?: Coordonnee[] | null;
  filigrane?: FiligraneDoc | null;
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
    filigrane: reglages?.filigrane ?? null,
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
 * Jusqu'à trois emplacements : le libellé SOULIGNÉ (« Pour acquit »,
 * « Le fournisseur »), et dessous la place de signer, où se pose
 * l'image (cachet, signature scannée). Pas de trait séparé à distance
 * du libellé : c'est la présentation des factures d'ici (retour du
 * 25/09) — on signe sous le libellé souligné.
 *
 * `page-break-inside:avoid` : un libellé orphelin en bas de page ne se
 * signe pas — le bloc part en entier.
 */
export function blocSignatures(signatures: SignatureDoc[] | undefined | null): string {
  const s = (signatures ?? []).filter(x => x.libelle?.trim());
  if (s.length === 0) return "";
  const largeur = s.length === 3 ? "30%" : "45%";
  // Aux bords de la page les libellés s'alignent sur la marge ; au milieu, centrés.
  const alignement = (i: number) =>
    s.length === 1 ? "right" : i === 0 ? "left" : i === s.length - 1 ? "right" : "center";
  const flex = { left: "flex-start", right: "flex-end", center: "center" } as const;
  const une = (x: SignatureDoc, i: number) => `
    <div class="doc-sig" style="width:${largeur};text-align:${alignement(i)}">
      <div class="doc-sig-libelle" style="font-size:11px;font-weight:bold;text-decoration:underline;text-underline-offset:2px">${esc(x.libelle)}</div>
      <div style="height:22mm;display:flex;align-items:center;justify-content:${flex[alignement(i)]}">
        ${x.image ? `<img src="${x.image}" alt="" style="max-height:21mm;max-width:100%;object-fit:contain"/>` : ""}
      </div>
    </div>`;
  return `
    <div class="doc-signatures" style="display:flex;justify-content:${s.length === 1 ? "flex-end" : "space-between"};
                margin-top:14px;page-break-inside:avoid">
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

// =====================================================================
//  La page : en-tête et pied répétés, filigrane (retour du 25/09)
// =====================================================================
//
// Comme dans Word : l'en-tête en haut de CHAQUE page et le pied en bas
// de chaque page, collés aux bords du papier (aucune marge, sur aucun
// des axes) ; le texte, lui, garde sa marge. Avant, l'en-tête n'était
// qu'en haut de la première page et le pied qu'au bas de la dernière.
//
// Comment, sans script (l'aperçu n'en exécute pas) :
// - l'en-tête est le `<thead>` d'un tableau qui porte tout le document :
//   le moteur d'impression le répète en haut de chaque page ;
// - le pied est posé en `position:fixed` au bas de la page — à
//   l'impression, un élément fixe se répète sur chaque page. Le
//   `<tfoot>` en garde une copie invisible : c'est elle qui réserve sa
//   hauteur au bas de chaque page, pour que le texte ne passe jamais
//   dessous ;
// - le filigrane, fixe lui aussi, se répète au milieu de chaque page,
//   sans jamais masquer le texte (voir `.cadre-filigrane`).

export type Papier = "a4" | "a5";

const PAPIER: Record<Papier, { l: number; h: number }> = {
  a4: { l: 210, h: 297 },
  a5: { l: 148, h: 210 },
};

export interface PageDoc {
  /** Le titre de la fenêtre (et du fichier PDF). */
  titre: string;
  papier: Papier;
  /** Marge du texte à gauche et à droite — et en haut / en bas sans bandeau. */
  marge: string;
  /** La mention de bas de page (HTML brut), quand il n'y a pas d'image de pied. */
  mention?: string | null;
  /** « Imprimé le … », en bas de chaque page. */
  imprimeLe?: string | null;
  /** Taille de la police du document, en px. */
  police?: number;
  /** Les styles propres au document. */
  css?: string;
  contenu: string;
  /** Le script d'impression, en fin de document. */
  script?: string;
}

/**
 * Le filigrane de ce document, ou rien : une image (l'emporte) ou un mot,
 * estompé (`opacity`) et flouté (`blur`) pour ne pas se mêler aux lignes
 * des tableaux. Rien si le genre l'a retiré.
 */
export function blocFiligrane(h: Habillage, papier: Papier): string {
  const f = h.filigrane;
  if (!f || h.reglage?.filigrane === false) return "";
  const effet = `opacity:${(f.opacite / 100).toFixed(2)};filter:blur(${f.flou}px)`;
  if (f.image) {
    return `<div class="cadre-filigrane"><img src="${f.image}" alt=""
      style="width:${f.taille}%;height:auto;${effet}"/></div>`;
  }
  const t = f.texte?.trim();
  if (!t) return "";
  // Un caractère gras d'Arial fait ~0,62 em : la police qui donne au mot
  // la largeur demandée, sans dépasser 70 mm pour un mot très court.
  const largeur = (PAPIER[papier].l * f.taille) / 100;
  const police = Math.min(70, largeur / (0.62 * Array.from(t).length));
  return `<div class="cadre-filigrane"><div class="doc-filigrane-texte"
    style="font:bold ${police.toFixed(1)}mm Arial,sans-serif;white-space:nowrap;color:#000;
           transform:rotate(-30deg);${effet}">${esc(t)}</div></div>`;
}

/** Le document complet : la page, ses bandeaux répétés, son contenu. */
export function documentHTML(h: Habillage, p: PageDoc): string {
  const { h: hauteur } = PAPIER[p.papier];
  const image = (src: string, classe: string) =>
    `<img class="${classe}" src="${src}" alt="" style="width:100%;height:auto;display:block"/>`;

  const lignesPied = [
    !h.pied && p.mention ? `<div class="doc-mention">${p.mention}</div>` : "",
    p.imprimeLe ? `<div class="doc-imprime">Imprimé le ${esc(p.imprimeLe)}</div>` : "",
  ].join("");
  const pied =
    (lignesPied ? `<div class="cadre-pied-texte${h.pied ? " sur-image" : ""}">${lignesPied}</div>` : "")
    + (h.pied ? image(h.pied, "doc-pied") : "");

  const enTete = h.entete
    ? `${image(h.entete, "doc-entete")}<div style="height:5mm"></div>`
    : `<div style="height:${p.marge}"></div>`;
  const reserveBas = pied
    ? `<div style="height:4mm"></div><div class="cadre-reserve" aria-hidden="true">${pied}</div>`
    : `<div style="height:${p.marge}"></div>`;

  return `<!DOCTYPE html>
<html lang="fr">
<head>
  <meta charset="UTF-8">
  <title>${esc(p.titre)}</title>
  <style>
    * { margin:0; padding:0; box-sizing:border-box; }
    body { font-family:Arial,sans-serif; font-size:${p.police ?? 11}px; color:#000; }
    /* Le cadre ne doit rien prendre aux styles du document (un « td »
       bordé, un « table » à marge) : sélecteurs plus précis que les leurs. */
    table.cadre { width:100%; border-collapse:collapse; margin:0; position:relative; z-index:1; }
    table.cadre > thead > tr > td, table.cadre > tfoot > tr > td { padding:0; border:0; background:none; }
    table.cadre > tbody > tr > td.cadre-corps { padding:0 ${p.marge}; border:0; vertical-align:top; background:none; }
    .cadre-bas { position:fixed; left:0; right:0; bottom:0; z-index:2; }
    .cadre-reserve { visibility:hidden; }
    /* Le filigrane passe PAR-DESSUS, en « multiply » : le noir du texte
       reste noir, seul le blanc du papier prend la teinte. Posé dessous,
       les lignes des tableaux le hachaient une sur deux quand la boîte
       d'impression n'imprime pas les fonds (son réglage par défaut). */
    .cadre-filigrane { position:fixed; top:0; left:0; right:0; bottom:0; z-index:3;
                       mix-blend-mode:multiply;
                       display:flex; align-items:center; justify-content:center;
                       overflow:hidden; pointer-events:none; }
    .cadre-pied-texte { margin:0 ${p.marge}; padding:5px 0 4mm; text-align:center;
                        border-top:1px solid #ddd; }
    .cadre-pied-texte.sur-image { border-top:0; padding:0 0 2px; }
    .doc-mention { font-size:10px; color:#555; }
    .doc-imprime { font-size:9px; color:#999; margin-top:2px; }
    /* À l'écran (aperçu) : au moins une page, pour que le pied tombe
       au bas du papier comme à l'impression. */
    @media screen { body { min-height:${hauteur}mm; } }
    @media print { @page { size:${PAPIER[p.papier].l}mm ${hauteur}mm; margin:0; } }
    ${p.css ?? ""}
  </style>
</head>
<body>
  ${blocFiligrane(h, p.papier)}
  ${pied ? `<div class="cadre-bas">${pied}</div>` : ""}
  <table class="cadre">
    <thead><tr><td>${enTete}</td></tr></thead>
    <tfoot><tr><td>${reserveBas}</td></tr></tfoot>
    <tbody><tr><td class="cadre-corps">${p.contenu}</td></tr></tbody>
  </table>
  ${p.script ?? ""}
</body>
</html>`;
}
