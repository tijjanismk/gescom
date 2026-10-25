// Multiposte (v3, serveur + PostgreSQL) : deux caisses, deux navigateurs,
// deux comptes, qui ecrivent EN MEME TEMPS. Ce que la boutique a deux
// caisses vit le samedi : douze devis tires ensemble n'ont pas deux fois
// le meme numero ; dix ventes du meme article, cinq par caisse, lancees
// ensemble, font baisser le stock de dix et entrer dix fois le prix dans
// la caisse commune — ni plus, ni moins, ni une vente perdue.
//
//   .\lancer.ps1 multiposte -Neuf ; node m2-deux-caisses.mjs
import { navigateur, connecter, verifieur, erreursUtiles, capture } from "./pw.mjs";
const { b, page: a, erreurs } = await navigateur();
const ok = verifieur();

// La caisse B : un second contexte (son propre localStorage), un autre
// poste pour le serveur, un autre compte.
const ctxB = await b.newContext({ viewport: { width: 1280, height: 800 }, locale: "fr-FR" });
await ctxB.addInitScript(() => {
  if (!localStorage.getItem("gescom_reseau")) {
    localStorage.setItem("gescom_reseau", JSON.stringify({ posteNom: "Caisse M2", posteEmpreinte: "banc-m2-caisse-b" }));
  }
});
const pB = await ctxB.newPage();
const erreursB = [];
pB.on("pageerror", e => erreursB.push("pageerror: " + e.message));
pB.on("console", m => { if (m.type() === "error") erreursB.push("console: " + m.text()); });

await connecter(a);
await connecter(pB, "employe", "employe123", "Employe-2026!");
const id = async (p) => p.evaluate(async () => (await import("/src/lib/pont.ts")).etatReseau());
const [ea, eb] = [await id(a), await id(pB)];
ok(!!ea.jeton && !!eb.jeton && ea.posteId !== eb.posteId, "deux sessions, deux postes distincts pour le serveur");
ok(ea.utilisateurId !== eb.utilisateurId, "deux comptes : le patron et l'employé");

// ---- La preparation, sur A : la caisse commune ouverte, un article en stock.
const n0 = erreurs.length;
const prep = await a.evaluate(async () => {
  const { appeler } = await import("/src/lib/pont.ts");
  try { await appeler("ouvrir_session_caisse", { fondOuverture: 0 }); } catch { /* deja ouverte */ }
  const depot = (await appeler("lire_depot_defaut", {})).id;
  const arts = await appeler("lire_articles_avec_unites", {});
  const art = arts.filter(x => x.unites?.length && x.stock >= 20).sort((x, y) => y.stock - x.stock)[0];
  const session = (await appeler("lire_sessions_caisse", {})).find(s => s.statut === "ouverte");
  const client = (await appeler("lire_client_generique", {})).id;
  return { depot, art, session, client };
});
erreurs.splice(n0); // la caisse deja ouverte repond 409 : c'est la preparation
ok(!!prep.art, `un article avec au moins 20 en stock (${prep.art?.nom}, ${prep.art?.stock})`);
ok(!!prep.session, "la caisse du dossier est ouverte");
const u = prep.art.unites.find(x => x.facteur === 1) ?? prep.art.unites[0];

// ---- Douze devis, six par poste, lances ensemble.
const devis = (p, n, nom) => p.evaluate(async ({ n, client, art, u, nom }) => {
  const { appeler } = await import("/src/lib/pont.ts");
  const lignes = [{ article_id: art.id, unite_vente_id: u.id, quantite: 1, prix_unitaire: u.prix_reference, remise_pct: 0, taux_tva: 0 }];
  return Promise.all(Array.from({ length: n }, () =>
    appeler("creer_piece", { clientId: client, typePiece: "devis", lignes }).then(r => r.numero, e => "ERREUR " + nom + " : " + e)));
}, { n, client: prep.client, art: prep.art, u, nom });
const [na, nb] = await Promise.all([devis(a, 6, "A"), devis(pB, 6, "B")]);
const numeros = [...na, ...nb];
const ratees = numeros.filter(x => String(x).startsWith("ERREUR"));
ok(ratees.length === 0, "les douze devis sont enregistrés" + (ratees.length ? " : " + ratees.join(" | ") : ""));
ok(new Set(numeros).size === numeros.length, `douze numéros, aucun doublon (${numeros.slice(0, 3).join(", ")}…)`);

// ---- Dix ventes comptant du meme article, cinq par caisse, ensemble.
const vendre = (p, n, nom) => p.evaluate(async ({ n, prep, u, nom }) => {
  const { appeler } = await import("/src/lib/pont.ts");
  const ligne = { article_id: prep.art.id, unite_vente_id: u.id, depot_source_id: prep.depot, source_approvisionnement: "stock",
    quantite: 1, facteur: u.facteur, prix_reference: u.prix_reference, prix_pratique: u.prix_reference };
  return Promise.all(Array.from({ length: n }, () =>
    appeler("creer_vente", { clientId: prep.client, depotId: prep.depot, modeReglement: "comptant",
      montantPaye: u.prix_reference, modePaiement: "especes", lignes: [ligne] })
      .then(r => r.vente_id, e => "ERREUR " + nom + " : " + e)));
}, { n, prep, u, nom });
const [va, vb] = await Promise.all([vendre(a, 5, "A"), vendre(pB, 5, "B")]);
const ventes = [...va, ...vb];
const vRatees = ventes.filter(x => String(x).startsWith("ERREUR"));
ok(vRatees.length === 0, "les dix ventes sont enregistrées" + (vRatees.length ? " : " + vRatees.join(" | ") : ""));
ok(new Set(ventes).size === 10, "dix ventes distinctes");

// ---- Relu depuis CHAQUE poste : le meme stock. La caisse, par le
// patron (l'employe ne lit pas la caisse d'un autre : C-1).
const relire = (p) => p.evaluate(async ({ art, session }) => {
  const { appeler } = await import("/src/lib/pont.ts");
  const arts = await appeler("lire_articles_avec_unites", {});
  const s = (await appeler("lire_sessions_caisse", {})).find(x => x.id === session.id);
  return { stock: arts.find(x => x.id === art.id).stock, entrees: s?.entrees_especes ?? null };
}, { art: prep.art, session: prep.session });
const [ra, rb] = [await relire(a), await relire(pB)];
const attenduStock = prep.art.stock - 10 * u.facteur;
ok(ra.stock === attenduStock, `stock lu sur A : ${prep.art.stock} → ${ra.stock} (attendu ${attenduStock})`);
ok(rb.stock === ra.stock, `B lit le même stock (${rb.stock})`);
const attenduCaisse = prep.session.entrees_especes + 10 * u.prix_reference;
ok(ra.entrees === attenduCaisse, `caisse commune : ${prep.session.entrees_especes} → ${ra.entrees} F (attendu ${attenduCaisse}), les ventes de B comprises`);

// ---- Et l'ecran de B le montre : la page Stock nomme l'article.
await pB.getByText("Stock", { exact: true }).first().click();
await pB.waitForTimeout(1500);
ok(await pB.getByText(prep.art.nom).count() > 0, "l'écran Stock de la caisse B s'ouvre et nomme l'article");
await capture(pB, "m2-caisse-b-stock");

const utiles = erreursUtiles([...erreurs, ...erreursB]);
ok(utiles.length === 0, "aucune erreur dans les consoles" + (utiles.length ? " : " + utiles.join(" | ") : ""));
console.log(`${ok.total()} vérifications`);
await b.close();
