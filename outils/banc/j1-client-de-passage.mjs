// D35 (25/10/2026) : chaque vente sans nom a sa fiche « Client de
// passage ». Au comptoir, comme l'ecran le fait : une vente au
// « Comptant » et sa facture — le serveur ouvre la fiche PAS-…, la
// facture est a elle. Une vente au passant payee en partie est refusee,
// le stock ne bouge pas. Clients : la fiche n'est pas dans la liste ;
// « Passages » la retrouve par le numero du ticket ; « Passer en client
// ordinaire » (nom, telephone) — il entre dans la liste, sa facture
// garde « Client de passage ».
//
//   serveur (sqlite ou multiposte) : node j1-client-de-passage.mjs
import { navigateur, connecter, capture, verifieur, erreursUtiles } from "./pw.mjs";
const { b, page, erreurs } = await navigateur();
const ok = verifieur();
await connecter(page);
const s = Date.now() % 100000;
const attendu = async (f) => { const n = erreurs.length; const r = await f(); erreurs.splice(n); return r; };

// ---- Au comptoir : une vente au « Comptant », puis sa facture.
const vente = await attendu(() => page.evaluate(async () => {
  const { appeler } = await import("/src/lib/pont.ts");
  try { await appeler("ouvrir_session_caisse", { fondOuverture: 0 }); } catch { /* deja ouverte */ }
  const comptant = await appeler("lire_client_generique", {});
  const depot = (await appeler("lire_depot_defaut", {})).id;
  const art = (await appeler("lire_articles_avec_unites", {})).find(x => x.unites?.length && x.stock >= 5);
  const u = art.unites.find(x => x.facteur === 1) ?? art.unites[0];
  const ligne = (q) => [{ article_id: art.id, unite_vente_id: u.id, depot_source_id: depot, source_approvisionnement: "stock",
    quantite: q, facteur: u.facteur, prix_reference: u.prix_reference, prix_pratique: u.prix_reference }];
  const stock = async () => (await appeler("lire_articles_avec_unites", {})).find(x => x.id === art.id).stock;
  // Payee en partie : refusee, rien ne sort.
  const avant = await stock();
  let refus = null;
  try {
    await appeler("creer_vente", { clientId: comptant.id, depotId: depot, modeReglement: "comptant",
      montantPaye: u.prix_reference, modePaiement: "especes", lignes: ligne(2) });
  } catch (e) { refus = String(e); }
  const apresRefus = await stock();
  // Payee en entier : la vente, puis la facture (l'ecran passe le « Comptant »).
  const v = await appeler("creer_vente", { clientId: comptant.id, depotId: depot, modeReglement: "comptant",
    montantPaye: 2 * u.prix_reference, modePaiement: "especes", lignes: ligne(2) });
  const f = await appeler("creer_facture_depuis_vente", { venteId: v.vente_id, clientId: comptant.id, modeReglement: "comptant" });
  const donnees = await appeler("lire_donnees_piece", { pieceId: f.piece_id });
  return { comptant, refus, avant, apresRefus, apres: await stock(), v, f, nomFacture: donnees.piece.client_nom, tiers: donnees.piece.client_id };
}));
ok(!!vente.refus && /paie en entier/.test(vente.refus), `payée en partie : refusée (« ${vente.refus?.slice(0, 70)}… »)`);
ok(vente.apresRefus === vente.avant, "et le stock n'a pas bougé");
ok(/^(.+-)?PAS-\d{6}$/.test(vente.v.client_code ?? ""), `payée en entier : sa fiche de passage ${vente.v.client_code}`);
ok(vente.v.client_id !== vente.comptant.id && vente.tiers === vente.v.client_id, "la facture est à la fiche, pas au « Comptant »");
ok(vente.nomFacture === "Client de passage", `la facture dit « ${vente.nomFacture} »`);
ok(vente.apres === vente.avant - 2, "le stock sort de 2");

// ---- Clients : hors de la liste, retrouve par le ticket.
await page.getByRole("navigation").getByText("Clients", { exact: true }).first().click();
await page.waitForTimeout(1500);
const liste = await page.locator("main").innerText();
ok(!liste.includes(vente.v.client_code), "la fiche de passage n'est pas dans la liste des clients");
await page.getByRole("button", { name: "Passages", exact: true }).click();
await page.waitForTimeout(1200);
await page.getByPlaceholder("Nom ou téléphone...").fill(vente.f.numero);
await page.waitForTimeout(1500);
const lignes = page.getByTestId("passage");
ok(await lignes.count() === 1 && (await lignes.first().innerText()).includes(vente.v.client_code),
  `« Passages » la retrouve par le numéro du ticket (${vente.f.numero})`);
await capture(page, "j1-01-passages");

// ---- Passer en client ordinaire.
const nom = `Moussa Passant ${s}`;
await lignes.first().getByRole("button", { name: /Passer en client ordinaire/ }).click();
const boite = page.getByRole("dialog");
await boite.getByPlaceholder("Nom du client").fill(nom);
await boite.getByPlaceholder("76 00 00 00").fill("76 12 34 56");
await boite.getByRole("button", { name: "Enregistrer" }).click();
await page.waitForTimeout(1500);
ok((await page.getByRole("status").innerText().catch(() => "")).includes("client ordinaire"), "« … est maintenant un client ordinaire »");
ok(await page.getByTestId("passage").count() === 0, "il quitte les passages");
await page.getByRole("button", { name: "Passages", exact: true }).click();
await page.getByPlaceholder("Nom ou téléphone...").fill(nom);
await page.waitForTimeout(1500);
ok((await page.locator("main").innerText()).includes(nom), "il est dans la liste des clients, sous son nom");
await capture(page, "j1-02-client");

const apres = await page.evaluate(async (pieceId) => {
  const { appeler } = await import("/src/lib/pont.ts");
  return (await appeler("lire_donnees_piece", { pieceId })).piece.client_nom;
}, vente.f.piece_id);
ok(apres === "Client de passage", `sa facture émise garde « ${apres} »`);

const utiles = erreursUtiles(erreurs);
ok(utiles.length === 0, "aucune erreur dans la console" + (utiles.length ? " : " + utiles.join(" | ") : ""));
console.log(`${ok.total()} vérifications`);
await b.close();
