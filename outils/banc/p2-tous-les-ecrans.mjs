// Monoposte (v3) : chaque entree du menu et chaque onglet de Parametres
// s'ouvre dans la VRAIE fenetre Tauri (WebView2, pas Edge), sans erreur
// dans la console ni message d'erreur a l'ecran. Ce que d2 fait dans un
// navigateur, ici avec ce qu'un navigateur n'a pas : les commandes
// locales (poste.json, impression) y repondent pour de bon.
//
//   apres p1 (la fenetre est branchee) : node p2-tous-les-ecrans.mjs
import { fenetre, boitesMuettes, connecter, capture, verifieur, erreursUtiles, URL_FENETRE } from "./pw.mjs";
const ok = verifieur();
const { b, page, erreurs } = await fenetre();
await boitesMuettes(page);
if (await page.getByText("Brancher ce poste").count()) throw new Error("La fenêtre n'est pas branchée : lancer p1 d'abord.");
if (!(await page.getByText("Tableau de bord", { exact: true }).count())) {
  await connecter(page, "admin", "Admin-2026!", "Admin-2026!", "Ma boutique", URL_FENETRE);
}
erreurs.splice(0);

const menu = page.getByRole("navigation");
const entrees = await menu.getByRole("button").allInnerTexts();
const noms = entrees.map(t => t.trim()).filter(t => t && !/Paramètres|Patron|Aller/.test(t));
ok(noms.length >= 10, `le menu a ${noms.length} entrées`);
const rouge = /erreur technique|introuvable|impossible de charger|n'est pas encore disponible|n'a pas de version Base|n'est disponible que dans l'application/i;
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
  .filter(t => ["Société","Documents","Magasins","Articles","Codes-barres","Import/Export","Catégories","Ventes","Utilisateurs","Rôles","Sauvegarde","Réseau","TVA","Dettes fourn.","Irrécouvrable","Avoirs","Dossiers","Comptabilité","Menu"].includes(t));
for (const o of [...new Set(onglets)]) {
  const n = erreurs.length;
  await page.locator("main").getByRole("button", { name: o, exact: true }).first().click();
  await page.waitForTimeout(1200);
  const texte = await page.locator("main").innerText().catch(() => "");
  const nouvelles = erreursUtiles(erreurs.slice(n));
  ok(nouvelles.length === 0 && !rouge.test(texte), `Paramètres › ${o}` + (nouvelles.length ? ` : ${nouvelles.join(" | ")}` : rouge.test(texte) ? ` : ${texte.match(rouge)[0]}` : ""));
}
// L'onglet Reseau, dans la fenetre, lit poste.json (commande locale).
await page.locator("main").getByRole("button", { name: "Réseau", exact: true }).first().click();
await page.waitForTimeout(1200);
const role = await page.getByTestId("role-poste").innerText().catch(() => "");
ok(role.includes("Ce poste garde la base") && role.includes("127.0.0.1:7300"),
  "Paramètres › Réseau dit le rôle lu dans poste.json : ce poste garde la base, serveur ici");
await capture(page, "p2-01-reseau");
console.log(`${ok.total()} vérifications`);
await b.close();
