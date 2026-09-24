// C-2 : les droits valent par dossier. Le patron regle, dans la fenetre
// Dossiers de Parametres -> Utilisateurs, le frere (patron de sa quincaillerie, rien chez toi) et
// la comptable (comptable chez toi, caissiere chez lui). Chacun se
// connecte : le frere tombe chez lui sans choisir et ne gere pas les
// comptes ; la comptable choisit, et le menu suit le role du dossier.
import { navigateur, connecter, capture, parametres, verifieur, erreursUtiles } from "./pw.mjs";
const { b, page, erreurs } = await navigateur();
const ok = verifieur();
await connecter(page);
const s = Date.now() % 100000;
const quinc = `Quincaillerie C2${s}`;
await page.evaluate(async ({ s, quinc }) => {
  const { appeler } = await import("/src/lib/pont.ts");
  await appeler("creer_dossier", { code: `C2${s}`, societe: quinc });
  await appeler("creer_utilisateur", { nom: `Frère ${s}`, pseudo: `frere${s}`, email: null, motDePasse: "banc-secret", roleNom: "patron" });
  await appeler("creer_utilisateur", { nom: `Awa ${s}`, pseudo: `awa${s}`, email: null, motDePasse: "banc-secret", roleNom: "comptable" });
}, { s, quinc });

async function ouvrirDossiers(nom) {
  await parametres(page, "Utilisateurs");
  await page.getByRole("button", { name: `Dossiers de ${nom}` }).click();
  await page.waitForTimeout(1500);
  return page.getByTestId("dossiers-utilisateur");
}

// ---- Le frere : patron chez lui, rien chez toi.
let bloc = await ouvrirDossiers(`Frère ${s}`);
ok(await bloc.count() === 1, "un patron aussi a son bouton Dossiers");
ok((await bloc.innerText()).includes("Comme son rôle « patron » : tous les dossiers"), "par défaut : comme son rôle, tous les dossiers");
await bloc.getByLabel("Par dossier").check();
await bloc.getByLabel("Rôle dans Ma boutique").selectOption("");
await bloc.getByLabel(`Rôle dans ${quinc}`).selectOption("patron");
await bloc.getByRole("button", { name: "Enregistrer les dossiers" }).click();
await page.waitForTimeout(1200);
ok((await bloc.getByRole("status").innerText()).includes(`Dossiers de Frère ${s} enregistrés`), "le frère : enregistré");
await capture(page, "c2-01-frere");
await page.keyboard.press("Escape");
await page.waitForTimeout(500);

// ---- La comptable : comptable chez toi, caissiere chez lui.
bloc = await ouvrirDossiers(`Awa ${s}`);
await bloc.getByLabel("Par dossier").check();
await bloc.getByLabel("Rôle dans Ma boutique").selectOption("comptable");
await bloc.getByLabel(`Rôle dans ${quinc}`).selectOption("caissier");
await bloc.getByRole("button", { name: "Enregistrer les dossiers" }).click();
await page.waitForTimeout(1200);
ok((await bloc.getByRole("status").innerText()).includes("enregistrés"), "la comptable : enregistrée");
await page.keyboard.press("Escape");

// ---- Le frere se connecte : chez lui d'office.
const nouvel = async () => {
  const ctx = await b.newContext({ viewport: { width: 1440, height: 900 }, locale: "fr-FR" });
  const p = await ctx.newPage();
  p.on("console", m => { if (m.type() === "error") erreurs.push(m.text()); });
  return p;
};
const pf = await nouvel();
await connecter(pf, `frere${s}`, "banc-secret", "banc-secret", quinc);
ok(await pf.getByText("Quel dossier ouvrir ?").count() === 0, "le frère ne choisit pas : un seul dossier lui est ouvert");
ok(await pf.getByText(quinc).count() > 0, "la barre dit sa quincaillerie");
await pf.getByText("Paramètres", { exact: true }).first().click();
await pf.waitForTimeout(1200);
const onglets = await pf.getByRole("main").innerText();
ok(!onglets.includes("Utilisateurs") && !onglets.includes("Sauvegarde"), "ni comptes ni sauvegarde : communs à tous les dossiers");
ok(onglets.includes("Dossiers"), "ses dossiers et ses exercices, oui");
const clients = await pf.evaluate(async () => {
  const { appeler } = await import("/src/lib/pont.ts");
  return (await appeler("lire_clients", {})).map(c => c.nom);
});
ok(clients.length === 1 && clients[0] === "Comptant", `ses clients seulement (${clients.join(", ")})`);
await capture(pf, "c2-02-frere-chez-lui");

// ---- La comptable choisit ; chez lui, elle est caissiere.
const pa = await nouvel();
await pa.goto(page.url());
await pa.waitForTimeout(1500);
await pa.locator("input").nth(0).fill(`awa${s}`);
await pa.locator('input[type="password"]').first().fill("banc-secret");
await pa.keyboard.press("Enter");
await pa.waitForTimeout(2500);
ok(await pa.getByText("Quel dossier ouvrir ?").count() === 1, "la comptable choisit");
const choix = await pa.getByRole("button").filter({ hasText: /Ma boutique|Quincaillerie/ }).allInnerTexts();
ok(choix.length === 2, `deux dossiers proposés, pas plus (${choix.length})`);
await pa.getByLabel(/directement la prochaine fois/).uncheck();
await pa.getByRole("button", { name: new RegExp(quinc) }).click();
await pa.waitForTimeout(2500);
const menu = await pa.locator("nav").first().innerText().catch(() => "");
ok(menu.includes("Ventes") && !menu.includes("Historique") && !menu.includes("Rapports"), "caissière ici : Ventes, mais ni Historique ni Rapports");
await capture(pa, "c2-03-comptable-caissiere");

const utiles = erreursUtiles(erreurs);
ok(utiles.length === 0, "aucune erreur dans la console" + (utiles.length ? " : " + utiles.join(" | ") : ""));
console.log(`${ok.total()} vérifications`);
await b.close();
