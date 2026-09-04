// lib/unites.ts — Unités de vente courantes chez les commerçants maliens.
//
// Pourquoi une liste plutôt qu'un champ libre : `unite_base` était du
// texte libre, et « m2 », « M² » et « mètre carré » finissaient par
// coexister sur trois articles. L'import CSV, les états de stock et
// l'impression les traitent alors comme trois unités différentes.
//
// La liste SUGGÈRE sans enfermer : le champ « Autre… » reste ouvert,
// parce qu'aucune liste ne couvrira jamais toute une quincaillerie.

export interface GroupeUnites {
  groupe: string;
  unites: string[];
}

/**
 * Ordonnées du plus petit au plus grand DANS chaque groupe.
 *
 * C'est un rappel visuel de D39 : `unite_base` doit être la plus petite
 * unité réellement vendue. Quelqu'un qui parcourt la liste de haut en
 * bas tombe d'abord sur la bonne.
 */
export const UNITES_COURANTES: GroupeUnites[] = [
  {
    groupe: "À l'unité",
    unites: ["pièce", "paquet", "boîte", "carton", "sachet", "lot"],
  },
  {
    groupe: "Poids",
    unites: ["g", "kg", "sac", "tonne"],
  },
  {
    groupe: "Volume",
    unites: ["litre", "bidon", "fût", "citerne"],
  },
  {
    groupe: "Longueur",
    unites: ["cm", "mètre", "barre", "rouleau"],
  },
  {
    groupe: "Surface et volume",
    unites: ["m²", "m³"],
  },
  {
    groupe: "Matériaux",
    unites: ["botte", "planche", "plaque", "brique", "palette", "voyage"],
  },
];

/** Liste à plat, pour les recherches et les validations. */
export const TOUTES_UNITES: string[] =
  UNITES_COURANTES.flatMap(g => g.unites);

/**
 * Rapproche une saisie libre d'une unité connue.
 *
 * « M2 », « m 2 » et « mètre carré » désignent la même chose. Retourne
 * l'unité canonique si elle est reconnue, sinon la saisie nettoyée.
 */
export function normaliserUnite(saisie: string): string {
  const brut = saisie.trim();
  if (!brut) return brut;

  const cle = brut.toLowerCase()
    .replace(/[\s._-]/g, "")
    .replace(/[éèê]/g, "e")
    .replace(/[àâ]/g, "a");

  const alias: Record<string, string> = {
    m2: "m²", metrecarre: "m²", metrescarres: "m²",
    m3: "m³", metrecube: "m³",
    kilo: "kg", kilos: "kg", kilogramme: "kg",
    gramme: "g", grammes: "g",
    metre: "mètre", metres: "mètre", m: "mètre",
    l: "litre", litres: "litre",
    pce: "pièce", piece: "pièce", pieces: "pièce", u: "pièce",
    unite: "pièce", unites: "pièce",
    ct: "carton", cartons: "carton",
    sacs: "sac", bottes: "botte", barres: "barre",
    t: "tonne", tonnes: "tonne",
  };

  if (alias[cle]) return alias[cle];

  const connue = TOUTES_UNITES.find(u =>
    u.toLowerCase().replace(/[\s._-]/g, "") === cle);
  return connue ?? brut;
}
