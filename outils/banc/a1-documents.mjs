// A-1 : Parametres -> Documents, par l'ecran. En-tete, coordonnees,
// reglage d'un genre, cachet, rechargement, usine, exemple en direct.
import { navigateur, connecter, capture, parametres, verifieur, erreursUtiles, png } from "./pw.mjs";
const { b, page, erreurs } = await navigateur();
const ok = verifieur();
const entete = png("entete.png", 800, 110, [30, 64, 175]);
const cachet = png("cachet.png", 120, 120, [200, 30, 30]);
await connecter(page);
for (const g of ["facture", "devis", "bon_commande", "bon_livraison", "recu", "releve", "ticket"]) {
  await page.evaluate(async g => (await import("/src/lib/pont.ts")).appeler("retablir_reglage_document", { genre: g }), g);
}
await parametres(page, "Documents");
await capture(page, "a1-01-documents");
ok(await page.getByText("Papier à en-tête").count() > 0, "l'onglet Documents s'ouvre");

const [fc] = await Promise.all([page.waitForEvent("filechooser"),
  page.getByRole("button", { name: /^(Téléverser|Changer)$/ }).first().click()]);
await fc.setFiles(entete);
await page.waitForTimeout(1500);
ok(await page.getByText("En-tête enregistré.").count() === 1, "en-tête téléversé");

await page.getByLabel("RCCM").uncheck();
await page.waitForTimeout(800);

const ex = page.frameLocator('iframe[title="Exemple de document"]');
ok(await ex.getByText("Coulibaly Awa").count() >= 1, "l'exemple s'affiche");
await page.getByRole("radio", { name: "A5" }).click();
await page.getByRole("radiogroup", { name: "Colonne remise" }).getByRole("radio", { name: "Toujours" }).click();
await page.waitForTimeout(300);
ok(await ex.locator("th", { hasText: "Remise" }).count() === 1, "l'exemple suit la case, avant d'enregistrer");
await page.getByRole("button", { name: "Ajouter une signature" }).click();
await page.getByRole("textbox", { name: "Signature 3" }).fill("Le magasinier");
await page.getByRole("button", { name: "Enregistrer" }).click();
await page.waitForTimeout(1200);
ok(await page.getByText("Facture : réglages enregistrés.").count() === 1, "facture enregistrée");

const [fc2] = await Promise.all([page.waitForEvent("filechooser"), page.getByRole("button", { name: "Image…" }).first().click()]);
await fc2.setFiles(cachet);
await page.waitForTimeout(1200);
ok(await page.getByText("Image posée sur la signature.").count() === 1, "cachet posé");
await capture(page, "a1-02-facture-reglee");
ok(await page.getByRole("button", { name: "Ajouter une signature" }).count() === 0, "pas de 4e signature");

await page.getByRole("tab", { name: "Ticket de caisse" }).click();
ok(await page.getByRole("radio", { name: "A4" }).count() === 0, "ticket : pas d'A4");
ok(await page.getByText("Signatures", { exact: true }).count() === 0, "ticket : pas de signatures");
await page.getByRole("tab", { name: "Reçu de paiement" }).click();
await page.waitForTimeout(400);
ok(await ex.getByText("REÇU DE RÈGLEMENT").count() === 1, "exemple de reçu");

await page.reload(); await page.waitForTimeout(2500);
if (await page.getByText("Papier à en-tête").count() === 0) await parametres(page, "Documents");
ok(await page.getByRole("radio", { name: "A5", checked: true }).count() === 1, "A5 relu");
ok(await page.getByRole("textbox", { name: "Signature 3" }).inputValue() === "Le magasinier", "signature 3 relue");
ok(!(await page.getByLabel("RCCM").isChecked()), "RCCM décoché relu");
ok(await page.locator('img[alt="Cachet de Pour acquit"]').count() === 1, "cachet relu");
await capture(page, "a1-03-apres-rechargement");

await page.getByRole("button", { name: "Réglages d'usine" }).click();
await page.waitForTimeout(1200);
ok(await page.getByRole("radio", { name: "A4", checked: true }).count() === 1, "usine : A4");
ok(await page.locator('img[alt^="Cachet de"]').count() === 0, "usine : cachets retirés");
await page.getByLabel("RCCM").check();
await page.getByRole("button", { name: "Retirer" }).first().click();
await page.waitForTimeout(800);
console.log(`${ok.total()} vérifications — erreurs :`, erreursUtiles(erreurs));
await b.close();
