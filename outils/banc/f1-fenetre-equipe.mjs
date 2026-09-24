// F-1 : Gescom Equipe, la seconde fenetre (PLAN-EQUIPE, D28). Meme
// serveur, memes comptes, meme choix de dossier ; le menu suit les
// droits : le patron voit tout, la caissiere le suivi client, le
// magasinier rien — et on le lui dit.
import { navigateur, connecter, capture, verifieur, erreursUtiles, URL_EQUIPE } from "./pw.mjs";
const { b, page, erreurs } = await navigateur();
const ok = verifieur();
const s = Date.now() % 100000;

await connecter(page, "admin", "admin123", "Admin-2026!", "Ma boutique", URL_EQUIPE);
ok(await page.title() === "Gescom Équipe", "la fenêtre s'appelle Gescom Équipe");
const menu = page.getByRole("navigation", { name: "Menu Équipe" });
const entrees = await menu.getByRole("button").allInnerTexts();
ok(["Accueil", "Personnel", "Paie", "Suivi clients"].every(e => entrees.some(t => t.startsWith(e))), `le patron voit tout (${entrees.length})`);
ok(await page.getByRole("heading", { name: /Bonjour/ }).count() === 1, "l'accueil salue");
ok(await page.getByTestId("module-personnel").count() === 1 && await page.getByTestId("module-paie").count() === 1, "les modules sur l'accueil");
ok((await page.getByTestId("dossier-ouvert").innerText()).length > 0, "le dossier ouvert est dit");
await capture(page, "f1-01-patron");

await menu.getByRole("button", { name: /Personnel/ }).click();
ok((await page.getByTestId("bientot-personnel").innerText()).includes("F-2"), "Personnel : annoncé pour F-2, pas un écran vide");

// Des comptes, créés par le serveur comme dans Gescom.
await page.evaluate(async s => {
  const { appeler } = await import("/src/lib/pont.ts");
  await appeler("creer_utilisateur", { nom: `Fanta ${s}`, pseudo: `fanta${s}`, email: null, motDePasse: "banc-secret", roleNom: "caissier" });
  await appeler("creer_utilisateur", { nom: `Issa ${s}`, pseudo: `issa${s}`, email: null, motDePasse: "banc-secret", roleNom: "magasinier" });
}, s);

const nouvelle = async () => {
  const ctx = await b.newContext({ viewport: { width: 1280, height: 800 }, locale: "fr-FR" });
  const p = await ctx.newPage();
  p.on("console", m => { if (m.type() === "error") erreurs.push(m.text()); });
  return p;
};

// La caissière : le mot de passe donné par le patron se change d'abord.
const pc = await nouvelle();
await connecter(pc, `fanta${s}`, "banc-secret", "banc-secret2", "Ma boutique", URL_EQUIPE);
const menuC = await pc.getByRole("navigation", { name: "Menu Équipe" }).getByRole("button").allInnerTexts();
ok(menuC.some(t => t.startsWith("Suivi clients")) && !menuC.some(t => t.startsWith("Paie")) && !menuC.some(t => t.startsWith("Personnel")),
  `la caissière : suivi client, ni paie ni personnel (${menuC.join(", ").replace(/\n/g, " ")})`);
await capture(pc, "f1-02-caissiere");

// Le magasinier : rien ici, et on le lui dit.
const pm = await nouvelle();
await connecter(pm, `issa${s}`, "banc-secret", "banc-secret2", "Ma boutique", URL_EQUIPE);
ok(await pm.getByTestId("rien-a-faire").count() === 1, "le magasinier : « rien pour l'instant, demander au patron »");

// Se déconnecter ramène à la connexion d'Équipe.
await page.getByRole("button", { name: "Se déconnecter" }).click();
await page.waitForTimeout(1500);
ok(await page.getByText("Personnel, paie, suivi des clients").count() === 1, "déconnecté : la connexion d'Équipe");

const utiles = erreursUtiles(erreurs);
ok(utiles.length === 0, "aucune erreur dans la console" + (utiles.length ? " : " + utiles.join(" | ") : ""));
console.log(`${ok.total()} vérifications`);
await b.close();
