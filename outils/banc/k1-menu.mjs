// D37 (26/10/2026) : le menu se regle par dossier. Parametres → Menu :
// « Boutique simple » cache Chèques, Relances, Historique… ; le menu et
// la palette (Ctrl K) les perdent, le reglage tient au rechargement ; le
// serveur refuse de cacher la Caisse ; le geste est dans l'Historique ;
// « Complet » remet tout.
//
//   serveur (sqlite ou multiposte) : node k1-menu.mjs
import { navigateur, connecter, parametres, capture, verifieur, erreursUtiles } from "./pw.mjs";
const { b, page, erreurs } = await navigateur();
const ok = verifieur();
await connecter(page);
const menu = page.getByRole("navigation");
const dansLeMenu = async (nom) => (await menu.getByText(nom, { exact: true }).count()) > 0;

await parametres(page, "Menu");
ok(await page.getByRole("checkbox", { name: "Caisse" }).isDisabled(), "Caisse : case grisée, toujours dans le menu");
await page.getByRole("button", { name: "Boutique simple" }).click();
ok(!(await page.getByRole("checkbox", { name: "Chèques" }).isChecked()), "« Boutique simple » décoche Chèques");
await page.getByRole("button", { name: "Enregistrer le menu" }).click();
await page.getByRole("status").waitFor({ timeout: 10000 }).catch(() => {});
ok((await page.getByRole("status").innerText().catch(() => "")).includes("Menu enregistré"), "« Menu enregistré »");
await page.waitForTimeout(800);
for (const cache of ["Chèques", "Relances", "Historique", "Transferts", "Journal"]) {
  ok(!(await dansLeMenu(cache)), `${cache} quitte le menu`);
}
for (const reste of ["Tableau de bord", "Ventes", "Achats", "Stock", "Clients", "Caisse", "Rapports", "Paramètres"]) {
  ok(await dansLeMenu(reste), `${reste} reste`);
}
await capture(page, "k1-01-boutique-simple");

// La palette suit le meme reglage.
await page.keyboard.press("Control+K");
const recherche = page.getByPlaceholder("Aller à… faire… ou le nom d'un client, d'un fournisseur");
await recherche.waitFor({ timeout: 5000 }).catch(() => {});
await recherche.fill("Relances");
await page.waitForTimeout(600);
const propose = await page.getByRole("button", { name: /^Relances/ }).count();
ok(propose === 0, "Ctrl K ne propose plus « Relances »");
await page.keyboard.press("Escape");

// Le reglage tient au rechargement (il vit cote serveur, par dossier).
await page.reload();
await page.waitForTimeout(2500);
ok(!(await dansLeMenu("Chèques")) && await dansLeMenu("Stock"), "rechargé : le menu reste réglé");

// Le serveur juge, quel que soit l'ecran ; le geste est trace.
const r = await page.evaluate(async () => {
  const { appeler } = await import("/src/lib/pont.ts");
  let refus = null;
  try { await appeler("definir_menu", { caches: ["caisse"] }); } catch (e) { refus = String(e); }
  const h = await appeler("lire_historique", { filtre: {
    du: null, au: null, auteur_id: null, type_evenement: "menu_modifie",
    tiers_id: null, piece_id: null, article_id: null, recherche: null, page: 0, par_page: 50 } });
  return { refus, lignes: h.lignes.length, libelle: h.lignes[0]?.libelle_type };
});
erreurs.splice(0); // le refus (409) est le sujet
ok(!!r.refus && /toujours/.test(r.refus), `cacher la Caisse : refusé (« ${r.refus?.slice(0, 60)}… »)`);
ok(r.lignes >= 1 && r.libelle === "Menu réglé", `l'Historique dit « ${r.libelle} »`);

// « Complet » remet tout.
await parametres(page, "Menu");
await page.getByRole("button", { name: "Complet" }).click();
await page.getByRole("button", { name: "Enregistrer le menu" }).click();
await page.waitForTimeout(1500);
ok(await dansLeMenu("Chèques") && await dansLeMenu("Historique") && await dansLeMenu("Relances"), "« Complet » : tout revient");

const utiles = erreursUtiles(erreurs);
ok(utiles.length === 0, "aucune erreur dans la console" + (utiles.length ? " : " + utiles.join(" | ") : ""));
console.log(`${ok.total()} vérifications`);
await b.close();
