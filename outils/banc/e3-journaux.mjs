// E-3 : Rapports -> Journaux comptables. Une vente comptant et une
// depense du jour ; les quatre journaux s'affichent equilibres, les
// ventes du jour font le chiffre du cahier, le filtre garde un journal,
// et l'export CSV se telecharge, lisible par Excel.
import fs from "node:fs";
import { navigateur, connecter, capture, verifieur, erreursUtiles } from "./pw.mjs";
const { b, page, erreurs } = await navigateur();
const ok = verifieur();
await connecter(page);
const jour = new Date().toISOString().slice(0, 10);

// Une vente comptant et une dépense de loyer, aujourd'hui. La caisse
// est peut-être déjà ouverte : le refus attendu (409) sort de la console.
const n0 = erreurs.length;
const prep = await page.evaluate(async () => {
  const { appeler } = await import("/src/lib/pont.ts");
  try { await appeler("ouvrir_session_caisse", { fondOuverture: 0 }); } catch { /* déjà ouverte */ }
  const arts = await appeler("lire_articles_avec_unites", {});
  const a = arts.find(x => x.unites?.length);
  const u = a.unites[0];
  const depot = (await appeler("lire_depot_defaut", {})).id;
  const client = (await appeler("lire_client_generique", {})).id;
  await appeler("creer_vente", {
    clientId: client, depotId: depot, modeReglement: "comptant", montantPaye: u.prix_reference, modePaiement: "especes",
    lignes: [{ article_id: a.id, unite_vente_id: u.id, depot_source_id: depot, source_approvisionnement: "stock",
               quantite: 1, facteur: u.facteur, prix_reference: u.prix_reference, prix_pratique: u.prix_reference }],
  });
  await appeler("enregistrer_depense", { montant: 1500, libelle: "Loyer banc", categorie: "loyer" });
  return "ok";
});
erreurs.splice(n0);
ok(prep === "ok", "une vente et une dépense du jour");

await page.getByText("Rapports", { exact: true }).first().click();
await page.waitForTimeout(1200);
await page.getByRole("button", { name: /Journaux comptables/ }).click();
await page.waitForTimeout(2000);
for (const code of ["VT", "AC", "RG", "CA"]) {
  const t = await page.getByTestId(`journal-${code}`).innerText();
  ok(t.includes("équilibré") && !t.includes("DÉSÉQUILIBRÉ"), `${code} affiché, équilibré`);
}
const ca = await page.getByTestId("journal-CA").innerText();
ok(ca.includes("622") && ca.includes("Loyer banc"), "le loyer va sur 622");
await capture(page, "e3-01-journaux");

// Les ventes du journal = le chiffre du cahier du jour.
const cmp = await page.evaluate(async jour => {
  const { appeler } = await import("/src/lib/pont.ts");
  const c = await appeler("lire_journal_du_jour", { date: jour });
  const cahier = c.ventes.reduce((s, v) => s + v.montant_ttc, 0);
  const j = await appeler("lire_journaux_comptables", { du: jour, au: jour, journal: "VT" });
  const ventes = j.journaux[0].ecritures.filter(e => e.libelle.startsWith("Vente"));
  const credits = ventes.flatMap(e => e.lignes).filter(l => l.compte !== "411").reduce((s, l) => s + l.credit, 0);
  return { cahier, credits };
}, jour);
ok(cmp.cahier > 0 && cmp.cahier === cmp.credits, `ventes du journal = CA du cahier (${cmp.credits} / ${cmp.cahier})`);

// Un seul journal.
await page.getByLabel("Journal").selectOption("RG");
await page.waitForTimeout(1500);
ok(await page.getByTestId("journal-RG").count() === 1 && await page.getByTestId("journal-VT").count() === 0, "le filtre garde les règlements");

// L'export.
const [dl] = await Promise.all([
  page.waitForEvent("download"),
  page.getByRole("button", { name: /Exporter CSV/ }).click(),
]);
ok(dl.suggestedFilename() === `journaux_RG_${jour.slice(0, 8)}01_${jour}.csv`, `fichier nommé ${dl.suggestedFilename()}`);
const chemin = await dl.path();
const texte = fs.readFileSync(chemin, "utf8");
ok(texte.charCodeAt(0) === 0xFEFF, "UTF-8 avec BOM : Excel lit les accents");
const lignes = texte.slice(1).trim().split("\r\n");
ok(lignes[0] === "Date;Journal;Compte;Libellé du compte;Libellé;Débit;Crédit;Pièce", "l'en-tête attendu");
ok(lignes.length > 1 && lignes.slice(1).every(l => l.split(";")[1] === "RG"), `${lignes.length - 1} lignes, toutes RG`);
const tot = lignes.slice(1).reduce((t, l) => { const c = l.split(";"); return [t[0] + (+c[5] || 0), t[1] + (+c[6] || 0)]; }, [0, 0]);
ok(tot[0] === tot[1], `le fichier s'équilibre (${tot[0]})`);

const utiles = erreursUtiles(erreurs);
ok(utiles.length === 0, "aucune erreur dans la console" + (utiles.length ? " : " + utiles.join(" | ") : ""));
console.log(`${ok.total()} vérifications`);
await b.close();
