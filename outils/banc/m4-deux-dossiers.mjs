// Multiposte (v3, serveur + PostgreSQL) : deux dossiers ouverts EN MEME
// TEMPS sur deux postes. A travaille dans « Ma boutique », B dans la
// boutique du frere, creee pour l'occasion. Chacun a sa caisse ouverte,
// ses clients, son stock (les articles sont communs, pas les quantites),
// ses numeros de piece ; rien ne passe de l'un a l'autre.
import { navigateur, connecter, verifieur, erreursUtiles, capture } from "./pw.mjs";
const { b, page: a, erreurs } = await navigateur();
const ok = verifieur();
const s = Date.now() % 100000;
const societe = `Boutique du frère M4${s}`;

await connecter(a);
const attendu = async (liste, f) => { const n = liste.length; const r = await f(); liste.splice(n); return r; };
const cree = await a.evaluate(async ({ s, societe }) => {
  const { appeler } = await import("/src/lib/pont.ts");
  await appeler("creer_dossier", { code: `M4${s}`, societe });
  return (await appeler("lire_dossiers", {})).map(d => d.societe ?? d.nom);
}, { s, societe });
ok(cree.includes(societe), `A crée le second dossier (${cree.length} dossiers)`);

// B : le meme patron, un autre poste, qui ouvre l'autre dossier.
const ctxB = await b.newContext({ viewport: { width: 1280, height: 800 }, locale: "fr-FR" });
await ctxB.addInitScript(() => {
  if (!localStorage.getItem("gescom_reseau")) {
    localStorage.setItem("gescom_reseau", JSON.stringify({ posteNom: "Poste du frère", posteEmpreinte: "banc-m4-poste-b" }));
  }
});
const pB = await ctxB.newPage();
const erreursB = [];
pB.on("pageerror", e => erreursB.push("pageerror: " + e.message));
pB.on("console", m => { if (m.type() === "error") erreursB.push("console: " + m.text()); });
await connecter(pB, "admin", "admin123", "Admin-2026!", societe);
const [da, db] = await Promise.all([a, pB].map(p => p.evaluate(async () => (await import("/src/lib/pont.ts")).etatReseau().dossierId)));
ok(!!da && !!db && da !== db, "A et B travaillent dans deux dossiers différents, en même temps");
ok(await pB.getByText(societe).count() > 0, "la barre de B nomme la boutique du frère");

// ---- Les deux caisses ouvertes en meme temps, une par dossier.
const caisses = await attendu(erreurs, () => attendu(erreursB, async () => Promise.all([a, pB].map(p => p.evaluate(async () => {
  const { appeler } = await import("/src/lib/pont.ts");
  try { await appeler("ouvrir_session_caisse", { fondOuverture: 0 }); } catch { /* deja ouverte */ }
  return (await appeler("lire_sessions_caisse", {})).filter(x => x.statut === "ouverte").map(x => x.id);
})))));
ok(caisses[0].length === 1 && caisses[1].length === 1, "une caisse ouverte de chaque côté");
ok(caisses[0][0] !== caisses[1][0], "et ce ne sont pas la même");

// ---- B : un client, un devis, dans son dossier ; A : un devis aussi.
const nomClient = `Client M4${s}`;
const surB = await pB.evaluate(async (nomClient) => {
  const { appeler } = await import("/src/lib/pont.ts");
  const c = await appeler("creer_client_rapide", { nom: nomClient });
  const arts = await appeler("lire_articles_avec_unites", {});
  const art = arts.find(x => x.unites?.length);
  const devis = await appeler("creer_piece", { clientId: c.id ?? c, typePiece: "devis",
    lignes: [{ article_id: art.id, unite_vente_id: art.unites[0].id, quantite: 1, prix_unitaire: art.unites[0].prix_reference, remise_pct: 0, taux_tva: 0 }] });
  return { clients: (await appeler("lire_clients", {})).map(x => x.nom), art: { id: art.id, nom: art.nom, stock: art.stock }, nb: arts.length, numero: devis.numero };
}, nomClient);
const surA = await a.evaluate(async (artId) => {
  const { appeler } = await import("/src/lib/pont.ts");
  const arts = await appeler("lire_articles_avec_unites", {});
  const art = arts.find(x => x.id === artId);
  const client = (await appeler("lire_client_generique", {})).id;
  const devis = await appeler("creer_piece", { clientId: client, typePiece: "devis",
    lignes: [{ article_id: art.id, unite_vente_id: art.unites[0].id, quantite: 1, prix_unitaire: art.unites[0].prix_reference, remise_pct: 0, taux_tva: 0 }] });
  return { clients: (await appeler("lire_clients", {})).map(x => x.nom), stock: art.stock, nb: arts.length, numero: devis.numero };
}, surB.art.id);
ok(surB.clients.includes(nomClient) && !surA.clients.includes(nomClient), `le client de B (${nomClient}) n'existe pas pour A`);
ok(!surB.clients.includes("Amadou Diarra") && surA.clients.includes("Amadou Diarra"), "les clients de la démo restent dans Ma boutique");
ok(surA.nb === surB.nb, `les articles sont communs (${surA.nb} de chaque côté)`);
ok(surB.art.stock === 0 && surA.stock > 0, `le stock ne l'est pas : ${surB.art.nom} à ${surB.art.stock} chez B, ${surA.stock} chez A`);
ok(!!surA.numero && !!surB.numero && surA.numero !== surB.numero, `deux numérotations : ${surA.numero} chez A, ${surB.numero} chez B`);

// L'ecran de B : sa liste de clients.
await pB.getByText("Clients", { exact: true }).first().click();
await pB.waitForTimeout(1500);
ok(await pB.getByText(nomClient).count() > 0 && await pB.getByText("Amadou Diarra").count() === 0,
  "l'écran Clients de B montre son client, pas ceux de A");
await capture(pB, "m4-b-clients");

const utiles = erreursUtiles([...erreurs, ...erreursB]);
ok(utiles.length === 0, "aucune erreur dans les consoles" + (utiles.length ? " : " + utiles.join(" | ") : ""));
console.log(`${ok.total()} vérifications`);
await b.close();
