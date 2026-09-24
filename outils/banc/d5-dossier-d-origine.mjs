// D-5 : la base d'avant la v3 est le dossier « Ma boutique ». L'ecran
// invite a le nommer ; on le nomme, la barre suit, l'Historique le dit.
// A la fin on lui rend son nom d'usine : le banc reste rejouable.
import { navigateur, connecter, capture, parametres, verifieur, erreursUtiles } from "./pw.mjs";
const { b, page, erreurs } = await navigateur();
const ok = verifieur();
await connecter(page);
const nom = `Boutique du banc ${Date.now() % 10000}`;

await parametres(page, "Dossiers");
const invite = page.getByTestId("a-nommer");
ok(await invite.count() === 1, "le dossier au nom d'usine invite à être nommé");
ok((await invite.innerText()).includes("Tout ce que vous aviez avant"), "l'invitation dit ce qu'il contient");
await capture(page, "d5-01-a-nommer");

await invite.getByRole("button", { name: "Le nommer" }).click();
ok(await page.getByRole("button", { name: "Renommer", exact: true }).isDisabled(), "un nom vide ne s'envoie pas");
await page.getByLabel("Nouveau nom du dossier").fill(nom);
await page.getByRole("button", { name: "Renommer", exact: true }).click();
await page.waitForTimeout(1200);
ok((await page.getByRole("status").first().innerText()).includes(`Dossier renommé : ${nom}`), "renommé");
ok(await page.getByTestId("a-nommer").count() === 0, "l'invitation disparaît");
ok(await page.getByTestId("dossier").filter({ hasText: nom }).filter({ hasText: "PRINCIPAL" }).count() === 1,
  "même code, nouveau nom");
ok((await page.getByTestId("exercices").innerText()).includes(`Dates de travail de « ${nom} »`), "les dates de travail disent le nouveau nom");

await page.reload();
await page.waitForTimeout(2000);
ok(await page.getByText(nom).count() > 0, "après rechargement, la barre dit le nouveau nom");
await capture(page, "d5-02-nomme");

await page.getByText("Historique", { exact: true }).first().click();
await page.waitForTimeout(1500);
ok((await page.getByRole("main").innerText()).includes("Dossier renommé"), "l'Historique dit le renommage");

// Rendre le nom d'usine, par le crayon.
await parametres(page, "Dossiers");
await page.getByRole("button", { name: `Renommer ${nom}` }).click();
await page.getByLabel("Nouveau nom du dossier").fill("Ma boutique");
await page.getByRole("button", { name: "Renommer", exact: true }).click();
await page.waitForTimeout(1200);
ok(await page.getByTestId("a-nommer").count() === 1, "rendu au nom d'usine : l'invitation revient");

const utiles = erreursUtiles(erreurs);
ok(utiles.length === 0, "aucune erreur dans la console" + (utiles.length ? " : " + utiles.join(" | ") : ""));
console.log(`${ok.total()} vérifications`);
await b.close();
