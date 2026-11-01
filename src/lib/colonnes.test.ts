// Vérifie lib/colonnes.ts (D58) : `node --test src/lib/colonnes.test.ts`
// Une colonne n'apparaît que cochée ; sans TVA, jamais un mot de TVA ;
// cacher une colonne ne cache jamais ce qui fait le montant.
import { test } from "node:test";
import assert from "node:assert/strict";
import {
  normaliserColonnes, basculer, choixImpression, alertesSaisie, totauxSaisie, COLONNES_FIXES,
} from "./colonnes.ts";

const sansTva = [{ taux_tva: 0, remise_pct: 0 }, { taux_tva: 0, remise_pct: 0 }];
const avecTva = [{ taux_tva: 0.18, remise_pct: 0 }, { taux_tva: 0, remise_pct: 0 }];
const remisee = [{ taux_tva: 0, remise_pct: 10 }];

test("d'office : les quatre colonnes fixes, ni TVA ni remise", () => {
  assert.deepEqual(normaliserColonnes(null), [...COLONNES_FIXES]);
  const c = choixImpression(null, false, avecTva);
  assert.equal(c.tva, false, "une TVA décochée ne s'imprime pas en colonne");
  assert.equal(c.remise, false);
  assert.equal(c.reference, false);
  assert.equal(c.documentATva, true, "mais les totaux HT/TVA/TTC restent");
});

test("une liste lue se range, garde les fixes et oublie l'inconnue", () => {
  assert.deepEqual(
    normaliserColonnes(["tva", "marge", "total", "reference"]),
    ["reference", "designation", "quantite", "prix_unitaire", "tva", "total"],
  );
});

test("basculer coche une réglable et ne retire jamais une fixe", () => {
  const c = basculer(normaliserColonnes(null), "tva", true);
  assert.deepEqual(c, ["designation", "quantite", "prix_unitaire", "tva", "total"]);
  assert.deepEqual(basculer(c, "tva", false), [...COLONNES_FIXES]);
  assert.deepEqual(basculer(c, "prix_unitaire", false), c, "le prix reste");
});

test("la loi : sans TVA, ni colonne TVA ni récapitulatif, même cochés", () => {
  const c = choixImpression(["tva", "designation", "quantite", "prix_unitaire", "total"], true, sansTva);
  assert.equal(c.tva, false);
  assert.equal(c.recapTva, false);
  assert.equal(c.documentATva, false, "ni HT ni TTC sur le papier");
});

test("avec TVA, la colonne et le récapitulatif suivent leur case", () => {
  const coche = choixImpression(["tva"], true, avecTva);
  assert.equal(coche.tva, true);
  assert.equal(coche.recapTva, true);
  const decoche = choixImpression([], false, avecTva);
  assert.equal(decoche.tva, false);
  assert.equal(decoche.recapTva, false);
});

test("une colonne cochée s'imprime même vide : plus d'« auto »", () => {
  const c = choixImpression(["remise", "reference"], false, sansTva);
  assert.equal(c.remise, true, "colonne remise de « — »");
  assert.equal(c.reference, true);
  assert.equal(c.remiseSousDesignation, false);
});

test("remise cachée : elle se dit sous la désignation", () => {
  assert.equal(choixImpression([], false, remisee).remiseSousDesignation, true);
  assert.equal(choixImpression(["remise"], false, remisee).remiseSousDesignation, false);
});

test("les totaux de la saisie comptent la TVA, colonne cachée ou non", () => {
  // 3 × 1 000 F, remise 10 %, TVA 18 % ; 2 × 500 F sans TVA ; remise globale 5 %.
  const t = totauxSaisie([
    { quantite: 3, prix_unitaire: 1000, remise_pct: 10, taux_tva: 0.18 },
    { quantite: 2, prix_unitaire: 500, remise_pct: 0, taux_tva: 0 },
  ], 5);
  assert.equal(t.totalHt, 2700 + 1000);
  assert.equal(t.remiseMontant, 185);
  assert.equal(t.totalNet, 3515);
  // Ligne 1 : 2 700 − 135 = 2 565 HT, TVA 462 ; TTC 3 027 → 1 009 F × 3 = 3 027.
  // Ligne 2 : 1 000 − 50 = 950 ; 475 × 2 = 950.
  assert.equal(t.totalTva, 462);
  assert.equal(t.totalTtc, 3027 + 950);
  const sansTva = totauxSaisie([{ quantite: 2, prix_unitaire: 500 }], 0);
  assert.equal(sansTva.totalTva, 0);
  assert.equal(sansTva.totalTtc, sansTva.totalNet, "sans TVA, le total est le net");
});

test("la saisie compte ce que les colonnes cachées portent", () => {
  const lignes = [{ taux_tva: 0.18, remise_pct: 5 }, { taux_tva: 0.18, remise_pct: 0 }, { taux_tva: 0, remise_pct: 0 }];
  assert.deepEqual(alertesSaisie([], lignes), { tvaCachee: 2, remiseCachee: 1 });
  assert.deepEqual(alertesSaisie(["tva", "remise"], lignes), { tvaCachee: 0, remiseCachee: 0 });
  assert.deepEqual(alertesSaisie([], sansTva), { tvaCachee: 0, remiseCachee: 0 });
});
