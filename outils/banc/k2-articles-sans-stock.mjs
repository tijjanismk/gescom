// D36 (26/10/2026) : un article peut ne pas suivre son stock.
// Parametres → Articles : un « Sachet » cree case « Suivre le stock »
// decochee ; il se vend (au comptoir, paye) ; l'ecran Stock ne le montre
// pas, « Non suivis » le nomme ; transfert et inventaire le refusent ;
// « Suivre le stock » demande le comptage de chaque magasin et le stock
// repart de la quantite comptee.
//
//   serveur (sqlite ou multiposte) : node k2-articles-sans-stock.mjs
import { navigateur, connecter, parametres, capture, verifieur, erreursUtiles } from "./pw.mjs";
const { b, page, erreurs } = await navigateur();
const ok = verifieur();
await connecter(page);
const s = Date.now() % 100000;
const nom = `Sachet banc ${s}`;
const attendu = async (f) => { const n = erreurs.length; const r = await f(); erreurs.splice(n); return r; };

// ---- Creer l'article, case decochee.
await parametres(page, "Articles");
await page.getByRole("button", { name: /Nouvel article/ }).click();
const boite = page.getByRole("dialog");
await boite.getByPlaceholder("Nom de l'article").fill(nom);
await boite.getByPlaceholder("0").first().fill("50");
await boite.getByRole("checkbox").uncheck();
await boite.getByRole("button", { name: /Créer|Enregistrer|Ajouter/ }).last().click();
await page.waitForTimeout(2000);
const ligne = page.locator("main").getByText(nom, { exact: true }).locator("xpath=..");
ok((await ligne.innerText().catch(() => "")).includes("stock non suivi"), "créé case décochée : « stock non suivi »");
await capture(page, "k2-01-article");

// ---- Il se vend, sans decouvert ; transfert et inventaire le refusent.
const r = await attendu(() => page.evaluate(async (nom) => {
  const { appeler } = await import("/src/lib/pont.ts");
  try { await appeler("ouvrir_session_caisse", { fondOuverture: 0 }); } catch { /* deja ouverte */ }
  const art = (await appeler("lire_articles_avec_unites", {})).find(a => a.nom === nom);
  const u = art.unites[0];
  const depot = (await appeler("lire_depot_defaut", {})).id;
  const comptant = (await appeler("lire_client_generique", {})).id;
  const v = await appeler("creer_vente", { clientId: comptant, depotId: depot, modeReglement: "comptant",
    montantPaye: 3 * u.prix_reference, modePaiement: "especes",
    lignes: [{ article_id: art.id, unite_vente_id: u.id, depot_source_id: depot, source_approvisionnement: "stock",
      quantite: 3, facteur: u.facteur, prix_reference: u.prix_reference, prix_pratique: u.prix_reference }] });
  const refus = async (f) => { try { await f(); return null; } catch (e) { return String(e); } };
  return {
    suivi: art.gere_en_stock, statut: v.statut, art, depot,
    transfert: await refus(() => appeler("enregistrer_transfert", { depotSource: depot, depotDest: "ailleurs",
      lignes: [{ article_id: art.id, unite_vente_id: u.id, quantite: 1, facteur: 1 }], motif: null })),
    inventaire: await refus(() => appeler("enregistrer_ajustement_inventaire", { articleId: art.id, depotId: depot, quantiteReelle: 10, motif: null })),
  };
}, nom));
ok(r.suivi === false, "la caisse sait qu'il ne suit pas son stock");
ok(r.statut === "payee", "il se vend, payé");
ok(/ne suit pas son stock/.test(r.transfert ?? ""), "transfert refusé");
ok(/ne suit pas son stock/.test(r.inventaire ?? ""), "inventaire refusé");

// ---- L'ecran Stock : absent de la liste, nomme par « Non suivis ».
await page.getByRole("navigation").getByText("Stock", { exact: true }).first().click();
await page.waitForTimeout(1500);
await page.getByPlaceholder("Rechercher un article...").fill("Sachet banc");
await page.waitForTimeout(1500);
ok(!(await page.locator("main").innerText()).includes(nom), "pas dans la liste du stock");
await page.getByRole("button", { name: "Non suivis", exact: true }).click();
await page.waitForTimeout(1500);
ok(await page.getByTestId("article-non-suivi").filter({ hasText: nom }).count() === 1, "« Non suivis » le nomme");
await capture(page, "k2-02-non-suivis");

// ---- Suivre le stock : compter chaque magasin.
await parametres(page, "Articles");
await page.locator("main").getByText(nom, { exact: true }).click();
await page.waitForTimeout(800);
await page.getByTestId("suivi-stock").getByRole("button", { name: "Suivre le stock" }).click();
const compte = page.getByRole("dialog");
await compte.waitFor({ timeout: 5000 });
const champs = compte.getByPlaceholder("Quantité comptée");
const n = await champs.count();
ok(n >= 1, `un champ par magasin (${n})`);
ok(await compte.getByRole("button", { name: "Suivre le stock" }).isDisabled(), "tant qu'un magasin n'est pas compté : fermé");
for (let i = 0; i < n; i++) await champs.nth(i).fill(i === 0 ? "12" : "0");
await compte.getByRole("button", { name: "Suivre le stock" }).click();
await page.waitForTimeout(2000);
ok((await page.getByTestId("suivi-stock").innerText().catch(() => "")).includes("Stock suivi"), "« Stock suivi »");
const stock = await page.evaluate(async ({ id, depot }) => {
  const { appeler } = await import("/src/lib/pont.ts");
  return (await appeler("lire_stock_article_depots", { articleId: id })).find(m => m.depot_id === depot)?.quantite;
}, { id: r.art.id, depot: r.depot });
ok(stock === 12, `le stock repart de la quantité comptée (${stock})`);

const utiles = erreursUtiles(erreurs);
ok(utiles.length === 0, "aucune erreur dans la console" + (utiles.length ? " : " + utiles.join(" | ") : ""));
console.log(`${ok.total()} vérifications`);
await b.close();
