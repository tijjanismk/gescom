// C-3 : les plafonds se reglent a l'ecran (role, personne) et le
// serveur les applique : une remise de 40 % d'un caissier est refusee
// avec « Demander au patron » ; le patron passe.
import { navigateur, connecter, capture, parametres, verifieur, erreursUtiles } from "./pw.mjs";
const { b, page: patron, erreurs } = await navigateur();
const ok = verifieur();
// Les refus ATTENDUS (plafond, caisse deja ouverte) laissent un 409 dans
// la console : ce n'est pas une erreur de l'ecran.
const attendu = async (f) => { const n = erreurs.length; const r = await f(); erreurs.splice(n); return r; };
await connecter(patron);
const s = Date.now() % 100000;
const pseudo = `awa${s}`;
await patron.evaluate(async ({ pseudo, s }) => {
  const { appeler } = await import("/src/lib/pont.ts");
  await appeler("creer_utilisateur", { nom: `Awa ${s}`, pseudo, email: null, motDePasse: "banc-secret", roleNom: "caissier" });
}, { pseudo, s });

// ---- Parametres -> Roles : le caissier plafonne a 15 % / 50 000 F.
await parametres(patron, "Rôles");
const bloc = patron.getByTestId("plafonds-caissier");
ok(await bloc.count() === 1, "le rôle caissier a ses plafonds");
ok(await patron.getByTestId("plafonds-patron").count() === 0, "le patron (accès complet) n'en a pas");
await bloc.getByLabel("Remise max. caissier").fill("15");
await bloc.getByLabel("Remboursement max. caissier").fill("50 000");
await bloc.getByLabel("Crédit max. par vente caissier").fill("");
await bloc.getByRole("button", { name: "Enregistrer les plafonds caissier" }).click();
await patron.waitForTimeout(1000);
ok((await bloc.getByRole("status").innerText()) === "Plafonds enregistrés.", "enregistrés");
await capture(patron, "c3-01-roles");
await patron.reload(); await patron.waitForTimeout(2500);
await parametres(patron, "Rôles");
ok(await patron.getByTestId("plafonds-caissier").getByLabel("Remise max. caissier").inputValue() === "15", "relus après rechargement");
ok(await patron.getByTestId("plafonds-caissier").getByLabel("Remboursement max. caissier").inputValue() === "50000", "le montant relu");

// Une saisie absurde est refusee par le serveur, message a l'ecran.
const bloc2 = patron.getByTestId("plafonds-caissier");
await bloc2.getByLabel("Remise max. caissier").fill("150");
await attendu(async () => {
  await bloc2.getByRole("button", { name: "Enregistrer les plafonds caissier" }).click();
  await patron.waitForTimeout(1000);
});
ok(/entre 0 et 100/.test(await bloc2.getByRole("status").innerText()), "150 % : refusé, le serveur le dit");
await bloc2.getByLabel("Remise max. caissier").fill("15");
await bloc2.getByRole("button", { name: "Enregistrer les plafonds caissier" }).click();
await patron.waitForTimeout(800);

// ---- La caissiere, dans un autre navigateur.
const ctx = await b.newContext({ viewport: { width: 1280, height: 800 }, locale: "fr-FR" });
const caisse = await ctx.newPage();
await connecter(caisse, pseudo, "banc-secret", "banc-secret");
const essai = async (page, remise) => attendu(() => page.evaluate(async remise => {
  const { appeler } = await import("/src/lib/pont.ts");
  const arts = await appeler("lire_articles_avec_unites", {});
  const a = arts.find(x => x.unites?.length);
  const u = a.unites[0];
  const depot = (await appeler("lire_depot_defaut", {})).id;
  const client = (await appeler("lire_client_generique", {})).id;
  const pratique = Math.round(u.prix_reference * (100 - remise) / 100);
  try { await appeler("ouvrir_session_caisse", { fondOuverture: 0 }); } catch { /* deja ouverte */ }
  try {
    await appeler("creer_vente", {
      clientId: client, depotId: depot, modeReglement: "comptant", montantPaye: pratique, modePaiement: "especes",
      lignes: [{ article_id: a.id, unite_vente_id: u.id, depot_source_id: depot, source_approvisionnement: "stock",
                 quantite: 1, facteur: u.facteur, prix_reference: u.prix_reference, prix_pratique: pratique }],
    });
    return "vendu";
  } catch (e) { return String(e); }
}, remise));
const refus = await essai(caisse, 40);
ok(refus.includes("Remise de 40 % — votre plafond est 15 %. Demander au patron."), `40 % refusés : « ${refus.slice(0, 70)} »`);
ok(await essai(caisse, 10) === "vendu", "10 % : vendu");
ok(await essai(patron, 40) === "vendu", "le patron passe, avec son compte");

// ---- Le sur-mesure d'une personne : 25 % pour elle seule.
await parametres(patron, "Utilisateurs");
await patron.locator("div").filter({ hasText: `Awa ${s}` })
  .filter({ has: patron.getByRole("button", { name: "Permissions" }) })
  .last().getByRole("button", { name: "Permissions" }).click();
await patron.waitForTimeout(1200);
const perso = patron.getByTestId(`plafonds-Awa ${s}`);
await perso.getByLabel(`Remise max. Awa ${s}`).fill("25");
await perso.getByRole("button", { name: `Enregistrer les plafonds Awa ${s}` }).click();
await patron.waitForTimeout(1000);
ok((await perso.getByRole("status").innerText()) === "Plafonds enregistrés.", "sur-mesure enregistré");
await capture(patron, "c3-02-personne");
ok(await essai(caisse, 20) === "vendu", "20 % : permis pour elle (25 %)");

const utiles = erreursUtiles(erreurs);
ok(utiles.length === 0, "aucune erreur dans la console" + (utiles.length ? " : " + utiles.join(" | ") : ""));
console.log(`${ok.total()} vérifications`);
await b.close();
