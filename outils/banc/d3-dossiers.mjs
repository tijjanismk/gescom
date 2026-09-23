// D-3 : Parametres -> Dossiers. Creer un dossier avec ses dates de
// travail (D21) sur une base SQLite (D22), le choisir a la connexion,
// et y trouver ses affaires a lui : ni les clients ni le stock du
// premier, son propre magasin.
import { navigateur, connecter, capture, parametres, verifieur, erreursUtiles, URL } from "./pw.mjs";
const { b, page, erreurs } = await navigateur();
const ok = verifieur();
await connecter(page);
const code = `Q${Date.now() % 100000}`;
const societe = `Quincaillerie ${code}`;

await parametres(page, "Dossiers");
ok(await page.getByTestId("dossier").filter({ hasText: "ouvert ici" }).count() === 1, "le dossier ouvert est marqué");
await page.getByLabel("Code du dossier").fill(code);
await page.getByLabel("Société").fill(societe);
await page.getByLabel("Début des dates de travail").fill("2026-03-01");
await page.getByLabel("Fin des dates de travail").fill("2027-02-28");
await page.getByRole("button", { name: /Créer le dossier/ }).click();
await page.waitForTimeout(1500);
const avis = await page.getByRole("status").innerText();
ok(avis.includes(`Dossier ${code} créé`) && avis.includes("1er mars 2026") && avis.includes("28 février 2027"), `créé, dates dites : « ${avis.slice(0, 90)}… »`);
ok(await page.getByTestId("dossier").filter({ hasText: societe }).count() === 1, "il est dans la liste");
await capture(page, "d3-01-dossiers");

// Le meme code deux fois : refus a l'ecran.
const n0 = erreurs.length;
await page.getByLabel("Code du dossier").fill(code);
await page.getByLabel("Société").fill("Doublon");
await page.getByRole("button", { name: /Créer le dossier/ }).click();
await page.waitForTimeout(1200);
erreurs.splice(n0);
ok((await page.getByRole("status").innerText()).includes("pris"), "le même code : refusé");

// ---- Se reconnecter dans le nouveau dossier.
const ctx = await b.newContext({ viewport: { width: 1440, height: 900 }, locale: "fr-FR" });
const p2 = await ctx.newPage();
await connecter(p2, "admin", "Admin-2026!", "Admin-2026!", societe);
ok(await p2.getByText(societe).count() > 0, "la barre dit le dossier ouvert");
const lu = await p2.evaluate(async () => {
  const { appeler } = await import("/src/lib/pont.ts");
  const clients = await appeler("lire_clients", {});
  const depot = await appeler("lire_depot_defaut", {});
  const arts = await appeler("lire_articles_avec_unites", {});
  const exercices = await appeler("lire_exercices", {});
  return { clients: clients.map(c => c.nom), depot: depot.nom, stock: arts.map(a => a.stock), nbArticles: arts.length, exercices };
});
ok(lu.clients.length === 1 && lu.clients[0] === "Comptant", `ses clients : son seul client de passage (${lu.clients.join(", ")})`);
ok(lu.depot === "Magasin principal", "son magasin principal");
ok(lu.nbArticles > 0 && lu.stock.every(q => q === 0), "les articles sont communs, le stock est le sien : vide");
ok(lu.exercices.length === 1 && lu.exercices[0].date_debut === "2026-03-01" && lu.exercices[0].date_fin === "2027-02-28", "son exercice : ses dates de travail");
await capture(p2, "d3-02-dans-le-dossier");

const utiles = erreursUtiles(erreurs);
ok(utiles.length === 0, "aucune erreur dans la console" + (utiles.length ? " : " + utiles.join(" | ") : ""));
console.log(`${ok.total()} vérifications`);
await b.close();
