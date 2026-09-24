// E-2 : Parametres -> Comptabilite -> Affectations. Chaque operation a
// son compte par defaut ; on met les especes sur une caisse a nous
// (un sous-compte 571…), la liste ne propose que des comptes de
// tresorerie, on revient au defaut, l'Historique le dit.
import { navigateur, connecter, capture, parametres, verifieur, erreursUtiles } from "./pw.mjs";
const { b, page, erreurs } = await navigateur();
const ok = verifieur();
await connecter(page);
const caisse = `571${Date.now() % 10000}`;

await parametres(page, "Comptabilité");
const bloc = page.getByTestId("affectations");
const lignes = bloc.getByTestId("affectation");
const n = await lignes.count();
ok(n >= 25, `chaque opération a sa ligne : ${n}`);
ok(await lignes.filter({ hasText: "défaut" }).count() === n, "au départ, tout est au défaut");
ok(await bloc.getByLabel("Compte : Espèces").inputValue() === "571", "espèces → 571");
ok(await bloc.getByLabel("Compte : Ventes de marchandises").inputValue() === "701", "ventes → 701");
ok(await bloc.getByLabel("Compte : Loyer").inputValue() === "622", "loyer → 622");
const choix = await bloc.getByLabel("Compte : Espèces").locator("option").allInnerTexts();
ok(choix.length > 0 && choix.every(t => t.startsWith("5")), `les espèces ne proposent que la trésorerie (${choix.length})`);
await capture(page, "e2-01-affectations");

// Un sous-compte de caisse, puis les espèces dessus.
await page.getByLabel("Numéro du sous-compte").fill(caisse);
await page.getByLabel("Libellé du sous-compte").fill("Caisse boutique");
await page.getByRole("button", { name: /Ajouter/ }).click();
await page.waitForTimeout(1500);
await bloc.getByLabel("Compte : Espèces").selectOption(caisse);
await page.waitForTimeout(1200);
ok((await bloc.getByRole("status").innerText()).includes(`Espèces → ${caisse}`), "espèces sur la caisse de la boutique");
const especes = lignes.filter({ hasText: "Espèces" }).first();
ok(await especes.getByRole("button", { name: "Défaut : Espèces" }).count() === 1, "la ligne propose de revenir au défaut");
await capture(page, "e2-02-caisse-a-nous");

// Recharger : le réglage tient.
await page.reload();
await page.waitForTimeout(1500);
await parametres(page, "Comptabilité");
ok(await page.getByTestId("affectations").getByLabel("Compte : Espèces").inputValue() === caisse, "après rechargement, le réglage tient");

await page.getByTestId("affectations").getByRole("button", { name: "Défaut : Espèces" }).click();
await page.waitForTimeout(1200);
ok(await page.getByTestId("affectations").getByLabel("Compte : Espèces").inputValue() === "571", "retour au défaut : 571");

await page.getByText("Historique", { exact: true }).first().click();
await page.waitForTimeout(1500);
ok((await page.getByRole("main").innerText()).includes("Affectation comptable modifiée"), "l'Historique le dit");

const utiles = erreursUtiles(erreurs);
ok(utiles.length === 0, "aucune erreur dans la console" + (utiles.length ? " : " + utiles.join(" | ") : ""));
console.log(`${ok.total()} vérifications`);
await b.close();
