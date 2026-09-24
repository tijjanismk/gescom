// A-2 par l'ecran : Pieces -> Apercu d'un bon de livraison s'ouvre en A5,
// signatures reglees, et le bouton A4 bascule.
import { navigateur, connecter, capture, verifieur, erreursUtiles, png, CAPTURES } from "./pw.mjs";
const { b, page, erreurs } = await navigateur();
const ok = verifieur();
await connecter(page);
await page.getByText("Pièces", { exact: true }).first().click();
await page.waitForTimeout(2000);
// La ligne d'un BL
const ligneBL = page.locator("tr", { hasText: "BL-2026" }).first();
await ligneBL.getByTitle("Aperçu").click();
await page.waitForTimeout(2500);
const bouton = n => page.getByRole("dialog").getByRole("button", { name: n, exact: true });
ok(await bouton("A5").getAttribute("aria-pressed") === "true", "BL : l'aperçu s'ouvre au format réglé (A5)");
const doc = page.frameLocator('iframe[title="Aperçu de la pièce"]');
ok(await doc.getByText("Reçu par", { exact: true }).count() === 1, "BL : trois signatures dans l'aperçu");
await capture(page, "a2ui-01-apercu-bl-a5");
await bouton("A4").click();
await page.waitForTimeout(800);
ok(await bouton("A4").getAttribute("aria-pressed") === "true", "bascule en A4");
ok(await page.getByText("Modèle de l'atelier").count() === 0 && await page.getByText("(actif)").count() === 0, "plus de modèles à choisir");
await page.getByRole("button", { name: "Fermer" }).first().click();
await page.waitForTimeout(500);
const ligneF = page.locator("tr", { hasText: "FAC-2026" }).first();
await ligneF.getByTitle("Aperçu").click();
await page.waitForTimeout(2500);
ok(await bouton("A4").getAttribute("aria-pressed") === "true", "facture : A4");
ok(await doc.getByText("Pour la société", { exact: true }).count() === 1, "facture : signature réglée");
ok(await doc.locator("img").count() >= 1, "facture : le cachet s'affiche");
await capture(page, "a2ui-02-apercu-facture");
console.log(`${ok.total()} vérifications — erreurs :`, erreursUtiles(erreurs));
await b.close();
