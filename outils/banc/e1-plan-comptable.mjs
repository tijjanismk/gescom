// E-1 : Parametres -> Comptabilite. Le plan SYSCOHADA est la (une
// centaine de comptes, par classe) ; on cherche, on ajoute un
// sous-compte client, le meme numero est refuse, l'Historique le dit.
import { navigateur, connecter, capture, parametres, verifieur, erreursUtiles } from "./pw.mjs";
const { b, page, erreurs } = await navigateur();
const ok = verifieur();
await connecter(page);
const numero = `411${Date.now() % 10000}`;

await parametres(page, "Comptabilité");
const comptes = page.getByTestId("compte");
const n = await comptes.count();
ok(n >= 100, `le plan est là : ${n} comptes`);
const plan = await page.getByTestId("plan-comptable").innerText();
ok(["Classe 1 — Ressources durables", "Classe 4 — Tiers", "Classe 7 — Produits"].every(t => plan.toUpperCase().includes(t.toUpperCase())),
  "rangé par classe");
await capture(page, "e1-01-plan");

await page.getByLabel("Chercher un compte").fill("caisse");
await page.waitForTimeout(300);
ok((await page.getByTestId("plan-comptable").innerText()).includes("571"), "chercher « caisse » trouve 571");

await page.getByLabel("Numéro du sous-compte").fill(numero);
await page.getByLabel("Libellé du sous-compte").fill("Client Coulibaly");
await page.getByRole("button", { name: /Ajouter/ }).click();
await page.waitForTimeout(1200);
ok((await page.getByRole("status").innerText()).includes(`Sous-compte ${numero} « Client Coulibaly » ajouté sous 411`), "sous-compte ajouté sous 411");
const ligne = page.getByTestId("compte").filter({ hasText: numero });
ok(await ligne.count() === 1 && (await ligne.innerText()).includes("ce dossier"), "il apparaît, marqué « ce dossier »");
await capture(page, "e1-02-sous-compte");

const n0 = erreurs.length;
await page.getByLabel("Numéro du sous-compte").fill(numero);
await page.getByLabel("Libellé du sous-compte").fill("Doublon");
await page.getByRole("button", { name: /Ajouter/ }).click();
await page.waitForTimeout(1000);
erreurs.splice(n0);
ok((await page.getByRole("status").innerText()).includes("existe déjà"), "le même numéro : refusé");
await page.getByLabel("Numéro du sous-compte").fill("1901");
await page.getByLabel("Libellé du sous-compte").fill("Hors plan");
await page.getByRole("button", { name: /Ajouter/ }).click();
await page.waitForTimeout(1000);
erreurs.splice(n0);
ok((await page.getByRole("status").innerText()).includes("ne se rattache"), "hors du plan : refusé, avec quoi faire");

const comptable = await page.evaluate(async () => {
  const { appeler } = await import("/src/lib/pont.ts");
  const roles = await appeler("lire_roles");
  return JSON.stringify(roles.find(r => r.nom === "comptable")?.permissions ?? "");
});
ok(comptable.includes("comptabilite:gerer"), "le comptable a la permission d'office");

await page.getByText("Historique", { exact: true }).first().click();
await page.waitForTimeout(1500);
ok((await page.getByRole("main").innerText()).includes("Sous-compte créé"), "l'Historique le dit");

const utiles = erreursUtiles(erreurs);
ok(utiles.length === 0, "aucune erreur dans la console" + (utiles.length ? " : " + utiles.join(" | ") : ""));
console.log(`${ok.total()} vérifications`);
await b.close();
