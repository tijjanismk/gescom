// lib/ean13.ts — dessiner un EAN-13 en SVG, sans police ni bibliothèque.
//
// Les étiquettes n'imprimaient que le NUMÉRO : « des barres en CSS ne se
// scannent pas ». Vrai pour des barres approximatives ; faux pour un
// SVG où chaque module fait exactement la même largeur. Un EAN-13 tient
// en 95 modules ; à 0,33 mm le module (la norme), le code fait 31 mm et
// se lit avec une douchette ordinaire sur une imprimante de bureau.
//
// Le calcul du chiffre de contrôle vit côté noyau (coeur::codebarre) ;
// ici on ne fait que dessiner ce qu'on reçoit, et on refuse ce qui n'a
// pas la forme.

/** Les sept modules de chaque chiffre, jeu L (gauche, parité impaire). */
const L = ["0001101", "0011001", "0010011", "0111101", "0100011", "0110001", "0101111", "0111011", "0110111", "0001011"];
/** Jeu G : L lu à l'envers et inversé — la parité paire de la partie gauche. */
const G = L.map((m) => m.split("").reverse().map((b) => (b === "0" ? "1" : "0")).join(""));
/** Jeu R (droite) : L inversé. */
const R = L.map((m) => m.split("").map((b) => (b === "0" ? "1" : "0")).join(""));
/** Quel jeu (L ou G) pour chacun des six chiffres de gauche, selon le premier chiffre. */
const PARITE = ["LLLLLL", "LLGLGG", "LLGGLG", "LLGGGL", "LGLLGG", "LGGLLG", "LGGGLL", "LGLGLG", "LGLGGL", "LGGLGL"];

/** Les 95 modules (« 1 » = barre noire) d'un EAN-13, ou null si le code n'a pas la forme. */
export function modulesEan13(code: string): string | null {
  if (!/^\d{13}$/.test(code)) return null;
  const chiffres = code.split("").map(Number);
  const parite = PARITE[chiffres[0]];
  let m = "101";
  for (let i = 1; i <= 6; i++) m += (parite[i - 1] === "L" ? L : G)[chiffres[i]];
  m += "01010";
  for (let i = 7; i <= 12; i++) m += R[chiffres[i]];
  m += "101";
  return m;
}

/**
 * Le SVG d'une étiquette : les barres, et le numéro dessous en clair —
 * lisible et saisissable à la main si la douchette manque. Largeur en
 * mm ; les barres de garde descendent plus bas, comme sur un vrai code.
 */
export function svgEan13(code: string, largeurMm = 31, hauteurMm = 14): string {
  const modules = modulesEan13(code);
  if (!modules) return "";
  const module = largeurMm / 95;
  const texteMm = 3.2;
  const gardes = new Set([0, 1, 2, 45, 46, 47, 48, 49, 92, 93, 94]);
  let barres = "";
  for (let i = 0; i < 95; i++) {
    if (modules[i] !== "1") continue;
    const h = gardes.has(i) ? hauteurMm : hauteurMm - texteMm;
    barres += `<rect x="${(i * module).toFixed(3)}" y="0" width="${module.toFixed(3)}" height="${h.toFixed(2)}"/>`;
  }
  const y = (hauteurMm - 0.6).toFixed(2);
  const texte =
    `<text x="${(1.5 * module).toFixed(2)}" y="${y}" font-size="2.6" font-family="Arial,sans-serif" text-anchor="start">${code[0]}</text>` +
    `<text x="${(24.5 * module).toFixed(2)}" y="${y}" font-size="2.6" font-family="Arial,sans-serif" text-anchor="middle" textLength="${(38 * module).toFixed(2)}">${code.slice(1, 7)}</text>` +
    `<text x="${(70.5 * module).toFixed(2)}" y="${y}" font-size="2.6" font-family="Arial,sans-serif" text-anchor="middle" textLength="${(38 * module).toFixed(2)}">${code.slice(7, 13)}</text>`;
  return (
    `<svg xmlns="http://www.w3.org/2000/svg" width="${largeurMm}mm" height="${hauteurMm}mm" ` +
    `viewBox="0 0 ${largeurMm} ${hauteurMm}" shape-rendering="crispEdges" fill="#000">` +
    `<rect x="0" y="0" width="${largeurMm}" height="${hauteurMm}" fill="#fff"/>${barres}${texte}</svg>`
  );
}
