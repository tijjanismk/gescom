// B-2 : une erreur de la fenetre arrive dans le journal technique du
// serveur, avec le nom du poste et l'ecran ouvert.
import fs from "node:fs";
import path from "node:path";
import { navigateur, connecter, verifieur, TRAVAIL } from "./pw.mjs";
const { b, page } = await navigateur();
const ok = verifieur();
const JOURNAL = path.join(TRAVAIL, "essai.log");
const lire = () => (fs.existsSync(JOURNAL) ? fs.readFileSync(JOURNAL, "utf8") : "");
await connecter(page);
await page.getByText("Stock", { exact: true }).first().click();
await page.waitForTimeout(1000);

const marque = `banc-b2-${Date.now() % 100000}`;
await page.evaluate(m => {
  setTimeout(() => { throw new Error(`${m} boum`); });
  Promise.reject(new Error(`${m} promesse`));
}, marque);
await page.waitForTimeout(1500);
const lignes = lire().split("\n").filter(l => l.includes(marque));
ok(lignes.length === 2, `deux lignes pour ${marque} (${lignes.length})`);
const erreur = lignes.find(l => l.includes("boum")) ?? "";
const promesse = lignes.find(l => l.includes("promesse")) ?? "";
ok(erreur.includes("[POSTE ]"), "niveau POSTE");
ok(/POST \/journal-poste · [^·]+ · stock · Uncaught Error: /.test(erreur), "le nom du poste et l'écran ouvert");
ok(promesse.includes("promesse rejetée"), "une promesse rejetée est dite");
ok(erreur.includes("pile : "), "la pile suit");
console.log("  " + erreur.slice(0, 200));

// La meme erreur repetee n'est envoyee qu'une fois en dix secondes.
await page.evaluate(m => { for (let i = 0; i < 5; i++) setTimeout(() => { throw new Error(`${m} boucle`); }); }, marque);
await page.waitForTimeout(1500);
ok(lire().split("\n").filter(l => l.includes(`${marque} boucle`)).length === 1, "une boucle d'erreurs n'envoie qu'une ligne");
console.log(`${ok.total()} vérifications`);
await b.close();
