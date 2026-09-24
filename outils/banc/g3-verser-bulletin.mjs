// G-3 : Equipe -> Paie -> Fiches du mois. Une fiche validee se paie en
// deux versements, hors caisse (le reste suit), puis le bulletin
// s'affiche : lignes, net en lettres, versements, deux signatures. Dans
// Gescom, Parametres -> Documents regle le bulletin comme les autres.
import { navigateur, connecter, capture, parametres, verifieur, erreursUtiles, URL, URL_EQUIPE } from "./pw.mjs";
const { b, page, erreurs } = await navigateur();
const ok = verifieur();
const s = Date.now() % 100000;
const awa = `Awa ${s}`;
const d = new Date();
const mois = d.toISOString().slice(0, 7);
const fin = new Date(d.getFullYear(), d.getMonth() + 1, 0).getDate();

await connecter(page, "admin", "admin123", "Admin-2026!", "Ma boutique", URL_EQUIPE);
await page.evaluate(async ({ awa, mois, fin }) => {
  const { appeler } = await import("/src/lib/pont.ts");
  try { await appeler("ouvrir_session_caisse", { fondOuverture: 0 }); } catch { /* déjà ouverte */ }
  const e = await appeler("creer_employe", { fiche: { nom: awa, fonction: "vendeuse", salaire_mensuel: 60000, telephone: "76 00 11 22" } });
  const f = await appeler("preparer_fiche_paie", { employeId: e.id, du: `${mois}-01`, au: `${mois}-${fin}`, prorata: false });
  await appeler("valider_fiche_paie", { ficheId: f.id });
}, { awa, mois, fin });
erreurs.splice(0);

await page.getByRole("navigation", { name: "Menu Équipe" }).getByRole("button", { name: /Paie/ }).click();
await page.getByRole("tab", { name: "Fiches du mois" }).click();
await page.waitForTimeout(1500);
const net = t => t.replace(/\s/g, " ");
const statut = async () => net(await page.getByRole("status").innerText());
await page.getByTestId("fiche").filter({ hasText: awa }).click();
await page.waitForTimeout(1200);

// Premier versement : 20 000 en espèces.
await page.getByLabel("Montant à verser").fill("20000");
await page.getByRole("button", { name: /^Verser$/ }).click();
await page.waitForTimeout(1500);
ok((await statut()).includes(`20 000 F versés à ${awa}. Reste 40 000 F.`), "20 000 F versés, reste 40 000 F");
ok(net(await page.getByTestId("reste-a-verser").innerText()) === "40 000 F", "le reste à verser se lit sur la fiche");
ok(net(await page.getByTestId("fiche").filter({ hasText: awa }).innerText()).includes("reste 40 000 F"), "la liste le dit aussi");

// Trop : refusé.
const n0 = erreurs.length;
await page.getByLabel("Montant à verser").fill("50000");
await page.getByRole("button", { name: /^Verser$/ }).click();
await page.waitForTimeout(1200);
ok((await statut()).includes("Il ne reste que 40 000 F"), "verser plus que le reste : refusé");
erreurs.splice(n0);

// Le reste, en Orange Money (montant vide = tout ce qui reste).
await page.getByLabel("Montant à verser").fill("");
await page.getByLabel("Versé par").selectOption("orange_money");
await page.getByRole("button", { name: /^Verser$/ }).click();
await page.waitForTimeout(1500);
ok((await statut()).includes("Fiche payée"), "le reste versé : fiche payée");
ok(await page.getByTestId("versement").count() === 2 && await page.getByRole("button", { name: /^Verser$/ }).count() === 0, "deux versements, plus rien à verser");
ok(await page.getByTestId("fiche").filter({ hasText: awa }).getByText("payée").count() === 1, "« payée » dans la liste");

// Le bulletin.
await page.getByRole("button", { name: /Bulletin/ }).click();
await page.waitForTimeout(2000);
const bu = page.frameLocator('iframe[title="Bulletin de paie"]');
const texte = net(await bu.locator("body").innerText());
ok(texte.includes("BULLETIN DE PAIE") && texte.includes(awa) && texte.includes("PAIE-"), "le bulletin : titre, personne, numéro");
ok(texte.includes("Salaire du mois") && texte.includes("60 000 F") && /soixante mille francs CFA/i.test(texte), "la ligne, le net en chiffres et en lettres");
ok(texte.includes("Orange Money") && texte.includes("Payé"), "les versements et « Payé »");
ok(texte.includes("L'employé") && texte.includes("Pour la société"), "les deux signatures");
await capture(page, "g3-01-bulletin");
await page.getByRole("button", { name: "Fermer le bulletin" }).click();

// Dans Gescom : le bulletin se règle avec les autres documents.
await connecter(page, "admin", "admin123", "Admin-2026!", "Ma boutique", URL);
await parametres(page, "Documents");
await page.getByRole("tab", { name: "Bulletin de paie" }).click();
await page.waitForTimeout(1200);
ok(await page.getByRole("radio", { name: "Ticket 80 mm" }).count() === 0, "Documents → Bulletin de paie : pas de format rouleau");
ok(await page.frameLocator('iframe[title="Exemple de document"]').getByText("BULLETIN DE PAIE").count() === 1, "l'exemple du bulletin s'affiche");

const utiles = erreursUtiles(erreurs);
ok(utiles.length === 0, "aucune erreur dans la console" + (utiles.length ? " : " + utiles.join(" | ") : ""));
console.log(`${ok.total()} vérifications`);
await b.close();
