// D-2 : le serveur sert tout par `Base`. Chaque ecran du menu et chaque
// onglet de Parametres s'ouvre, sur une base SQLite, sans erreur dans la
// console ni message d'erreur a l'ecran.
import { navigateur, connecter, capture, verifieur, erreursUtiles } from "./pw.mjs";
const { b, page, erreurs } = await navigateur();
const ok = verifieur();
await connecter(page);
const menu = page.getByRole("navigation");
const entrees = await menu.getByRole("button").allInnerTexts();
const noms = entrees.map(t => t.trim()).filter(t => t && !/Paramètres|Patron|Aller/.test(t));
const rouge = /erreur technique|introuvable|impossible de charger|n'est pas encore disponible|n'a pas de version Base/i;
for (const nom of noms) {
  const n = erreurs.length;
  await menu.getByText(nom, { exact: true }).first().click();
  await page.waitForTimeout(1500);
  const texte = await page.locator("main").innerText().catch(() => "");
  const nouvelles = erreursUtiles(erreurs.slice(n));
  ok(nouvelles.length === 0 && !rouge.test(texte), `« ${nom} » s'ouvre` + (nouvelles.length ? ` : ${nouvelles.join(" | ")}` : rouge.test(texte) ? ` : ${texte.match(rouge)[0]}` : ""));
}
await menu.getByText("Paramètres", { exact: true }).first().click();
await page.waitForTimeout(1200);
const onglets = (await page.locator("main button").allInnerTexts()).map(t => t.trim())
  .filter(t => ["Société","Documents","Magasins","Articles","Codes-barres","Import/Export","Catégories","Ventes","Utilisateurs","Rôles","Sauvegarde","Réseau","TVA","Dettes fourn.","Irrécouvrable","Avoirs"].includes(t));
for (const o of [...new Set(onglets)]) {
  const n = erreurs.length;
  await page.locator("main").getByRole("button", { name: o, exact: true }).first().click();
  await page.waitForTimeout(1200);
  const texte = await page.locator("main").innerText().catch(() => "");
  const nouvelles = erreursUtiles(erreurs.slice(n));
  ok(nouvelles.length === 0 && !rouge.test(texte), `Paramètres › ${o}` + (nouvelles.length ? ` : ${nouvelles.join(" | ")}` : ""));
}
await capture(page, "d2-01-dernier-onglet");
console.log(`${ok.total()} vérifications`);
await b.close();
