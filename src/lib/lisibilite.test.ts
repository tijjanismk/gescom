// Le garde-fou de lisibilité : `node --test src/lib/lisibilite.test.ts`
//
// Le 28-29/10/2026, le propriétaire a trouvé du « jaune sur jaune » dans
// l'Historique de caisse puis dans le résultat d'un import : 43 encadrés
// sur fond orange, ambre ou jaune pâle dans 22 écrans, et 25 textes
// orange clair (l'orange 500 fait 2,80:1 sur blanc). Règle retenue,
// WCAG 2.2 niveau AA (4,5:1 pour le texte, 3:1 pour un bord) :
//
//   - un ENCADRÉ d'alerte : fond de carte (`bg-card`), bord gris, la
//     couleur seulement au liseré (`border-l-4 border-l-orange-600`) et à
//     l'icône, texte normal ;
//   - une PASTILLE d'état : fond pâle (`bg-*-100`), texte 900 ;
//   - un TEXTE orange, ambre ou jaune : nuance 700 au moins (≥ 4,5:1) ;
//   - un BOUTON plein : 700 au moins, le texte blanc y fait 5,2:1.
//
// Ce test relit tous les écrans et nomme chaque ligne qui revient en
// arrière. Le thème sombre (`dark:`) n'est pas concerné.

import { test } from "node:test";
import assert from "node:assert/strict";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";

const RACINE = join(import.meta.dirname, "..");
const CHAUDES = "(?:orange|amber|yellow)";
// Pas de préfixe (`dark:`, `hover:`…) : le thème clair au repos.
const NU = "(?<![\\w:-])";

const REGLES: { nom: string; motif: RegExp }[] = [
  { nom: "texte clair (300 à 600) : prendre 700 ou plus", motif: new RegExp(`(?<![\\w:-])(?:hover:)?text-${CHAUDES}-(?:300|400|500|600)\\b`) },
  { nom: "fond pâle d'encadré (50) : bg-card et un liseré", motif: new RegExp(`${NU}bg-${CHAUDES}-50(?:/\\d+)?\\b`) },
  { nom: "fond teinté transparent : bg-card et un liseré", motif: new RegExp(`${NU}bg-${CHAUDES}-\\d+/(?:5|10|15|20)\\b`) },
  { nom: "bouton 500/600 (texte blanc < 4,5:1) : prendre 700", motif: new RegExp(`${NU}bg-${CHAUDES}-(?:500|600)\\b`) },
];

function fichiers(dossier: string): string[] {
  return readdirSync(dossier).flatMap(nom => {
    const chemin = join(dossier, nom);
    if (statSync(chemin).isDirectory()) return fichiers(chemin);
    return nom.endsWith(".tsx") ? [chemin] : [];
  });
}

test("aucune couleur claire sur fond clair dans les écrans", () => {
  const fautes: string[] = [];
  for (const f of fichiers(RACINE)) {
    readFileSync(f, "utf8").split("\n").forEach((ligne, i) => {
      for (const r of REGLES) {
        if (r.motif.test(ligne)) fautes.push(`${relative(RACINE, f)}:${i + 1} — ${r.nom}`);
      }
      // Une pastille (fond 100) porte un texte 900 de sa couleur.
      if (new RegExp(`${NU}bg-${CHAUDES}-100\\b`).test(ligne)
          && new RegExp(`${NU}text-${CHAUDES}-(?:600|700|800)\\b`).test(ligne)) {
        fautes.push(`${relative(RACINE, f)}:${i + 1} — pastille : texte 900 sur fond 100`);
      }
    });
  }
  assert.deepEqual(fautes, [], `\n${fautes.join("\n")}`);
});

test("les couleurs retenues passent 4,5:1 sur blanc (3:1 pour un bord)", () => {
  const lum = (hex: string) => {
    const c = [0, 2, 4].map(i => parseInt(hex.slice(i, i + 2), 16) / 255)
      .map(x => (x <= 0.04045 ? x / 12.92 : ((x + 0.055) / 1.055) ** 2.4));
    return 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
  };
  const rapport = (a: string, b: string) => {
    const [x, y] = [lum(a), lum(b)].sort((p, q) => q - p);
    return (x + 0.05) / (y + 0.05);
  };
  // Les valeurs de Tailwind (palette par défaut).
  assert.ok(rapport("c2410c", "ffffff") >= 4.5, "orange 700, texte");
  assert.ok(rapport("b45309", "ffffff") >= 4.5, "ambre 700, texte");
  assert.ok(rapport("9a3412", "ffffff") >= 4.5, "orange 800, texte");
  assert.ok(rapport("ffffff", "c2410c") >= 4.5, "blanc sur bouton orange 700");
  assert.ok(rapport("7c2d12", "ffedd5") >= 4.5, "pastille orange 900 sur 100");
  assert.ok(rapport("78350f", "fef3c7") >= 4.5, "pastille ambre 900 sur 100");
  assert.ok(rapport("ea580c", "ffffff") >= 3, "liseré orange 600");
  assert.ok(rapport("d97706", "ffffff") >= 3, "liseré ambre 600");
  assert.ok(rapport("f97316", "ffffff") < 4.5, "l'orange 500 d'avant ne passait pas");
});
