// Multiposte (v3, serveur + PostgreSQL) : ce qu'un poste fait, l'autre
// le voit, parce qu'il n'y a qu'une base. Une vente a credit sur A ; B
// lit la creance et en encaisse une part ; A relit la creance diminuee
// et l'argent entre dans la caisse commune. Puis A ferme la caisse : B
// est refuse (CAISSE_FERMEE) et la creance n'a pas bouge.
import { navigateur, connecter, verifieur, erreursUtiles, capture } from "./pw.mjs";
const { b, page: a, erreurs } = await navigateur();
const ok = verifieur();

const ctxB = await b.newContext({ viewport: { width: 1280, height: 800 }, locale: "fr-FR" });
await ctxB.addInitScript(() => {
  if (!localStorage.getItem("gescom_reseau")) {
    localStorage.setItem("gescom_reseau", JSON.stringify({ posteNom: "Caisse M3", posteEmpreinte: "banc-m3-caisse-b" }));
  }
});
const pB = await ctxB.newPage();
const erreursB = [];
pB.on("pageerror", e => erreursB.push("pageerror: " + e.message));
pB.on("console", m => { if (m.type() === "error") erreursB.push("console: " + m.text()); });
// B : la comptable, sur son poste — c'est elle qui lit les soldes et
// encaisse les creances (l'employe n'a ni `tiers:lire_solde` ni
// `creances:gerer`, C-1).
await connecter(a);
const s = Date.now() % 100000;
await a.evaluate(async (s) => {
  const { appeler } = await import("/src/lib/pont.ts");
  await appeler("creer_utilisateur", { nom: `Comptable M3${s}`, pseudo: `comptam3${s}`, email: null, motDePasse: "banc-secret", roleNom: "comptable" });
}, s);
await connecter(pB, `comptam3${s}`, "banc-secret", "banc-secret", "Ma boutique");
// Les refus ATTENDUS (409) sortent de la console : ils sont le sujet.
const attendu = async (liste, f) => { const n = liste.length; const r = await f(); liste.splice(n); return r; };

// ---- A : la caisse ouverte, une vente a credit a un client de la demo.
const v = await attendu(erreurs, () => a.evaluate(async () => {
  const { appeler } = await import("/src/lib/pont.ts");
  try { await appeler("ouvrir_session_caisse", { fondOuverture: 0 }); } catch { /* deja ouverte */ }
  const depot = (await appeler("lire_depot_defaut", {})).id;
  const art = (await appeler("lire_articles_avec_unites", {})).find(x => x.unites?.length && x.stock >= 5);
  const u = art.unites.find(x => x.facteur === 1) ?? art.unites[0];
  const client = (await appeler("lire_clients", {})).find(c => c.code === "CLIENT00002") ?? (await appeler("lire_clients", {}))[1];
  const avant = (await appeler("lire_etat_creances_client", { clientId: client.id })).total_du;
  const r = await appeler("creer_vente", { clientId: client.id, depotId: depot, modeReglement: "credit",
    lignes: [{ article_id: art.id, unite_vente_id: u.id, depot_source_id: depot, source_approvisionnement: "stock",
      quantite: 2, facteur: u.facteur, prix_reference: u.prix_reference, prix_pratique: u.prix_reference }] });
  return { client, art, u, depot, avant, vente: r, apres: (await appeler("lire_etat_creances_client", { clientId: client.id })).total_du };
}));
const prix2 = 2 * v.u.prix_reference;
ok(v.vente.statut !== "paye" && v.apres === v.avant + prix2, `A : vente à crédit à ${v.client.nom}, créance ${v.avant} → ${v.apres} F`);

