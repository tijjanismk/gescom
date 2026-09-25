// H-2 : Equipe -> Suivi clients. Un rappel attribue a une personne
// apparait dans « Mes rappels » de cette personne, se marque fait ; un
// prospect devient client tout seul a sa premiere vente.
import { navigateur, connecter, capture, verifieur, erreursUtiles, URL, URL_EQUIPE } from "./pw.mjs";
const { b, page, erreurs } = await navigateur();
const ok = verifieur();
const s = Date.now() % 100000;
const nomProspect = `Boutique Konaté ${s}`;

await connecter(page, "admin", "admin123", "Admin-2026!", "Ma boutique", URL_EQUIPE);
erreurs.splice(0);

await page.getByRole("navigation", { name: "Menu Équipe" }).getByRole("button", { name: /Suivi clients/ }).click();
await page.waitForTimeout(1000);

// ---- Prospects ----
await page.getByRole("tab", { name: "Prospects" }).click();
await page.waitForTimeout(800);
await page.getByLabel("Nom du prospect").fill(nomProspect);
await page.getByLabel("Téléphone du prospect").fill("76 12 34 56");
await page.getByLabel("Origine du prospect").fill("salon");
await page.getByRole("button", { name: "Ajouter" }).click();
await page.waitForTimeout(1200);
ok(await page.getByTestId("prospect").filter({ hasText: nomProspect }).count() === 1, "le prospect apparaît dans la liste");
ok((await page.getByTestId("prospect").filter({ hasText: nomProspect }).innerText()).includes("salon"), "son origine s'affiche");

// Sa fiche 360 s'ouvre comme celle d'un client, marquée « prospect ».
await page.getByTestId("prospect").filter({ hasText: nomProspect }).click();
await page.waitForTimeout(1000);
ok(await page.getByText("prospect", { exact: true }).count() >= 1, "la fiche le montre comme prospect");

// Un rappel se pose sur cette fiche, pour le patron (admin) — verifie
// depuis « Mes rappels » ensuite.
await page.getByLabel("Pour qui").selectOption({ label: "Patron (moi)" });
await page.getByLabel("Quand").fill("2027-01-15");
await page.getByLabel("Quoi").fill("Rappeler pour le prix en gros");
await page.getByRole("button", { name: "Rappeler" }).click();
await page.waitForTimeout(1200);
ok(await page.getByTestId("rappel").filter({ hasText: "Rappeler pour le prix en gros" }).count() === 1, "le rappel est noté sur la fiche");
await page.getByRole("button", { name: "Retour" }).click();
await page.waitForTimeout(800);

// ---- Mes rappels ----
await page.getByRole("tab", { name: "Mes rappels" }).click();
await page.waitForTimeout(1000);
ok(await page.getByTestId("mes-rappels").getByText("Rappeler pour le prix en gros").count() === 1, "le rappel attribué au patron apparaît dans ses rappels");
await capture(page, "h2-01-mes-rappels");

await page.getByTestId("mes-rappels").getByRole("button", { name: /Marquer fait/ }).click();
await page.waitForTimeout(1000);
ok(await page.getByTestId("mes-rappels").getByText("Rappeler pour le prix en gros").count() === 0, "fait, il sort de la liste active");

// ---- La premiere vente rend le prospect client ----
const { prospectId } = await page.evaluate(async ({ nomProspect }) => {
  const { appeler } = await import("/src/lib/pont.ts");
  const prospects = await appeler("lire_prospects");
  const p = prospects.find(x => x.nom === nomProspect);
  return { prospectId: p.id };
}, { nomProspect });

await page.getByRole("tab", { name: "Prospects" }).click();
await page.waitForTimeout(800);
ok(await page.getByTestId("prospect").filter({ hasText: nomProspect }).count() === 1, "encore prospect avant sa première vente");

await page.evaluate(async ({ prospectId }) => {
  const { appeler } = await import("/src/lib/pont.ts");
  const depots = await appeler("lire_depots_detail");
  const depot = depots.find(d => d.est_defaut) ?? depots[0];
  const articles = await appeler("lire_articles_avec_unites");
  const sucre = articles.find(a => a.nom === "Sucre") ?? articles[0];
  const unite = sucre.unites[0];
  await appeler("creer_vente", {
    clientId: prospectId, depotId: depot.id, modeReglement: "credit",
    lignes: [{
      article_id: sucre.id, unite_vente_id: unite.id, depot_source_id: depot.id,
      source_approvisionnement: "stock", quantite: 1, facteur: unite.facteur,
      prix_reference: unite.prix_reference, prix_pratique: unite.prix_reference,
    }],
  });
}, { prospectId });

await page.reload();
await page.waitForTimeout(1500);
await page.getByRole("navigation", { name: "Menu Équipe" }).getByRole("button", { name: /Suivi clients/ }).click();
await page.waitForTimeout(800);
await page.getByRole("tab", { name: "Prospects" }).click();
await page.waitForTimeout(1000);
ok(await page.getByTestId("prospect").filter({ hasText: nomProspect }).count() === 0, "sa première vente l'a rendu client : il ne reparaît plus comme prospect");

await page.getByRole("tab", { name: "Clients" }).click();
await page.waitForTimeout(500);
await page.getByLabel("Rechercher un client").fill(nomProspect);
await page.waitForTimeout(1000);
ok(await page.getByTestId("resultat-client").filter({ hasText: nomProspect }).count() === 1, "et se retrouve dans les clients, sans avoir touché la caisse à la main");

const utiles = erreursUtiles(erreurs);
ok(utiles.length === 0, "aucune erreur dans la console" + (utiles.length ? " : " + utiles.join(" | ") : ""));
console.log(`${ok.total()} vérifications`);
await b.close();
