// H-1 : Equipe -> Suivi clients. Un echange note et relu sur la fiche
// 360 d'un client, fondu avec ses relances de creance existantes (D34 :
// pas une liste de plus) ; les chiffres (CA, ventes) s'affichent.
import { navigateur, connecter, capture, verifieur, erreursUtiles, URL_EQUIPE } from "./pw.mjs";
const { b, page, erreurs } = await navigateur();
const ok = verifieur();

await connecter(page, "admin", "admin123", "Admin-2026!", "Ma boutique", URL_EQUIPE);
erreurs.splice(0);

// Une vente a credit pour avoir un client "reel" avec une creance, et
// une relance dessus — pour verifier qu'elle apparait dans le fil.
const { venteId, clientNom } = await page.evaluate(async () => {
  const { appeler } = await import("/src/lib/pont.ts");
  const clients = await appeler("lire_clients_pagines", { page: 0, limite: 5, recherche: null, avecCreancesSeulement: false, ventesFiltre: null, tri: null });
  const client = clients.donnees[0];
  const depots = await appeler("lire_depots_detail");
  const depot = depots.find(d => d.est_defaut) ?? depots[0];
  const articles = await appeler("lire_articles_avec_unites");
  const sucre = articles.find(a => a.nom === "Sucre") ?? articles[0];
  const unite = sucre.unites[0];
  const vente = await appeler("creer_vente", {
    clientId: client.id, depotId: depot.id, modeReglement: "credit",
    lignes: [{
      article_id: sucre.id, unite_vente_id: unite.id, depot_source_id: depot.id,
      source_approvisionnement: "stock", quantite: 2, facteur: unite.facteur,
      prix_reference: unite.prix_reference, prix_pratique: unite.prix_reference,
    }],
  });
  await appeler("enregistrer_relance", { venteId: vente.vente_id, canal: "whatsapp", note: "Rappel amical" });
  return { venteId: vente.vente_id, clientNom: client.nom };
});
ok(!!venteId, "une vente a credit et une relance sont posees pour le test");

await page.getByRole("navigation", { name: "Menu Équipe" }).getByRole("button", { name: /Suivi clients/ }).click();
await page.waitForTimeout(1000);
ok(await page.getByRole("tab", { name: "Clients" }).count() === 1, "l'onglet Clients est là");

await page.getByLabel("Rechercher un client").fill(clientNom);
await page.waitForTimeout(1000);
await page.getByTestId("resultat-client").filter({ hasText: clientNom }).first().click();
await page.waitForTimeout(1200);
ok(await page.getByTestId("fiche-360").count() === 1, "la fiche 360 s'ouvre");
ok((await page.getByTestId("fiche-360").innerText()).includes(clientNom), "le nom du client y est");

// La relance de creance deja la se voit dans le meme fil.
ok(await page.getByTestId("echange").filter({ hasText: "Rappel amical" }).count() === 1, "la relance de créance apparaît dans le fil des échanges");
ok(await page.getByTestId("echange").filter({ hasText: "Rappel amical" }).getByText("relance de créance").count() === 1, "annotée comme une relance");

// Un nouvel echange, note depuis la fiche.
await page.getByLabel("Genre").selectOption("visite");
await page.getByLabel("Ce qui s'est passé").fill("Passée au magasin, veut du sucre en gros");
await page.getByLabel("Suite prévue").fill("Rappeler vendredi avec un prix");
await page.getByRole("button", { name: "Noter" }).click();
await page.waitForTimeout(1200);
ok(await page.getByTestId("echange").filter({ hasText: "Passée au magasin" }).count() === 1, "le nouvel échange est noté");
ok((await page.getByTestId("echange").filter({ hasText: "Passée au magasin" }).innerText()).includes("Rappeler vendredi avec un prix"), "sa suite prévue s'affiche");
ok(await page.getByTestId("echange").count() === 2, "les deux se suivent dans le même fil : pas une liste de plus");

// Les chiffres du client sont là.
ok(/\d[\d\s]* F/.test(await page.getByText("Chiffre d'affaires").locator("..").innerText()), "le chiffre d'affaires s'affiche");

await capture(page, "h1-01-fiche-360");

const utiles = erreursUtiles(erreurs);
ok(utiles.length === 0, "aucune erreur dans la console" + (utiles.length ? " : " + utiles.join(" | ") : ""));
console.log(`${ok.total()} vérifications`);
await b.close();
