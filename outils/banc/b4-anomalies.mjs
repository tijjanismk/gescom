// B-4 : le compteur rouge « anomalies à vérifier » du tableau de bord,
// qui ouvre l'Historique filtré ; « Marquer vue » dit par qui, quand,
// et le compteur redescend.
import path from "node:path";
import { DatabaseSync } from "node:sqlite";
import { navigateur, connecter, capture, verifieur, erreursUtiles, TRAVAIL } from "./pw.mjs";
const { b, page, erreurs } = await navigateur();
const ok = verifieur();

// Deux anomalies, comme le noyau les ecrit (le banc ne sait pas
// provoquer a coup sur un « montant ni imputable ni remboursable »).
const base = new DatabaseSync(path.join(TRAVAIL, "essai.db"));
const marque = `banc-b4-${Date.now() % 100000}`;
const inserer = base.prepare(
  `INSERT INTO journal (id, type_evenement, entite_type, entite_id, nouveau_valeur, origine, date_evenement)
   VALUES (?, 'anomalie', 'vente', 'v-banc', ?, 'app', ?)`);
for (const n of [1, 2]) {
  inserer.run(`${marque}-${n}`, JSON.stringify({ message: `${marque} n°${n}` }), new Date(Date.now() + n).toISOString());
}
base.close();

await connecter(page);
await page.getByText("Tableau de bord", { exact: true }).first().click();
await page.waitForTimeout(1500);
const compteur = page.getByTestId("compteur-anomalies");
ok(await compteur.count() === 1, "le compteur rouge est là");
const avant = parseInt(await compteur.locator("strong").innerText(), 10);
ok(avant >= 2, `il compte les anomalies (${avant})`);
await capture(page, "b4-01-tableau-de-bord");

await compteur.click();
await page.waitForTimeout(1500);
ok(await page.getByTestId("filtre-contexte").innerText() === "Anomalies à vérifier", "ouvre l'Historique filtré");
const lignes = page.getByTestId("ligne-historique");
ok(await lignes.count() === avant, `autant de lignes que le compteur (${await lignes.count()})`);
const lA = lignes.filter({ hasText: `${marque} n°1` });
ok(await lA.count() === 1, "l'anomalie du banc est dans la liste");
await lA.getByRole("button", { name: "Marquer vue" }).click();
await page.waitForTimeout(1200);
ok(await lignes.filter({ hasText: `${marque} n°1` }).count() === 0, "vue : elle quitte « à vérifier »");

// Sans le filtre : elle est là, avec par qui et quand.
await page.getByRole("button", { name: "Retirer ce filtre" }).click();
await page.getByLabel("Type").selectOption({ label: "Anomalie" });
await page.waitForTimeout(1200);
const vue = lignes.filter({ hasText: `${marque} n°1` });
ok(/Vue par Patron le \d{2}\/\d{2}\/\d{4} \d{2}:\d{2}/.test(await vue.getByTestId("anomalie-vue").innerText()), "« Vue par Patron le … »");
ok(await lignes.filter({ hasText: `${marque} n°2` }).getByRole("button", { name: "Marquer vue" }).count() === 1, "l'autre reste à marquer");
await capture(page, "b4-02-historique-anomalies");

await page.getByRole("button", { name: "Retour", exact: true }).click();
await page.waitForTimeout(1500);
const apres = await compteur.count() ? parseInt(await compteur.locator("strong").innerText(), 10) : 0;
ok(apres === avant - 1, `le compteur redescend (${avant} → ${apres})`);

const utiles = erreursUtiles(erreurs);
ok(utiles.length === 0, "aucune erreur dans la console" + (utiles.length ? " : " + utiles.join(" | ") : ""));
console.log(`${ok.total()} vérifications`);
await b.close();
