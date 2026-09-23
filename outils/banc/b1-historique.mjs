// B-1 : l'Historique, par l'ecran. Un reglement annule se retrouve par
// le nom du client ; filtres ; ouverture depuis une fiche client, une
// piece, un article ; un employe ne voit pas l'entree de menu.
import { navigateur, connecter, capture, verifieur, erreursUtiles, URL } from "./pw.mjs";
const { b, ctx, page, erreurs } = await navigateur();
const ok = verifieur();
await connecter(page);

// ---- Le geste a retrouver, fait par l'API comme le ferait la caisse.
// Deux refus sont ATTENDUS (caisse deja ouverte, employe sans droit) :
// leur 409 dans la console n'est pas une erreur de l'ecran.
const attendu = async (f) => { const n = erreurs.length; const r = await f(); erreurs.splice(n); return r; };
const motif = `Erreur banc B1 n°${Date.now() % 100000}`;
const geste = await attendu(() => page.evaluate(async (motif) => {
  const { appeler } = await import("/src/lib/pont.ts");
  try { await appeler("ouvrir_session_caisse", { fondOuverture: 0 }); } catch { /* deja ouverte */ }
  const articles = await appeler("lire_articles_avec_unites", {});
  const a = articles.find(x => x.unites?.length) ?? articles[0];
  const u = a.unites[0];
  const depot = (await appeler("lire_depot_defaut", {})).id;
  const clients = await appeler("lire_clients", {});
  const c = clients.find(x => !x.est_generique);
  const v = await appeler("creer_vente", {
    clientId: c.id, depotId: depot, modeReglement: "credit",
    lignes: [{
      article_id: a.id, unite_vente_id: u.id, depot_source_id: depot,
      source_approvisionnement: "stock", quantite: 2, facteur: u.facteur,
      prix_reference: u.prix_reference, prix_pratique: u.prix_reference,
    }],
  });
  await appeler("regler_creance", { venteId: v.vente_id, montant: 300, mode: "especes" });
  const regs = await appeler("lire_reglements_client", { clientId: c.id });
  const p = regs.find(r => r.montant === 300 && !r.deja_annule && !r.est_annulation);
  await appeler("annuler_reglement", { paiementId: p.id, motif, remboursement: false });
  // Une piece, dupliquee : la copie porte un evenement a elle.
  const devis = await appeler("creer_piece", {
    clientId: c.id, typePiece: "devis",
    lignes: [{ article_id: a.id, unite_vente_id: u.id, quantite: 1, prix_unitaire: u.prix_reference, remise_pct: 0, taux_tva: 0 }],
  });
  const copie = await appeler("dupliquer_piece", { pieceId: devis.id });
  return { client: c.nom, article: a.nom, copie: copie.numero };
}, motif));
ok(!!geste.client, `règlement de ${geste.client} saisi puis annulé`);

// ---- Le menu, la recherche par le nom.
await page.getByText("Historique", { exact: true }).first().click();
await page.waitForTimeout(1200);
ok(await page.getByRole("heading", { name: "Historique" }).count() === 1, "l'Historique s'ouvre depuis le menu");
ok(await page.getByTestId("ligne-historique").count() > 0, "des lignes, sans filtre");

const morceau = geste.client.slice(0, 5).toLowerCase();
await page.getByLabel("Recherche").fill(morceau);
await page.waitForTimeout(1200);
const annulation = page.getByTestId("ligne-historique").filter({ hasText: "Règlement annulé" }).filter({ hasText: motif });
ok(await annulation.count() === 1, `« ${morceau} » retrouve l'annulation`);
const texte = (await annulation.first().innerText()) || "";
ok(texte.includes(geste.client), "la ligne nomme le client (sur quoi)");
ok(texte.includes(motif), "la ligne dit le motif (après)");
ok(texte.includes("300 F"), "et le montant, en francs (avant)");
ok(/\d{2}\/\d{2}\/\d{4} \d{2}:\d{2}/.test(texte), "et quand");
await capture(page, "b1-01-recherche");

