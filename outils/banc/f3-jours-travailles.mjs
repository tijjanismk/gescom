// F-3 : Equipe -> Personnel -> Jours travailles. La grille du mois
// dernier : « tout le monde present » d'un clic sur le jour, une case
// qui tourne (P, ½, A), le total de la ligne qui suit ; un jour pas
// encore arrive ne se marque pas.
import { navigateur, connecter, capture, verifieur, erreursUtiles, URL_EQUIPE } from "./pw.mjs";
const { b, page, erreurs } = await navigateur();
const ok = verifieur();
const s = Date.now() % 100000;
const moussa = `Moussa ${s}`;

await connecter(page, "admin", "admin123", "Admin-2026!", "Ma boutique", URL_EQUIPE);
await page.evaluate(async nom => {
  const { appeler } = await import("/src/lib/pont.ts");
  await appeler("creer_employe", { fiche: { nom, fonction: "manœuvre", tarif_journalier: 2500 } });
}, moussa);

const d = new Date();
const dernier = new Date(d.getFullYear(), d.getMonth() - 1, 1);
const mois = `${dernier.getFullYear()}-${String(dernier.getMonth() + 1).padStart(2, "0")}`;

await page.getByRole("navigation", { name: "Menu Équipe" }).getByRole("button", { name: /Personnel/ }).click();
await page.getByRole("tab", { name: "Jours travaillés" }).click();
await page.waitForTimeout(1200);
await page.getByLabel("Mois").fill(mois);
await page.waitForTimeout(1500);
const ligne = page.getByTestId("ligne-jours").filter({ hasText: moussa });
ok(await ligne.count() === 1, "Moussa a sa ligne dans le mois dernier");
const total = async () => (await ligne.getByTestId("total-jours").innerText()).trim();
const t0 = await total();
ok(t0 === "0", `rien de marqué : 0 jour (${JSON.stringify(t0)})`);

await page.getByRole("button", { name: `Tous présents le ${mois}-03` }).click();
await page.waitForTimeout(1200);
ok(await total() === "1", "« tous présents » le 3 : 1 jour");
const cas = ligne.getByRole("button", { name: `${moussa} le ${mois}-04` });
await cas.click(); await page.waitForTimeout(900);
ok(await total() === "2" && (await cas.innerText()) === "P", "le 4, un clic : présent, 2 jours");
await cas.click(); await page.waitForTimeout(900);
ok(await total() === "1,5" && (await cas.innerText()) === "½", "deux clics : demi-journée, 1,5");
await cas.click(); await page.waitForTimeout(900);
ok(await total() === "1" && (await cas.innerText()) === "A", "trois clics : absent, 1");
// Un « tous présents » le 4 ne touche pas à l'absence.
await page.getByRole("button", { name: `Tous présents le ${mois}-04` }).click();
await page.waitForTimeout(1200);
ok((await cas.innerText()) === "A" && await total() === "1", "« tous présents » ne défait pas une absence");
await capture(page, "f3-01-grille");

// Ce mois-ci : demain ne se marque pas.
const demain = new Date(d.getFullYear(), d.getMonth(), d.getDate() + 1);
if (demain.getMonth() === d.getMonth()) {
  await page.getByLabel("Mois").fill(d.toISOString().slice(0, 7));
  await page.waitForTimeout(1500);
  const j = `${demain.getFullYear()}-${String(demain.getMonth() + 1).padStart(2, "0")}-${String(demain.getDate()).padStart(2, "0")}`;
  ok(await page.getByTestId("ligne-jours").filter({ hasText: moussa }).getByRole("button", { name: `${moussa} le ${j}` }).isDisabled(), "demain : pas encore arrivé, la case ne s'ouvre pas");
} else {
  ok(true, "(dernier jour du mois : pas de lendemain dans la grille)");
}

const utiles = erreursUtiles(erreurs);
ok(utiles.length === 0, "aucune erreur dans la console" + (utiles.length ? " : " + utiles.join(" | ") : ""));
console.log(`${ok.total()} vérifications`);
await b.close();