// ---- B relit la creance faite sur A, en encaisse 300 F. La caisse se
// lit sur A (le patron) : c'est sa session, la caisse commune du dossier.
const avantB = await a.evaluate(async () => {
  const { appeler } = await import("/src/lib/pont.ts");
  const s = (await appeler("lire_sessions_caisse", {})).find(x => x.statut === "ouverte");
  return { entrees: s.entrees_especes, sessionId: s.id };
});
const surB = await pB.evaluate(async ({ clientId, venteId }) => {
  const { appeler } = await import("/src/lib/pont.ts");
  const du = (await appeler("lire_etat_creances_client", { clientId })).total_du;
  await appeler("regler_creance", { venteId, montant: 300, mode: "especes" });
  return { du };
}, { clientId: v.client.id, venteId: v.vente.vente_id });
surB.entrees = avantB.entrees;
surB.sessionId = avantB.sessionId;
ok(surB.du === v.apres, `B (la comptable) lit la créance faite sur A (${surB.du} F) et en encaisse 300`);

// ---- A relit : creance diminuee, argent entre.
const surA = await a.evaluate(async ({ clientId, sessionId }) => {
  const { appeler } = await import("/src/lib/pont.ts");
  const du = (await appeler("lire_etat_creances_client", { clientId })).total_du;
  const s = (await appeler("lire_sessions_caisse", {})).find(x => x.id === sessionId);
  return { du, entrees: s.entrees_especes };
}, { clientId: v.client.id, sessionId: surB.sessionId });
ok(surA.du === v.apres - 300, `A relit la créance diminuée du règlement de B (${surA.du} F)`);
ok(surA.entrees === surB.entrees + 300, `les 300 F de B sont dans la caisse commune (${surB.entrees} → ${surA.entrees})`);

// L'ecran de A : la fiche du client dit son reste.
await a.getByText("Clients", { exact: true }).first().click();
await a.waitForTimeout(1500);
ok(await a.getByText(v.client.nom).count() > 0, "A : l'écran Clients nomme le client");
await capture(a, "m3-a-clients");

// ---- A ferme la caisse ; B veut vendre comptant : refuse, rien ne bouge.
const fermee = await a.evaluate(async (sessionId) => {
  const { appeler } = await import("/src/lib/pont.ts");
  const s = (await appeler("lire_sessions_caisse", {})).find(x => x.id === sessionId);
  await appeler("fermer_session_caisse", { sessionId, especesComptees: s.fond_ouverture + s.entrees_especes - s.sorties_especes });
  return (await appeler("lire_sessions_caisse", {})).find(x => x.id === sessionId).statut;
}, surB.sessionId);
ok(fermee !== "ouverte", `A ferme la caisse (statut ${fermee})`);
// B veut encaisser 100 F de plus sur la meme vente : l'argent entrerait
// dans un tiroir ferme.
const refus = await attendu(erreursB, () => pB.evaluate(async ({ clientId, venteId }) => {
  const { appeler } = await import("/src/lib/pont.ts");
  const du = async () => (await appeler("lire_etat_creances_client", { clientId })).total_du;
  const avant = await du();
  let erreur = null;
  try { await appeler("regler_creance", { venteId, montant: 100, mode: "especes" }); } catch (e) { erreur = String(e); }
  return { erreur, avant, apres: await du() };
}, { clientId: v.client.id, venteId: v.vente.vente_id }));
ok(!!refus.erreur && /caisse/i.test(refus.erreur), `B refusé, caisse fermée (« ${refus.erreur} »)`);
ok(refus.apres === refus.avant, `la créance n'a pas bougé (${refus.avant} F)`);
const ouvertes = await a.evaluate(async () => {
  const { appeler } = await import("/src/lib/pont.ts");
  return (await appeler("lire_sessions_caisse", {})).filter(s => s.statut === "ouverte").length;
});
ok(ouvertes === 0, "aucune caisse ne s'est rouverte en douce");

// On rend la caisse ouverte aux parcours suivants.
await attendu(erreurs, () => a.evaluate(async () => {
  const { appeler } = await import("/src/lib/pont.ts");
  try { await appeler("ouvrir_session_caisse", { fondOuverture: 0 }); } catch { /* */ }
}));

const utiles = erreursUtiles([...erreurs, ...erreursB]);
ok(utiles.length === 0, "aucune erreur dans les consoles" + (utiles.length ? " : " + utiles.join(" | ") : ""));
console.log(`${ok.total()} vérifications`);
await b.close();
