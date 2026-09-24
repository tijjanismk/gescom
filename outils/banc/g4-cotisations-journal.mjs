// G-4 : Equipe -> Paie -> Cotisations, vides par defaut, remplies a la
// main ; elles ne touchent que la personne declaree (retenue sur la
// fiche, charge patronale a part, bulletin) ; Gescom -> Rapports ->
// Journaux comptables : le journal de paie PA, equilibre, sur 661 / 422
// / 431 / 664.
import { navigateur, connecter, capture, verifieur, erreursUtiles, URL, URL_EQUIPE } from "./pw.mjs";
const { b, page, erreurs } = await navigateur();
const ok = verifieur();
const s = Date.now() % 100000;
const kadia = `Kadia ${s}`;
const awa = `Awa ${s}`;
const d = new Date();
const jour = d.toISOString().slice(0, 10);

await connecter(page, "admin", "admin123", "Admin-2026!", "Ma boutique", URL_EQUIPE);
// Repartir sans cotisation (un passage précédent du banc a pu en laisser).
await page.evaluate(async ({ kadia, awa }) => {
  const { appeler } = await import("/src/lib/pont.ts");
  for (const c of await appeler("lire_cotisations")) await appeler("retirer_cotisation", { cotisationId: c.id });
  await appeler("creer_employe", { fiche: { nom: kadia, fonction: "comptable", salaire_mensuel: 100000, declare: true, numero_inps: "998877" } });
  await appeler("creer_employe", { fiche: { nom: awa, fonction: "vendeuse", salaire_mensuel: 60000 } });
}, { kadia, awa });

await page.getByRole("navigation", { name: "Menu Équipe" }).getByRole("button", { name: /Paie/ }).click();
await page.getByRole("tab", { name: "Cotisations" }).click();
await page.waitForTimeout(1200);
ok(await page.getByTestId("aucune-cotisation").count() === 1, "vides par défaut : rien n'est retenu");
const net = t => t.replace(/\s/g, " ");
const statut = async () => net(await page.getByRole("status").innerText());
const ajouter = async (nom, qui, taux, plafond = "") => {
  await page.getByRole("button", { name: /Ajouter une cotisation/ }).click();
  await page.getByLabel("Nom de la cotisation").fill(nom);
  await page.getByLabel("Qui la paie").selectOption(qui);
  await page.getByLabel("Taux").fill(taux);
  await page.getByLabel("Plafond").fill(plafond);
  await page.getByRole("button", { name: "Enregistrer" }).click();
  await page.waitForTimeout(1200);
};
await ajouter("INPS part salariale", "salarie", "3,6");
ok((await statut()).includes("INPS part salariale enregistrée"), "une cotisation salariale saisie");
await ajouter("INPS part patronale", "employeur", "16,4", "80000");
ok(await page.getByTestId("cotisation").count() === 2 && net(await page.getByTestId("liste-cotisations").innerText()).includes("plafonné à 80 000 F"), "deux cotisations, le plafond dit");
const n0 = erreurs.length;
await ajouter("Mauvaise", "salarie", "150");
ok((await statut()).includes("entre 0 et 100"), "un taux de 150 % refusé");
erreurs.splice(n0);
await page.getByRole("button", { name: "Annuler" }).click();

// Les fiches : Kadia déclarée, Awa non.
await page.getByRole("tab", { name: "Fiches du mois" }).click();
await page.waitForTimeout(1500);
await page.getByRole("button", { name: `Préparer la fiche de ${kadia}` }).click();
await page.waitForTimeout(1500);
ok(net(await page.getByTestId("net-a-payer").innerText()) === "96 400 F", "Kadia déclarée : 100 000 F moins 3,6 %");
ok(net(await page.getByTestId("charges-patronales").innerText()).includes("13 120 F"), "la part patronale à part, plafonnée (16,4 % de 80 000 F)");
await page.getByRole("button", { name: /Valider la fiche/ }).click();
await page.waitForTimeout(1500);
await page.getByRole("button", { name: /Bulletin/ }).click();
await page.waitForTimeout(2000);
const bu = net(await page.frameLocator('iframe[title="Bulletin de paie"]').locator("body").innerText());
ok(bu.includes("INPS part salariale") && bu.includes("À la charge de l'employeur") && bu.includes("N° INPS 998877"), "le bulletin : la retenue, la charge pour information, le numéro INPS");
await capture(page, "g4-01-bulletin-declare");
await page.getByRole("button", { name: "Fermer le bulletin" }).click();
await page.getByRole("button", { name: `Préparer la fiche de ${awa}` }).click();
await page.waitForTimeout(1500);
ok(net(await page.getByTestId("net-a-payer").innerText()) === "60 000 F" && await page.getByTestId("charges-patronales").count() === 0, "Awa non déclarée : aucune cotisation");
await page.getByRole("button", { name: /Valider la fiche/ }).click();
await page.waitForTimeout(1200);

// Gescom : le journal de paie.
await connecter(page, "admin", "admin123", "Admin-2026!", "Ma boutique", URL);
await page.getByText("Rapports", { exact: true }).first().click();
await page.waitForTimeout(1200);
await page.getByRole("button", { name: /Journaux comptables/ }).click();
await page.waitForTimeout(1500);
await page.getByLabel("Journal").selectOption("PA");
await page.waitForTimeout(1500);
const pa = await page.getByTestId("journal-PA").innerText();
ok(pa.includes("équilibré") && !pa.includes("DÉSÉQUILIBRÉ"), "PA affiché, équilibré");
ok(["661", "422", "431", "664"].every(c => pa.includes(c)) && pa.includes(kadia), "salaires 661, dus 422, organismes 431, charges 664");
await capture(page, "g4-02-journal-pa");

// Le patron (qui a la paie) le reçoit aussi dans « Tous ».
const refus = await page.evaluate(async jour => {
  const { appeler } = await import("/src/lib/pont.ts");
  const tous = await appeler("lire_journaux_comptables", { du: jour, au: jour, journal: null });
  return tous.journaux.map(j => j.code).join(",");
}, jour);
ok(refus.includes("PA"), `le patron lit tous les journaux (${refus})`);

const utiles = erreursUtiles(erreurs);
ok(utiles.length === 0, "aucune erreur dans la console" + (utiles.length ? " : " + utiles.join(" | ") : ""));
console.log(`${ok.total()} vérifications`);
await b.close();
