// C-1 : les permissions de lecture, par l'ecran. Un caissier n'a ni
// Journal, ni Rapports, ni Historique ; son accueil n'a pas de chiffres ;
// il ne voit ni les creances d'un client, ni les pieces fournisseur, ni
// un prix d'achat. Le comptable, lui, lit tout sauf les caisses des autres.
import path from "node:path";
import { DatabaseSync } from "node:sqlite";
import { navigateur, connecter, capture, verifieur, erreursUtiles, TRAVAIL } from "./pw.mjs";
// La demo n'a pas de prix d'achat : on en pose un, comme un achat l'aurait fait.
{
  const base = new DatabaseSync(path.join(TRAVAIL, "essai.db"));
  base.prepare("UPDATE article SET dernier_prix_achat = 800 WHERE nom = 'Sucre'").run();
  base.close();
}
const { b, page: patron, erreurs } = await navigateur();
const ok = verifieur();
await connecter(patron);
const suffixe = Date.now() % 100000;
const comptes = await patron.evaluate(async s => {
  const { appeler } = await import("/src/lib/pont.ts");
  const faire = async (pseudo, nom, role) => {
    try { await appeler("creer_utilisateur", { nom, pseudo, email: null, motDePasse: "banc-secret", roleNom: role }); } catch { /* deja la */ }
    return pseudo;
  };
  return {
    caissier: await faire(`fanta${s}`, `Fanta ${s}`, "caissier"),
    comptable: await faire(`sali${s}`, `Sali ${s}`, "comptable"),
  };
}, suffixe);
ok(await patron.getByText("Rapports", { exact: true }).count() === 1, "le patron a les Rapports");

// ---- Le caissier, dans un autre navigateur.
const ctx = await b.newContext({ viewport: { width: 1440, height: 900 }, locale: "fr-FR" });
const caisse = await ctx.newPage();
caisse.on("pageerror", e => erreurs.push("pageerror: " + e.message));
caisse.on("console", m => { if (m.type() === "error") erreurs.push("console: " + m.text()); });
await connecter(caisse, comptes.caissier, "banc-secret", "banc-secret");
const menu = caisse.getByRole("navigation");
for (const entree of ["Journal", "Rapports", "Historique"]) {
  ok(await menu.getByText(entree, { exact: true }).count() === 0, `caissier : pas de « ${entree} » au menu`);
}
ok(await caisse.getByTestId("accueil-sans-chiffres").count() === 1, "caissier : l'accueil sans chiffres");
await capture(caisse, "c1-01-accueil-caissier");

await menu.getByText("Clients", { exact: true }).click();
await caisse.waitForTimeout(1500);
ok(await caisse.getByRole("button", { name: /^Créances/ }).count() === 0, "caissier : pas d'onglet Créances");
ok(await caisse.getByRole("button", { name: "État des créances" }).count() === 0, "caissier : pas d'état des créances");
ok(await caisse.getByText("Avec créances").count() === 0, "caissier : pas de filtre par dette");
await caisse.getByRole("button", { name: "Fiche", exact: true }).first().click();
await caisse.waitForTimeout(1500);
ok(await caisse.getByText("Nb ventes").count() === 1, "la fiche client s'ouvre");
ok(await caisse.getByText("Encours", { exact: true }).count() === 0, "caissier : pas de tuile Encours");
ok(await caisse.getByRole("button", { name: "État de créance" }).count() === 0, "caissier : pas d'état de créance");
await capture(caisse, "c1-02-fiche-client-caissier");

await menu.getByText("Pièces", { exact: true }).click();
await caisse.waitForTimeout(1200);
ok(await caisse.getByRole("button", { name: /Pièces fournisseur/ }).count() === 0, "caissier : pas de pièces fournisseur");

// Un refus ATTENDU (le cahier du jour) : son 403 n'est pas une erreur d'ecran.
const avant = erreurs.length;
const lu = await caisse.evaluate(async () => {
  const { appeler } = await import("/src/lib/pont.ts");
  const arts = await appeler("lire_articles_avec_unites", {});
  let refus = "";
  try { await appeler("lire_journal_du_jour", { date: new Date().toISOString().slice(0, 10) }); } catch (e) { refus = String(e); }
  return { prix: arts.map(a => a.dernier_prix_achat), refus };
});
erreurs.splice(avant);
ok(lu.prix.every(p => p === null), "caissier : aucun prix d'achat dans le catalogue");
ok(lu.refus.includes("rapports:lire"), "caissier : le cahier du jour est refusé, la permission nommée");

// ---- Le comptable.
const ctx2 = await b.newContext({ viewport: { width: 1440, height: 900 }, locale: "fr-FR" });
const compta = await ctx2.newPage();
await connecter(compta, comptes.comptable, "banc-secret", "banc-secret");
const menu2 = compta.getByRole("navigation");
for (const entree of ["Journal", "Rapports", "Historique"]) {
  ok(await menu2.getByText(entree, { exact: true }).count() === 1, `comptable : « ${entree} » au menu`);
}
ok(await compta.getByTestId("accueil-sans-chiffres").count() === 0, "comptable : le tableau de bord chiffré");
const prixCompta = await compta.evaluate(async () => {
  const { appeler } = await import("/src/lib/pont.ts");
  return (await appeler("lire_articles_avec_unites", {})).some(a => typeof a.dernier_prix_achat === "number");
});
ok(prixCompta, "comptable : le prix d'achat se lit (la permission, plus le nom du rôle)");
await capture(compta, "c1-03-accueil-comptable");

const utiles = erreursUtiles(erreurs);
ok(utiles.length === 0, "aucune erreur dans la console" + (utiles.length ? " : " + utiles.join(" | ") : ""));
console.log(`${ok.total()} vérifications`);
await b.close();
