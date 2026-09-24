// A-3 : l'atelier est parti. Plus d'onglet, plus de commande, et la
// Societe renvoie a Documents.
import { navigateur, connecter, verifieur, erreursUtiles, capture } from "./pw.mjs";
const { b, page, erreurs } = await navigateur();
const ok = verifieur();
await connecter(page);
await page.getByText("Paramètres", { exact: true }).first().click();
await page.waitForTimeout(1200);
ok(await page.getByRole("button", { name: "Modèles de documents" }).count() === 0, "plus d'onglet « Modèles de documents »");
ok(await page.getByRole("button", { name: "Documents" }).count() === 1, "l'onglet Documents est là");
await page.getByRole("button", { name: "Société" }).first().click();
await page.waitForTimeout(800);
ok(await page.getByText("Paramètres → Documents").count() >= 1, "la Société renvoie à Documents");
await capture(page, "a3-01-societe");
const refus = await page.evaluate(async () => {
  const { appeler } = await import("/src/lib/pont.ts");
  const r = {};
  for (const c of ["lire_modeles", "exporter_modeles", "lister_images"]) {
    try { await appeler(c, {}); r[c] = "servie"; } catch (e) { r[c] = String(e); }
  }
  return r;
});
for (const [c, m] of Object.entries(refus)) ok(/ne connaît pas la commande/.test(m), `« ${c} » n'existe plus au serveur`);
// Les 404 des commandes disparues sont attendus ici.
console.log(`${ok.total()} vérifications — erreurs :`, erreursUtiles(erreurs).filter(e => !e.includes("404")));
await b.close();
