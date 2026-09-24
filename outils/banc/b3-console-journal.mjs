// B-3 : la console du serveur lit son journal technique. Filtre par
// niveau ; une ligne venue d'une caisse s'affiche en texte, jamais en
// HTML.
import { navigateur, connecter, capture, verifieur } from "./pw.mjs";
const { b, page } = await navigateur();
const ok = verifieur();
const SERVEUR = `http://127.0.0.1:${process.env.PORT_SERVEUR ?? 7300}`;

// Une caisse signale une « erreur » piegee.
await connecter(page);
const marque = `banc-b3-${Date.now() % 100000}`;
await page.evaluate(async m => {
  const { signalerErreur } = await import("/src/lib/pont.ts");
  signalerErreur("erreur", `${m} <img src=x onerror="document.title='pirate'">`);
}, marque);
await page.waitForTimeout(800);

// La console.
await page.goto(SERVEUR);
await page.waitForTimeout(800);
ok(await page.locator("#carte-journal").isHidden(), "le journal est caché avant de s'identifier");
await page.fill("#identifiant", "admin");
await page.fill("#motdepasse", "Admin-2026!");
await page.click("#form-connexion button[type=submit]");
// Attendre la carte plutot qu'un delai fixe : sous charge (une suite
// cargo en parallele), 1,5 s ne suffisait pas.
await page.locator("#carte-journal").waitFor({ state: "visible", timeout: 15000 }).catch(() => {});
await page.locator("#journal div").first().waitFor({ timeout: 15000 }).catch(() => {});
ok(await page.locator("#carte-journal").isVisible(), "l'onglet Journal apparaît");
const lignes = page.locator("#journal div");
ok(await lignes.count() > 1, `des lignes (${await lignes.count()})`);
ok((await lignes.first().innerText()) >= (await lignes.last().innerText()), "les plus récentes en haut");

await page.getByRole("button", { name: "Caisses" }).click();
await page.waitForTimeout(800);
const n = await lignes.count();
let toutes = n > 0;
for (let i = 0; i < n; i++) toutes &&= (await lignes.nth(i).innerText()).includes("[POSTE ]");
ok(toutes, `« Caisses » ne montre que les lignes POSTE (${n})`);
const piege = lignes.filter({ hasText: marque });
ok(await piege.count() === 1, "la ligne de la caisse est là");
ok((await piege.innerText()).includes("<img src=x"), "affichée en texte");
ok(await page.locator("#journal img").count() === 0 && (await page.title()) !== "pirate", "rien n'est injecté");
await capture(page, "b3-01-console-journal");

await page.getByRole("button", { name: "Erreurs" }).click();
await page.waitForTimeout(800);
const e = await lignes.allInnerTexts();
ok(e.every(l => l.includes("[ERREUR]") || l === "Rien à ce niveau."), "« Erreurs » filtre");
await page.getByRole("button", { name: "Tout" }).click();
await page.waitForTimeout(800);
ok(await lignes.count() >= n, "« Tout » rend tout");
console.log(`${ok.total()} vérifications`);
await b.close();
