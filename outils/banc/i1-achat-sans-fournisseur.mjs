// Correctif du 25/09 : une entree de stock SANS fournisseur ne disait la
// somme nulle part (ni piece, ni paiement ; a credit, rien du tout).
// Maintenant : « A credit » est ferme sans fournisseur, l'achat se paie
// comptant et se note chez « Fournisseur divers » — une facture payee,
// sur la fiche de ce fournisseur, la somme dite.
import { navigateur, connecter, capture, verifieur, erreursUtiles } from "./pw.mjs";
const { b, page, erreurs } = await navigateur();
const ok = verifieur();

await connecter(page, "admin", "admin123", "Admin-2026!", "Ma boutique");
await page.evaluate(async () => {
  const { appeler } = await import("/src/lib/pont.ts");
  try { await appeler("ouvrir_session_caisse", { fondOuverture: 0 }); } catch { /* déjà ouverte */ }
});
const avant = await page.evaluate(async () => {
  const { appeler } = await import("/src/lib/pont.ts");
  return (await appeler("lire_toutes_pieces_fournisseur", { typeFiltre: "facture_fournisseur" })).length;
});
erreurs.splice(0); // la caisse déjà ouverte répond 409 : c'est la préparation

await page.getByRole("link", { name: "Achats" }).or(page.getByRole("button", { name: "Achats" })).first().click();
await page.waitForTimeout(1500);

ok(await page.getByRole("button", { name: "À crédit" }).isDisabled(), "sans fournisseur, « À crédit » est fermé");
ok(await page.getByText("l'achat se paie comptant et se note chez").count() === 1, "l'écran dit pourquoi, et où l'achat se notera");

await page.getByPlaceholder("Rechercher un article...").fill("Sucre");
await page.waitForTimeout(600);
await page.getByRole("button", { name: /^Sucre/ }).first().click();
await page.waitForTimeout(500);
await page.getByPlaceholder("Prix unitaire").fill("400");
await page.getByRole("spinbutton").first().fill("2");
await page.getByRole("button", { name: "Ajouter →" }).click();
await page.waitForTimeout(500);
await page.getByRole("button", { name: /Enregistrer la réception/ }).click();
await page.waitForTimeout(800);
const modale = page.getByRole("dialog");
ok((await modale.innerText()).includes("Fournisseur divers") && (await modale.innerText()).includes("payé comptant"),
  "la confirmation dit « Fournisseur divers », payé comptant");
await capture(page, "i1-01-confirmation");
await modale.getByRole("button", { name: "Confirmer" }).click();
await page.waitForTimeout(2000);

const apres = await page.evaluate(async () => {
  const { appeler } = await import("/src/lib/pont.ts");
  const fournisseurs = await appeler("lire_fournisseurs");
  const divers = fournisseurs.find(f => f.est_generique);
  const pieces = await appeler("lire_toutes_pieces_fournisseur", { typeFiltre: "facture_fournisseur" });
  return { divers, pieces, sienne: pieces.filter(p => divers && p.tiers_id === divers.id) };
});
ok(!!apres.divers && apres.divers.nom === "Fournisseur divers", "le « Fournisseur divers » existe, marqué comme tel");
ok(apres.pieces.length === avant + 1, "une facture fournisseur de plus");
const faf = apres.sienne.at(-1) ?? apres.pieces.find(p => p.tiers_nom === "Fournisseur divers");
ok(!!faf && String(faf.numero).startsWith("FAF-"), `numérotée (${faf?.numero})`);
ok(!!faf && faf.statut === "paye", "payée : on ne doit rien à personne");
ok(!!faf && JSON.stringify(faf).includes("800"), "la somme est dite : 800 F");

// Choisir « Fournisseur divers » dans la liste ne rouvre pas le crédit.
await page.getByPlaceholder("Rechercher un fournisseur...").fill("divers");
await page.waitForTimeout(500);
await page.getByRole("button", { name: /Fournisseur divers/ }).first().click();
await page.waitForTimeout(300);
ok(await page.getByRole("button", { name: "À crédit" }).isDisabled(), "« Fournisseur divers » choisi : toujours comptant");

const utiles = erreursUtiles(erreurs);
ok(utiles.length === 0, "aucune erreur dans la console" + (utiles.length ? " : " + utiles.join(" | ") : ""));
console.log(`${ok.total()} vérifications`);
await b.close();
