// Monoposte, D36 : « Ne plus suivre le stock » dans la vraie fenetre —
// le seul endroit ou la confirmation est une boite native. Elle dit
// combien d'unites sortent du stock ; apres, l'article est « stock non
// suivi », la caisse ne l'annonce plus en rupture. Puis on remet le
// suivi en comptant (pour laisser la base comme on l'a trouvee).
//
//   apres p1 : node p7-ne-plus-suivre.mjs
import { fenetre, boitesMuettes, boites, connecter, parametres, capture, verifieur, erreursUtiles, URL_FENETRE } from "./pw.mjs";
const ok = verifieur();
const { b, page, erreurs } = await fenetre();
await boitesMuettes(page);
await page.reload();
await page.waitForTimeout(2500);
if (await page.getByText("Pseudo ou email").count()) {
  await connecter(page, "admin", "Admin-2026!", "Admin-2026!", "Ma boutique", URL_FENETRE);
}
erreurs.splice(0);
const avant = await page.evaluate(async () => {
  const { appeler } = await import("/src/lib/pont.ts");
  const art = (await appeler("lire_articles_avec_unites", {})).find(a => a.nom === "Sucre");
  return { id: art.id, stock: art.stock, suivi: art.gere_en_stock };
});
ok(avant.suivi === true && avant.stock > 0, `le Sucre suit son stock (${avant.stock})`);

await parametres(page, "Articles");
await page.locator("main").getByText("Sucre", { exact: true }).click();
await page.waitForTimeout(800);
await page.getByTestId("suivi-stock").getByRole("button", { name: "Ne plus suivre le stock" }).click();
await page.waitForTimeout(2000);
const demande = (await boites(page)).find(x => x.message.includes("ne suivra plus son stock"));
ok(!!demande, "la confirmation est demandée");
ok(!!demande && demande.message.includes(`${avant.stock} kg sortent`), `elle dit ce qui sort du stock (« ${avant.stock} kg sortent… »)`);
ok((await page.getByTestId("suivi-stock").innerText()).includes("Stock non suivi"), "l'article : « Stock non suivi »");
await capture(page, "p7-01-non-suivi");

const apres = await page.evaluate(async (id) => {
  const { appeler } = await import("/src/lib/pont.ts");
  return (await appeler("lire_articles_avec_unites", {})).find(a => a.id === id).gere_en_stock;
}, avant.id);
ok(apres === false, "la caisse le sait");

// On remet le suivi, en comptant ce qu'il y avait.
await page.getByTestId("suivi-stock").getByRole("button", { name: "Suivre le stock" }).click();
const compte = page.getByRole("dialog");
await compte.waitFor({ timeout: 5000 });
const champs = compte.getByPlaceholder("Quantité comptée");
for (let i = 0; i < await champs.count(); i++) await champs.nth(i).fill(i === 0 ? String(avant.stock) : "0");
await compte.getByRole("button", { name: "Suivre le stock" }).click();
await page.waitForTimeout(2000);
ok((await page.getByTestId("suivi-stock").innerText()).includes("Stock suivi"), "suivi repris, en comptant");

const utiles = erreursUtiles(erreurs);
ok(utiles.length === 0, "aucune erreur dans la console" + (utiles.length ? " : " + utiles.join(" | ") : ""));
console.log(`${ok.total()} vérifications`);
await b.close();