// ---- Type, dates, tout effacer.
await page.getByLabel("Recherche").fill("");
await page.getByLabel("Type").selectOption({ label: "Règlement annulé" });
await page.waitForTimeout(1200);
const n = await page.getByTestId("ligne-historique").count();
const nAnnul = await page.getByTestId("ligne-historique").filter({ hasText: "Règlement annulé" }).count();
ok(n >= 1 && n === nAnnul, "le filtre par type ne garde que ce type");
await page.getByLabel("Du").fill("2000-01-01");
await page.getByLabel("Au").fill("2000-01-02");
await page.waitForTimeout(1200);
ok(await page.getByText("Rien ne correspond à ces filtres.").count() === 1, "un jour sans rien le dit");
await page.getByRole("button", { name: "Tout effacer" }).click();
await page.waitForTimeout(1200);
ok(await page.getByTestId("ligne-historique").count() > n, "tout effacer rend tout");

// ---- Depuis la fiche client.
await page.getByText("Clients", { exact: true }).first().click();
await page.waitForTimeout(1500);
await page.locator("div").filter({ hasText: geste.client })
  .filter({ has: page.getByRole("button", { name: "Fiche", exact: true }) })
  .last().getByRole("button", { name: "Fiche", exact: true }).click();
await page.waitForTimeout(1500);
await page.getByRole("main").getByRole("button", { name: "Historique" }).click();
await page.waitForTimeout(1500);
ok(await page.getByTestId("filtre-contexte").innerText() === `Client : ${geste.client}`, "ouvert filtré sur le client");
const lignesClient = page.getByTestId("ligne-historique");
const nc = await lignesClient.count();
let tous = nc > 0;
for (let i = 0; i < nc; i++) tous &&= (await lignesClient.nth(i).innerText()).includes(geste.client);
ok(tous, `les ${nc} lignes touchent toutes ce client`);
await capture(page, "b1-02-fiche-client");
await page.getByRole("button", { name: "Retirer ce filtre" }).click();
await page.waitForTimeout(1000);
ok(await page.getByTestId("filtre-contexte").count() === 0, "le filtre se retire d'un clic");
await page.getByRole("button", { name: "Retour", exact: true }).click();
await page.waitForTimeout(1500);
ok(await page.getByText(geste.client).count() > 0 && await page.getByRole("button", { name: "Nouvelle pièce" }).count() === 1,
  "Retour ramène à la fiche");

// ---- Depuis une piece.
await page.getByText("Pièces", { exact: true }).first().click();
await page.waitForTimeout(1500);
const bouton = page.getByRole("button", { name: `Historique ${geste.copie}`, exact: true });
ok(await bouton.count() === 1, "la pièce a son bouton Historique");
await bouton.click();
await page.waitForTimeout(1500);
ok(await page.getByTestId("filtre-contexte").innerText() === `Pièce : ${geste.copie}`, "ouvert filtré sur la pièce");
ok(await page.getByTestId("ligne-historique").filter({ hasText: "Pièce dupliquée" }).count() === 1, "sa duplication y est");

// ---- Depuis un article.
await page.getByText("Stock", { exact: true }).first().click();
await page.waitForTimeout(1500);
const ligneStock = page.locator("div.group").first();
await ligneStock.hover();
await ligneStock.getByRole("button", { name: "Historique" }).click();
await page.waitForTimeout(1500);
ok((await page.getByTestId("filtre-contexte").innerText()).startsWith("Article : "), "ouvert filtré sur l'article");
await capture(page, "b1-03-article");

// ---- Un employe n'a pas l'entree.
const p2 = await ctx.newPage();
// Meme navigateur : on oublie la session du patron avant de se connecter.
await p2.goto(URL);
await p2.evaluate(() => localStorage.clear());
await connecter(p2, "employe", "employe123", "Employe-2026!");
await p2.waitForTimeout(800);
ok(await p2.getByText("Tableau de bord").count() > 0, "l'employé est connecté");
ok(await p2.getByText("Historique", { exact: true }).count() === 0, "l'employé ne voit pas l'Historique");
const refus = await attendu(() => p2.evaluate(async () => {
  const { appeler } = await import("/src/lib/pont.ts");
  try { await appeler("lire_historique", { filtre: {} }); return "accepté"; } catch (e) { return String(e); }
}));
ok(refus.includes("journal:lire") || /refus|permission/i.test(refus), `le serveur refuse l'employé (${refus.slice(0, 60)}…)`);

const utiles = erreursUtiles(erreurs);
ok(utiles.length === 0, "aucune erreur dans la console" + (utiles.length ? " : " + utiles.join(" | ") : ""));
console.log(`${ok.total()} vérifications`);
await b.close();
