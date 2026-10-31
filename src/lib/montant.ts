// lib/montant.ts — lire un montant en francs CFA tapé à l'écran.
//
// Le franc CFA n'a pas de centimes : un point, une virgule ou une espace
// dans un montant séparent les milliers. « 15.000 », « 15 000 »,
// « 15,000 » et « 15000 » valent quinze mille francs — `Number("15.000")`
// en donnait 15 (revue de la paie du 28/10). Le serveur juge aussi la
// saisie (D27) : ceci n'est que la lecture, pas une garde.
//
// Pur, sans import : `MoneyInput` le reprend, et `montant.test.ts` le
// vérifie avec `node --test` (Node lit le TypeScript sans outil).

/** Les seuls chiffres d'un montant tapé : séparateurs de milliers
 * (espaces, espaces fines insécables, points, virgules, apostrophes)
 * retirés, « F » ou « FCFA » final toléré. Vide si autre chose s'y
 * trouve (une lettre, un signe moins) : on ne devine pas. */
export function chiffresDuMontant(texte: string): string {
  const sans = texte
    .trim()
    .replace(/\s*(f\s*cfa|fcfa|f)$/i, "")
    .replace(/[\s  .,'’]/g, "");
  return /^\d*$/.test(sans) ? sans : "";
}

/** Un montant en francs entiers, ou `null` si le texte n'en est pas un
 * (vide, lettres, négatif). */
export function lireMontant(texte: string): number | null {
  const c = chiffresDuMontant(texte);
  if (!c) return null;
  const n = Number(c);
  return Number.isSafeInteger(n) ? n : null;
}
