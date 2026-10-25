// Monoposte (v3) : l'argent, dans la vraie fenetre. Caisse fermee : une
// vente comptant est refusee et rien ne bouge. La caisse s'ouvre PAR
// L'ECRAN (Caisse → Ouvrir la caisse, 10 000 F de fond). Une vente
// comptant de deux unites : le stock baisse de deux, la caisse monte du
// prix. Une vente a credit : la creance du client monte ; son reglement
// la ramene et l'argent entre. La fermeture tombe juste (ecart 0).
//
//   apres p1 : node p3-argent.mjs
import { fenetre, boitesMuettes, connecter, capture, verifieur, erreursUtiles, boites, URL_FENETRE } from "./pw.mjs";
const ok = verifieur();
const { b, page, erreurs } = await fenetre();
await boitesMuettes(page);
// Repartir d'une page propre : une boite laissee ouverte par un passage
// precedent masquerait le menu.
await page.reload();
await page.waitForTimeout(2500);
if (await page.getByText("Brancher ce poste").count()) throw new Error("La fenêtre n'est pas branchée : lancer p1 d'abord.");
if (!(await page.getByText("Tableau de bord", { exact: true }).count())) {
  await connecter(page, "admin", "Admin-2026!", "Admin-2026!", "Ma boutique", URL_FENETRE);
}
erreurs.splice(0);
const attendu = async (f) => { const n = erreurs.length; const r = await f(); erreurs.splice(n); return r; };
const api = (f, arg) => page.evaluate(f, arg);

// ---- La preparation : un article en stock, aucune caisse ouverte.
const prep = await attendu(() => api(async () => {
  const { appeler } = await import("/src/lib/pont.ts");
  for (const s of (await appeler("lire_sessions_caisse", {})).filter(x => x.statut === "ouverte")) {
    await appeler("fermer_session_caisse", { sessionId: s.id, especesComptees: s.fond_ouverture + s.entrees_especes - s.sorties_especes });
  }
  const depot = (await appeler("lire_depot_defaut", {})).id;
  const art = (await appeler("lire_articles_avec_unites", {})).find(x => x.unites?.length && x.stock >= 10);
  const u = art.unites.find(x => x.facteur === 1) ?? art.unites[0];
  const generique = (await appeler("lire_client_generique", {})).id;
  const client = (await appeler("lire_clients", {})).find(c => c.code === "CLIENT00001");
  return { depot, art, u, generique, client };
}));
ok(!!prep.art && !!prep.client, `un article en stock (${prep.art?.nom}, ${prep.art?.stock}) et un client de la démo (${prep.client?.nom})`);
const ligne = (q) => ({ article_id: prep.art.id, unite_vente_id: prep.u.id, depot_source_id: prep.depot, source_approvisionnement: "stock",
  quantite: q, facteur: prep.u.facteur, prix_reference: prep.u.prix_reference, prix_pratique: prep.u.prix_reference });
const stock = () => api(async (id) => {
  const { appeler } = await import("/src/lib/pont.ts");
  return (await appeler("lire_articles_avec_unites", {})).find(x => x.id === id).stock;
}, prep.art.id);

// ---- Caisse fermee : refus, rien ne bouge.
const refus = await attendu(() => api(async ({ prep, l }) => {
  const { appeler } = await import("/src/lib/pont.ts");
  try {
    await appeler("creer_vente", { clientId: prep.generique, depotId: prep.depot, modeReglement: "comptant",
      montantPaye: prep.u.prix_reference, modePaiement: "especes", lignes: [l] });
    return null;
  } catch (e) { return String(e); }
}, { prep, l: ligne(1) }));
ok(!!refus && /caisse/i.test(refus), `caisse fermée : vente refusée (« ${refus} »)`);
ok(await stock() === prep.art.stock, "le stock n'a pas bougé");

