// F-2 : Equipe -> Personnel. Trois personnes comme dans une boutique :
// Awa au mois sans contrat, Moussa a la journee, Fanta vendeuse a la
// commission avec son compte. On modifie, on fait partir, on fait
// revenir. Qui gere sans preparer la paie ne voit pas les montants.
import { navigateur, connecter, capture, verifieur, erreursUtiles, URL_EQUIPE } from "./pw.mjs";
const { b, page, erreurs } = await navigateur();
const ok = verifieur();
const s = Date.now() % 100000;
const awa = `Awa ${s}`, moussa = `Moussa ${s}`, fanta = `Fanta ${s}`;

await connecter(page, "admin", "admin123", "Admin-2026!", "Ma boutique", URL_EQUIPE);
const idFanta = await page.evaluate(async s => {
  const { appeler } = await import("/src/lib/pont.ts");
  await appeler("creer_utilisateur", { nom: `Fanta ${s}`, pseudo: `fanta${s}`, email: null, motDePasse: "banc-secret", roleNom: "caissier" });
  await appeler("creer_utilisateur", { nom: `Issa ${s}`, pseudo: `issa${s}`, email: null, motDePasse: "banc-secret", roleNom: "magasinier" });
  const us = await appeler("lire_utilisateurs");
  const issa = us.find(u => u.nom === `Issa ${s}`).id;
  // Issa gère le personnel, sans préparer la paie.
  await appeler("definir_permission_utilisateur", { utilisateurId: issa, permission: "personnel:gerer", accorde: true });
  return us.find(u => u.nom === `Fanta ${s}`).id;
}, s);

await page.getByRole("navigation", { name: "Menu Équipe" }).getByRole("button", { name: /Personnel/ }).click();
await page.waitForTimeout(1200);
const statut = () => page.getByRole("status").innerText();

async function nouvelle(nom, fonction, remplir) {
  await page.getByRole("button", { name: /Nouvelle personne/ }).click();
  const f = page.getByTestId("formulaire-employe");
  await f.getByLabel("Nom").fill(nom);
  await f.getByLabel("Ce qu'elle fait").fill(fonction);
  await remplir(f);
  await f.getByRole("button", { name: "Enregistrer" }).click();
  await page.waitForTimeout(1200);
}

await nouvelle(awa, "vendeuse", async f => {
  await f.getByLabel("Au mois").first().check();
  await f.getByRole("textbox", { name: "Au mois" }).fill("60000");
});
ok((await statut()).includes(`${awa} ajouté(e)`), "Awa ajoutée, au mois, sans rien d'autre");
await nouvelle(moussa, "manœuvre", async f => {
  await f.getByLabel("À la journée").first().check();
  await f.getByRole("textbox", { name: "À la journée" }).fill("2500");
});
await nouvelle(fanta, "vendeuse", async f => {
  await f.getByLabel("À la commission").first().check();
  await f.getByRole("textbox", { name: "À la commission" }).fill("2");
  await f.getByText("Plus d'informations").click();
  await f.getByLabel("Compte Gescom").selectOption(idFanta);
  await f.getByLabel("Contrat écrit").check();
});

const ligne = n => page.getByTestId("employe").filter({ hasText: n });
ok((await ligne(awa).innerText()).includes("60 000 F par mois") && (await ligne(awa).innerText()).includes("sans contrat écrit"), "Awa : 60 000 F par mois, sans contrat écrit");
ok((await ligne(moussa).innerText()).includes("2 500 F par jour"), "Moussa : 2 500 F par jour");
const tf = await ligne(fanta).innerText();
ok(tf.includes("2 % des ventes") && tf.includes(`compte Fanta ${s}`) && !tf.includes("sans contrat"), "Fanta : commission, son compte, contrat écrit");
await capture(page, "f2-01-personnel");

// Un refus du serveur, dit à l'écran : le compte de Fanta déjà pris.
// (Le 409 attendu sort de la console.)
const n0 = erreurs.length;
await nouvelle(`Kadia ${s}`, "vendeuse", async f => {
  await f.getByText("Plus d'informations").click();
  await f.getByLabel("Compte Gescom").selectOption(idFanta);
});
ok((await statut()).includes(`déjà celui de ${fanta}`), "un compte déjà lié : refusé, dit");
erreurs.splice(n0);
await page.getByTestId("formulaire-employe").getByRole("button", { name: "Annuler" }).click();

// Modifier Awa.
await ligne(awa).getByRole("button", { name: "Modifier" }).click();
await page.getByTestId("formulaire-employe").getByLabel("Ce qu'elle fait").fill("caissière");
await page.getByTestId("formulaire-employe").getByRole("button", { name: "Enregistrer" }).click();
await page.waitForTimeout(1200);
ok((await ligne(awa).innerText()).includes("caissière"), "Awa devient caissière");

// Moussa part, puis revient.
await page.getByRole("button", { name: `Départ de ${moussa}` }).click();
await page.getByLabel("Motif du départ").fill("Retour au village");
await page.getByRole("button", { name: "Confirmer le départ" }).click();
await page.waitForTimeout(1200);
ok((await statut()).includes("sa fiche reste") && await ligne(moussa).count() === 0, "Moussa parti : hors de la liste");
await page.getByLabel("Voir les départs").check();
await page.waitForTimeout(1000);
ok((await ligne(moussa).innerText()).includes("partie le"), "avec les départs : Moussa, parti");
await page.getByRole("button", { name: `Retour de ${moussa}` }).click();
await page.waitForTimeout(1200);
ok((await statut()).includes("de retour"), "Moussa revient");

// Issa gère le personnel, sans la paie : ni montants ni leur saisie.
const ctx = await b.newContext({ viewport: { width: 1280, height: 800 }, locale: "fr-FR" });
const pi = await ctx.newPage();
pi.on("console", m => { if (m.type() === "error") erreurs.push(m.text()); });
await connecter(pi, `issa${s}`, "banc-secret", "banc-secret2", "Ma boutique", URL_EQUIPE);
await pi.getByRole("navigation", { name: "Menu Équipe" }).getByRole("button", { name: /Personnel/ }).click();
await pi.waitForTimeout(1500);
const vue = await pi.getByTestId("employe").filter({ hasText: awa }).innerText();
ok(vue.includes("au mois") && !vue.includes("60 000"), "Issa voit comment elle est payée, pas combien");
await pi.getByRole("button", { name: /Nouvelle personne/ }).click();
ok((await pi.getByTestId("formulaire-employe").innerText()).includes("Les montants se règlent par qui prépare la paie"), "le formulaire le lui dit");
await capture(pi, "f2-02-sans-montants");

await page.getByText("Accueil", { exact: true }).first().click();
const utiles = erreursUtiles(erreurs);
ok(utiles.length === 0, "aucune erreur dans la console" + (utiles.length ? " : " + utiles.join(" | ") : ""));
console.log(`${ok.total()} vérifications`);
await b.close();
