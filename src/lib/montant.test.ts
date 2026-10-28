// Vérifie lib/montant.ts : `node --test src/lib/montant.test.ts`
// (Node 22.18+ lit le TypeScript sans outil ; pas de dépendance ajoutée).
import { test } from "node:test";
import assert from "node:assert/strict";
import { chiffresDuMontant, lireMontant } from "./montant.ts";

test("le point, la virgule et l'espace séparent les milliers", () => {
  for (const t of ["15.000", "15 000", "15,000", "15000", " 15 000 ", "15 000", "15 000", "15'000", "15 000 F", "15.000 FCFA"]) {
    assert.equal(lireMontant(t), 15_000, t);
  }
  assert.equal(lireMontant("1.500.000"), 1_500_000);
  assert.equal(lireMontant("2 500"), 2_500);
});

test("ce qui n'est pas un montant ne se devine pas", () => {
  for (const t of ["", "   ", "abc", "-5000", "15 000 euros", "1e5"]) {
    assert.equal(lireMontant(t), null, t);
  }
  assert.equal(lireMontant("99999999999999999999"), null, "au-delà d'un entier sûr");
});

test("les chiffres seuls, pour le champ qui se reformate", () => {
  assert.equal(chiffresDuMontant("15.0"), "150");
  assert.equal(chiffresDuMontant("15."), "15");
  assert.equal(chiffresDuMontant("15a"), "");
  assert.equal(chiffresDuMontant(""), "");
});