// ---- La caisse s'ouvre par l'ecran.
await page.getByRole("navigation").getByText("Caisse", { exact: true }).first().click();
await page.waitForTimeout(1500);
ok((await page.locator("main").innerText()).includes("Aucune session ouverte"), "l'écran Caisse dit qu'aucune session n'est ouverte");
await page.locator("main").getByRole("button", { name: "Ouvrir", exact: true }).click();
const dialogue = page.getByRole("dialog");
await dialogue.waitFor({ timeout: 5000 });
await dialogue.getByPlaceholder("0").fill("10000");
await dialogue.getByRole("button", { name: "Ouvrir", exact: true }).click();
await page.waitForTimeout(2000);
const session = await api(async () => {
  const { appeler } = await import("/src/lib/pont.ts");
  return (await appeler("lire_sessions_caisse", {})).find(x => x.statut === "ouverte");
});
ok(!!session && session.fond_ouverture === 10000, `ouverte par l'écran, fond de ${session?.fond_ouverture} F`);
await capture(page, "p3-01-caisse-ouverte");

// ---- Vente comptant de deux unites.
await api(async ({ prep, l }) => {
  const { appeler } = await import("/src/lib/pont.ts");
  await appeler("creer_vente", { clientId: prep.generique, depotId: prep.depot, modeReglement: "comptant",
    montantPaye: 2 * prep.u.prix_reference, modePaiement: "especes", lignes: [l] });
}, { prep, l: ligne(2) });
const caisse = () => api(async (id) => {
  const { appeler } = await import("/src/lib/pont.ts");
  return (await appeler("lire_sessions_caisse", {})).find(x => x.id === id);
}, session.id);
let s1 = await caisse();
ok(await stock() === prep.art.stock - 2 * prep.u.facteur, `stock ${prep.art.stock} → ${prep.art.stock - 2 * prep.u.facteur}`);
ok(s1.entrees_especes === 2 * prep.u.prix_reference, `caisse : +${s1.entrees_especes} F (2 × ${prep.u.prix_reference})`);

// ---- Vente a credit, puis son reglement.
const credit = await api(async ({ prep, l }) => {
  const { appeler } = await import("/src/lib/pont.ts");
  const du = async () => (await appeler("lire_etat_creances_client", { clientId: prep.client.id })).total_du;
  const avant = await du();
  const v = await appeler("creer_vente", { clientId: prep.client.id, depotId: prep.depot, modeReglement: "credit", lignes: [l] });
  const pendant = await du();
  await appeler("regler_creance", { venteId: v.vente_id, montant: v.total, mode: "especes" });
  return { avant, pendant, apres: await du(), total: v.total };
}, { prep, l: ligne(1) });
ok(credit.pendant === credit.avant + credit.total, `à crédit : créance ${credit.avant} → ${credit.pendant} F`);
ok(credit.apres === credit.avant, `réglée : créance revenue à ${credit.apres} F`);
const s2 = await caisse();
ok(s2.entrees_especes === s1.entrees_especes + credit.total, `le règlement est entré en caisse (+${credit.total} F)`);

// ---- La fermeture tombe juste.
const fin = await api(async (s) => {
  const { appeler } = await import("/src/lib/pont.ts");
  await appeler("fermer_session_caisse", { sessionId: s.id, especesComptees: s.fond_ouverture + s.entrees_especes - s.sorties_especes });
  return (await appeler("lire_sessions_caisse", {})).find(x => x.id === s.id);
}, s2);
ok(fin.statut !== "ouverte" && fin.ecart === 0, `fermée, écart ${fin.ecart} F (théorique ${fin.solde_theorique} F)`);

const b0 = await boites(page);
ok(b0.length === 0, "aucune boîte d'erreur ne s'est ouverte" + (b0.length ? " : " + b0.join(" | ") : ""));
const utiles = erreursUtiles(erreurs);
ok(utiles.length === 0, "aucune erreur dans la console" + (utiles.length ? " : " + utiles.join(" | ") : ""));
console.log(`${ok.total()} vérifications`);
await b.close();
